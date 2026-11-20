# 12 — Initial catalog

## Pokemon Gracidea — `gracidea` (game, source `penumbra`, access public)
Current distribution (server `www/development/Gracidea/`), driven by its own old launcher
(`launcher/GracideaLauncher.zip`, 7 MB exe) reading `data/version.dat`:
| Part | Version | Files |
|------|---------|-------|
| exe | 0.3.14 | `PokemonGracidea.zip` (24 MB → 4 files: `Pokemon - Gracidea Version.exe`, `data.win`, `D3DX9_43.dll`, `options.ini`) |
| sprites | 1.5.1 | `data.zip` (130 MB → 5683 files: `audio/`, sprites, ...) |
| data | 1.2.51 | `items.ini, moves.ini, trainers.ini, encounters.ini, Pokedex.ini, abilities.ini, animations.ini, pokemonforms.ini, lang_en/de/jp.ini`, `maps.zip` |

Plan: the publisher assembles all three parts into **one install tree** and publishes it as one item version
(chunk dedup means a data-only update downloads only the changed data files). Launcher version string:
`<exe>+<sprites>+<data>` e.g. `0.3.14+1.5.1+1.2.51`, or a single version the user picks.
`keep`: `options.ini`, save folder (TBD). Launch `Pokemon - Gracidea Version.exe`.
Open: exact target folders for data.zip / maps.zip / ini files (what GracideaLauncher does) — see open questions.
Media: background candidates exist in `Gracidea/launcher/data/*.jpg` (fan art — check usage rights before using).

## Asperia Chronicles — `asperia` (game, source `penumbra`, unreleased)
- Server folder `www/development/asperia/` currently only has the analytics dashboard + `old_data/` config files.
  Old layout used `Asperia.zip` (exe) + `.conf` data + langs en/de/jp, same three-part idea as Gracidea.
- Userbase app `000013` exists with device sign-in → the game can sign in on its own; the launcher can pass its
  token (`passToken`) later.
- Catalog: `status: "coming-soon"` → visible with art + "Coming soon" button; `access: "role:admin"` channel
  `dev` so it can be tested through the launcher before release.

## Pirate Galaxy AFK Bot — `pgbot` (software, source `github`, access public)
See [11-sources-github.md](11-sources-github.md#initial-github-item-pirate-galaxy-afk-bot).
Group: Software. Show the README's "use at your own risk" note in the item description.

## Links (for testing the `link` type)
To pick with the user later; candidates: PG Database (`https://pg.eclip5e.com/`), eclip5e.com.
