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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChunkRef {
    /// BLAKE3 of the uncompressed chunk, lowercase hex.
    pub hash: String,
    /// Uncompressed size.
    pub size: u64,
    /// Compressed (zstd) size on the server.
    pub csize: u64,
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
            if f.chunks.iter().map(|c| c.size).sum::<u64>() != f.size {
                return Err(ManifestError::ChunkSizes(f.path.clone()));
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
        FileEntry {
            path: path.into(),
            size,
            hash: "00".into(),
            exec: false,
            chunks: vec![ChunkRef { hash: "00".into(), size, csize: size }],
        }
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
