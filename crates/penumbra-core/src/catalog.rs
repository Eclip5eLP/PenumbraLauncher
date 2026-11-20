//! Catalog types (docs/04-server.md, docs/11-sources-github.md). Serialized as camelCase JSON; the UI gets the
//! same shape (see apps/launcher/src/lib/types.ts — keep both in sync).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub format: u32,
    pub generated: String,
    #[serde(default)]
    pub mirrors: Vec<String>,
    #[serde(default)]
    pub announcements: Vec<Announcement>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ItemType {
    Game,
    Software,
    Installer,
    Link,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ItemStatus {
    #[default]
    Released,
    ComingSoon,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ItemType,
    pub group: String,
    pub name: String,
    #[serde(default)]
    pub tagline: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub status: ItemStatus,
    #[serde(default)]
    pub media: Media,
    /// `public` | `account` | `restricted`
    #[serde(default = "public")]
    pub access: String,
    #[serde(default)]
    pub locked: bool,
    /// Link items only.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub channels: BTreeMap<String, Channel>,
    #[serde(default)]
    pub news: Vec<NewsPost>,
    #[serde(default)]
    pub banners: Vec<Banner>,
    #[serde(default)]
    pub links: BTreeMap<String, String>,
    /// How to start it, for sources without a manifest `launch` (GitHub/URL). Relative to the install folder.
    #[serde(default)]
    pub launch: Option<crate::manifest::Launch>,
    /// Things that must be on the PC (checked before launch), e.g. Python.
    #[serde(default)]
    pub requires: Vec<Requirement>,
    /// Detached ed25519 signature over the canonical item JSON (without this field).
    #[serde(default)]
    pub sig: Option<String>,
}

fn public() -> String {
    "public".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub bg_video: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// rename_all renames the variants ("github"); rename_all_fields the variants' fields ("stripRoot").
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum Source {
    Penumbra,
    Github {
        repo: String,
        #[serde(default)]
        strip_root: bool,
        #[serde(default = "yes")]
        unpack: bool,
        #[serde(default)]
        keep: Vec<String>,
    },
    Url {
        #[serde(default = "yes")]
        unpack: bool,
        #[serde(default)]
        keep: Vec<String>,
    },
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    /// Resolved current version; `None` = nothing published yet (e.g. GitHub repo without releases).
    #[serde(default)]
    pub version: Option<String>,
    /// Installed size in bytes.
    #[serde(default)]
    pub size: Option<u64>,
    /// Full download size in bytes.
    #[serde(default)]
    pub download: Option<u64>,
    #[serde(default)]
    pub access: Option<String>,
    /// GitHub channels: `release` | `branch` | `tag`.
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub required: bool,

    // --- GitHub channel settings (signed, set by the publisher) ---
    /// Release mode: glob over asset names. None/no match = the release's source zip.
    #[serde(default)]
    pub asset: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    /// Branch mode: branch name (default: the repo's default branch).
    #[serde(default)]
    pub branch: Option<String>,
    /// Tag mode: glob over tag names, e.g. `v*`.
    #[serde(default)]
    pub tag_pattern: Option<String>,
    /// Only install this sub-folder of the archive.
    #[serde(default)]
    pub path: Option<String>,
    /// Globs of archive paths not to install.
    #[serde(default)]
    pub exclude: Vec<String>,

    // --- Resolved download (GitHub: from the server resolver / client; URL sources: signed) ---
    #[serde(default)]
    pub url: Option<String>,
    /// Hex sha256 of the downloaded file, when known.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Publish date (RFC 3339) of the resolved version.
    #[serde(default)]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// `python` (more kinds later)
    pub kind: String,
    #[serde(default)]
    pub version: Option<String>,
    /// Where to get it.
    #[serde(default)]
    pub help: Option<String>,
}

/// GitHub download URLs a resolver may hand out for `repo` (owner/name). The repo itself is pinned by the signed
/// catalog item, so an unsigned resolution can only point at that repository's own files.
pub fn github_url_allowed(repo: &str, url: &str) -> bool {
    let repo = repo.to_ascii_lowercase();
    let url = url.to_ascii_lowercase();
    !repo.contains("..")
        && repo.split('/').count() == 2
        && [
            format!("https://github.com/{repo}/releases/download/"),
            format!("https://codeload.github.com/{repo}/zip/"),
        ]
        .iter()
        .any(|p| url.starts_with(p.as_str()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn github_source_fields_are_camel_case() {
        let s: super::Source =
            serde_json::from_str(r#"{ "kind": "github", "repo": "a/b", "stripRoot": true, "unpack": false }"#).unwrap();
        match s {
            super::Source::Github { strip_root, unpack, .. } => assert!(strip_root && !unpack),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn github_urls_are_pinned_to_the_repo() {
        let r = "Eclip5eLP/Pirate-Galaxy-AFK-Bot";
        assert!(super::github_url_allowed(r, "https://github.com/Eclip5eLP/Pirate-Galaxy-AFK-Bot/releases/download/v1/a.zip"));
        assert!(super::github_url_allowed(r, "https://codeload.github.com/eclip5elp/pirate-galaxy-afk-bot/zip/abc123"));
        assert!(!super::github_url_allowed(r, "https://github.com/evil/repo/releases/download/v1/a.zip"));
        assert!(!super::github_url_allowed(r, "https://evil.com/Eclip5eLP/Pirate-Galaxy-AFK-Bot/releases/download/"));
        assert!(!super::github_url_allowed(r, "http://github.com/Eclip5eLP/Pirate-Galaxy-AFK-Bot/releases/download/v1/a.zip"));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsPost {
    pub id: String,
    /// `event` | `notice` | `info`
    pub category: String,
    pub title: String,
    pub date: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Banner {
    pub id: String,
    pub image: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}
