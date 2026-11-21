//! Local folders used by the publisher:
//! - `catalog/`            item metadata, media sources, build specs (in git)
//! - `out/stage/`          files waiting to be uploaded, laid out exactly like the server root
//! - `out/<item>/<ver>/`   readable manifest + signature of each built version
//! - `out/index.tsv`       known chunks (server + staged): hash, compressed size

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ed25519_dalek::SigningKey;

use crate::remote::Config;

pub struct Workspace {
    pub root: PathBuf,
    config_path: PathBuf,
}

impl Workspace {
    pub fn new(root: PathBuf, config_path: Option<PathBuf>) -> Self {
        let config_path = config_path
            .or_else(|| std::env::var_os("PENUMBRA_PACK_CONFIG").map(PathBuf::from))
            .unwrap_or_else(|| root.join("tools/private/pack.toml"));
        Self { root, config_path }
    }

    pub fn config(&self) -> Result<Config> {
        Config::load(&self.config_path)
    }

    pub fn out(&self) -> PathBuf {
        self.root.join("out")
    }
    pub fn stage(&self) -> PathBuf {
        self.out().join("stage")
    }
    pub fn catalog(&self) -> PathBuf {
        self.root.join("catalog")
    }
    pub fn version_dir(&self, item: &str, version: &str) -> PathBuf {
        self.out().join(item).join(version)
    }

    pub fn stage_write(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        let p = self.stage().join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, bytes).with_context(|| format!("writing {}", p.display()))
    }

    /// All staged files as (local path, server-relative path), ordered for a consistent upload:
    /// chunks, then media, then content (manifests), then the catalog.
    pub fn staged_files(&self) -> Result<Vec<(PathBuf, String)>> {
        let stage = self.stage();
        let mut files = vec![];
        if stage.exists() {
            for e in walkdir::WalkDir::new(&stage).min_depth(1) {
                let e = e?;
                if e.file_type().is_file() {
                    let rel = e.path().strip_prefix(&stage)?.to_string_lossy().replace('\\', "/");
                    files.push((e.path().to_path_buf(), rel));
                }
            }
        }
        let rank = |r: &str| match r.split('/').next() {
            Some("chunks") => 0,
            Some("media") => 1,
            Some("content") | Some("data") => 2,
            Some("catalog") if r.ends_with("items.jsonl") => 4, // the very last file: makes the release live
            Some("catalog") => 3,
            // Launcher updater manifests point at the installer: they must arrive after it.
            Some("launcher") if r.ends_with(".json") => 6,
            _ => 5,
        };
        files.sort_by(|a, b| rank(&a.1).cmp(&rank(&b.1)).then(a.1.cmp(&b.1)));
        Ok(files)
    }

    fn index_path(&self) -> PathBuf {
        self.out().join("index.tsv")
    }

    pub fn load_index(&self) -> Result<HashMap<String, u64>> {
        let Ok(text) = std::fs::read_to_string(self.index_path()) else { return Ok(HashMap::new()) };
        Ok(text
            .lines()
            .filter_map(|l| {
                let (h, s) = l.split_once('\t')?;
                Some((h.to_string(), s.parse().ok()?))
            })
            .collect())
    }

    pub fn save_index(&self, idx: &HashMap<String, u64>) -> Result<()> {
        std::fs::create_dir_all(self.out())?;
        let mut lines: Vec<String> = idx.iter().map(|(h, s)| format!("{h}\t{s}")).collect();
        lines.sort();
        std::fs::write(self.index_path(), lines.join("\n"))?;
        Ok(())
    }

    pub fn signing_key(&self) -> Result<SigningKey> {
        let path = self.config()?.keys.manifest;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading signing key {path}"))?;
        Ok(penumbra_core::sign::signing_key_from_b64(&text)?)
    }

    pub fn rel<'a>(&self, p: &'a Path) -> std::borrow::Cow<'a, str> {
        p.strip_prefix(&self.root).unwrap_or(p).to_string_lossy()
    }
}
