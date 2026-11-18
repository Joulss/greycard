//! Whether a root is on a network filesystem. On Linux and macOS a
//! watch is no use there: inotify (and FSEvents) fire for what this
//! machine does through the mount, never for what the server or
//! another machine does, and setting a recursive inotify watch up
//! walks every folder under the root, a round trip each. A root on one
//! is not watched there. It is brought up to date by the pass at
//! launch, by the list's read at each report, and by the timer
//! (`roots::Poll`). On Windows a share is watched like any folder:
//! ReadDirectoryChangesW over SMB is one call a root, and the server
//! sends its changes back (SMB's CHANGE_NOTIFY). There the answer is
//! only said in the log.
//!
//! On Linux the mount table says: the longest mount point the root is
//! under, the later of two at the same point (an automount's `autofs`
//! entry comes before the share mounted over it), and its type. On
//! macOS `statfs` on the root says, by `MNT_LOCAL` and the type's name;
//! the prefetch's [`not_to_peek`], which must never wait on a share,
//! reads the list of mounts instead (`getfsstat`, `MNT_NOWAIT`) and
//! answers skip where that list cannot say for sure. On Windows a UNC
//! path is remote, and a drive letter is asked of `GetDriveTypeW`, for
//! both.
//!
//! A share can also be mounted below a local root (`~/Pictures/NAS`
//! under `~/Pictures`). On Linux every mount point under a root is
//! classified the same way ([`remote_under`]), and a network one is
//! left out of the root's watch and passed over on the timer.

use std::path::{Path, PathBuf};

/// Whether a filesystem type, as the mount table or `statfs` names it,
/// is one whose changes are made elsewhere: a network or cluster
/// filesystem, a virtual machine's shared folder, or anything through
/// FUSE (sshfs, rclone, davfs2, gvfs, s3fs) but `fuseblk`, which is a
/// local disk (ntfs-3g, exfat-fuse), and the desktop's document
/// portal. Windows asks the drive's type instead.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn network(fs: &str) -> bool {
    const NETWORK: &[&str] = &[
        // NFS, SMB and their kin.
        "nfs",
        "nfs4",
        "cifs",
        "smb",
        "smb2",
        "smb3",
        "smbfs",
        "ncp",
        "ncpfs",
        "afs",
        "coda",
        "afpfs",
        "webdav",
        "davfs",
        "ftp",
        "sshfs",
        // Cluster and distributed filesystems.
        "ceph",
        "glusterfs",
        "lustre",
        "gpfs",
        "beegfs",
        "orangefs",
        "pvfs2",
        "moosefs",
        "ocfs2",
        "gfs2",
        // A virtual machine's view of its host's folders.
        "9p",
        "virtiofs",
        "vboxsf",
        "vmhgfs",
        "prl_fs",
        // FUSE with its type unsaid, and macOS's.
        "fuse",
        "macfuse",
        "osxfuse",
    ];
    let fs = fs.to_ascii_lowercase();
    NETWORK.contains(&fs.as_str()) || (fs.starts_with("fuse.") && fs != "fuse.portal")
}

/// A mount table's field as the kernel escapes it: a space, a tab, a
/// newline and a backslash as three octal digits after a backslash.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            // Three digits up to \377, a byte; \4xx and above is not an
            // escape the kernel writes, and is left as it is.
            && (b'0'..=b'3').contains(&bytes[i + 1])
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let v = (bytes[i + 1] - b'0') * 64 + (bytes[i + 2] - b'0') * 8 + (bytes[i + 3] - b'0');
            out.push(v);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The mount `path` is on, by a mount table in `/proc/self/mounts`'s
/// format: its mount point and its type. The longest mount point that
/// holds the path, and of two alike the later, which is mounted over
/// the earlier. Linux's alone; the other platforms' tests use it.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn mount_of(table: &str, path: &Path) -> Option<(PathBuf, String)> {
    let mut best: Option<(PathBuf, String)> = None;
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_device), Some(point), Some(fs)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let point = PathBuf::from(unescape(point));
        if !path.starts_with(&point) {
            continue;
        }
        let longer = best
            .as_ref()
            .is_none_or(|(b, _)| point.as_os_str().len() >= b.as_os_str().len());
        if longer {
            best = Some((point, fs.to_string()));
        }
    }
    best
}

