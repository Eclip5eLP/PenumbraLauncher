# 09 — Self-update & release

## Installer
- Tauri NSIS bundle, `installMode: currentUser` → `%LOCALAPPDATA%\Programs\Penumbra Launcher`, no admin prompt.
- WebView2: `embedBootstrapper` (Win 10 without WebView2 still works; Win 11 has it built in).
- Registers `penumbra://` URL protocol, Start menu shortcut, optional desktop shortcut, uninstaller
  (asks whether to keep games — default keep; games live in libraries, not in the program folder).

## Updater
- `tauri-plugin-updater`, endpoints `https://eclip5e.com/penumbra/launcher/latest.json` (beta: `latest-beta.json`).
- Updates are signed with the Tauri updater key (minisign). Private key: `C:\Eclip5eDev\keys\updater.key`,
  password in `updater.key.password` next to it — never in git. Public key committed as
  `apps/launcher/src-tauri/updater.key.pub` (goes into `tauri.conf.json` -> `plugins.updater.pubkey` in M6).
- Behaviour: check at start (after first paint, 10 s delay) and every 6 h; download in background; show
  "Restart to update" pill in the title bar; applied on restart, or automatically on next start if
  `autoUpdateLauncher`. Never interrupt running downloads: the pill waits; restart pauses jobs (they resume).
- `latest.json` may contain `"minimum": "<ver>"` → forced update screen for breaking server changes.

## Versioning
SemVer `MAJOR.MINOR.PATCH`, `-beta.N` for beta channel. Single source of truth: `apps/launcher/src-tauri/tauri.conf.json`
version (Cargo.toml + package.json kept in sync by `tools/release.ps1`). Changelog in `CHANGELOG.md`.

## Release process (`tools/release.ps1 -Version 1.2.0 [-Channel beta]`)
1. Check clean git tree, run tests (`cargo test`, `npm run check`, `npm test`).
2. Bump versions, update CHANGELOG, commit, tag `launcher-v1.2.0`.
3. `npm run tauri build` with `TAURI_SIGNING_PRIVATE_KEY` from the key file.
4. Generate `latest*.json` (version, notes, pub_date, url, signature).
5. Upload installer + sig, then `latest*.json` last, to `www/penumbra/launcher/` via SSH.
6. Smoke test: older installed launcher updates to the new one.

## Code signing (Authenticode)
Without a certificate Windows SmartScreen warns on the first install ("unknown publisher"). Decision: ship
unsigned for now; **a certificate is required before a wider public release** (options: Azure Trusted Signing,
OV/EV cert). `release.ps1` gets a `signCommand` hook so adding it later is one config change.
