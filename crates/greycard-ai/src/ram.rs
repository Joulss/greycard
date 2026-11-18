//! The machine's memory, for what a model or the editor's worker may
//! keep in it: what it has ([`physical`]) and what it could still give
//! ([`available`]).
//!
//! Asked of the system once per call: `sysconf` on Linux, the Mac, and
//! the other Unixes, `GlobalMemoryStatusEx` on Windows. `None` where
//! the answer is not to be had, and the caller keeps to its smallest
//! use.

/// The bytes of physical memory the machine has, or `None` when the
/// system will not say.
pub fn physical() -> Option<u64> {
    let bytes = query()?;
    (bytes > 0).then_some(bytes)
}

/// The bytes the system could give a process now without swapping:
/// `MemAvailable` on Linux, the share of physical memory
/// `kern.memorystatus_level` says is free on the Mac, `ullAvailPhys`
/// on Windows. [`physical`] where the system has no such reader or it
/// fails.
pub fn available() -> Option<u64> {
    query_available()
        .filter(|&bytes| bytes > 0)
        .or_else(physical)
}

#[cfg(unix)]
fn query() -> Option<u64> {
    // SAFETY: `sysconf` reads a system constant and has no
    // preconditions; a name it does not know answers -1.
    let (pages, size) = unsafe {
        (
            libc::sysconf(libc::_SC_PHYS_PAGES),
            libc::sysconf(libc::_SC_PAGESIZE),
        )
    };
    let pages = u64::try_from(pages).ok()?;
    let size = u64::try_from(size).ok()?;
    pages.checked_mul(size)
}

#[cfg(windows)]
fn memory_status() -> Option<windows_sys::Win32::System::SystemInformation::MEMORYSTATUSEX> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    // SAFETY: the struct is plain data, zero is a valid value for each
    // field, and its length is set as the call requires before it.
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is a live, writable MEMORYSTATUSEX with its
    // length set.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0).then_some(status)
}

#[cfg(windows)]
fn query() -> Option<u64> {
    memory_status().map(|s| s.ullTotalPhys)
}

#[cfg(not(any(unix, windows)))]
fn query() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
fn query_available() -> Option<u64> {
    mem_available(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

/// The Mac's memory pressure as the share of physical memory free, in
/// percent, read by `sysctlbyname`; the kernel keeps it for its own
/// pressure notifications.
#[cfg(target_os = "macos")]
fn query_available() -> Option<u64> {
    let mut level: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    // SAFETY: the name is a NUL-terminated string, `level` is a live
    // int and `size` its length, and nothing is written to the system.
    let failed = unsafe {
        libc::sysctlbyname(
            c"kern.memorystatus_level".as_ptr(),
            (&raw mut level).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if failed != 0 || size != std::mem::size_of::<libc::c_int>() {
        return None;
    }
    let level = u64::try_from(level).ok().filter(|&l| l <= 100)?;
    Some(physical()? / 100 * level)
}

#[cfg(windows)]
fn query_available() -> Option<u64> {
    memory_status().map(|s| s.ullAvailPhys)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn query_available() -> Option<u64> {
    None
}

/// `MemAvailable` out of the text of `/proc/meminfo`, in bytes: the
/// kernel's estimate of what could be given without swapping, the
/// reclaimable caches counted in.
#[cfg(any(target_os = "linux", test))]
fn mem_available(meminfo: &str) -> Option<u64> {
    meminfo.lines().find_map(|line| {
        let mut words = line.strip_prefix("MemAvailable:")?.split_whitespace();
        let amount: u64 = words.next()?.parse().ok()?;
        match words.next() {
            Some("kB") => amount.checked_mul(1024),
            None => Some(amount),
            Some(_) => None,
        }
    })
}

#[cfg(test)]
mod tests {
    /// This machine says how much memory it has, and it is more than
    /// a gigabyte and less than a petabyte: a number read in the
    /// right unit.
    #[test]
    fn the_machine_says_how_much_memory_it_has() {
        let bytes = super::physical().expect("the system says");
        assert!(bytes > 1 << 30, "{bytes}");
        assert!(bytes < 1 << 50, "{bytes}");
    }

    /// What it could give is something, and no more than it has.
    #[test]
    fn the_machine_says_what_it_could_give() {
        let available = super::available().expect("the system says");
        let physical = super::physical().expect("the system says");
        assert!(
            available > 0 && available <= physical,
            "{available} of {physical}"
        );
    }

    /// `MemAvailable` read in kilobytes, wherever it sits in the file,
    /// and nothing made of a file without it or a line gone wrong.
    #[test]
    fn mem_available_is_read_from_meminfo() {
        let meminfo = "MemTotal:       65536000 kB\n\
                       MemFree:         7340032 kB\n\
                       MemAvailable:   33554432 kB\n\
                       Buffers:          123456 kB\n";
        assert_eq!(super::mem_available(meminfo), Some(33_554_432 * 1024));
        assert_eq!(
            super::mem_available("MemTotal: 1 kB\nMemFree: 1 kB\n"),
            None
        );
        assert_eq!(super::mem_available("MemAvailable: lots kB\n"), None);
        assert_eq!(super::mem_available("MemAvailable: 12 MB\n"), None);
        assert_eq!(super::mem_available("MemAvailable: 4096\n"), Some(4096));
        assert_eq!(super::mem_available("MemAvailableX: 1 kB\n"), None);
    }
}