/// The network mounts below `root`, by a mount table in
/// `/proc/self/mounts`'s format: each mount point under the root (not
/// the root's own), with the type that is mounted there as [`mount_of`]
/// reads it (the later of two at one point), when that type is a
/// network one, and with none when only an automount is there so far
/// (`autofs`, a systemd `x-systemd.automount` before its first use):
/// what it will mount is not known until a look into it mounts it, which
/// the watcher's build does on purpose (`roots::mount_on_purpose`) and
/// then reads the table again. Only the outermost: a
/// mount inside one already found is under it, and goes with it. A
/// local disk mounted inside a share is inside the share's subtree, and
/// goes with it too.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn remote_under_in(table: &str, root: &Path) -> Vec<(PathBuf, Option<String>)> {
    let mut points: Vec<PathBuf> = Vec::new();
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_device), Some(point)) = (fields.next(), fields.next()) else {
            continue;
        };
        let point = PathBuf::from(unescape(point));
        if point != root && point.starts_with(root) && !points.contains(&point) {
            points.push(point);
        }
    }
    // Outermost first, so a mount inside a share found is passed over.
    points.sort_by_key(|p| p.components().count());
    let mut found: Vec<(PathBuf, Option<String>)> = Vec::new();
    for point in points {
        if found.iter().any(|(f, _)| point.starts_with(f)) {
            continue;
        }
        match mount_of(table, &point) {
            Some((at, fs)) if at == point && network(&fs) => found.push((point, Some(fs))),
            Some((at, fs)) if at == point && fs == "autofs" => found.push((point, None)),
            _ => {}
        }
    }
    found
}

/// The network mounts below `root` (canonical, as the roots are): on
/// Linux from the mount table, as [`remote_under_in`] reads it. Elsewhere
/// none is found: macOS mounts its shares under `/Volumes`, and Windows
/// watches a share as any folder. The binary's tests swap it out
/// (`roots::remote_under_fs`).
pub fn remote_under(root: &Path) -> Vec<(PathBuf, Option<String>)> {
    #[cfg(target_os = "linux")]
    {
        let Ok(table) = std::fs::read_to_string("/proc/self/mounts")
            .or_else(|_| std::fs::read_to_string("/proc/mounts"))
        else {
            return Vec::new();
        };
        remote_under_in(&table, root)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = root;
        Vec::new()
    }
}

/// The type of the network filesystem `root` is on, or none when it is
/// on a local one or it cannot be told. `root` is canonical, as the
/// roots are. It may ask the disk (`statfs` on macOS, `GetDriveTypeW`
/// on Windows), so it is asked off the window's thread. The binary's
/// tests swap it out (`roots::remote_fs`).
pub fn remote(root: &Path) -> Option<String> {
    remote_on(root)
}

/// Why the folder `folder` is not to be read ahead of the user (the
/// prefetch's peek at a neighbor's sidecar, on the worker, which must
/// never wait on a share or mount one): `a network mount (nfs4)`, or `an
/// automount not mounted yet`, whose first read would mount it and wait
/// on the server; none for a local folder. `folder` is taken as it
/// stands (a symlink into a share is not followed), made absolute and
/// its `.` and `..` resolved by name alone ([`whole`]). Asked of the
/// kernel's list of mounts alone, never the mount, every time (it is
/// local and cheap), so a share mounted since is seen at the next ask.
/// What cannot be told is not read.
pub fn not_to_peek(folder: &Path) -> Option<String> {
    match whole(folder) {
        Some(folder) => not_to_peek_on(&folder),
        None => Some("a folder whose path cannot be resolved".into()),
    }
}

/// `path` made absolute against the working folder (`.` for an empty
/// one, the parent of a bare file name), and its `.` and `..` resolved
/// by name, with no disk asked: `/home/u/../../mnt/x` is `/mnt/x`. None
/// when it cannot be made absolute, or climbs above its root.
fn whole(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let absolute = std::path::absolute(path).ok()?;
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for part in absolute.components() {
        match part {
            Component::Prefix(_) | Component::RootDir => out.push(part.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None;
                }
                out.pop();
                depth -= 1;
            }
            Component::Normal(name) => {
                out.push(name);
                depth += 1;
            }
        }
    }
    Some(out)
}

