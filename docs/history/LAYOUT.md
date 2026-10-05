# Layout history — how `docs/` got its shape

```
audience:    all
status:      reference
authoritative-for: the record of documentation/source layout changes
verified-against: docs/ (the tree it records)
```

This file records every restructure of the repository and the `docs/` tree, so a
future reorganisation does not have to re-derive the archaeology. Documents
dated before a move may name old paths in historical context — that is correct,
not stale.

---

## 2026-09 overhaul

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
server-side UoT content is current) with its client-era framing trimmed to a note
— **that last part was reversed on 2026-10-05**, when the document was archived;
see the section below, which is the current state of the tree.

---

## 2026-10-05 — the index and the archive split

**What changed, and why.** Not a directory reorganisation — a *labelling and
reachability* one. The audit found the doc set green under every guard but
fragmented in three ways no guard could see:

1. **Two indexes, drifted apart.** `docs/README.md` covered `operate/` and
   `reference/` and stopped; `docs/STATE.md` carried a second, competing map of
   topics. All 19 business files, the whole `redesign/` corpus and
   `client/docs/CONTRIBUTING_i18n.md` were reachable from neither — findable only
   by browsing the filesystem. Two files answering "which document is this?" is
   worse than one incomplete answer, because they can disagree.
2. **Titles and statuses that lied.** `GAMING-UDP.md` was `status: live`, titled
   "Implementation Plan", and its own body opened with two supersession banners.
   Nothing connected the two, so a document could announce its own retirement and
   stay labelled current indefinitely.
3. **Retired material living among the live.** A prepared deployment packet for a
   deployment that had already happened, and a design half that was removed rather
   than shipped, sat in live folders with no folder-level separation.

**Moves** (all `git mv`, so history follows):

| Old path | New path | Why |
|---|---|---|
| `docs/GAMING-UDP.md` | `docs/archive/GAMING-UDP.md` | The UoT *plan*, superseded in part; the live facts are owned by `business/04-tiers.md` §4.1 |
| `docs/operate/DEPLOY-3.2.7.md` | `docs/archive/DEPLOY-3.2.7.md` | A deployment packet for a deployment completed; it had zero inbound links |
| `docs/business/redesign/03-device-binding.md` | `docs/business/redesign/archive/03-device-binding.md` | Device binding was **removed, not shipped** |
| `docs/business/redesign/implementation/03-device-binding.md` | `docs/business/redesign/archive/implementation-03-device-binding.md` | Same, and renamed so its sibling relationship is visible from the filename |

**Rewrites.** `docs/README.md` is now the **single** index — every live document,
one row each, `status` shown, including `client/docs/`. `docs/STATE.md` lost its
competing map and now answers only "where does this fact go", with the ownership
table kept as the de-duplication index. Four misleading titles were renamed
(`CONTEXT.md` "…(V5 Reference)", `LOGIC-INVENTORY.md` "the exact modules to
write", `STATE.md`, and the two archived files).

**New guard — §28.** Index completeness (every live document must be linked from
`docs/README.md`) and head-level self-contradiction (a `status: live` document
whose title or first 25 lines say it is superseded). Both halves were observed
**failing** against the defect before being kept. §25's archive exemption was
extended to cover `docs/business/redesign/archive/`, which follows the same
folder-carries-status rule.

**The §28 lesson, recorded because it is the reusable part.** The first draft
scanned the *whole body* for retirement words and fired on seven healthy
documents — a dated log recording that something else was removed, a backlog
whose job is marking superseded items, history sections about retired
mechanisms. Every hit was legitimate. §7's rule applied: scope the check to where
the defect actually lived (the head), not to everywhere the word could appear. A
guard that cries wolf is one the reader learns to ignore.

### The follow-up pass, same day — widening §8 broke it silently

Closing the gaps the first pass named exposed two more, and one of them was
self-inflicted:

- **§8 scanned only `.md` files**, which is why two code comments pointed at
  `docs/ARCHITECTURE.md` for a year after it moved. The scan now covers the
  source extensions across `client/src`, `client/src-tauri/src`, `server`,
  `.github` and `scripts`, using the *same* regex and basename allow-list.
- **Widening §8 broke it.** `md_files` is a generator; joining two of them with
  `+` raised `TypeError`, the heredoc died, and §8 reported
  **"OK — every link and anchor resolves"** for every file while checking none.
  A guard that crashes reads exactly like a guard that passes. Caught only
  because an expected failure proof produced no output.
- **`client/CONTRIBUTING.md` was an orphan** — referenced by nothing at all. §28
  now holds top-level `client/*.md` to the index (entry points and tool manuals
  excluded, each with a reason), and `client/AGENTS.md` gained a "Where to read"
  table naming the four previously unreachable documents.

Recorded here rather than only in `FIXES.md` because the *kind* of mistake —
editing a guard's scope and not re-proving it can fail — is not specific to
documentation and will recur wherever a check is widened.

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
