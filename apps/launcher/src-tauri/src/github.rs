//! GitHub channel resolution (docs/11-sources-github.md). The server normally resolves channels and sends them in
//! the catalog's unsigned `resolved` map; this module (a) applies that map safely and (b) resolves directly against
//! the GitHub API when the server sent nothing (old server / server can't reach GitHub).

use std::collections::BTreeMap;

use penumbra_core::catalog::{Catalog, Channel, Source, github_url_allowed};
use penumbra_core::plan::glob_match;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    pub version: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub download: Option<u64>,
    pub date: Option<String>,
}

pub type ResolvedMap = BTreeMap<String, BTreeMap<String, Resolved>>;

/// Copies resolved downloads into GitHub items' channels. URLs outside the item's repository are ignored.
pub fn apply(catalog: &mut Catalog, resolved: &ResolvedMap) {
    for item in &mut catalog.items {
        let Some(Source::Github { repo, .. }) = &item.source else { continue };
        let Some(chs) = resolved.get(&item.id) else { continue };
        for (name, r) in chs {
            let (Some(ch), Some(url)) = (item.channels.get_mut(name), &r.url) else { continue };
            if !github_url_allowed(repo, url) {
                tracing::warn!(item = %item.id, %url, "ignoring resolved URL outside the item's repository");
                continue;
            }
            fill(ch, r);
        }
    }
}

fn fill(ch: &mut Channel, r: &Resolved) {
    ch.version = r.version.clone();
    ch.url = r.url.clone();
    ch.sha256 = r.sha256.clone().filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()));
    ch.download = r.download;
    ch.date = r.date.clone();
}

async fn api(http: &reqwest::Client, path: &str) -> Option<Value> {
    let r = http
        .get(format!("https://api.github.com/{path}"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .ok()?;
    if !r.status().is_success() {
        tracing::warn!(%path, status = %r.status(), "GitHub API");
        return None;
    }
    serde_json::from_slice(&r.bytes().await.ok()?).ok()
}

/// Resolves every GitHub channel directly (fallback path).
pub async fn resolve_direct(http: &reqwest::Client, catalog: &mut Catalog) {
    for item in &mut catalog.items {
        let Some(Source::Github { repo, .. }) = item.source.clone() else { continue };
        for ch in item.channels.values_mut() {
            let r = match ch.mode.as_deref().unwrap_or("release") {
                "release" => release(http, &repo, ch).await,
                "branch" => branch(http, &repo, ch).await,
                "tag" => tag(http, &repo, ch).await,
                _ => None,
            };
            if let Some(r) = r {
                fill(ch, &r);
            }
        }
    }
}

async fn release(http: &reqwest::Client, repo: &str, ch: &Channel) -> Option<Resolved> {
    let rels = api(http, &format!("repos/{repo}/releases?per_page=20")).await?;
    let rel = rels.as_array()?.iter().find(|r| {
        !r["draft"].as_bool().unwrap_or(false) && (ch.prerelease || !r["prerelease"].as_bool().unwrap_or(false))
    })?;
    let tag = rel["tag_name"].as_str()?.to_string();
    let date = rel["published_at"].as_str().map(String::from);
    if let Some(pattern) = &ch.asset {
        for a in rel["assets"].as_array().into_iter().flatten() {
            if glob_match(pattern, a["name"].as_str().unwrap_or("")) {
                return Some(Resolved {
                    version: Some(tag),
                    url: a["browser_download_url"].as_str().map(String::from),
                    sha256: a["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(String::from),
                    download: a["size"].as_u64(),
                    date,
                });
            }
        }
    }
    Some(Resolved {
        url: Some(format!("https://codeload.github.com/{repo}/zip/refs/tags/{}", crate::net::path_segment(&tag))),
        version: Some(tag),
        date,
        ..Default::default()
    })
}

async fn branch(http: &reqwest::Client, repo: &str, ch: &Channel) -> Option<Resolved> {
    let branch = match &ch.branch {
        Some(b) => b.clone(),
        None => api(http, &format!("repos/{repo}")).await?["default_branch"].as_str()?.to_string(),
    };
    let c = api(http, &format!("repos/{repo}/commits/{}", crate::net::path_segment(&branch))).await?;
    let sha = c["sha"].as_str()?;
    Some(Resolved {
        version: Some(format!("{branch}@{}", &sha[..7.min(sha.len())])),
        url: Some(format!("https://codeload.github.com/{repo}/zip/{sha}")),
        date: c["commit"]["committer"]["date"].as_str().map(String::from),
        ..Default::default()
    })
}

async fn tag(http: &reqwest::Client, repo: &str, ch: &Channel) -> Option<Resolved> {
    let tags = api(http, &format!("repos/{repo}/tags?per_page=50")).await?;
    let pattern = ch.tag_pattern.as_deref().unwrap_or("*");
    let name = tags.as_array()?.iter().filter_map(|t| t["name"].as_str()).find(|n| glob_match(pattern, n))?;
    Some(Resolved {
        version: Some(name.to_string()),
        url: Some(format!("https://codeload.github.com/{repo}/zip/refs/tags/{}", crate::net::path_segment(name))),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_urls_are_not_applied() {
        let mut c: Catalog = serde_json::from_value(serde_json::json!({
            "format": 1, "generated": "x",
            "items": [{ "id": "bot", "type": "software", "group": "software", "name": "Bot",
                        "source": { "kind": "github", "repo": "me/bot" },
                        "channels": { "stable": { "mode": "release" }, "dev": { "mode": "branch" } } }]
        }))
        .unwrap();
        let mut res = ResolvedMap::new();
        res.entry("bot".into()).or_default().insert(
            "stable".into(),
            Resolved { version: Some("v1".into()), url: Some("https://evil.example/x.zip".into()), ..Default::default() },
        );
        res.entry("bot".into()).or_default().insert(
            "dev".into(),
            Resolved {
                version: Some("main@abc1234".into()),
                url: Some("https://codeload.github.com/me/bot/zip/abc".into()),
                ..Default::default()
            },
        );
        apply(&mut c, &res);
        assert_eq!(c.items[0].channels["stable"].version, None);
        assert_eq!(c.items[0].channels["dev"].version.as_deref(), Some("main@abc1234"));
    }
}
