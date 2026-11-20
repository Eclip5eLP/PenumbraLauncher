//! `penumbra-pack`: publisher CLI (docs/03-content-format.md#publisher-cli-penumbra-pack).
//! Skeleton: the command surface exists, implementations land in milestone M2.

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "penumbra-pack", version, about = "Build, sign and publish Penumbra Launcher content")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create the manifest signing keypair (once).
    Keygen {
        #[arg(long, default_value = r"C:\Eclip5eDev\keys\manifest_ed25519.key")]
        out: String,
    },
    /// Chunk a build (folder, zip or build.toml) into a new item version.
    Build {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        input: String,
        #[arg(long)]
        no_unpack: bool,
    },
    /// Upload a built version to the server (chunks first, manifest last).
    Upload {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
    },
    /// Point a channel of an item at a version and publish the signed catalog.
    Promote {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
        #[arg(long, default_value = "stable")]
        channel: String,
    },
    /// List versions per channel.
    List {
        #[arg(long)]
        item: Option<String>,
    },
    /// Check that every chunk of a version exists on the server.
    Verify {
        #[arg(long)]
        item: String,
        #[arg(long)]
        version: String,
    },
    /// Delete chunks not referenced by the newest N versions of any item.
    Gc {
        #[arg(long, default_value_t = 3)]
        keep: usize,
    },
    /// Convert a GitHub release into a penumbra version (private repos, delta updates).
    ImportGithub {
        #[arg(long)]
        item: String,
        #[arg(long)]
        tag: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let name = match cli.cmd {
        Cmd::Keygen { .. } => "keygen",
        Cmd::Build { .. } => "build",
        Cmd::Upload { .. } => "upload",
        Cmd::Promote { .. } => "promote",
        Cmd::List { .. } => "list",
        Cmd::Verify { .. } => "verify",
        Cmd::Gc { .. } => "gc",
        Cmd::ImportGithub { .. } => "import-github",
    };
    bail!("`{name}` is not implemented yet (planned for milestone M2)")
}
