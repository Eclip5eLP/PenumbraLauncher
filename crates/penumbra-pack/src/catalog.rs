//! Catalog publishing (docs/04-server.md#get-penumbraapiv1catalogphp).
//!
//! Source of truth in git:
//! - `catalog/catalog.json`        meta: mirrors, groups, announcements, `items` (display order)
//! - `catalog/<id>/item.json`      one item (camelCase, see penumbra_core::catalog::Item); media paths relative
//! - `catalog/<id>/media/...`      media sources
//! - `catalog/<id>/build.toml`     build spec (builder.rs)
//!
//! Staged result: `catalog/meta.json` + `catalog/items.jsonl` (one signed item per line) and content-hashed
//! media files `media/<id>/<name>.<hash8>.<ext>` (immutable, cacheable forever).

use std::path::Path;

use anyhow::{Context, Result, bail};
use penumbra_core::manifest::Manifest;
use serde_json::{Map, Value, json};

use crate::workspace::Workspace;

pub fn item_path(ws: &Workspace, id: &str) -> std::path::PathBuf {
    ws.catalog().join(id).join("item.json")
}

fn read_json(p: &Path) -> Result<Value> {
    serde_json::from_slice(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?)
        .with_context(|| format!("parsing {}", p.display()))
}

fn write_json(p: &Path, v: &Value) -> Result<()> {
    let mut s = serde_json::to_string_pretty(v)?;
    s.push('\n');
    std::fs::write(p, s)?;
    Ok(())
}

/// Points `channel` of `item` at a built version (sizes come from its manifest), then stages the catalog.
pub fn promote(ws: &Workspace, item: &str, version: &str, channel: &str) -> Result<()> {
    let mpath = ws.version_dir(item, version).join("manifest.json");
    let manifest: Manifest = serde_json::from_slice(
        &std::fs::read(&mpath).with_context(|| format!("{} not found: build it first", ws.rel(&mpath)))?,
    )?;
    let ipath = item_path(ws, item);
    let mut v = read_json(&ipath)?;
    let channels = v
        .as_object_mut()
        .context("item.json must be an object")?
        .entry("channels")
        .or_insert_with(|| json!({}));
    let ch = channels.as_object_mut().context("channels must be an object")?.entry(channel).or_insert_with(|| json!({}));
    let ch = ch.as_object_mut().context("channel must be an object")?;
    ch.insert("version".into(), json!(version));
    ch.insert("size".into(), json!(manifest.total_size));
    ch.insert("download".into(), json!(manifest.download_size));
    write_json(&ipath, &v)?;
    eprintln!("{item}: {channel} -> {version}");
    stage_catalog(ws)
}

const MEDIA_FIELDS: &[&str] = &["icon", "logo", "bg", "bgVideo", "cover"];

/// Builds, validates and signs the catalog into the stage folder.
pub fn stage_catalog(ws: &Workspace) -> Result<()> {
    let cfg = ws.config()?;
    let key = ws.signing_key()?;
    let meta = read_json(&ws.catalog().join("catalog.json"))?;
    let order: Vec<String> = meta["items"]
        .as_array()
        .context("catalog.json: `items` must list item ids")?
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    let mut lines = vec![];
    for id in &order {
        let mut item = read_json(&item_path(ws, id))?;
        if item["id"] != json!(id) {
            bail!("{id}/item.json has a different id");
        }
        // Media: relative paths -> uploaded, content-hashed absolute URLs.
        let media_dir = ws.catalog().join(id).join("media");
        let stage_media = |rel: &str| -> Result<String> {
            if rel.starts_with("https://") {
                return Ok(rel.to_string());
            }
            let src = media_dir.join(rel);
            let bytes = std::fs::read(&src).with_context(|| format!("media {}", ws.rel(&src)))?;
            let p = Path::new(rel);
            let stem = p.file_stem().and_then(|s| s.to_str()).context("media name")?;
            let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("bin");
            let hash = &penumbra_core::chunk::hash_hex(&bytes)[..10];
            let target = format!("media/{id}/{stem}.{hash}.{ext}");
            ws.stage_write(&target, &bytes)?;
            Ok(format!("{}{}", cfg.server.public_url, target))
        };
        if let Some(media) = item.get_mut("media").and_then(Value::as_object_mut) {
            for f in MEDIA_FIELDS {
                if let Some(Value::String(rel)) = media.get(*f).cloned() {
                    media.insert((*f).into(), Value::String(stage_media(&rel)?));
                }
            }
        }
        if let Some(banners) = item.get_mut("banners").and_then(Value::as_array_mut) {
            for b in banners {
                if let Some(Value::String(rel)) = b.get("image").cloned() {
                    b["image"] = Value::String(stage_media(&rel)?);
                }
            }
        }
        // Type check against the launcher's model before signing.
        serde_json::from_value::<penumbra_core::catalog::Item>(item.clone())
            .with_context(|| format!("{id}/item.json doesn't match the catalog schema"))?;
        penumbra_core::sign::sign_item(&key, &mut item);
        lines.push(penumbra_core::sign::canonical_json(&item));
    }

    let mut out_meta = Map::new();
    out_meta.insert("format".into(), json!(1));
    out_meta.insert("generated".into(), json!(crate::now_rfc3339()));
    for k in ["mirrors", "groups", "announcements"] {
        out_meta.insert(k.into(), meta.get(k).cloned().unwrap_or(json!([])));
    }
    ws.stage_write("catalog/meta.json", serde_json::to_string_pretty(&Value::Object(out_meta))?.as_bytes())?;
    ws.stage_write("catalog/items.jsonl", (lines.join("\n") + "\n").as_bytes())?;
    eprintln!("catalog staged: {} items. Next: `penumbra-pack upload`.", lines.len());
    Ok(())
}

/// Prints channels of every catalog item.
pub fn list(ws: &Workspace, only: Option<&str>) -> Result<()> {
    let meta = read_json(&ws.catalog().join("catalog.json"))?;
    for id in meta["items"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if only.is_some_and(|o| o != id) {
            continue;
        }
        let item = read_json(&item_path(ws, id))?;
        println!("{id} ({}, {})", item["type"].as_str().unwrap_or("?"), item["access"].as_str().unwrap_or("public"));
        if let Some(chs) = item["channels"].as_object() {
            for (name, ch) in chs {
                println!(
                    "  {name:<8} {:<24} {}",
                    ch["version"].as_str().unwrap_or("-"),
                    ch["download"].as_u64().map(crate::human).unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}
