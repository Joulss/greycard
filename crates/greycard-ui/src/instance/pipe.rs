//! The named pipe on Windows. Its name is the user's SID, the session
//! and the configuration folder's key; only the user and the system may
//! open it; a launch opens it for identification only, so the editor
//! cannot act as the launch, and checks the editor's process is the
//! same user before it sends. Every wait has an end: the pipe is
//! overlapped, and a read, a write or a connect that outlasts its time
//! is canceled.

use super::*;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::time::Instant;
use windows_sys::Win32::Foundation::{
    BOOL, ERROR_ACCESS_DENIED, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE, LocalFree, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX, ReadFile,
    SECURITY_IDENTIFICATION, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, GetNamedPipeServerProcessId,
    PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT, WaitNamedPipeW,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, OpenProcess, OpenProcessToken,
    PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow;

pub(super) fn user(key: &str) -> Option<Address> {
    let mut session = 0u32;
    // SAFETY: a plain out parameter; on failure it stays 0.
    unsafe { ProcessIdToSessionId(std::process::id(), &mut session) };
    match named(&format!("{session}-{key}")) {
        Ok(address) => Some(address),
        Err(e) => {
            tracing::warn!("no instance pipe: the user's SID could not be read: {e}");
            None
        }
    }
}

/// The pipe `\\.\pipe\greycard-<SID>-<suffix>`.
pub(super) fn named(suffix: &str) -> io::Result<Address> {
    let sid = Sid::of_current()?.string()?;
    let shown = format!(r"\\.\pipe\greycard-{sid}-{suffix}");
    Ok(Address {
        pipe: shown.encode_utf16().chain(std::iter::once(0)).collect(),
        sid,
        shown,
    })
}

fn last() -> io::Error {
    io::Error::last_os_error()
}

fn is(e: &io::Error, code: u32) -> bool {
    e.raw_os_error() == Some(code as i32)
}

/// A user's SID, as its process token holds it.
struct Sid {
    /// A `TOKEN_USER` and the SID after it, aligned for both.
    buf: Vec<u64>,
}

impl Sid {
    fn of_process(process: HANDLE) -> io::Result<Self> {
        let mut token: HANDLE = std::ptr::null_mut();
        // SAFETY: a process handle and a plain out parameter.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(last());
        }
        // SAFETY: a handle just opened, owned by nothing else.
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut len = 0u32;
        // SAFETY: asked with no buffer, for the length alone.
        unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                std::ptr::null_mut(),
                0,
                &mut len,
            )
        };
        if len == 0 {
            return Err(last());
        }
        let mut buf = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: a buffer of at least `len` bytes.
        if unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                buf.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        } == 0
        {
            return Err(last());
        }
        Ok(Self { buf })
    }

    fn of_current() -> io::Result<Self> {
        // SAFETY: the pseudo handle, which needs no closing.
        Self::of_process(unsafe { GetCurrentProcess() })
    }

    fn of_pid(pid: u32) -> io::Result<Self> {
        // SAFETY: a process id; the handle is checked.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return Err(last());
        }
        // SAFETY: a handle just opened, owned by nothing else.
        let process = unsafe { OwnedHandle::from_raw_handle(process) };
        Self::of_process(process.as_raw_handle())
    }

    fn psid(&self) -> PSID {
        // SAFETY: the buffer holds a TOKEN_USER, written by the system.
        unsafe { (*self.buf.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn same(&self, other: &Sid) -> bool {
        // SAFETY: two valid SIDs.
        unsafe { EqualSid(self.psid(), other.psid()) != 0 }
    }

    fn string(&self) -> io::Result<String> {
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: a valid SID and a plain out parameter.
        if unsafe { ConvertSidToStringSidW(self.psid(), &mut text) } == 0 {
            return Err(last());
        }
        // SAFETY: a nul-terminated string the system made, freed after.
        let string = unsafe {
            let len = (0..).take_while(|&i| *text.add(i) != 0).count();
            let string = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            string
        };
        Ok(string)
    }
}

/// The pipe's DACL: the user and the system, nobody else, not even to
/// read.
struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    fn private(sid: &str) -> io::Result<Self> {
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: a nul-terminated string and a plain out parameter.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(last());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: made by the system with LocalAlloc.
        unsafe { LocalFree(self.0) };
    }
}

/// An instance of the pipe, the first only if there is none yet:
/// overlapped, byte mode, local clients only, the user's alone.
fn make_pipe(address: &Address, first: bool) -> io::Result<OwnedHandle> {
    let descriptor = Descriptor::private(&address.sid)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let open = PIPE_ACCESS_DUPLEX
        | FILE_FLAG_OVERLAPPED
        | if first {
            FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            0
        };
    // SAFETY: a nul-terminated name and attributes that outlive the call.
    let handle = unsafe {
        CreateNamedPipeW(
            address.pipe.as_ptr(),
            open,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            &attributes,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(last());
    }
    // SAFETY: a handle just made, owned by nothing else.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// An overlapped pipe handle read and written with an end to every
/// wait: an operation still out at the deadline is canceled and is a
/// timeout.
struct Io {
    handle: OwnedHandle,
    event: OwnedHandle,
    deadline: Instant,
}

impl Io {
    fn new(handle: OwnedHandle, wait: Duration) -> io::Result<Self> {
        // SAFETY: a manual-reset event, unnamed, with no attributes.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(last());
        }
        Ok(Self {
            handle,
            // SAFETY: a handle just made, owned by nothing else.
            event: unsafe { OwnedHandle::from_raw_handle(event) },
            deadline: Instant::now() + wait,
        })
    }

    fn overlapped(&self) -> OVERLAPPED {
        // SAFETY: OVERLAPPED is plain data, zero its start.
        let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
        ov.hEvent = self.event.as_raw_handle();
        ov
    }

    fn h(&self) -> HANDLE {
        self.handle.as_raw_handle()
    }

    /// Wait on the event for at most `ms`: whether it came.
    fn signaled(&self, ms: u32) -> bool {
        // SAFETY: an event handle of ours.
        unsafe { WaitForSingleObject(self.event.as_raw_handle(), ms) == WAIT_OBJECT_0 }
    }

    /// The operation `ov` began canceled, and waited out so `ov` is
    /// free again.
    fn cancel(&self, ov: &OVERLAPPED) {
        let mut n = 0;
        // SAFETY: `ov` is the operation's, and lives through the wait.
        unsafe {
            CancelIoEx(self.h(), ov);
            GetOverlappedResult(self.h(), ov, &mut n, 1);
        }
    }

    /// An operation begun with `ov`, its bytes, waited for until the
    /// deadline.
    fn finish(&self, ov: &OVERLAPPED, begun: BOOL) -> io::Result<u32> {
        if begun == 0 {
            let e = last();
            if !is(&e, ERROR_IO_PENDING) {
                return Err(e);
            }
            let left = self.deadline.saturating_duration_since(Instant::now());
            if !self.signaled(u32::try_from(left.as_millis()).unwrap_or(u32::MAX)) {
                self.cancel(ov);
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "the other end of the pipe went quiet",
                ));
            }
        }
        let mut n = 0;
        // SAFETY: `ov` is the finished operation's.
        if unsafe { GetOverlappedResult(self.h(), ov, &mut n, 0) } == 0 {
            return Err(last());
        }
        Ok(n)
    }
}

