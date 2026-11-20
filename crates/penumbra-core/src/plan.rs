//! Install / update / repair planning (docs/03-content-format.md#delta-planning). Pure: the caller describes
//! what is on disk, the planner says what to skip, copy locally, download and delete.

use std::collections::{BTreeMap, HashMap};

use crate::manifest::Manifest;

/// What the caller knows about a file on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalState {
    Missing,
    /// `verified`: `Some(true/false)` after a full hash check (repair), `None` when only the size was checked.
    Present { size: u64, verified: Option<bool> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWrite {
    /// Index into the new manifest's `files`.
    pub file: usize,
    pub path: String,
    pub size: u64,
    pub hash: String,
}

/// One segment of a downloaded chunk that lands in a file being written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegTarget {
    /// Index into `Plan::write`.
    pub write: usize,
    pub file_offset: u64,
    pub chunk_offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkFetch {
    pub hash: String,
    pub csize: u64,
    pub len: u64,
    pub targets: Vec<SegTarget>,
}

/// Copy `size` bytes from an existing local file instead of downloading them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCopy {
    pub write: usize,
    pub file_offset: u64,
    pub src_path: String,
    pub src_offset: u64,
    pub size: u64,
}

#[derive(Debug, Default, Clone)]
pub struct Plan {
    pub write: Vec<FileWrite>,
    pub fetch: Vec<ChunkFetch>,
    pub reuse: Vec<LocalCopy>,
    pub delete: Vec<String>,
    pub dirs: Vec<String>,
    /// Compressed bytes to download.
    pub download_bytes: u64,
    /// Bytes copied from local files.
    pub reuse_bytes: u64,
}

impl Plan {
    pub fn is_noop(&self) -> bool {
        self.write.is_empty() && self.delete.is_empty()
    }
}

/// Glob match for `keep` rules: `**` = anything, `*` = anything but `/`, `?` = one char but `/`.
/// Case-insensitive (Windows).
pub fn glob_match(pattern: &str, path: &str) -> bool {
    fn rec(p: &[u8], s: &[u8]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some(b'*') if p.get(1) == Some(&b'*') => {
                let rest = if p.get(2) == Some(&b'/') { &p[3..] } else { &p[2..] };
                (0..=s.len()).any(|i| rec(rest, &s[i..]))
            }
            Some(b'*') => (0..=s.len()).take_while(|&i| i == 0 || s[i - 1] != b'/').any(|i| rec(&p[1..], &s[i..])),
            Some(b'?') => !s.is_empty() && s[0] != b'/' && rec(&p[1..], &s[1..]),
            Some(&c) => !s.is_empty() && s[0] == c && rec(&p[1..], &s[1..]),
        }
    }
    rec(pattern.to_ascii_lowercase().as_bytes(), path.to_ascii_lowercase().as_bytes())
}

fn is_kept(keep: &[String], path: &str) -> bool {
    keep.iter().any(|k| glob_match(k, path))
}

