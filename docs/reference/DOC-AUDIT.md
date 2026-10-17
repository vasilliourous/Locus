# Documentation audit — every document, evaluated

```
audience:    all
status:        live
authoritative-for: the state of the documentation set as of 2026-10-05, and what is wrong with it
verified-against: this checkout at 2026-10-05; check-consistency.sh run after each change
```

> **What this is.** A dated, whole-set evaluation: nobody had asked, file by file,
> whether each document is true, current, and worth its maintenance cost. Read it
> as an **observation**, not a rule — it decays from the moment it is written. Per `CLAIMS.md` §2, the live rules it references
> (`README.md` rules, `STATE.md` ownership, §-numbered guards) are the authorities;
> this page only describes where they currently bite and where they do not.

---

## 0. The headline

**The guards pass, and the set is still wrong in places they cannot see.**

`check-consistency.sh` exits green on all 29 sections. That is a real and hard-won
result — broken internal links across all 103 files: **zero**. But a guard only
sees what someone encoded into it, and the recurring defect in this repository is a
**correct sentence that stays in place while the code moves underneath it**. This
audit found that class in four places (§3), and one case where a guard was
*enforcing* the defect (§3.1).

**Scale:** ~34,000 lines across the doc set. The two largest files are the two
records, not the contracts — `docs/reference/FIXES.md` (a dated log that must never
be read as a statement about today) and `docs/reference/STILL-OPEN.md` (what is not
verified). That split is deliberate; see §2.2.

---

## 1. How the set is structured

`docs/` is not one set. It is five with different maintenance contracts, and
evaluating them by one standard is the mistake that keeps producing the same ticket.

| Set | Files | Lines | Contract | Evaluated for |
|---|---:|---:|---|---|
| `docs/operate/` | 10 | ~3,700 | Procedure an operator follows | Is it **true now**? |
| `docs/reference/` | 6 | ~10,900 | Contracts; one append-only log | Live parts true now; log entries **dated, never current** |
| `docs/business/` | 30 | ~4,300 | Commercial/policy plan | Is it **decided**, not whether it is deployed |
| `docs/archive/` + `docs/history/` | 23 | ~9,000 | Retired | Read for *why*. **Never** for *what is* |
| `client/docs/` | 9 | ~2,500 | Client internals | Is it **true now**? |

Three structural facts worth stating because they are commonly misread:

- **`CLAUDE.md` and `GEMINI.md` are 1–2 line pointers to `AGENTS.md`**, not
  duplicates. There is nothing to reconcile. `client/` mirrors this.
- **`docs/history/` is not "old documentation"** — it is curated research
  (`attacker-perspective.md` and `defender-perspective.md` alone are ~3,900 lines
  of security analysis). It is the only place the *reasoning* behind the transport
  and identity decisions survives.
- **`legacy/` (11 files) is out of scope by rule** — read it, never build it.

---

## 2. Per-set evaluation

### 2.1 `docs/operate/` — healthy, with one live gap

| Document | Verdict | Note |
|---|---|---|
| `DEPLOY.md` | **Good** | Front-matter, verified-against `server/setup.sh` + `server/modules/`. Correctly points to the blank-box proof for evidence. |
| `OPS.md` | **Good** | 1,018 lines, the 2am document. Backup/restore section is the one that mattered on 2026-10-18; see §4. |
| `CI-CD.md` | **Good** | Describes `.github/workflows/client.yml` and why it must sit at the repo root. Accurate as read. |
| `RELEASING.md` | **Good** | Updated 2026-10-16 with the signing rotation. Documents that hub publish is a *separate manual step* — the single most important line in the set. |
| `UPDATE-SYSTEM.md` | **Good** | End-to-end updater path with an honest verified-vs-not split. |
| `POCKETBASE-SETUP.md` | **Good** | Console, `/admin/`, collection specifics. |
| `SECRETS-MANAGEMENT.md` | **Good** | Rewritten 2026-10-16 (passphrase-encrypted, not keypair). Current. |
| `CLAIMS.md` | **Good, and now carries a live warning** | §5 register annotated 2026-10-18 — see §4. |
| `RECOVER-WINDOWS-UPDATE.md` / `RECOVER-MACOS-UPDATE.md` | **Good** | Ordered recovery procedures. Both describe *real, paid-for* incidents; the depth is earned, not padding. |

