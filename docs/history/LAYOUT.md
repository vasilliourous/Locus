# Layout history — where files moved, and what still points at them

```
audience:    all
status:      reference
authoritative-for: the record of documentation/source layout changes, and the old names that are dead
verified-against: docs/ (the tree it records)
```

Old paths go in this file so nobody re-derives them by grepping. Documents dated
before a move may name old paths — that is history, not rot. Full before/after
tables for every pass are in git; what follows is what a reader still needs.

## Dead names — search for the new one instead

Moves used `git mv`, so history follows and `git log --follow` works.

| Old name (dead) | Current file |
|---|---|
| `docs/DEPLOY.md`, `OPS.md`, `POCKETBASE-SETUP.md`, `SECRETS-MANAGEMENT.md`, `UPDATE-SYSTEM.md`, `RELEASING.md`, `CI-CD.md` | `docs/operate/` |
| `docs/API.md`, `FIXES.md`, `STILL-OPEN.md` | `docs/reference/` |
| `docs/ARCHITECTURE.md` (retired client) | `docs/archive/ARCHITECTURE-wails.md` |
| `docs/ENGINE-SWAP-ANALYSIS.md`, `docs/GAMING-UDP.md` | `docs/archive/` |
| `docs/operate/DEPLOY-3.2.7.md` | `docs/archive/DEPLOY-3.2.7.md` |
| `docs/business/redesign/03-device-binding.md` | `docs/business/redesign/archive/` |
| `GOTCHAS-FROM-THIS-WORK.md` | `docs/history/SESSION-GOTCHAS.md` (since removed — its rules live in `../CONTEXT.md` §6) |
| `extra-details/`, `v5/docs/` | `docs/` |
| `v5/server/`, `v5/console/`, `v5/client/`, `v4/` | `server/`, `server/console/`, `legacy/wails-client/`, `legacy/v4/` |

**Deleted, not moved.** The retired Wails client's version/release machinery is
gone: root `VERSION`, `bump.sh`, the bump/syso scripts, `release-cut.sh`, the
`.syso` resources, `.github/workflows/build.yml`. Nothing should re-add them —
the client has its own version sites and its own CI
([`../operate/RELEASING.md`](../operate/RELEASING.md)).

## The three passes

| When | What it did |
|---|---|
| 2026-09 | Split live material into `operate/` and `reference/`; replaced prose `⚠️ STATUS:` banners with front-matter; moved volatile facts to `state.toml` + `CLAIMS.md`. |
| 2026-09-23 | Separated `client/`, `server/` and `legacy/`. |
| 2026-10-05 | Made [`../README.md`](../README.md) the single index (`STATE.md` had carried a competing map); moved retired material into `archive/`, where the folder carries the status. |

Every one of these is the same rule applied to a different tree: **a document's
status is structural, not a sentence in it.**
