//! Reading one directory, the fast way for each platform.
//!
//! A walker's whole job is `read_dir`: list one directory and size every entry
//! in it, returning the names back to back in one buffer. Recursion,
//! threading, the tree and hard-link accounting live in `scan.rs`, so they are
//! the same on every platform and testable without one.

use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", windows)))]
mod portable;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
    /// Symlink, junction, any reparse point. Never followed.
    Link,
    /// Sockets, fifos, devices: listed, sized as they report, never opened.
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct Raw {
    pub kind: Kind,
    pub logical: u64,
    pub disk: u64,
    pub mtime: u32,
    /// Device the entry lives on (0 where the platform does not say).
    pub dev: u64,
    /// Inode / file id (0 where the platform does not say).
    pub ino: u64,
    /// Hard link count (1 where the platform does not say).
    pub nlink: u32,
}

#[derive(Default)]
pub struct Listing {
    pub names: Vec<u8>,
    /// Offset into `names`, length, metadata.
    pub entries: Vec<(u32, u16, Raw)>,
    /// Entries that were listed but could not be sized.
    pub unsizable: u32,
}

impl Listing {
    pub(crate) fn push(&mut self, name: &[u8], raw: Raw) {
        let len = name.len().min(u16::MAX as usize);
        self.entries
            .push((self.names.len() as u32, len as u16, raw));
        self.names.extend_from_slice(&name[..len]);
    }
}

/// What a walker knows about the root it was started on.
#[derive(Debug, Clone, Copy, Default)]
pub struct RootInfo {
    pub dev: u64,
    /// Allocation unit, for rounding on platforms that report only logical
    /// sizes in a listing (Windows). 0 when unknown.
    pub cluster: u64,
}

/// The name of the walker in use, for the status bar.
pub fn walker_name() -> &'static str {
    #[cfg(target_os = "linux")]
    return "getdents64 + statx";
    #[cfg(windows)]
    return "Folder walk";
    #[cfg(not(any(target_os = "linux", windows)))]
    return "Portable walk";
}

pub fn root_info(path: &Path) -> io::Result<RootInfo> {
    #[cfg(target_os = "linux")]
    return linux::root_info(path);
    #[cfg(windows)]
    return windows::root_info(path);
    #[cfg(not(any(target_os = "linux", windows)))]
    return portable::root_info(path);
}

pub fn read_dir(path: &Path, root: &RootInfo, want_mtime: bool) -> io::Result<Listing> {
    #[cfg(target_os = "linux")]
    return linux::read_dir(path, root, want_mtime);
    #[cfg(windows)]
    return windows::read_dir(path, root, want_mtime);
    #[cfg(not(any(target_os = "linux", windows)))]
    return portable::read_dir(path, root, want_mtime);
}

/// True if the directory at `path` is a virtual filesystem that must never be
/// scanned (`/proc`, `/sys`, ...). Always false off Linux.
pub fn is_virtual_fs(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    return linux::is_virtual_fs(path);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

pub(crate) fn clamp_secs(secs: i64) -> u32 {
    secs.clamp(0, u32::MAX as i64) as u32
}
