//! `install-test`: a dependency-free reference client (uses the system `curl`) that exercises the public
//! server exactly like the launcher will: signed catalog -> signed manifest -> chunks -> files.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use penumbra_core::chunk;
use penumbra_core::manifest::Manifest;
use penumbra_core::sign;
use serde_json::Value;

use crate::workspace::Workspace;

fn curl(url: &str) -> Result<(Vec<u8>, String)> {
    let out = Command::new("curl").args(["-sS", "-f", "-D", "-", "--compressed", url]).output().context("running curl")?;
    if !out.status.success() {
        bail!("GET {url}: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    // Headers come first, separated by a blank line (possibly several header blocks after redirects).
    let raw = out.stdout;
    let mut split = 0;
    let mut i = 0;
    while i + 3 < raw.len() {
        if &raw[i..i + 4] == b"\r\n\r\n" {
            split = i + 4;
            if !raw[split..].starts_with(b"HTTP/") {
                break;
            }
        }
        i += 1;
    }
    Ok((raw[split..].to_vec(), String::from_utf8_lossy(&raw[..split]).to_lowercase()))
}

pub fn run(ws: &Workspace, item: &str, channel: &str, dest: &Path) -> Result<()> {
    let cfg = ws.config()?;
    let base = cfg.server.public_url.clone();
    let pubkey = sign::verifying_key_from_b64(&std::fs::read_to_string(ws.root.join(&cfg.keys.manifest_pub))?)?;

    let (body, _) = curl(&format!("{base}api/v1/catalog.php"))?;
    let catalog: Value = serde_json::from_slice(&body)?;
    let items = catalog["items"].as_array().context("catalog has no items")?;
    for it in items {
        sign::verify_item(&pubkey, it).with_context(|| format!("signature of item {}", it["id"]))?;
    }
    eprintln!("catalog: {} items, all signatures valid", items.len());
    let it = items.iter().find(|i| i["id"] == item).context("item not in catalog")?;
    let version = it["channels"][channel]["version"].as_str().context("channel has no version")?.to_string();

    let mbase = format!("{base}content/{item}/{}", version.replace('+', "%2B"));
    let (zst, _) = curl(&format!("{mbase}.json.zst"))?;
    let (sig, _) = curl(&format!("{mbase}.json.sig"))?;
    let json = zstd::stream::decode_all(&zst[..])?;
    sign::verify_bytes(&pubkey, &json, std::str::from_utf8(&sig)?)?;
    let m: Manifest = serde_json::from_slice(&json)?;
    m.validate()?;
    eprintln!("manifest {item} {version}: signature valid, {} files", m.files.len());

    // Download each distinct chunk once (sequential: this is a correctness test, not a speed test).
    let mut wanted: Vec<(&str, u64)> = vec![];
    let mut seen = std::collections::HashSet::new();
    for c in m.files.iter().flat_map(|f| &f.chunks) {
        if seen.insert(c.hash.as_str()) {
            wanted.push((&c.hash, c.chunk_len()));
        }
    }
    let mut cache: HashMap<String, Vec<u8>> = HashMap::new();
    let mut cf: HashMap<String, usize> = HashMap::new();
    let started = std::time::Instant::now();
    let mut bytes = 0u64;
    for (i, (h, len)) in wanted.iter().enumerate() {
        let (stored, headers) = curl(&format!("{base}{}", chunk::chunk_path(h)))?;
        bytes += stored.len() as u64;
        let status = headers.lines().find_map(|l| l.strip_prefix("cf-cache-status: ")).unwrap_or("none").trim().to_string();
        *cf.entry(status).or_default() += 1;
        cache.insert(h.to_string(), chunk::decode_chunk(&stored, h, *len).with_context(|| format!("chunk {h}"))?);
        eprint!("\r  chunks {}/{}", i + 1, wanted.len());
    }
    let secs = started.elapsed().as_secs_f64();
    eprintln!("\n  {} in {secs:.1}s ({}/s), cf-cache-status: {cf:?}", crate::human(bytes), crate::human((bytes as f64 / secs) as u64));

    if dest.exists() {
        std::fs::remove_dir_all(dest)?;
    }
    for f in &m.files {
        let mut data = Vec::with_capacity(f.size as usize);
        for c in &f.chunks {
            let src = &cache[&c.hash];
            data.extend_from_slice(&src[c.offset as usize..(c.offset + c.size) as usize]);
        }
        if chunk::hash_hex(&data) != f.hash {
            bail!("file hash mismatch: {}", f.path);
        }
        let p = dest.join(&f.path);
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, data)?;
    }
    for d in &m.dirs {
        std::fs::create_dir_all(dest.join(d))?;
    }
    eprintln!("installed {} files to {} — all hashes match", m.files.len(), dest.display());
    Ok(())
}