**The gap:** none of these describe the **redeploy with B2 backups** that is
currently in flight. That is expected — it has not happened yet — but it means
`OPS.md`'s backup section describes the *intended* state and must be re-checked
against what the redeploy actually installs before anyone relies on it.

### 2.2 `docs/reference/` — the strongest set, with a concentration risk

| Document | Verdict | Note |
|---|---|---|
| `API.md` | **Verified accurate** | Spot-checked `/api/activate` rate limiting against **both** gates: `activation.pb.js` (`recentCount >= 5`, over a 10-minute window) and `templates/Caddyfile` (`events 5`, `window 10m`). The "double-gated" claim is literal and correct. |
| `DEBUGGING-METHOD.md` | **Excellent** | The four-shipped-through-green-pipelines postmortem. This is the best document in the repository. |
| `DEVICE-IDENTITY.md` | **Good** | Covers entitlement survival *and* why device identity was removed. |
| `EGRESS-READINESS.md` | **Good** | The "stuck on connecting" chain, traced to real files. |
| `THEMES.md` | **Good** | Verified-against `state.toml`, so the theme list is data-backed. |
| `STILL-OPEN.md` | **Good, and load-bearing** | 1,397 lines of what is *not* verified. This is the document that makes the rest honest. |
| `FIXES.md` | **Leave alone** | 8,067 lines, append-only, dated. Its value is searchability. **Do not reduce it, do not "update" it.** Its only failure mode is *contradicting* a live document, and that is checked separately (§3.1). |

**Concentration risk:** `FIXES.md` + `STILL-OPEN.md` are 9,464 lines — **70% of
`docs/reference/`**. That is defensible for an incident log, but it means the
"reference" folder is mostly *history*, not *contract*. Anyone told "read
`docs/reference/`" will mostly read the past.

### 2.3 `docs/business/` — policy, and correctly labels itself as such

30 files. The `04-tiers.md` → `05-pricing.md` → `06-unit-economics.md` chain is the
real content; the rest is market, growth, risk, continuity.

`04-tiers.md` is the **canonical home for tiers, ports and caps** — a job it now
holds alone, because the tier table that used to sit in the root `README.md` was
removed on 2026-10-18 (§3.1). That makes it load-bearing rather than descriptive.

`redesign/` (13 files) is `status: design-record` and says so. Its
`implementation/05-not-yet-true.md` is explicitly a list of what is *not* built,
which is the right way to ship a design.

### 2.4 `docs/archive/` + `docs/history/` — correctly retired

23 files, no front-matter (the folder carries the status, per `README.md` rule).
The only hardcoded addresses in the entire doc set live here, and every one is a
public DNS resolver or a private/loopback range — **not** a leak, and none of them
is a hub address. §10's IP check correctly does not police this folder.
This set is in the shape it should be.

`archive/DEPLOY-3.2.7.md` is the one to watch: it is an *archived deploy note*
that still reads like a procedure. It is correctly filed, but an agent grepping
for "deploy" will hit it.

### 2.5 `client/docs/` — the weakest set