/// [`not_to_peek`] by a mount table in `/proc/self/mounts`'s format, for
/// an absolute `folder`. A table that says nothing of it is not trusted.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn not_to_peek_in(table: &str, folder: &Path) -> Option<String> {
    match mount_of(table, folder) {
        None => Some("a mount the table does not list".into()),
        Some((_, fs)) => peek_answer(&fs, !network(&fs)),
    }
}

/// [`not_to_peek`]'s answer for a folder on a filesystem of type `fs`,
/// `local` as the platform says.
#[cfg_attr(windows, allow(dead_code))]
fn peek_answer(fs: &str, local: bool) -> Option<String> {
    if fs == "autofs" {
        Some("an automount not mounted yet".into())
    } else if !local || network(fs) {
        Some(format!("a network mount ({fs})"))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn mount_table() -> Option<String> {
    std::fs::read_to_string("/proc/self/mounts")
        .or_else(|_| std::fs::read_to_string("/proc/mounts"))
        .ok()
}

#[cfg(target_os = "linux")]
fn remote_on(root: &Path) -> Option<String> {
    let (_, fs) = mount_of(&mount_table()?, root)?;
    network(&fs).then_some(fs)
}

#[cfg(target_os = "linux")]
fn not_to_peek_on(folder: &Path) -> Option<String> {
    match mount_table() {
        Some(table) => not_to_peek_in(&table, folder),
        None => Some("a mount whose table cannot be read".into()),
    }
}

#[cfg(target_os = "macos")]
fn remote_on(root: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(root.as_os_str().as_bytes()).ok()?;
    // SAFETY: statfs is plain data, and all zeroes is a valid one.
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: a NUL-terminated path and a buffer of the right type.
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let name: Vec<u8> = st
        .f_fstypename
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let fs = String::from_utf8_lossy(&name).into_owned();
    let local = st.f_flags & (libc::MNT_LOCAL as u32) != 0;
    (!local || network(&fs)).then_some(fs)
}

/// Each mounted filesystem as macOS's list of them says: its mount
/// point's bytes, its type, and whether it is local. `getfsstat` with
/// `MNT_NOWAIT` returns what the kernel holds of each mount without
/// asking any of them, so a share whose server is gone does not hold
/// it, where `statfs` on the path would; it fills a buffer of our own,
/// unlike `getmntinfo`'s shared one, so two threads may ask at once.
/// Compiled for macOS here, never run on a Mac.
#[cfg(target_os = "macos")]
fn mounts_on_mac() -> Option<Vec<(Vec<u8>, String, bool)>> {
    let name = |field: &[libc::c_char]| -> Vec<u8> {
        field
            .iter()
            .take_while(|&&c| c != 0)
            .map(|&c| c as u8)
            .collect()
    };
    // SAFETY: a null buffer asks only how many mounts there are.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if count <= 0 {
        return None;
    }
    // Room for a few more, mounted between the two calls.
    let room = count as usize + 8;
    // SAFETY: statfs is plain data, and all zeroes is a valid one.
    let mut all: Vec<libc::statfs> = vec![unsafe { std::mem::zeroed() }; room];
    let bytes = (room * std::mem::size_of::<libc::statfs>()) as libc::c_int;
    // SAFETY: a buffer of `room` statfs, its size in bytes given.
    let filled = unsafe { libc::getfsstat(all.as_mut_ptr(), bytes, libc::MNT_NOWAIT) };
    if filled <= 0 {
        return None;
    }
    all.truncate(filled as usize);
    Some(
        all.iter()
            .map(|st| {
                let fs = String::from_utf8_lossy(&name(&st.f_fstypename)).into_owned();
                let local = st.f_flags & (libc::MNT_LOCAL as u32) != 0;
                (name(&st.f_mntonname), fs, local)
            })
            .collect(),
    )
}

#[cfg(target_os = "macos")]
fn not_to_peek_on(folder: &Path) -> Option<String> {
    use std::os::unix::ffi::OsStrExt;
    match mounts_on_mac() {
        Some(mounts) => not_to_peek_mac(&mounts, folder.as_os_str().as_bytes()),
        None => Some("a mount the system does not list".into()),
    }
}

/// [`not_to_peek`] on macOS by the list of mounts `mounts` (each one's
/// point, type, and whether it is local), for the absolute `path`. The
/// longest mount point that holds the path by its bytes is the one. A
/// match that may be wrong answers skip: a path under the system's
/// share and home folders (`/Volumes`, `/home`, `/net`, `/Network`)
/// found on the boot volume's `/` or `/System/Volumes/Data` (a firmlink
/// or an automounted home the byte match does not see), or no match.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn not_to_peek_mac(mounts: &[(Vec<u8>, String, bool)], path: &[u8]) -> Option<String> {
    let under = |point: &[u8]| {
        path.starts_with(point)
            && (point == b"/" || path.len() == point.len() || path.get(point.len()) == Some(&b'/'))
    };
    let Some((point, fs, local)) = mounts
        .iter()
        .filter(|(point, _, _)| under(point))
        .max_by_key(|(point, _, _)| point.len())
    else {
        return Some("a mount the system does not list".into());
    };
    let on_boot = point.as_slice() == b"/" || point.as_slice() == b"/System/Volumes/Data";
    // The data volume's own spelling of the same folders, as well.
    let rest = path
        .strip_prefix(b"/System/Volumes/Data".as_slice())
        .filter(|rest| rest.first() == Some(&b'/'))
        .unwrap_or(path);
    let shared = [b"/Volumes".as_slice(), b"/home", b"/net", b"/Network"]
        .iter()
        .any(|top| rest.starts_with(top) && matches!(rest.get(top.len()), None | Some(b'/')));
    if on_boot && shared {
        return Some("a folder whose mount cannot be told for sure".into());
    }
    peek_answer(fs, *local)
}