/// Plans going from `old` (installed manifest, if any) + what's on disk to `new`.
pub fn plan(new: &Manifest, old: Option<&Manifest>, local: &dyn Fn(&str) -> LocalState) -> Plan {
    let mut p = Plan { dirs: new.dirs.clone(), ..Default::default() };
    let old_by_path: HashMap<String, &crate::manifest::FileEntry> =
        old.map(|m| m.files.iter().map(|f| (f.path.to_lowercase(), f)).collect()).unwrap_or_default();

    // A local file is trustworthy when its size matches what the old manifest says and a full check (if one
    // was done) passed.
    let trusted = |path: &str, expected_size: u64| match local(path) {
        LocalState::Present { size, verified } => size == expected_size && verified != Some(false),
        LocalState::Missing => false,
    };

    // Segment sources from intact old files: (chunk, offset, size) -> (path, file offset).
    let mut sources: HashMap<(String, u64, u64), (String, u64)> = HashMap::new();
    if let Some(old) = old {
        for f in &old.files {
            if !trusted(&f.path, f.size) {
                continue;
            }
            let mut off = 0;
            for c in &f.chunks {
                sources.entry((c.hash.clone(), c.offset, c.size)).or_insert((f.path.clone(), off));
                off += c.size;
            }
        }
    }

    let mut fetch: BTreeMap<String, ChunkFetch> = BTreeMap::new();
    for (idx, f) in new.files.iter().enumerate() {
        let state = local(&f.path);
        if is_kept(&new.keep, &f.path) && state != LocalState::Missing {
            continue; // user data: never overwrite
        }
        let unchanged = match state {
            LocalState::Present { size, verified: Some(true) } => size == f.size,
            LocalState::Present { size, verified: None } => {
                size == f.size && old_by_path.get(&f.path.to_lowercase()).is_some_and(|o| o.hash == f.hash)
            }
            _ => false,
        };
        if unchanged {
            continue;
        }
        let w = p.write.len();
        p.write.push(FileWrite { file: idx, path: f.path.clone(), size: f.size, hash: f.hash.clone() });
        let mut off = 0;
        for c in &f.chunks {
            if let Some((src, src_off)) = sources.get(&(c.hash.clone(), c.offset, c.size)) {
                p.reuse.push(LocalCopy {
                    write: w,
                    file_offset: off,
                    src_path: src.clone(),
                    src_offset: *src_off,
                    size: c.size,
                });
                p.reuse_bytes += c.size;
            } else {
                fetch
                    .entry(c.hash.clone())
                    .or_insert_with(|| ChunkFetch { hash: c.hash.clone(), csize: c.csize, len: c.chunk_len(), targets: vec![] })
                    .targets
                    .push(SegTarget { write: w, file_offset: off, chunk_offset: c.offset, size: c.size });
            }
            off += c.size;
        }
    }
    p.fetch = fetch.into_values().collect();
    p.download_bytes = p.fetch.iter().map(|c| c.csize).sum();

    if let Some(old) = old {
        let new_paths: std::collections::HashSet<String> = new.files.iter().map(|f| f.path.to_lowercase()).collect();
        for f in &old.files {
            if !new_paths.contains(&f.path.to_lowercase()) && !is_kept(&new.keep, &f.path) {
                p.delete.push(f.path.clone());
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{ChunkRef, FileEntry};

    fn h(c: char) -> String {
        std::iter::repeat_n(c, 64).collect()
    }

    fn file(path: &str, hash: char, chunks: &[(char, u64, u64, u64)]) -> FileEntry {
        // (chunk hash char, size, offset, len)
        FileEntry {
            path: path.into(),
            size: chunks.iter().map(|c| c.1).sum(),
            hash: h(hash),
            exec: false,
            chunks: chunks
                .iter()
                .map(|&(c, size, offset, len)| ChunkRef {
                    hash: h(c),
                    size,
                    csize: len / 2,
                    offset,
                    len: if len == size { None } else { Some(len) },
                })
                .collect(),
        }
    }

    fn manifest(files: Vec<FileEntry>, keep: &[&str]) -> Manifest {
        Manifest {
            format: 1,
            item: "x".into(),
            version: "1".into(),
            created: String::new(),
            total_size: 0,
            download_size: 0,
            launch: None,
            files,
            dirs: vec![],
            keep: keep.iter().map(|s| s.to_string()).collect(),
            local: false,
        }
    }

    #[test]
    fn glob() {
        assert!(glob_match("options.ini", "Options.INI"));
        assert!(glob_match("*.ini", "a.ini"));
        assert!(!glob_match("*.ini", "data/a.ini"));
        assert!(glob_match("**/*.ini", "data/a.ini"));
        assert!(glob_match("**/*.ini", "a.ini"));
        assert!(glob_match("Saves/**", "saves/x/y.dat"));
        assert!(glob_match("save?.ini", "save1.ini"));
        assert!(!glob_match("save?.ini", "save10.ini"));
    }

    #[test]
    fn fresh_install_downloads_every_chunk_once() {
        // Two small files in one pack 'p', one big file of two chunks.
        let m = manifest(
            vec![
                file("a.ini", '1', &[('p', 10, 0, 30)]),
                file("b.ini", '2', &[('p', 20, 10, 30)]),
                file("big.pak", '3', &[('c', 100, 0, 100), ('d', 50, 0, 50)]),
            ],
            &[],
        );
        let p = plan(&m, None, &|_| LocalState::Missing);
        assert_eq!(p.write.len(), 3);
        assert_eq!(p.fetch.len(), 3);
        let pack = p.fetch.iter().find(|c| c.hash == h('p')).unwrap();
        assert_eq!(pack.targets.len(), 2);
        assert_eq!(pack.targets[1], SegTarget { write: 1, file_offset: 0, chunk_offset: 10, size: 20 });
        let d = p.fetch.iter().find(|c| c.hash == h('d')).unwrap();
        assert_eq!(d.targets[0].file_offset, 100);
        assert_eq!(p.download_bytes, 15 + 50 + 25);
    }

    #[test]
    fn update_reuses_local_segments_skips_unchanged_and_deletes_removed() {
        let old = manifest(
            vec![
                file("same.txt", '1', &[('s', 10, 0, 10)]),
                file("big.pak", '2', &[('c', 100, 0, 100), ('d', 50, 0, 50)]),
                file("gone.txt", '3', &[('g', 5, 0, 5)]),
            ],
            &[],
        );
        let new = manifest(
            vec![
                file("same.txt", '1', &[('s', 10, 0, 10)]),
                // big.pak changed at the end: first chunk reused, new tail downloaded
                file("big.pak", '4', &[('c', 100, 0, 100), ('e', 60, 0, 60)]),
            ],
            &[],
        );
        let local = |p: &str| match p {
            "same.txt" => LocalState::Present { size: 10, verified: None },
            "big.pak" => LocalState::Present { size: 150, verified: None },
            "gone.txt" => LocalState::Present { size: 5, verified: None },
            _ => LocalState::Missing,
        };
        let p = plan(&new, Some(&old), &local);
        assert_eq!(p.write.iter().map(|w| w.path.as_str()).collect::<Vec<_>>(), vec!["big.pak"]);
        assert_eq!(p.reuse, vec![LocalCopy { write: 0, file_offset: 0, src_path: "big.pak".into(), src_offset: 0, size: 100 }]);
        assert_eq!(p.fetch.len(), 1);
        assert_eq!(p.fetch[0].hash, h('e'));
        assert_eq!(p.fetch[0].targets[0].file_offset, 100);
        assert_eq!(p.delete, vec!["gone.txt".to_string()]);
    }

    #[test]
    fn keep_files_are_never_overwritten_or_deleted() {
        let old = manifest(vec![file("options.ini", '1', &[('o', 10, 0, 10)]), file("cfg/x.ini", '2', &[('x', 3, 0, 3)])], &["options.ini", "cfg/**"]);
        let new = manifest(vec![file("options.ini", '9', &[('n', 12, 0, 12)])], &["options.ini", "cfg/**"]);
        let p = plan(&new, Some(&old), &|_| LocalState::Present { size: 1, verified: None });
        assert!(p.write.is_empty());
        assert!(p.delete.is_empty());
        // ...but they are written when missing.
        let p = plan(&new, Some(&old), &|_| LocalState::Missing);
        assert_eq!(p.write.len(), 1);
    }

    #[test]
    fn repair_rewrites_broken_files_and_never_copies_from_them() {
        let m = manifest(vec![file("a.bin", '1', &[('a', 10, 0, 10)]), file("b.bin", '2', &[('a', 10, 0, 10)])], &[]);
        let local = |p: &str| match p {
            "a.bin" => LocalState::Present { size: 10, verified: Some(false) },
            _ => LocalState::Present { size: 10, verified: Some(true) },
        };
        let p = plan(&m, Some(&m), &local);
        assert_eq!(p.write.len(), 1);
        assert_eq!(p.write[0].path, "a.bin");
        // b.bin is intact and holds the same segment: copied locally instead of downloaded.
        assert_eq!(p.reuse.len(), 1);
        assert_eq!(p.reuse[0].src_path, "b.bin");
        assert!(p.fetch.is_empty());
    }
}
