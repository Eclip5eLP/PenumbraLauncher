//! Shared building blocks for the Penumbra Launcher and the `penumbra-pack` publisher.
//!
//! - [`catalog`]: the catalog served by `penumbra/api/v1/catalog.php` (docs/04-server.md)
//! - [`manifest`]: per-version file manifests (docs/03-content-format.md)
//! - [`paths`]: validation of relative install paths (no traversal, no reserved Windows names)

pub mod catalog;
pub mod chunk;
pub mod manifest;
pub mod paths;
pub mod sign;
