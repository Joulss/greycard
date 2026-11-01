//! The Unix socket on Linux and macOS: in the runtime folder when it is
//! the user's alone, else in the cache folder, readable and writable by
//! the user only, and each side's peer checked to be the same user.

use super::*;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Instant;

pub(super) fn user(key: &str) -> Option<Address> {
    socket_path(
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from),
        dirs::cache_dir(),
        uid(),
        key,
    )
    .map(at)
}

/// A socket at `path`.
pub(super) fn at(socket: PathBuf) -> Address {
    Address {
        shown: socket.display().to_string(),
        socket,
    }
}

/// The socket's path: `greycard-<uid>-<key>.sock` in the runtime
/// folder when there is one that is absolute, the user's, and open to
/// nobody else (as the base directory spec asks of it, and a desktop
/// that sets it to something shared does not keep), else in the cache
/// folder's `greycard`. `key` is the configuration folder's
/// ([`config_key`]).
pub fn socket_path(
    runtime: Option<PathBuf>,
    cache: Option<PathBuf>,
    uid: u32,
    key: &str,
) -> Option<PathBuf> {
    let name = format!("greycard-{uid}-{key}.sock");
    match runtime.filter(|r| r.is_absolute() && private_to(r, uid)) {
        Some(runtime) => Some(runtime.join(name)),
        None => cache
            .filter(|c| c.is_absolute())
            .map(|c| c.join("greycard").join(name)),
    }
}

/// A folder the user owns that no one else may enter.
fn private_to(dir: &Path, uid: u32) -> bool {
    std::fs::metadata(dir).is_ok_and(|m| m.is_dir() && m.uid() == uid && m.mode() & 0o077 == 0)
}

fn uid() -> u32 {
    // SAFETY: getuid cannot fail and touches no memory of ours.
    unsafe { libc::getuid() }
}

/// The user at the other end of a connected socket.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: a connected socket and a buffer of the size given.
    let got = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast(),
            &mut len,
        )
    };
    if got != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(cred.uid)
}

/// The user at the other end of a connected socket.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn peer_uid(stream: &UnixStream) -> io::Result<u32> {
    let (mut uid, mut gid) = (u32::MAX, u32::MAX);
    // SAFETY: a connected socket and two plain out parameters.
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

/// The other end is this user, or the error that says it is not.
fn same_user(stream: &UnixStream) -> io::Result<()> {
    let peer = peer_uid(stream)?;
    if peer != uid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("the other end is user {peer}"),
        ));
    }
    Ok(())
}

/// The socket file's device and inode, which say whether it is still
/// the one this editor bound.
fn identity(path: &Path) -> io::Result<(u64, u64)> {
    let meta = std::fs::symlink_metadata(path)?;
    Ok((meta.dev(), meta.ino()))
}

/// The socket this editor bound taken away; one put there since by
/// another (after a stale removal) is that one's and stays.
pub(super) fn remove(path: &Path, made: (u64, u64)) {
    if identity(path).ok() == Some(made) {
        match std::fs::remove_file(path) {
            Ok(()) => tracing::info!("instance socket removed: {}", path.display()),
            Err(e) => tracing::warn!("instance socket {}: {e}", path.display()),
        }
    }
}

/// Connect first: an editor of this user that answers takes the launch.
/// No socket, or one nobody listens on (an editor that crashed left
/// it), and this launch binds and becomes the editor, a stale socket
/// removed first. The look and the bind are made under a lock beside
/// the socket, so two launches at once cannot both find nobody: the
/// second waits, then finds the first.
pub(super) fn claim(address: &Address, launch: &Launch) -> Claimed {
    let alone = |e, stale_removed| Claimed {
        claim: Claim::Alone(e),
        stale_removed,
    };
    let socket = &address.socket;
    if let Some(dir) = socket.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        return alone(e, false);
    }
    let lock = match std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(socket.with_extension("lock"))
    {
        Ok(lock) => lock,
        Err(e) => return alone(e, false),
    };
    // Held only for a look and a bind; one held longer is a launch
    // stopped in the middle, and this one goes on alone.
    let since = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) if since.elapsed() < ANSWER_WAIT => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return alone(
                    io::Error::new(io::ErrorKind::TimedOut, "the instance lock stayed held"),
                    false,
                );
            }
            Err(std::fs::TryLockError::Error(e)) => return alone(e, false),
        }
    }
    let mut stale_removed = false;
    match UnixStream::connect(socket) {
        Ok(mut stream) => {
            drop(lock);
            let handed = same_user(&stream)
                .and_then(|()| stream.set_read_timeout(Some(ANSWER_WAIT)))
                .and_then(|()| stream.set_write_timeout(Some(ANSWER_WAIT)))
                .and_then(|()| hand(&mut stream, launch));
            return match handed {
                Ok(pid) => Claimed {
                    claim: Claim::Handed(pid),
                    stale_removed,
                },
                Err(e) => alone(e, stale_removed),
            };
        }
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            if let Err(e) = std::fs::remove_file(socket) {
                return alone(e, false);
            }
            stale_removed = true;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return alone(e, false),
    }
    let made = UnixListener::bind(socket).and_then(|listener| {
        // Others may not connect at all; the peer check is behind it.
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
        Ok((listener, identity(socket)?))
    });
    match made {
        Ok((listener, made)) => Claimed {
            claim: Claim::Instance(Instance {
                listener,
                address: address.clone(),
                removal: Served {
                    left: Arc::default(),
                    socket: Some((socket.clone(), made)),
                },
            }),
            stale_removed,
        },
        Err(e) => alone(e, stale_removed),
    }
}

pub(super) fn serve(listener: UnixListener, left: &AtomicBool, take: impl Fn(Launch) -> bool) {
    for stream in listener.incoming() {
        if left.load(Ordering::SeqCst) {
            // Closing: the launches still waiting are let go, each to
            // a window of its own.
            return;
        }
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                // Out of descriptors, say: not a reason to spin.
                tracing::warn!("a launch could not be accepted: {e}");
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        if let Err(e) = same_user(&stream) {
            tracing::warn!("a connection to the instance socket refused: {e}");
            continue;
        }
        if let Err(e) = stream
            .set_read_timeout(Some(READ_WAIT))
            .and_then(|()| stream.set_write_timeout(Some(READ_WAIT)))
        {
            tracing::warn!("a launch could not be read: {e}");
            continue;
        }
        serve_one(&mut stream, left, &take);
    }
}

/// A bare connection to the editor, for a test to send on in its own
/// time.
#[cfg(test)]
pub(super) fn connect_bare(address: &Address) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(&address.socket)?;
    stream.set_read_timeout(Some(ANSWER_WAIT))?;
    Ok(stream)
}
