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
#[serde(rename_all = "camelCase", tag = "kind")]
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
