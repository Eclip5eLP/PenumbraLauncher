# 13 — Repository & branches

| Branch | Remote | Visibility | Contents |
|--------|--------|------------|----------|
| `dev` | `origin` (private GitHub repo) | private | everything: code, docs, private notes, dev tooling |
| `main` | `public` (public GitHub repo) | public | snapshot of `dev` minus `.publicignore` paths |

## Rules
- All work is committed on `dev`. Never commit on `main` directly and never merge `main` ↔ `dev`.
- Private material goes only into paths listed in `.publicignore` (`CLAUDE.md`, `docs/private/`, `tools/private/`,
  `.claude/`). Server addresses, SSH details, deploy scripts, worklog and open questions live there.
- Secrets never go into git at all, not even `dev` (keys live in `C:\Eclip5eDev\keys\`).
- Publish to `main` with `tools/sync-public.ps1 [-Message "..."] [-Push]`. It builds a snapshot commit from `dev`'s
  tree in a temporary index (working tree untouched), removes the private paths, double-checks nothing private is
  left, and adds the commit on top of `main`. `main` has no shared history with `dev` (ADR-012).
- Sync at milestone ends or whenever the public repo should be updated; there's no need to sync every commit.
- Release tags (`launcher-vX.Y.Z`) are set on `main` (public) after syncing, and mirrored on `dev`.
