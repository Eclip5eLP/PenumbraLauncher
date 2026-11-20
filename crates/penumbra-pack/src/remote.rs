//! Config + talking to the server over SSH (upload = a tar stream into `tar -x` on the server).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub server: Server,
    pub keys: Keys,
}

#[derive(Debug, Deserialize)]
pub struct Server {
    /// `user@host`
    pub ssh: String,
    /// Private key file for ssh `-i` (optional: default ssh config otherwise).
    pub identity: Option<String>,
    /// Run remote commands as this user via `sudo -n -u` (optional).
    pub run_as: Option<String>,
    /// Absolute path of the penumbra root on the server (served as `public_url`).
    pub root: String,
    /// Public URL of `root`, with trailing slash.
    pub public_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Keys {
    /// Manifest/catalog ed25519 secret key (base64 seed).
    pub manifest: String,
    /// Public key file committed to the repo.
    pub manifest_pub: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config {} (see docs/03-content-format.md)", path.display()))?;
        let mut c: Config = toml::from_str(&text).context("parsing config")?;
        if !c.server.public_url.ends_with('/') {
            c.server.public_url.push('/');
        }
        Ok(c)
    }
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

impl Server {
    fn command(&self, remote: &str) -> Command {
        let remote = match &self.run_as {
            Some(u) => format!("sudo -n -u {} bash -c {}", sh_quote(u), sh_quote(remote)),
            None => format!("bash -c {}", sh_quote(remote)),
        };
        let mut cmd = Command::new("ssh");
        if let Some(id) = &self.identity {
            cmd.arg("-i").arg(id);
        }
        cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"]).arg(&self.ssh).arg(remote);
        cmd
    }

    /// Runs a command on the server and returns stdout.
    pub fn run(&self, remote: &str) -> Result<String> {
        let out = self.command(remote).output().context("starting ssh")?;
        if !out.status.success() {
            bail!("remote command failed ({}): {}", out.status, String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Lists chunks on the server: hash -> stored (compressed) size.
    pub fn chunk_index(&self) -> Result<HashMap<String, u64>> {
        let root = sh_quote(&self.root);
        let out = self.run(&format!(
            "mkdir -p {root}/chunks && cd {root} && find chunks -name '*.bin' -printf '%f %s\\n'"
        ))?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let (name, size) = l.split_once(' ')?;
                Some((name.strip_suffix(".bin")?.to_string(), size.trim().parse().ok()?))
            })
            .collect())
    }

    /// Streams `files` (local path, remote path relative to root) as one tar archive into the server root.
    /// Files are extracted in the given order, so put chunks first and manifests/catalog last.
    pub fn upload(&self, files: &[(PathBuf, String)]) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        let root = sh_quote(&self.root);
        let mut child = self
            .command(&format!("mkdir -p {root} && umask 022 && tar -x --no-same-owner -C {root} -f -"))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("starting ssh")?;
        {
            let stdin = child.stdin.take().expect("piped");
            let mut tar = tar::Builder::new(std::io::BufWriter::with_capacity(1 << 20, stdin));
            tar.mode(tar::HeaderMode::Deterministic);
            let total = files.len();
            for (i, (local, remote)) in files.iter().enumerate() {
                let mut f = std::fs::File::open(local).with_context(|| format!("opening {}", local.display()))?;
                let meta = f.metadata()?;
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let mut header = tar::Header::new_gnu();
                header.set_size(meta.len());
                header.set_mode(0o644);
                header.set_mtime(mtime);
                header.set_entry_type(tar::EntryType::Regular);
                tar.append_data(&mut header, remote, &mut f).with_context(|| format!("sending {remote}"))?;
                if (i + 1) % 200 == 0 || i + 1 == total {
                    eprint!("\r  uploaded {}/{} files", i + 1, total);
                }
            }
            let mut w = tar.into_inner().context("finishing tar")?;
            w.flush()?;
        }
        eprintln!();
        let out = child.wait_with_output()?;
        if !out.status.success() {
            bail!("upload failed: {}", String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(())
    }
}
