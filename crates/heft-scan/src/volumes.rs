//! The disks a person would want to scan: one row each in the sidebar.
//!
//! Windows: every fixed or removable drive letter with a filesystem. Linux:
//! every mount whose filesystem stores files (ext4, btrfs, xfs, ...), never
//! the kernel's own (`proc`, `sysfs`, `cgroup`, ...), never a squashfs snap,
//! and each device once.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Volume {
    /// Where to scan from: `C:\`, `/`, `/home`.
    pub path: PathBuf,
    /// What to call it: the volume label, or the mount point.
    pub label: String,
    pub total: u64,
    pub free: u64,
}

impl Volume {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.free)
    }
}

#[cfg(windows)]
pub fn list() -> Vec<Volume> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    let mut out = Vec::new();
    // SAFETY: no arguments.
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let w: Vec<u16> = std::ffi::OsStr::new(&root)
            .encode_wide()
            .chain([0])
            .collect();
        // SAFETY: w is NUL-terminated; out-pointers are valid.
        unsafe {
            let kind = GetDriveTypeW(w.as_ptr());
            if kind != DRIVE_FIXED && kind != DRIVE_REMOVABLE {
                continue;
            }
            let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
            if GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) == 0 {
                continue; // an empty card reader
            }
            let mut name = [0u16; 261];
            let label = if GetVolumeInformationW(
                w.as_ptr(),
                name.as_mut_ptr(),
                name.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            ) != 0
            {
                let n = name.iter().position(|&c| c == 0).unwrap_or(0);
                String::from_utf16_lossy(&name[..n])
            } else {
                String::new()
            };
            let label = if label.is_empty() {
                format!("Local disk ({letter}:)")
            } else {
                format!("{label} ({letter}:)")
            };
            out.push(Volume {
                path: PathBuf::from(root),
                label,
                total,
                free,
            });
        }
    }
    out
}

#[cfg(target_os = "linux")]
pub fn list() -> Vec<Volume> {
    const STORAGE: &[&str] = &[
        "ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "jfs", "reiserfs", "zfs", "bcachefs",
        "vfat", "exfat", "ntfs", "ntfs3", "fuseblk", "hfsplus", "nilfs2",
    ];
    let Ok(mounts) = std::fs::read_to_string("/proc/self/mounts") else {
        return Vec::new();
    };
    let mut out: Vec<Volume> = Vec::new();
    let mut seen_dev = std::collections::HashSet::new();
    for line in mounts.lines() {
        let mut f = line.split_whitespace();
        let (Some(dev), Some(mnt), Some(fs)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        if !STORAGE.contains(&fs) {
            continue;
        }
        // /proc/mounts escapes spaces and friends as octal.
        let mnt = unescape(mnt);
        if mnt.starts_with("/boot")
            || mnt.starts_with("/snap")
            || mnt.starts_with("/var/lib/docker")
        {
            continue;
        }
        // A btrfs volume is mounted many times (subvolumes); show it once,
        // at its shortest mount point, which is the order mounts lists them.
        if !seen_dev.insert(dev.to_string()) {
            continue;
        }
        let Ok(s) = rustix::fs::statvfs(mnt.as_str()) else {
            continue;
        };
        let total = s.f_blocks * s.f_frsize;
        let free = s.f_bavail * s.f_frsize;
        if total == 0 {
            continue;
        }
        let label = if mnt == "/" {
            "System (/)".to_string()
        } else {
            mnt.clone()
        };
        out.push(Volume {
            path: PathBuf::from(mnt),
            label,
            total,
            free,
        });
    }
    out
}

#[cfg(target_os = "linux")]
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
        {
            let v = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
            out.push(v);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn list() -> Vec<Volume> {
    vec![Volume {
        path: PathBuf::from("/"),
        label: "/".into(),
        total: 0,
        free: 0,
    }]
}
