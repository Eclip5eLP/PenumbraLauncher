# 12 — Initial catalog

## Pokemon Gracidea — `gracidea` (game, source `penumbra`, access public)
Current distribution (server `www/development/Gracidea/`), driven by its own old launcher
(`launcher/GracideaLauncher.zip`, 7 MB exe) reading `data/version.dat`:
| Part | Version | Files |
|------|---------|-------|
| exe | 0.3.14 | `PokemonGracidea.zip` (24 MB → 4 files: `Pokemon - Gracidea Version.exe`, `data.win`, `D3DX9_43.dll`, `options.ini`) |
| sprites | 1.5.1 | `data.zip` (130 MB → 5683 files: `audio/`, sprites, ...) |
| data | 1.2.51 | `items.ini, moves.ini, trainers.ini, encounters.ini, Pokedex.ini, abilities.ini, animations.ini, pokemonforms.ini, lang_en/de/jp.ini`, `maps.zip` |

Published as one install tree (`catalog/gracidea/build.toml`), layout taken from the GameMaker project
(`checkIntegrity`, `obj_download`, `obj_boot`):

| Folder | Content |
|--------|---------|
| root | exe zip: `Pokemon - Gracidea Version.exe`, `data.win`, `D3DX9_43.dll`, `options.ini` (`keep`) |
| `assets/` | data.zip: `audio/`, `items/`, `pokemon/` |
| `maps/` | maps.zip (`rm_*` files) |
| `data/` | `items.ini moves.ini trainers.ini encounters.ini Pokedex.ini abilities.ini animations.ini pokemonforms.ini lang_en/de/jp.ini version.dat` |

Not shipped: `mysterygift.ini` (the game fetches it over HTTP), `RegionalDex.txt` (unused).
Version string `<exe>+<sprites>+<data>[-rN]`; current stable `0.3.14+1.5.1+1.2.51-r2` (r1 had the wrong layout and
crashed). Saves: GameMaker save area in `%LOCALAPPDATA%` (untouched by updates/uninstall).

**The game updates itself too:** on start `obj_boot` downloads `version.dat` from `global.server`
(`https://eclip5e.bplaced.net/development/Gracidea/`, hard-coded in `pb_load_config`), overwrites
`data/version.dat` and re-downloads parts whose version differs. Shipping the matching `data/version.dat` keeps it
quiet, but if the server's copy changes (or the old bplaced host serves a different one) the game will patch the
launcher-managed files itself. Long term: remove the in-game updater (or point it at a launcher-aware endpoint);
until then publish launcher versions together with the server's `version.dat`.

Media: backgrounds from `Gracidea/launcher/data/*.jpg` (user confirmed OK to use).

## Asperia Chronicles — `asperia` (game, source `penumbra`, unreleased)
- Server folder `www/development/asperia/` currently only has the analytics dashboard + `old_data/` config files.
  Old layout used `Asperia.zip` (exe) + `.conf` data + langs en/de/jp, same three-part idea as Gracidea.
- Userbase app `000013` exists with device sign-in → the game can sign in on its own; the launcher can pass its
  token (`passToken`) later.
- Catalog: `status: "coming-soon"` → visible with art + "Coming soon" button; `access: "role:admin"` channel
  `dev` so it can be tested through the launcher before release.

## Pirate Galaxy AFK Bot — `pgbot` (software, source `github`, access public)
See [11-sources-github.md](11-sources-github.md#initial-github-item-pirate-galaxy-afk-bot). The user adds GitHub releases
themselves; `stable` = latest release.
Group: Software. Show the README's "use at your own risk" note in the item description.

## Links
None for now (user). The `link` type is still built and tested with a dev-only catalog entry.
