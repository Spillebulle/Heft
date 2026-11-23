//! Windows: `FindFirstFileExW` with `FindExInfoBasic` and a large fetch.
//!
//! The listing carries every entry's size and attributes, so no file is
//! opened or stat'ed: one call returns a buffer full of entries. Sizes on disk
//! are the logical size rounded up to the volume's cluster, except where the
//! attributes say rounding would lie (compressed and sparse files ask
//! `GetCompressedFileSizeW`; cloud placeholders take no space).
//!
//! This is the fallback walker. The fast one reads the NTFS master file table
//! through an elevated helper and is not built yet (CLAUDE.md).

use super::{Kind, Listing, Raw, RootInfo, clamp_secs};
use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_FILES, GetLastError, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_COMPRESSED, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE,
    FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS, FILE_ATTRIBUTE_RECALL_ON_OPEN,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_ATTRIBUTE_SPARSE_FILE, FIND_FIRST_EX_LARGE_FETCH, FindClose,
    FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW,
    GetCompressedFileSizeW, GetDiskFreeSpaceW, GetVolumePathNameW, WIN32_FIND_DATAW,
};

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn root_info(path: &Path) -> io::Result<RootInfo> {
    let w = wide(path);
    let mut vol = [0u16; 1024];
    let mut cluster = 0u64;
    // SAFETY: buffers are sized as passed and NUL-terminated.
    unsafe {
        if GetVolumePathNameW(w.as_ptr(), vol.as_mut_ptr(), vol.len() as u32) != 0 {
            let (mut spc, mut bps, mut free, mut total) = (0u32, 0u32, 0u32, 0u32);
            if GetDiskFreeSpaceW(vol.as_ptr(), &mut spc, &mut bps, &mut free, &mut total) != 0 {
                cluster = spc as u64 * bps as u64;
            }
        }
    }
    Ok(RootInfo { dev: 0, cluster })
}

/// Windows FILETIME (100 ns since 1601) to seconds since 1970.
fn filetime_secs(lo: u32, hi: u32) -> u32 {
    let t = ((hi as u64) << 32) | lo as u64;
    clamp_secs((t / 10_000_000) as i64 - 11_644_473_600)
}

pub fn read_dir(path: &Path, root: &RootInfo, want_mtime: bool) -> io::Result<Listing> {
    let pattern = wide(&path.join("*"));
    let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    // SAFETY: pattern is NUL-terminated; data is a valid out-pointer.
    let h = unsafe {
        FindFirstFileExW(
            pattern.as_ptr(),
            FindExInfoBasic,
            &mut data as *mut _ as *mut _,
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        )
    };
    if h == INVALID_HANDLE_VALUE {
        let code = unsafe { GetLastError() };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(Listing::default());
        }
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let mut out = Listing::default();
    let cluster = root.cluster;
    loop {
        let len = data
            .cFileName
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(data.cFileName.len());
        let name16 = &data.cFileName[..len];
        let skip = name16 == [b'.' as u16] || name16 == [b'.' as u16, b'.' as u16];
        if !skip {
            let attr = data.dwFileAttributes;
            let logical = ((data.nFileSizeHigh as u64) << 32) | data.nFileSizeLow as u64;
            let kind = if attr & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                Kind::Link
            } else if attr & FILE_ATTRIBUTE_DIRECTORY != 0 {
                Kind::Dir
            } else {
                Kind::File
            };
            // Folders and links take no space of their own here; a cloud
            // placeholder's bytes are not on this disk.
            let disk = if kind != Kind::File
                || attr
                    & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS
                        | FILE_ATTRIBUTE_RECALL_ON_OPEN
                        | FILE_ATTRIBUTE_OFFLINE)
                    != 0
            {
                0
            } else if attr & (FILE_ATTRIBUTE_COMPRESSED | FILE_ATTRIBUTE_SPARSE_FILE) != 0 {
                compressed_size(&path.join(OsString::from_wide(name16))).unwrap_or(logical)
            } else if cluster > 0 {
                logical.div_ceil(cluster) * cluster
            } else {
                logical
            };
            let name = OsString::from_wide(name16);
            out.push(
                name.as_encoded_bytes(),
                Raw {
                    kind,
                    logical,
                    disk,
                    mtime: if want_mtime {
                        filetime_secs(
                            data.ftLastWriteTime.dwLowDateTime,
                            data.ftLastWriteTime.dwHighDateTime,
                        )
                    } else {
                        0
                    },
                    dev: 0,
                    ino: 0,
                    nlink: 1,
                },
            );
        }
        // SAFETY: h is a live find handle.
        if unsafe { FindNextFileW(h, &mut data) } == 0 {
            let code = unsafe { GetLastError() };
            unsafe { FindClose(h) };
            if code == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
    }
    Ok(out)
}

fn compressed_size(p: &Path) -> Option<u64> {
    let w = wide(p);
    let mut hi = 0u32;
    // SAFETY: w is NUL-terminated; hi is a valid out-pointer.
    let lo = unsafe { GetCompressedFileSizeW(w.as_ptr(), &mut hi) };
    if lo == u32::MAX && unsafe { GetLastError() } != 0 {
        return None;
    }
    Some(((hi as u64) << 32) | lo as u64)
}