| Document | Verdict | Note |
|---|---|---|
| `ARCHITECTURE.md` | Good | |
| `UPDATE-ARCHITECTURE.md` | **Fixed 2026-10-18** | Said "three sites" *and* pointed the current value at `docs/STATE.md`, which by its own rule is not a fact table. Now points at `state.toml [client.version]`, names the runtime subset without a count, and points the full bump set at `[client.version_sites]`. |
| `FRONTEND.md` | **Fixed 2026-10-18** | Line 169 named `eco`/`stealth`/`strike` as the tier set. `stealth` was retired 2026-10-05; the field now names `free`/`strike` with the customer-facing Free/Full and says `stealth` is never returned. |
| `LOGIC-INVENTORY.md` | **Fixed 2026-10-18** | Line 291 documented a `stealth` profile, including a warning that it embeds a live tier PSK. Row removed, not re-worded. Also corrected a second, separate error: it claimed `eco` was a legacy row kept so field codes resolve, but that row was **removed 2026-10-06**. |
| `RESTRUCTURE.md` | **Fixed 2026-10-18** | `status: design-record`, and its "three sites" is defensible *in its own context* (the runtime-agreement set) — but it pointed at `docs/STATE.md` for the value, which is wrong, and stated the count. Both corrected: it now links `[client.version_sites]` and says explicitly that the three listed are the runtime subset. |
| `SIGNING.md` | **Current** | Updated with the 2026-10-16 key rotation. |
| `UPSTREAM-CHANGES.md` | **Fixed 2026-10-18** | The authoritative upstream diff. Its version row pointed at `docs/STATE.md`; now `state.toml [client.version]`. |
| `IDENTITY-MIGRATION.md` | Good | |
| `CONTRIBUTING_i18n.md` | Unremarkable | Fine. |

---

## 3. Defects found in this pass

### 3.1 A guard was **enforcing** the defect it was written to prevent

**Fixed 2026-10-18.** `check-consistency.sh` §23(g) and §27(g) required `README.md`
to contain the literal string `tc caps (1/100 Mbps)`. The comment above it explains
the guard exists because `README.md` once restated the caps *wrongly* (`5/100/200`).

The guard's cure was the disease: the only way to pass was to keep a second,
hand-maintained copy of the caps in prose — exactly what `STATE.md` rule 4 forbids.
Its failure mode was never "the docs are wrong", it was "somebody removed the
duplicate", so the first person to correctly de-duplicate the sentence got a red
build and put it back.

**Now asserts the negative:** no cap string in `README.md` at all. The cap has one
prose home (`04-tiers.md`) backed by `state.toml [tiers.*].cap`, which §27
recomputes from `04-tc.sh`.

**Verified to fail**, per the debugging method:

- re-inserting `tc caps (1/100 Mbps)` → 2 × `BAD`
- re-inserting the retired `Eco 5Mbit` / `5M tc` → 3 × `BAD`
- restored tree → green

### 3.2 `README.md` was a 499-line duplicate front door

**Collapsed to ~185 lines, 2026-10-18.** It carried its own tier table, its own
activation flow, its own directory tree, its own version-authority section and its
own degraded architecture diagram — none of it in `docs/`, none of it with
front-matter, none of it reachable from the index, and none of it guarded by §28.

It is now a front door that **points**. Unique content was preserved (the
Connect/Disconnect product surface, the code format, the two release traps, the
one-paragraph history); everything with an owner was replaced by a link.

The retired Wails architecture diagram was **removed rather than kept** — it was
labelled "retired" and served only to make the current architecture look like it
lived in `README.md`. `client/docs/ARCHITECTURE.md` owns that.

### 3.3 "Three version sites" in `docs/CONTEXT.md` — three times

**Fixed.** Lines 180, 382 and 634 each stated a count that `state.toml` says is
five, and that `release-version.mjs` writes to **five** places. The 2026-10-06
overhaul fixed this in `FIXES.md` and the archive and **missed `CONTEXT.md`**,
which is the file `docs/README.md` describes as "the project in full".

