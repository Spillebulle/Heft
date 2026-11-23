//! Linux: `getdents64` on a directory fd, `statx` relative to that fd.
//!
//! The directory is opened once by path; every entry in it is sized relative
//! to the open fd, so the kernel never walks the full path per file. Only the
//! fields a size needs are asked for (`STATX_DONT_SYNC` keeps network
//! filesystems from round-tripping for freshness we do not need).

use super::{Kind, Listing, Raw, RootInfo, clamp_secs};
use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, StatxFlags};
use std::io;
use std::path::Path;

fn dev(major: u32, minor: u32) -> u64 {
    ((major as u64) << 32) | minor as u64
}

pub fn root_info(path: &Path) -> io::Result<RootInfo> {
    let st = rustix::fs::statx(
        CWD,
        path,
        AtFlags::empty(),
        StatxFlags::TYPE | StatxFlags::INO,
    )?;
    Ok(RootInfo {
        dev: dev(st.stx_dev_major, st.stx_dev_minor),
        cluster: 0,
    })
}

pub fn read_dir(path: &Path, _root: &RootInfo, want_mtime: bool) -> io::Result<Listing> {
    let fd = rustix::fs::openat(
        CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?;
    let mut dir = Dir::read_from(&fd)?;
    let mut mask = StatxFlags::TYPE
        | StatxFlags::SIZE
        | StatxFlags::BLOCKS
        | StatxFlags::INO
        | StatxFlags::NLINK;
    if want_mtime {
        mask |= StatxFlags::MTIME;
    }
    let mut out = Listing::default();
    while let Some(entry) = dir.next() {
        let entry = entry?;
        let name = entry.file_name();
        let bytes = name.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        match rustix::fs::statx(
            &fd,
            name,
            AtFlags::SYMLINK_NOFOLLOW | AtFlags::STATX_DONT_SYNC,
            mask,
        ) {
            Ok(st) => {
                let kind = match FileType::from_raw_mode(st.stx_mode as _) {
                    FileType::RegularFile => Kind::File,
                    FileType::Directory => Kind::Dir,
                    FileType::Symlink => Kind::Link,
                    _ => Kind::Other,
                };
                out.push(
                    bytes,
                    Raw {
                        kind,
                        logical: st.stx_size,
                        disk: st.stx_blocks * 512,
                        mtime: if want_mtime {
                            clamp_secs(st.stx_mtime.tv_sec)
                        } else {
                            0
                        },
                        dev: dev(st.stx_dev_major, st.stx_dev_minor),
                        ino: st.stx_ino,
                        nlink: st.stx_nlink,
                    },
                );
            }
            Err(_) => {
                // Listed but not sizable (vanished, or no search permission
                // on an entry). Kept, so the folder does not look smaller
                // than it is without saying so.
                let kind = match entry.file_type() {
                    FileType::Directory => Kind::Dir,
                    FileType::Symlink => Kind::Link,
                    _ => Kind::File,
                };
                out.unsizable += 1;
                out.push(
                    bytes,
                    Raw {
                        kind,
                        logical: 0,
                        disk: 0,
                        mtime: 0,
                        dev: 0,
                        ino: 0,
                        nlink: 1,
                    },
                );
            }
        }
    }
    Ok(out)
}

/// Filesystem magic numbers that describe kernel state, not storage.
const VIRTUAL: &[i64] = &[
    0x9fa0,      // proc
    0x6265_6572, // sysfs
    0x1373,      // devtmpfs (devfs)
    0x0027_e0eb, // cgroup
    0x6367_7270, // cgroup2
    0x6462_6720, // debugfs
    0x7472_6163, // tracefs
    0x7363_6673, // securityfs
    0x6165_676c, // pstore
    0xcafe_4a11, // bpf
    0x6265_6570, // configfs
    0x6573_5543, // fusectl
    0x1980_0202, // mqueue
    0x4249_4e4d, // binfmt_misc
    0x0187,      // autofs
    0xde5e_81e4, // efivarfs
    0x0000_1cd1, // devpts
];

pub fn is_virtual_fs(path: &Path) -> bool {
    match rustix::fs::statfs(path) {
        Ok(s) => VIRTUAL.contains(&(s.f_type as i64)),
        Err(_) => false,
    }
}
