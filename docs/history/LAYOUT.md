# Layout history — how `docs/` got its shape

```
audience:    all
status:      reference
authoritative-for: the record of documentation/source layout changes
verified-against: docs/STATE.md
```

This file records every restructure of the repository and the `docs/` tree, so a
future reorganisation does not have to re-derive the archaeology. Documents
dated before a move may name old paths in historical context — that is correct,
not stale.

---

## 2026-09 overhaul (this one)

**What changed.** The documentation set was reorganised to be readable by humans
rather than only by agents. The problem it fixed: documents were organised by
*provenance* (which era/agent wrote them) and carried their status as a prose
`⚠️ STATUS:` banner, so a reader paid a paragraph of banner tax to learn one bit,
and no fact had a single home (the tier table appeared in ~21 files, the client
version in ~12).

| Before | After |
|---|---|
| `docs/DEPLOY.md` | `docs/operate/DEPLOY.md` |
| `docs/OPS.md` | `docs/operate/OPS.md` |
| `docs/POCKETBASE-SETUP.md` | `docs/operate/POCKETBASE-SETUP.md` |
| `docs/SECRETS-MANAGEMENT.md` | `docs/operate/SECRETS-MANAGEMENT.md` |
| `docs/UPDATE-SYSTEM.md` | `docs/operate/UPDATE-SYSTEM.md` |
| `docs/RELEASING.md` | `docs/operate/RELEASING.md` |
| `docs/CI-CD.md` | `docs/operate/CI-CD.md` |
| `docs/API.md` | `docs/reference/API.md` |
| `docs/FIXES.md` | `docs/reference/FIXES.md` |
| `docs/STILL-OPEN.md` | `docs/reference/STILL-OPEN.md` |
| `docs/ARCHITECTURE.md` (wholly retired client) | `docs/archive/ARCHITECTURE-wails.md` |
| `docs/ENGINE-SWAP-ANALYSIS.md` (retired client) | `docs/archive/ENGINE-SWAP-ANALYSIS.md` |
| `docs/CONTEXT.md` §3 hardening list | `docs/archive/CONTEXT-client-hardening.md` |
| — (new) | `docs/STATE.md` — the single home for volatile facts |
| — (new) | `docs/STATE.md` — the single home for volatile facts |

**Other changes.** Prose `⚠️ STATUS:` banners were replaced by a 4-line
front-matter block (`audience` / `status` / `authoritative-for` /
`verified-against`) on every live document. `GAMING-UDP.md` stayed live (its
server-side UoT content is current) with its client-era framing trimmed to a note.

*(Version-drift and dead-link enforcement lives in
`server/scripts/check-consistency.sh` §7 and §8, which predates this overhaul.
An earlier draft of this change added a second, redundant `scripts/check-docs.sh`;
it was removed in favour of the existing check.)*

---

## 2026-09-23 restructure

`v5/server/` → `server/`, `v5/console/` → `server/console/`, `v5/client/` →
`legacy/wails-client/`, `v4/` → `legacy/v4/`, `v5/docs/` + `extra-details/` →
`docs/`. All moves used `git mv`, so history is intact.

Filenames changed in the merge, so a few old names are **dead** — search for the
new one instead:

| Old name (dead) | Current file |
|---|---|
| `GOTCHAS-FROM-THIS-WORK.md` | `docs/history/SESSION-GOTCHAS.md` |
| `extra-details/` | `docs/` (contents merged) |
| `v5/docs/` | `docs/` |

Later, the archived-client **version/release machinery was deleted** (root
`VERSION`, `bump.sh`, the bump/syso scripts, `release-cut.sh`, the `.syso`
resources, `.github/workflows/build.yml`). References to those are historical,
not instructions — see `docs/STATE.md` for how versioning works now.
