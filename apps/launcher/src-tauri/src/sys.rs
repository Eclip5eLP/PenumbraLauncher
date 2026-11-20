//! Small Windows helpers: free disk space, default library location.

use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDrives};

const DRIVE_FIXED: u32 = 3;

fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str().encode_wide().chain(Some(0)).collect()
}

/// Free bytes available to the user on the volume of `path` (or its nearest existing parent).
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    let mut free = 0u64;
    // SAFETY: valid NUL-terminated wide string; out-pointer to a local u64; other outputs are optional.
    let ok = unsafe { GetDiskFreeSpaceExW(wide(p).as_ptr(), &mut free, std::ptr::null_mut(), std::ptr::null_mut()) };
    (ok != 0).then_some(free)
}

/// `<fixed drive with the most free space>:\Penumbra Games`
pub fn suggested_library() -> PathBuf {
    // SAFETY: plain query without arguments.
    let mask = unsafe { GetLogicalDrives() };
    let mut best: Option<(u64, char)> = None;
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = PathBuf::from(format!("{letter}:\\"));
        // SAFETY: valid NUL-terminated wide string.
        if unsafe { GetDriveTypeW(wide(&root).as_ptr()) } != DRIVE_FIXED {
            continue;
        }
        if let Some(free) = free_space(&root)
            && best.is_none_or(|(b, _)| free > b)
        {
            best = Some((free, letter));
        }
    }
    PathBuf::from(format!("{}:\\Penumbra Games", best.map(|(_, l)| l).unwrap_or('C')))
}

pub fn human(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < units.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", units[i]) }
}

/// Names Windows won't accept as folder names are replaced, so item names can become folder names.
pub fn folder_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c < ' ' { '_' } else { c }).collect();
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() { "Game".into() } else { s }
}
