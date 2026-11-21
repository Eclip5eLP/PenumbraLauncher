# 06 — UI design

## Window
- Frameless, custom title bar (drag region, min/max/close, launcher-update pill, account chip).
- Default 1280×760, min 1024×640, remembers size/position/maximized. Optional Mica backdrop (Windows 11).
- Dark theme only at first (brand colors below); accent may be tinted per game (`media.accent`).

## Layout
```
┌──┬──────────────────────────────────────────────────────── ─ □ ✕ ┐
│▣ │  [game logo]                                [update ⟳] [◉ User] │
│──│                                                                 │
│◉ │            FULL-BLEED BACKGROUND (image / muted video)          │
│◉ │                                                                 │
│◉ │  ┌──── news carousel ────┐                                      │
│──│  │ banner 1/3   ● ○ ○    │                                      │
│◉ │  ├───────────────────────┤                ┌──────────────────┐ │
│◉ │  │ Events | Notices | Info│                │ v1.2.2 · stable ▾│ │
│──│  │ • post title    11/20 │                │  ▶  PLAY     [⚙] │ │
│⇩ │  │ • post title    11/18 │                └──────────────────┘ │
│⚙ │  └───────────────────────┘     ▬▬▬▬▬▬▬▬▬▬▬ 43% · 12.4 MB/s · 3m │
└──┴─────────────────────────────────────────────────────────────────┘
```
- **Left rail** (64 px): logo/home on top, item icons grouped (Games / Software / Web) with thin separators,
  status dot on icons (update available, downloading ring, running). Hover: name tooltip. Drag to reorder,
  right-click: pin, hide, open folder. Bottom: Downloads (badge with count), Settings.
- **Item page** (HoYoPlay): background, logo top-left, news widget bottom-left, action cluster bottom-right.
- **Action button** states: Install · Queued · Downloading x% (click = pause) · Paused (Resume) · Update ·
  Verifying · Play · Running · Open (links) · Sign in to download (locked) · Locked (no access).
  Gear menu: Verify & repair, Open folder, Move, Change channel, Launch options, Uninstall.
- **Install dialog**: path picker (default library), size required / free space bar, desktop shortcut checkbox,
  channel.
- **Downloads page** (Riot/Steam-ish): current job card with speed sparkline, queue (drag to reorder), completed
  list.
- **Library page**: grid of cover tiles for all items with filters (installed / all / web), search (Ctrl+K quick
  switcher across items and settings).
- **Settings page**: left section nav (General, Downloads, Appearance, Account, Advanced, About), right content.
- **Account**: chip top-right → popover (username, sign out, manage account → opens accounts.eclip5e.com).
  Device sign-in dialog: "Your browser opened — confirm code ABCD-EFGH" with copy button + QR + fallback link.
- **Toasts** bottom-right for job done/errors; errors have "Details" (copies log excerpt).

## Visual language
Tokens (CSS custom properties, `src/styles/tokens.css`):
```
--bg #131313   --panel #1d1e22   --panel-2 #25272c   --line #2e3036
--text #e8e8ea --muted #9a9ca3   --accent #5385c1 (per-item override) --ok #2f7d4f --err #a33a3a --warn #c28a2e
--radius 10px  --radius-lg 16px  --shadow  0 8px 30px rgb(0 0 0 / .45)
--glass: rgb(20 20 24 / .55) + backdrop-filter: blur(18px)   (news + action panels over the background)
font: Inter variable (self-hosted)
```
- Motion: 120–200 ms ease-out; background crossfade 300 ms; button press scale .98; respect reduced motion.
- Background readability: bottom + left gradient scrims, never text directly on raw art.
- Icons: Lucide (tree-shaken SVGs).

## Assets per item (made by publisher, see catalog/)
icon 128×128 webp · logo ≤800×300 webp (transparent) · bg 1920×1080 webp (+ optional bg.webm ≤ 6 MB, 1080p,
loop, muted) · banners 1280×600 webp · cover 600×900 webp (library grid).

## Desktop integration + links (M8)
- Tray: left click opens the window; menu Open / Library / Downloads / Quit. Close button hides to the tray when
  Settings > General > Close button = "Minimize to tray" (default).
- Start with Windows writes `HKCU\...\Run\Penumbra Launcher` = `<exe> --autostart`; with "Start minimized" the
  window stays hidden (tray only).
- Ctrl+K quick switcher (items, library, downloads, every settings section). Right-click on rail/library items:
  pin, hide, open folder, verify. Drag rail icons to reorder (`railOrder`, synced).
- `penumbra://open/<item>`, `penumbra://install/<item>` (opens the install dialog), `penumbra://play/<item>`
  (navigates only). Registered by the installer and by release builds on start; dev builds don't register.

"Open in launcher" button for any page on eclip5e.com (falls back to the download page if the launcher isn't installed
— browsers simply do nothing for unknown schemes, so offer both):
```html
<a href="penumbra://open/gracidea">Open in Penumbra Launcher</a>
<small>No launcher yet? <a href="https://eclip5e.com/penumbra/">Download it</a></small>
```
