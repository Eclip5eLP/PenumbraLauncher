# 08 — Accounts & cloud

## Existing system (Penumbra Account, `www/userbase`, not ours to change without asking)
- Base `https://accounts.eclip5e.com`. OAuth-like: browser flow (code + PKCE) and **device flow**.
- Launcher app already registered: id **`000007`** "Penumbra Games Launcher", trusted, `allow_device = 1`,
  no redirect URIs.
- Endpoints:
  - `POST /api/device.php` `client_id=000007` → `device_code, user_code, verification_uri, verification_uri_complete, expires_in (600), interval (5)`
  - `POST /api/token.php` `grant_type=urn:ietf:params:oauth:grant-type:device_code&client_id&device_code`
    → pending / slow_down / access_denied / expired_token / tokens
  - `POST /api/token.php` `grant_type=refresh_token&client_id&refresh_token` → new access + refresh token
  - `GET /api/userinfo.php` (who am I), `DELETE /api/userinfo.php` (sign out / revoke)
  - `GET|PUT|DELETE /api/data.php?key=` per-user per-app key/value store (raw bytes)
- Tokens: access 24 h, refresh 180 days sliding, **single-use rotating refresh** (reuse of an old refresh token
  revokes the family → sign-out). Tokens die on password change / disconnect / ban.
- Storage limits (defaults): 64 KB per value, 256 KB per user per app, 200 keys. Admin can raise per app up to
  8 MB per value / 64 MB per user.

## Launcher sign-in flow
1. Click "Sign in" → `auth_start_device_login` → POST device.php.
2. Open `verification_uri_complete` in the default browser automatically; dialog shows `user_code`, copy button,
   QR code (for phones), "Open again" link.
3. Poll token.php every `interval` s (honour `slow_down`), until success/expiry/cancel.
4. Store refresh token in Credential Manager; access token in memory only. Fetch userinfo → `auth:changed`.
5. On start: if a refresh token exists, refresh in background (UI shows cached username meanwhile).
6. Refresh is serialized by a mutex and the new refresh token is persisted **before** using the new access token,
   so a crash can't lose the rotation. On `invalid_grant` → signed out state, keep everything else working.

The user decided to keep the userbase as is (no loopback-redirect change), so device flow is final.

## As implemented (M5)
- `auth.rs`: device flow (`device.php` -> poll `token.php`, honours `slow_down`), access token in memory,
  refresh token only in Windows Credential Manager (`cred.rs`, target `Penumbra Launcher/refresh_token`).
  Refresh runs under one async mutex and stores the rotated refresh token before releasing it (a second refresh
  with an old token would revoke the whole family). `invalid_grant` -> local sign-out.
- `account.json` (APPDATA) caches the user (`id`, `username`, `role`) for instant display; no secrets.
- Sign-out: `DELETE api/userinfo.php` (revokes the token family), then local cleanup.
- Catalog requests carry the bearer token; sign-in/out triggers a catalog refresh (restricted items appear/vanish).
- `penumbra-pack build` writes manifests of non-public items (or `--gated`) to `data/content/` (ADR-008).

## Gated content
- Catalog items carry `access`. Signed-out users see `account` items as **locked** ("Sign in to download");
  `restricted` items (role/user lists) are not listed at all unless entitled.
- `manifest.php` enforces the rule. Chunks stay static and public-but-unguessable (256-bit hashes only revealed by
  the gated manifest). Good enough for "account required"; not DRM. (ADR-008)
- Already-installed gated items keep launching offline.

## Settings sync
- Key `settings` in the launcher's data store (app 000007): JSON of the sync-marked settings + per-item prefs +
  `updatedAt`. Pull on sign-in/start, push debounced 5 s after changes. Conflict: newer `updatedAt` wins
  (whole-document; fine for settings).

## Cloud saves
- Catalog declares save locations per item: `cloudSaves.paths` (globs with `%APPDATA%`, `%LOCALAPPDATA%`,
  `%USERPROFILE%`, `%DOCUMENTS%`, `{install}`) and `exclude`.
- Flow: before launch → compare cloud snapshot meta vs local → download newer cloud save (with conflict dialog
  showing both timestamps/sizes if both changed); after the game process exits → zip (zstd) the save set → upload.
- Storage, phase 1: userbase data API under key `save.<item>` + `save.<item>.meta`. User approved raising the
  launcher app's (000007) limits in the userbase admin page to the caps: 8 MB per value / 64 MB per user
  (admin setting, no code change). Some games need more → phase 2.
- Storage, phase 2 (only if saves get bigger): `penumbra/api/v1/saves.php` with files under `penumbra/data/saves/
  <user>/<item>/` and keeping the last 5 snapshots, auth via the same bearer token.
- Local backup of the previous save set before every cloud download (`%LOCALAPPDATA%\...\save-backups`, last 5).

## Playtime
Tracked locally per item (process watcher). When signed in, `playtime` key synced (sum per item, max-merge per
device id).