impl Read for Io {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut ov = self.overlapped();
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: a buffer of `len` bytes and an OVERLAPPED that outlive
        // the operation, which `finish` waits out or cancels.
        let begun = unsafe {
            ReadFile(
                self.h(),
                buf.as_mut_ptr(),
                len,
                std::ptr::null_mut(),
                &raw mut ov,
            )
        };
        match self.finish(&ov, begun) {
            Ok(n) => Ok(n as usize),
            // The other end hung up: the end of what it sent.
            Err(e) if is(&e, ERROR_BROKEN_PIPE) => Ok(0),
            Err(e) => Err(e),
        }
    }
}

impl Write for Io {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut ov = self.overlapped();
        let len = u32::try_from(buf.len()).unwrap_or(u32::MAX);
        // SAFETY: as for the read.
        let begun = unsafe {
            WriteFile(
                self.h(),
                buf.as_ptr(),
                len,
                std::ptr::null_mut(),
                &raw mut ov,
            )
        };
        self.finish(&ov, begun).map(|n| n as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Connect first: an editor of this user that answers takes the
/// launch. No pipe of the name, and this launch makes the first
/// instance of it and is the editor; a pipe goes with its process, so
/// none is ever stale. The first instance is made only if no other is
/// there, so of two launches at once one makes it and the other
/// connects to it.
pub(super) fn claim(address: &Address, launch: &Launch) -> Claimed {
    let done = |claim| Claimed {
        claim,
        stale_removed: false,
    };
    for _ in 0..5 {
        match open(address) {
            Ok(pipe) => {
                return done(match hand_to(pipe, launch) {
                    Ok(pid) => Claim::Handed(pid),
                    Err(e) => Claim::Alone(e),
                });
            }
            Err(e) if is(&e, ERROR_PIPE_BUSY) => {
                // SAFETY: a nul-terminated name.
                unsafe { WaitNamedPipeW(address.pipe.as_ptr(), 2000) };
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => match make_pipe(address, true) {
                Ok(pipe) => {
                    return done(Claim::Instance(Instance {
                        pipe,
                        address: address.clone(),
                        removal: Served {
                            left: Arc::default(),
                        },
                    }));
                }
                // Another launch made it first: connect to it.
                Err(e) if is(&e, ERROR_ACCESS_DENIED) => {}
                Err(e) => return done(Claim::Alone(e)),
            },
            Err(e) => return done(Claim::Alone(e)),
        }
    }
    done(Claim::Alone(io::Error::new(
        io::ErrorKind::TimedOut,
        "the running editor's pipe stayed busy",
    )))
}

/// The pipe opened as a launch opens it: overlapped, and for
/// identification only, so the editor at the other end cannot act as
/// this user.
fn open(address: &Address) -> io::Result<std::fs::File> {
    std::fs::File::options()
        .read(true)
        .write(true)
        .custom_flags(FILE_FLAG_OVERLAPPED)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(&address.shown)
}

/// The launch handed over once the process at the other end is known
/// to be this user's: a pipe of the name made by another user first
/// is not sent a byte.
fn hand_to(pipe: std::fs::File, launch: &Launch) -> io::Result<u32> {
    let mut server = 0u32;
    // SAFETY: an open pipe handle and a plain out parameter.
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut server) } == 0 {
        return Err(last());
    }
    if !Sid::of_current()?.same(&Sid::of_pid(server)?) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the pipe is another user's",
        ));
    }
    // The foreground, which Windows gives only to the process the user
    // last started, this one, is the editor's to take.
    // SAFETY: a process id.
    unsafe { AllowSetForegroundWindow(server) };
    let mut io = Io::new(OwnedHandle::from(pipe), ANSWER_WAIT)?;
    hand(&mut io, launch)
}

