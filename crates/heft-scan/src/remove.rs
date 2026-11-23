//! Getting rid of things, which is the one write Heft makes.
//!
//! The default is the Recycle Bin / Trash. Permanent deletion is a separate
//! call the window only makes after its own confirmation. Both re-check each
//! target first: between the scan and the click a path may have been replaced
//! (a folder swapped for a link to somewhere else is the case that matters),
//! and anything whose type differs from what was scanned is refused.
//!
//! Protected places are refused with a reason, and the window draws the
//! control disabled with that reason as its tooltip.

use crate::scan::Scan;
use crate::tree::{NodeId, flag};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Trash,
    Permanently,
}

/// Why a node may not be removed, in a sentence for a tooltip. `None` means
/// it may.
pub fn refusal(scan: &Scan, node: NodeId) -> Option<String> {
    let t = scan.tree();
    if t.parent(node) == crate::NONE {
        return Some("A scan root cannot be removed from here.".into());
    }
    if t.has(node, flag::REMOVED) {
        return Some("Already removed.".into());
    }
    if t.has(node, flag::OTHER_FS) || t.has(node, flag::VIRTUAL) {
        return Some("This is where another filesystem is mounted.".into());
    }
    drop(t);
    let path = scan.fs_path(node);
    protected(&path)
}

/// The places that are never Heft's to remove.
pub fn protected(path: &Path) -> Option<String> {
    let plain = display_form(path);
    let lower = plain.to_string_lossy().to_lowercase().replace('/', "\\");
    #[cfg(windows)]
    {
        let rest = lower.get(3..).unwrap_or("");
        let top = rest.split('\\').next().unwrap_or("");
        let is_top = !rest.contains('\\');
        if matches!(
            top,
            "windows" | "program files" | "program files (x86)" | "programdata"
        ) && (is_top || top == "windows")
        {
            return Some(
                "Windows needs this folder. Uninstall programs instead of deleting them.".into(),
            );
        }
        if rest.is_empty() {
            return Some("A whole disk cannot be removed from here.".into());
        }
        if is_top && top == "users" {
            return Some("The Users folder holds every account on this computer.".into());
        }
        if let Some(home) = std::env::var_os("USERPROFILE")
            && plain == Path::new(&home)
        {
            return Some("This is your home folder.".into());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = &lower;
        let s = plain.to_string_lossy();
        const SYSTEM: &[&str] = &[
            "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib32", "/lib64", "/opt",
            "/proc", "/root", "/run", "/sbin", "/srv", "/sys", "/usr", "/var",
        ];
        if SYSTEM.contains(&s.trim_end_matches('/')) || s == "/" {
            return Some("The system needs this folder.".into());
        }
        if s.starts_with("/usr/") || s.starts_with("/boot/") || s.starts_with("/etc/") {
            return Some("This belongs to the system's packages. Uninstall them instead.".into());
        }
        if let Some(home) = std::env::var_os("HOME")
            && plain == Path::new(&home)
        {
            return Some("This is your home folder.".into());
        }
    }
    None
}

#[cfg(windows)]
fn display_form(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p.to_path_buf(),
    }
}

#[cfg(not(windows))]
fn display_form(p: &Path) -> PathBuf {
    p.to_path_buf()
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub removed: Vec<NodeId>,
    /// What could not be removed, and why, in sentences.
    pub failed: Vec<(PathBuf, String)>,
}

/// Removes `nodes`, re-checking each first, and takes what went from the tree.
/// Blocking: call it off the UI thread.
pub fn remove(scan: &Scan, nodes: &[NodeId], how: How) -> Outcome {
    let mut out = Outcome::default();
    for &n in nodes {
        let path = scan.fs_path(n);
        let shown = display_form(&path);
        if let Some(why) = refusal(scan, n) {
            out.failed.push((shown, why));
            continue;
        }
        let was_dir = scan.tree().is_dir(n);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() == was_dir && !m.file_type().is_symlink() => {}
            Ok(_) => {
                out.failed.push((
                    shown,
                    "It changed since the scan. Rescan, then try again.".into(),
                ));
                continue;
            }
            Err(_) => {
                out.failed.push((shown, "It is no longer there.".into()));
                // Gone either way: take it out of the totals.
                scan.tree_mut().mark_removed(n);
                continue;
            }
        }
        let result = match how {
            How::Trash => trash::delete(display_form(&path)).map_err(|e| e.to_string()),
            How::Permanently => if was_dir {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            }
            .map_err(|e| e.to_string()),
        };
        match result {
            Ok(()) => {
                scan.tree_mut().mark_removed(n);
                out.removed.push(n);
            }
            Err(e) => out.failed.push((shown, e)),
        }
    }
    scan.bump();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_places_are_refused() {
        #[cfg(windows)]
        {
            assert!(protected(Path::new(r"C:\")).is_some());
            assert!(protected(Path::new(r"C:\Windows")).is_some());
            assert!(protected(Path::new(r"C:\Windows\Temp\x")).is_some());
            assert!(protected(Path::new(r"\\?\C:\Program Files")).is_some());
            assert!(protected(Path::new(r"C:\Program Files\Some App")).is_none());
            assert!(protected(Path::new(r"D:\Games\old")).is_none());
        }
        #[cfg(not(windows))]
        {
            assert!(protected(Path::new("/")).is_some());
            assert!(protected(Path::new("/usr")).is_some());
            assert!(protected(Path::new("/usr/lib/x")).is_some());
            assert!(protected(Path::new("/home")).is_some());
            assert!(protected(Path::new("/home/someone/.cache")).is_none());
        }
    }
}