/// On Windows a UNC path is told from the path alone. A drive letter
/// is asked of `GetDriveTypeW` with the drive's root (`X:\`): it answers
/// from the type of the device the letter is assigned to, which the
/// I/O manager holds, and opens no file on the volume, so a mapped
/// drive whose server is gone answers without a round trip to it (a
/// letter whose mapping is being restored may still wait on the
/// redirector; this is Microsoft's described behavior, not measured
/// here).
#[cfg(windows)]
fn not_to_peek_on(folder: &Path) -> Option<String> {
    remote_on(folder).map(|kind| format!("a network mount ({kind})"))
}

#[cfg(windows)]
fn remote_on(root: &Path) -> Option<String> {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    // `GetDriveTypeW`'s answer for a network drive. It is in the
    // WindowsProgramming feature, which nothing else here needs.
    const DRIVE_REMOTE: u32 = 4;
    let Some(Component::Prefix(prefix)) = root.components().next() else {
        return None;
    };
    let letter = match prefix.kind() {
        Prefix::UNC(..) | Prefix::VerbatimUNC(..) => return Some("UNC".into()),
        Prefix::Disk(l) | Prefix::VerbatimDisk(l) => l,
        _ => return None,
    };
    let drive: Vec<u16> = std::ffi::OsStr::new(&format!("{}:\\", letter as char))
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: a NUL-terminated wide string.
    let kind = unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(drive.as_ptr()) };
    (kind == DRIVE_REMOTE).then(|| "network drive".into())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn remote_on(_root: &Path) -> Option<String> {
    None
}