enum Connected {
    Yes,
    /// The editor is closing.
    Stop,
    Failed(io::Error),
}

/// Wait for a launch on this instance, looking every so often whether
/// the editor is closing.
fn connect(io: &Io, left: &AtomicBool) -> Connected {
    let mut ov = io.overlapped();
    // SAFETY: an OVERLAPPED that outlives the operation, which is
    // waited out or canceled below.
    if unsafe { ConnectNamedPipe(io.h(), &raw mut ov) } != 0 {
        return Connected::Yes;
    }
    let e = last();
    if is(&e, ERROR_PIPE_CONNECTED) {
        return Connected::Yes;
    }
    if !is(&e, ERROR_IO_PENDING) {
        return Connected::Failed(e);
    }
    loop {
        if io.signaled(200) {
            let mut n = 0;
            // SAFETY: the finished operation's OVERLAPPED.
            return if unsafe { GetOverlappedResult(io.h(), &ov, &mut n, 0) } != 0 {
                Connected::Yes
            } else {
                Connected::Failed(last())
            };
        }
        if left.load(Ordering::SeqCst) {
            io.cancel(&ov);
            return Connected::Stop;
        }
    }
}

/// Each instance waits for a launch; once one connects the next is
/// made before this one is read, so there is always one to connect to
/// and the name is never free for another to take. A launch gone
/// before it was listened to (ERROR_NO_DATA) leaves its instance
/// unusable: a fresh one takes its place, made before it closes.
pub(super) fn serve(
    first: OwnedHandle,
    address: &Address,
    left: &AtomicBool,
    take: impl Fn(Launch) -> bool,
) {
    let mut current = Some(first);
    while let Some(pipe) = current.take() {
        if left.load(Ordering::SeqCst) {
            return;
        }
        let mut io = match Io::new(pipe, READ_WAIT) {
            Ok(io) => io,
            Err(e) => {
                tracing::warn!("the instance pipe cannot wait: {e}; no launch is heard");
                return;
            }
        };
        match connect(&io, left) {
            Connected::Yes => {}
            Connected::Stop => return,
            Connected::Failed(e) => {
                tracing::info!("a launch could not be accepted: {e}");
                current = match make_pipe(address, false) {
                    Ok(fresh) => Some(fresh),
                    Err(made) => {
                        tracing::warn!("no fresh instance pipe: {made}");
                        std::thread::sleep(Duration::from_millis(100));
                        // SAFETY: an instance handle of ours.
                        unsafe { DisconnectNamedPipe(io.h()) };
                        Some(io.handle)
                    }
                };
                continue;
            }
        }
        current = match make_pipe(address, false) {
            Ok(next) => Some(next),
            Err(e) => {
                tracing::warn!("no pipe for the next launch: {e}; this one is the last");
                None
            }
        };
        io.deadline = Instant::now() + READ_WAIT;
        // An answer read before the handle closes: wait for the launch
        // to hang up, as it does once it has read it, and no longer
        // than a read's time for one that never does. Unanswered, it
        // is let go at once.
        if serve_one(&mut io, left, &take) {
            io.deadline = Instant::now() + READ_WAIT;
            let mut rest = [0u8; 64];
            while matches!(io.read(&mut rest), Ok(n) if n > 0) {}
        }
    }
}

/// A bare connection to the editor, for a test to send on in its own
/// time.
#[cfg(test)]
pub(super) fn connect_bare(address: &Address) -> io::Result<impl Read + Write> {
    Io::new(OwnedHandle::from(open(address)?), ANSWER_WAIT)
}
