//! Anything that is neither Linux nor Windows: `std::fs`, slow but correct.
//! Not a shipped target; it keeps the engine compiling elsewhere.

use super::{Kind, Listing, Raw, RootInfo, clamp_secs};
use std::io;
use std::path::Path;

pub fn root_info(_path: &Path) -> io::Result<RootInfo> {
    Ok(RootInfo::default())
}

pub fn read_dir(path: &Path, _root: &RootInfo, want_mtime: bool) -> io::Result<Listing> {
    let mut out = Listing::default();
    for e in std::fs::read_dir(path)? {
        let e = e?;
        let name = e.file_name();
        let Ok(m) = e.path().symlink_metadata() else {
            out.unsizable += 1;
            continue;
        };
        let ft = m.file_type();
        let kind = if ft.is_symlink() {
            Kind::Link
        } else if ft.is_dir() {
            Kind::Dir
        } else if ft.is_file() {
            Kind::File
        } else {
            Kind::Other
        };
        let mtime = if want_mtime {
            m.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| clamp_secs(d.as_secs() as i64))
                .unwrap_or(0)
        } else {
            0
        };
        out.push(
            name.as_encoded_bytes(),
            Raw { kind, logical: m.len(), disk: m.len(), mtime, dev: 0, ino: 0, nlink: 1 },
        );
    }
    Ok(out)
}
