//! Version manifest: the complete file list of one item version (docs/03-content-format.md).

use serde::{Deserialize, Serialize};

use crate::paths::{validate_rel_path, PathError};

pub const MANIFEST_FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: u32,
    pub item: String,
    pub version: String,
    /// RFC 3339 timestamp.
    pub created: String,
    /// Installed size in bytes.
    pub total_size: u64,
    /// Sum of compressed chunk sizes for a full install.
    pub download_size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<Launch>,
    pub files: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs: Vec<String>,
    /// Globs of user files that updates never overwrite or delete.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<String>,
    /// Built by the launcher from a GitHub/URL archive: files carry hashes but no chunks.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Launch {
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    #[serde(default)]
    pub admin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    /// BLAKE3 of the whole file, lowercase hex.
    pub hash: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exec: bool,
    pub chunks: Vec<ChunkRef>,
}

/// One segment of a file: `size` bytes taken from chunk `hash` starting at `offset`.
/// Big files use whole content-defined chunks (`offset` 0, `len` omitted = `size`). Small files are slices of a
/// shared pack chunk (ADR-016), where `len` is the pack's full uncompressed length.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkRef {
    /// BLAKE3 of the uncompressed chunk, lowercase hex.
    pub hash: String,
    /// Bytes of this segment.
    pub size: u64,
    /// Compressed (zstd) size of the whole chunk on the server.
    pub csize: u64,
    /// Start of this segment inside the uncompressed chunk.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: u64,
    /// Uncompressed length of the whole chunk, when it differs from `size` (pack chunks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub len: Option<u64>,
}

fn is_zero(v: &u64) -> bool {
    *v == 0
}

impl ChunkRef {
    pub fn chunk_len(&self) -> u64 {
        self.len.unwrap_or(self.size)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("unsupported manifest format {0}")]
    Format(u32),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("duplicate path (case-insensitive): {0}")]
    Duplicate(String),
    #[error("chunk sizes of {0} don't add up to the file size")]
    ChunkSizes(String),
    #[error("bad chunk reference in {0}")]
    BadChunk(String),
}

impl Manifest {
    /// Structural checks that must pass before a manifest is used for anything.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.format != MANIFEST_FORMAT {
            return Err(ManifestError::Format(self.format));
        }
        let mut seen = std::collections::HashSet::new();
        for f in &self.files {
            validate_rel_path(&f.path)?;
            if !seen.insert(f.path.to_lowercase()) {
                return Err(ManifestError::Duplicate(f.path.clone()));
            }
            if !self.local && f.chunks.iter().map(|c| c.size).sum::<u64>() != f.size {
                return Err(ManifestError::ChunkSizes(f.path.clone()));
            }
            for c in &f.chunks {
                if !crate::chunk::is_valid_hash(&c.hash) || c.offset + c.size > c.chunk_len() || c.chunk_len() > 64 << 20 {
                    return Err(ManifestError::BadChunk(f.path.clone()));
                }
            }
        }
        for d in &self.dirs {
            validate_rel_path(d)?;
        }
        if let Some(l) = &self.launch {
            validate_rel_path(&l.exe)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, size: u64) -> FileEntry {
        let h = "ab".repeat(32);
        FileEntry {
            path: path.into(),
            size,
            hash: h.clone(),
            exec: false,
            chunks: vec![ChunkRef { hash: h, size, csize: size, offset: 0, len: None }],
        }
    }

    #[test]
    fn pack_slices_must_fit_their_chunk() {
        let mut f = file("a", 10);
        f.chunks[0].offset = 5;
        f.chunks[0].len = Some(12);
        assert!(manifest(vec![f.clone()]).validate().is_err());
        f.chunks[0].len = Some(15);
        manifest(vec![f]).validate().unwrap();
    }

    fn manifest(files: Vec<FileEntry>) -> Manifest {
        Manifest {
            format: 1,
            item: "x".into(),
            version: "1".into(),
            created: "2026-11-20T00:00:00Z".into(),
            total_size: 0,
            download_size: 0,
            launch: None,
            files,
            dirs: vec![],
            keep: vec![],
            local: false,
        }
    }

    #[test]
    fn valid_manifest_roundtrips() {
        let m = manifest(vec![file("a.exe", 3), file("data/b.bin", 5)]);
        m.validate().unwrap();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), m);
    }

    #[test]
    fn rejects_case_duplicates_and_bad_sizes() {
        assert!(manifest(vec![file("A.exe", 1), file("a.EXE", 1)]).validate().is_err());
        let mut f = file("a", 10);
        f.size = 11;
        assert!(manifest(vec![f]).validate().is_err());
        assert!(manifest(vec![file("../evil", 1)]).validate().is_err());
    }
}
