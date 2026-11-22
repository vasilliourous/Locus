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

## The 2026-11-23 rename (Locus → Chrysalis)

A **naming** migration, not a layout one. Only the paths moved:

| Old name (dead) | Current file |
|---|---|
| `client/src-tauri/src/locus/` | `client/src-tauri/src/chrysalis/` |
| `client/src-tauri/src/cmd/locus.rs` | `client/src-tauri/src/cmd/chrysalis.rs` |
| `client/src/services/locus.ts` | `client/src/services/chrysalis.ts` |
| `client/src/services/locus.generated.ts` | `client/src/services/chrysalis.generated.ts` |
| `server/scripts/locus-key-extract.sh` | `server/scripts/chrysalis-key-extract.sh` |
| `server/templates/locus-fetch.service` | `server/templates/chrysalis-fetch.service` |
| `.locus-keys/` | `.chrysalis-keys/` |

Also renamed without moving a file: the Cargo package (`locus` → `chrysalis`), the
`APP_ID`-adjacent backup directory names the *app creates* (not `APP_ID` itself —
see below), and every `crate::locus::` reference.

**What this rename deliberately did not move** — the bundle identifier, the
signing keys, the artefact file names, the bootstrap header and one landing-page
hostname — is the subject of
[`../reference/NAME-MIGRATION.md`](../reference/NAME-MIGRATION.md). Those are
identity and infrastructure, not paths, and they are the names that would break a
deployed client if a later reader "completed" the rename.

## The three passes

| When | What it did |
|---|---|
| 2026-09 | Split live material into `operate/` and `reference/`; replaced prose `⚠️ STATUS:` banners with front-matter; moved volatile facts to `state.toml` + `CLAIMS.md`. |
| 2026-09-23 | Separated `client/`, `server/` and `legacy/`. |
| 2026-10-05 | Made [`../README.md`](../README.md) the single index (`STATE.md` had carried a competing map); moved retired material into `archive/`, where the folder carries the status. |

Every one of these is the same rule applied to a different tree: **a document's
status is structural, not a sentence in it.**