/// Where nothing can say what a folder is on, nothing is read ahead.
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn not_to_peek_on(_folder: &Path) -> Option<String> {
    Some("a mount this system cannot name".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = "\
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
/dev/nvme0n1p2 / btrfs rw,noatime,compress=zstd:1,ssd,subvol=/@ 0 0
/dev/nvme0n1p2 /home btrfs rw,noatime,compress=zstd:1,ssd,subvol=/@home 0 0
tmpfs /tmp tmpfs rw,nosuid,nodev,size=32G 0 0
systemd-1 /mnt/archive autofs rw,relatime,fd=48,pgrp=1,timeout=0 0 0
nas:/volume1/archive /mnt/archive nfs4 rw,relatime,vers=4.1,hard,proto=tcp 0 0
//nas/photos /mnt/photos cifs rw,relatime,vers=3.1.1,cache=strict 0 0
/dev/sdb1 /mnt/photos/local ext4 rw,relatime 0 0
jess@host:/srv /home/jess/remote fuse.sshfs rw,nosuid,nodev,user_id=1000 0 0
gvfsd-fuse /run/user/1000/gvfs fuse.gvfsd-fuse rw,nosuid,nodev 0 0
/dev/sdc1 /run/media/jess/My\\040Card exfat rw,nosuid,nodev 0 0
/dev/sdd1 /run/media/jess/Old\\040Drive fuseblk rw,nosuid,nodev 0 0
nas:/share /mnt/share\\040two nfs rw 0 0
";

    fn fs_of(path: &str) -> Option<String> {
        mount_of(TABLE, Path::new(path)).map(|(_, fs)| fs)
    }

    fn remote_in_table(path: &str) -> bool {
        fs_of(path).is_some_and(|fs| network(&fs))
    }

    #[test]
    fn the_types_whose_changes_are_made_elsewhere() {
        for fs in [
            "nfs",
            "nfs4",
            "cifs",
            "smb3",
            "smbfs",
            "fuse.sshfs",
            "fuse.rclone",
            "fuse.gvfsd-fuse",
            "davfs",
            "afs",
            "ceph",
            "9p",
            "glusterfs",
            "virtiofs",
            "NFS4",
        ] {
            assert!(network(fs), "{fs}");
        }
        for fs in [
            "ext4",
            "btrfs",
            "xfs",
            "zfs",
            "exfat",
            "vfat",
            "ntfs3",
            "fuseblk",
            "tmpfs",
            "apfs",
            "hfs",
            "fuse.portal",
            "overlay",
            "autofs",
        ] {
            assert!(!network(fs), "{fs}");
        }
    }

    #[test]
    fn a_root_takes_the_longest_mount_it_is_under() {
        assert_eq!(fs_of("/home/jess/Pictures").as_deref(), Some("btrfs"));
        assert_eq!(fs_of("/").as_deref(), Some("btrfs"));
        assert_eq!(fs_of("/mnt/photos/2026").as_deref(), Some("cifs"));
        // A local disk mounted inside a share is local.
        assert_eq!(fs_of("/mnt/photos/local/day").as_deref(), Some("ext4"));
        // By component: `/homework` is not under `/home`.
        assert_eq!(
            mount_of(TABLE, Path::new("/homework")).map(|(p, _)| p),
            Some(PathBuf::from("/"))
        );
        assert!(remote_in_table("/home/jess/remote/shoots"));
        assert!(remote_in_table(
            "/run/user/1000/gvfs/smb-share:server=nas,share=x"
        ));
        assert!(!remote_in_table("/home/jess/Pictures"));
        assert!(!remote_in_table("/mnt/photos/local"));
    }

    /// An automount's `autofs` entry comes first, and the share it
    /// mounted over the same point after: the share is what the path
    /// is on.
    #[test]
    fn an_automounted_share_is_the_share() {
        assert_eq!(fs_of("/mnt/archive/Photos").as_deref(), Some("nfs4"));
        assert!(remote_in_table("/mnt/archive/Photos"));
        // Not mounted yet: the autofs entry alone, which is not taken
        // for a share.
        let table = "systemd-1 /mnt/archive autofs rw 0 0\n/dev/sda2 / ext4 rw 0 0\n";
        let (_, fs) = mount_of(table, Path::new("/mnt/archive/Photos")).unwrap();
        assert_eq!(fs, "autofs");
        assert!(!network(&fs));
    }

    /// Not read ahead: a folder under an automount not mounted yet (its
    /// first read would mount it), and once the table shows the share
    /// mounted there, a folder on the share. A local folder is.
    #[test]
    fn a_folder_is_peeked_only_on_a_local_mount() {
        let unmounted = "systemd-1 /mnt/archive autofs rw 0 0\n/dev/sda2 / ext4 rw 0 0\n";
        assert_eq!(
            not_to_peek_in(unmounted, Path::new("/mnt/archive/2026")).as_deref(),
            Some("an automount not mounted yet")
        );
        let mounted = format!("{unmounted}nas:/archive /mnt/archive nfs4 rw 0 0\n");
        assert_eq!(
            not_to_peek_in(&mounted, Path::new("/mnt/archive/2026")).as_deref(),
            Some("a network mount (nfs4)")
        );
        assert_eq!(
            not_to_peek_in(&mounted, Path::new("/home/p/Pictures")),
            None
        );
        assert_eq!(
            not_to_peek_in(TABLE, Path::new("/mnt/photos/local/day")),
            None,
            "a local disk inside a share"
        );
        assert!(not_to_peek_in("", Path::new("/home")).is_some(), "no table");
    }

    /// A folder given relative (`greycard .` from a shell inside a
    /// share) is made absolute against the working folder before the
    /// table is asked, with no disk asked.
    #[cfg(unix)]
    #[test]
    fn a_relative_folder_is_asked_as_an_absolute_one() {
        let here = std::env::current_dir().unwrap();
        let table = format!(
            "/dev/sda2 / ext4 rw 0 0\nnas:/x {} nfs rw 0 0\n",
            here.display()
        );
        for folder in [".", "", "sub/dir", "sub/../dir/."] {
            let folder = whole(Path::new(folder)).expect("resolved");
            assert_eq!(
                not_to_peek_in(&table, &folder).as_deref(),
                Some("a network mount (nfs)"),
                "{}",
                folder.display()
            );
        }
        assert_eq!(whole(Path::new("/a/b")), Some(PathBuf::from("/a/b")));
    }

    /// `..` and `.` are resolved by name before the table is asked: a
    /// path that climbs out of a local folder into a share is the share's.
    /// One that climbs above the root is not resolved, and not read.
    #[cfg(unix)]
    #[test]
    fn a_path_that_climbs_into_a_share_is_the_share_s() {
        let table = "/dev/sda2 / ext4 rw 0 0\n/dev/sda3 /home ext4 rw 0 0\n\
                     nas:/share /mnt/share nfs4 rw 0 0\n";
        let climbing = Path::new("/home/u/../../mnt/share/./x");
        assert_eq!(
            whole(climbing),
            Some(PathBuf::from("/mnt/share/x")),
            "resolved by name"
        );
        assert_eq!(
            not_to_peek_in(table, &whole(climbing).unwrap()).as_deref(),
            Some("a network mount (nfs4)")
        );
        assert_eq!(
            not_to_peek_in(table, &whole(Path::new("/mnt/share/../../home/u")).unwrap()),
            None,
            "back out to a local folder"
        );
        assert_eq!(whole(Path::new("/home/../..")), None);
        assert_eq!(
            not_to_peek(Path::new("/home/../../x")).as_deref(),
            Some("a folder whose path cannot be resolved")
        );
    }

    /// macOS's answer by its list of mounts, the match by bytes: a share
    /// under `/Volumes` by its own mount; a local disk there by its own;
    /// a path under the share and home folders that only the boot volume
    /// holds by bytes (a firmlink, an automounted home) is not trusted;
    /// a local folder elsewhere on the boot volume is read; no match at
    /// all is not trusted.
    #[test]
    fn the_mac_answer_trusts_only_a_sure_match() {
        let mount =
            |point: &str, fs: &str, local: bool| (point.as_bytes().to_vec(), fs.to_string(), local);
        let mounts = vec![
            mount("/", "apfs", true),
            mount("/System/Volumes/Data", "apfs", true),
            mount("/Volumes/NAS", "smbfs", false),
            mount("/Volumes/Card", "exfat", true),
            mount("/Volumes/Auto", "autofs", true),
        ];
        let ask = |p: &str| not_to_peek_mac(&mounts, p.as_bytes());
        assert_eq!(
            ask("/Volumes/NAS/2026").as_deref(),
            Some("a network mount (smbfs)")
        );
        assert_eq!(ask("/Volumes/Card/DCIM"), None);
        assert_eq!(
            ask("/Volumes/Auto/x").as_deref(),
            Some("an automount not mounted yet")
        );
        for unsure in [
            "/Volumes/Gone/x",
            "/home/p/Pictures",
            "/net/host/x",
            "/Network/Servers/x",
            "/System/Volumes/Data/home/p",
        ] {
            assert_eq!(
                ask(unsure).as_deref(),
                Some("a folder whose mount cannot be told for sure"),
                "{unsure}"
            );
        }
        assert_eq!(ask("/Users/p/Pictures"), None);
        assert_eq!(ask("/Volumesque/x"), None, "by component");
        assert_eq!(
            not_to_peek_mac(&[], b"/Users/p").as_deref(),
            Some("a mount the system does not list")
        );
    }

    #[test]
    fn a_mount_point_with_a_space_in_it() {
        assert_eq!(
            fs_of("/run/media/jess/My Card/DCIM").as_deref(),
            Some("exfat")
        );
        assert_eq!(
            fs_of("/run/media/jess/Old Drive").as_deref(),
            Some("fuseblk")
        );
        assert!(remote_in_table("/mnt/share two/2026"));
        assert!(!remote_in_table("/mnt/share"));
        assert_eq!(unescape(r"a\134b\011c"), "a\\b\tc");
        // Not an escape: left as it is.
        assert_eq!(unescape(r"a\9b\04"), r"a\9b\04");
        assert_eq!(unescape(r"a\477b"), r"a\477b");
    }

    /// A share mounted below a local root is found, by its own point,
    /// the outermost only; a local disk below a root is not, nor the
    /// root's own mount, nor an automount not mounted yet.
    #[test]
    fn the_network_mounts_below_a_root() {
        let table = "\
/dev/nvme0n1p2 / btrfs rw 0 0
/dev/nvme0n1p2 /home btrfs rw 0 0
//nas/photos /home/p/Pictures/NAS cifs rw 0 0
/dev/sdb1 /home/p/Pictures/NAS/local ext4 rw 0 0
nas:/more /home/p/Pictures/NAS/more nfs4 rw 0 0
/dev/sdc1 /home/p/Pictures/Card exfat rw 0 0
systemd-1 /home/p/Pictures/Archive autofs rw 0 0
nas:/archive /home/p/Pictures/Archive nfs4 rw 0 0
systemd-1 /home/p/Pictures/Later autofs rw 0 0
host:/srv /home/p/Pictures/My\\040Share fuse.sshfs rw 0 0
//nas/elsewhere /home/p/Music cifs rw 0 0
";
        let found = remote_under_in(table, Path::new("/home/p/Pictures"));
        assert_eq!(
            found,
            vec![
                (
                    PathBuf::from("/home/p/Pictures/NAS"),
                    Some("cifs".to_string())
                ),
                (
                    PathBuf::from("/home/p/Pictures/Archive"),
                    Some("nfs4".to_string())
                ),
                // An automount not mounted yet: left out, its type not
                // known.
                (PathBuf::from("/home/p/Pictures/Later"), None),
                (
                    PathBuf::from("/home/p/Pictures/My Share"),
                    Some("fuse.sshfs".to_string())
                ),
            ]
        );
        // A root on a share has no mount below it in the table here, and
        // a root with none below it finds none.
        assert!(remote_under_in(table, Path::new("/home/p/Pictures/NAS/local/x")).is_empty());
        assert!(remote_under_in(table, Path::new("/home/p/Documents")).is_empty());
        // A root's own mount is not below it.
        assert!(remote_under_in(table, Path::new("/home/p/Music")).is_empty());
        // By component: `/home/p/Pictures2` is not under the root.
        let table = "//nas/x /home/p/Pictures2 cifs rw 0 0\n";
        assert!(remote_under_in(table, Path::new("/home/p/Pictures")).is_empty());
    }

    /// An automount below a root, before its first use and after: the
    /// table read again once it is looked into says what it mounted.
    #[test]
    fn an_automount_below_a_root_resolves_when_the_table_is_read_again() {
        let before = "\
/dev/nvme0n1p2 / btrfs rw 0 0
systemd-1 /home/p/Pictures/Archive autofs rw 0 0
systemd-1 /home/p/Pictures/Card autofs rw 0 0
";
        let root = Path::new("/home/p/Pictures");
        assert_eq!(
            remote_under_in(before, root),
            vec![
                (PathBuf::from("/home/p/Pictures/Archive"), None),
                (PathBuf::from("/home/p/Pictures/Card"), None),
            ]
        );
        let after = format!(
            "{before}nas:/archive /home/p/Pictures/Archive nfs4 rw 0 0\n\
             /dev/sdc1 /home/p/Pictures/Card exfat rw 0 0\n"
        );
        // The share by its type; the local disk no longer among them.
        assert_eq!(
            remote_under_in(&after, root),
            vec![(
                PathBuf::from("/home/p/Pictures/Archive"),
                Some("nfs4".to_string())
            )]
        );
    }

    #[test]
    fn a_table_that_says_nothing_says_nothing() {
        assert_eq!(mount_of("", Path::new("/x")), None);
        assert_eq!(mount_of("garbage\n\n", Path::new("/x")), None);
    }

    /// This machine's own table: a temporary folder is on some mount,
    /// and whatever it is, the question is answered.
    #[cfg(target_os = "linux")]
    #[test]
    fn this_machine_s_table_is_read() {
        let table = std::fs::read_to_string("/proc/self/mounts").unwrap();
        assert!(mount_of(&table, &std::env::temp_dir()).is_some());
        let _ = remote(&std::env::temp_dir());
    }
}
