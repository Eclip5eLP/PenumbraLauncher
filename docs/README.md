# Docs index

| # | Doc | What's in it |
|---|-----|--------------|
| 01 | [Vision & scope](01-vision-and-scope.md) | Goals, non-goals, item types, feature list |
| 02 | [Architecture](02-architecture.md) | Components, repo layout, data flow, tech choices |
| 03 | [Content format](03-content-format.md) | Chunks, manifests, delta updates, signing, publisher CLI |
| 04 | [Server](04-server.md) | Server facts, folder layout, HTTP API, caching, deploy |
| 05 | [Launcher app](05-launcher-app.md) | Rust modules, IPC, local state, download engine, launching, performance budget |
| 06 | [UI design](06-ui-design.md) | Layout, screens, visual language, interactions |
| 07 | [Settings](07-settings.md) | Every setting, defaults, where stored, what syncs |
| 08 | [Accounts & cloud](08-accounts-and-cloud.md) | Sign-in, tokens, gated content, settings sync, cloud saves |
| 09 | [Self-update & release](09-self-update-and-release.md) | Installer, updater, keys, release process |
| 10 | [Roadmap](10-roadmap.md) | Milestones with checkboxes |
| 11 | [Sources & GitHub](11-sources-github.md) | `penumbra` / `github` / `url` download sources, runtime requirements |
| 12 | [Initial catalog](12-initial-catalog.md) | Gracidea, Asperia, PG Bot: what exists and how to publish it |
| 13 | [Repository & branches](13-repository.md) | Public `main` vs private `dev`, sync script |
| — | [Decisions](decisions.md) | ADR log: why things are the way they are |

Private (dev branch only, `docs/private/`): server internals, open questions, worklog.

Quick facts to remember:
- Launcher OAuth app id: `000007` ("Penumbra Games Launcher", device sign-in enabled, already exists).
- Accounts base URL: `https://accounts.eclip5e.com` (endpoints under `/api/`).
- Launcher server root: `https://eclip5e.com/penumbra/` = server path `~/docker/penumbra/www/penumbra/`.
- Brand colors (from the account site): bg `#131313`, panel `#1d1e22`, line `#2e3036`, text `#e8e8ea`,
  muted `#9a9ca3`, accent `#5385c1`, ok `#2f7d4f`, err `#a33a3a`.
