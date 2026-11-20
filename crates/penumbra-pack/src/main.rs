//! `penumbra-pack`: publisher CLI (docs/03-content-format.md#publisher-cli-penumbra-pack).
//!
//! Typical release:
//!   penumbra-pack build --item gracidea --version 0.3.15 --input D:\builds\gracidea
//!   penumbra-pack promote --item gracidea --version 0.3.15 --channel stable
//!   penumbra-pack upload

mod builder;
mod catalog;
mod install_test;
mod remote;
mod workspace;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use penumbra_core::manifest::Manifest;

use workspace::Workspace;

#[derive(Parser)]
#[command(name = "penumbra-pack", version, about = "Build, sign and publish Penumbra Launcher content")]
struct Cli {
    /// Repository root (contains catalog/ and out/). Default: current folder.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// Config file. Default: $PENUMBRA_PACK_CONFIG or tools/private/pack.toml.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the manifest/catalog signing keypair (once). Refuses to overwrite.
    Keygen,
    /// Chunk a build into a new item version and stage it for upload.
    Build {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
        /// Build folder or zip. With catalog/<item>/build.toml parts: the folder the parts are relative to.
        #[arg(long)]
        input: PathBuf,
        /// zstd level (1-22).
        #[arg(long, default_value_t = 19)]
        level: i32,
        /// Don't ask the server which chunks already exist.
        #[arg(long)]
        offline: bool,
    },
    /// Point a channel of an item at a built version, then stage the signed catalog.
    Promote {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
        #[arg(long, default_value = "stable")]
        channel: String,
    },
    /// Re-sign and stage the catalog (after editing catalog/ metadata, news or media).
    Catalog,
    /// Upload everything staged (chunks, media, manifests, catalog last).
    Upload {
        /// Only list what would be uploaded.
        #[arg(long)]
        dry_run: bool,
    },
    /// List items and their channel versions.
    List {
        #[arg(long)]
        item: Option<String>,
    },
    /// Check that every chunk of a built version exists on the server.
    Verify {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
    },
    /// Upload the server API code (server/penumbra) to the server root. Never deletes anything there.
    DeployServer,
    /// End-to-end check from the public URL: verify catalog + manifest signatures, download every chunk
    /// (via curl), rebuild the install into --dest and check all file hashes. What the launcher will do.
    InstallTest {
        #[arg(long)]
        item: String,
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(long)]
        dest: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(r) => r,
        None => std::env::current_dir()?,
    };
    let ws = Workspace::new(root, cli.config);

    match cli.cmd {
        Cmd::Keygen => keygen(&ws),
        Cmd::Build { item, version, input, level, offline } => {
            let spec_path = ws.catalog().join(&item).join("build.toml");
            let spec = if spec_path.exists() {
                let s: builder::BuildSpec = toml::from_str(&std::fs::read_to_string(&spec_path)?)
                    .with_context(|| format!("parsing {}", ws.rel(&spec_path)))?;
                if s.item != item {
                    bail!("{} is for item `{}`", ws.rel(&spec_path), s.item);
                }
                s
            } else {
                builder::BuildSpec { item: item.clone(), ..Default::default() }
            };
            builder::build(&ws, builder::BuildArgs { version, input, spec, level: level.clamp(1, 22), offline })
        }
        Cmd::Promote { item, version, channel } => catalog::promote(&ws, &item, &version, &channel),
        Cmd::Catalog => catalog::stage_catalog(&ws),
        Cmd::Upload { dry_run } => upload(&ws, dry_run),
        Cmd::List { item } => catalog::list(&ws, item.as_deref()),
        Cmd::Verify { item, version } => verify(&ws, &item, &version),
        Cmd::DeployServer => deploy_server(&ws),
        Cmd::InstallTest { item, channel, dest } => install_test::run(&ws, &item, &channel, &dest),
    }
}

fn keygen(ws: &Workspace) -> Result<()> {
    let cfg = ws.config()?;
    let key_path = PathBuf::from(&cfg.keys.manifest);
    if key_path.exists() {
        bail!("{} already exists; refusing to overwrite a signing key", key_path.display());
    }
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    if let Some(d) = key_path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&key_path, penumbra_core::sign::signing_key_to_b64(&key) + "\n")?;
    let pub_path = ws.root.join(&cfg.keys.manifest_pub);
    if let Some(d) = pub_path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&pub_path, penumbra_core::sign::verifying_key_to_b64(&key.verifying_key()) + "\n")?;
    eprintln!("secret key: {} (back it up, never commit it)", key_path.display());
    eprintln!("public key: {}", pub_path.display());
    Ok(())
}

fn upload(ws: &Workspace, dry_run: bool) -> Result<()> {
    let files = ws.staged_files()?;
    if files.is_empty() {
        eprintln!("nothing staged");
        return Ok(());
    }
    let bytes: u64 = files.iter().map(|(p, _)| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)).sum();
    eprintln!("{} files, {} to upload", files.len(), human(bytes));
    if dry_run {
        for (_, r) in &files {
            println!("{r}");
        }
        return Ok(());
    }
    ws.config()?.server.upload(&files)?;
    std::fs::remove_dir_all(ws.stage())?;
    eprintln!("uploaded; stage cleared");
    Ok(())
}

fn verify(ws: &Workspace, item: &str, version: &str) -> Result<()> {
    let mpath = ws.version_dir(item, version).join("manifest.json");
    let m: Manifest = serde_json::from_slice(&std::fs::read(&mpath).context("manifest not built locally")?)?;
    let server = ws.config()?.server;
    let remote = server.chunk_index()?;
    let mut missing = std::collections::BTreeSet::new();
    for c in m.files.iter().flat_map(|f| &f.chunks) {
        match remote.get(&c.hash) {
            Some(&s) if s == c.csize => {}
            _ => {
                missing.insert(c.hash.clone());
            }
        }
    }
    let base = format!("content/{item}/{version}");
    let ls = server.run(&format!("cd '{}' && ls {base}.json.zst {base}.json.sig 2>&1 || true", server.root))?;
    if missing.is_empty() && !ls.contains("No such file") {
        eprintln!("ok: manifest and all {} chunks are on the server", m.files.iter().flat_map(|f| &f.chunks).count());
        Ok(())
    } else {
        bail!("{} chunks missing or wrong size on the server; manifest: {}", missing.len(), ls.trim())
    }
}

fn deploy_server(ws: &Workspace) -> Result<()> {
    let src = ws.root.join("server/penumbra");
    let mut files = vec![];
    for e in walkdir::WalkDir::new(&src).min_depth(1) {
        let e = e?;
        if e.file_type().is_file() {
            let rel = e.path().strip_prefix(&src)?.to_string_lossy().replace('\\', "/");
            files.push((e.path().to_path_buf(), rel));
        }
    }
    eprintln!("deploying {} files from server/penumbra", files.len());
    ws.config()?.server.upload(&files)?;
    eprintln!("done");
    Ok(())
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

/// UTC timestamp, RFC 3339, without pulling in a date crate.
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (secs / 86400, secs % 86400);
    // Civil-from-days (Howard Hinnant).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