There is a real ambiguity here worth recording, because it is what let the error
survive: **"three sites" is correct for the runtime-agreement set**
(`package.json`, `Cargo.toml`, `tauri.conf.json`, asserted by
`version_consistency.rs`) and **"five sites" is correct for the bump set** (those
three plus `Cargo.lock` and this file's `[client.version]`). Both statements are
true of different sets. The fix is therefore **not** a find-and-replace: it is to
**stop writing the number in prose at all** and link to
`[client.version_sites]`, which §17 recomputes. That is what was done.

### 3.4 `docs/CONTEXT.md` self-declared unreliability — **DONE 2026-10-05**

It carried a `⚠️` banner saying it "spans two eras and is not uniformly current"
and a title note recording that its previous title described an era it outlived. A
document whose premise is "do not trust this uniformly" is a merge conflict
rendered as prose.

**Fixed by the former of the two recommendations:** the retired-Wails sections
(code structure, stack table, CI/migration notes) and the per-round VPS bug lists
were cut — they are owned by `archive/WAILS-MIGRATION.md`, `archive/ARCHITECTURE-wails.md`
and `FIXES.md` — and what replaced them is a 200-line orientation: glossary,
product, layout, naming rule, live-data rule, standing decisions and the pitfalls
index. Both banners are gone. Nothing was lost: the deleted material survives in
`archive/`, in `FIXES.md`, or in git.

---

---

## 4. The hub: `verified` claims are now **falsified**

**Recorded 2026-10-18 in `operate/CLAIMS.md` §5.** The hub has been taken down and
will be redeployed with B2-backed backups. Under `CLAIMS.md`'s own vocabulary that
distinction matters:

- **A1, A10, A11, A12, A13** named observations of a *specific box*. That box no
  longer exists. They are **false about the world**, not stale.
- **A4, A6, A7, A8** are observations about **CI and real hardware**, not the hub.
  The redeploy does not touch them. They are **stale** (dated to `5ebfc89`,
  `v3.2.12`, `a8b0acf`) and re-running the checks would settle them.
- **A5 is invalidated by 3.3.0 regardless of the redeploy** — the signing key was
  rotated 2026-10-16, so a client on the previous key series cannot verify a 3.3.0
  payload. That is a **new question**, not a re-check.
- **A3's subject no longer exists** — the `build-preview` release and its 136
  assets were deleted 2026-10-18 to cut storage. It must be rewritten against a
  recreation or retired; it cannot sit as "unverified".

The dates and evidence were deliberately **kept**. A claim's history is the record
that it was once true and how. The reading changed, not the register.

---

## 5. What this audit did **not** verify

Per `DEBUGGING-METHOD.md`, these are different claims and must not be blended:

- **Verified by running:** `check-consistency.sh` green (29 sections); the two new
  guards fail when the defect is reintroduced and pass when it is removed; zero
  broken internal links across 103 files; `/api/activate`'s rate limit matches
  both `activation.pb.js` and `Caddyfile`.
- **Verified by reading:** the "three version sites" contradiction; the tier
  names in `FRONTEND.md`/`LOGIC-INVENTORY.md`; the `state.toml` pointer errors in
  `client/docs/`.
- **Not verified:** that any *operate/* document is currently true of the live
  system — **the hub was down throughout**, so this was impossible rather than
  skipped. Every procedural document above is "true as written, unconfirmed
  against a running box".
- **Structural argument only:** that collapsing `README.md` loses nothing. It is
  argued from a section-by-section read and a link check, not from a reader study.

---

## 6. What is left

Per the ownership rule this audit itself cites, the *live* list of what is
unfinished is [`STILL-OPEN.md`](STILL-OPEN.md) — it is not repeated here. What
this audit uniquely holds is the one item that is about the **doc set's guards**:

**§10 of `check-consistency.sh` cannot see word-numbers.** It passed on
`CONTEXT.md`'s "three version sites" because *three* is a word, not a digit —
the exact failure `DEBUGGING-METHOD.md` describes: a guard that cannot see the
defect it exists for reads like a guard that passed. Extending it to spelled-out
numbers (where the sentence also names the version sites) is the highest-leverage
guard change left.

---

*Written 2026-10-18; revised 2026-10-05 for the `CONTEXT.md` rewrite and the trim.
This document describes a moment: when it and the code disagree, the code wins and
this is the bug.*
