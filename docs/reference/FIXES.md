# Fixes

```
audience:    builder
status:      live
authoritative-for: the dated defect log (append-only; newest first)
verified-against: docs/state.toml (version/release facts)
```

Everything below is a dated, append-only log of real defects and their fixes, in
reverse-chronological order; each entry names the release it went out in. **Treat
entries as historical** — later entries sometimes correct earlier ones, and the
corrections are marked. For what is *still* broken, read `STILL-OPEN.md`.

> **Looking for what is currently open, not what was fixed?** This file is a
> *record*, 4,900 lines long. The live list of unfinished and unverified work is
> [`STILL-OPEN.md`](STILL-OPEN.md); the commercial gaps are
> [`../business/17-not-built.md`](../business/17-not-built.md). The newest
> entries here are the most likely to still have an open follow-up.

> **The server-side halves do not reach the live host until `setup.sh` re-runs.**
> `setup.sh` deploys from `/root/server/` on the VPS, not from this repo, so every
> hook and script change here is inert until that re-run. The client-side changes
> are active as soon as the matching version installs. A heartbeat that lacks
> `enforcement_version` in its response is proof the hook is still stale — see
> `STILL-OPEN.md`.

---

## THE DOC SET WAS GREEN AND FRAGMENTED: TWO INDEXES, LYING TITLES, RETIRED MATERIAL IN LIVE FOLDERS (2026-10-05, docs + a new guard)

| | |
|---|---|
| Severity | 🟡 No code defect. Forty documents were reachable only by browsing the filesystem, and four described themselves wrongly |
| **Reported as** | *"restructuring and properly labelling the documentation to be clearer, as drift has caused the documentation to be fragmented"* |
| **Files:** | `docs/README.md`, `docs/STATE.md`, `docs/CONTEXT.md`, `docs/history/LAYOUT.md`, `client/docs/LOGIC-INVENTORY.md`, four moved documents, `server/scripts/check-consistency.sh` (§25 amended, **§28 new**), `client/src-tauri/src/feat/{tun,core_upgrade}.rs` |
| **Fixed in:** | docs only — no client change, so nothing to release |

### The defect, in one line

**Every guard was green and a reader still could not find or trust the docs.**
`check-consistency.sh` passed 26 sections; §8 proved every link *resolved*, §25
proved every live document *had* front-matter. Neither asked the questions that
had actually gone wrong, which were *"is it reachable?"* and *"does it agree with
its own label?"*

### Three findings, each invisible to the existing checks

1. **Two indexes, drifted.** `docs/README.md` covered `operate/` and `reference/`
   and stopped; `docs/STATE.md` carried a second, competing topic map. Between
   them they missed **39 live documents** — all 19 business files, the whole
   `redesign/` corpus, `client/docs/CONTRIBUTING_i18n.md`. A reader who trusted
   the index did not learn they existed. §8 could not see this: an unlinked
   document has no broken link.

2. **Titles and statuses that lied.** `docs/GAMING-UDP.md` was `status: live`,
   titled *"Implementation Plan"*, and its **own body** opened with two
   supersession banners. Nothing connected the front-matter to the prose, so a
   document could announce its retirement and stay labelled current forever.
   `CONTEXT.md` carried a `(V5 Reference)` era suffix spanning two eras;
   `LOGIC-INVENTORY.md` was titled *"the exact modules to write"* while being a
   `design-record` of modules that were written.

3. **Retired material in live folders.** `operate/DEPLOY-3.2.7.md` was a prepared
   deployment packet for a deployment that had **already happened** (zero inbound
   links); `business/redesign/03-device-binding.md` documented a mechanism that
   was **removed, not shipped**, with no folder-level separation from the half
   that shipped.

### The fix

- **One index.** `docs/README.md` is now the single map — every live document,
  one row, `status` shown, `client/docs/` included. `docs/STATE.md` dropped its
  competing map and answers only *"where does this fact go"*; its ownership table
  survives as the de-duplication index. Two files answering the same question was
  the fragmentation.
- **Retired material in `archive/`**, moved with `git mv` so history follows:
  `GAMING-UDP.md` and `DEPLOY-3.2.7.md` → `docs/archive/`;
  the two device-binding records → `business/redesign/archive/`. **The folder
  carries the status**, so their front-matter was removed rather than corrected.
- **Four titles renamed** to say what the document *is*.
- **Both halves guarded — §28.**

### The guard, and the lesson in getting it right

§28 checks the two things that failed: **index completeness** (every live
document under `docs/` and `client/docs/` must be linked from `docs/README.md`)
and **head-level self-contradiction** (a `status: live` document whose title or
first 25 lines say it is superseded).

Both halves were **observed failing** against the defect before being kept —
a live doc de-linked from the index, and a `SUPERSEDED` title on a `live` file —
and restored afterwards.

**§28's own first draft had the defect this project keeps re-earning.** It scanned
the *whole body* for retirement words and fired on **seven healthy documents**:
`FIXES.md` (a dated log whose heading records that something *else* was removed),
`STILL-OPEN.md` (whose job is marking superseded items), and five history sections
about retired mechanisms. Every hit was legitimate. The fix was to **scope the
check to where the defect actually lived** — the head — not to everywhere the
word could appear. §7's rule, applied to a new guard: *a warning nobody can
action trains the reader to ignore the one that matters.*

### Also repaired

- Two **live stale paths in code comments**: `client/src-tauri/src/feat/tun.rs`
  and `core_upgrade.rs` both pointed at `docs/ARCHITECTURE.md`, which moved to
  `docs/archive/ARCHITECTURE-wails.md` in the **2026-09** reorg and was never
  followed. §8's stale-path scan reads only `.md` files, which is why it survived.
- §25's archive exemption now covers `business/redesign/archive/`, and §10's scan
  skips it too (retired material is not held to the live-prose rule).
- The §7 version-basis exclusion and §10 URL-runbook exclusion both named
  `operate/DEPLOY-3.2.7.md`; repointed. A dead exclusion silently widens a check.

### Verified / not verified

- **Verified (ran):** `bash server/scripts/check-consistency.sh` exits **0**, 27
  sections, with **no new warning** — the §7 version-basis report is empty again
  after the index stopped naming a version in prose. `cargo test` 603 lib + 30
  integration green; `cargo clippy --all-targets --features clippy -- -D warnings`
  clean; `pnpm test` 96 green. Every markdown link target resolves, and the four
  moves were confirmed as `git mv` renames.
- **Verified by observation (the guard):** both §28 halves were watched going red
  against the exact defect, then restored, per `DEBUGGING-METHOD.md` §3.
- **Not verified:** that the *reorganised* structure is the right one for a reader
  — that is a judgement, not a measurement, and only use will settle it. Nothing
  here was checked against a deployed system; this change touches documentation
  and one guard, and no runtime behaviour.

---

## THE DOCS DESCRIBED REMOVED MECHANISMS AS LIVE, AND TWO PROSE COPIES OF A CAP WERE WRONG (2026-10-05, docs + guards)

**Not a code defect — a documentation set that had drifted from the code it
described, in the specific class the doc architecture exists to prevent.** The
tree's own guards were all green throughout: `check-consistency.sh` passed 24
sections, every link and anchor resolved, and every *value* it recomputed agreed.
What it could not see was the prose.

The audit that found this, and the fixes:

1. **The claim register contradicted itself.** `operate/CLAIMS.md` §5 A1 ("the
   hub answers `/api/health`") was marked `unverified`, and the file's own closing
   paragraph said the environment had *"no route to the hub"* — while
   `verify-live.sh` returned `A1 PASS` and `/api/health` returned 200. A1 was
   advanced to a dated verification and the paragraph was re-scoped from a
   present-tense claim to dated history.

2. **The verifier's date disagreed with the project's dateline.**
   `verify-live.sh` suggested `today's date` from `date -u` (UTC), but every
   register entry is written in the operator's NZ dateline (see `git log`), which
   is up to a day ahead. A date copied from the script would have been a day
   behind its neighbours. The script now prints the register's date, and A1 uses
   it (2026-10-05).

3. **Two prose copies of the free-tier cap were wrong.** The root `README.md`
   said `Eco 5Mbit ... tc caps (5/100/200 Mbps)` and `04-tc.sh`'s own header
   comment said `Eco (port 8443): 5 Mbps` — both the **pre-rename** value, above
   code that applies `1mbit`, for months, with every value-check green. Fixed, and
   **§23(g)** now pins those two restatements against `state.toml`. *A check on a
   value is not a check on what a document says about it.*

4. **The `uot_port` contract was documented backwards.** `client/docs/LOGIC-INVENTORY.md`
   claimed *"the hub hook renames on the way out"*; the hook passes the tier
   config **verbatim** and it is the **client** that tolerates both spellings
   (`contract.rs`). The doc is the bug; corrected.

5. **Three documents named a "client is 3.0.0" / "three version sites" fact in
   present tense**, contradicting the five-site rule the release path enforces.
   The count moved into `docs/state.toml` `[client.version_sites]` (derived, §9),
   and the prose links it, so a sixth site fails the build.

6. **The redesign corpus described removed device binding as live.** 17 files
   under `docs/business/redesign/**` had banners but no front-matter `status:`;
   three presented the removed one-code-per-device rule as current. All now carry
   front-matter, the removed mechanisms are marked in-place, and **§25** fails the
   build on a live document with no `audience:`/`status:` or an invented value —
   which immediately caught a seventh (`DEPLOY-3.2.7.md` had prose in its
   `status:` field).

7. **A permanent 14-file WARN wall was read as green.** §7 reported every
   document naming a non-current 3.x version, with a note that *"some are
   legitimately historical"* — and nobody triaged it. The historical ones are now
   excluded by exact path with a reason, so §7 is a real check: it reports only
   genuine drift and was observed failing against a seeded one.

Each new guard (§23(g), §25, the §9 `version_sites` fact, §7's triage) was
**observed failing** against the defect it catches before being kept, and the
suite is green after each fix.


| | |
|---|---|
| **Reported as** | *"the update to 3.2.26 is not an installer (48150583 bytes); refusing to execute it — this build cannot install a raw executable, and neither can the platform installer"*, from a client on 3.2.24. Concurrently, the admin console's *Fetch & publish* refused a **v3.2.28** publish with *"the hub's platform filenames disagree with CI's manifest … macos_intel: the hub would serve `locus-darwin-amd64`, but CI's manifest advertises `locus-darwin-amd64.app.tar.gz`"* |
| **Files** | `server/scripts/fetch-release.py` (`resolve_platform_names`, `fetch_release`), `server/scripts/check-consistency.sh` §24 (h)/(i)/(j), `server/scripts/smoke-macos-payload-resolution.sh` (new), `docs/operate/RECOVER-MACOS-UPDATE.md` (new) |

### The client was right; the hub served the wrong file

The error is precise, and every figure matches the live hub:

| Fact | Value |
|---|---|
| Error reports | `48150583 bytes` |
| `locus-darwin-arm64` `Content-Length` | `48150584` (48150583 == its exact byte count) |
| First bytes of that file | `cf fa ed fe 0c 00 00 01` — a **Mach-O 64-bit arm64** |
| `/api/update?version=3.2.24&platform=macos_arm` | `"version":"3.2.26"`, url `…/locus-darwin-arm64` |
| `…/locus-darwin-arm64.app.tar.gz` | **HTTP 404** |

`tauri_plugin_updater`'s macOS path is `GzDecoder` + `tar::Archive` over an
`.app` bundle. It cannot apply a bare Mach-O. The client's `is_installer_payload`
refused it — correctly — and **the hub had published an artifact no macOS client
could install**.

### Two defects, not one

**(1) The silent fallback published it.** `resolve_platform_names(available=…)`
returned the pre-fix bare Mach-O when a release carried no `.app.tar.gz`, logged
a WARNING at a level nobody reads, and carried on. v3.2.26 genuinely predates the
macOS packaging step (see the 2026-10-04 entry below), so the fallback fired and
wrote a live `update_config` row pointing every macOS client at a payload its
installer can never apply. **Degrading to an uninstallable payload is worse than
refusing:** a refusal is visible at publish time; a broken row looks healthy from
the operator's seat. This is the retired client's worst failure mode — *an update
advertised and then unable to install, with nothing saying why* — re-created on
the hub side.

**(2) The cross-check answered a different question from staging.** The staging
loop resolved the names *with* `available=set(assets)` ("what will the hub
fetch?"), while the manifest cross-check re-called `resolve_platform_names(version)`
*without* it ("what does the hub prefer?"). The two agreed only while every
release carried every preferred name, so the check could pass while a different
file was served — the "two sources of truth with no reconciliation" shape this
project keeps paying for.

### Why three existing guards passed

- §24's six assertions check that the **declarations agree** (CI's map, the hub's
  `NEW_PLATFORMS`, `publish-release.sh`'s `PLATFORMS`, the doc). They did agree.
  The defect was a **runtime branch that downgraded a correct declaration into a
  broken artifact** — invisible to a declaration-only check.
- `publish-release.sh` (the CLI route) was already safe: it hard-codes the
  tarball names and refuses a partial publish. Only the **fetch** route had the
  fallback. The console error was the CLI-shaped guard firing on a version whose
  release genuinely lacked the tarball.
- The macOS hash cross-check hashed whichever file the *manifest* named, so it
  agreed with itself by construction — the 2026-10-01 lesson, unlearned for a
  second slot.

### The fix

1. **The fallback is gone.** `resolve_platform_names(available=…)` now **raises
   `FetchError`** for a macOS slot with no `.app.tar.gz`, naming the platform, the
   missing file, and the pre-fix binary it refused to serve.
2. **One answer.** The manifest cross-check iterates the *staged* `resolved` list,
   so what is verified is what is served. The mismatch message now points at the
   real remedy first — *almost always a stale deployed `fetch-release.py`* — and
   `hooks-sync.sh --fetch-service`.
3. **Guards, each shown to fail** against the code it catches (the repository's
   rule):
   - §24(h) — `resolved = legacy` must not exist; the refusal must `raise FetchError`.
   - §24(i) — staging binds `resolved` from `available=set(assets)`; the
     cross-check iterates that list.
   - §24(j) — runs `smoke-macos-payload-resolution.sh`, which **imports the
     shipped function** and asserts the behaviour. Proven necessary: an evasive
     rewrite (`resolved = (legacy)`) slips past the (h) grep and is caught only
     by (j).

### Verified / not verified

| | Status |
|---|---|
| The live hub serves a bare Mach-O for `3.2.26` macOS slots | **Verified** (read-only `curl`; bytes and length match the report) |
| v3.2.26 predates the packaging step; v3.2.28 carries the tarballs + `.sig` | **Verified** (GitHub Release asset lists and both manifests) |
| The fallback, reproduced, published a bare Mach-O | **Verified** (imported the module; before/after in the session) |
| Each new guard fails against the pre-fix code | **Verified** (§24 (h)/(i)/(j) each shown red on a reverted copy) |
| The deployed host's `fetch-release.py` is the stale half | **Not verified** — needs `hooks-sync.sh --fetch-service --dry-run` on the host |
| A real end-to-end macOS update installs | **Not verified** — needs macOS hardware; recovery is `RECOVER-MACOS-UPDATE.md` |

---

## A MACOS STUDENT WAS OFFERED NOTHING, AND NOTHING SAID WHY (2026-10-04, v3.2.27)

| | |
|---|---|
| Severity | 🔴 macOS auto-update had never worked, and every path that suppressed an offer was invisible |
| **Reported as** | *"The update wasn't offered to a mac user on 3.2.24"* |
| **Files:** | `.github/workflows/client.yml`, `server/scripts/fetch-release.py`, `server/scripts/publish-release.sh`, `client/src-tauri/src/locus/update/{mod.rs,install.rs}`, `client/src-tauri/src/locus/{runtime.rs,store.rs}`, `client/src-tauri/src/config/verge.rs`, `client/src-tauri/src/cmd/locus.rs`, `client/src/pages/account.tsx`, `client/src/services/locus.ts`, `server/scripts/check-consistency.sh` §24 |
| **Fixed in:** | 3.2.27 |

### Two defects, and the one that explained the report

**First, the reason was unobservable.** Every path that suppresses an update
offer logged at **`debug`**, and the default log level is **`Info`**:

```rust
logging!(debug, Type::System, "[locus] update {version} is available but automatic checking is off; not offering");
```

So an offer could be silently dropped and the machine would carry **no trace at
all** — not in the log, not on screen. That is the report exactly: "it wasn't
offered", with nothing anywhere saying why. An update advertised and then
ignored is indistinguishable from a stable release with no update, which is the
failure mode this whole path exists to avoid.

Fixed by promoting those lines to `warn`/`info` and making the reason a **type**
(`NoOfferReason`) rather than a log call — then surfacing it on the Account page,
so the question "why is this device not updating?" has an answer on screen.

The trigger is most likely `auto_check_update`, which is **inherited from
upstream Clash Verge Rev** and whose Account-page row was once mis-wired to
auto-launch — so a stale `false` can persist across upgrades. That case now gets
its own wording and a one-tap fix.

**Second, macOS could not install what it was offered.** The hub's macOS slots
named a bare Mach-O (`locus-darwin-arm64`), while `tauri_plugin_updater` on macOS
runs `GzDecoder` + `tar::Archive` expecting:

```
Locus.app.tar.gz
└── Locus.app/
    └── Contents/...
```

and resolves `extract_path` to the `.app` **bundle**, not a single file. A bare
Mach-O downloads ~48 MB, verifies its signature, and then fails at extraction.
There was never a path in which a macOS client updated itself.

Fixed by packaging the ad-hoc-signed bundle as `*.app.tar.gz` in CI (after
signing, from the rebuilt `.dmg`), and repointing all three sites that name the
payload: CI's manifest, `fetch-release.py` `PLATFORMS`, and
`publish-release.sh` `PLATFORMS`.

### Why no guard caught this

§1 covers the Windows installer name and §1c covers the macOS *human download*
zip, but **nothing checked what a macOS client downloads to replace itself** —
so the mismatch survived every green pipeline. §24 now asserts the agreement
across all three sites, plus the negative (no bare Mach-O in a macOS update slot)
and the client-side half (`is_bare_executable` must match Mach-O magic, so a bare
macOS binary is refused as the wrong artifact kind rather than falling through as
an unknown "container").

§24's first draft had the defect it exists to catch: its client-side check grepped
for the words "Mach-O", which also appear in the doc comment and the tests. Both
scoped checks were observed **failing** against a deleted implementation before
being kept.

### Verified / not verified

- **Verified:** 603 lib tests (5 new); `cargo clippy -D warnings`, `tsc`, `lint`,
  96 frontend tests, `check-consistency.sh` (109 checks). All six §24 assertions
  and the client-side check observed **failing** against the exact defect, then
  restored. The hub's macOS format check now reads the tarball and confirms the
  inner bundle's architecture.
- **Not verified:** **that a macOS client installs the new payload.** No Mac was
  involved, and the packaged tarball has never been consumed by a real updater.
  The change is verified as far as "the artifact is the shape the plugin
  documents", which is a structural argument, not a measurement. See
  `STILL-OPEN.md`.

---

## THE FREE TIER'S ADVICE FIELDS COULD BREAK THE WHOLE HEARTBEAT, AND FOUR OTHER PRE-EMPTIVE FIXES (2026-10-04, v3.2.26)

| | |
|---|---|
| Severity | 🟠 Advisory free-tier data could fail a beat that carries contract data; three arithmetic edge cases could throttle or un-throttle a student wrongly |
| **Reported as** | *"Start patching gaps and writing in pre-emptive safety checks/error messages"* |
| **Files:** | `client/src-tauri/src/locus/{heartbeat.rs,usage.rs,store.rs,runtime.rs}`, `client/src-tauri/src/config/verge.rs`, `client/src/components/connection/usage-bar{,-model}.{tsx,ts}`, `client/src/locales/en/home.json`, `server/scripts/check-consistency.sh` §23 |
| **Fixed in:** | 3.2.26 (client only — no hook change, so nothing to redeploy) |

### The defects, each a latent failure rather than a reported one

None of these had been observed in the field. Each is a case where a *bad or
merely unusual* input produces a wrong outcome silently, which is the class this
project has repeatedly paid for.

1. **An advisory field could fail the whole beat.** `free_allowance_mb` was typed
   `Option<u64>`, so a hub sending `-1`, `5.5` or `"5120"` made the entire
   `HeartbeatResponse` fail to parse — which `classify_response` reads as
   `Unreachable`. The beat is then **discarded**, taking the `server_config`, the
   `expires_at` and any update signal with it, for a field that does not apply to
   a paying student at all. Fixed with a tolerant deserializer: the advisory
   fields degrade to `None`, everything else parses. This is deliberately the
   **opposite** rule from `uot_port` and the frozen wire names, where a wrong
   value *should* be loud — advisory data tolerates, contract data does not.

2. **An absurd allowance read as a permanent throttle.** At `u64::MAX` mebibytes
   the byte conversion saturates, and the saturated value then overflows
   `used * 10` in `classify` — so `>=` misreads every window as spent. The
   student is throttled instantly and permanently, on a hub that believes it sent
   an enormous allowance. Now clamped to 1 PiB (`sane_allowance_mb`), which
   degrades toward *unlimited* — the safe direction — and the runtime **logs**
   the bad value, because clamping silently would hide a server-side fault.

3. **A future-dated window underflowed the elapsed-time maths.** A window start
   ahead of `now` makes `now - start` wrap in release to an enormous value that
   reads as "the window just rolled" on every call: the quota silently ceases to
   exist and the student gets a fresh allowance on every poll. Now repaired
   (`window_start_is_skewed`, five-minute tolerance for ordinary clock noise).

4. **A corrupt counter reset with no trace.** `store::usage` treated an
   unreadable value as absent — correct, since a counter must never block a
   connection — but it did so **silently**, so "my free data keeps resetting" was
   a support report with no diagnosable cause. Now logged, via a pure
   `decode_usage` that is directly testable.

5. **The upgrade route did not exist.** The throttle state said what happened and
   that it resets, but offered nowhere to go. Now followed by a sentence naming
   the action. It deliberately does **not** offer a purchase: there is no
   self-serve checkout — a code is a physical card bought from a middleman — so a
   "Buy now" button would have nowhere to go.

### The guard was wrong on its first draft, in the way it warns about

§23's two new hub-key assertions grepped for the key name, and the key also
appears in the file's own comments and in its enforcement-version note. Deleting
the assignment therefore left the check **green** — a check that cannot fail,
which is the exact defect this whole section exists to catch, written by a change
quoting it. Both now anchor on the assignment (`response.free_allowance_mb =`),
and were observed failing against a deleted assignment before being kept.

A sixth assertion was added for a subtler no-op: a value that is **stored but
never read**. `free_throttle_mbps` is exactly that today — parsed, persisted, and
applied to nothing — so the guard asserts a reader exists, which makes the
remaining gap explicit and greppable rather than hidden behind a field that looks
wired.

### Verified / not verified

- **Verified:** 597 lib tests pass (10 new across `usage`, `heartbeat` and
  `store`); `cargo clippy --all-targets --features clippy -- -D warnings` is
  clean; `tsc`, `lint`, 96 frontend tests and the web build pass;
  `check-consistency.sh` exits 0 with 101 checks. Each new guard was **observed
  failing** against the defect it catches — the removed skew repair, the strict
  `u64` field, the deleted hub assignment, and the removed store reader — then
  restored.
- **Not verified:** that any of this behaves differently on a **live Core**,
  because none of it was run against one. In particular the throttle is still not
  applied, and the clamping only triggers on a hub sending a value no deployed
  hub sends. See `STILL-OPEN.md`.

---

## THE FREE TIER HAD NO ENFORCEMENT, AND THE BUSINESS PLAN READ AS IF IT DID (2026-10-04)

> **Follow-up entry below (3.2.26) covers the safety guards and the upgrade route.
> Read this one first — it is the build the follow-up hardens.**

| | |
|---|---|
| Severity | 🟠 A whole tier was specified in the plan and absent from the tree; the docs read as though it shipped |
| **Reported as** | *"Let's start getting to work on the undone work mentioned in business(free)"* |
| **Files:** | `server/modules/04-tc.sh`, `server/scripts/seed-pb.py`, `server/scripts/seed-live.py`, `server/scripts/fix-tier-configs.py`, `server/pb_hooks/heartbeat.pb.js`, `client/src-tauri/src/locus/{usage.rs,heartbeat.rs,store.rs,runtime.rs,mod.rs}`, `client/src-tauri/src/cmd/locus.rs`, `client/src/config/verge.rs`, `client/src/services/locus.ts`, `client/src/components/connection/usage-bar.{tsx,ts}`, `client/src/pages/connection.tsx`, `server/scripts/check-consistency.sh` §23, `docs/business/*` |
| **Fixed in:** | next tag (client) + the hook and `tc` module, deployed on a `setup.sh` re-run |

### The defect, in one line

`docs/business/04-tiers.md` described a free tier at 1 Mbps with a 5 GB
client-counted allowance; the tree had `create_tc_service "eco" … "5mbit" 8443`
and a client that had never heard of an allowance. Nothing was broken by this —
nothing had been built yet — but the document's own table called itself
"code-verified" while its §4.5 conceded the cap was "proposed".

### What was built, and the two things that were nearly got wrong

The enforcement now exists end to end: a 1 Mbps `tc` cap on 8443, a `free`
`tier_configs` row beside the legacy `eco` one, an allowance sent on every
heartbeat, and a client that counts its own window and renders it.

Two traps, both of which the §23 guard exists to catch:

1. **The cap lives in two places in one file.** `04-tc.sh` applies the class
   *and* writes the `tc-eco-cap.service` oneshot that rebuilds it at boot. Editing
   only the first is a change that works until the next reboot — the classic
   half-applied fix. §23 asserts both, and was observed failing against each.
2. **The hub key needs a client reader.** This is the UoT failure mode again
   (`FIXES.md` 29): a wire key with no reader is a *silent no-op*, not an error.
   The guard's client-side assertion greps the **field declaration**, not a bare
   mention of the name — the first draft matched the doc comments and would have
   passed with the field renamed. It was observed failing against exactly that.

### The genuinely soft part, stated rather than hidden

The quota is counted **client-side** and is tamperable. That is a decision, not an
oversight: the hub has no per-user accounting, because each tier is one
shadowsocks instance with a single shared password, so free users are
indistinguishable on the wire. The threat model is a student who wants free fast
internet, not an adversary. The alternative is P4 in `18-open-items.md`.

The counter is **not** reported back to the hub, deliberately — doing so would
create the per-user record this product's design refuses to keep.

### Verified / not verified

- **Verified:** 587 client lib tests pass (11 new for `usage`, 3 for the
  allowance wire shape); `cargo clippy --all-targets --features clippy -- -D warnings`
  is clean; `pnpm exec tsc --noEmit`, `pnpm lint` and `pnpm test` (95 tests) pass;
  `pnpm run web:build` succeeds; `check-consistency.sh` exits 0. Every §23
  assertion was **observed failing** against the defect it catches (the 5 Mbps
  cap, the missing `free` row, the missing heartbeat key, the ungated allowance,
  and the renamed client field), then restored.
- **Not verified:** that the client applies the slower cap to a **live Core** —
  the classification is tested, the effect on a running tunnel is not. No hub was
  redeployed, so **none of the server half is live**. Both are in `STILL-OPEN.md`.

---

## macOS SHIPPED A TERMINAL-ONLY REMEDY: A README THE STUDENT COULD NOT ACT ON (2026-10-03)

| | |
|---|---|
| Severity | 🟠 Every macOS student who met the Gatekeeper dialog needed a command line to get past it |
| **Reported as** | *"remove the read me text from the macos install, really unnecessary"* |
| **Files:** | `.github/workflows/client.yml`, `client/src-tauri/packages/macos/READ ME FIRST.txt` (deleted), `server/scripts/check-consistency.sh` §20, `docs/operate/OPS.md` |
| **Fixed in:** | next tag (macOS only) |

### The defect, in one line

The mitigation for an unsigned macOS build told the student to run `xattr -cr` in
**Terminal** — which is not a remedy for someone who cannot evaluate a shell
command, and it was the only path forward, because an unsigned + quarantined
`.app` produces *"damaged and can't be opened"*, a dialog with **no** "Open
Anyway".

### Why the fix is signing, not better wording

The wording was already accurate. What was wrong was the class of failure: macOS
distinguishes a bundle with **no** signature (reported as corrupt) from one with a
signature that is **not a Developer ID** (reported as unverified), and only the
second is surfaced in System Settings → Privacy & Security with an actionable
button. So `codesign --force --deep --sign -` does not make the app trusted — it
moves the failure into the class the student can escape **without a Terminal**.

The README was removed rather than kept alongside it. Two remedies that disagree
about which is primary is worse than one, so `check-consistency.sh` §20 now
asserts the file is **gone** and that the signing step exists — inverting a guard
that previously asserted the opposite.

### The guard was wrong twice before it was right, and both times were instructive

`§20`'s replacement is an inverted assertion, and the first two drafts failed on
**their own documentation**:

1. A bare `grep 'READ ME FIRST'` matched the comment explaining *why* the file was
   removed. The cheapest way to make it green would have been to delete the
   explanation — a guard passing for the wrong reason, with the reasoning lost.
   Fixed by matching the **mechanism** (`cp`/`install` of that path, or a read-back
   of it) rather than the name.
2. The doc check then flagged the sentence "the `.dmg` **no longer** carries a
   README" — a sentence that *agrees* with the removal. Fixed by asserting the
   operational verb and filtering explicit negations.

This is the §3 class ("a check that cannot distinguish a right answer from a wrong
one reads exactly like a check that passed") found in a guard written by the same
change that was quoting it — which is the argument for probing a guard before
keeping it, not after.

### Verified / not verified

- **Verified:** all five §20 assertions and all eleven §1c assertions were
  observed **failing** against the defect each catches, then restored; the workflow
  and both embedded scripts parse; `check-consistency.sh` exits 0 (89 OK / 0 BAD);
  the console builds and type-checks.
- **Not verified:** that macOS actually draws the corrupt-vs-unverified distinction
  for this build, that "Open Anyway" appears, and that a student can open the
  extracted `.app`. No Mac was involved. See `STILL-OPEN.md`.

---

## A NEW LOOKUP STATUS NAME LEFT REINSTALLING STUDENTS UNABLE TO RE-ENTER THEIR CODE (2026-10-03)

| Severity | 🔴 Every student who uninstalled and reinstalled was stranded, and could not update out of it |
|----------|--------------------------------------------------------------------------------|
| **Reported as** | *"they can re-enter the code, but if they uninstall and reinstall, the code no longer works for them, and requires a release from the admin ui"* |
| **Files:** | `server/pb_hooks/code_lookup.pb.js`, `locus/contract.rs`, `locus/activation.rs`, `tests/activation_contract.rs` |
| **Fixed in:** | 3.2.23 (client) + the hook, deployed immediately for 3.2.22 and older |

### The defect, in one line

The hub invented a **new status string** on a route whose old clients parse it as
an **allow-list**, and the fallback they land on is a dead end.

The lookup began reporting a redeemed code as `already_used`. A deployed
client's `LookupStatus` is an allow-list with `#[serde(other)] -> Unknown`, and
the deployed `classify_lookup` read `Unknown` as `ready: false` — so the client
showed "Could not check this code right now" and **never called
`/api/activate`**. The hub's activate path was correct the whole time and would
have returned `200 "Already activated"` with the tier config. The gate stopped
the request before it was made.

The only visible symptom was on the client, and the only remedy an operator had
was to **release the code** — which clears `activated_at`, so the lookup then
reported `unbound` and the old client accepted it.

### Why the mistake looked safe at the time

The comment that shipped with the change said an unrecognised status "falls back
to the activate-time check, which is the correct behaviour anyway". That was
**wrong**: the fallback is `Unknown = not ready`, i.e. a refusal. The same
reasoning is repeated in the client enum's own note. A fallback is only a
fallback if it fails *open*; this one failed closed, and nothing tested the
two sides against each other.

**The lesson, stated as a rule:** on a route a client parses by allow-list, a new
status string is not additive — it is a **breaking change that disables the
feature for every client that cannot be updated**. Where the meaning already
exists, reuse the frozen name.

### The fix, both halves

- **Hub:** report a redeemed code as the **frozen** `bound_this_device`, which
  every deployed build already maps to `ready: true`. No client update was needed
  for anyone in the field, so this shipped to the hub immediately and unblocked
  everyone still on 3.2.22 or older.
- **Client:** accept both names (`#[serde(alias = "bound_this_device")]`), and
  treat a genuinely unknown status as **proceed**, not block — so a future hub
  cannot strand a client this way again. A client that cannot get past the lookup
  cannot reach the updater either, which is what made this unrecoverable.

### The tests assert the AGREEMENT, not each side alone

- `a_redeemed_code_reports_a_status_deployed_clients_accept` — the hub emits the
  one name deployed clients treat as ready, and does *not* emit `already_used`.
- `an_unknown_lookup_status_does_not_block_activation` — the client's `Unknown`
  arm is `ready: true`.

Both were shown to **fail** against the bug before being kept (reverting the
hub status to `already_used`; flipping the client arm to `false`).

### Verified live (2026-10-03)

Against the deployed hub, with a real code:

1. fresh code → lookup `unbound`; activate → `200 "Activation successful"`.
2. same code again → lookup **`bound_this_device`** (was `already_used`).
3. **simulated reinstall** (different fingerprint) → activate → `200 "Already
   activated"` **with `server_config`**, i.e. a working tunnel, no operator
   action.
4. `activated_at` was **not** re-stamped by the re-activation, so a reinstall
   does not reset the term.

The probe code was released afterwards, restoring the hub to its prior state.

### Unverified

That a real Windows/macOS client, uninstalled and reinstalled, now completes this
flow through the UI. The hub half is proven by the calls above; the client half
is proven by unit tests and the contract tests. No client on 3.2.23 exists yet.

---


| Severity | 🔴 A student who reinstalled, reset, or was offline past the grace period lost their code and had to find a card they may have thrown away |
|----------|--------------------------------------------------------------------------------|
| **Reported as** | *"when an update happens, or the app is shut off after some unexplainable circumstances, the code for the client app is no longer saved so it puts you back into the code entering menu … the hardware identification logic … is completely broken and does not work, should be removed entirely"* |
| **Files:** | `locus/credential.rs` (new), `locus/store.rs`, `locus/runtime.rs`, `utils/resolve/mod.rs`, `locus/identity.rs` (deleted), `locus/activation.rs`, `locus/heartbeat.rs`, `cmd/locus.rs`, `src/main.tsx`, `src/pages/activation.tsx`, `src/services/locus.ts`, `src/pages/recognition-notice.ts` (deleted), `server/pb_hooks/*`, `server/scripts/*` |
| **Fixed in:** | next tag (client + hook) |

### Two defects, one decision

**(1) The code lived only in `verge.yaml`** — the app's own config, which an
uninstall deletes, an update can replace, and the app's backup/restore can reset.
On top of that, `store::clear()` wiped it on a **lapsed grace period**, which is a
local statement that the hub could not be reached for a week — not a statement
that the code is bad. Two ways for a student to lose a code they may no longer
have a card for.

**The fix is durability, not a safety net.** The code is mirrored to the same
machine-scoped store the identity used (`locus/credential.rs`), owner-only, and
`store::read` prefers the config with the mirror as fallback.
`store::clear` is split: `clear_entitlement` (a definitive hub refusal — suspended,
expired, refunded) removes the code and the mirror; `record_lapsed_grace` clears
only the session and **keeps the code**. The rule: *a local inability to confirm
the entitlement must never destroy the credential.*

**(2) Device recognition was removed, not repaired.** It asked the hub whether it
remembered the device so the code prompt could be skipped, keyed on a durable
device identity that was never actually written on the activation path — so it
looked up a row that did not exist and every device looked unknown. Codes were
additionally bound to that identity, so the student's own machine was refused
after a reinstall. The report says to remove it, and removing it is the right
call: binding an entitlement to a device is a heavier mechanism than the problem
needs, and it fails whenever the device is replaced — the ordinary case.

The replacement is strictly simpler: a code is **single-use and not tied to a
device**, and `codes.activated_at` is the whole record of redemption.
Re-activating a code restores access (a reinstall, a new machine) rather than
being refused, which is the recovery path; and because the code is now durable on
the device, a student rarely needs even that.

### What was removed

- Client: `locus/identity.rs`, `activation::recognise`, the `RecognitionResult`
  command, the `Credential::Token` heartbeat path, the retired `verge.yaml`
  fields (kept as inert `Option`s so an upgrade does not fail to deserialize),
  and the frontend recognition UI (`recognition-notice.ts`, the `Shell` effect,
  the `ActivationScreen` prop).
- Hub: `registerIdentity`, `migrateBindingIfSameDevice`, `recordBinding`,
  `deviceBoundToOtherCode`, the `device_bindings` index, the token branch of
  `/api/heartbeat`, and `/api/device-recognise` (a tombstone answering a uniform
  `unknown` so deployed 3.2.x clients fall through to the prompt instead of 404ing).
- The lookup ~~reports `already_used` instead of `bound_this_device` /
  `bound_other`~~. **Corrected 2026-10-03** — see the entry at the top of this
  file. Reporting a new status name to clients that parse the vocabulary as an
  allow-list broke reinstall for every deployed client. The lookup reports the
  frozen `bound_this_device` again.
- The console's `codes.rebind`, `device.get` and `devices.list` are retired
  (each returns a sentence explaining why); `codes.unbind`/`admin/unbind-code`
  now release the single-use stamp.

### The guard that could not fail

`check-consistency.sh` §(e) asserted that any hook writing a fingerprint
normalised it first — a rule about `device_bindings` and `codes.bound_fingerprint`.
With both gone its grep matched nothing, so it would have read **green forever
while checking nothing**. It was retired and replaced with two checks that can
fail: one that only the release paths clear `activated_at`, and one that no hook
references `bound_fingerprint`. Both were verified to fail against a deliberate
edit before being kept.

### Verified / unverified

- **Verified:** `cargo test` (567 lib + 28 integration), `pnpm typecheck`, `pnpm
  lint`, `pnpm test`, `node --check` on every hook, `check-consistency.sh` exits
  0, and both new guards fail against a deliberate edit.
- **Unverified:** the hub hooks are not executed here. They need a PocketBase
  deploy and a real activation/reinstall to exercise end to end. The retired
  collections are left in place on an existing hub and are not dropped.

---


| Severity | 🔴 The recurring "stuck on connecting" report, fourth occurrence |
|----------|------------------------------------------------------------------|
| **Reported as** | *"the tunnel can form, my ip can properly change, and data can flow properly, however it still shows 'connecting' without ever changing to connected … this has been recurring across several fixes where the agent claimed to fix it however failed"* |
| **Files:** | `core/manager/traffic_probe.rs` (new), `core/manager/probe.rs`, `core/manager/mod.rs`, `core/tray/speed_task.rs`, `core/tray/mod.rs`, `cmd/locus.rs`, `src/services/locus.ts` |
| **Fixed in:** | next tag |

### The defect, in one line

`Readiness::Ready` had exactly one route — a mihomo **delay test** — and a delay test
is a question we put to the Core, so it can be answered "no" about a tunnel that is
carrying the student's traffic perfectly well.

### Why the two previous fixes could not have worked

The 2026-10-01 fix tuned `delay_is_a_measurement` (how a *returned* delay is
classified); the 2026-10-02 fix moved the question from the `select` group to each
outbound by name (which member is *asked*). Both are repairs to the same load-bearing
assumption: **that asking the Core to dial something is how we find out whether the
tunnel works.** Three symptoms in three days with the same remaining shape is the
signature of a wrong premise, not a wrong constant. The student's own screenshot —
download and upload numbers moving on the same screen that says "connecting" — was
the disproof sitting in the report the whole time.

### The fix: a second proof that is not a question

The Core pushes `/traffic` once a second with `{up, down, upTotal, downTotal}`. A
sample with `up > 0 || down > 0`, **arriving recently**, is a byte the Core moved on
the student's behalf — observed, not inferred, and impossible to be a false positive.

`decide` is now a disjunction: **proven egress OR observed traffic**. Either is
enough. A false negative in one half is exactly what the other covers — the delay
test misses when the machine is idle or the tier's outbound is not the one carrying
traffic; the traffic stream is silent when the student is reading a document.

### The trap, stated because it is the obvious way to get this wrong

mihomo reports `up`/`down` (this sample's **rate**) *and* `upTotal`/`downTotal` (the
Core's **lifetime** totals). "Show connected if the numbers are non-zero" reads
correctly and is wrong: a total is a latch with a counter in front of it, true for a
tunnel that moved one byte and then died, and true for bytes moved *before* the
student pressed Connect. Only the current sample's rate counts, and only while
samples keep arriving.

### Guards, each observed failing

- `observed_traffic_alone_is_ready` — **red** under the previous one-condition rule.
- `traffic_that_stops_ends_readiness` — **red** under the same revert; pins that a
  counter is a measurement and not a latch.
- `a_quiet_sample_after_traffic_stops_being_evidence`,
  `traffic_older_than_the_window_is_not_active` — **red** against a
  `return true`-on-total implementation.
- `a_serving_core_carrying_nothing_is_not_connected` — the school-wifi rule, still
  green: the disjunction did not weaken it, only added a second honest route.

One subscription, two readers: the macOS tray rate task already reads `/traffic`, so
`traffic_probe` starts its own stream **only** when that task is not running
(`Tray::speed_task_running`) and the tray publishes into the same sink. The stream
starts lazily from `observe_readiness`, so a machine that never reads readiness never
opens the socket.

### Not verified

That this was the reporter's cause, and that a real tunnel now reports ready — no run
on a real school link. The delay test's false-negative mechanism is argued from the
plugin source and from the report's own screenshot; the traffic route is pinned by
unit tests over a pure rule, not against a live Core. See `STILL-OPEN.md`.

---

## A CODE WAS BOUND TO A RE-DERIVED HARDWARE HASH, SO AN UPDATE COULD TELL A STUDENT THEIR OWN CODE BELONGED TO SOMEONE ELSE (2026-10-02)

| Severity | 🔴 A student's own code refused after an update, with no self-service recovery |
|----------|--------------------------------------------------------------------------------|
| **Reported as** | *"codes keep becoming invalid for the very devices they were originally bound to after updates … sometimes it gives the message 'Welcome back — this device is already registered, so no code is needed', but on the same page it asks for an activation code"* |
| **Files:** | `locus/device.rs`, `locus/identity.rs`, `cmd/locus.rs`, `locus/runtime.rs`, `server/pb_hooks/activation.pb.js`, `src/main.tsx`, `src/pages/recognition-notice.ts` |
| **Fixed in:** | next tag (client + hook) |

### The defect, in one line

Two device identifiers existed and disagreed, and codes were bound to the **unstable
one**.

| | Identifier | Persisted? | Used for |
|---|---|---|---|
| A | `device::fingerprint()` — `MAC + disk serial + board UUID`, hashed | **No** — `OnceLock` for the process only | **binding** (`codes.bound_fingerprint`, `device_bindings`) |
| B | `identity::DeviceIdentity::device_id` / `verifier` | **Yes** — machine store, then app-config fallback | recognition (`device_identities`) |

A is re-derived on **every launch**. A NIC enumerating in a different order, a disk
serial becoming unreadable, or the `combine` rung degrading (`mac+disk+board` →
`mac+board` → `mac+host`) all produce a *different* digest — and the hub refuses the
code with `403 "Code bound to another device"` (`activation.pb.js`). The identity
work (commit `59c632a`) had already recognised this exact hazard for recognition —
its resolver says *"**A stored identity wins.** … Re-deriving on every launch is the
bug being fixed."* — but binding was left on the re-derived hash. The fix finishes
that decision rather than inventing a third scheme.

The same-page contradiction is the same divergence seen from the UI: recognition
(B) succeeds and says "no code is needed" while the gate is still up, because
whether the app is `activated` is `locus_status`'s answer and that value came from
the **stored activation keyed to A**.

### The fix

1. **Binding uses the durable identity.** `locus_activate` sends
   `identity.binding_id()` (the persisted `device_id`) as the binding value instead
   of `device::fingerprint()`. `locus_check_code`, `locus_status` and the heartbeat
   credential path (`runtime.rs`) do the same, so every site that names the device
   now names the same one. `DeviceIdentity::binding_id` is the single definition.
2. **The hub migrates a pre-existing binding.** A device bound *before* this change
   has the old fingerprint on its code row, so its first post-update activation
   presents the new id and would 403 — turning an intermittent per-update failure
   into a guaranteed one for **every already-bound device**. `migrateBindingIfSameDevice`
   runs when the fingerprint mismatches and rebinds the row only when the incoming
   `verifier` proves the caller holds a secret `device_identities` already names as
   belonging to **that very code**. It cannot move a code to a device that was not
   already entitled to it, and it fails closed on any uncertainty (→ the ordinary
   403).
3. **The gate no longer makes a promise it cannot keep.** `noticeForGate` suppresses
   the durable `restoring` notice ("no code is needed") while the gate is still
   rendered; if the entitlement really landed, the gate clears and the screen
   unmounts, so that notice is only ever visible when it would be false. The
   durability *warning* and the `unavailable` caution pass through unchanged.

### The decisions, and why

**`bound_fingerprint` and `device_bindings` keep their names.** They are read by
`admin_console.pb.js`, `admin_unbind.pb.js` and `code_lookup.pb.js`, and are frozen
wire contracts. Only the *value's provenance* changed.

**The binding value is the `device_id`, not the `verifier`.** The `device_id` is
non-secret and already the string the hub stores in `device_identities.device_id`;
the `verifier` is the credential and must not become a durable, displayed binding
key.

**`device::fingerprint()` is retired from every activation and heartbeat path, but
not deleted.** It still seeds a first-run identity (`identity::resolve`) and serves
diagnostics. Its module doc now says so, and why it is unfit to be a binding key.

### Verified / not verified

**Verified locally:** `cargo test` — 557 lib + 17 contract + 5 + 3 + 4 + 3
integration, 0 failed; `check-consistency.sh` exits 0; vitest 93/93; `tsc --noEmit`
and `eslint src --max-warnings=0` clean.

**Demonstrated failing (three guards, each shown red against the code it catches,
then restored):**
- `a_code_binds_to_the_durable_identity_not_the_hardware_fingerprint` — reverting
  `locus_activate` to `device::fingerprint()` fails it with the drift message;
- `the_activation_request_carries_both_the_binding_value_and_the_verifier` —
  removing the migration **call site** fails it. (An earlier version of this guard
  checked only that the helper was *defined*, and passed with the migration inert —
  the "a check that cannot fail" trap, caught and tightened.)
- `locus::identity::tests::a_stored_identity_wins_over_changed_hardware` — disabling
  the stored-wins rule fails it.

**Observed live against the hub (2026-10-02):** `/api/device-recognise` answers an
identical uniform miss for a malformed and a well-formed-unknown verifier;
`/api/activate` returns 400 (bad checksum / missing fingerprint) and 404 (unknown
code) as documented.

**Not verified, and stated as such:**
- The `403 → migrate → 200` path has **not** been exercised end to end. It needs a
  real code bound to an old fingerprint plus a device presenting the identity; no
  live code was touched (it is production data), so this is a **structural**
  argument from `activation.pb.js`, not an observed behaviour.
- That a real Windows/macOS update-and-reinstall now keeps its code keeps needs the
  hardware; this environment cannot show it. See `STILL-OPEN.md`.
- **The hook is inert until `setup.sh` re-runs on the host** — `pb_hooks/` deploys
  from `/root/server/`, not this repo. The client half is active on install; the
  migration half is not, and without it an already-bound device would 403. This
  ordering matters: **deploy the hook before or with the client.**

---

## THE "CONNECTING" BUG, THIRD TIME: THE PROBE ASKED THE GROUP, WHICH ANSWERS FOR ONE MEMBER (2026-10-02)

| Severity | 🔴 A working tunnel rendered as **connecting** for the third report running |
|----------|-----------------------------------------------------------------------------|
| **Reported as** | *"the logic for the 'connecting' state is still broken, the tunnel is obviously formed … however it's still stuck on connecting"* |
| **File:** | `core/manager/probe.rs` (`egress_attempt_once`, `decide`), `locus/tier.rs` (`OUTBOUND_NAMES`) |
| **Fixed in:** | 3.2.20 |

### The defect, in one line

The probe asked mihomo about the **proxy group**, and a group delay test answers
for the group's **currently selected member** — not for the outbound the student's
traffic uses.

### Why the two earlier fixes could not have caught it

Both previous entries worked on `delay_is_a_measurement` — *how a returned delay is
classified*. This defect happens one hop earlier: the probe never receives a
classifiable value at all. `delay_group` raises any non-2xx as `Err`
(`ret_failed_resp!` in `tauri-plugin-mihomo`'s `mihomo.rs`), so when the selected
member fails, the engine's `504 {"message":"get delay: all proxies timeout"}`
reaches the client as an opaque transport error with no delay map to inspect. No
value of the classifier can rescue it. A third round of tuning that function, which
is what the first two reports produced, could not have fixed this.

### Why the group's selected member is the wrong thing to ask

`tier::proxy_group` builds a `type: select` group whose members are
`["Locus-UoT", "Locus"]` on a UDP tier and `["Locus"]` otherwise. A `select` group
with no explicit choice uses its **first** member, so a Strike tier defaults to
`Locus-UoT` — while ordinary TCP traffic falls through `MATCH,Locus Auto` to
whatever the group has selected, and the two are different outbounds with different
server ports. The probe could therefore ask a question the tunnel did not have to
answer: `Locus-UoT` fails its test, mihomo reports "all proxies timeout", and the
client concludes the tunnel is dead while the student's traffic, taking the other
path, flows.

Measured against the real sidecar (v1.19.31) with a two-member group whose first
member is unreachable:

```text
GET /group/Locus%20Auto/delay?url=…&timeout=5   -> 504 {"message":"get delay: all proxies timeout"}
```

### The fix, and the asymmetry it rests on

The probe now asks each outbound **by name**, trying the next when one fails:

```rust
for member in crate::locus::tier::OUTBOUND_NAMES {
    let probe = …delay_proxy_by_name(member, EGRESS_TEST_URL, timeout_secs);
    match … { Ok(Ok(r)) if delay_is_a_measurement(r.delay, ..) => return true, _ => {} }
}
```

This works because the two plugin calls fail **differently**, and that difference is
the whole fix:

| Route | A non-2xx becomes | What the caller can do |
|---|---|---|
| `/group/<g>/delay` | `Err` (`ret_failed_resp!`) | nothing — no map to inspect |
| `/proxies/<n>/delay` | `Ok(ProxyDelay { delay: 0 })` | classify it: `0` is not a measurement, so this member failed |

So a dead member is an ordinary rejected value and the loop moves on; only "none of
the members could" is `EgressOutcome::Failing`.

### The simplification the same report asked for

The reporter's own diagnosis — *"I believe the current logic is over engineered"* —
was correct, and it was load-bearing rather than cosmetic. `Readiness` had four
states, and two of them were the same fact:

- `NotReady` — "the Core is not answering";
- `NoEgress` — "the Core is answering but no packet got through".

**Nothing consumed the difference.** `phaseFromStatus` derives the phase from `ready`
alone, so both rendered *connecting*. Carrying two states bought no behaviour, and it
cost a defect: the 2026-10-01 second-round bug lived in the untested one of the pair.

`Readiness` is now `Ready | Connecting | Stopped`, and `decide` answers one question:

```rust
if matches!(egress, EgressOutcome::Ok) { return Readiness::Ready; }
if matches!(outcome, ProbeOutcome::NotRunning) || !latch_active { return Readiness::Stopped; }
Readiness::Connecting
```

`ProbeOutcome::Serving` was also dropped as a *prerequisite* for `Ready`. It was
treating "the Core can answer its own control API" as a condition on top of "a
packet completed through the tunnel" — but the second implies the first (a Core that
moved a packet is answering), so requiring both was a second vote on a settled
question, and one that could refuse to say *connected* while carrying traffic. The
local probe is now only a gate on whether asking is worth it.

### Guards, each observed failing

- `only_proven_egress_is_ready` — passing egress with a non-`Serving` outcome. Under
  the old rule (`Ready` also requires `Serving`) this is red; **observed failing**
  before being kept.
- `every_no_egress_cause_is_the_same_state` — pins that no-egress causes collapse to
  one state, so a future split has to argue for itself.
- `a_dead_member_answers_with_a_non_measurement_not_an_error` (engine test) — pins
  the member-route contract the loop depends on.
- `every_outbound_the_probe_asks_about_is_one_the_profile_defines` — pins that
  `OUTBOUND_NAMES` and `build_profile` agree. **This caught a genuine bug in the
  fix itself**: the first draft asked about `Locus-UoT` on a TCP-only tier, where the
  profile defines only `Locus`. Harmless (the probe falls through) but it wasted a
  round trip on every check, and the test's stricter first form failed loudly.

### Claim class

- **Verified:** the group-vs-member error asymmetry, read from the plugin source and
  reproduced against a local sidecar; the tier's group defaults to its first member;
  the full suite (587 Rust, 87 vitest) and `check-consistency.sh`.
- **Not verified:** that this was the reporter's cause, and that a real tunnel now
  reports ready — no Locus tunnel on a school link was measured. **The engine test's
  positive case SKIPs green on this machine** (the runner cannot complete a round
  trip), so its green tick is not evidence. See `STILL-OPEN.md`.

---

## THE "CONNECTING" BUG CAME BACK A THIRD TIME, AND THE TEST PINNING IT WAS STALE (2026-10-02)

| Severity | 🔴 A working tunnel still rendered as **connecting** |
|----------|-------------------------------------------------------|
| **Reported as** | *"the tunnel is obviously formed from both the download/upload bars reporting numbers, and whatsmyip reporting the vpns ip, however it's still stuck on connecting"* |
| **File:** | `client/src-tauri/tests/egress_probe_engine.rs` (the stale pin), `client/src-tauri/src/core/manager/probe.rs` (the live rule) |
| **Status:** | The *test* drift is fixed here. The live cause is **narrowed to one reachable case**, below — and it is not the case the last two fixes addressed. |

### What is actually new here

The previous two entries both fixed the *classifier* (`delay_is_a_measurement`).
Both were verified only by unit tests and a struct-shape assertion. While
re-checking against the real engine (mihomo v1.19.31) this time, two facts fell
out that neither entry had measured.

**1. The integration test was pinning a rule the app no longer had.** Its copy of
the classifier still read:

```rust
delay > 0 && delay < timeout_ms && delay <= IMPLAUSIBLE_DELAY   // <-- withdrawn
```

while the shipped rule had dropped the `delay < timeout_ms` term in the
2026-10-01 second-round fix. So the test that exists to keep the two in step was
asserting the **exact behaviour that second-round fix removed** — and it stayed
green, because nothing ran it against the shipped rule. Restored to the shipped
rule here, and `the_engine_sentinels_are_not_measurements` was rewritten: it had
been asserting `!delay_is_a_measurement(3000, 3)`, i.e. pinning the bug. It failed
red before the rewrite, which is how it was found.

**2. A group whose members all fail returns no delay map at all.** Measured
against the real sidecar:

```text
GET /group/Locus%20Auto/delay?url=…&timeout=5     -> 504 {"message":"get delay: all proxies timeout"}
GET /proxies/Locus-Dead/delay?url=…&timeout=5     -> 503 {"message":"An error occurred in the delay test"}
```

The plugin raises any non-2xx as `Err` (`ret_failed_resp!` in `mihomo.rs`), so
`egress_attempt_once` collapses it to `false` and `decide()` returns
`Readiness::NoEgress` → `ready: false` → `phaseFromStatus` → **`connecting`**,
forever, while traffic flows. **This is the identical rendered symptom**, and it
is a *different defect* from the two already fixed: no value of
`delay_is_a_measurement` can reach it, because there is no delay value to
classify.

### Why this is the most likely live cause, and what is not proven

`probe_egress` tests the **group** (`tier::GROUP_NAME`, `"Locus Auto"`), and the
group is `type: select` whose default is its first member. On a UDP-enabled tier
that first member is `Locus-UoT`; otherwise it is `Locus`. mihomo's group delay
tests the group's **currently selected** member:

- a slow-but-working selected member → the `> timeout` case the classifier now
  tolerates (already fixed);
- a selected member that cannot complete the test at all — while the student's
  actual traffic is flowing over a **different** rule or member — → the 504 above,
  which nothing tolerates.

The reporter's evidence (both traffic bars moving, `whatsmyip` showing the exit
node) proves traffic flows over `MATCH,Locus Auto`-style routing, but it does
**not** prove the group's *selected* member is testable by mihomo's delay probe.
That gap is exactly where a permanent amber button can hide.

**Not verified:** which of the two above is happening on the reporter's machine.
It needs `locus.log` plus one `curl` to the Core's own controller while connected:

```sh
# while the button says connecting — the group's selected member
curl -s http://127.0.0.1:<controller>/proxies/Locus%20Auto | jq .now
# the probe's own question, to the group
curl -s "http://127.0.0.1:<controller>/group/Locus%20Auto/delay?url=$(python3 -c 'import urllib.parse;print(urllib.parse.quote("http://cp.cloudflare.com/generate_204",safe=""))')&timeout=5"
```

A `504` on the second line with a working tunnel is this defect confirmed. The
controller port and secret are in the client's config.

### The fix, and what it is not

**Fixed here:** the stale pin. The test now restates the *shipped* rule, and the
all-dead-group contract is pinned by
`a_group_whose_members_all_fail_returns_no_delay_map` — so the next person
changing the classifier finds out from a red test rather than a student.

**Not yet fixed:** the probe still judges the tunnel by the group's selected
member. Making `Readiness` honest for that case needs a decision, not a patch —
the options are to probe every member, to fall back to the member the client's
traffic actually uses, or to treat a 504 as *inconclusive* rather than
`NoEgress` so it cannot override a `Ready` latch. Each has a cost in probe
latency or in false positives, and the wrong one recreates the
"connected with no internet" lie this file has already paid for twice. Recorded
in `STILL-OPEN.md` rather than guessed at.

---

## THE CODE WAS "ALREADY IN USE ON ANOTHER DEVICE" AFTER A REINSTALL, AND AUTO-SIGN-IN NEVER FIRED (2026-10-02)

| Severity | 🔴 A paying student was told their own code belonged to someone else |
|----------|----------------------------------------------------------------------|
| **Reported as** | *"the code I used is recognised as 'already in use on another device' right after uninstalling and updating, despite me binding the code just beforehand — so both the code binding and the auto-sign-in are broken"* |
| **File:** | `server/pb_hooks/activation.pb.js`, `server/pb_hooks/device_recognise.pb.js`, `client/src-tauri/src/locus/activation.rs` |
| **Status:** | Root cause located. Fix needs a `setup.sh` re-run to reach the host, and is **not** applied here. |

### One defect produces both symptoms

The two complaints are one missing write. **`/api/activate` never creates a row
in `device_identities`.** Grepping the hook set is conclusive:

```console
$ grep -rln device_identities server/pb_hooks/
server/pb_hooks/heartbeat.pb.js
server/pb_hooks/device_recognise.pb.js
```

`activation.pb.js` is not in that list. It binds the code — it writes
`bound_fingerprint`, `activated_at`, the binding index — and then **stops**. The
only two hooks that touch `device_identities` only ever *read* it
(`findFirstRecordByFilter`), except for clearing a dead `code` pointer. Nothing
in the repository ever **creates** an identity row.

So:

- **Auto-sign-in cannot work for anyone.** Recognition (`/api/device-recognise`)
  looks up `device_identities` by `verifier`; with no row for the device it
  returns the uniform `unknown`, and the client shows the code prompt. The
  feature has never had a wired write path. It is not "broken after an update" —
  it was never enabled.
- **The binding is a fingerprint, not an identity.** Because activation records
  only `bound_fingerprint`, whether a reinstall is recognised as "the same
  device" depends entirely on whether `device::fingerprint()` re-derives the same
  value — not on the durable identity at all. The 2026-10-01 identity work made
  `identity.rs` durable and reused-verbatim, but activation still asks for the
  **fingerprint** (`cmd/locus.rs::locus_activate` → `device::fingerprint()`), and
  `/api/activate` reads only `data.fingerprint`. The durable store that was built
  to survive exactly this is not consulted on the path that decides it.

Then the 403: `boundFp !== incomingFp` fires, and the student is told their code
is in use elsewhere when the truth is that this machine presented a different
value.

### Why it presents right after uninstall/update, and why that is a red herring

Uninstalling can delete the app-config store while the machine store survives (or
the reverse, if the machine store was never writable and the identity fell back
to app config — reported as `Store::AppFallback`, which *says* it does not survive
a reinstall). Either way the device re-derives a *different* fingerprint and
becomes a stranger to its own code. The timing is a coincidence of what the
installer removes, which is why this reads as "the update broke my code".

### The fix

**Applied 2026-10-02.** Activation now registers the identity it already has:

- `activation.pb.js` gained `registerIdentity(verifier, device_id, store, code)`,
  called on **both** success paths — the first-activation bind and the same-device
  re-activation (the branch a repair lands on). Idempotent by `verifier`, which is
  `UNIQUE`, so a renewal updates the row instead of throwing on a duplicate
  insert. Best-effort: a registration failure is swallowed, because the
  entitlement is already bound and saved, and trading a working code for a clean
  database is the wrong direction.
- `CodeRequest` gained `verifier`, `device_id` and `store`, **omitted when empty**
  so an older client's body is byte-identical to today's and an older hub ignores
  unknown fields.
- `locus_activate` resolves the durable identity and passes it through the new
  `activate_with_identity`.

The verifier is `sha256(secret)` — never the secret — the same digest discipline
`/api/device-recognise` already relies on, so a database read yields nothing
replayable.

**Guards, each shown failing** (in `activation_contract.rs`, which reads the hook
source because goja cannot be imported):

- `activation_registers_the_device_identity` — asserts the **call sites**, not the
  symbol, counting `registerIdentity(` occurrences. Pinning the symbol is the
  mistake `check-consistency.sh` §15 made for `is_installer_payload`: matching the
  definition passes with every call removed. Removing both calls → red.
- `the_activation_request_carries_the_identity` — field presence and the
  omit-when-empty attribute. Deleting `verifier` from `CodeRequest` fails at
  **compile time** in the two call sites, which is stronger than the test.
- `the_activation_command_passes_the_resolved_identity` — reverting
  `locus_activate` to the plain `activate()` compiles and passes every other
  check while registering nobody. That mutation → red.

### What must happen for this to take effect

**The hook is inert until `setup.sh` re-runs on the host** — `pb_hooks/` is
deployed from `/root/server/`, not from this repo. A device already bound will
register on its next activation or recognition attempt; a device that never calls
`/api/activate` again still has no row until then. **Unverified:** that this
closes it end to end. It needs a device, a code, and a reinstall, and it has not
been exercised against the live hub from this environment.

---

## "THE UPDATE TO 3.2.18 IS NOT AN INSTALLER" — THE CLIENT REFUSED A GENUINE INSTALLER (2026-10-02)

| Severity | 🔴 No Windows client on 3.2.17 could update, and the error blamed the payload |
|----------|--------------------------------------------------------------------------------|
| **Reported as** | *"On 3.2.17 attempting to update to 3.2.18, it gives the error message 'the update to 3.2.18 is not an installer (58631194 bytes); refusing to execute it'"* |
| **File:** | `client/src-tauri/src/locus/update/install.rs` (`is_installer_payload`), `server/scripts/fetch-release.py` (`_is_nsis_installer`) |
| **Fixed in:** | the 3.2.18 uncommitted work; ships to *other* clients from 3.2.19 |

### The twenty-word version

The guard looked for `b"NullsoftInstaller"`, which no real NSIS installer
contains; the hub was serving the correct installer and the client refused it.

### What was actually happening

The byte count in the error is the whole diagnosis, because it identifies the
artifact. Reproduced against the live hub:

```sh
curl -s 'https://networkingguides.duckdns.org/api/update?version=3.2.17&platform=windows'
# url: .../updates/3.2.18/installer-Locus_3.2.18_x64-setup.exe
# sha256: 3912658f…fc514
```

Downloaded, that file is **58,631,194 bytes** — exactly the number in the error —
with SHA-256 matching the manifest, an `MZ` header, and the NSIS firstheader
(`\xef\xbe\xad\xde` + `NullsoftInst`) at byte **68100**. It is a real installer.
The client refused it anyway, and the message told the student the payload was at
fault.

The fault was the constant. `is_installer_payload` accepted a PE only if it
contained `b"NullsoftInstaller"`:

- **No real installer contains that string.** The header is the 4-byte magic
  `0xDEADBEEF`, then the 12-byte ASCII `NullsoftInst`, then a 4-byte flags word —
  so `NullsoftInst` is followed by flags, never by `aller`. Measured on this
  artifact: `NullsoftInstaller` appears **nowhere in the file**.
- **The raw `locus-windows-amd64.exe` *does* contain it** — a Tauri app embeds an
  NSIS uninstaller stub — at offset ~36,700,169. It was excluded only because
  that is past the 4 MiB scan window: luck, not design.

So the constant was wrong in both directions at once, and the 4 MiB bound was
concealing half of it.

### The fix

Require the magic *immediately before* the 12-byte string, on both sides:

```rust
const NSIS_SIGNATURE: &[u8] = b"\xef\xbe\xad\xdeNullsoftInst";
```

Verified against the real v3.2.18 asset (accept) and the documented raw-app shape
(refuse). `fetch-release.py` carries the identical constant and the same 4 MiB
bound, so the hub and the client agree on the bytes.

### Why the tests did not catch it

They fabricated the marker they were checking:

```rust
bytes.extend_from_slice(b"NullsoftInstaller");
assert!(is_installer_payload(&bytes));   // self-referential
```

The fixture emitted the same string the constant held, so the pair moved together
and the test could never fail. The negative fixture was no better —
`bare_application()` had no `Nullsoft*` bytes at all, so it could not distinguish
"refused because it is not an installer" from "refused because it is featureless".
The rewrite emits the real firstheader bytes transcribed from a genuine installer
and gives the raw-app fixture the real `NullsoftInstaller` substring. **With the
old constant restored, the new test fails** — that is the proof it can fail.

### A guard that could not fail, found while adding this one

`check-consistency.sh` §15 verified the client half with:

```sh
grep -q 'is_installer_payload' "$INSTALL_GUARD"
```

That grep matches the *definition* and fifteen *unit tests*, so deleting the call
from `ReadyInstall::install` — the one line that protects a student's machine —
left eighteen matches and the check still printed OK. Demonstrated: rewriting the
call as `if false {` passed §15. §15 now anchors to the negated call over the
downloaded bytes (`if !is_installer_payload(&bytes)`), and the constant is checked
for agreement across the two files in both directions. All three mutations
(client constant reverted; hub constant reverted; call site removed) were observed
failing before the check was kept.

### Claim class

- **Verified:** the hub serves an NSIS installer at the advertised URL for a
  Windows 3.2.17 client (downloaded, hashed, firstheader located, 2026-10-02);
  the pre-fix constant refuses those exact bytes; the post-fix constant accepts
  them and still refuses the raw-app shape; the fixed guard fires on all three
  mutations.
- **Not verified:** that a Windows client on 3.2.17 successfully installs 3.2.18
  after the fix — **3.2.17 cannot receive this fix**, because the fix lives in
  the payload it refuses. Recovery is a manual install (below).
- **Structural argument:** that the raw `locus-windows-amd64.exe` is refused by
  the *post-fix* constant. The artifact is no longer published (404), so the
  fixture is reconstructed from the documented offset rather than measured.

### Recovery for the machine that reported it

3.2.17 cannot bootstrap itself past this. On the affected Windows machine:

1. Download `https://networkingguides.duckdns.org/updates/3.2.18/installer-Locus_3.2.18_x64-setup.exe`
   in a browser and run it by hand (it is the same file the updater fetched).
2. Confirm the app reports **3.2.18**.
3. The in-app updater works from 3.2.18 onward — 3.2.19 is the first release the
   fixed guard can install.

Do **not** seed the hub's `windows` entry for 3.2.17 with the raw
`locus-windows-amd64.exe` to force a same-version reinstall: that would hand the
plugin a PE to `ShellExecute`, which is the *other* failure this guard exists to
prevent (see the 2026-09-30 and 2026-10-01 entries).

---

## RECOVERY FROM A LOST SERVICE OWNER RESET THE WRONG PLATFORM'S PROXY, AND DID NOTHING ON THE RIGHT ONE (2026-10-01)

*Client-side fix; ships in the next version tagged. Not a hub change.*

Found while tracing the service-recovery path, not from a report. It is the
`DEBUGGING-METHOD.md` §1.3 shape again — a rule that reads as a deliberate
decision, is covered by a test that asserts the decision, and is wrong on both
platforms at once.

### The defect, in one line

`service.rs::owner_recovery_policy` decided whether owner recovery may clear the
machine-wide system proxy with `reset_system_proxy: !is_macos` — a hard `false`
on macOS and a hard `true` everywhere else. It is the wrong answer on both sides,
and the macOS setting was masked by an ordering bug that made the whole branch
dead.

### Why macOS was already inert, and why that was not luck

`recover_after_owner_loss_while_locked` performs the clear through the Service:

```text
proxy_control::clear()
  -> clear_inner() -> ProxyBackendRoute::Service
  -> service::set_system_proxy_by_service(&Disabled)
  -> active_service_session()?          // <- needs the owner session
```

and it calls `clear_active_service_session()` **before** it gets there, three
lines above. The session is gone, so the clear returns
`"service owner session is not active"` and the retry loop logs three failures.
On macOS the branch could not work; `!is_macos` is what kept it from being
*reached* with anything to do.

### What it did on Windows and Linux

`reset_system_proxy` was `true`, so recovery *ran* the clear — attempting to
switch off the machine-wide proxy through a service we had been told we no longer
own. That is the second half of the same ordering bug: on the platforms where the
clear could execute, the reason it was running was one where it must not.

Recovery is reached for three reasons, and the clear is wrong for all three:

| Reason | Why clearing is wrong |
|---|---|
| `Displaced` | Another instance owns the Service now. We do not hold the session, and if we did, we would be switching off the *new* owner's proxy — the student's live tunnel. |
| `SameOwnerFailure` | Still ours, but the Core died. `Core::stop` / `Handle::restart_core` already clear on that path, so recovery can only undo a restart's re-apply. |
| `TransportFailure` | The Service is unreachable, so the clear fails anyway. |

### The fix

`reset_system_proxy` is now `false` for every reason on every platform. Recovery
keeps the two things it can honestly do with local state — `stop_guard()` and
dropping core readiness — and leaves the machine-wide proxy to the Core's own
stop/restart path, where the owner session is still valid. The `reason` and
`is_macos` parameters are retained so the decision stays visible at the call site
and a future platform-specific rule has somewhere to go; the answer simply no
longer depends on them.

### The guard, and the evidence it can fail

The existing test `macos_recovery_never_resets_machine_wide_proxy` **pinned the
bug**: it asserted `!reset_system_proxy` for macOS *and*
`reset_system_proxy` for everything else, so the Windows behaviour was written
down as intended. A test that encodes the defect is worse than no test, because
it converts the defect into a requirement.

It is replaced by
`recovery_never_resets_the_machine_wide_proxy`, which asserts the same rule for
all three reasons **and both OSes**, so a fix applied to only the platform the
author ran cannot pass.

**Demonstrated failing against the pre-fix rule** (the new test kept, the
production arm restored to `!is_macos`):

```text
test core::service::tests::recovery_never_resets_the_machine_wide_proxy ... FAILED
Displaced (macos=false) must not clear the system proxy
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 553 filtered out
```

### Verified / not verified

**Verified locally:** the new test fails against the pre-fix rule and passes
after it; `cargo test --all-targets` — 554 lib tests plus the integration suites,
0 failed.

**Not verified, and stated as such:** that the Windows/Linux recovery path was
ever *observed* switching off a live proxy. The argument above is structural —
from the call order and the session guard in the code — and is the reason the
change is safe rather than the reason it was urgent. `STILL-OPEN.md` records it.

---

## DEVICE RECOGNITION WORKED, AND THE STUDENT WAS LEFT ON THE CODE PROMPT FOR FIVE MINUTES (2026-10-01)

*Client-side fix; ships in the next version tagged. Not a hub change.*

> **This closes the half of `039bb41` ("device recognition and code-free
> re-activation") that was never wired up.** The Rust side was complete and
> correct; the frontend fetched its answer and threw it away.

### The two defects, one of which no check could see

**1. The recognition result never reached the UI.** `main.tsx` held it in state
and never read it, so `ActivationScreen` had no idea whether the hub had
recognised the device. The only visible symptom was an eslint error
(`State variable 'recognised' is defined but never used`) — which is the kind of
finding that gets silenced with an underscore rather than investigated. The
feature was inert on every install.

**2. Nothing re-read the status after a successful recognition.** This is the one
that took tracing rather than linting, and it is the worse of the two:

```text
locus_recognise() -> hub recognises the device
  -> Rust STORES the entitlement and applies the tier   (cmd/locus.rs)
  -> returns RecognitionResult::Recognised { tier, durable }
  -> ...and the UI does not re-read locus_status
  -> `activated` stays false until the NEXT poll
  -> ENTITLEMENT_POLL_MS is 5 minutes
```

So the fix for defect 1, on its own, would have produced a **worse** screen than
the bug: a returning student told *"this device is already registered, no code is
needed"* while staring at a code prompt that would not go away for five minutes.
The entitlement was already live on disk the whole time — the gate simply had no
reason to look again.

**The lesson, which generalises past this feature:** a command that mutates state
the rest of the app polls must say so. `locus_recognise` changes `activated` as a
side effect and its own return value does not carry that change, so the caller is
the only place the two can be joined. Nothing in the type system or the linter
flagged it, because both halves are individually correct.

### The fixes

1. `ActivationScreen` takes an optional `recognition` prop; `main.tsx` passes the
   result through.
2. `main.tsx` gained an `entitlementRestored` counter, bumped when recognition
   returns `recognised`, and `entitlementRestored` is a dependency of the status
   effect — so the gate re-reads **immediately** instead of waiting out the poll.
   A counter rather than a boolean because re-recognition should re-trigger, and a
   boolean would latch and fire once.

### The decisions, and why

**The mapping from result to sentence is a pure function.**
`src/pages/recognition-notice.ts`, following `connect-notice.ts` and `phase.ts`.
It decides whether a student is told something *true* about their own account,
which is not a rule to bury in JSX.

**`unavailable` is never rendered as a refusal.** `cmd/locus.rs` is explicit that
`RecognitionResult` is deliberately not a `bool` because "we could not reach the
hub" and "the hub does not know you" are different claims — telling a student
their device is not recognised during an outage is a false statement about their
entitlement. The test that pins this (`reports an unreachable hub as a caution,
not as a refusal`) was demonstrated failing by collapsing `unavailable` into
silence.

**`unknownDevice` renders nothing at all.** The code prompt is already the correct
message; adding "we do not recognise you" would be a claim that changes nothing
the student can do.

**`durable: false` shows the warning INSTEAD of the "restoring" message,** not
alongside it. Two notices would say "we remember you" and "we might not next
time" at once, which reads as a contradiction rather than a caveat. The Rust type
carries `durable` specifically so the UI cannot promise durability an install does
not have.

**The sentences are literals, not i18n keys.** `activation.tsx` uses **no i18n at
all** — "Enter the activation code from your card." and "Secure school VPN" are
both fixed English — so a lone translated string would be the odd one out. The
pure function still returns i18n *keys*, so translating the screen later is a
change to one lookup table.

### Verified / not verified

**Verified locally:** 87/87 vitest (12 files); `tsc --noEmit` clean;
`eslint src --max-warnings=0` clean — **the CI lint gate is green again**;
`check-consistency.sh` exits 0.

**Demonstrated failing:** the `unavailable` guard, by returning `null` for that
case — the intended regression, and only that test went red.

**Not verified:** the recognition path has never been exercised against the live
hub from this environment. That the entitlement is restored and the gate then
clears is a **structural argument** from the code (`cmd/locus.rs` stores it;
`locus_status` reads it), not an observed behaviour. Confirming it needs a device
with a durable identity, a hub that knows it, and a reinstall — see
`STILL-OPEN.md`.

---

*Client-side only. Not a defect fix: a feature, plus the guard gap it exposed.*

### The gap this exposed, which matters more than the feature

`client/src/**/*.test.ts` and `client/tests/` held **69 passing tests, and no job
in `.github/workflows/client.yml` invoked them.** `verify` ran
`check-consistency.sh`, the tag check, `pnpm run web:build` (which includes
`tsc --noEmit`) and `pnpm run lint` — and stopped. `pnpm test` was never called,
in any job, on any trigger.

That is the same failure shape as the Rust suite before 2026-09-29 (it existed
and passed locally while a tag could build, sign and publish with the suite red
— 530 cases then, 554 at the last count on 2026-10-01, which is why no count is
quoted as current) and the same shape as the two-sided filename contract in
`AGENTS.md`. A test suite that nothing runs is not a guard; it is a comment with a
`describe` around it.

**Fixed in the same change:** a `Frontend tests` step (`pnpm test`) in the
`verify` job. It is placed there rather than in a build job because the whole
suite finishes in under three seconds, so it costs a 15-minute gate essentially
nothing. `pnpm test` is `vitest run` and does not watch, so it cannot hang the
job waiting for input.

### The feature: six named themes, as a layer that can be removed

A student can now pick one of six appearances from a dropdown on **Account**:
`default-dark`, `default-light`, `midnight` (OLED), `paper` (warm light),
`high-contrast`, and `forest` (decorated, larger radii). The design record is
[`THEMES.md`](THEMES.md); this entry records only what a reader of the log needs.

The three properties that were the whole design constraint, each with its guard:

1. **Additive.** `verge.theme_id` is a new `Option<String>`; when it is unset —
   every existing install — the hook resolves through `theme_mode` exactly as
   before. `default-dark` and `default-light` are built *from* `LOCUS_COLORS` and
   `LOCUS_LIGHT` rather than restating their hex values, so the shipped appearance
   is a member of the registry. Pinned by the byte-identity test, because relaxing
   it silently converts this into a visual change for every existing install.
2. **`setting.X || dt.X` is unchanged.** The legacy custom-colour fields keep
   their precedence and still win. Only *which base* they fall back to changed —
   from a fixed light/dark pair to the resolved theme. No stored value is
   reinterpreted, so there is no migration.
3. **Shape and decoration cannot reach a component.** Radii travel as CSS
   variables (`cardSx` reads `var(--card-radius, 12px)`), and decoration is one
   **named preset**, never injected CSS. The registry test asserts no preset
   contains `{`, `}` or `url(`, which is what stops a preset escaping its
   `[data-theme-skin]` scope.

### What went wrong while building it, for the record

**`themeDefaultFor` was briefly written without `font_family`.** The hook reads
`dt.font_family` in four places, so `tsc` caught it immediately — noted only
because it shows the projection function is load-bearing for behaviour, not just
for colour.

**`pnpm test` was assumed to be wired into CI.** It was not. The claim "the tests
pass" was true and irrelevant, which is `DEBUGGING-METHOD.md` §1.3 exactly.

**`knip` flagged `ThemePalette` as an unused export.** It is not part of any
pipeline (there is no `knip` step in CI), but it was un-exported rather than left:
a second name for the theme shape invites a consumer to type against it instead of
against the registry, which is the thing that is actually guarded.

### The guards, and the evidence they can fail

Five registry guards in `client/tests/theme-colors.test.ts`, plus one in
`check-consistency.sh` §9. **Every one was run against deliberately broken code
before being kept** (`DEBUGGING-METHOD.md` §3):

| Guard | Broken deliberately by | Observed failure |
|---|---|---|
| Contrast floors | `paper.textSecondary` → `#B9AE9C` | `paper: textSecondary/surface 2.19` |
| Mode matches palette | `midnight` set to `mode: 'light'` | `[ Array(1) ] to deeply equal []` |
| Structural relationships | `forest.surface` = its background | `forest: surface/background 1.000` |
| Preset cannot escape scope | preset containing `body { … }` and `url(…)` | `expected '…' not to contain '{'` |
| Unknown id falls back | `resolveTheme` returning `THEMES[id]` directly | `Cannot read properties of undefined` |
| Registry vs `state.toml` | state trimmed to 3 themes / 7th theme added to registry only | `BAD client.theme_ids`, exit 1, **both directions** |

The last one is two-sided on purpose: it catches the registry drifting from
`state.toml` *and* `state.toml` drifting from the registry, which is the
`AGENTS.md` rule that when two things must agree, assert the agreement.

### Verified / not verified

**Verified locally:** 79/79 vitest; `tsc --noEmit` clean; eslint clean on all five
touched files; `cargo check --all-targets` clean; `pnpm run web:build` succeeds;
`knip` clean for the new files; `bash server/scripts/check-consistency.sh` exits 0
with `client.theme_ids … agrees`.

**Not verified:** that any theme *looks* good; that `forest-glow` renders as
intended on all four platforms; that the §5 cold-start transition (every theme
starts as the default dark, then resolves) is imperceptible in practice. These are
visual checks and are stated as unverified, not as passing.

**Pre-existing and untouched:** `eslint src/main.tsx` reports an unused
`recognised` binding, committed in `039bb41` and explicitly left alone by
`09a985f`. It is not from this change, but the `pnpm run lint` step in `verify`
fails on it independently, so the CI gate is red until someone decides about it.

### Open follow-ups

See [`THEMES.md`](THEMES.md) §10. The one needing a **product decision, not a code
change**: there is no UI control that clears `theme_id`, so once a theme is chosen
`theme_mode` — and with it "follow the system" — is unreachable from the UI.

--- — "CONNECTING" ON A WORKING TUNNEL, STILL (2026-10-01)

*Client-side fix; ships in the next version tagged. Not a hub change.*

> **This corrects the entry further down, "THE BUTTON SAT ON 'CONNECTING' WHILE
> THE TUNNEL WAS CARRYING TRAFFIC (2026-10-01)".** That entry's diagnosis was
> right and its retry/caching work still stands, but **the classifier it
> introduced is the cause of this recurrence.** Read this one first; then read
> that one for the parts that are still true.

The same report came back after v3.2.14: the traffic panel showed bytes moving and
`whatsmyip` showed the exit node, while the button pulsed amber on **connecting**
indefinitely. The previous fix did not remove it, because the previous fix
introduced it.

### The defect, in one line

`probe.rs::delay_is_a_measurement` rejected a delay **at or above the probe's own
per-attempt budget**:

```rust
// wrong — the budget decides what counts as egress
let timeout_ms = timeout_secs.saturating_mul(1000);
delay > 0 && delay < timeout_ms && delay <= IMPLAUSIBLE_DELAY
```

With `EGRESS_ATTEMPT_TIMEOUT = 5`, the acceptance window was `0 < delay < 5000`.
mihomo reports a member that **hit its budget** as the timeout value itself
(`5000`), so the probe read *"this was slow"* as *"this is dead"* — and because the
rule was derived from the same constant the engine was given, raising the budget
from 3 s to 5 s **did not widen tolerance, it moved the rejection threshold**. The
fix for the first report made the check more likely to fail on a slow link, which
is precisely the link it was meant to tolerate.

The comment above `EGRESS_ATTEMPT_TIMEOUT` had already predicted the outcome while
the code did the opposite:

> on a slow school link the first packet can take several seconds to get through,
> and calling that "no egress" is the **single most likely way to strand a working
> tunnel on "connecting" forever**.

### Why it presents as "stuck", not as "failed"

The chain, unchanged by the previous fix:

```text
delay map has no value below 5000
  -> egress_attempt_once() -> false, twice, inside the 12 s deadline
  -> probe_egress() -> EgressOutcome::Failing
  -> decide(ProbeOutcome::Serving, Failing, latch) -> Readiness::NoEgress
  -> locus_status.ready = false   (core_up = true)
  -> phaseFromStatus: connected && !ready -> 'connecting'
```

`Readiness::NoEgress` and "the tunnel is not up" **render identically** —
`phaseFromStatus` derives solely from `ready`, and `NoEgress` is not `ready`. So
"stuck on connecting" cannot be distinguished from "not connected" on screen. That
ambiguity is why this took two rounds.

### The fix

`delay_is_a_measurement` now rejects only the two values that are unambiguously
*not* a measurement:

```rust
// correct — only the sentinels are non-measurements
delay > 0 && delay <= IMPLAUSIBLE_DELAY
```

The engine dialled the target and the request left the machine; the only thing that
ran out was a clock **we chose**. That is evidence of egress, not of failure. The
`0` sentinel remains the one value read as "no traffic moved", and `> 1e5` still
guards against an error sentinel being read as a latency.

`timeout_secs` is kept in the signature (now unused) so the coupling is visible at
the call site rather than hidden — the same budget is still passed to the engine.

### The deliberate divergence from `classifyDelay`

The previous entry's stated goal was "one classifier for delay values" mirroring
`client/src/utils/delay.ts`. **That goal was wrong, and the divergence is now
intentional and documented in both files.**

| | `classifyDelay` (frontend) | `delay_is_a_measurement` (probe) |
|---|---|---|
| Question | *how fast is this node?* | *did the tunnel carry a packet at all?* |
| Consumer | a latency list | the readiness gate |
| `5000` means | `'timeout'` — show a timeout badge | **egress observed** — slow is still working |
| Sentinels agreed | `0`, `> 1e5` | `0`, `> 1e5` |

The two must agree on the **sentinel set**; they must not agree on what a slow node
means for their own callers. Conflating them is the bug: the probe adopted a
display classifier's verdict and inherited its "slow = bad" bias.

### The test that guarded the bug

`a_timeout_sentinel_is_not_a_measurement` asserted the defective behaviour, and its
own comment described the fix backwards — *"the specific disagreement with
`classifyDelay` this function removes."* A test pinning the wrong contract made the
defect look deliberate. It is replaced by `a_delay_at_the_budget_is_still_egress`,
which asserts `5000`, `6000` and `7500` all count as egress.

**Verified to fail against the old rule** (per `DEBUGGING-METHOD.md` §"a check that
cannot fail reads like one that passed"):

```text
thread ... panicked at probe.rs:583:
assertion failed: delay_is_a_measurement(5000, 5)
```

### What this does NOT fix

- **A group where *every* member times out** still returns mihomo's
  `504 "get delay: all proxies timeout"`, which the plugin raises as `Err`, which
  `egress_attempt_once` collapses to `false`. Reachable on Strike, where the group
  holds both `Locus-UoT` and `Locus` and the probe tests the group's *selected*
  member — not necessarily the one carrying the student's traffic. **Still open.**
- **The integration test's blind spot.** `tests/egress_probe_engine.rs` SKIPs when
  the runner has no network, and its positive case uses a `direct` outbound to a
  public host. It has never exercised *slow-but-working* through a real tunnel —
  the case that broke. The blind spot is unchanged; see `STILL-OPEN.md`.

For the full diagnostic walk-through, see
[`EGRESS-READINESS.md`](EGRESS-READINESS.md).

---

## DEVICE IDENTITY WAS NOT DURABLE — CODES "STOPPED BEING RECOGNISED", AND A REINSTALL LOST THEM (2026-10-01)

*Ships in the client's 3.2.x line; the hub half is inert until `setup.sh` re-runs.*

Two symptoms reported as separate problems, both **one defect**:

* a code bound to a device stops being recognised as bound to it (noticed around
  updates, but not caused by one);
* an uninstall/reinstall loses the code, and a student who discarded the paper
  card has no self-service recovery.

### `bound_fingerprint` was right; the identifier was not

The hub's `boundFp !== incomingFp` guard fired correctly. The device presented a
**different** value, because `device.rs::compute()` derived the fingerprint from
hardware on every launch and **fell back to a random value** when the hardware
told it nothing:

```rust
match platform_sources() {
    Some(fingerprint) => fingerprint,
    None => random_fingerprint(),   // <- persisted ONLY in the app's own config
}
```

That random value lived in the app config — the file an uninstall deletes — so
a reinstall could not re-derive it and generated a new one. The device became a
stranger to its own code. The `OnceLock` cache made it stable *within* a process,
which is why it presented as "works, then randomly does not".

### The fix

* `locus/identity.rs` — a durable identity split into three parts that must not
  be conflated: a **device id** (a non-secret name), a **secret** (the
  credential), and the hub's `sha256(secret)` verifier. A stored identity is
  reused **verbatim**, so neither a hardware change nor a reinstall re-derives it.
* Persisted machine-scoped (`/var/lib/locus`, `/Library/Application Support/Locus`,
  `%PROGRAMDATA%\Locus`) so it survives an uninstall, with an app-config fallback
  that **reports it does not** — so the UI cannot over-promise.
* `resolve()` is pure over injected sources, so the reinstall-vs-wipe distinction
  is provable **without** real Windows/macOS hardware. That is what unblocked a
  fix that had been deliberately deferred for exactly that reason (see
  `business/redesign/implementation/05-not-yet-true.md` §5.2).

### Two things learned the hard way while building it

* **The hook runtime HAS crypto.** A hand-rolled mixing function was written for
  the token hash on the assumption that goja had none. `$security.sha256` /
  `randomString` exist (confirmed by the 2026-09-30 global probe above). Use the
  real primitive; home-made crypto is this file's most repeated lesson.
* **A recursive async loop start does not compile.** `runtime::start` was made
  async to read the token, then called from inside the heartbeat's own outcome
  callback — the spawned future was non-`Send` and the compiler refused it. Split
  into `start` (async, resolves the credential) and `start_with` (sync, spawns
  the loop).

Detail, wire contracts and debugging steps:
[`DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md).

---

## A GUARD THAT WAS CITED IN SIX DOCUMENTS DID NOT EXIST (2026-10-01)

`activation_contract_test` was named as pinning the 403/409 refusal contract in
**six live documents and two code comments**. No such test was in the tree.

The contract it claimed to protect is genuinely fragile: the client classifies an
activation refusal by status first, then by whether a **403's message contains the
substring `suspended`**. A hub-side reword would make every deployed client report
a suspension as "your code is bound to another device" and send the student to a
middleman with the wrong question — and **nothing in the build would fail**.

It now exists: `client/src-tauri/tests/activation_contract.rs`, reading the hook
source and asserting the literal status codes and messages the classifier is
written against. Verified by mutating the hook's suspension wording and watching
it fail, then restoring the file byte-identical.

**The lesson is the meta-guard it now carries.** A guard described in prose but
absent is worse than no guard: it stops anyone from checking. The claim was
repeated until it read as fact, and the planning pass for the work above trusted
it. `the_claimed_contract_test_exists` now makes the claim self-checking.

---

## THE WINDOWS INSTALLER WAS NEVER SIGNED: TWO DEFECTS IN ONE STEP (2026-10-01)

*CI-side fix. Found by cutting a release, not by reading the workflow.*

The entry below ("NO WINDOWS CLIENT COULD UPDATE") diagnosed the hub half of the
Windows outage. This is the **other half**, and it is upstream of it: CI was
never producing the signature the hub needed to publish.

It took two releases to surface, because each defect hid the other.

### Defect 1 — the step's condition could never be true

```yaml
- name: Sign the Windows installer
  if: matrix.os == 'windows-latest'     # the matrix pins os: windows-2022
```

The build matrix (`.github/workflows/client.yml`) pins runner images on purpose —
`ubuntu-24.04`, `windows-2022`, `macos-14` — because a `*-latest` label is a world
claim nobody re-checks, and this project already lost every Intel macOS run to the
retirement of `macos-13`. But this step's condition still tested the old
`windows-latest` name, so it **silently skipped on every run**.

The installer was therefore built, staged and uploaded **unsigned**, and the
failure was invisible: a skipped step is a green step. Nothing in CI could see it.

Every other platform gate in the job already used `matrix.label`
(`linux`/`windows`/`macos-intel`/`macos-arm`). This was the lone outlier. The rule
now: **gate on `matrix.label`, not `matrix.os`** — `label` is a name the file
chooses, while `os` is a runner image that gets bumped and silently detaches every
condition written against the old value.

### Defect 2 — the step globbed a name that does not exist yet

With the condition fixed the step ran for the first time, and failed:

```console
##[error]no installer-Locus_*_x64-setup.exe to sign in installer/
-rwxr-xr-x  runneradmin  ...  Locus_3.2.16_x64-setup.exe
```

The `installer-` prefix is added by the **release** job's "Prefix and stage the
installers" step. The sign step runs in the **build** job, where the file still
carries Tauri's raw name (`Locus_<v>_x64-setup.exe`). The glob could never match,
so the step exited 1 — and the cost was a ~30 minute four-platform build that ends
with no release.

The fix signs the file under its build-job name and lets the release job prefix
both the file and its `.sig` together. **The pairing is the point:** minisign signs
exact bytes and the `.sig` is a sibling filename, so a rename applied to one and
not the other produces a signature that verifies nothing.

### Why v3.2.14 published anyway, and v3.2.15 did not

`v3.2.14` reached the hub with an unsigned Windows installer because the manifest
step did not yet require the signature — so the hub published an entry every
Windows client refused. `v3.2.15` is where the manifest step's
`elif filename.startswith("locus-") or key == "windows"` clause began refusing it:

```console
$ gh run view … --log-failed
refusing to publish a partial release — missing: installer-Locus_3.2.15_x64-setup.exe.sig
```

That refusal is correct, and it is the guard working. It simply fires ~30 minutes
in, at the end of the expensive part, which is why it took two attempts to isolate
the two defects behind it.

### The guards, and the evidence they can fail

Two sections in `check-consistency.sh`, each demonstrated failing against the
pre-fix tree:

| Guard | Broken deliberately by | Observed failure |
|---|---|---|
| §19 — every `if: matrix.X == 'lit'` names a value the matrix produces | `matrix.os == 'windows-latest'` | `BAD …: condition tests matrix.os, which the matrix does not define` |
| §18 — the sign step's glob and the release job's prefix **agree** | glob → `installer/installer-Locus_*` | `BAD the sign step globs 'installer/installer-Locus_*_x64-setup.exe' in the BUILD job` |
| §18 — the step is gated on `matrix.label` | `if: matrix.os == 'windows-latest'` | `BAD the sign step is gated on matrix.os … , not matrix.label` |

§18 asserts the **agreement between the two jobs** rather than each half
(`AGENTS.md`, "when two things must agree, assert the agreement"): checking only
that the sign step has *a* glob, or only that the release job has *a* prefix,
passes while the pair disagrees — which is exactly the state that shipped.

### Verified / not verified

**Verified:** `v3.2.17` built all four platforms and released with
`installer-Locus_3.2.17_x64-setup.exe.sig` present; its `manifest.json` carries a
`signature` for the `windows` platform; the served installer's SHA-256 matches the
manifest (`da881fe0…`, 58,595,274 bytes, `MZ` header). `check-consistency.sh`
exits 0, and both new sections were shown failing before being kept.

**Not verified:** that a Windows client *installs* the resulting update. That needs
the hub to publish `3.2.17`, which needs the host — see below.

### Still blocked on the host

CI now produces a signed installer, but the hub cannot serve it until
`fetch-release.py` is redeployed (`hooks-sync.sh --fetch-service`) and the release
is re-published. The deployed fetcher still resolves the raw binary, and the host
was unreachable throughout this work. Until then the Windows path is fixed at the
supply end and not at the serving end —
`RECOVER-WINDOWS-UPDATE.md` remains the operator's guide.

---

## NO WINDOWS CLIENT COULD UPDATE: THE HUB SERVED THE RAW BINARY, NOT THE INSTALLER (2026-10-01)

A Windows client on `3.2.13` was offered `3.2.14`. The download reached **100%**
and then the app refused it:

```text
the update to 3.2.14 is not an installer (51741696 bytes); refusing to execute it
  — this build cannot install a raw executable, and neither can the platform installer
```

`51741696` is the exact size of `locus-windows-amd64.exe` — the **raw PE**, not
the NSIS setup. Reproduced against the live hub:

```console
$ curl -s "https://…/api/update?version=3.2.13&platform=windows"
{"version":"3.2.14","url":"…/updates/3.2.14/locus-windows-amd64.exe",
 "signature":"dW50cnVzdGVkIGNvbW1lbnQ6…","sha256":"50ef4bcb…"}
```

### The client was right; the hub was wrong

The refusal came from `is_installer_payload`, which is the **3.2.12 fix working
as designed**. It refuses a payload that is a PE without an NSIS overlay, because
`tauri_plugin_updater` accepts *any* PE as an NSIS installer and ShellExecutes
it. Had that guard not existed, `3.2.13` would have relaunched a copy of itself
outside `C:\Program Files\Locus\` — the same defect as the entry below.

So this is not a regression of the client fix. It is the **other half of the
contract** having been left behind.

### The defect: the hub's fetcher never followed CI

The 3.2.12 change taught CI to advertise the installer. It updated
`manifest.json` — verified on the real release:

```json
"windows": { "file": "installer-Locus_3.2.14_x64-setup.exe", "sha256": "519164…" }
```

Three sites were missed, all on the hub side:

| File | Stale value |
|---|---|
| `server/scripts/fetch-release.py` | `("windows", "locus-windows-amd64.exe", "download_windows")` |
| `server/scripts/publish-release.sh` | `"windows:locus-windows-amd64.exe:update_windows"` |
| `server/scripts/publish-release.sh` | `FILES = {… "windows": "locus-windows-amd64.exe" …}` |

`fetch-release.py` resolves assets from that hardcoded allowlist and **discards
`manifest.json`'s `file` field entirely**. So it staged the raw binary into the
Windows slot, hashed it, and paired it with the raw binary's own signature —
which is why the served signature reads `trusted comment: Locus_windows`. The
installer has no `.sig` at all on the release; it was never on this path.

### Why three guards passed on a release no Windows client could install

This is the part worth keeping. Every check was real, and every one was blind to
the actual disagreement:

- **`check-consistency.sh` §15** greps `.github/workflows/client.yml` for the
  manifest entry. The workflow was correct. It never looked at the hub.
- **`fetch-release.py`'s all-or-nothing check** asserts all four *platforms* are
  present. They were — all four were the wrong files.
- **The manifest hash cross-check** (in both scripts) hashed whichever file the
  **manifest** named. The manifest named the installer, the check hashed the
  installer, the hashes matched, and it printed `ok`. It agreed with itself by
  construction and never compared the name the hub would actually **serve**.
- **§1's platform check** only asserted the string `"windows"` appeared somewhere
  in the file. It was present before, during and after the bug.

A check that cannot tell a right answer from a wrong one reads exactly like a
check that passed.

### The fix

**Hub scripts.** `fetch-release.py` and `publish-release.sh` now resolve
`installer-Locus_<version>_x64-setup.exe` for the Windows slot, from a single
named template (`installer_name()` on the Python side, `WINDOWS_INSTALLER` on the
shell side) so the name has one definition per script. Both scripts now also
**compare the filename against `manifest.json`** and refuse to publish on a
mismatch — the check that was missing.

`fetch-release.py` additionally refuses a Windows PE with **no NSIS overlay**,
mirroring the client's `is_installer_payload`. Hub and client now apply the same
rule to the same bytes.

**CI.** The installer needs its **own** minisign signature. It is not
interchangeable with the raw binary's — minisign signs exact bytes — and the
plugin verifies one mandatorily with no bypass (`RemoteReleaseInner` refuses to
deserialize without a `signature` field; `install_inner` calls
`verify_signature()` unconditionally). A new `Sign the Windows installer` step
signs the staged NSIS setup, the manifest embeds that signature, and the
`Verify every artifact is signed` step now covers `installer-*` as well as
`locus-*` — previously it looped over `release/locus-*` only, so the one artifact
that was actually broken was the one unchecked.

**Guards, each shown to fail.**

- `check-consistency.sh` §1a pins the filename in **both** hub scripts and fails
  on the stale one. Run against the reintroduced pre-fix tree, it emits three
  `BAD` lines and exits non-zero; the old mention-only check passed on the same
  tree.
- `smoke-publish.sh` case **5b** publishes with a manifest that names a different
  file. It fails `reports the name mismatch` against the pre-fix cross-check and
  passes against the new one.
- The client's existing tests (`the_raw_windows_binary_is_not_an_installer`,
  `an_nsis_installer_is_an_installer`) already pin the client half — 17 passed.

### The hub deploy, and the evidence it took

The repo fix is inert until the host's copy is replaced, so the deploy is part of
the fix, not a follow-up. Recorded here because the *verification* is the part
worth copying.

**Before** — confirmed on the host, not inferred:

```console
$ ssh root@<host> 'sha256sum /root/server/scripts/fetch-release.py'
68f4aaf88b247c33428a2cdf379b98b69e9879493d3d0ef4cb454864f92da3b8
$ ssh root@<host> 'grep -c "installer-Locus" /root/server/scripts/fetch-release.py'   # 0
$ ssh root@<host> 'grep -c "locus-windows-amd64.exe" /root/server/scripts/fetch-release.py'  # 1
```

The deployed fetcher knew nothing about the installer and resolved the raw
binary. That is the defect, standing on the box.

**Deploy** — `hooks-sync.sh --fetch-service` (added in this change; no script
previously synced this file at all). The sequence, for reproducing by hand:

1. `scp` to `fetch-release.py.uploading` (never in place — the unit execs this file).
2. `python3 -m py_compile` the **uploaded** copy on the host, *before* swapping.
   An `active` service with a syntax error serves every request as a failure,
   which is the same silent-failure shape as the original bug.
3. Keep a timestamped `.bak`, then `mv` into place; `chown root:root`, `chmod 755`.
4. `rm -rf __pycache__` — a stale bytecode cache can mask the swap.
5. `systemctl restart locus-fetch`, then poll `/health` rather than sleeping fixed.

**After** — matched by hash and by behaviour:

```console
$ ssh root@<host> 'sha256sum /root/server/scripts/fetch-release.py'
35920d63eec98e815fe24c5817cec04b5ad40e6f735d5d936b03048be1ed8c96   # == repo
$ ssh root@<host> 'curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8091/health'
200
$ ssh root@<host> 'journalctl -u locus-fetch --since "-2 min" -p err -q'   # (empty)
```

and the loaded file resolves the right name — asked of the copy that runs, since
`grep` counts prose comments too:

```console
$ ssh root@<host> 'cd /root/server/scripts && python3 -c "…resolve_platform_names(\"3.2.14\")"'
   linux        -> locus-linux-amd64
   windows      -> installer-Locus_3.2.14_x64-setup.exe
   macos_intel  -> locus-darwin-amd64
   macos_arm    -> locus-darwin-arm64
```

**The service was restarted and verified; the fleet is still affected.** A deploy
does not change `update_config` — that happens at publish, and neither 3.2.14 nor
any existing release can be published (see below). The live row continued to
serve the raw `.exe` after the deploy, correctly.

### Bookkeeping the deploy required

- **The host's SSH key had changed.** `known_hosts` carried three entries whose
  fingerprints matched none of the keys the server now offers. Replaced with a
  fresh `ssh-keyscan` **after** comparing fingerprints and stating the ambiguity:
  a changed host key is indistinguishable from an interception at that layer.
  The stale-entry reading was accepted on two pieces of evidence (the box
  resolves to a private address, and it had been rebuilt), not waved through.
- **Access was borrowed, not assumed.** The box had password-only root SSH, so a
  short-lived key was generated, installed, used, and removed — then the removal
  **verified** (`ssh` refused, `authorized_keys` back to 0 bytes). The local key
  material was deleted. Leaving standing access behind is a worse defect than the
  one being fixed.
- **`authorized_keys` held only that temporary key.** Confirmed before emptying
  it, so no pre-existing access was destroyed.

Server-side halves do not reach the live host until `setup.sh` re-runs; that does
**not** apply here, because the fetcher was deployed directly and verified by
hash.

### What this says about the class

The 3.2.12 entry below ends by saying every check tests that a bundle *builds*,
and that none launches either artifact. This is that observation one level up:
every check tested that **one side** of a two-sided contract was right. CI and
the hub are separate programs, in separate languages, that must agree on a
filename — and nothing compared them. The fix is not a stronger assertion about
either side; it is a check that the two sides **agree**.

### What is NOT verified

**`CLAIMS.md` §5 A7 remains unverified.** A real Windows machine has still never
taken an advertised update and relaunched from the Start Menu. This change makes
the hub serve the right bytes with a signature that verifies; it does not
demonstrate an install. The end-to-end test needs Windows hardware, and the
release must be re-published for any client to see the fix — the currently-live
`update_config` row still names the raw binary, so **Windows clients remain
unable to update until that publish happens**.

---

## THE BUTTON SAT ON "CONNECTING" WHILE THE TUNNEL WAS CARRYING TRAFFIC (2026-10-01)

A connect reported **connecting** forever even though every packet was routed
through the tunnel — `whatsmyip` showed the exit node, browsing worked, and the
button still pulsed amber. The screen derives its state from one backend field,
`ready`, so "always connecting" means the backend was persistently returning
`connected: true, ready: false` — which is exactly `Readiness::NoEgress`, the state
where the Core is up but the through-tunnel egress proof failed.

This is the mirror image of the school-wifi fix (FIXES.md, "Connected on a machine
with no usable internet"): that one stopped a *false* `ready`; this one is a
*false* `not-ready`. Both come from the same egress check, and both are worth the
same discipline.

> **What was actually observed, and what was inferred.** The three defects below
> are all real and all fixed, and each *can* produce this symptom. What was **not**
> available to pin the runtime trigger was a live Locus hub and a real school
> tunnel: against a real mihomo sidecar the probe's rule and its call shape behave
> correctly (see "checked against a real engine" below). So the specific transient
> that fired on the reporter's machine is inferred, not proven — but a check that
> judges a tunnel on one 3 s round trip is the shape of thing that fails exactly
> this way, and it is now gone.

### What was wrong

The egress check judged the tunnel on **one** round trip, with a **3 s** budget,
and classified the result with `delays.values().any(|&delay| delay > 0)`:

- **One attempt, 3 s.** A cold proxy connection, a CDN blip, or a DNS answer that
  had to be re-queried fails once and succeeds immediately after. A single transient
  miss flipped a working tunnel back to "connecting", and a first request through a
  freshly-dialled tunnel can exceed 3 s on a slow link. The docs had already named
  this "the most likely false negative".
- **`> 0` disagreed with the app's own delay semantics.** mihomo reports a *failed*
  test and a *timed-out* test in the same numeric field a real measurement uses —
  `0` for a failure, and the timeout value itself for a timeout. The frontend's
  `classifyDelay` (`client/src/utils/delay.ts`) already treats `0`, `>= timeout` and
  `> 1e5` as non-measurements; the probe's bare `> 0` did not. Two implementations
  of one rule, free to drift — and the probe's half would accept a sentinel.
- **The poll storm multiplied the check.** Readiness is re-read every 750 ms while a
  connect settles (and every 250 ms by the connect loop), and each read ran its own
  full through-tunnel request. So a slow tunnel did not merely look slow: every
  tick was a fresh chance to catch a transient miss, and each read could be
  overtaken by a later, stale one.

### The fix

**`probe_egress` retries within a bounded deadline.** A per-attempt budget of **5 s**
stands in for the old 3 s; up to **two** attempts run inside a **12 s** overall
deadline, with a 500 ms gap so the retry does not race the first attempt's socket.
A genuinely dead tunnel is still reported promptly — the deadline is shared, not
per-attempt.

**One classifier for delay values.** New `delay_is_a_measurement(delay, timeout_secs)`
mirrors `classifyDelay`: `0`, the timeout value, and anything above `1e5` are not
measurements. Unit tests pin each sentinel, and were shown to fail against the old
`> 0` check before being kept.

**The check is coalesced and briefly cached.** `CoreManager::observe_egress`
serialises the through-tunnel probe behind a lock and reuses a completed result for
**1.5 s**, so the status timer and the connect loop collapse onto one round trip. The
cache is cleared on every `core_started`/`core_stopped`, so a fresh connect never
reads a pre-connect answer and a stopped Core never leaves a stale `Ready` behind.

**The frontend stops stacking status reads.** `use-connection.ts`'s poll is a
`setInterval`, which fires on the clock rather than after the previous read
finishes; a re-entrancy guard now keeps one `locusStatus` call in flight at a time,
so a slow, stale read cannot land after a newer good one and drag a connected
tunnel back to "connecting".

### The contract is now checked against a real engine

`cargo test --test egress_probe_engine` (new) starts the real sidecar with the
tier's own group name and pins three facts the probe depends on: a reachable member
yields `200` with a usable delay; an unreachable one yields a non-`200`; and the
`sentinel` values in the delay map are not measurements. It also recorded a
behavioural fact that had been assumed — mihomo's `direct` outbound reports `0` for
a *loopback* target, so the positive case uses the real egress URL and SKIPs when
the runner has no network. What is *still* unverified — a real Locus tunnel on a
real school network — is listed in `STILL-OPEN.md`.

---



> **This supersedes both earlier `ERR_FILE_NOT_FOUND` diagnoses for THIS report.**
> The first attributed it to a stale `start_page` (withdrawn); the second, the
> updater re-executing the client's own binary (real, and fixed in v3.2.12, but a
> different report — it needs an *update*, and this one was a fresh install). Read
> the distinctions before reaching for either.

A Windows client installed `installer-Locus_3.2.12_x64-setup.exe` **pulled
directly from the release page** and launched it. The window showed:

```text
File not found
It may have been moved, edited, or deleted.
ERR_FILE_NOT_FOUND
```

with a **Microsoft Edge logo**, inside a Locus window. Two facts in the report
rule out both earlier causes:

- the install was **fresh from the release page**, so no `tauri_plugin_updater`
  handoff occurred — the process that failed is the installed application;
- `verge.yaml` carried `start_page: /`, so no persisted-URL path could produce a
  bad value.

The decisive line, in `latest.log`:

```text
[Window] page load started:  url=file:///D:/
[Window] page load finished: url=file:///D:/
```

### The defect

`frontendDist` is not only a build-time input. `tauri::generate_context!()`
(`client/src-tauri/src/lib.rs`) resolves it **at compile time and bakes the
resolved path into the binary**, where it becomes the runtime asset root for
`WebviewUrl::App(..)`. The committed `tauri.conf.json` carries the correct
relative `"frontendDist": "../dist"`.

The "better workflow" commit (`ba01ed2`), which made CI faster by building the
frontend once in `verify` and downloading it into each build job, overrode
`frontendDist` to an **absolute runner path** so `tauri build` would reuse that
bundle:

```json
{"build":{"beforeBuildCommand":"","frontendDist":"${{ github.workspace }}/client/dist"}}
```

On the Windows runner `github.workspace` is `D:\a\Locus\Locus`, so the shipped
binary carried `D:/a/Locus/Locus/client/dist` as the directory it expected its
web assets in. On a student's machine that path does not exist. Tauri resolved
the start page against a missing root, WebView2 rendered Chromium's own error
page (Edge logo, `ERR_FILE_NOT_FOUND`) in a window that is `.visible(false)` until
page-load *Finished* — which never arrives — so the app never appeared at all.

Nothing in the report names Locus, Tauri, or CI. And because the bad value is in
the **executable**, not in config: a restart changes nothing, and a reinstall
changes nothing (it installs the same binary).

### Why a green build hid it, twice

`17ef21a` ("don't interpolate a Windows path raw into `TAURI_CONFIG` JSON") fixed
a *different* symptom of the same line: interpolated raw into JSON, `D:\a\...`
contains `\a`, an invalid JSON escape, and the `tauri` build script panicked at
`invalid escape at line 1 column 54`. That commit normalised the path to forward
slashes and encoded it with a real JSON encoder, so **the build went green** — and
the absolute runner path stayed in the binary. The escaping was never the defect;
an absolute build-machine path had no business in a shipped artifact at all. The
commit message shows the author reading `D:\a\Locus\Locus` and treating it as a
character-encoding problem rather than asking why it was there.

### The fix

**Workflow**, `.github/workflows/client.yml` — `TAURI_CONFIG` now sets **only**
the empty `beforeBuildCommand`:

```yaml
TAURI_CONFIG: '{"build":{"beforeBuildCommand":""}}'
```

`frontendDist` is not mentioned. The build job already downloads the bundle to
`client/dist`, and `tauri.conf.json` (in `client/src-tauri/`) resolves its
relative `../dist` to exactly that directory — on every runner, with no path
arithmetic, and nothing to escape, so the JSON-panic cannot recur either. The
shell-time normalization (`DIST=`, the `node` encoder) is deleted with it.

**Guard**, `check-consistency.sh` §16 — three assertions:

- no **live** line of the workflow may set `frontendDist` (comment lines are
  excluded: a `#` line cannot set an environment variable, so it cannot reach the
  build — a trailing comment on a live line is still checked);
- the committed `tauri.conf.json` must keep `frontendDist` relative;
- the build job must download the bundle to `client/dist`, the directory `../dist`
  resolves to, or the relative path silently points at nothing.

The guard was **reintroduced against all three defect shapes and failed on each**:
the original `ba01ed2` literal, the `17ef21a` node-assembled form, and an absolute
committed config. Restored, the tree passes. Guard can fail, and does.

### How it was verified — on the artifact, not the run colour

The fix was pushed to `main` (no tag; the `preview` job runs, `release` is skipped)
as `a8b0acf`, and the push triggered CI run **`36686775049`**. Every job went green
— `Verify`, all four builds incl. `windows-2022`, `Preview installers` — and
`Release` correctly skipped.

**That is exactly why the run colour was not the verification.** Four consecutive
green runs preceded this bug: 3.2.9, 3.2.10, 3.2.11, 3.2.12. A green run means a
bundle *built*; this defect lived entirely in the built artifact. So the
**artifact was read back** — the raw Windows updater payload
(`locus-windows-amd64.exe`) was downloaded from the run and the Tauri context
string embedded in it was inspected directly, with the broken `v3.2.12` build
(`5ebfc89`) as the control:

```text
broken  (5ebfc89):  ...localhost:3000/ d:/a/Locus/Locus/client/dist icons/128x128.png...
fixed   (a8b0acf):  ...localhost:3000/ ../dist                        icons/32x32.png...
```

```sh
strings locus-windows-amd64.exe | grep -c 'a/Locus/Locus'   # broken: 1   fixed: 0
```

The defect string is present in the broken binary and **absent** in the fixed one,
which now carries the committed relative `../dist` — resolved beside the executable
at run time, which is the correct behaviour and the thing `file:///D:/` violated.
The app identity (`com.locus.client`, `icons/128x128.png`) is intact in both, so
the context was not merely stripped. `CLAIMS.md` §5 **A8** is moved to
**verified 2026-09-30 for `a8b0acf`**.

This is the class-level check the project had been saying it lacked, performed once
by hand: **inspect the build's own output, not the run's colour.** It is not yet a
CI step — see the note below.

### What is still NOT verified

- **The interactive install on Windows is now CONFIRMED** (2026-09-30, after this
  entry was first written): `installer-Locus_3.2.12_x64-setup.exe` was run
  interactively on real Windows, the NSIS setup completed, and Locus installed.
  That closes `CLAIMS.md` §5 **A6**, which had been unverified since the NSIS
  `installerHooks` defect. This is the first Windows install the project has ever
  confirmed. It settles the installer, not the app: the window rendering after a
  real install is A8's interactive side, and A7 (surviving an *update*) remains
  unverified.
- **The artefact-level check is manual.** It is not wired into CI, so the next
  platform-specific path leak would ship undetected exactly as this one did. The
  suggested CI step (run the built binary, assert its page-load URL is not
  `file://`) remains unbuilt — see below.
- **The `build-preview` release is accumulating assets rather than replacing
  them.** Observed 2026-09-30: it held installers for 3.2.7 through 3.2.12 at once,
  contradicting the workflow's stated "replaced wholesale on each run". Not the
  defect above and not fixed here; recorded so it is not mistaken for a clean
  state.

### The class this is the fourth instance of

Three consecutive green releases shipped something unusable (3.2.9 the installer,
3.2.10 the update, 3.2.11 the wrong fix), and this is the fourth. Every check in
this project tests that a bundle **builds**; none launches either artifact, so
each defect after "the bundle compiled" is invisible. This one is worse than its
predecessors in one respect: a build-machine path was *deliberately* injected into
a user-facing artifact for speed, and the only thing standing between it and the
fleet was nobody noticing.

**This entry is the first where the artifact was read back by hand** — and that
read is what turned an unverified fix into a verified one. It is the proof the
class has needed: the build's output is inspectable, and inspecting it catches what
run colour cannot. The CI step that does this automatically (run the built binary,
assert its page-load URL is not `file://`) is still not built. Until it is, this
class is caught by whoever remembers to look, which is not a mechanism.

---

## THE APP RE-EXECUTED ITSELF INSTEAD OF UPDATING, AND SHOWED AN EDGE "FILE NOT FOUND" (2026-09-30)

> **This is the entry the two above were looking for.** Both earlier 2026-09-30
> entries — *THE WINDOWS INSTALLER ABORTED* and the withdrawn
> *start_page* diagnosis — described real defects that were **not** this report.
> Read this one first; the distinctions are in *Why the earlier entries were the
> wrong diagnosis* below.

A student installed `installer-Locus_3.2.10_x64-setup.exe` successfully, launched
Locus from the Start Menu, and got:

```text
File not found
It may have been moved, edited, or deleted.
ERR_FILE_NOT_FOUND
```

with a **Microsoft Edge logo**, drawn **inside the Locus window**. Their app data
root held a freshly written default `verge.yaml` (`start_page: /`, every `locus_*`
field null — never activated), and their install was in `C:\Program Files\Locus\`.

The decisive evidence was two lines from `latest.log`:

```text
[Window] page load started:  url=file:///D:/
[Window] page load finished: url=file:///D:/
```

**`file:///D:/` on a machine with no `D:` drive at all.** That cannot be a stored
path; it has to be *generated*. It is Tauri resolving `WebviewUrl::App(..)`
against a resource directory that does not exist, with the drive letter coming
from the process's current directory — a GitHub Actions runner path
(`D:\a\Locus\Locus`) leaked through the build, and reappearing at run time because
the process was started from a leftover `D:`-rooted staging directory.

### The defect

The artifact this project advertises for a Windows client to **install from
itself** is a raw executable, not an installer. In `manifest.json`, as published
with `v3.2.9` and `v3.2.10`:

```json
"windows": { "file": "locus-windows-amd64.exe", "sha256": "8cdfa29c…", "signature": "…" }
```

That file is staged by `Stage the raw updater artifact (windows)`, which copies
`target/x86_64-pc-windows-msvc/release/locus.exe`. It is the client's own program.

`tauri_plugin_updater` 2.11.0 does not check what it is handed. `extract_exe`
asks only whether the bytes are a PE:

```rust
fn extract_exe(&self, bytes: &[u8]) -> Result<WindowsUpdaterType> {
    if infer::app::is_exe(bytes) {
        let (path, temp) = self.write_to_temp(bytes, ".exe")?;
        Ok(WindowsUpdaterType::nsis(path, temp))
```

`install_inner` then runs it with `ShellExecuteW(.., "open", ..)` and calls
`std::process::exit(0)`. So the update **launched a second copy of Locus as if it
were a setup program** and quit the first one.

### Why the symptom looked like an installer failure

The second copy is not the installed application. It is a bare executable, so:

- nothing it needs is beside it — no `resources/` tree, and no bundled web assets;
- Tauri resolves `WebviewUrl::App` against a resource root that does not exist;
- Windows 11 hands the window to the installed WebView2 runtime, which shows
  Chromium's own error page — Edge logo, `ERR_FILE_NOT_FOUND` — inside a frame
  that still says "Locus".

Nothing in that chain names Locus, Tauri, or the updater. And because the
relaunch happens per *release*, not per launch, the failure is stabilised by the
time anyone looks: the client keeps accepting whatever the manifest names as
newest, so every new release reproduces it and every reinstall "does not help".

### Why the earlier entries were the wrong diagnosis

| | This defect | The NSIS `installerHooks` defect | The withdrawn `start_page` claim |
|---|---|---|---|
| When | After install, on app launch | While the setup `.exe` runs | On app launch |
| Who draws the error | The launched process's WebView2 | The installer's WebView2 bootstrapper | The app's WebView2 |
| Where the URL comes from | The manifest's `windows` entry | A missing `.nsh` at makensis time | A persisted config value |
| Does the reporting config allow it? | **Yes** — nothing in it is involved | No — 3.2.10 installed fine | **No** — `start_page` was `/` |

The `start_page` claim is withdrawn for the plainest reason available:
`config/verge.rs` already rewrites a stored `/home` to `/`, and the config in the
report contained `/`. The code path could not fire. It was inferred from source
twice and ship-checked never, which is what let two wrong releases out.

### The fix

Both halves, deliberately — either alone would have prevented this.

**Client**, `client/src-tauri/src/locus/update/install.rs` — a new
`is_installer_payload` runs **before** the bytes reach the plugin:

- a PE/ELF is accepted only on **positive** evidence that it is an NSIS installer
  (`NullsoftInstaller`, scanned over a bounded window);
- everything else must be a container the platform installer owns — DMG, deb,
  rpm, MSI, AppImage — by an explicit allow-list;
- anything unrecognised, empty, or truncated is refused with a coded error.

The direction matters more than the rules. This began as
`!is_bare_executable(bytes)`, which is true of a one-byte file — a test written
alongside it caught that, and it is why the guard is now built from what a payload
*is* rather than what it is not. A guard for executing the wrong binary cannot be
assembled out of double negatives.

**CI**, `.github/workflows/client.yml` — the `windows` platform entry now names
`installer-Locus_*_x64-setup.exe`. Linux and macOS entries are unchanged: their
formats reach a package manager rather than a bare executable, so they were never
this bug. The manifest step additionally refuses to publish if the Windows entry
is the raw binary, and refuses if no installer can be found at all — it never
falls back.

`v3.2.11` was **reverted in full** (`45d9402`). It shipped a fix for the
withdrawn `start_page` cause and would have added a third wrong explanation to
this log.

### Shipped in v3.2.12, and what the release itself proved

The tag build is **run `36669449010`**, fully green: `Verify`, all four `Build`
jobs (including `windows-2022`), `Release`, `Report`. That is deliberately
recorded here rather than left to a session log, because this release's whole
lesson is that a green run proves very little on its own — three consecutive
green runs (3.2.9, 3.2.10, 3.2.11) each shipped something unusable.

So the check that matters is the *artifact*, not the colour of the run. The
published `manifest.json` for `v3.2.12` was fetched and read back:

```json
"windows": { "file": "installer-Locus_3.2.12_x64-setup.exe", "sha256": "…" }
```

The `windows` key now names the NSIS setup executable, and the release carries
the asset to match it. The three other platform entries are unchanged
(`locus-linux-amd64`, `locus-darwin-amd64`, `locus-darwin-arm64`), which is the
evidence that this change was scoped to the one platform with the bug rather than
applied blindly to all four.

### The guard

`check-consistency.sh` §15 asserts that no manifest platform entry advertises
`locus-windows-amd64.exe` for self-install, that the Windows entry resolves to a
setup executable, and that the client still calls `is_installer_payload`. The
defect was reintroduced line-for-line and the guard failed on it; the Rust tests
were run against a simulated pre-fix `return true` and 4 of them failed,
including the one that pins this bug. Both were restored before the tree was kept.

Server-side halves do not reach the live host until `setup.sh` re-runs; this change
is client and CI only, so nothing here waits on that.

### What was NOT verified, and must be

No Windows machine and no NSIS bundle were available. The claim "an installed
Windows client survives an advertised update and keeps launching from its install
directory" is **unverified** and is recorded as `CLAIMS.md` §5 **A7**. The next
Windows install *and an update taken on it* is the test — installing alone does
not exercise this path, which is precisely how it survived three releases.

### What this says about CI

Every check in this project tests that a bundle **builds**. None launches either
artifact, so each defect after "the bundle compiled" is invisible: the installer
that cannot install (3.2.9), the update that cannot update (3.2.10), the window
that cannot draw. A step that runs the built binary and asserts its page-load URL
is not `file://` would have caught all three. That is the real fix this class
still needs and does not yet have.

---

## THE WINDOWS INSTALLER ABORTED WITH AN EDGE "FILE NOT FOUND" DIALOG (2026-09-30)

> **CORRECTED 2026-09-30.** This entry was written as the explanation for a report
> of `ERR_FILE_NOT_FOUND` and **is not it**. That report was an *update* that
> re-executed the client's own binary — see the entry above. The defect described
> here is real and shipped in a build with a different symptom: the setup `.exe`
> aborting *while installing*. Kept on its own terms, because a matching error
> string is not a matching cause and the deciding question is **when** the dialog
> appears.

A student installed the 3.2.9 Windows setup and got, instead of an application:

```text
File not found
It may have been moved, edited, or deleted.
ERR_FILE_NOT_FOUND
```

with a **Microsoft Edge logo**. Nothing in that message names Locus, so the
obvious reading — an Edge/WebView2 problem — is wrong. The dialog belongs to the
NSIS installer's WebView2 bootstrapper, and it appeared because the installer
never got as far as running it.

### The defect

`tauri.windows.conf.json` named two files that no build step could produce:

```json
"installerHooks": "./packages/windows/core-hashes.nsh",
"template":       "./packages/windows/installer.nsi"
```

`core-hashes.nsh` was generated by `scripts/prebuild.mjs`, and that ran inside
`beforeBuildCommand` — which **every CI build job blanks** via `TAURI_CONFIG`,
because the frontend bundle is built once in `verify` and downloaded. So the
file did not exist on the Windows runner. The custom `installer.nsi` carried the
line that consumed it:

```nsis
!include "{{installer_hooks}}"
```

An `!include` of a non-existent file is a **fatal** makensis error, but the
failure surfaces as an error-page dialog rather than a compile-time stop: the
bundle built, and the broken installer shipped.

### Why nothing caught it

The run was fully green — `verify` ✓, all four builds ✓, `release` ✓. Every
check the project has is a check on **building** a bundle. Nothing ever **ran**
the installer, and the defect existed only at install time. This is the
`v3.2.9` shape of the project's oldest bug class: a claim that was true when
written and never re-established — a bundle that builds is not an installer that
installs.

### The fix

Minimal, and in the direction of deleting the parts that could not work:

- **The custom template is deleted.** `tauri.windows.conf.json` names no
  `template`, so the bundler uses its OWN `installer.nsi` for the pinned CLI
  (2.11.4). That restores the upstream WebView2 handling, the fork-specific
  window-state cleanup and legacy-run-key removal in one step — all of which
  lived only in the deleted file, and none of which can now drift from the
  bundler it is compiled by. Verified against the vendored CLI binary: the
  built-in template contains `Win\COM.nsh` / `Win\Propkey.nsh`,
  `${StrCase}`/`${StrLoc}`, and **never** references `WinVer.nsh`,
  `MIHOMO_SHA256` or `core-hashes` — every construct in the deleted file that
  the bundler could not have supplied.
- **The `installerHooks` entry is gone**, and so are `resolveCoreHashes` and
  `core-hashes.nsh`.
- **`prebuild.mjs` now runs as an explicit workflow step** on every platform, so
  the `winOnly` tasks have a producer in CI. This half is load-bearing on its
  own: without it, the next person who adds `installerHooks` back recreates the
  bug exactly.
- The `report` job's step summary now **states what the run did not verify** —
  that the Windows installer is built and signed but never executed.

### What was NOT verified, and must be

Deleting the template is a **compile-time and structural** argument, not a
running installer. No NSIS bundle was produced on this machine (no Windows toolchain),
so the claim "the Windows installer works" is **unverified** as of this entry
and belongs in `CLAIMS.md` §5 as such. The next Windows install is the test.

> **UPDATE 2026-09-30 (later the same day).** That test has now happened:
> `installer-Locus_3.2.12_x64-setup.exe` was run interactively on real Windows and
> completed, installing Locus. `CLAIMS.md` §5 **A6** is verified. The paragraph
> above is kept as written because it was true when written; read it as history.

### The guard

`check-consistency.sh` §14 fails the build when the Windows bundler config names
an `installerHooks`/`template` file that is absent — or that exists but is
**generated and uncommitted**, which is the exact `v3.2.9` state — and when no
workflow step runs `prebuild.mjs` directly. All three failure modes were
reproduced against a deliberately broken tree before the guard was kept.

Client-side only; no server change, so nothing here waits on a `setup.sh` re-run.

---

## THE UPDATE DOWNLOADED INTO RAM AND KILLED THE APP (2026-09-30)

"Install Now" left the student with a progress bar frozen at 0%, and then the
app died. The log recorded the hub offering 3.2.8 and the heartbeat and nothing
else — no line about the update starting, none about it failing:

```text
[2026-09-30 03:05:49.601] INFO [Config] [locus] starting heartbeat (tier strike)
[2026-09-30 03:05:49.948] INFO [System] [locus] the hub is offering update 3.2.8
```

That is not a shortage of logs at the point of failure. It is a path with **no
log lines to have**: `locus_install_update` was silent until it either failed to
check or succeeded. A process that died mid-download left nothing behind, which
is why the report could not say where it stopped.

### The defect

The install path called the Tauri plugin's `download_and_install`. Reading the
plugin (`tauri-plugin-updater-2.11.0/src/updater.rs`, the body of `download`), it
buffers the **entire artifact in memory** before verifying it:

```rust
let mut buffer = Vec::new();
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    on_chunk(chunk.len(), content_length);
    buffer.extend(chunk);
}
```

On the machine this product is for — a school laptop, already running the VPN
core — a 60 MB installer held in RAM is enough to abort the allocation. The task
dies, the IPC call never resolves, and the UI keeps its spinner.

### Why a whole module was already written to avoid this and was never called

`client/src-tauri/src/locus/update/apply.rs` streams to disk, hashes
incrementally, bounds the download, and fails closed on a missing checksum. It
had **zero production callers**: `cmd/locus.rs` imported `crate::locus::apply`
(the *config* module) and the update path went plugin-only. The two modules had
also been written from incompatible plans — `apply.rs`'s header describes handing
a hand-built `Update` to the plugin, which `install.rs`'s header explicitly says
is impossible (`extract_path` and `context` are private). So the safe downloader
sat beside the unsafe one, and only the unsafe one was wired up.

Nothing noticed, because nothing checked.

### The fix

The download is now the client's (`apply::download`, streaming to disk) and only
the install is the plugin's:

- `PendingInstall::download_to_file` streams the artifact to the app data root
  and verifies SHA-256 on the bytes on disk, then returns a `ReadyInstall`.
- `ReadyInstall::install` hands the verified bytes to `Update::install`, which
  still owns NSIS/AppImage/deb/rpm and still re-verifies the minisign signature.
- The hub's checksum is read from the plugin's preserved `raw_json` (the plugin
  does not model a checksum, but it keeps the whole response). A missing hash is
  refused **before a byte is fetched** (`LOCUS_UPDATE_NO_CHECKSUM`) rather than
  discovered after.
- Progress is emitted cumulatively and throttled to one event per percent — the
  plugin's callback fires per network chunk, and an event per chunk floods the
  webview.
- **Every branch now logs**: start of attempt, download started, download
  verified, installer handed the bytes, installed, and each failure with its
  reason.

A process-wide **panic hook** was also installed at startup. An async Tauri
command that panics is otherwise swallowed — the task dies, the call never
resolves, and the next reader gets the same empty log. The hook writes the panic
to the app log, so the next one of these is a stack trace rather than a spinner.

### The guard

`check-consistency.sh` §12 fails the build if the install path calls
`download_and_install` again, if `apply::download` loses its production caller,
if the install command loses its start-of-attempt log line, or if the hub
checksum stops being enforced. This is the class of defect a guard is for: two
modules that disagree, with no mechanism that notices.

Client-side only; no server change, so nothing here waits on a `setup.sh` re-run.

---

## THE GUARD THAT REFUSED EVERY RELEASE: goja HAS NO `atob` (2026-09-30)

Publishing 3.2.7 from the admin console was refused outright, naming all four
platforms:

```text
refusing to publish 3.2.7 — the fetch did not produce a verified, signed
artifact for: linux (signature not base64 of a minisign .sig file),
windows (signature not base64 of a minisign .sig file),
macos_intel (signature not base64 of a minisign .sig file),
macos_arm (signature not base64 of a minisign .sig file).
```

The message blames the **fetch**. The fetch was innocent. This is the direct
sequel to *THE UPDATE COULD NOT BE INSTALLED: THE SIGNATURE WAS UNDECODABLE*
(2026-09-29, above): that entry established the correct wire format —
`base64(entire .sig file text)` — and added a shape guard at the hub. The guard
was written against a base64 decoder **that this runtime does not have**, so it
returned `false` for every input, correct ones included.

### The defect

`server/pb_hooks/admin_console.pb.js`, `isPlausibleMinisignSignature()`:

```js
var decoded = $os ? atob(s) : "";
```

`$os` is truthy, so this evaluates `atob(s)`. **`atob` is not defined in
PocketBase 0.22.21's goja**, so the call throws a `ReferenceError` — caught by the
function's own `catch (decErr) { return false; }`, which converts a *crash* into
the honest-sounding verdict "not base64 of a minisign .sig file".

A guard whose decoder does not exist fails **closed on every input**. That is why
it read like a data outage: the only way to make it pass would have been to
supply something that is not a signature at all.

### What goja actually provides (measured, PB 0.22.21)

Probed live with a temporary hook, then removed:

| Global | Available? |
|---|---|
| `atob`, `btoa` | **no** |
| `Buffer`, `TextDecoder` | **no** |
| `$os` | yes — args/exec/**filesystem**/getenv only; **no base64** |
| `$security` | yes — md5/sha256/sha512/hs256/hs512/equal/randomString/JWT/encrypt/decrypt; **no base64** |

There is **no builtin base64 anywhere in the hook runtime**. Any hook needing it
must implement it in pure JS. `$security`'s presence is the trap: it looks like
the place a base64 helper would live, and it has none.

### Why the error was misleading, and how that was proved

Every other component in the chain was correct and was verified rather than
assumed:

- the four `.sig` files on disk under `/var/www/updates/3.2.7/` are real 4-line
  minisign signatures;
- `fetch-release.py` returns them base64-encoded, once, in
  `artifacts[<platform>].signature` (linux: 364 chars, `dW50cnVzdGVk…` = base64 of
  `untrusted…`) — confirmed by calling the live endpoint;
- `publish-release.sh`, the client contract test, and CI's `manifest.json` all
  agree on the same wire format.

The decisive test was to **post the live fetch output straight back into
`releases.publish`**. It reproduced the refusal verbatim — so the payload was
right and the *consumer* was wrong. A producer/consumer error message that blames
the producer should always be checked this way before touching the producer.

### The fix

Replace the `$os ? atob(s) : ""` line with a self-contained pure-JS base64
decoder (alphabet lookup plus a bit accumulator) and scan the decoded text for
minisign's four-line shape. No globals, so nothing to be missing. Verified
against the four real live signatures: each decodes to 4 lines whose first line
is `untrusted comment: ` and third is `trusted comment: `.

`releases.set` shares this helper, so the activate path was refusing releases for
the same reason. Both are fixed by the one change.

### The operational trap this exposed

**PocketBase caches `pb_hooks` at startup.** Editing a `.pb.js` file under
`/opt/pocketbase/pb_hooks/` has **no effect** until `systemctl restart pocketbase`.
During this investigation a correctly-applied patch appeared not to apply because
the running process was still serving the previously-loaded hook — the classic
"the fix did not work" false negative. Any hook diagnostic that relies on a new
code path must restart first, or it will measure the *old* code and lie.

### For whoever picks this up

- **Prefer a guard that can fail for a legitimate reason.** A predicate returning
  `false` on every input is indistinguishable from a total data loss in its
  output; the failure mode here was invisible for exactly as long as nobody
  published a release.
- **The same class is still live elsewhere if `atob`/`btoa` are used anywhere
  else.** At the time of the fix, `atob` appeared in the hook tree in this one
  place only — worth re-checking with `grep -rn 'atob\|btoa' server/pb_hooks/`
  before assuming it stays that way.
- The client-side contract test cannot catch this: it exercises Rust and Python,
  not goja. A hub-side guard needs a hub-side test, or the same blind spot
  returns.

---

## VOLATILE FACTS WERE ASSERTED AS PROPERTIES, AND NOTHING NOTICED (2026-09-29)

The hub's IP address was recorded as a fact in `docs/STATE.md`, in a code test and
in a diagnostic script. It had already gone stale **three times**
(`114.23.136.59` → `134.199.155.166` → `170.64.196.179`), each replacement written
in with equal confidence, and the last one **stopped existing** while the file
still called itself "the single source of truth for facts that change". No test
failed, because no test was looking; the world moved and the documents did not.

The same file asserted *"DNS resolves the hostname to the VPS IP automatically, no
DuckDNS updates needed"* — the claim that made writing an address down seem safe,
and which was contradicted by the file's own history.

**This is not the doc/code drift class.** In drift, a document and the code
disagree. Here they agreed perfectly when written, and the sentence became false
without a single commit. It is the same *shape* as the `unbound_at` bug: something
that looked implemented and recorded nothing.

**The fix is structural, not editorial** — careful prose does not fail loudly:

- **`docs/state.toml`** — new. Derived facts (version, domain, console path,
  platform list) as data, each with a `source` and a `provenance`. There is
  deliberately **no `host`/`ip` key**: an address that changes on every redeploy is
  not a derived fact.
- **`docs/operate/CLAIMS.md`** — new. The register of **Class A world claims** (hub
  reachable, codes in use, preview current, CI ran) with a date and a re-verify
  command each. **All five are currently `unverified`**, which is the honest state.
- **`docs/STATE.md`** — rewritten. No facts at all now: it is the ownership map and
  the rule for where a fact goes.
- **`check-consistency.sh` §9** — recomputes every `derived` fact against the tree
  and fails on drift; reports `asserted` and warns on `unverified`. A fact marked
  `derived` with no checker fails, because that is false confidence.
- **`check-consistency.sh` §10** — fails when a live document inlines an IP
  address or a headcount. Generalises §7's version rule: *you cannot have drifted
  from a value you never wrote down.*
- **`server/scripts/verify-live.sh`** — new. Probes the world claims and returns
  **three** outcomes: PASS, FAIL, and **SKIP** (exit 2) when the network is
  unreachable. **SKIP is not PASS** — an environment that cannot see the hub has
  learned nothing, and must never advance a `last-verified` date.
- **Swept**: the dead IP removed from `STATE.md`, `CONTEXT.md`, `OPS.md`,
  `STILL-OPEN.md`, `GAMING-UDP.md`, `client/src-tauri/tests/tier_profile_engine.rs`
  (fixture now `hub.example.invalid`) and `scripts/vps-test/transport-probe.sh`
  (now resolves the domain, and **fails loudly if it does not resolve**).

**The live-customer datum was removed from the repo.** Every file that stated
"11 real Strike codes for middleman 'Wanzhen'" now states the **rule** — *never
bulk-delete `codes`/`code_events`; revoke with Suspend, move with Unbind* — with no
count and no name. It decayed the moment it was written, and it identified a
customer in a product whose central design decision is to store no identity. The
reasoning is `CLAIMS.md` §4.

**Honest limit:** this makes the world claims *dated, single-homed and checkable*.
It does not make them *true*. Re-run `verify-live.sh` to advance a date; it refuses
to advance one on a SKIP.

---

## UPDATES UNINSTALLABLE, AND "CONNECTED" WITH NO INTERNET (2026-09-29)

Client **3.2.7**. Two severe defects, both reported from real use, both with a
root cause that every automated check called healthy.

### 1. The update was unusable by every client, on all four platforms

**Symptom.** Triggering an update failed with an error about an *offset*
(`Invalid byte …, offset N.`), the app then crashed, and every later attempt
failed immediately.

**Root cause.** The live `update_config` row stored the `signature_<platform>`
field as the **raw minisign `.sig` file text**. `tauri-plugin-updater` runs
`base64_to_string()` over that field *before* parsing it as minisign text, and the
`.sig` file contains spaces and newlines — which a base64 decoder rejects with
exactly the offset error the student saw. Verified against production at the time:
all four platforms served a field that failed `base64.b64decode(validate=True)`.

**Why every attempt failed after the first.** The client only clears the offered
update on **success** (`store::clear_update_offer` after `install()` returns
`Ok`), so each retry re-fetched the same corrupt field and failed identically.

**Why it was not caught.** The encoding was correct in `publish-release.sh`, and
the tests exercised *that* path plus the client's decode contract. But the live row
had been written by the **other** two paths:

- `server/scripts/fetch-release.py` stored the raw `.sig` text
  (`entry["signature"] = f.read().strip()`).
- `server/pb_hooks/admin_console.pb.js`'s `releases.publish` wrote it verbatim,
  guarding only that the field was **non-empty** — while `releases.set` (which
  *activates* the release) demanded base64 shape. Publish and activate disagreed
  about what a valid signature is, so a release one accepted the other rejected.

**Fixes.** `fetch-release.py` now base64-encodes the `.sig` text at the single
point a `.sig` becomes a stored field. `releases.publish` validates the shape with
the **same** `isPlausibleMinisignSignature` predicate `releases.set` already used,
hoisted to handler scope so both paths share it. `check-consistency.sh` §1b now
fails if any of the three (fetch encoding, publish guard, set guard) drifts —
verified by re-introducing each bug and watching it fail.

**The bad row is data, not code** — fixing the scripts does not repair it.
`publish-release.sh <version>` re-fetches the `.sig` files and PATCHes the row with
the correct encoding; see `docs/operate/UPDATE-SYSTEM.md` §3 for the repair and the
one-line verification.

### 2. "Connected" on a machine with no usable internet

**Symptom.** With no internet connection, the app still reported **connected** and
routed no traffic — worst on flaky school wifi, where the student most needs a
correct answer.

**Root cause.** Readiness was a single *local* observation: `probe_core_api` asked
the Core's control API (`/version`). mihomo binds its external controller and
answers that whether or not a packet can leave the machine, so a Core that could
route nothing still read `Serving` → `Ready` → **Connected**. The probe proved the
Core was up, never that the tunnel worked.

**Fix.** Readiness now needs **two** proofs. `probe_egress` runs a real request
*through* the tunnel via mihomo's own `delay_group` for the tier's `Locus Auto`
group and a 204 test URL; `Ready` requires the Core answering **and** egress
succeeding. A Core that is up but cannot carry a packet is `Readiness::NoEgress`,
surfaced as `coreUp: true, ready: false` — which the connection screen renders as
**connecting** ("the tunnel exists but the internet is not reachable through it"),
never as connected. No front-end state machine change was needed: `phaseFromStatus`
already treated `connected && !ready` as connecting.

Per the same session's decision, this is the *strong* form — egress proof is
required before `Ready`, not merely reported alongside it.

---

## TERM-BASED ACCESS, RENEWAL, AND ONE CODE PER DEVICE (unreleased)

Client **3.2.5** still; these are hub-side and console changes, plus one client
change (`DeviceAlreadyActivated`). Commits `9a91da1`, `6a422bc`, `0f20f0e`.

The code and the business plan disagreed about what a customer buys. The plan
sold **term passes** (`docs/business/07-billing.md` §7.1: *"lead with the term
pass"*, priced at $10/$19 in `18-open-items.md` P2), but the tree could only
express **one absolute `expires_at` fixed at MINT time**. So the recommended
product was not sellable, and there was no way to take money for a second term
at all. Three structural faults, all symptoms of one absence — the hub had no
way to express a term, an entitlement, or a device.

**What replaced it.** A code carries a **term** (`term_days`) measured from
**activation**, materialised into `expires_at` when a device binds. `expires_at`
kept its type and meaning throughout, which is why the four enforcement readers,
the wire format and the client's whole `expiry.rs` needed **no changes at all**
— the term is only how the instant is computed. `codes.renew` extends from the
**later of now and the current expiry**, so paying early never loses days.
Design record: `docs/business/redesign/`; implementation reference:
`docs/business/redesign/implementation/`.

### 1. The fingerprint was normalised in half the places it was used

Found while verifying the implementation docs against the code. Three sites
disagreed about what a fingerprint *is*, and every one fails **silently** — the
writes succeed, the rows look correct, and only the check quietly stops
matching.

| Site | Defect | Failure if it fired |
|---|---|---|
| `recordBinding` (activation) | looked up the **stripped** value, inserted the **raw** one | the row lands under a key the lookup cannot find, so the uniqueness check **misses it** and the same device binds a second code |
| the bind path | wrote raw `fp` to the code's `bound_fingerprint`, while the index got the stripped one | code and index disagree about which device it is |
| `activation.pb.js` re-activation branch, `code_lookup.pb.js` pre-check | compared a stored **stripped** value against an incoming **raw** fingerprint | a returning student is told their own code is **"bound to another device"** and sent to a middleman |

Real fingerprints are hex, so the two values coincide and none of it fired in
practice. It was fixed anyway because the failure mode is invisible: no error,
no log line, no failing test — just a guarantee that stops holding. All three
now normalise once and use that single value for the lookup, the write and every
comparison. `check-consistency.sh` group 6(e) **enforces** it (verified by
reintroducing the bug and watching the guard fail).

### 2. A force-deleted bound code would have bricked the device

`codes.delete` can force-delete a **bound** code. Under the old model that was
harmless — the code was gone, so the binding was gone with it. With the new
binding index the row survived, pointing at a code that no longer existed, and
the uniqueness check would then refuse **every** future activation on that
machine — **with no code left on the hub for an operator to unbind.** A deletion
would have permanently bricked a device.

Fixed: `codes.delete` releases the binding first. The state is also *visible*
now rather than silent — `device.get`, `devices.list` and the Devices page all
report `code_missing`.

### 3. There was no way to find a code by the student's name

`codes.list`'s query matched **the code string only** — `label` and `notes` were
returned in every row but searched by nothing. So an operator taking a support
call ("it's Kahu, my code stopped working") could not find Kahu's code, and
every renewal began with guessing which code was whose.

It matters more than it sounds: renewals are operator-initiated and nothing
prompts one, so this is the step the whole revenue operation depends on. The
query now matches `label`, `notes` and `middleman` case-insensitively, and the
console's search box says so.

### 4. The renewal worklist did not exist

Specified and not built. Since nothing in the system prompts a renewal, the
queue **is** the mechanism: if nobody opens it, students lapse silently and no
money is collected. `codes.list` gained `expiring_within_days`, worklists sort
by **urgency rather than recency**, rows carry `days_remaining`, and the
Dashboard's "Expiring in 30 days" card — previously an inert metric — now opens
the worklist.

### 5. The binding index was write-only

`device_bindings` gained rows but nothing could read them, so the
one-code-per-device rule was **unverifiable** from the console. Added
`device.get`, `devices.list` (with a duplicate-live-fingerprint check, which
should always be empty) and a read-only Devices page.

### 6. Documentation drift, and a checker that cried wolf

* `docs/OPS.md` listed **none** of the new actions and still minted with an
  absolute `expires_at` and no term — the operator reference was materially out
  of date.
* `docs/API.md` had no 409, no `term_days`, and no admin-console section.
* `check-consistency.sh`'s version check warned on 8 files that were all
  **legitimate history** ("first shipped as 3.0.0", "the 3.1.0 rework", example
  command arguments). A warning that fires on correct content trains the reader
  to ignore it. It also had a real bug: the `docs/history/` exclusion had no
  wildcard, so it never excluded the files inside that directory. Both fixed,
  with every exclusion justified in the script.
* **71 dead cross-reference anchors** across the business docs — links like
  `04-tiers.md#44` (meaning §4.4) resolve to nothing, because the real anchor
  for "## 4.4 The free tier, in full" is `#44-the-free-tier-in-full`.
  `19-cross-reference.md` alone had 24 broken. All fixed, and
  `check-consistency.sh` group 8 now fails on a dead link or a fragment that
  matches no heading.

---

Released as **3.2.5**. Found while reviewing the tree after the update-signature
fix (the section below); the first item is the one that mattered.

### 1. CI never ran a single test

`client.yml`'s `verify` job ran `check-consistency.sh`, `pnpm run web:build` and
`pnpm run lint` — and nothing else. **There was no `cargo test`, no `cargo clippy`
and no `pnpm test` anywhere in `.github/workflows/`.**

So the Rust and frontend suites (530 and 49 cases at the time — see the note
below on counts) existed and passed locally while being decorative from CI's
perspective: a commit could break the expiry arithmetic, the no-downgrade gate or
a wire-shape pin and still build, sign and publish on a `v*` tag. That is exactly
the bug class `check-consistency.sh` was written for — the guard existed, it
simply was not wired to the thing that ships.

**Fix:** `cargo test --all-targets` and `cargo clippy --all-targets --features
clippy -- -D warnings` now run in the **build matrix job**, alongside the
platform build; `pnpm test` runs in `verify`, because the frontend suite finishes
in seconds and can ride the fast gate. The workflow's `paths:` filter also gained
`server/**`, because the consistency guard it runs reads that tree and a
server-only commit would otherwise skip the check entirely.

> **Correction (2026-10-01).** This entry originally said the Rust suite runs in
> `verify`, "not the build matrix, so a failure costs seconds rather than four
> cross-platform builds". The workflow has never done that — the steps are in the
> matrix job, so a Rust failure costs the four builds this sentence claimed it
> avoided. The claim was wrong when written and is corrected here rather than
> silently, because a reader deciding where to add a check will consult this
> paragraph. See [`CI-CD.md`](../operate/CI-CD.md) for the current split.

### 2. The "or as `X-Admin-Token`" fallback had never run

`admin_console.pb.js` documented two auth transports: `admin_token` in the body,
or an `X-Admin-Token` header. The second was dead by construction.
`$apis.requestInfo(e).data` carries the request **body** — PocketBase copies body
keys into the key-value store, and a hyphenated header name is not a valid
identifier, so `e.request().header.get("X-Admin-Token")` read an empty store
unconditionally.

It appeared to work only because the console sends the token in *both* places.
**Fix:** read `$apis.requestInfo(e).headers`, which is the documented accessor and
is case-insensitive. The header path is now real, which matters because
`fetch-release.py` (the other admin surface) honours the header properly — the two
had different real semantics while claiming to be the same.

### 3. Admin tokens were compared with an early exit

Both admin hooks compared the token with `!==`, which stops at the first
differing byte. **Fix:** an inline `constantTimeEquals` in each hook (inline
because file-scope helpers are invisible to `routerAdd` callbacks in this build).
goja has no `crypto.timingSafeEqual`, so this is the closest available; the length
check still leaks length, which is acceptable for a fixed-format high-entropy
secret behind TLS and a rate limit. `admin_unbind.pb.js` is now explicitly
documented as body-only, since a second auth shape there would only widen it.

### 4. The `CIPHER-AND-PORTING-REVIEW.md` deletion lost its reasoning

`e743a4b` deleted the file, but `docs/FIXES.md` still pointed at it (a dangling
reference) and the **rejection analysis for proposal 1** existed nowhere else.
That is the reasoning that stops someone re-proposing port fan-out.

**Fix:** the reasoning is recovered into the SS2022 section below, and the
reference corrected. Deleting it was fine; deleting the only copy of a rejected
design's justification was not.

### 5. `docs/CONTEXT.md` described the retired client as current

It is advertised twice as "read this first" with no status banner, yet it is a
hybrid: §193 presented the retired Wails client's hardening as the live client's,
the Key Metrics table was stale (~7,700 lines/38 files against a measured 9,504
across 48), and the pitfalls list said **"The client code now lives in
`legacy/wails-client/` — always build from legacy/wails-client/"**, which is
exactly backwards and the belief the restructure existed to eliminate.

**Fix:** a scope banner, `⚠️ HISTORICAL` banners on the retired-client sections,
the corrected pitfall, and the corrected metric.

### 6. A user-facing error message contained 22 consecutive spaces

`LOCUS_NO_TIER_CONFIG` shipped with a run of 22 spaces mid-sentence — a manual
re-wrap whose newline was replaced rather than removed. It reached the student
verbatim and no test looked at it.

**Fix:** the message is corrected and a new test
(`user_facing_messages_have_no_collapsed_whitespace`) scans the file's message
literals for runs of 3+ spaces, so the class cannot return. Verified by
reintroducing the bug and watching the test fail.

### 7. Nothing checked version agreement in prose

Nine documents were left saying `3.2.3` after the client reached `3.2.4` —
including `README.md` and `docs/README.md`. The three manifests are guarded by
`version_consistency.rs`, but the documentation had no guard at all.

**Fix:** `check-consistency.sh` gained a version-agreement group (informational —
some documents legitimately record a past version), and the stale references are
corrected across the tree. It also filters dependency pins like `^3.4.0` so the
report is signal rather than noise.

### Also in this release

- **Machine-checkable guards for the PocketBase trap class.** `check-consistency.sh`
  now fails on file-scope helper functions (invisible inside `routerAdd` callbacks),
  on any real `findRecordsByFilter(` call (returns zero rows with no error on this
  build), on a non-constant-time token compare, and on a header read through the
  requestInfo key-value store. Each guard was verified by injecting the bug it
  targets and confirming it fires. The `findRecordsByFilter` check strips `//`
  comments first, because every hook carries a warning comment naming the function
  and a naive grep reported the documentation as the defect.
- **Front-end formatting debt cleared.** `biome format` fixed the 22 files that
  were failing `pnpm format:check` (not in CI, but real drift).

---

## THE UPDATE COULD NOT BE INSTALLED: THE SIGNATURE WAS UNDECODABLE (2026-09-29)

The first real update attempt after the updater was wired. The prompt appeared
correctly, the student pressed **INSTALL NOW**, and got:

```text
the update could not be installed: The signature
RWT9eeTiJVDWZDZ8hFQ7TOE6BW7OEwARW+kVEU7m2jIlxcjfiGR9InKW could not be
decoded, please check if it is a valid base64 string. The signature must be
the contents of the `.sig` file generated by the Tauri bundler, as a string.
```

Two independent defects, one per field the plugin decodes. **Neither could be
seen from inside either language** — each half was internally consistent, and the
mismatch only existed where a Rust config value, a Python publish script, a JS
hub field and a third-party decode chain meet.

### 1. `tauri.conf.json`'s `pubkey` was the bare key, not base64 of the `.pub` file

The value was `RWT9eeTiJVDWZDZ8hFQ7TOE6BW7OEwARW+kVEU7m2jIlxcjfiGR9InKW` — the
second line of `locus_update.pub`, and exactly what `minisign -G` prints. It is
the *correct* key material and the *wrong* wire form.

`tauri-plugin-updater`'s `verify_signature()` calls `base64_to_string()` on the
pubkey **before** `PublicKey::decode()`, and that helper requires the decoded
bytes to be valid UTF-8 **text**. A bare key decodes to 42 binary bytes, so it
fails the first step — and because the pubkey is decoded *first*, the error names
the **signature** while the fault is in the **pubkey**. That is why the message
above is so misleading: it quotes the pubkey back at you and calls it a
signature.

The documented form is `base64(entire .pub file text)`:

```sh
base64 -w0 .locus-keys/locus_update.pub
```

Confirmed against the real crate versions, with a real signature from the real
key:

| `plugins.updater.pubkey` | Result |
|---|---|
| base64(whole `.pub` text) | ✅ decodes → `PublicKey::decode` OK → signature verifies |
| the bare key line | ❌ `base64_to_string` fails: *invalid utf-8 sequence of 1 bytes from index 2* |

### 2. The hub advertised a signature it had never checked the shape of

Both the `/api/update` hook and the `releases.set` guard asserted only that
`signature_<platform>` was **non-empty**. Any non-empty string passed. So a
malformed value was published, served, downloaded, and rejected at the very last
step — on the student's machine, for a release every automated check called
healthy. Presence is not shape.

### 3. What the fix does NOT change, and why that matters

`publish-release.sh` stored `base64(entire .sig file text)` — the **correct**
value for this field. It was checked against a real signature rather than
assumed; the three neighbouring encodings are all wrong:

| value stored in `signature_<platform>` | result |
|---|---|
| base64(whole `.sig` text) | ✅ base64 decodes → `Signature::decode` OK → verifies |
| the `.sig` text verbatim | ❌ base64 decode fails — the file has spaces/newlines |
| the signature line only | ❌ decodes to binary signature bytes, not UTF-8 text |
| base64(the signature line only) | ❌ loses the trusted comment; 4-line parse fails |

An earlier attempt to "fix" this by removing the base64 layer was **wrong** and
was caught by testing rather than reasoning — which is the point of the new test.

### The guard that would have caught it

`client/src-tauri/tests/update_signature_contract.rs` reproduces the plugin's
exact decode path and runs real key material through it. Four tests:

- the configured pubkey is base64 of the `.pub` text **and** parses, with its key
  line matching `.locus-keys/locus_update.pub`;
- the **bare key is rejected** — the trap is pinned, not just the fix;
- `signature_<platform>` is base64 of the whole `.sig`, and each neighbouring
  encoding is asserted to fail;
- `publish-release.sh` still refuses a `.sig` that is not minisign's shape.

It fails on the old value. Verified by reverting the pubkey and watching
`the_configured_pubkey_is_base64_of_the_public_key_file` fail with exactly the
error above.

`releases.set` now also applies the same shape check at the hub, so a bad field
is refused at publish time instead of at install time.

### For whoever picks this up

- **A client built with the broken pubkey cannot be updated by this release** —
  and, importantly, is not the situation here: the pubkey is compiled in, so any
  installed build that has the old value will fail verification on *every*
  future update, no matter how correct the signatures are. Those builds must be
  reinstalled from an installer. Check which builds are actually in the field
  before assuming an update will reach them.
- The `pubkey` is part of the app's identity. Rotating it is a breaking change
  (`SIGNING.md` §5); changing its *encoding* is not a rotation, but it does mean
  builds on either side of this change disagree.

---

## EXPIRY INVISIBLE TO THE UI, AND THE UPDATE TOGGLE WIRED TO THE WRONG SETTING (2026-09-28)

Two client defects, both reported by a user against 3.2.3.

### 1. Days-to-expiry never appeared, and would not refresh

Three faults stacked, which is why it read as "it simply will not appear".

| # | Fault | File |
|---|-------|------|
| a | The Rust enum serialized its struct fields as **snake_case** while the TypeScript type read **camelCase** | `client/src-tauri/src/cmd/locus.rs` |
| b | `#[serde(rename_all = "camelCase")]` on an enum renames the **variants**, not the fields inside them — the field-level attribute is `rename_all_fields` | same |
| c | `/api/activate` never returned `expires_at` at all, so the date only arrived on the first heartbeat | `server/pb_hooks/activation.pb.js` |

**The wire mismatch was invisible to every existing Rust test**, because none of
them looked at the serialized bytes: `classify_subscription` was tested against
the enum, and the enum was correct. What the frontend received was
`{"days_remaining":12,"expires_at":"…"}` while `src/services/locus.ts` declares
`daysRemaining`/`expiresAt`. A new test now pins the wire shape
(`the_active_wire_shape_is_camel_case`) and **fails** if `rename_all_fields` is
removed — verified by removing it and watching the test report the exact payload.

Fault (c) is why the row did not appear *at all* after activating: the only writer
of the stored date was the heartbeat, so a freshly activated device had no date
until a beat landed (a floor of five minutes), and the row renders only for
`state === 'active'`. `/api/activate` now returns `expires_at` on both success
paths (fresh bind and same-device re-activation), and the client stores it during
`locus_activate`. The heartbeat still writes it, so renewals keep working without
a restart.

### 2. "Check for updates automatically" was miswired and had no effect

| Severity | 🟡 Toggle showed and edited the wrong setting |
|----------|----------------------------------------------|
| **File:** | `client/src/pages/account.tsx` |
| **Issue:** | The row labelled "Check for updates automatically" read and wrote **`enable_auto_launch`** (start-at-login). It therefore displayed the auto-launch state under the wrong label, and toggling it changed autostart — never update checking. Separately, `auto_check_update` was **dead config**: declared, defaulted and patchable, but read by no code, so even a correct toggle would have done nothing. |
| **Fix:** | The row now binds `auto_check_update`, and `locus::runtime::record_update_offer` reads it before recording an offer. `unwrap_or(true)` keeps automatic checking **on by default** (the product decision: fresh installs pick up fixes without being told), and an absent value — a fresh or migrated install — resolves to true rather than opting the device out. A separate **Auto Launch** row was added so `enable_auto_launch` (which is honoured by `core/autostart.rs`) keeps a control instead of losing its only one. |

The update offer is delivered by the heartbeat, so this setting gates the
automatic *prompt*; the manual "Check status now" path is unaffected, because that
is an explicit request rather than an automatic one.

---

## MIGRATED THE CIPHER STACK TO SHADOWSOCKS 2022 (2026-09-28)

Three proposals were put to the project: many-ports-per-tier with rotation,
modernising the cipher, and adding SS2022 padding. The first was rejected and the
other two implemented together.

### The rejected proposal, and why (recovered here — it previously lived in a separate file)

The full analysis was in `docs/CIPHER-AND-PORTING-REVIEW.md`, **deleted 2026-09-29**.
The rejection reasoning is recorded here because it is the only place it exists and
this proposal is attractive enough to be re-proposed by anyone who has not seen it.
The proposal was: N listening ports per tier, each client handed a list, one port
chosen per client and rotated on a slow timer, gated on low traffic.

**It has a real theoretical case.** A stable `(client IP, server IP, port)` tuple
carrying 200 Mbps for six hours is a beacon; rotating the port resets
connection-tracking state and defeats naive "long-lived high-volume flow on a
nonstandard port" heuristics. The idle-gated trigger is a thoughtful detail.

**It was still rejected, for five reasons, one of which is fatal:**

1. **It destroys the traffic-shaping model.** The tc cap classifies on **source
   port** — `match ip sport 8443 flowid 1:10` in `04-tc.sh`. A port *is* the
   tier's identity. With 16 ports per tier you either grow `04-tc.sh` from 3
   filters to 48 (each needing a class *and* an `fq_codel` leaf), or you can no
   longer tell which tier a connection belongs to — so a free-tier student
   reaches the 200 Mbps range. That is a revenue leak and a broken tier promise.
2. **It multiplies the firewall surface for no gain.** UFW opens exactly
   8443/8444/8445 (+8446). A contiguous open range is the shape port-scanning
   automation flags, and this deployment already records benign scanner probes on
   the existing ports.
3. **It does not defeat the thing N4L actually does** — the decisive argument, and
   the same one that chose Shadowsocks: N4L fingerprints and blocks *protocols*,
   not flows, and cannot do volume-based VPN detection on an AEAD stream that is
   already indistinguishable from random. It defends against an attacker there is
   no evidence of facing, at real cost.
4. **A port list is node selection by another name**, which the 3.1.0 rework
   deliberately deleted ~4,900 lines to remove. The egress check has no concept of
   a "current port": `via_locus == reported IP == the stored tier server`.
5. **P2P is already addressed by UoT on 8446**, and rotating mid-session is exactly
   what breaks an established UoT association — the "wait until idle" trigger is an
   admission of that.

**This was possible as a clean-slate change** because there is no fleet and the
server is not deployed, which removed the version-floor and rollback constraints
that would otherwise have forced an additive, version-gated migration.

### What changed

**Cipher: `aes-256-gcm` (legacy AEAD) → `2022-blake3-aes-256-gcm` (Shadowsocks
2022)**, on all three tiers and the sing-box UoT inbound. SS2022 is a different
wire protocol, not a cipher-name swap: it brings an encrypted fixed-length header
and mandatory replay protection, which is what makes the stream harder to
fingerprint.

**Keys: passphrases → SS2022 keys.** SS2022 requires the base64 encoding of
exactly 32 bytes. The old values were `openssl rand -hex 16` (16 ASCII chars),
which SS2022 **rejects outright**. Generation is now
`openssl rand -base64 32 | tr -d '\n'`; the `tr -d` strips the newline openssl
appends, because a key carrying one fails at handshake time with no useful error.

**The cipher is now defined once.** It had been duplicated in ten places — three
templates, two config writers, four seed/publish scripts and a cosmetic print —
with nothing binding them. That is the same defect class as the
`uot_port` / `server_port_uot` mismatch (entry 29 below): two definitions that
agree today and drift silently later. `SS_METHOD` (and `SS_KEY_BYTES`, derived
from it) now live in `server/modules/00-env.sh` and every other site reads them.

**Padding: enabled.** SS2022 applies handshake-scoped padding internally; it is
not a config key, so no config surface was added — which matters because the
sing-box inbound rejects unknown keys hard.

### Failing loudly instead of silently

Both new failure modes are caught at deploy time rather than on a student's
machine:

- **Module 00** rejects a key whose decoded length is not `SS_KEY_BYTES`, and
  rejects an unsupported `SS_METHOD` outright (the legacy AEAD ciphers are
  deliberately excluded from the allowlist).
- **`02-shadowsocks.sh`** re-checks the same thing, because it is runnable
  standalone (`bash 02-shadowsocks.sh`).

Without these, an old-format key would produce a host that looks deployed, serves
clients, and refuses every handshake.

### sing-box stays pinned at v1.12.1 — deliberately

It was **not** bumped, and the reason is recorded in `02-shadowsocks.sh`: v1.12.1
already supports `2022-blake3-aes-256-gcm`, and the shadowsocks inbound has no
padding field to set. A version bump would therefore add risk (the pin exists for
a recorded reason; the module warns that unknown keys kill the daemon) without
adding any capability. If it is ever bumped, that is its own change with its own
UoT end-to-end re-test.

### Verified

- `cargo test`: **514 passed**, 0 failed.
- **`tier_profile_engine` (3 passed)** — the real mihomo v1.19.31 accepts
  generated profiles carrying the SS2022 cipher, for both the Eco (TCP-only) and
  Strike (UoT) paths. This is the check that matters: a config can pass every
  unit test and still be refused by the engine.
- Both generated configs (tier + UoT) parse as valid JSON with the new cipher.
- Key validation exercised against real values: a base64-32 key decodes to 32
  bytes and passes; the old hex-16 form decodes to 24 and is rejected.

**Not verified here:** a live client↔server round trip. That needs a deployed
host; `server/scripts/verify-ss2022.sh` is the tool for it. Until it runs, the
honest status is *built and engine-validated, not yet proven over the wire*.

---

## A SUSPENDED CODE SAID NOTHING, AND THE UPDATER WAS DEAD CODE (2026-09-28, third round)

Reported, in the user's words:

> when a code is suspended, the client has no indication that it's happened, and
> just gets a little message saying something along the lines of no code, find it on
> card, then needs to close and reopen locus to get to the code menu. Then, the
> updater is not working, on the client app there is no way to manually trigger an
> update, nor does the automatic updater work. The original clash verge rev has a
> little pop-up saying it's time to update, the logic should still be present, just
> not working.

Both reports were exact, including the workaround.

### 1. A suspension produced the wrong message, and needed a restart to see

Three faults stacked, and all three had to be fixed for the student to learn
anything.

- **The reason was rendered where it could not be seen.** The previous round added
  a `refused` state carrying the hub's own sentence, and rendered it inside the
  *connection page*. But a refusal withdraws the entitlement, so `main.tsx` gates
  the whole app to `ActivationScreen` — the connection page never mounts. The one
  message written to explain a suspension was unreachable in every case where a
  suspension had happened.
- **The activation screen did not know about it.** It showed "Enter the activation
  code from your card", which is an instruction to re-enter a code the student
  already has and which the hub has already refused.
- **`main.tsx` read the entitlement exactly once, on mount.** A heartbeat that
  cleared the entitlement hours later changed nothing on screen. The only way to
  see it was to close and reopen the app — which is precisely the workaround the
  report describes.

**Fix:** the refusal is now the activation screen's first message (the hub's
sentence, verbatim — an operator writes an instruction, and paraphrasing it would
replace an action with a description), and `main.tsx` re-reads the entitlement on
the same interval as the subscription indicator, so a refusal moves the app to the
activation screen by itself.

The priority rule was extracted to `src/pages/connect-notice.ts` as a pure function
and unit-tested, because *which* message wins is the thing that was wrong, and a
rule that can only be exercised by mounting React against a Tauri mock is a rule
that regresses unnoticed. Mutation-tested: restoring the old priority fails 4 tests.

Two things were deliberately NOT done. A transient read failure no longer flips a
running device to unactivated — only a confirmed refusal does, because a momentary
storage hiccup must not present as a surprise logout. And the generic card prompt
now stands down when a refusal is shown, rather than being displayed alongside it.

### 2. The updater was fully implemented, fully tested, and called by nothing

This is the whole of "the updater is not working", and it was not one bug but
four, each of which alone was enough to break it:

1. **The hub's signal was discarded.** `runtime.rs::handle_success` received
   `update_available` and did `logging!(info, ...)` — nothing else. The comment said
   the signal was "recorded, not installed"; nothing recorded it.
2. **The plugin was never registered.** `tauri-plugin-updater` is in `Cargo.toml`
   and `locus/update/install.rs` calls `app.updater_builder()`, but it was absent
   from `setup_plugins()`. So the one code path that reaches it would have failed at
   runtime — and `cargo check` cannot see a missing `.plugin(...)` registration,
   which is why this survived.
3. **No capability permission.** `capabilities/desktop.json` had no
   `updater:default`, so the plugin's commands would have been denied.
4. **Nothing called the decoder or the installer.** `signal::decode` and
   `PendingInstall::check` were exported, documented and unit-tested, and referenced
   nowhere. The only registered update command was `locus_update_staging_dir`.

**Fix:** the plugin is registered (reading pubkey and install mode from
`tauri.conf.json` rather than duplicating them), `updater:default` is granted, the
offered version is actually recorded on a successful beat, and the missing commands
exist — `locus_update_status`, `locus_install_update`, `locus_dismiss_update`. The
prompt is the Clash Verge one the user remembered, rebuilt on Locus's own
hub-mediated updater.

The offer rule was extracted as a pure function and tested, because the *silent*
cases matter as much as the loud one: prompting for something that cannot be
installed sends a student into a download that ends in a verification failure. So a
version that is not newer is refused (a stale `update_config` row must not advertise
a downgrade), and so is one with no artifact for this platform. Mutation-tested:
removing the downgrade gate fails a test.

Installing is never silent, and never automatic. The download replaces the running
binary, so it drops the connection — doing that unasked would cut off a student
mid-session, which on a school network is the worst possible moment.

**No server work was needed.** The hub side was already complete and correct:
`/api/update` serves the plugin's dynamic manifest shape, returns **204** for
"up to date", refuses partial offers, and `publish-release.sh` requires and records
`signature_<platform>`. The CI signs the bare executables with minisign directly
(`createUpdaterArtifacts` is deliberately false, and the workflow documents why).

---

## THE BUTTON STILL LIED, THE EXPIRY STILL DID NOTHING, AND THE CHECK CAME OUT (2026-09-28, second round)

Reported after the previous round was committed and tested:

> the button STILL doesn't have a "connecting" state … In best-case-scenario it
> causes quite a while of no internet before traffic starts being routed where both
> download and upload reports 0. Worst case scenario, it actually reported
> "connected" even though there was no wifi … the expiry/subscription thing still
> isn't working … I don't think there's an indicator either way … you have NO
> POSSIBLE WAY of checking subscription status, when you need to renew … the
> "Connection Check" thing is just hopeless and completely broken, remove it from
> the homepage

All three are real. The previous round fixed the *front-end* rule and the *hub*
guard, and both were correct — but neither was where the lie was coming from, which
is why the bug survived a green test run. This is the round that found the actual
causes.

### 1. "Connected" was a latch, not an observation

The front-end rule was already right: `connected && !ready -> connecting`. The
problem was `ready`.

`CoreManager::is_core_ready()` answered by reading a bit that `mark_core_ready()`
set **the instant the start call returned**:

```rust
service::run_core_by_service(config_file).await?;   // "start it"
self.mark_core_ready();                              // "it's ready" — immediately
```

Nothing ever re-observed the Core. The bit was only cleared by an explicit
`core_stopped()`. So `ready` meant *"we once asked it to start"*, not *"it is
working"*, which produces exactly the two reports:

- **Connected, no wifi.** The process exists, so the latch is set, so `ready` is
  true, so the app claims a tunnel on a machine that cannot carry a packet.
- **Connected with 0/0 traffic for a long window.** A Core that bound nothing still
  satisfied the latch.

There was no health probe anywhere in the readiness path — no port check, no API
ping, no evidence of any kind.

**Fix:** readiness is now observed. `core/manager/probe.rs` asks the Core's own
control API (`mihomo /version`, via the plugin, so the transport — HTTP controller
or sidecar Unix socket — is whatever the rest of the app already uses). Rules:

- only a **serving** Core is `Ready`; the latch is necessary and no longer
  sufficient (`decide`);
- a failed probe **revokes the latch** (`revokes_latch`), because the latch is read
  from several places that have no probe of their own — reporting `NotReady` without
  revoking would leave every other reader seeing the stale `true`;
- `PROBE_TIMEOUT` is 400 ms and applied *around* the call, because the plugin builds
  its client with no timeout and a hung probe would freeze the button exactly when
  it is being watched;
- a Core that should be up but is not answering reads `connecting`, not
  `disconnected` — work is in flight, and flashing "not connected" at a Core that is
  still coming up was the other half of the original complaint.

`locus_connect` also waited on the latch, so it could resolve `connected: true` for
a Core that had bound nothing. It now waits on `observe_readiness`.

The old public `is_core_ready` is gone; the latch survives as a private
`core_readiness_latched`, consumed only by `observe_readiness`.

Mutation-tested both ways: making the latch sufficient again fails 3 tests; making
`revokes_latch` always false fails 1.

### 2. Expiry produced no *visible* outcome, even when it worked

The previous round correctly made the hub return 410 and the client treat 410 as a
refusal. But a refusal called `store::clear()`, which erased the entitlement — and
with it every trace of *why*. The student was left with an app that had silently
deactivated itself: indistinguishable from a fresh install, and explaining nothing.
"Your subscription expired" and "an operator suspended your code" need different
actions and looked identical (i.e. invisible).

Two more gaps sat behind it:

- **Nothing could ask the hub on demand.** The client learns its expiry from a
  heartbeat whose interval is a floor of **5 minutes** (up to **2 hours** after
  failures). There was no way to trigger one, so "check my status" could only
  re-read a local copy that provably had not changed.
- **The sidebar chip rendered nothing** when the state was `unknown` — and `unknown`
  is also "activated, but the hub has never confirmed a date". A student whose status
  could not be read saw *no indicator at all*, which is the "NO POSSIBLE WAY of
  checking subscription status" report, precisely.

**Fix:**

- `store::record_refusal` writes the hub's own sentence before `clear()` runs (order
  matters — a crash between the two leaves an *explained* refusal, never an
  unexplained one). `store::clear` now also clears the stale `locus_expires_at`,
  which previously lingered beside a code that no longer existed.
- `SubscriptionStatus::Refused { reason }` is checked **before** the unactivated
  fallback in `classify_subscription`, because a refusal always leaves the device
  unactivated — testing `activated` first would make the state unreachable.
- The chip now says something whenever the device is activated *or* refused: a
  refusal shows the hub's own words, and an activated-but-unconfirmed subscription
  shows "status unavailable" rather than vanishing.
- `HeartbeatLoop::beat_now` + `locus_check_subscription` let the student ask the hub
  now; the Account page gains a "Check status now" action and a
  `last_confirmed_at` line, so "expires in 30 days" is no longer indistinguishable
  between confirmed-a-minute-ago and confirmed-last-week.

Mutation-tested: removing the refusal branch fails 2 tests.

### 3. The Connection Check is removed

Agreed with the report. It compared a live egress reading against a stored
tunnel-down "baseline", and on a school network the public address can be the *same*
before and after (shared NAT/proxy) — while with no baseline recorded it rendered
"cannot confirm it is Locus", the useless text being complained about. It answered a
question the Connect button should answer, and could not answer it reliably. The
whole stack is gone: component, hook, `locus_check_ip`, `locus/egress.rs`, the
`locus_egress_baseline` preference, and the 20 `check.*` keys in all 13 locales.

Removing it also retired the last user of the readiness *latch*, which is what made
the latch's dead code visible.

---

## SUBSCRIPTIONS NEVER EXPIRED, AND NOTHING TOLD THE STUDENT THEY WERE ABOUT TO (2026-09-28)

Reported plainly: *"the expiry for a client absolutely does not work — I purposely
expired one of the codes tied to my test device, it's connecting without the
slightest hiccup."* Then, as a second requirement, that a student is never told
about renewal or expiry anywhere in the app.

Both were true, and the causes were four independent holes, each harmless on its
own and a full bypass together. Fixing any one would have looked like progress
and changed nothing.

### 1. The hub never checked expiry on the heartbeat

`activation.pb.js` checks `expires_at` and returns **410 "Code expired"** — and its
own comment says why, having already fixed this class of bug once:

> Every expiry check used to read `new Date(exp)` … and because the parse ALWAYS
> returned NaN, the isNaN guard was always taken and the comparison NEVER RAN — so
> no code ever expired.

`heartbeat.pb.js` never got that check. It verified `suspended` and nothing else,
so a device that was already bound beat successfully forever. Expiry could only
ever bite a **new activation** — never a running client, which is exactly the
machine it most needs to bite. Fixed by adding `parsePBDate` (copied verbatim from
`activation.pb.js`, defined inside the callback because goja does not hoist across
scopes) and returning 410 when the date has passed.

The guard is deliberately shaped so a **missing or unparseable** date does not
lock anyone out: `!isNaN(expMs) && expMs > 0 && expMs < Date.now()`. A permanent
code parses to 0 and never expires; a hub that cannot read its own date treats it
as "no expiry" rather than "expired". This is the one place the safe default is
"allow", because the alternative silently ends paying students' subscriptions.
Verified by exercising the parser and guard under Node against eight inputs
including the PB space-separated form, `null`, whitespace and garbage.

### 2. The client filed "expired" as a network blip

`heartbeat.rs` treated `403 | 404` as a definite refusal and everything else as
retryable. But the hub's status for an expired code is **410** — so even once the
hub *did* refuse, the client bucketed it as `Unreachable`, backed off, and retried
as though the school network were flaky. The tunnel stayed up.

410 now joins the refusal set, and the classification was extracted into a pure
`classify_response(code, text)` so the whole mapping is testable without a
network — the same treatment `egress::classify` got. Mutation-tested: removing 410
fails two tests.

### 3. The client never checked expiry before connecting

`locus_connect` gated on TUN capability and tier config, and nothing else. The
lapsed state existed (`classify_subscription`) but fed only the Account screen's
*display*. So a lapsed code sitting in local storage started a tunnel that then
worked until the next heartbeat happened to be refused.

Added `subscription_allows_connect`, a hard gate at the top of `locus_connect`,
before anything is written: **only a known past date refuses.** `unknown` allows,
because a fresh install or a hub that never reported a date must never be read as
expired. The refusal is `LOCUS_SUBSCRIPTION_LAPSED` with an actionable sentence,
shown by the connection screen the same way the TUN refusal already is.

### 4. The grace period was computed but never enforced

`Backoff::remaining_grace` had thorough tests and no caller. Nothing stopped a
tunnel when the window since the last good beat was spent, so a device that could
not reach the hub ran indefinitely on an entitlement nobody had confirmed. Now
`handle_outcome`'s `Unreachable` branch calls `enforce_grace_period`, which stops
the core and clears the entitlement once the window is exhausted. A device that has
never beaten gets the full window, not zero — it is a fresh install, not a lapsed
one.

### The deliberate split: hard gate on connect, grace on a live tunnel

These two look contradictory and are not. A lapsed code **cannot start a new
connect** — no grace, refused in Rust before any state changes. A tunnel **already
up** keeps working until grace runs out, so a student is not dropped mid-game by a
clock. That is what the two decisions (enforce everywhere; honour grace) actually
mean together, and it is the only arrangement where a renewal lapse stops new
access immediately without punishing someone for the school's network.

### 5. Expiry is now visible everywhere, not just on Account

The subscription was shown on one page a connected student has no reason to open.
`useSubscription` is one shared query (deduped through the existing SWR layer) that
three surfaces read, so they cannot disagree: the **sidebar** shows a permanent
urgency-coloured chip (grey / amber in the last week / red when lapsed) next to the
traffic figures, present on every page; the **connection screen** shows a warning
line for the urgent and lapsed cases; **Account** keeps its detailed view.

Silent in exactly two cases, both correctness: not activated (nothing to describe),
and unknown (no date to describe). A badge that says "expires in ?" trains a
student to ignore it, and then the warning that matters is noise too. Three new
locale keys × 13 locales added and regenerated.

### Verified

- `cargo test`: **514 passed** (+9: heartbeat classification ×4, expiry lapse/state
  ×2, connect gate ×3).
- **Mutation-tested**: removing 410 from the refusal set fails 2 tests; neutering
  `subscription_allows_connect` fails 2 more. The verifiers can fail.
- `cargo clippy --all-targets`: **5 errors, all pre-existing** and in untouched
  files — down from 8, because the `expiry` and `heartbeat` test modules now carry
  the same `#[allow(clippy::panic)]` the other modules use.
- `tsc`/`web:build`, `lint --max-warnings=0`, `vitest` 39, i18n 13 locales
  `missing=0`, header guard exercised under Node.
- Real `verge-mihomo -t`: 3/3 tier profiles still accepted.
- **Not verified here:** the live end-to-end — an actual expired code refusing a
  real heartbeat, and the badge's appearance on a real screen. This host is
  headless. Recorded in `STILL-OPEN.md`.
- **Deploy note:** the hook change reaches the live hub only on a `setup.sh`
  re-run (hooks deploy from `/root/server/`, not the repo). Until then only the
  client-side gate bites.

---

## FOUR HONESTY BUGS, AND THE SIDECAR THAT CAUSED TWO OF THEM (2026-09-27)

Reported as four separate problems. Two of them were the same defect wearing
different clothes: **the app asserted a state it had not verified**. The third was
the same defect in the UI's timing, and the fourth was a layout that refused to
admit its content was taller than the window.

### Bug 1 — "the first open shows Connected, but it is not"

Reported as: open the app, it says Connected; press Disconnect and it says the
**service is missing and needs installing**, which reveals the tunnel was never up.

That sequence is only possible through one path, and it was the **sidecar**: a
non-service way of running the core, as a child of the app process. It set
`RunningMode::Sidecar`, which `core_is_running` (added the previous round) counted
as "connected" — so the app advertised a tunnel that had no service behind it, and
the next disconnect discovered the service was absent.

**The sidecar is removed entirely, not hidden.** The product decision is that the
service is the only supported way to run the core. That is now enforced by the
type system rather than by convention: `RunningMode` has two variants (`Service`,
`NotRunning`), so there is no longer a second kind of "connected" to disagree
about. Removed with it: `StartupDecision::Sidecar`, `ServiceStatus::SidecarAllowed`,
`RunState::sidecar_allowed` and the allowance/withdraw/restore machinery,
`continue_with_sidecar`, the handoff watcher, the Windows Job Object the sidecar
used, the migration dialog's "Continue with Sidecar" escape hatch, and the tray's
notion of a second mode.

Note what this *deletes* rather than fixes: the class of bug where "connected"
meant two different things. A future edit cannot reintroduce it without adding a
variant back.

### Bug 2 — "the check says traffic is going through Locus when the VPN is off"

The check compared the echo service's address against **this machine's own
addresses** and reported "routed" when they differed. "Not one of my addresses"
does not mean "through Locus" — it also means carrier NAT, a school proxy, a
different route, or an unreadable interface list (the old code returned `true`,
"routed", when it could not enumerate interfaces at all).

The verdict is now built from `EgressCheck`, which can only state what it
established:

| situation | verdict |
| --- | --- |
| reported address **is** the tier's server | going through Locus |
| reported address is one of ours | not tunnelled at all |
| neither, and a tier server is stored | leaving by another route |
| neither, and **no** tier server to compare | checked, unconfirmed |

The last row is the bug. Only an equality against the tier server can prove the
claim, so only that equality is allowed to make it — `via_locus` is reachable
solely through that comparison, and the "no server to compare" case is its own
state rather than a reassuring default. The local address travels with the verdict
so the screen can show both addresses instead of asking anyone to take it on trust.

> **Superseded 2026-09-28 — see §3a below.** The equality above was itself the next
> bug: the stored server is a **hostname**, so `server.parse::<IpAddr>()` never
> matched and every run took the "no comparison" row. The comparison is now a
> difference against a recorded tunnel-down baseline, not an equality with the
> server. The table is kept as the record of what was tried.

### Bug 3 — "the button says Connected while the tunnel is still coming up"

`locus_connect` resolved when `start_core()` returned, which is when the engine
process was *spawned*. Binding ports and dialling the server takes seconds more,
so the button announced a tunnel that was still dialling, and a student who checked
their address in that window was told they were not connected by a tunnel that was
merely unfinished.

- `LocusStatus` gains `ready` next to `connected`. They are different facts on
  purpose: `connected` is "the engine process exists", `ready` is "it can carry a
  packet". `CoreManager::is_core_ready()` is the new public predicate.
- `locus_connect` now waits for readiness, bounded at 30s. A timeout is a
  **result**, not an error (`connected: false` plus the backend's own sentence), so
  the screen can say "still starting" instead of hanging.
- The button is **cancellable while connecting**: `locus_cancel_connect` sets a
  one-shot flag the connect loop consumes, stops the half-started core, and returns.
  The retired client's failure was a transition that could not be abandoned; the
  guard against repeating it is that the connecting button stays pressable and
  means "cancel", and a 6s threshold adds a "still connecting" line so slow is
  distinguishable from stuck.

### Bug 4 — "no scrollbar"

`.layout-content__right .the-content` is `position: absolute; inset: 0` with no
`overflow`, inside a parent chain that is `overflow: hidden` the whole way up. The
`page.scss` rule that does scroll (`overflow: auto`) only applies to pages rendered
through `BasePage`, which the connection and account screens are not. Content past
the fold was clipped with no way to reach it. Fixed with `overflow-y: auto` and
`scrollbar-gutter: stable`, plus a 4px right inset so the bar does not sit under
the 6px resize handles.

### Also fixed in the same function

`locus_connect` called `apply::apply_tier(&config, false)` — **`udp_relay`
hardcoded to `false`**. Every tier therefore built a TCP-only profile, and the
Strike tier's UDP-over-TCP endpoint was never used: game and voice traffic went out
as raw UDP onto a network that drops it, on the tier sold for games. It now reads
the flag from storage (`store::udp_relay()`), where activation put it.

### Verification

`cargo test` 493 passed (was 492 before this round, +1 net after removing the
sidecar's tests); `cargo clippy` no new lints (97 vs the 99 pre-existing on the
baseline — the remaining ones are in test modules); `tsc` clean; `pnpm lint` clean;
`web:build` clean; `vitest` 38 passed; `i18n:check` 0 missing, 0 unused.

**Not run on real hardware.** This host is headless Linux, so the Windows
elevation prompt, the tray Connect/Disconnect item, the service-required install
flow, and the visual layout of the new verdicts and scrollbar are
built-and-verified-but-not-run.

---

## WINDOWS CI COULD NOT CHECK OUT THE REPO — 62 FILES WITH A COLON IN THE NAME (2026-09-27)

The Windows job failed at `actions/checkout@v4`:

```
error: invalid path 'locus-icons-todo-implement/locus-icons/src-tauri/icons/128x128.png:Zone.Identifier'
The process 'C:\Program Files\Git\bin\git.exe' failed with exit code 128
```

### What this was NOT

The first explanation offered (and the one worth recording, because it is the
plausible wrong answer) was that `128x128.png` "has an associated `Zone.Identifier`
alternate data stream". **That is impossible and was not the cause.** An NTFS
alternate data stream is invisible filesystem metadata on a file; it never
appears in `git ls-files` and cannot appear in a checkout error, because it is
not a path. Acting on that diagnosis (`Remove-Item -Stream Zone.Identifier`,
`core.longpaths`, `.gitattributes * -text`) would have changed nothing.

### What it actually was

**62 tracked files whose literal filename ends in `:Zone.Identifier`**, with
contents `[ZoneTransfer]\nZoneId=3` — the Windows "downloaded from the Internet"
marker, materialised as real files. Copying an icon folder off a Windows machine
turned each ADS into a sibling file named `<original>:Zone.Identifier`; those got
committed. A colon is illegal in a Windows path component, so **Git for Windows
cannot create the file and refuses the entire checkout** — which is exactly why
the failure is at checkout and only on `windows-latest`.

### The fix, and why the obvious one is incomplete

All 62 came in with commit `806298c` ("escalation fix"), which also added a whole
**147-file `locus-icons-todo-implement/` directory**. That directory turned out to
be a pure accident: unreferenced anywhere in the repo, and every one of its real
files **byte-identical** to an existing file under `client/` (verified by
comparing blob hashes against both possible counterpart paths — zero unique
files). The whole directory is removed, plus a `.gitignore` rule
(`*:Zone.Identifier`) so a future Windows copy cannot reintroduce them.

**A forward fix does not repair history.** `806298c` is *already pushed to public
`origin/main`*, so every reachable commit from there to `96bec36` still contains
the colon paths and still cannot be checked out on Windows:

```
$ git rev-list --all | <each commit> | grep ':'  ->  62 unique paths
$ git ls-tree -r --name-only HEAD | grep ':'     ->  0
```

Any checkout of the **new** HEAD is clean (which is what the CI job needs going
forward), but checking out an older commit — or a `git bisect` through that range
— still fails on Windows. Purging them from history would need `git filter-repo`
plus a **force-push to public `main`**, which rewrites commits other people may
have pulled. That was **deliberately not done**; it is a separate decision with a
blast radius outside this repo. If it is ever done, the `.gitignore` and the
directory removal are already in place, so the rewrite only has to drop paths.

### The lesson

`git ls-files | grep ':'` is the check that finds this class of bug, and it needs
running after **any** Windows-authored commit — the branch was green on Linux for
a day while Windows could not fetch it at all. And an error that names a file
ending in `:Zone.Identifier` means a file *called* that, not a stream *on* a file:
the colon is the whole bug, and anything that treats it as filesystem metadata
will look right and fix nothing.

---

## THE TRAFFIC DISPLAY WAS TOO THIN, AND "CONNECTED" COULD NOT BE TRUSTED (2026-09-27, 3.2.0)

Two more reports on the same client: *"the download speed and upload speed is way
too simplistic, and the numbers appear too low for non-tech-savvy people … the
original had 'uploaded' 'downloaded' 'core usage' and 'active connections'"*, and
*"add an extra menu on home to check the current ip and whether it's actually
connected because the connect button is quite unreliable"*. Plus a question —
*"confirm that the default is to stay open in the background with TUN"* — which
turned out to be already true and is now written down rather than assumed.

### 1. Four metrics, not two — the original dashboard restored

The 3.1.0 rework had reduced the traffic card to a live up/down speed, which
answers "is it moving?" but not "what has it moved?" or "is the engine healthy?".
The card now carries the four figures the original dashboard had:

| Figure | Source | Why it is there |
|---|---|---|
| Uploaded / Downloaded | `traffic.upTotal`/`downTotal` | what this session has moved |
| Core usage | `useMemoryData` → `inuse` | the engine's own footprint — "is it alive" |
| Active connections | `connect_connections_count` | "is anything actually flowing" |
| Up/down speed (live) | `traffic.up`/`down` | the graph's companion |

`useTrafficSummary` remains the single computation point (the Account screen uses
it too), so the two screens cannot disagree about the same session. A metric we do
not have yet renders as an em dash, never as `0` — "we do not know" and "nothing"
are different facts.

### 2. "The numbers appear too low" — a `B/s` reading looks broken

The live speed was `parseTraffic(847)` → `"847 B/s"`. To someone who has not heard
of a byte, a bare `B` is a mystery and a sub-`1`-unit number is indistinguishable
from "not working" — and an idle page legitimately produces a few hundred bytes a
second, so this is the *common* case, not an edge one.

`utils/format-speed.ts` wraps the shared parser for the one place a human reads a
live speed: sub-kilobyte speeds are promoted to `KB/s` with one decimal (`0.8 KB/s`),
a tiny nonzero speed never rounds to `0.0`, and zero is reported as *idle* rather
than `0 B/s` (a bare zero is what a broken tunnel shows *and* what a quiet one
shows; the graph and the totals are the honest evidence of which). The shared
`parseTraffic` is untouched — it still backs the graph and the totals, where
canonical units are correct.

### 3. A "Connection check" — the one thing the app's own state cannot tell you

"Connected" is a claim about the app; it says nothing about the network. A tunnel
can be up while traffic is not being carried (a stale route, a core that started
and died, a bypass covering the site in question), and the student sees a green
badge next to no internet with no way to tell the two apart.

The new card answers the question directly: it asks a public echo service which
address this device's traffic appears to come from. Because the request is made by
the app, it travels the same path as everything else — through the tunnel when it
is up, directly when it is not — so the answer is the *egress* address, and a
comparison against the machine's own local addresses yields the verdict:

- the reported address is not one of ours → **"your traffic is going through
  Locus"** (it left by some other path);
- it *is* one of ours → **"your traffic is not going through Locus"**;
- the check could not complete → **"could not check"**, with the reason.

Deliberate choices, each because the wrong one is a lie a student would act on:

- **`locus_check_ip` never returns an error.** A failed check is a *result*
  (`EgressCheck::Unknown { reason }`), because the screen must show "we could not
  check" rather than catch an error and render it as "not connected". A test pins
  that every failure sentence avoids the words "not connected"/"disconnected".
- **The reply is validated as an address.** A captive portal or an intercepting
  proxy returns a page of HTML; showing that as the student's IP is worse than
  saying the check failed. Anything that is not a parseable `IpAddr` is a failed
  check, and the failure sentence says something may be intercepting the request.
- **It does not claim Locus is the carrier.** An IP check can prove traffic was
  *routed*, not *by whom*; asserting the carrier would need a comparison against
  the tier's server and would silently stop working whenever DNS for the hub host
  failed. "Your traffic is going through Locus" is backed by "the address is not
  one of ours", which is what was actually measured.
- **It is manual, not polled.** It contacts a third party, so running it on a
  timer would be wasteful and a small privacy cost for a number that only matters
  on demand. There is a "Check now" button.

The egress probe lives in `src-tauri/src/locus/egress.rs` (9 tests) and is exposed
as `locus_check_ip`; the card is `components/connection/connection-check.tsx`.

### 4. "Does it stay open in the background with TUN?" — yes, and now it is written down

Confirmed by reading the code, and needing **no change**:

- closing the window calls `handle_window_close`, which does `api.prevent_close()`
  and then hides the window to the tray — it does not exit;
- the core (and therefore TUN) is stopped **only** on an explicit quit
  (`feat::quit`) or an OS session end, never on window close;
- the tray is created at startup, so there is somewhere for the app to go;
- `enable_silent_start` defaults `false`, so a normal launch shows the window;
- `auto_close_connection` is unrelated — it closes *proxy* connections when the
  proxy mode or core changes, and has nothing to do with window close.

So the default is already "stays running in the tray with the tunnel up". This is
recorded in `client/docs/ARCHITECTURE.md` so the next person does not have to
re-derive it to answer the same question.

### 5. Found and fixed in passing: the version bump was half-broken

`pnpm release-version` failed with *"clash-verge package entry was not found in
Cargo.lock"* — the script still matched the **upstream** package name, while the
workspace package is `locus`. It had already written `package.json` and
`Cargo.toml` before failing, so the repo was left with version sites mid-bump.
The pattern now matches `locus`, so a bump cannot again leave the lockfile behind.

### Verified

- `cargo test`: **511 passed** (+9 for the egress module), `version_consistency` 3.
- `cargo clippy`: zero warnings.
- `tsc`, `pnpm run lint` (`--max-warnings=0`), `pnpm run web:build`: clean.
- `vitest`: **37 passed** (+5 for `format-speed`).
- `pnpm run i18n:check`: **zero missing** in all 13 locales (13 new keys added to
  each, English included).
- **Version 3.2.0** across all four sites (`package.json`, `Cargo.toml`,
  `Cargo.lock`, `tauri.conf.json`).
- **Not verified here:** the cards' appearance on a real screen, and the egress
  check's behaviour on a school network (the case it exists for — an intercepting
  portal — is covered by a unit test, not by a live run). This host is Linux and
  headless.

### 3a. Correction: the connection check above could never confirm anything, and there was no connecting phase

Two defects, reported from real use and fixed together because the second one
made the first unworkable.

**The check was structurally incapable of its only useful verdict.** It compared
the egress address against the tier's configured `server` value. That value is a
**hostname** — the hub hands out `networkingguides.duckdns.org`
(`server/scripts/seed-live.py`, `fix-tier-configs.py`) — and the code did
`server.parse::<IpAddr>()`, which never succeeds for a hostname. So the comparison
was unreachable: every run fell through to `Unverifiable`, and the card said
"Checked, but cannot confirm it is Locus" whether the tunnel was up, down, or
carrying traffic. This is exactly the reported symptom, and it was not a UI
problem — the backend comparison literally never ran.

Even had it been an IP, equality is the wrong test: the VPS's inbound and
outbound addresses need not match, and a client behind carrier-grade NAT has an
egress address the server never records.

**The fix is a measurement of change, not an equality.** The check now records a
**baseline** — one egress reading taken while the backend is certain the core is
*not running* — and a later reading that **differs** from it is proof the traffic
has moved to a different path, which with the tunnel up is what "Locus is carrying
this" means. No DNS, no server change, no hub coupling; it survives CGNAT and split
addresses because it never has to name the server. A reading that **matches** the
baseline is "leaving the same way as before"; **no baseline yet** stays its own
`unverifiable` state, so "we cannot tell" is never rendered as "not routed". The
baseline is persisted (`IVerge.locus_egress_baseline`) so the check still works
after a restart, and is only written while the core is down — a reading taken
*through* a live tunnel would compare equal to a later one and be reported as "not
routed".

The verdict is now a pure function, `egress::classify`, so the whole matrix is
testable without a network: tunnel-down → baseline; address is ours → not
tunnelled; no baseline → unverifiable; differs → routed; matches → not routed.
Six tests cover it, and a mutation restoring the old "not-ours-means-routed" logic
was confirmed to fail two of them.

**There was no connecting phase.** The backend already publishes both `connected`
(the engine process exists) and `ready` (ports bound, can carry a packet), but
`phaseFromStatus` consulted only `ready` and sent everything else to
`disconnected`. Worse, the UI could only *enter* `connecting` optimistically from
the local connect command — never from a status read — so on app open a core left
half-dialled by a previous session settled straight onto a green state with no
transition at all. `phaseFromStatus` now derives `connecting` from
`connected && !ready`, which is a backend fact; the poll guard was corrected so a
failed or dead connect settles to `disconnected` instead of stranding the button
in `connecting` forever. `phase.test.ts` pins all four cases, and removing the new
branch was confirmed to fail two of them.

That makes `connecting` reachable two ways — a connect the student started, and a
half-dialled core found on app open — so `toggle()` distinguishes them with an
in-flight ref: a press with a command running cancels it, while a press with none
takes the half-formed tunnel down (a disconnect). Without that, the second case's
press would set a cancel flag nothing was watching, and the button would look dead.

**Verified:** `cargo test` **505 passed** (+6 for the verdict matrix); `cargo
clippy --all-targets` unchanged at the 8 pre-existing errors, none in the touched
files; `tsc`/`web:build` clean; `pnpm run lint --max-warnings=0` clean; `vitest`
**39 passed**; `pnpm run i18n:types` regenerated (3 new keys × 13 locales);
real `verge-mihomo -t` accepts all three tier profiles.

**Not verified here:** the check end-to-end on a school network — the
disconnect→baseline→connect→differ sequence needs a real tunnel transition, which
this headless Linux host cannot produce. Recorded in `STILL-OPEN.md`.

---

## CONNECT DID NOTHING, AND THE APP NEVER ASKED FOR ELEVATION (2026-09-27)

Two reports, one session: *"the app fails to request UAC every time it's opened
(after installation as well)"* and *"when I press connect, the extra info … flash for
an instant then disappear, meaning the connect button does absolutely nothing."*
Both were real; the second was a genuine product-breaking bug that the earlier
"connect path works" work had not caught, because it is invisible to anyone testing
the backend directly.

### 1. The connect button "did nothing" — a UI that guessed instead of asking

The traffic panel (up/down speed and the graph) renders only when the connection
phase is `connected`. On a successful press the screen did this:

```ts
setPhase(result.connected ? 'connected' : 'disconnected')  // panel appears
void refresh()                                             // panel disappears
```

and `refresh()` contained:

```ts
next.activated ? 'disconnected' : 'unknown'
```

`locusStatus` carried entitlement (`activated`) but **no tunnel state at all**, so
`refresh` — which runs immediately after every connect and again every 15 s — had no
way to express "connected". It concluded `disconnected` unconditionally, on an
activated device, every time. The panel therefore appeared for exactly one frame and
was then wiped, and no amount of waiting brought it back. This is precisely the
reported "flash for an instant then disappear", and it also means **the connection
screen could never stably show a live tunnel**, even when one was up.

**Fix, in two halves, because one half alone would be a patch over a guess:**

- **The backend now answers the question.** `LocusStatus` gained a `connected:
  bool`, derived from the run state's `RunningMode` (`Service` and `Sidecar` both
  count; `NotRunning` is the only "down"). It is the same run state the core manager
  publishes, so the screen renders what is true rather than what it last decided.
- **The screen now asks.** A pure `phaseFromStatus()` (`components/connection/phase.ts`)
  maps `connected → 'connected'`, `activated && !connected → 'disconnected'`,
  `!activated → 'unknown'`, and `refresh` uses it. Pulled into its own module
  importing only a type so it is unit-testable without mounting React against a
  Tauri mock — a rule that regressed once and must not do so silently.

Tests: 4 frontend (`phase.test.ts`, including the exact old-ternary inversion) and 1
Rust (`any_running_mode_reads_as_connected`). `cargo test` 502 (was 501), vitest 32.

### 2. No UAC at launch — the app was `asInvoker`

Only the **NSIS installer** elevated (`RequestExecutionLevel admin` in
`installer.nsi`); the installed app binary had no execution-level declaration, so
Windows launched it as the invoking user and never prompted. The UAC the reporter saw
appeared *mid-session*, when the service install fired — an unexplained prompt seconds
after a window that already looked ready.

**Fix:** the app binary now declares `requestedExecutionLevel level=
"requireAdministrator"` in an app manifest passed to `tauri-build`
(`build.rs` → `WindowsAttributes::app_manifest`). Windows prompts **before any Locus
code runs**, so the privilege TUN needs is granted up front.

**The manifest replaces Tauri's default, it does not merge** — so the default's
Common-Controls v6 dependency is reproduced inside ours. Losing it would break
Tauri's dialog APIs at runtime, on Windows only, which is exactly the class of bug
that ships. DPI-awareness is set to match the installer at the same time.

**The tradeoff, stated plainly:** this makes every launch prompt once. It buys an
honest, predictable prompt at the moment an icon is double-clicked instead of a
mystery one later, at the cost of that prompt being unskippable. The service path is
unchanged and still works — elevation just makes `tun_capable()`
(`is_admin || service_usable()`) true on its own, so the service becomes optional
rather than load-bearing. Auto-launch stays safe: Windows autostart already uses a
**scheduled task** with `RunLevel=HighestAvailable`, not a Run-key entry, so a task
start at logon does not raise a second prompt.

### 3. Two on-demand elevation sites still used the wrong classifier

The same session found that the Windows service-uninstall and UWP-loopback helpers
each called `deelevate`'s `privilege_level()` directly — the classifier already
identified as wrong (it reports `NotPrivileged` for a genuinely elevated token that
is not UAC-split). With the app now *always* elevated, that would spawn a redundant
`runas` — **a second, pointless UAC prompt for an operation the process can already
perform**. Both now route through `is_process_elevated()`.

### 4. A pre-existing clippy failure, fixed in passing

`cargo clippy` had been failing on `utils/resolve/window.rs` (`clippy::panic` in a
`const fn`), which contradicted `client/AGENTS.md`'s "zero warnings" rule — so the
guarantee the docs claimed was not actually holding. The panic is a const-eval-only
guard over colour literals, so it is now an `#[allow]` with that reason rather than a
`Result` threaded through every call site. Clippy is back to zero warnings.

### Verified

- `cargo test`: **502 passed** (+1), `version_consistency` 3 passed.
- `cargo clippy`: **zero warnings** (was 1 error — see 4).
- `pnpm run web:build` (tsc), `pnpm run lint` (`--max-warnings=0`), vitest **32
  passed** (7 files, +1 file).
- **Not verified here:** the manifest's effect on a real Windows launch. This host is
  Linux; the elevation prompt, the Common-Controls dependency and the upgraded
  autostart path all execute only on Windows. The change compiles and the manifest is
  the documented Tauri mechanism, but the honest status is *built, not run on
  Windows* — the same limit this repo has recorded before.

---

## THE APP NOW LOOKS LIKE LOCUS, NOT LIKE CLASH VERGE REV (2026-09-26)

Every previous entry in this file treated the fork's appearance as a text problem —
rename the strings, delete the Verge pages. That was necessary and not sufficient:
the app still *shipped another product's colour scheme*. Buttons were iOS blue
(`#007AFF`), surfaces were Clash Verge grey (`#2E303D`), and the window flashed that
grey before painting.

The brand already existed. `docs/archive/UI-AESTHETICS.md` defines it — a green-black
window, Locus green (`#2EA86A`) as the single accent, tier badges that "sell
themselves" — but it was written for the retired Wails client and **never ported**.
So this was not a design exercise; it was implementing a spec the repo already had.

### The palette is now the default, not an override

`src/pages/_theme.tsx` is the single file `use-custom-theme` reads, and it held
Verge's colours. It now holds Locus's, exported as `LOCUS_COLORS` / `LOCUS_LIGHT`
alongside the tier colours. The existing per-field override is untouched
(`setting.X || dt.X`), so anyone who customised a colour keeps it — only the default
changed, which is what a fresh install and most existing ones see.

**Light mode was not in the spec.** The archived document is dark-only, because the
retired client shipped dark-only. A light palette is derived here rather than quoted,
documented as an addition so it is not later mistaken for the original design. The
light accent is a **darker** green (`#1E7A4A`, ~4.6:1 on white) because `#2EA86A` on
white is ~3.0:1 — fine for fills and large text, short of the 4.5:1 body-text rule.
Using one green for both would have meant either failing contrast in light mode or
dulling the brand in the mode that matters most.

### The white flash had three layers, not one

The spec calls out setting the window background natively so there is no flash. The
fork had Verge grey in **all three** layers that paint during startup:

| Layer | File | Was | Now |
|---|---|---|---|
| Native window (OS-painted, before any content) | `utils/resolve/window.rs` | `#2E303D` / `#F5F5F5` | `#06130C` / `#F4F8F5` |
| Document (parsed before any bundle runs) | `src/index.html` | `#2e303d` / `#f5f5f5` | `#06130c` / `#f4f8f5` |
| App theme | `src/pages/_theme.tsx` | Verge palette | Locus palette |

Plus two stragglers inside the theme builder (`#ECECEC`, `#3E3E3E`, `#2E303D`
dialogs, `#666` scrollbar hovers) that would have left neutral grey furniture on a
green page.

The layers cannot be merged — layer 2 cannot import the theme module, and layer 1 is
Rust — so they are **pinned against each other by tests instead**:

- `tests/theme-colors.test.ts` asserts the document's `--bg-color` equals the theme's
  background, that no Verge grey survives in either, and that the tier colours are the
  spec values.
- `window.rs`'s own test module asserts the native colour, and derives the `Color`
  tuple from the hex string via `parse_hex` so the two cannot drift. (Two independent
  literals per mode is exactly how a tuple and its hex diverge, and the symptom — a
  subtly wrong flash colour — is one no other test would catch.)

Those tests found two real things while being written: the palette check was
comparing a lowercased haystack to uppercase needles (so it would have passed with a
Verge colour present), and the check was scanning the file's own prose, which names
the colours it replaced. Both fixed; a "guard on the guard" test now asserts the
comment-stripping did not become so eager that it strips real code.

### What a student sees now

- **Activation** — Locus wordmark in brand green, "Secure school VPN" subtitle, a
  green radial wash, a filled code field, and the Activate button in `#2EA86A`. The
  screen is fully decoupled from MUI's theme (`useTheme` is no longer used there at
  all), which is the correct end state: it renders outside the theme provider and
  cannot read the app palette, so every value is now explicit.
- **Connection** — spec status colours (green connected, amber connecting with a
  pulse, grey disconnected), the tier as its own **badge** rather than concatenated
  into the status text, and cards on the spec's surface/border/12px-radius treatment.
- **Account** — the same card treatment via one shared token module
  (`src/pages/_surfaces.ts`), so the two screens cannot drift apart.

MUI's shadow scale is disabled app-wide, so cards separate themselves with a border.
A card relying on `elevation` would be invisible; the shared token is what prevents
each screen re-deciding that.

### Still outstanding

**There is no Locus logo asset in the repo.** The spec calls for a 48×48 shield in
`#2EA86A`; the activation screen shows the wordmark alone rather than substituting
Clash Verge Rev's mark, which is still what the icon files contain. Deliberate, not
an oversight — drawing a brand mark needs a decision this work did not have.

### Verified

- 8 new front-end tests (palette/layer consistency), 4 new Rust tests (hex derivation).
- `tsc`, CI lint, `cargo test --lib` (501 passed), `vitest` (26 passed) all green.
- Built and ran: the activation screen reports `page_bg` and heading colour from
  computed style, so the palette is confirmed in the running app rather than inferred
  from source.

---
## THE STUDENT'S REPORT: "THE APP IS NOT WORKING AND THE REBRANDING IS INCOMPLETE" (2026-09-26)

The client opened, and that was the last thing that worked. The report was specific
and it was correct on every point: a pure-white activation page, `[object Object]` on
Connect, "Core temporarily unavailable" on the proxy page, a Sidecar fallback that
failed with a missing-file error referencing AppData, `networkingguides.duckdns.org`
visible in two places, "Verge Version" on the system card, and — the headline —
**"the ui is just clash verge rev"**.

### The framing that was wrong, and why it mattered

Earlier entries in this file treat the client as a product that happens to contain
some Verge code. It is the other way round: **the client *is* Clash Verge Rev, with a
thin Locus surface on top.** Measured: 8 of ~203 front-end files mention Locus at all.
The proxy page, the settings pages, the profiles and node UI, the clash-mode and
current-proxy cards, the service/sidecar machinery and all their strings are inherited
and reachable.

That is why "the rebranding broke a lot of stuff" is the right description — the
rebrand was mostly never *done* — and it is why treating each symptom as a separate
cosmetic bug would have produced a fifth release that still felt broken.

### 1. The white activation page — a colour-scheme disagreement

`index.html` sets `body { color: var(--text-color) }`, and that variable flips to
`#ffffff` under `prefers-color-scheme: dark`. But the activation screen renders
**outside `ThemeModeProvider`** (it is the pre-activation gate), so it receives a bare
`createTheme()` — a *light* palette, i.e. a white `background.default`.

On a machine in dark mode those two disagree, and the result is white text on a white
background. Every string was present and correct in the DOM; the only visible marks
were the input border and the one explicitly-coloured link, which is exactly what the
report described. Confirmed by probing computed style in the running app, not by
reading:

    before:  heading_color=rgb(255,255,255)   body_color=rgb(255,255,255)
    after:   heading_color=rgba(0, 0, 0, 0.87)  body_color=rgb(255,255,255)

**Fix:** the screen states its own `color` (and the input's `color` and resting border)
from the theme instead of inheriting the document's variable.

### 2. `[object Object]` on Connect — two `String(err)` calls

Rust failures cross the IPC boundary as `CommandFailure { code, detail, operation }` —
a plain object; the type does not survive serialisation. Both Locus catch sites did
`String(err)`, which is `"[object Object]"`, discarding the one useful sentence the
backend had produced. The repo already had the correct helper — `errorDetail()` — used
by every inherited path; Locus was the only code ignoring it.

### 3. The hub domain was student-visible, twice

The activation page rendered `HUB_URL` as a link, and the home header's `?` button
opened it. The host is the address the client *calls*, not a destination for users, and
a hostname cannot answer "my code says it is already used". Both removed. The constant
stays in `services/hub.ts` — activation, heartbeat and updates genuinely need it; only
its *display* was wrong.

### 4. "Verge Version", "Verge Basic Setting", and 174 locale values

The version row on the system card is this app's **own** version under another
product's label. Renamed across all 13 locales ("Verge Version" → "Version",
"Verge Basic/Advanced Setting" → "Basic/Advanced Settings"), and every
`Clash Verge` / `Clash-Verge` / bare `Verge` occurrence in a user-visible string
replaced — including the German hyphenated forms and the Chinese version-copy strings
that an ASCII-only sweep had missed. Result: **zero** Verge product-name strings remain
in any locale value. The engine name "Clash" (mihomo is a Clash-compatible core) is a
different thing and was deliberately left alone — 169 such strings.

Also fixed: `# Generated by Clash Verge` in every generated `config.yaml`, and the
`[ClashVergeRev]` prefix on every log line (`Type::ClashVergeRev` → `Type::App`).

### 5. Connect forced TUN on, which is what made the core unavailable

This is the root cause of the proxy-page failure, and it is an **ordering** bug.

`locus_connect()` set `enable_tun_mode = true` at *connect time*. Verge's
`prepare_startup` reads that as `service_required`; with the service `NotInstalled` it
returns `StartupDecision::Wait`, so **the core never starts** — and the UI reports
"Core temporarily unavailable", a message about the core for a problem that was never
the core's.

The app already solves this problem, at the wrong moment: `reconcile_startup_tun_availability`
runs at startup and turns TUN off *precisely so the core can start on the Sidecar*.
Connect then set it straight back on, re-arming the condition startup had just cleared.
The app started cleanly and walked into the wall on the first press of Connect.

**Fix:** `locus_connect()` now consults `RUN_STATE.state().tun_capable()` (the same
predicate the manager uses: `is_admin || service_usable()`) **before writing anything**,
and refuses with `LOCUS_TUN_NOT_AVAILABLE` and an actionable sentence when TUN cannot
start. Nothing is persisted on the refusal path, so a failed Connect no longer leaves
`enable_tun_mode` flipped on for the next launch to trip over.

TUN remains always-on. This does not make Locus connect without it — it stops the app
from demanding privileges it does not have and then blaming the core.

### 6. The install prompt reported the wrong thing

The service installer elevates via `pkexec` (falling back to `sudo`); when that fails it
reports only `elevated installer failed with <status>`, and every distinct cause — no
polkit agent, dismissed prompt, wrong password — arrives identically. It used to surface
as a generic install failure, sending support to look for a service problem that did not
exist. Now classified as `SERVICE_ELEVATION_FAILED` with a sentence naming both routes
(approve the prompt; or run from a terminal). The classifier matches on upstream's
message string, which is a real coupling — if that wording changes it degrades to the
generic code, which is the safe direction.

### Verified

- 5 new Rust tests for the TUN gate (including the in-flight-operation case and an
  assertion that the message does **not** say "TUN", which is our word, not the
  student's), 4 for the elevation classifier (including a negative case, so an unrelated
  failure is never reported as an elevation problem).
- `cargo test --lib`: **482 passed** (was 474). `version_consistency`: 3 passed.
- `tsc`, CI's own `pnpm run lint`, `vitest` (20) all green.
- All 130 locale JSON files parse; the locale diff is 174 insertions / 174 deletions —
  value-only, no structural change.

### What this does NOT fix, stated plainly

The Verge UI a student sees is untouched. That is the largest remaining piece of the
report and it is a front-end replacement, not a rename — recorded in "Deferred: the
Verge UI a student still meets" below. Also unchanged: this was verified on a
locally-built binary, and the install flow still has never been exercised on real
hardware, which remains the honest limit on what fix 6 can claim.

---
## THE BLANK WINDOW, PART 2: THE LOADING OVERLAY COVERED THE ACTIVATION SCREEN (2026-09-26)

The `base: './'` fix below was **necessary but not sufficient**. The window was still
blank after it, with the WebKit default right-click menu as the only interactive
thing — the signature of a document that loads fine and shows nothing.

### Root cause

`index.html` ships an opaque full-screen element:

```css
#initial-loading-overlay { position: fixed; inset: 0; z-index: 9999; }
```

It is removed only by `hideInitialOverlay()`, and that was called from exactly one
place: `useLoadingOverlay(themeReady)`, inside `pages/_layout.tsx`. But `Layout` is
reached through `RouterProvider`, which renders only in the `activated === true`
branch of the gate in `main.tsx`:

```jsx
if (activated === null) return <CircularProgress />   // overlay still up
if (!activated)        return <ActivationScreen />     // overlay still up
// RouterProvider -> Layout -> useLoadingOverlay  <- the ONLY removal, unreachable
```

So on an **unactivated device — every fresh install, and the first thing a student
ever sees** — nothing ever removed the overlay, and it sat permanently over a
correctly rendered activation screen.

### Why it was so hard to see

Everything that could be checked was correct:

- every asset returned **200**, zero 404s (`index-*.js`, `layout`, `home`,
  `settings`, `proxies`, `shared` all resolved)
- the page-load hook reported **`finished`** on `tauri://localhost/`
- **nothing threw** — no `window.onerror`, no unhandled rejection, no
  `console.error`
- `#root` contained the **rendered activation screen** the whole time:
  `children=1 text="LocusEnter the activation code from your card.Activate…"`

The app was working. It was simply invisible.

### Fix

1. Move the overlay removal to `main.tsx`, into `Shell`, so it runs on first mount
   whichever branch renders. `useLoadingOverlay` and its hook file are deleted, so
   there is exactly one owner.
2. Add a **CSS-only backstop**: an animation that hides the overlay after 5s
   regardless of whether any script runs. The JS path still removes it earlier on
   a healthy start, so the animation is only ever the safety net. This matters
   because the old design made a JS failure present as a blank window rather than
   as an error — the backstop converts that failure mode into "slow".

### Verified

DOM probes from the window initialization script, before and after:

| | before | after |
|---|---|---|
| `t+3s` | `overlay=present` | `overlay=removed` |
| `t+8s` | `overlay=present` | `overlay=removed` |

### The instrumentation is part of the fix

None of this was diagnosable before, and three additions are kept:

- `on_page_load` now logs the **URL and event** for both `started` and `finished`.
  A window that loads the wrong origin and one whose scripts all failed were
  previously indistinguishable.
- `on_web_resource_request` logs every asset and its **status**, so a 404 is
  visible. This is what proved asset resolution was fixed and the fault was
  elsewhere.
- A **JS→native log bridge** (`locus_js_log` + a window-init-script hook) forwards
  `window.onerror`, `unhandledrejection` and `console.error` into the native log.
  It is installed by the initialization script, which runs *before* any
  application module — deliberate, because the app's own error handlers do not
  exist yet when the app is what failed.

**The lesson: "renders nothing" and "renders nothing visible" are different bugs,
and only inspecting the DOM tells them apart.** Every signal available here — HTTP
status, page-load events, the console — was green while the app was unusable.

---



## THE CLIENT RENDERED A BLANK WINDOW — ASSET URLS RESOLVED AGAINST THE PROTOCOL ROOT (2026-09-26)

The 3.0.0 build opened a window containing only the static shell from `index.html`:
no React, no activation gate, nothing. The WebView was loading a document, and
every script and stylesheet in it 404'd.

### Root cause

`client/vite.config.mts` never set `base`, so Vite defaulted to `/` and rewrote
every asset URL to a root-absolute path:

```html
<script type="module" src="/assets/polyfills-CJXZ-NIF.js"></script>
<link rel="stylesheet" href="/assets/index-DqUKtJPb.css">
```

In production Tauri serves `dist/` over the **`tauri://localhost` custom protocol**,
where `/assets/...` resolves against the *protocol root*, not `dist/assets/`. So
every module preload, the entry chunk and the stylesheet failed to load, `#root`
was never populated, and the only thing that could render was the static chrome in
the document itself — which is why the window looked like "HTML that does not
exist" while a build and 474 tests were green.

This is invisible to every test we had: the build succeeds, the files exist on
disk, and `pnpm run web:dev` is unaffected because the dev server *does* serve
`/assets/...` from the root. Only the packaged app could show it.

### Fix

`base: './'` in `vite.config.mts`. Verified in the built output: **0** absolute
`/assets/` references, 15 relative `./assets/`, and the compiled binary embeds
`./assets/shared-B3fm-sXH.js`-style paths.

### Two further defects found while diagnosing it

1. **`createBrowserRouter` under a serverless protocol.** `_routers.tsx` used the
   HTML5-history router. `tauri://localhost` has no server behind it, so a route
   like `/proxies` requests a document that nothing serves — navigation and reload
   broke. Switched to `createHashRouter`, which keeps the route in the fragment.
2. **Five navigations to a route that no longer exists.** `/profile` was removed
   with the profiles UI, but `HomeProfileCard` — rendered on the home screen **by
   default** — plus the proxy empty state and two notice handlers still navigated
   there, each landing on a blank page. The card was subscription-management UI
   (import subscription, traffic quota, expiry) with no role in a code-activated
   client; it and the dead buttons are removed, and the now-unused `navigate`/`t`
   plumbing through `_layout.tsx` and `notification-handlers.ts` with them.

### Branding leaks fixed in the same pass

The window title said **"Clash Verge"** from two independent places, and the
second overrode the first on macOS: `index.html`'s `<title>` and
`lib.rs::handle_ready_resumed`'s `set_title("Clash Verge")`. Both now say Locus,
as does the startup-failure dialog in `utils/startup.rs`.

**Not fixed, and deliberately:** the icon assets (`logo.svg`, `icon_dark/light.svg`,
`icon.{png,ico}`, `Assets.car`, the Windows squares) are still the Clash Verge Rev
mark, and `logo.svg` is a 117×27 "Clash Verge" wordmark. No Locus-branded asset
exists anywhere in the repository, so replacing them is a design decision rather
than a bug fix. The documented brand is Locus green `#2EA86A` on `#06130C`
(`docs/ARCHITECTURE.md`, `docs/archive/UI-AESTHETICS.md`).

### The lesson

**A green build and a full test suite do not prove the packaged app can load its
own assets.** The tests ran against source, the dev server served from the root,
and the failure existed only in the artifact that ships. The check that catches
this class is inspecting the *built* `dist/index.html` for root-absolute paths —
which `pnpm run web:build` now produces correctly, and which is what the fix was
verified against.

---



## "LOCUS HAS NO LINUX ELEVATION PATH" WAS A CLAIM ABOUT THE RETIRED CLIENT, NOT THE FORK (2026-09-26)

Five documents said the client cannot create a TUN device on Linux because it has no
elevation mechanism. **All five were describing `legacy/wails-client/`** — the Go +
Wails + sing-box client that does not ship — and the claim was copied into contexts
that read as if it were about `client/`.

It was stated to the operator as current fact ("there is no Linux elevation path…
TUN cannot be created in a non-root session"). It is **false for the shipping fork.**

### What is actually true

`client/` is built on Clash Verge Rev and inherits its elevation path unchanged —
it needs no elevation code of its own:

- `crates/…/clash_verge_service_ipc` → `management.rs::elevate()` runs the installer
  under **`pkexec`**, falling back to **`sudo`** when `pkexec` is absent or exits 127.
  (`utils/help.rs::linux_elevator()` probes for `pkexec`; macOS uses
  `osascript … with administrator privileges`, Windows `Start-Process -Verb RunAs`.)
- `core/service.rs::install_service()` → `invoke_service_install()` →
  `clash-verge-service-install` installs a **root service**, which stages and runs
  the core from its own administrator-approved directory (`stage_approved_core`).
- `core/runstate/health.rs::tun_capable()` is `self.is_admin || self.service_usable()`,
  covered by `tun_is_capable_when_elevated_even_with_no_service` and
  `tun_is_capable_via_a_ready_service_without_elevation`.

So the chain is pkexec → install root service → service runs mihomo as root → TUN
works. This is the same mechanism that made Clash Rev Meta work in the original
2026-08-03 school test — which is exactly why the project rebuilt on Verge rather
than repairing the retired client (`docs/ENGINE-SWAP-ANALYSIS.md`, correction banner).

### Why the claim was wrong to inherit

The retired client's limitation was real *for it*: `legacy/wails-client/app.go:157`
forces `SetHelperMode(false)`, and its `autoStartHelper` drove a helper binary that
was never shipped. The architectural rule that replaced it is explicit —
upstream's machinery is *used, not replaced* (`client/docs/UPSTREAM-CHANGES.md` §1),
and elevation is deliberately **not** written
(`client/docs/LOGIC-INVENTORY.md` §10: the old path "was buggy three separate times
(FIXES #17, #40, #41) and should not be ported in any form").

### What was done

Corrected in all five places, each now saying what is true of the fork:

| File | Was | Now |
|---|---|---|
| `docs/STILL-OPEN.md` | "There is no elevation path on Linux" | The fork **has** one (pkexec → root service); only **running** it is open |
| `docs/ENGINE-SWAP-ANALYSIS.md` | Track A listed as the work to do | ⚠️ correction banner: both tracks describe the retired client; Track A is inherited |
| `docs/README.md` | "Track B is settled" | Track A is settled too; both describe the retired client |
| `client/docs/ARCHITECTURE.md` | "Linux support level: is it tested or best-effort?" | Elevation is not the open question — validation is |
| `docs/archive/README-legacy-v5.md` | "…the Linux TUN elevation gap" | marks both tracks as the retired client's |

Memory files `engine-swap-analysis-mihomo` and `v5-client-architecture` were
rewritten with status banners; `client-complete` now records the inherited path.

### The lesson worth keeping

**A retired component's limitation is not the live system's limitation, and a
document that names the product while meaning the component is a trap.** The
subject of "Locus has no elevation path" was one retired client among three; two
documents asserted it as a project-level fact, and one memory file still called
that dead code "current, authoritative".

Two cheap defences, both skipped: check what the claim is *about* before believing
it, and read the component that actually ships. Long-lived notes need an explicit
status banner, or they age into traps.

---

## `/api/hiddify` HAD NEVER WORKED, AND ITS `all=1` LEAK WAS NEVER EXPLOITABLE (2026-09-25)

`hiddify.pb.js` generated Hiddify-compatible `ss://` links for testing a tier
without the Locus client. It accepted `?all=1`, which returned **every** active
tier's link — and since a tier's password is one shared PSK, that would have
handed the Strike secret to anyone holding a $2 Eco code.

It was reported as a live tier-upgrade bypass. **That was wrong**, and the
correction matters because it changes the severity and the fix.

### What was actually true

`btoa()` is called on the path that builds every link, and **goja provides no
`btoa`, `atob` or `Buffer`**. Verified on the live host with a throwaway probe
hook:

```json
{ "hasBtoa": false, "hasAtob": false, "hasBuffer": false,
  "btoaError": "ReferenceError: btoa is not defined" }
```

So the endpoint threw `500 btoa is not defined` on **every valid code**, before
reaching any link-building code. The `all=1` branch was unreachable. The endpoint
had never worked since it was written.

Measured live, on a code that passes every earlier check:

```
GET /api/hiddify?code=<valid>      -> {"code":500,"message":"btoa is not defined"}
```

### Why it was still worth fixing

A latent vulnerability behind a runtime error is one `btoa` polyfill away from
being live, and the next person to touch this file would have been reading a
tier-scoped-looking handler that also had an all-tiers branch.

### What was done

**The endpoint was deleted**, not repaired. Nothing in the repo, the console, the
scripts or the tests referenced it; the Locus client supersedes its purpose; and
it was a credential-emitting surface with no remaining consumer.

The intermediate change (removing `all=1` and scoping the domain lookup to the
caller's own tier) was made first and is preserved in git history, but became
moot once the file was removed.

### The lesson worth keeping

**I characterised the severity from reading the code, not from exercising it.**
The read was correct — the branch really did return every tier — but "what the
code would do" and "what the deployed system does" diverged, because a runtime
that lacks `btoa` fails three lines earlier. Probing the live runtime took one
request and would have caught it immediately.

Test the path, not just the logic.

---

## RELEASE MACHINERY REMOVED, NOT REPAIRED (2026-09, current)

The archived Wails client's version/release machinery was **deleted** rather than
fixed a further time: root `VERSION`, `bump.sh`,
`server/scripts/bump-version.sh`, `stamp-syso.py`, `smoke-bump.sh`,
`scripts/release-cut.sh`, the committed `rsrc_windows_*.syso` resources, and
`.github/workflows/build.yml`.

Rationale: every entry below in this file that ends "…and the gate that knew"
describes a guard on a pipeline that built a client nobody ships. It versioned only
`legacy/wails-client/`, never the fork, and its tags were the source of repeated
confusion. `legacy/wails-client/` is now a stale, reference-only logic oracle; the
shipping fork's versioning and release path are an **open decision**
(`docs/STILL-OPEN.md`).

**Entries below this line are historical.** Where they name `bump.sh`, `VERSION`,
`release-cut.sh`, `stamp-syso.py`, `smoke-bump.sh`, `.syso`, or `build.yml`, those
files no longer exist. The *lessons* (a committed artifact can be stale while every
text check passes; `--no-verify` skipped the artifact update; a skipped CI job looks
green) remain worth knowing if any of this is ever rebuilt.

---

## RELEASE TAGGING — THE .syso WENT STALE AGAIN, AND THE GATE THAT KNEW (2026-09-20)

CI failed the 2.2.7 release in **Check version consistency**:

```
rsrc_windows_amd64.syso FileVersion = "2.2.6", want "2.2.7"
rsrc_windows_arm64.syso ProductVersion = "2.2.6", want "2.2.7"
```

The reaction this invites is "rerun `go generate` and push" — which is what the
first fix did. That is the wrong read, because the same thing had already
happened once and the guard built to stop it was bypassed, not missing. Fixing
only the artifact would have guaranteed a third occurrence.

### 57. `release-cut.sh` passed `--no-verify`, which skipped the artifact update too

`bump-version.sh` regenerated the `.syso` and re-read them as a self-check — that
gate was added after 2.1.0 shipped resources stamped `2.0.0` / product `MyVPN`.
But `--no-verify` short-circuited the script **before** that block, and
`scripts/release-cut.sh` line 20 passed `--no-verify` unconditionally. So the
automated release path — the one that actually cuts tags — never regenerated
anything, and warned only in text a reader could scroll past.

Evidence in the history: `cd4dbc0` (2.2.6) changed 8 files including both `.syso`;
`916aec7` (2.2.7) changed only 6, with no resources. Nothing noticed until CI ran.

This is the second-order version of the 2.1.0 bug. There, the guard did not exist.
Here it existed and the fast path routed around it — which is worse, because it
looks protected.

### 58. The fix depended on a Go toolchain that the release host does not have

The documented remedy was `cd v5/client && go generate -tags windows`. That needs
a Go install and network access to fetch `go-winres`, and this machine has no
system Go — so "regenerate the artifacts" was advice that could not be followed
where releases are actually cut. A step that is awkward to run is a step that
gets skipped, which is how the artifacts went stale twice.

It does not need a toolchain. A patch-level bump changes only four bytes: the two
UTF-16LE display strings after `FileVersion`/`ProductVersion`, and the packed
`VS_FIXEDFILEINFO` block whose `dwFileVersionMS/LS` and `dwProductVersionMS/LS`
carry the same number in binary. Missing the second one is the subtle failure —
the Properties *string* would read the new version while Windows' own version
comparisons and the installer still see the old one.

`v5/server/scripts/stamp-syso.py` rewrites both with the standard library. Its
output is **byte-for-byte identical** to `go-winres`' — verified in both
directions against the real 2.2.6 and 2.2.7 artifacts, and against the historical
2.2.6 bytes recovered from `cd4dbc0`. It refuses (exit 3) rather than guessing on
a version-width change such as `2.9.9` → `2.10.0`, where the fixed-size fields no
longer fit and `go generate` really is required.

### 59. The guard ran after the expensive work, and after the tag

Even when it fired, `TestCommittedSysoMatchesRepoVersion` ran deep inside the
`lint` job, producing an error that reads like a Go test failure rather than
"your artifacts are stale" — which is exactly how the 2.2.7 report was misread as
a generation-command problem. And it only ever ran **after** a tag had been
pushed, so the tag was already public and wrong.

Three changes, at three points in the pipeline:

| Where | What |
|---|---|
| `bump-version.sh` | The `.syso` stamping now runs **before** the `--no-verify` branch, unconditionally, with no toolchain. `--no-verify` skips the *test* gate, never the artifact update. |
| `release-cut.sh` | Re-checks the committed resources with `stamp-syso.py --check` and **refuses to commit or tag** a stale tree. Toolchain-free, so it cannot be skipped for want of one. |
| `build.yml` | The same check runs first among the version guards, so a stale artifact is attributed to the artifact and not to a failing test. |

### 60. The regression suite did not cover the path that broke

`smoke-bump.sh` ran every case with `--no-verify` and explicitly documented that
the `.syso` regeneration was "covered by CI, not by this smoke test" — so the
exact combination that failed (no-verify, plus no toolchain) had no offline test.
It now seeds a **real** `.syso` pair, stamps it down to the scratch version, and
asserts under `GO_BIN=definitely-not-a-real-go` that a bump still moves the
artifacts, that the packed `VS_FIXEDFILEINFO` fields move with the strings, that
the brand text survives, and that `--check` reports stale without writing.
`smoke-bump` went from 21 assertions to 33.

---

## CLIENT UPDATE — THE PORTABLE BINARY COULD NOT REPLACE ITSELF (2026-09-20)

Field report, immediately after 2.2.3 was published:

```
⚠ Update to 2.2.3 failed: download failed: cannot rename downloaded file: rename
C:\Users\Hello\Downloads\locus-windows-amd64.exe.new.tmp
C:\Users\Hello\Downloads\locus-windows-amd64.exe.new:
The process cannot access the file because it is being used by another process.
```

The download was fine — it reached the rename at the very end, after the
checksum had passed. The rename is the step that installs the new binary, and it
could not run. This was not a fluke of one machine: it is a structural
consequence of how the portable build was deployed.

### 54. The updater staged its download in whatever directory the app was launched from

`PerformUpdate` resolved its working directory as
`filepath.Dir(os.Executable())`. For a portable copy that is wherever the user
unzipped the zip, and the most common place a student runs a downloaded
executable from is **Downloads**.

Downloads is the most heavily observed directory on Windows. Defender,
SmartScreen, the search indexer, cloud-sync clients and download managers all
open a newly written `.exe` within milliseconds of it appearing. The updater
wrote `locus-windows-amd64.exe.new.tmp` and then tried to rename it *in that
same directory*, so it was aiming at the one folder most likely to have something
else holding a handle on the file.

Nothing here is a bug in the rename itself: renaming a file that another process
has open genuinely cannot succeed on Windows. The mistake was choosing a
directory the application does not own as the place to perform an install.

### 55. The download handle was still open at rename time

Independently of any third party, the code raced its own file descriptor:

```go
f, err := os.OpenFile(tmpPath, ...)
defer func() { _ = f.Close() }()   // runs at function RETURN
...
os.Rename(tmpPath, path)           // ...so the handle is still open here
```

`defer` runs when the function returns, which is *after* the rename. On Windows
a file with an open handle cannot be renamed, so this was guaranteed to fail
whenever the timing lined up. The reported error blamed "another process"; in
some of those cases the other process was Locus.

### 56. A transient blocker was never retried, and the error was unreadable

Two smaller defects made the same failure worse:

- `os.Rename` was called once. The holders described above are transient by
  nature — a scanner that opened the file the instant it appeared, a sync client
  finishing up — and a short bounded retry would have ridden most of them out.
- The surfaced error was the raw `os.Rename` output: two absolute paths, a
  Windows status phrase, and no indication of which process was involved or that
  retrying after closing a File Explorer window would work.

### The fix

**Install into a directory the application owns.** A new package
`v5/client/internal/install` resolves where this copy of Locus lives and how it
was deployed:

| Platform | Install location | Rationale |
|---|---|---|
| Windows | `%ProgramFiles%\Locus`, else `%LOCALAPPDATA%\Programs\Locus` | Owned directory; the client already runs elevated (its manifest sets `requireAdministrator`) |
| macOS | `/Applications/Locus.app` | A `.app` is the unit macOS replaces |
| Linux | `~/.local/bin`, else `~/.local/share/locus/bin` | Deliberately unintrusive — nothing system-wide is written |

Anything else resolves as **portable**, which is a supported deployment rather
than a failure — the launched directory is still used, just with a private
staging subdirectory inside it (`<install-dir>/.locus-staging/`). A portable copy
run from Downloads therefore keeps working: the download no longer lands on a
filename that the folder's watchers are sitting on.

The staging directory is deliberately *not* the system temp directory.
`os.Rename` is only atomic within a single filesystem, and `%TEMP%` is frequently
a different volume from the install location — renaming across volumes degrades
to a copy, which is neither atomic nor safe for a running executable.

Also fixed:

- **The handle is closed explicitly before the rename** (the `defer` stays as a
  safety net for error paths), and a close failure is now reported rather than
  silently proceeding into a rename that cannot succeed.
- **Transient rename failures retry** with a bounded backoff — 5 attempts,
  100→1600 ms (~3.1 s total). Permanent failures (missing path, read-only
  filesystem, cross-device) are classified and returned immediately, because
  waiting on them converts a fast, clear error into a slow one.
- **The error is actionable**: it names the file that was blocked, says another
  program is holding it, gives a platform-specific remedy, and states that the
  verified download was left intact so retrying is safe.
- **AppImage-style read-only mounts are detected** and reported as their own
  mode (`immutable`), since elevation cannot help there and the correct advice is
  to replace the bundle.
- **Diagnostics report the install location and mode**, so "update failed" is
  answerable from a support report instead of requiring a reproduction.

### Installers, and the portable build stays

Real installers now ship alongside the portable zips, which continue to be
published and supported:

- **Windows** — Inno Setup (`build/windows/locus.iss`), installs to
  `%ProgramFiles%\Locus`, registers an uninstaller, and removes the staging and
  rollback directories on uninstall.
- **macOS** — a real `Locus.app` bundle inside a `.dmg`
  (`build/macos/make-dmg.sh`), which installs into `/Applications` and carries a
  README explaining the unsigned-app first-launch steps.
- **Linux** — portable only, by design. `os.Rename` over a running binary is
  legal on Linux, so in-place update already worked and there is no reason to
  write anything system-wide.

No signing is performed on macOS; Gatekeeper still blocks the first launch, which
remains a separate known gap.

### Verified

- `internal/install` and `internal/updater` tests pass; the regression tests were
  each run against the reverted implementation to confirm they fail:
  `TestResolveNeverStagesInTheLaunchedDirectoryForInstalledCopies` reports
  "staging dir is the install dir itself; downloads would race directory watchers
  again", and `TestIsRetryableRenameError` caught a real defect in the first
  draft of the classifier (the Windows phrases were gated behind
  `runtime.GOOS == "windows"`, so the exact message from the field report was
  classified *permanent* when tested off Windows).
- `gofmt` clean; `GOOS=windows go vet -tags "desktop production"` clean;
  `golangci-lint` clean with the same linters CI uses (it caught one dead helper
  in the new package — the same class of failure that killed the 2.2.2 tag).
- The manifest generator was checked against the full post-change filename set:
  all four raw binaries still resolve, and neither the installer nor the DMG is
  picked up as a platform artifact.
- `fetch-release.py` needs no change: it resolves assets by name from an
  allowlist of the four binaries plus `manifest.json`, so the new installer
  assets on the GitHub Release are ignored rather than breaking the all-or-
  nothing check.

### Not fixed by this

The Windows rename fix is verified by inspection and by classification tests, but
**not on a real Windows machine** — this host cannot run one. Confirming the
update end to end on an installed copy is still required, and a portable copy
running from Downloads is the case worth testing explicitly.

---

## CLIENT TUNNEL/RECOVERY — NORTON KILLS THE SOCKET; THREE STATE BUGS IT EXPOSED (2026-09-20)

Field report: the client connected for ~5 seconds, then showed "Repairing…",
then "⚠ Tunnel could not be recovered automatically". Clicking the button while
"Repairing…" appeared to disconnect, but the next Connect failed with
"⚠ tunnel is already running" and only restarting the app recovered it. A
terminal window flashed on every Connect and kept flashing.

### Root cause of the tunnel drop: a third-party network filter (Norton)

The tunnel itself is fine. The hub was verified healthy (8445/8446 listening, UoT
live), and with the AV's protections disabled the tunnel holds. With them enabled
Windows tears the Shadowsocks socket down a few seconds after the adapter comes
up:

```
ERROR dns: exchange failed for login.live.com. IN A:
  read tcp 192.168.68.52:52716->170.64.196.179:8445:
  wsarecv: An established connection was aborted by the software in your host machine.
```

"Aborted by the software in your host machine" is a local teardown, not a network
timeout, and it kills every in-flight DNS query at once — the signature of a
filter driver detaching the connection rather than a path problem. It lands at
first-connect, exactly when Windows floods the brand-new TUN adapter with
telemetry, which is when a security filter is most likely to interfere.

**This is NOT fixed in code.** It is an environment interaction (the same class
as the earlier `strict_route` WFP finding) and must be surfaced honestly to the
user rather than silently "repaired" forever. What IS fixed is everything the
failure exposed: the client now stops churning, stops leaking engines, and stops
being a dead end when this happens. A user-facing note that Norton's network
filter must exclude the Locus engine belongs with the release.

Worth recording: the clients' DNS log showed `ncc.avast.com` and
`filerep-replica-win.ff.avast.com` resolving on a machine the user reports as
running **Norton**. Those are the machine's OWN background traffic (Locus merely
resolves DNS for it), so their presence is not evidence about which product is
filtering. An earlier diagnosis in this pass asserted "Avast" from those domain
names and was wrong — do not infer the filter vendor from the log's DNS names.

### 49. Disconnect did not stop the engine (the orphan behind every other symptom)

`app.disconnect()` began with `if !a.connected { return "Already disconnected" }`.
The watchdog's auto-disconnect calls this **after** the UI has already flipped to
Disconnected, or while the student is tapping Disconnect during a repair — so
`a.connected` was already false, the function returned early, and **`mgr.Stop()`
was never called**. sing-box kept running, untracked by the UI.

`a.connected` is a UI flag. It is not evidence about the process; only the
manager knows that. `Stop()` is already a safe no-op when nothing is tracked
(it cancels the context and removes the config file), so skipping it saved
nothing and leaked an engine.

**Fix:** stop unconditionally. `Stop()` is called on every disconnect path, and a
stop failure is logged and surfaced instead of swallowed (a failed stop is
exactly the condition that breaks the next Connect).

### 50. "⚠ tunnel is already running" was a dead end, and the cleanup that fixes it was unreachable

`Start()` opened with `if m.processAlive() { return "tunnel is already running" }`,
and the auto-clean block (`killForeignEngines` + `removeStaleTUN`) sat *below* it.
So in the one state where cleanup was needed — a live engine we do not track —
the guard fired first and the cleanup never ran. With bug 49 leaving orphans,
this became a guaranteed dead end whose only escape was restarting the app.

Two further problems in the same function:

- `m.cmd`/`m.exited` were published **after** `cmd.Start()` returned. In that
  window `processAlive()` reported false while a child was alive, so a concurrent
  `Stop()` took the "already stopped" path and returned without killing anything
  — manufacturing the orphan directly.
- `processAlive()` is only our *belief*. It can be stale in both directions.

**Fix:** reconcile tracking with reality before deciding (drop tracking if the
tracked process is gone), then treat only a LIVE, TRACKED engine as "already
running". An untracked engine is reclaimed through the cleanup path instead of
refused. `m.cmd`/`m.exited` are now published **before** `Start()`, with a
rollback if `Start()` fails, closing the race at its source.

### 51. The watchdog escalated forever, and restarted engines after disconnect

Three separate problems:

1. **`escalate()` had no notion of ownership.** `StopWatchdog()` closed the stop
   channel, but a probe cycle already in flight ran to completion and restarted
   the engine. Combined with 49/50 that is precisely the observed sequence —
   `watchdog: tunnel unrecovered after 5 tries — disconnecting` followed by four
   more sing-box startups, each one a terminal flash and a new orphan.
2. **The probe could not distinguish "no engine" from "engine up but broken".**
   Both returned a bare error, and `escalate` was reached for both — so a
   disconnected tunnel caused recovery to *spawn* an engine the user had not
   asked for.
3. **`watchdog: stopping engine for full reset: invalid argument`** — the
   teardown itself errored, so even the escalation path was failing silently.

**Fix:** a sentinel `errEngineNotRunning` (wrapped with `%w`, checked via
`errors.Is`, so the classification survives wrapping); `runProbeCycle` reports a
missing engine as `stopped` instead of escalating; `escalate` re-checks
`watchdogActive()` at the top and between each rung, so stopping the tunnel
mid-ladder abandons the ladder.

The probe's real limitation is now documented in the code rather than implied:
step 3 dials the server ADDRESS at host level, which proves the server is
reachable — not that the tunnel is carrying payloads. An in-tunnel probe is the
correct fix and is tracked separately; `ProbeTunnel` no longer claims more than
it measures.

### 52. A terminal window flashed on every Connect

`sing-box` was spawned with `HideWindow: true`, so the engine itself was fine.
Every **helper** was not: `tasklist`, `netsh`, `taskkill` and `powershell` were
spawned with a bare `exec.Command`, and a Windows console-subsystem child of a
GUI process gets a **new console allocated for it** — a window that appears and
vanishes. `foreignSingBoxRunning()` and `removeStaleTUN()` run on every `Start()`
(so: flash on Connect), and the watchdog's escalation called them again per
retry (so: continuous flashing that only stopped when the app was closed).

**Fix:** one `hiddenCommand(name, args...)` constructor applying `CREATE_NO_WINDOW`
on Windows, with `hiddenRun`/`hiddenOutput` wrappers, and **every** spawn in the
package routed through it — including `killProcessGroup`'s `taskkill` and the
PowerShell elevation call. This bug had been fixed one call site at a time and
kept returning; one constructor is what makes it unrepeatable.

Also in this pass: sing-box's `stdout`/`stderr` were attached to `os.Stdout`/
`os.Stderr`. This is a GUI binary with **no console**, so that both allocated a
console for the child (another flash source) and produced no readable
diagnostics. Both streams now go to the client log via a `managerLogWriter`
(`[sing-box]`-prefixed), while stderr is still captured in the bounded buffer
used to report startup failures.

### 53. "Repairing…" was a disabled button, so there was no way to stop a repair

The UI showed `Repairing…` on a button with `primaryDisabled = true`, and
`toggleConnection()` fell through a "button is disabled, but guard anyway" branch
to `return` — doing nothing. So a student watching an unwinnable repair (Norton
tearing the tunnel down every 10 seconds) had no way to stop it, and the disabled
styling made the app look as though it had disconnected.

**Fix:** while repairing, the button is an explicit **Stop repairing** in danger
styling and actually disconnects. `primaryDisabled` now blocks only a
double-tap during an in-flight connect.

### Tests (and how they were validated)

New `internal/manager/lifecycle_test.go`. The pre-existing watchdog test
**tolerated** a failing probe (`t.Logf`, not `t.Fatal`), which is why an unsound
probe could ship; these assert behaviour instead.

Critically, each test was run against the **reverted** fix to confirm it fails:

- `TestEscalateIsInertWhenWatchdogStopped` → **FAILS** with the `escalate` guard
  removed (`escalate must be inert once the watchdog has been stopped`). This is
  the test that pins the "engines restart after disconnect" bug.
- Two earlier drafts were **discarded because they passed with the fix reverted**
  and therefore proved nothing: one could not reach the orphan state (the probe
  reads `m.cmd`, which the test had cleared), and one omitted the retained
  `tunCfg` that recovery needs to actually spawn anything. Recorded here because
  a green test that cannot fail is worse than no test.

Verified: `gofmt` clean, `GOOS=windows go vet -tags "desktop production"` clean,
linux build clean, frontend build clean, `go test ./internal/manager/...` green
(existing tests plus the new ones).

---



Publishing a release used to run through the operator's browser. The admin
console held four file pickers, hashed each file in JavaScript, and POSTed it to
a dedicated Python uploader (`scripts/release_upload.py` + `locus-upload.service`
on `127.0.0.1:8091`) because PocketBase refuses request bodies somewhere between
1 MB and 5 MB and exposes no multipart API to hooks, and this Caddy build has no
upload handler.

That worked, and every guard in it was earned. But it made the hub depend on the
operator's laptop, its upload bandwidth and its browser to ship a release — for
bytes that CI had **already** published to a GitHub Release. The hub can fetch
them itself.

### 46. Publishing required an operator's machine to be the transport

The specific problems, none of which were bugs in isolation:

- **The transport was the weakest link.** A release could not ship from a phone,
  from a CI job, or from a box with a bad uplink. The bytes travelled
  laptop → hub over a home/school connection for no reason; GitHub had them
  already, and the hub is closer to GitHub than the operator is.
- **The size problem existed only because of the direction.** The 200 MB
  upload cap, the Content-Length *and* stream double-check, the browser-side
  hashing, the progress bars, the `~5 MB` PocketBase body cap that forced a
  whole second service to exist — all of it was workaround for pushing 30 MB
  *into* a 454 MB box. Fetching *out* from the hub has none of those
  constraints, so retiring the uploader deleted the entire class of problem
  rather than managing it.
- **Two writers, two field-name layers.** `publish-release.sh` (CLI) and the
  uploader/console wrote the same `update_config` columns from different code
  paths. Entry 31 exists because one of them wrote field names the hub does not
  read. Fewer writers is fewer ways to disagree.

**What replaced it.** `scripts/fetch-release.py` (+ `templates/locus-fetch.service`,
same loopback port) downloads the artifacts for a version **straight from its
GitHub Release**, and the new `releases.publish` hook action writes
`update_config` from the hashes the service computed on disk. Caddy routes
`/api/admin/fetch-release` and `/api/admin/fetch-link` to it; `05-caddy.sh`
retires `locus-upload` on deploy so the new unit can bind the port.

Deliberate carry-overs, so the guards that were earned are not lost:

- **Assets are resolved by name from the release manifest**, never by
  constructing a URL — GitHub's asset naming stays out of the contract, exactly
  as `publish-release.sh` already did.
- **The platform/format check moved to the server.** The console used to refuse
  a Windows `.exe` dropped into the Linux slot (`v5/console/src/lib/artifact.ts`).
  Deleting the upload UI would have deleted that check, so it now lives in the
  fetcher (`verify_artifact_kind`: ELF / PE / Mach-O, plus arm64-vs-x86_64 for
  macOS). Verified against a real release: a PE in the Linux slot and an arm64
  Mach-O in the Intel slot are both refused.
- **All-or-nothing.** Every platform plus `manifest.json` must be present, and
  the manifest must parse as JSON. A partial release is refused rather than
  published with nothing for those clients to download.
- **`publish-release.sh` stays** — it is now the only way to publish a
  hand-built or hotfixed binary that is not on a GitHub Release.

### 47. A 1 MB binary floor silently rejected a valid release

The fetcher inherited the uploader's "anything under 1 MB is a truncated
download or a Git LFS pointer" rule and applied it to every asset. `manifest.json`
is a legitimate 719 bytes, so the first real end-to-end fetch failed with
`downloaded only 719 bytes ... not a binary` — after downloading all four
correct binaries.

The floor is a guard against truncated *executables*, and a JSON manifest is not
one. It now applies to binaries only; the manifest is required to be non-empty
and to parse as JSON, which is the check that actually matters for it (it is
what proves the bytes came from the same build CI hashed). Caught by running the
fetcher against the real `v2.2.1` release before deploying, which is the only
reason it was not discovered live and blamed on GitHub.

### 48. A one-shot link must be single-use, and the version must be inside the signature

Fetches are triggered manually, and a fetch should be triggerable from somewhere
that does not hold the admin token — a phone, a CI job. That needs a link, and a
link that can be replayed is a permanent grant the moment it leaks into a chat
log, a browser history or a CI log.

So a link is `?version&nonce&exp&sig` with `sig = HMAC-SHA256(secret, version|nonce|exp)`:

- **Single use.** The nonce is burned on the first successful verification; a
  replay is `409`, not a second fetch.
- **Expiring.** 15 minutes by default (`FETCH_LINK_TTL`).
- **The version is inside the signed payload**, not just present in the query
  string — so a link minted for `2.2.1` cannot be edited to fetch `9.9.9`. A
  signature that covers only the nonce would leave that open, and it is the
  obvious thing to try first.
- **Minting requires the token.** A link cannot mint another link, or one
  leaked URL becomes an unlimited grant.

All four were verified live against the hub, including the failure paths: a
replayed link, a tampered signature, a version swapped under a valid signature,
an expired link, and an unauthenticated mint attempt.

### Verified live on the hub (2026-09-19)

- Fetch of `v2.2.1` from GitHub: all four binaries + manifest in **4.8 s**, formats
  detected `ELF` / `PE` / `Mach-O x86_64` / `Mach-O arm64`, hashes identical to
  the manifest's declared values.
- All four artifacts downloaded back **through Caddy** hash to the recorded
  values.
- `releases.publish` wrote `update_config` = 2.2.1 with all four `download_*`
  URLs and `sha256_*`; `/api/release` went from
  `{"platforms":{},"version":"2.2.0"}` (the broken state of entry 41) to four
  fully-populated platforms.
- A real bound code's heartbeat at 100% delivered `update_available`, all four
  `update_<platform>` URLs **and** all four per-platform
  `update_sha256_<platform>` hashes.
- Refusals, all live: unknown tag, `../etc` as a version, an incomplete
  artifact set, and `rollout > 0` on an unpublished version.
- `smoke-test.sh`: **23 passed / 0 failed / 0 warnings**. Real codes untouched
  (11 codes, 1 bound).
- Rollout left at **0** afterwards — a version nobody is offered yet is the
  correct resting state.

---

## ELEVATION (CONT'D) — THE TOKEN CHECK ITSELF WAS BROKEN; UPDATES NEEDED A CODE (2026-09-19)

## RELEASE TOOLING — VERSION DRIFT AND A PUBLISH THAT WROTE AN UNUSABLE ROW (2026-09-19, later pass)

`release.pb.js` implements `GET /api/release` — the update path that needs no
activation code. It exists for exactly the situation where an update matters
most: the installed build is broken enough that activation fails, or the code is
suspended, or the device never activated. In all three the heartbeat is
unreachable, so without this route the client can never be told that a fix has
been published. The failure prevents escaping the failure.

It was written, committed and reviewed. It was never copied to the host. In
production the route returned `{"code":404,"message":"Not Found."}`.

**Why nothing caught it:** hooks were deployed by hand, so there were two
sources of truth — the repo and `/opt/pocketbase/pb_hooks/` — with no
reconciliation between them. Nothing compared the two, and no check asked the
live hub whether its own routes worked. A route that 404s is not an error
anywhere; it is just a 404.

**Fix:** `v5/server/scripts/hooks-sync.sh`. It hashes each hook in the repo
against the host copy, uploads only what differs (to a temp name, moved into
place atomically, so a partial file is never loaded), restarts PocketBase, waits
for it to answer rather than sleeping a fixed amount, then verifies: service
active, no new `-p err` journal lines, and `GET /api/release` returning 200 with
valid JSON. A restart that leaves the hub broken now fails the deploy instead of
passing silently. It also runs standalone as `--check` for drift detection.

**Verified live:** the dry run named `release.pb.js` as the one hook missing and
the other six as current — so the repo and host really were in sync apart from
this. After deploy, `/api/release` returns 200.

### 44. `publish-release.sh` wrote `update_config` for a release with no artifacts

The live `update_config` row before this fix:

```
version=2.2.0  rollout=0  active=1
download_linux=''  download_windows=''  download_macos_intel=''  download_macos_arm=''
sha256_linux=''
```

Every URL and hash was an empty string. The script had been run with an empty
`RELEASE_DIR`: it warned once per missing platform, found no usable artifacts,
and **wrote the row anyway**. The result is a release that is advertised as
active with zero platforms — visible publicly as:

```
GET /api/release -> {"ok":true,"platforms":{},"published":true,"version":"2.2.0"}
```

The client handles this as `no_asset` rather than a crash, so the practical
effect is that no client can ever fetch this release while the hub reports that
one is published.

**Fix — three guards plus a source that cannot be got wrong:**

- `--from-github` fetches this version's assets from its GitHub Release by tag.
  The version argument selects the release, so there is no directory for an
  operator to point at the wrong place. Verified against the real `v2.2.0`
  release: all five assets fetched, and all four hashes matched CI's
  `manifest.json`.
- No usable artifacts at all → refuse, exit 4, `update_config` untouched.
- An incomplete platform set → refuse unless `ALLOW_PARTIAL=1`, which says
  loudly which platforms are being dropped. Dropping one is invisible from the
  operator's seat and shows up later as those students stuck on an old build.
- `--help` was parsed as the version (it died with "release dir not found"), and
  a non-version argument was accepted and used as a tag. Both fixed.
- `fail()` used `%s`, so every multi-line refusal printed the literal characters
  `\n`. The most important messages were the least readable. Now `%b`.

`smoke-publish.sh` (21 checks, `DRY_RUN=1`, no network, no hub changes) asserts
every refusal path, including that the script does not reach `update_config`.

**Not fixed, deliberately:** the live 2.2.0 row still has empty `download_*`
fields. 2.2.0 is superseded by 2.2.1, so it is left alone — republishing it is a
separate decision, not a side effect of this work. Until 2.2.1 is published,
`/api/release` advertises 2.2.0 with no platforms.

### 45. The version was maintained by hand across six files, and the lockfile had already drifted

The client version lives in six places that different tools read
independently. Bumping them by hand has already produced a shipped defect: in
2.1.0 the `go:generate` DIRECTIVE was bumped but the committed
`rsrc_windows_*.syso` were not regenerated, so the Windows executables reported
version "2.0.0" and product name "MyVPN" in their Properties tab. Every
text-based check passed — a correct directive says nothing about the artifact it
already produced.

Found while writing the replacement: **`frontend/package-lock.json` was rooted at
"2.0.0" while `package.json` said "2.2.0"**, and nothing checked the lockfile at
all, so it had drifted across two releases. Harmless only because CI runs
`npm install` rather than `npm ci` and ships built output — a latent hard CI
failure for whoever tightened that step.

**Fix:** `v5/server/scripts/bump-version.sh`. It rewrites all six, regenerates
the Windows resources, then re-reads the version OUT of the compiled `.syso`
before declaring success — the only check that can catch a stale artifact. It
refuses a dirty tree so a failed run is recoverable with `git checkout --`, and
it stops before any git command.

`bump.sh` at the repo root is the same thing with the next version computed
automatically from the current one.

Two guards, each **proven to fail on the defect it targets** rather than merely
passing on a clean tree:

- `TestFrontendLockfileVersionMatchesRepoVersion` checks both the document root
  and `packages[""]`, which drift independently.
- `TestNoUnaccountedCopyOfVersion` inverts the question the other guards ask.
  They assert the copies *we know about* agree; none can notice a NEW copy
  appearing. This greps the tree for the running version and requires every hit
  to be in a maintained file — so the seventh copy becomes a build failure
  instead of a silent divergence.

`smoke-bump.sh` (21 checks) runs the whole thing against a throwaway tree,
including that a deliberately introduced seventh copy is caught and named.


### 41. `GetTokenInformation` was called with four arguments instead of five

Entry 40 fixed the handoff logic, but the retest surfaced the deeper defect: the
error persisted even when Windows *had* elevated the process. The cause was in
the elevation probe itself.

The Win32 signature is

```c
BOOL GetTokenInformation(HANDLE TokenHandle,
                         TOKEN_INFORMATION_CLASS TokenInformationClass,
                         LPVOID TokenInformation,
                         DWORD TokenInformationLength,      // <-- omitted
                         PDWORD ReturnLength);
```

Five parameters. The Go P/Invoke passed four, omitting the buffer LENGTH, so:

- `&retLen`'s *pointer value* was consumed as the buffer size, and
- the real `ReturnLength` out-parameter read an unrelated stack slot.

The call could still return success while never writing to the `elevated`
buffer — which read as zero, i.e. "not elevated". **Every** caller therefore saw
`isElevated() == false`, including processes Windows had genuinely elevated,
which is why "Run as administrator" made no difference: the app was not
elevation-checking, it was reading uninitialised memory.

Verified the constant was NOT the problem before changing anything:
`TokenElevation = 20` is correct (confirmed against Microsoft Learn and
pinvoke.net). Worth recording, because the obvious-looking first guess
(`TokenElevationType = 18` being the intended value) would have been a
plausible-looking "fix" that changed nothing.

Fix: pass all five arguments with `unsafe.Sizeof(elevated)`, and treat a
`ReturnLength` that is not 4 as "cannot determine" rather than trusting an
unwritten buffer. Silently reading zero as "not elevated" is what turned a
malformed call into a confident, wrong error message.

### 42. Updates were only reachable through an activation code — a deadlock

Raised by the user, and correct: "if the updates only work when connected, how
can connection problems be updated?" The literal answer is better than it
sounds, but the design had a real hole.

**What was already true:** the heartbeat — which carries the update signal — is
a plain HTTPS call to the hub and does NOT require the tunnel. It runs whenever
the app is *activated*, connected or not. So a broken tunnel alone never blocked
an update.

**What was actually broken:** every update path began at
`GET /api/heartbeat`, which requires a bound activation code. Three real
situations therefore had no way to receive a fix at all:

1. the installed build is bad enough that activation fails;
2. the device has never activated;
3. the code is suspended or expired and the operator's remedy is a new build.

In each case the thing that is broken is the thing that would deliver the
repair. The only escape was the `--revert` CLI flag, which needs a terminal.

Fix, in two parts:

- **`GET /api/release`** (new hook, `pb_hooks/release.pb.js`) — a credential-free
  public manifest answering only "what version is published, and where". It
  deliberately does NOT expose `rollout_percent` (fleet policy), any per-device
  data, or any per-code data; the download URLs it returns are already served
  openly from `/updates/*`. Returns `published:false` rather than 404 when
  nothing is released, so "nothing published" stays distinguishable from
  "cannot ask".
- **Client fallback** (`internal/updater/manifest.go`) — `CheckForUpdate` falls
  back to that manifest when the heartbeat is unavailable *or* no code is bound,
  marks the result accordingly, and adopts it so `ApplyUpdate` works. Staged
  rollout is intentionally not honoured on this path (it is per-device and
  needs a fingerprint); the reason string says the check came from the public
  list.
- **Activation screen** gained a "Check for a newer version" affordance, so a
  device that cannot activate is not stranded with no way to install a fix.
  Previously `MainScreen` (which holds the update button) was not rendered at
  all while unactivated.

Tests: `manifest_test.go` covers parsing, the empty-manifest case, a
missing-platform asset, HTTP/JSON/`ok:false` failures, and — guarding the same
class of bug as the `server_port_uot`/`uot_port` mismatch — that the decoder
matches the exact key shape the hook emits. A field-name drift here would
silently report "no update" to every client.

**Not yet deployed.** The hook requires an SSH deploy to the live hub; until
then `/api/release` returns 404 and the client fallback degrades to reporting
the original heartbeat failure, which is the correct behaviour.

---

## ELEVATION — "ELEVATED COPY DID NOT HAVE PERMISSION" ON EVERY CONNECT (2026-09-19)

### 40. The elevation handoff mistook a normal auto-connect for a failed relaunch
Reported from a real Windows machine: pressing Connect raised the UAC prompt,
the user approved it, and the app replied

> Locus needs administrator permission to connect, but the elevated copy did not
> have permission. Close it and relaunch as Administrator…

and doing exactly that — launching elevated — produced the same message. A
second, apparently unrelated symptom accompanied it: an intermittent
`application is not ready — restart Locus`, curable only by pressing retry
several times, after which the app relaunched and showed the elevation error.

Four independent defects combined to produce that experience.

**(a) Elevation was inferred from the wrong flag.** `Connect()` decided "I am
the elevated copy that failed to elevate" from the presence of `--autoconnect`.
But `--autoconnect` is a legitimate user-facing flag and is *also* set on the
handoff, so an ordinary auto-connect run was indistinguishable from a relaunch
that had failed. The guard then refused to connect and printed the permission
error — including on processes that **were** elevated, which is why "Run as
administrator" changed nothing.

Fixed by giving the handoff a dedicated marker, `--elevated-attempt`, that is
never set by anything else. `--autoconnect` now means only "connect when ready".

**(b) The flags accumulated across handoffs.** `relaunchElevated` appended
unconditionally:

```go
allArgs := append([]string(nil), os.Args[1:]...)  // carries existing args
allArgs = append(allArgs, argsToAdd...)           // appends the same flag again
```

so a second handoff produced `--autoconnect --autoconnect` (and grew with each
attempt), and `--elevation-attempt=N` was appended rather than replaced, leaving
a stale count in place. Fixed with `mergeArgs`, which replaces a flag of the same
name instead of duplicating it.

**(c) Elevation got no second chance and no bound.** A single failed handoff was
terminal, with no retry counter — while the actual failure mode (a slow UAC
consent, an antivirus scan of the new process) is transient. Now bounded at
three attempts, after which the message tells the user to use "Run as
administrator" directly instead of relaunching in a loop.

**(d) The real cause of "application is not ready".** Startup launched the
auto-connect goroutine and relied on a fixed 1-second sleep to outrun its own
initialisation. On a cold elevated launch — WebView2 initialising, the new
process being scanned — `Startup` could still be running when the sleep expired,
so `Connect()` hit the not-ready guard and told the user to restart. Retrying
won the race often enough to look like a flaky crash rather than a race.

Fixed by publishing a `ready` flag **before** the goroutine starts, so readiness
is guaranteed rather than timed, and by distinguishing the two cases in
`notReady()`: a transient "still starting" (accurate, retried automatically, no
longer shown as an error) versus a recorded `startupErr` (where restarting is
genuinely the right advice).

Also fixed, found while verifying (a): the shipped `rsrc_windows_*.syso` carries
`requestedExecutionLevel="requireAdministrator"`, so a normally launched copy is
*always* elevated. Any user-visible elevation error therefore means the manifest
was bypassed or the binary is not the one being run — worth knowing before
debugging the token check itself.

Tests: `elevation_flags_test.go` covers arg merging (no duplication across
repeated handoffs, valued flags replaced, input not mutated) and, as a direct
regression test for the report, that `--autoconnect` alone is **not** treated as
an elevated relaunch. Writing them caught a real inconsistency in the first
revision of the fix: `hasFlag` compared the bare flag name while the attempt
constant included its `=`, so the valued form never matched.

---

## TIERS PAGE — STALE WARNING CONTRADICTED THE STRIKE DESIGN (2026-09-19)

### 27. The Web UI told operators to switch off the feature Strike exists for

The Tiers page carried this for every tier:

> **UDP relay** must stay off for this server: shadowsocks-rust does not
> implement sing-box's UDP-over-TCP, and leaving it on makes UDP traffic fail.

It was written 2026-08-01 and was **correct at the time**. The layout then was:
`udp_relay` unconditionally put `udp_over_tcp: true` on the client's shadowsocks
outbound, aimed at the ordinary shadowsocks-rust port. shadowsocks-rust does not
implement SagerNet's UoT protocol (magic domains `sp.udp-over-tcp.arpa`), so
every UoT dial was RST about 300ms later and UDP broke — FIXES.md Follow-up 9.

Two things changed on 2026-08-14 and the warning was never revisited:

1. **`udp_relay` no longer acts alone.** The client's condition is
   `uotEnabled := cfg.UDPRelay && cfg.ServerPortUOT > 0` — an AND. With no
   `uot_port` in the tier config, setting the flag is a no-op, so the old
   failure mode ("turning this on breaks UDP") became unreachable.
2. **The server changed.** `enable-uot.sh` installs a **sing-box** listener on
   port 8446, which *does* implement UoT. That was the whole conclusion of
   `GAMING-UDP.md`: "UoT was never the wrong protocol — it was the wrong server."

So the two fields now mean different things — `udp_relay` *declares* the tier
carries UDP; `uot_port` names the *mechanism*. The warning told operators to
disable the declaration that makes Strike's gaming feature work, and it would
have become actively harmful the moment someone enabled UoT properly.

### 28. `uot_port` was invisible and uneditable in the Web UI

`tiers.list` returned only `server`, `server_port` and `method` from the tier's
config JSON — `uot_port` was parsed over but never surfaced, and `tiers.update`
had no way to set it. An operator could therefore toggle the half of the switch
they could see while having no sight of, or control over, the half that decides
whether it does anything.

**Fixes:** `tiers.list` now returns `uot_port`; `tiers.update` accepts it
(0–65535, `0`/absent removes it) and is optional so an older console build keeps
working. The Tiers page gained a `UoT port (0 = none)` field and two
disagreement warnings — flag on without a port (a confusing no-op), and a port
set without the flag (an endpoint nobody is told about).

The replacement text states the real constraint: both switches are required, the
port must point at a sing-box UoT listener (not 8443/8444/8445), and on this
deployment no tier should set one because the UoT endpoint is not running.

### Verified live

- Web UI redeployed — asset `index-BC96z_48.js`.
- Hook deployed to `/opt/pocketbase/pb_hooks/` **and** the `/root/server/`
  staging copy; PocketBase restarted; previous hook backed up to
  `/root/admin_console.pb.js.bak.*`.
- `tiers.list` now returns `uot_port` for all three tiers, and the values are
  `0` for eco, stealth **and strike** — i.e. the live hub genuinely has no UoT
  endpoint, so the corrected warning describes the real state rather than
  guessing.

### Superseded later the same day

> **CORRECTION (2026-09-19, later pass).** The paragraph that stood here said the
> endpoint had "never run on the current host". That was already false when it
> was written: `787934c`/`78d193c` enabled UoT on `170.64.196.179` and the
> listener is live on TCP+UDP 8446 (verified by direct `ss -lntup` and by
> `systemctl is-active sing-box-uot`).
>
> **But the feature is still dead, for a different reason — entry 29 below.**
> The client parses `server_port_uot`; the hub emits `uot_port`. So `udp_relay`
> and `uot_port` are both set on Strike and *nothing on the client reads either
> one*. The server side was never the missing piece. See entry 29.

---

## UDP-OVER-TCP WAS NEVER CONNECTED, AND THE .syso SHIPPED STALE (2026-09-19, later pass)

### 29. The hub sent `uot_port`; the client only ever read `server_port_uot`

UoT was enabled on the live hub and the listener verified open on TCP+UDP 8446
(entry 28's successor, commit `78d193c`). That verification was real and
correct — and the feature still did nothing, because of a field name.

The hub stores the endpoint inside the tier's config JSON under `uot_port` and
passes that JSON through verbatim from both `/api/activate` and
`/api/heartbeat`. Confirmed against the live host:

```
$ sqlite3 /opt/pocketbase/pb_data/data.db \
    "SELECT tier, udp_relay, config FROM tier_configs"
strike|1|{...,"server_port":8445,...,"uot_port":8446}

$ curl -s -XPOST .../api/heartbeat -d '{"code":"RQ-...","fingerprint":"..."}'
{"server_config":{...,"server_port":8445,"uot_port":8446},"udp_relay":true,...}
```

The client declared only `server_port_uot` on all three of
`activation.ServerConfig`, `heartbeat.ServerConfig` and
`storage.ServerConfig`. Go's `encoding/json` **ignores unknown keys without
raising an error**, so:

```
ServerPortUOT == 0        (always)
uotEnabled := cfg.UDPRelay && cfg.ServerPortUOT > 0   →   false   (always)
```

`manager/process.go` therefore never appended the `proxy-uot` outbound. The
mechanism behind the Strike gaming tier was dead on every build, on every
platform, for the entire time it was advertised as working. `process.go` even
carried a comment asserting "LIVE SINCE 2026-09-19", which described intent
rather than behaviour.

**Why nothing caught it.** Both sides were individually correct, and each was
tested in isolation. No test asserted that the client deserialises the key the
server actually emits. A grep for `uot_port` in the client *did* match — in
comments — which is exactly the kind of hit that makes a human conclude the
wiring is fine.

**Fix — the wire key is frozen.** The hub must keep sending `uot_port`, because
already-deployed clients read that literal and a rename is a silent no-op to
them, not an error. So the tolerance lives on the client:

- New `internal/uotkey` is the **only** place that knows the field name.
  `Canonical = "uot_port"` (what the hub emits), `Legacy = "server_port_uot"`
  (accepted so a future rename is survivable in the other direction).
- The three structs' `UnmarshalJSON` delegate to it, so they cannot drift apart.
- Precedence resolves canonically from the raw map rather than by struct tag, so
  a payload carrying both keys is deterministic instead of dependent on Go's map
  iteration order (an earlier draft of this fix got that wrong; the
  `TestCanonicalBeatsLegacy` case fails on it).
- Both hooks now carry a FROZEN CONTRACT comment stating why the key cannot be
  renamed.

**Verified.** `internal/uotkey` carries 11 tests, including
`internal/uotkey/contract_test.go`, which unmarshals a verbatim live hub payload
into the real `heartbeat.Response`, `activation.ActivateResponse` and
`storage.ServerConfig` — and asserts the whole gate, not just the port, since
UoT also needs `udp_relay`. The tests were run against the pre-fix code first:
they fail with the exact production symptom (`ServerPortUOT = 0, want 8446`).
That is the assertion whose absence let both `78d193c` and the earlier 2.1.0
verification pass while the feature was dead.

**Still not validated:** a real game session on a school network. The client
path is now genuinely wired and a synthetic client dials 8446 successfully, but
`GAMING-UDP.md`'s acceptance gate remains open.

### 30. `locus.exe` reported version 2.0.0 and the product name "MyVPN"

The committed `rsrc_windows_{amd64,arm64}.syso` files — the compiled
`VS_VERSION_INFO` block that populates the Windows Properties tab — were stamped:

```
FileVersion   2.0.0
ProductVersion 2.0.0
ProductName   MyVPN
FileDescription MyVPN secure school VPN
```

while `v5/VERSION` said `2.1.0` and the product had been renamed to Locus. A
student right-clicking the executable saw a version two releases old under the
old name.

**Why nothing caught it.** `version_consistency_test.go`'s
`TestGeneratedWindowsResourceVersionMatchesRepoVersion` asserted on `main.go`'s
`go:generate` **directive** — and the directive was correct. Nothing regenerates
the `.syso` automatically (`go generate` is manual), so the artifacts had drifted
from the directive and no check looked at the output. A second, independent guard
in CI grepped the same directive text and passed for the same reason. Two guards,
one blind spot, because both checked the instruction rather than the result.

**Fix — read the bytes.** New `internal/winres` parses `VS_VERSION_INFO` out of
the compiled resource (UTF-16LE, both byte alignments, since the block's position
depends on everything preceding it) and
`TestCommittedSysoMatchesRepoVersion` asserts file version, product version,
product name and description prefix against `v5/VERSION`. It runs natively on any
platform because it only reads bytes — no Windows toolchain, no PE parser.

It also fails loudly (`ok=false`) if it cannot find the version block at all, so
a format change surfaces as "this guard is now blind, fix the parser" rather than
a silent pass.

The artifacts were regenerated with the repo's own directive
(`cd v5/client && go generate -tags windows`), and the guard was run against the
stale copies first — it fails with all four fields itemised. CI now invokes it
explicitly inside the version-consistency step so a recurrence is attributed to
versioning rather than looking like a generic test failure.

### Also in this pass

- **`v5/client/Makefile` fell back to `VERSION := 2.0.0`** when `v5/VERSION` was
  missing or unreadable. That is a version which no longer exists in the tree, so
  the build would report a number contradicting both `v5/VERSION` and the shipped
  resource — the same drift as above, arriving through a different door. It now
  fails loudly with instructions (pass `VERSION=` for a scratch build). The
  consistency test cannot see the Makefile, so the guard lives in the Makefile.
- **`docs/FIXES.md` restored.** Commit `15e7630` truncated it from 1769 lines to
  69, deleting entries 1-26, S1-S9 and Follow-ups 1-10 — the entire sing-box,
  WFP and DNS debugging history, and the only written record of why several
  guards in the client exist. The commit message described only *adding* entries
  27-28, so the loss was unintended. Recovered from `2a57d12` and re-merged, with
  the numbering scheme preserved (the file mixes `### N.` entries with unnumbered
  `### Follow-up N` and `### SN` blocks, so the sections are ordered
  newest-first rather than concatenated).
- The paragraph that previously ended entry 28 — claiming UoT "has never run on
  the current host `170.64.196.179`" — was already false when written and is now
  explicitly marked as superseded, so it does not send a future reader down the
  wrong path the way it nearly did.

---

## TWO PUBLISHERS DESCRIBED update_config DIFFERENTLY (2026-09-19, later pass)

> **CORRECTION (2026-09-23).** The account below is **wrong about the cause** and
> was corrected after a direct check. `publish-update.sh` does **not** contain
> `update_linux`/`update_windows` — zero occurrences, confirmed by grep and by
> `git log -S'update_linux' --all`, which returns nothing for that file. Both
> scripts write `download_*`.
>
> The real defects in `publish-update.sh` are (1) it emits **no
> `sha256_<platform>` columns at all**, so no platform could verify a download,
> and (2) its macOS URLs use `locus-macos-*` where CI produces `locus-darwin-*`,
> so they 404. The script has been retired to
> `legacy/publish-update.sh.broken` with this correction in its header.
>
> The lesson that survives is the real one: there was no single definition of the
> record, so nothing could notice the two scripts described it differently. That
> is what `internal/updatecfg` fixes. The entry is kept for that reasoning, not
> for its stated cause.

### 31. `publish-update.sh` wrote field names the hub does not read

`update_config` has three layers of field names, and nothing defined them in one
place:

| Layer | Producer | Consumer | Per-platform URL field |
|---|---|---|---|
| record | publish-release.sh / publish-update.sh | heartbeat.pb.js | `download_<platform>` |
| response | heartbeat.pb.js | the client | `update_<platform>` |
| struct | — | heartbeat.Response | `update_<platform>` |

`publish-release.sh` builds `body["download_" + key]` and is correct.
`publish-update.sh` emitted `"update_linux": ...` in a heredoc — the RESPONSE
name, written into the RECORD. The hub reads `download_*`, found nothing, and
omitted every per-platform URL and hash from the heartbeat.

The client then does what it is designed to do when a per-platform field is
absent: `PlatformDownloadURL()` falls through to the legacy single `update_url`.
Both scripts point that at the **linux** binary. So a release published through
`publish-update.sh` would have offered Windows and macOS clients a Linux
executable, which they would download in full and then reject on the checksum —
with no error surfaced anywhere, because "no per-platform URL" is
indistinguishable from "this release is linux-only".

**Fix.** New `internal/updatecfg` defines the three layers explicitly, records
why the rename between layers 1 and 2 exists, and warns against "simplifying" the
hook by making it emit `download_*` directly (deployed clients read `update_*`).
Its tests read the hook and BOTH scripts as text and assert they agree across all
four platforms — the cross-language check whose absence let two writers describe
the same record incompatibly while each looked right in isolation. Verified
against the pre-fix spelling: the guard fails with a message naming the wrong
layer.

### 32. `tiers.update` returned the tier password in cleartext

`return ok({tier: tierName, config: cfgT})` echoed the whole config object, which
includes the shadowsocks PSK every client on that tier shares. `tiers.list`
deliberately omits it — so the inconsistency was inside a single file.

Nothing consumed the field (the console re-renders from `tiers.list` and never
read `.config`), so it never appeared in the UI. It leaked only to anyone using
curl directly, and landed in terminal scrollback, browser devtools and any
body-capturing reverse proxy. The response now echoes just the editable fields:
`server`, `server_port`, `method`, `uot_port`, `udp_relay`, `active`.

### 33. Expiry and suspension were skipped on re-activation

`activation.pb.js` checked `expires_at` and `suspended` only on the
first-activation path, which sits BELOW the `if (boundFp)` branch that returns
`200 "Already activated"`. So a device that had already bound a code kept
re-activating successfully after that code expired — the check was unreachable
for precisely the machines it was written to protect. Only a fresh device was
ever refused.

Order is now **expiry → suspension → binding**, with two deliberate details:

- The 410 for expiry is returned before the fingerprint comparison, which is
  safe: an expired code tells a probing device nothing it could not have learned
  from the code itself.
- Suspension is checked after the fingerprint comparison on the bound path, so a
  device with a mismatched fingerprint still gets "bound to another device" and
  cannot use the endpoint to probe whether a code has been suspended.

### Also in this pass

- **`templates/Caddyfile` had the handler order `modules/05-caddy.sh` documents
  as a bug.** `/api/*` appeared before `/api/admin/upload`, so release uploads
  would have been proxied to PocketBase — which caps request bodies at a few MB
  — and counted against the general 100-requests/10s limit. The generated
  Caddyfile is correct, so production was never affected; the template is what a
  manual `sed`-based deploy uses, and its own header invites exactly that. The
  template now matches the generated config on the load-bearing ordering, and
  adopted the `route /admin { redir }` form (an exact-path matcher inside
  `handle` loses to the `handle_path` file_server beneath it, so the redirect
  never fired and `/admin` 404'd for a typed URL or bookmark).
- **`/api/code-lookup` and `/api/activate` shared a rate-limit bucket.** Both
  counted `rate_key = <fingerprint>`, so lookups and activation attempts drew
  from the same 5-per-10-minutes allowance. The lookup endpoint exists so a
  student can confirm a code is recognised *before* committing to an activation
  — but mistyping on the activation screen consumed the lookup budget, locking
  the student out of the affordance that would have explained the mistake.
  Confirmed live. Keys are now namespaced (`lookup_` / `activate_`).
- The API reference gained the expiry/suspension error rows and the two-bucket
  rate-limit note; it had been describing behaviour the hooks did not have.

---

### 34. A LIVE tier password was committed, because gitignore has no inline comments

`clash-verge-stealth.yaml` and `clash-verge-stealth-notun.yaml` were both tracked
in git, and both contain the stealth tier's shadowsocks PSK in cleartext:

```
clash-verge-stealth.yaml:53    password: "95fee4978796490984a9b9e6835a9473"
```

That value matched the live hub exactly (confirmed against `tier_configs` on
`170.64.196.179`). The files were on `origin/main` since commit `77619a2`
(2026-08-14) — a public repository — so simply deleting them now would not have
removed the secret from history.

**Root cause.** `.gitignore` contained:

```
clash-verge-stealth.yaml  # contains tier password — never commit
```

Gitignore has **no inline comment syntax**. A `#` only starts a comment at the
beginning of a line; anywhere else it is part of the pattern. So the rule being
matched was the literal string
`clash-verge-stealth.yaml  # contains tier password — never commit`, which
matches no file. `git check-ignore clash-verge-stealth.yaml` returned nothing —
the ignore never applied. The files were committed by a later `git add -A`.

The intent was correct and the comment documented it clearly. The mechanism did
nothing, and nothing verified the mechanism.

**Fix, in three parts.**

1. **Rotated the credential** — the only real remediation, since the value is in
   public history and cannot be un-published. New stealth PSK
   (`60f3159f…`, generated with `openssl rand -hex 16`), applied to both
   `/etc/shadowsocks/stealth.json` and the `tier_configs` record, listener
   restarted. Verified: identical in both places afterward, eco and strike
   untouched, all four listeners plus PocketBase and Caddy still active.

   Cost was zero by luck: the stealth tier had **no codes at all** — `0` issued,
   `0` bound — so no client was holding the old secret. Had any been active, the
   rotation would have needed a coordinated client refresh, because the client
   caches the PSK in `storage.json` and only picks up a new one from the next
   heartbeat.

2. **Removed the files from tracking** (`git rm --cached`) and corrected the
   ignore rules to patterns that actually match, with a comment explaining why
   they carry no trailing comment.

3. **Made the failure visible**: `git check-ignore` now resolves both files and
   `.gotool/`, which was previously untracked and unignored (one `git add -A`
   away from committing a Go toolchain).

**Residual risk, stated plainly:** the old PSK remains readable in the GitHub
history of this public repository. Rotation makes it useless against the live
host, which is what matters, but anyone who cloned before this commit has it.
History rewriting was not attempted — it does not remove it from clones or
forks, and it would invalidate every existing clone.

### 35. Legacy `myvpn` names: a rule, not a cleanup

The rebrand to Locus left `myvpn` strings throughout the server layer.
Blanket-replacing them would have broken the running system: several are paths
and unit names that exist on the deployed host right now — `/usr/local/bin/myvpn-backup.sh`,
`myvpn-tc-apply.sh`, `/etc/myvpn/tc`, `/var/log/myvpn-*.log`, and the client's
`.myvpn-backups/` rollback directory. Renaming those in the repo desynchronises
the definition from the machine, so a fresh `setup.sh` would install a file the
existing systemd units never call.

Split by consequence:

- **Fixed** — every user-visible or human-facing string: the Windows
  executable's product name and description (entry 30), the shadowsocks link tag
  in `hiddify.pb.js` (the tag is the display name a student sees in their client
  — the only one of these that actually reached a user), hook header comments,
  setup/restore banners, and code-card/PDF headings.
- **Left alone, documented** — live server paths, unit names, log filenames and
  the client backup directory. `v5/CONTEXT.md` now carries a naming rule at the
  top explaining which is which and that the second class must only be renamed
  as part of a deliberate redeploy.

Without that rule written down, the next person to run a global rename has no
way to tell the two classes apart, and the failure is silent until a fresh
deploy.

---

### 36. Two seed scripts described the same schema; one is now retired

`seed-pb.py` (398 lines) and `seed-live.py` (291 lines) had the same
`COLLECTIONS` list, the same tier-seeding loop, the same `update_config` and
`tier_configs` upserts, and the same `uot_port` handling. Only one of them is
wired into the deploy path — `06-pocketbase.sh` calls `seed-pb.py` — so the other
could drift indefinitely without anyone noticing, and "which do I run?" had no
answer in the repo.

Checked against the live host before deciding which to keep:

- `seed-pb.py` knows that PB 0.22.x has **no public API to create the first
  admin** (`POST /api/collections/_superusers/records` returns 404) and
  bootstraps via the CLI, then authenticates against `/api/admins`. Verified on
  `170.64.196.179` (PB 0.22.21): `/api/admins/auth-with-password` answers 400
  for bad credentials, while `/api/collections/_superusers/auth-with-password`
  answers 404 — the endpoint does not exist.
- The claim that `seed-live.py` was "broken on 0.22" was **wrong**, and worth
  recording as such: it does handle `_superusers`. It was simply the unmainted
  duplicate.

Both were schema-compatible with the live database (all four collections' columns
matched the running SQLite schema exactly).

**`seed-live.py`'s one unique behaviour was a throwaway end-to-end activation
test** — create a temporary eco code, activate it through the public
`/api/activate` with a synthetic fingerprint, print the returned `server_config`,
delete the code. That is the only thing in the deploy path that proves the hub
*serves* rather than merely *seeds*, so it was ported into `seed-pb.py` behind
`VERIFY=1` (opt-in: the default deploy should not write to `codes`).

The generator was cross-checked against the Go implementation the client and
hooks use — 20 generated codes, 20 accepted by `internal/activation`'s
`luhnModNCheck`. A generator that produced invalid codes would make the
verification fail on "Invalid code format" and look like a hub fault.

Exercised on the live host: activation returned
`networkingguides.duckdns.org:8443 aes-256-gcm`, and the code count was 11 before
and 11 after, so cleanup is reliable.

`seed-live.py` now exits immediately with a pointer to the replacement. It is not
deleted, so an existing runbook or a shell-history recall lands on an explanation
instead of "no such file".

### 37. Dead exports removed from the client's bridge

`getHubURL`, `getCodeCharset` and `getCodePrefix` were exported from
`frontend/src/lib/bridge.ts` and imported by nothing. The corresponding Go
methods stay — they are bound to the frontend by Wails and callable from a
devtools console — but the wrappers were surface a reader would assume was live.

Added a check that `AppBindings` in `types/index.ts` matches the exported `App`
methods in `app.go`: all 14 public methods are declared, and no declared method
is missing from Go. (The rest of `app.go`'s methods are Wails lifecycle hooks or
private helpers, which correctly have no frontend binding.)

---

### 38. `codes.expire` had no UI

The admin hook implemented `codes.expire` (set or clear a code's `expires_at`,
with an audit-log entry), but nothing in the console called it — an operator
could only change an expiry by editing PocketBase directly.

That was tolerable while expiry was effectively advisory, but entry 33 made an
expired code refused on **re-activation** as well as first activation. So the
one action that can remedy a lapsed code was the one with no affordance, and the
operator's only recourse was the raw admin UI.

The Codes page now exposes it in two places: an "Expiry" button on each row, and
an expiry field in the code detail modal. Both prompt for a `YYYY-MM-DD` date and
accept blank to clear it — with the prompt stating plainly that a past date
blocks both activation and re-activation, because that is the consequence the
operator is about to apply.

---

### 39. No code had ever expired: `new Date()` cannot parse PocketBase dates in goja

Found while trying to verify entry 33's fix on the live hub. The fix was deployed
and correct — and the expiry check still did nothing, on either path.

Five places computed an expiry with the same expression:

```js
var ed = new Date(exp).getTime();
if (!isNaN(ed) && ed < Date.now()) { ... }
```

Measured directly on the live host with a temporary probe hook:

```
typeof_get      = object
string_get      = 2020-01-01 00:00:00.000Z
new_Date_raw    = NaN        <- new Date(raw).getTime()
Date_parse_raw  = NaN        <- Date.parse(raw)
getDateTime     = NaN        <- record.getDateTime("expires_at")
T_replaced      = 1577836800000   <- raw.replace(" ", "T")
```

**goja cannot parse PocketBase's date format.** PocketBase returns
`"2027-09-19 00:00:00.000Z"`; the ECMAScript Date Time String Format requires a
`T` between the date and time parts, and goja enforces that strictly. Replacing
the space with `T` parses correctly.

**Why this was invisible.** The `isNaN(ed)` guard was written to skip empty or
unset values — a reasonable intent. But because the parse *always* produced NaN,
the guard was *always* taken, so the comparison never ran. The guard converted a
parse failure into **"still valid"**, which is the most dangerous possible
default for an expiry check. `expires_at` was decorative: every code was
permanently valid regardless of its date.

It also survived review because the line looks correct, and it survives testing
in Node/V8 — `new Date("2020-01-01 00:00:00.000Z")` parses fine there. The bug
only exists in goja, the one runtime these hooks actually run in. That is worth
recording: a JS-level unit test in Node would have passed.

The bug also predates this pass. Entry 33 fixed the check being *unreachable* on
the re-activation path; this is the separate reason it was *ineffective* even
where it did run.

**Fix.** A single `parsePBDate()` helper, defined **inside each `routerAdd`
callback**. It tries the value as-is, then with the space replaced by `T`, then
as a numeric epoch, and distinguishes the three cases that matter: `0` for
empty/unset, a number for a real date, `NaN` for a non-empty unparseable value —
so a future parse failure does not silently mean "no expiry". All five call sites
use it (`activation`, `code_lookup`, `hiddify`, and two in `admin_console`).

The placement is deliberate and was learned the hard way on this very deploy: a
file-level helper is invisible to the callback, because goja does not hoist
function declarations across scopes. `activation.pb.js` has carried a comment
saying exactly that since the beginning. Deploying the helper at file level
produced `parsePBDate is not defined` on every request.

**Verified live**, against PocketBase directly rather than through Caddy:

```
PASS  expired, unbound         want 410  got 410  Code expired
PASS  valid, first use         want 200  got 200  Activation successful
PASS  expired, re-activation   want 410  got 410  Code expired
PASS  no expiry, first use     want 200  got 200  Activation successful
```

The last case is the negative control: a code with no `expires_at` must stay
usable, or the fix would have broken every code issued without one.

**Incidental finding — Caddy rate-limits tests, not just abusers.**
`/api/activate` is capped at 5 requests per 10 minutes keyed on `{remote_host}`.
Iterating a verification script from one IP trips it, and the limiter returns an
empty-bodied 429, which reads exactly like a hook crash. The first three test
runs of this entry were invalidated by that before it was identified. The
verification script now targets `127.0.0.1:8090` and bypasses Caddy deliberately,
since the limiter is a deployment concern and not what is under test.

---

## CONSOLE RELEASES — AUTOMATIC ARTIFACT VERIFICATION (2026-09-19)

### 26. The console could silently publish the wrong binary

The console's Releases page accepted any file whose *name* matched the slot. It
never looked at the file's contents, so two failures passed undetected:

1. **Cross-slot mixup.** Dropping the Windows binary into the Linux slot
   published a `locus-linux-amd64` URL that serves a Windows PE file. Linux
   clients download it, fail the SHA-256 check, and never update — with no error
   anywhere. The hash matched, because the artifact is internally consistent;
   only the *platform* was wrong.
2. **Wrong release.** Uploading 2.0.0's binaries while typing 2.1.0 passes every
   existing check, and the fleet ends up on the wrong build.

`publish-release.sh` caught (1) and (2) via the `manifest.json` cross-check, but
the console had no equivalent — so the two publishing paths had materially
different safety guarantees with nothing in the UI to say so.

**Also confirmed:** the uploader's allowlist already accepted `manifest.json`,
but the console had no slot for it, so there was no way to supply one through
the web UI at all.

**Fix — automatic, no manifest needed.** The artifacts are self-describing, so
the console verifies each file directly (`v5/console/src/lib/artifact.ts`):

- **Format vs slot**, from the magic number: `MZ` = Windows PE, `\x7fELF` =
  Linux, `\xFE\xED\xFA\xCF` / `\xCF\xFA\xED\xFE` (thin) and `\xCA\xFE\xBA\xBE`
  (universal) = macOS, with the CPU type at offset 4 separating Intel
  (`0x01000007`) from Apple Silicon (`0x0100000C`). A mismatch is refused before
  anything is uploaded, naming what the file actually is.
- **Version vs typed version**, by scanning the binary for the version string
  (bounded to 4MB). A miss is treated as failure — the likely cause is a binary
  from a different release — while an unscannable large file degrades to a note
  rather than a block.
- Failed rows cannot be uploaded, and "Upload all" is disabled while any row has
  failed or is still being checked.
- The detected format is shown in the row, so the operator never has to take the
  filename on trust.

Re-picking a file, or changing the version, re-runs the check (only for files
not already uploaded or in flight).

**Verified:** the detection logic was exercised against real magic bytes for all
ten cases — PE, ELF, Mach-O arm64/x86_64 in both byte orders, universal, plus
zip and text as negative controls. This caught a genuine bug in the first
revision, which accepted `0xCAFEBABE` in either byte order and therefore
classified Java `.class` files as macOS binaries; universal Mach-O is accepted
big-endian only. Also verified: console builds, `/admin` base guard passes, and
`artifact.ts` typechecks.

**Result:** the two publishing paths now have equivalent integrity guarantees,
and the console needs no manifest upload.

---

## RELEASE PIPELINE — `manifest.json` WAS NEVER PRODUCED (2026-09-19)

Found while trying to publish the first real release through the updater.

### 25. `download-artifact` does not flatten, and the release job assumed it did

The build job uploads two things per platform:

```
dist/*.zip      → the bundles      (human download)
dist/raw/*      → the binaries     (the auto-updater's artifacts)
```

`actions/download-artifact` with `merge-multiple: true` **merges artifact names,
not directory paths** — the uploaded layout is preserved, so on the release
runner the bundles land at the top level and the binaries land under `raw/`.

Three release steps assumed one flat directory:

| Step | Assumption | Result |
|---|---|---|
| Generate release checksums | `sha256sum *.zip` | worked (zips are top-level) |
| **Generate updater manifest** | `os.listdir(".")` | **found zero artifacts → `sys.exit(1)` → the release job failed, so no `manifest.json` ever attached** |
| Create GitHub Release | `files: *.zip, *.exe` | would have attached no binaries either |

The manifest step's own "fail loudly on a missing platform" guard is what
turned a silent wrong-hash into a hard failure — but it was failing the whole
release, which is why the tag produced zips and nothing else.

**Fix:**
- New **Normalise artifact layout** step flattens `raw/` into the top level
  (names are already unique per platform, so nothing collides), giving every
  subsequent step the flat directory they were written for.
- The manifest generator now **walks the tree** (`os.walk`) instead of listing
  one directory, so a future layout change degrades to "still finds them"
  rather than "fails the release". It still fails loudly, naming the missing
  platforms, when an artifact really is absent.
- Release assets now explicitly include the four raw binaries
  (`locus-linux-amd64`, `locus-windows-amd64.exe`, `locus-darwin-amd64`,
  `locus-darwin-arm64`) plus `*.sha256` — previously only `*.exe` was globbed,
  which would have skipped the three extension-less Unix binaries.
- Checksums step asserts it found bundles and only covers `.zip` (the raw
  binaries are covered by `manifest.json`; mixing both made the file
  ambiguous).

### Verified

Replayed the exact steps locally against a directory reproducing the
`download-artifact` layout (zips top-level, binaries under `raw/`):

- normalise → all 8 files flat, no collisions
- checksums → 4 zip entries
- manifest → all four platforms with correct filenames and hashes
- manifest hashes agree with CI's own `.sha256` files
- `publish-release.sh RELEASE_DIR=… DRY_RUN=1 2.1.0` → finds all four, manifest
  cross-check passes on every platform
- deliberately removing two platforms → manifest step exits 1 naming
  `macos_arm, macos_intel` (still fails loudly)

**What to upload:** the four **raw binaries** (from `dist/raw/`, i.e. inside
each platform's inner zip) plus `manifest.json`. **Not** the outer `.zip`
bundles — the updater replaces the executable in place and cannot unpack an
archive.

---

The client had drifted into being a black box: it could not reliably say which
build it was, "no update available" had five indistinguishable causes, and the
Linux TUN gap was disguised as a mysterious engine failure. This pass fixes the
class of problem rather than the symptoms.

### 17. `isElevated()` returned `true` unconditionally on Unix

`elevate_unix.go` claimed non-Windows platforms "have no separate elevation
model", so `Connect()`'s privilege gate was skipped entirely on Linux. TUN needs
root or `CAP_NET_ADMIN`; the privileged helper is no longer shipped and direct
mode is forced, so on any non-root Linux session the tunnel was **guaranteed**
to fail — and it surfaced as a raw sing-box error deep in a log rather than
"you need root".

**Fix:** `isElevated()` now performs a real check (`os.Geteuid() == 0`) on Unix,
and `elevationUnsupportedReason()` supplies an actionable message. `Connect()`
fails immediately and explains the requirement instead of starting an engine
that cannot work. Windows behaviour is unchanged (real token check, UAC
handoff).

*Track A (shipping a real Linux elevation mechanism — pkexec/polkit or a
revived helper) is deliberately NOT in this change; the gap is now declared
honestly instead of hidden. See `ENGINE-SWAP-ANALYSIS.md`.*

### 18. Permission failures were reported as opaque engine errors

`startDirect` translated only Windows' "Access is denied". A Linux
"operation not permitted" (the exact output of the elevation gap above) fell
through to `"sing-box exited immediately: <raw text>"`, which tells a student
nothing. A shared `looksLikePermissionError` classifier now maps both dialects
to an actionable message.

### 19. Which build is this? The client could not answer

The version existed only as an ldflags string with no runtime introspection. A
dev build (`go build`, no flags) reported the `main.go` fallback literal —
which can drift from the source tree — and nothing anywhere distinguished it
from a release. That made update decisions unexplainable.

**Fix:** new `internal/buildinfo` package records the version, whether it was
injected by the pipeline, the commit/commit time, whether the tree was dirty,
and the toolchain. `main.go` writes it as the **first line of `locus.log`** so
every support log identifies its build; diagnostics and the update flow consume
it. An uninstrumented build is now flagged (`UNINSTRUMENTED`) rather than
silently trusted.

### 20. The version was duplicated in five places with nothing keeping them in sync

`v5/VERSION`, `main.go`'s fallback, `internal/buildinfo`'s fallback,
`wails.json` and `frontend/package.json`. Drift produces a binary whose
diagnostics, installer metadata and update comparisons all disagree — and it
surfaces weeks later as inexplicable client behaviour.

**Fix:** `version_consistency_test.go` fails the build when they disagree, and a
CI step does the same (including checking that a `v*` tag matches `v5/VERSION`,
so a release can never ship a binary reporting a different version than its
tag).

### 21. "No update available" collapsed five distinct causes into one boolean

`CheckForUpdate` returned `{available: false}` for: nothing published, rollout
has not bucketed this device, the advertised version is not newer, the network
is blocked, and every platform asset is missing. The UI rendered **nothing at
all** in every case — clicking "check for updates" appeared to do nothing.

**Fix:** the result now carries a machine-readable `status`
(`available`, `up_to_date`, `no_release`, `unreachable`,
`uninstrumented_build`, `not_activated`, `no_asset`), a human `reason`,
the client's own `currentVersion`/`currentInstrumented`/`platform`, and the
`advertisedVersion`. The UI renders the outcome, styles it (amber when the
check could not complete, so it never reads as a reassuring success), and shows
the platform in the footer. `ApplyUpdate` gained the same treatment: it refuses
an update with no asset/checksum for this platform *before* starting a doomed
download, and every `update:status` event now carries `from`/`to` versions so
progress reads "2.0.0 → 2.1.0" instead of a bare phase.

### 22. Update binary-swap failures could destroy the installation

Windows: if `os.Rename` of the new binary failed **and** the restore rename also
failed, the error was discarded and the machine was left with no runnable
binary (the old one stranded as `.old`). Linux/darwin: the `chmod` ran *after*
the rename, so a chmod failure reported a failed update for a swap that was
already committed — leaving the two-phase sentinel inconsistent with the
binary on disk.

**Fix:** Windows reports the salvage path explicitly ("the previous build is
preserved at `<path>`…"); Linux/darwin chmod the downloaded binary **before**
the rename, making the rename the single commit point.

### 23. Diagnostics could not answer the common support questions

`GetDiagnostics` omitted the resolved sing-box path (and whether it still
exists), the generated config path, the log file location, build provenance, and
the last update outcome. It also had broken indentation in the template.

**Fix:** the report now includes all of the above. A support report is
self-contained.

### 24. Windows-only code was never linted

CI lints on Ubuntu, so `//go:build windows` files were invisible to
golangci-lint. Running it against a Windows target surfaced pre-existing
findings: an unused `kernel32` var, three unchecked `syscall.CloseHandle`/
`messageBox.Call` error returns, and three uses of the deprecated
`syscall.StringToUTF16Ptr`. The unchecked `MessageBoxW` call was in the
**last-resort startup-error box** — a failure there shows no dialog at all,
which is precisely the "app silently does nothing" symptom that file exists to
prevent.

**Fix:** all fixed; `golangci-lint` is now clean for both Windows and Linux
targets. `internal/tray/tray.go` also passed `gofmt` again (the documented
pre-existing formatting failure is resolved).

### Not fixed (still open, by decision)

1. **Linux TUN elevation mechanism** — the gap is declared, not closed.
2. macOS builds unsigned (Gatekeeper) — unchanged.
3. sing-box is not updated by the release pipeline — unchanged.
4. `/update.json` remains a stale placeholder; the updater reads `update_config`.

---

Running the hub required SSH, Python and sqlite. That is fine for an engineer
and hopeless day-to-day, so the operations an operator actually performs now
have a web UI at **`/admin/`**.

### What was built

- **`pb_hooks/admin_console.pb.js`** — one authenticated route
  (`POST /api/admin/console`) with an `action` discriminator covering dashboard,
  code generation/listing/suspend/unbind/expiry/editing/history, middleman
  listing, tier read/write and release read/write. One auth check, one audit
  point, no scattered endpoints.
- **`v5/console/`** — Vue 3 + Vite SPA (Dashboard, Codes & Clients, Releases,
  Tiers). The admin token lives in sessionStorage only and is never in the
  built bundle; Caddy serves the static files with `noindex`.
- **`scripts/release_upload.py`** + `templates/locus-upload.service` — a
  dedicated uploader for release binaries.
- **`scripts/deploy-console.sh`** — build, package, upload, verify.
- New `code_events` collection, plus `unbound_at`, `unbind_reason`, `label`,
  `notes` on `codes`.

### 14. `admin_unbind.pb.js` wrote fields that did not exist

It called `record.set("unbound_at", …)` and `record.set("unbind_reason", …)`,
but neither column was ever in the schema. PocketBase silently discards unknown
fields, so the audit trail looked implemented and recorded **nothing**. Both
columns now exist (via the column-reconciliation added earlier), and an
`code_events` collection records admin actions properly.

### 15. Neither PocketBase nor Caddy can receive a release upload

Two dead ends found by testing rather than assuming:

- **PocketBase caps request bodies.** Raw bodies are accepted at 1 MB and
  rejected at 5 MB (`HTTP 400 "Something went wrong while processing your
  request."`). A Wails bundle is ~15–30 MB. `$apis.requestInfo(e).files` also
  does not exist in this build, so multipart is not available to hooks either.
- **This Caddy build has no upload handler** (`caddy list-modules` shows only
  `request_body`, no `upload`/`webdav`).

**Fix:** a small purpose-built service on `127.0.0.1:8091`, reachable only
through Caddy at `/api/admin/upload`. It authorises *before reading a byte*,
streams to a temp file hashing as it goes, caps the size on both the declared
length and the stream, validates the version and filename against a strict
allowlist (the filename becomes a path), and publishes atomically via
`os.replace` so a client can never fetch a half-written binary.

### 16. Caddy `handle` blocks are evaluated in order — `/api/*` swallowed uploads

The new `/api/admin/upload` block was placed *after* the general `/api/*`
proxy, so uploads were proxied to PocketBase (and hit its size cap) instead of
reaching the uploader. Caddy matches `handle` blocks top-down, so the specific
path must come first. Also verified: each `handle` block ends in its own
`file_server`/`reverse_proxy` (a missing terminal handler inside `handle_path`
is why an earlier revision 404'd).

### Live verification (2026-09-19)

| Check | Result |
|---|---|
| `/admin` (no slash) | 301 → `/admin/` → 200 |
| `/admin/` + hashed asset | 200, correct `/admin/assets/…` base |
| Admin API rejects a bad token | 403 `{"ok":false}` |
| Dashboard | counts + per-tier + activity feed |
| Generate codes (console) | created 4 codes |
| **Generated code validates client-side** | Luhn checksum correct |
| **Generated code activates a real client** | HTTP 200 + correct tier config |
| Subscribe/lookup a generated code | `ready to activate` |
| Suspend → heartbeat | 403 "Account suspended" |
| Reactivate → heartbeat | 200 |
| Unbind → status | returns to `available` |
| Event history | full trail with reasons and timestamps |
| Middleman grouping | `Sarah: 3, Tom: 1` |
| Upload: no token | 403 **before** reading the body |
| Upload: path traversal filename | refused |
| Upload: malformed version | refused |
| Upload: checksum mismatch | refused, with expected/actual |
| Upload: valid file | published; **served hash matches exactly** |
| Tier read/write, release read/write | working |

Test artifacts, test codes and the rollout setting were returned to a clean
state afterwards.

---

## CLIENT UPDATE SYSTEM — BUILT END-TO-END + PB HIDDEN BREAKAGE (2026-09-19)

The update pipeline was implemented in code but had **never worked in
production**: the gate was a silent no-op and there was no way to publish an
artifact. Both are now fixed and verified against the live hub.

### 10. `findRecordsByFilter` is broken — the update gate never fired

The single most consequential finding. In PocketBase 0.22.21's JS hook runtime,
**`$app.dao().findRecordsByFilter()` always returns an empty array**, for every
collection and every filter — including `1=1` and `id!=''` on a populated
table. It raises **no error**, so the surrounding `try/catch` never fires and
the caller simply sees "no records".

Proven with a temporary probe hook (since removed):

| Call | Result |
|---|---|
| `findRecordsByFilter("update_config", "active=true", "", 0, 1)` | **0 rows** |
| `findRecordsByFilter("update_config", "1=1", "-created", 0, 5)` | **0 rows** |
| `findRecordsByFilter("tier_configs", "tier='eco'", "-created", 1, 0)` | **0 rows** |
| `findFirstRecordByFilter("update_config", "active = true")` | **1 row** ✅ |
| `findRecordsByExpr("update_config", $dbx.exp("version = {:v}", …))` | **1 row** ✅ |
| `findFirstRecordByData("tier_configs", "tier", "eco")` | record ✅ |

Impact — silently broken, all in production:

- **Update gate ignored rollout entirely** → no client was ever offered an
  update no matter what `rollout_percent` said. This is why "we never saw an
  update arrive" was not a rollout-tuning problem.
- **Activation rate limiting (5/10 min) never enforced** — an unbounded
  enumeration oracle.
- **`/api/code-lookup` rate limiting (10/10 min) never enforced.**
- **`server_config` lookups** could return nothing, leaving a client
  "successfully activated" with no tunnel settings.

Also found in the same sweep: **top-level function declarations are not visible
inside `routerAdd` handlers** (`helperFn is not defined`). `hiddify.pb.js`
defined `sanitizeFilter()` at file scope, so the entire `/api/hiddify` endpoint
returned **HTTP 500**; it also called `$app.findRecordsByFilter`, which does not
exist on the `$app` object at all.

**Fix applied to all five hooks:** use `findFirstRecordByFilter` for single-row
lookups and `findRecordsByExpr` for counts; inline helper functions inside the
handler; verify each endpoint live.

### 11. Per-platform checksums were missing (updates could never verify)

`update_config` carried a single `update_sha256` but a release publishes four
different binaries, and `PerformUpdate` **refuses to apply an update whose
SHA256 is empty**. A single hash can only ever match one platform, so every
other platform would download successfully and then fail verification.

**Fix:** added `sha256_{linux,windows,macos_intel,macos_arm}` columns,
`UpdateInfo.PlatformSHA256()` (falling back to the legacy single hash), and
per-platform `update_sha256_*` fields in the heartbeat response. `PerformUpdate`
now resolves the hash for the artifact it is actually about to fetch.

### 12. The version gate could downgrade clients

`ApplyUpdate` compared with string equality (`Version == a.version`), so a
stale/rolled-back `update_config` row advertising an *older* release would
downgrade every client — and there is **no server-driven downgrade** in this
system, so recovery would mean shipping a new version. Also, `"1.9.0" >
"1.10.0"` under string comparison.

**Fix:** new `internal/updater/version.go` with `CompareVersions`/`IsNewer`
(numeric segments, zero-padding, pre-release ordering, build metadata ignored).
`recordUpdateSignal` now drops any signal that is not strictly newer (so the UI
never offers a downgrade) and `ApplyUpdate` refuses one regardless.

### 13. No way to publish an artifact (and `/updates/` was not served)

CI published only `.zip` bundles, which the updater cannot consume — writing
zip bytes over the running binary would brick the install. `update_config` was
all defaults with an empty `update_url`, and Caddy served no `/updates/` path
(404).

**Fix, in four parts:**
1. **CI** stages raw per-platform binaries (`locus-linux-amd64`,
   `locus-windows-amd64.exe`, `locus-darwin-amd64`, `locus-darwin-arm64`) plus
   per-file `.sha256`, and emits a `manifest.json`. The manifest generator
   **fails the build** if any platform artifact is missing, rather than
   publishing a partially-updatable release.
2. **Caddy** gained `handle_path /updates/*` → `file_server` on
   `/var/www/updates`, with directory listings disabled and a short cache TTL.
3. **`publish-release.sh`** uploads artifacts (atomic temp-name move so a
   client never fetches a half-written binary), re-downloads each one to verify
   the served hash matches, then updates `update_config` with per-platform URLs
   and hashes. It refuses to publish sub-1MB files or artifacts whose hashes
   disagree with `manifest.json`.
4. **Schema migration:** `seed-pb.py` now reconciles collections, *adding*
   columns that a pre-existing deployment lacks (this is how the missing macOS
   and `sha256_*` columns reach an already-provisioned hub). It never removes or
   retypes, so it cannot destroy data.

### Live verification (2026-09-19)

| Step | Result |
|---|---|
| Publish 4 artifacts via `publish-release.sh` | uploaded, and **each re-downloaded over HTTPS with a matching SHA256** |
| `update_config` after publish | version, rollout, 4 download URLs + 4 per-platform hashes |
| Heartbeat `rollout_percent=0` | no update fields ✅ |
| Heartbeat `rollout_percent=100` | `update_available` + all 4 URL/hash pairs ✅ |
| `active=0` | no update fields ✅ |
| **Client simulation**: download every advertised URL, hash it | **all 4 platforms match the advertised hash** ✅ |
| Directory listing `/updates/<v>/` | 404 (no listing) |
| Missing artifact | 404 |
| Guard rail: tampered artifact vs manifest | publish **refused** |
| Guard rail: truncated (<1MB) artifact | publish **refused** |
| `go test ./internal/updater/...` | 7 test funcs pass |
| Rollout restored to 0; test artifacts + test codes removed | ✅ hub left clean |

**Route note:** the hook registers `GET /api/hiddify` (not POST); testing it the
wrong way yields a 404 that looks like a missing route.

---

## SECRETS WIRING & BACKUP VERIFICATION — TWO MORE BUGS (2026-09-19, later pass)

Found while answering "are all secrets inserted, and are backups working?" on the
live hub. Both are silent-failure class: the system reported success while
doing the wrong thing.

### 8. `seed-pb.py` — creds file could disagree with the live admin password

`/root/.pb_admin_creds` recorded a password that **did not authenticate**
(`HTTP 400 Failed to authenticate`), so the admin UI could not be logged into
with the credential the deploy documented.

Two compounding causes:

1. **`os.environ` clobbering.** Step 1 copied *every* key from
   `/root/.pb_admin_creds` straight into `os.environ`. That silently overwrote
   `PB_ADMIN_PASS` (sourced from `secrets.env.age` by `setup.sh`) with the
   file's value. Any "is the env authoritative?" check therefore always saw the
   file's value — so a wrong recorded password could never be repaired.
2. **Random-password fallback.** When `PB_ADMIN_PASS` was absent, the script
   generated a random password and wrote *that* to the creds file, while the
   admin record had been created earlier with the secrets password.

Net effect: the recorded credential was wrong, and re-running could not fix it
(the CLI `admin create` only creates; it never updates an existing password).

**Fix:** read only `PB_TOKEN` from the creds file (into a local, not
`os.environ`); resolve the password env-first, then creds, then random; and
when the resolved password is the *environment* value and login still fails,
force a reset via `pocketbase admin update` so the recorded credential becomes
truthful. Verified by deliberately breaking the password and the creds file,
then confirming one run converges both back to the secrets value
(`creds == secrets`, login `HTTP 200`).

### 9. `07-backups.sh` — "✓ Backup verified" did not verify the backup

The verify step relied on `b2 download-file-by-name`, which was **removed in b2
CLI v5** and fails with `ERROR: File not present`. Worse, the checksum command
that followed was:

```bash
sha256sum -c "${TMP_DIR}/${COMPRESSED}.sha256"   # hashes the LOCAL source file
```

so it re-hashed the file that had just been uploaded — not the downloaded copy.
Result: `✓ Backup verified` printed even when nothing was retrieved. A backup
whose integrity check cannot fail is not an integrity check.

**Fix:** download with `b2 file download "b2://…" <local>` and compare the
SHA256 of the **downloaded bytes** against the local sum; set `EXIT_CODE=1` and
warn on mismatch or failed download. Verified live: remote and local hashes now
match (`4be5f822…`), and B2 reports `Checksum matches`.

### Round-trip restore proven (2026-09-19)

Separately confirmed that a backup is genuinely restorable, not merely
uploadable:

| Step | Result |
|---|---|
| Download newest `backups/*.db.gz` from B2 | OK (8809 bytes) |
| SHA256 vs the stored `.sha256` object | **exact match** |
| `gunzip` | OK (143360 bytes) |
| `PRAGMA integrity_check` | **ok** |
| Tables present | all 9 (`codes`, `tier_configs`, `activation_attempts`, `update_config`, `users`, `_admins`, …) |
| `tier_configs` rows survive | 3, with intact server/port/password JSON |
| Lifecycle rule | hide after 5 days, delete after 7, prefix `backups/` |

> Note: that snapshot contained `update_config` = **2 rows**, which is the exact
> duplication bug fixed as item 6 — independent confirmation that the fix was
> needed.

**Secrets audit (all green):** all 10 keys present in `secrets.env.age`;
`/root/.tier_passwords`, `/root/.pb_admin_creds`, `/root/.b2-creds`,
`/root/.admin_api_token` all mode `600`; the three tier passwords match
`/etc/shadowsocks/*.json` exactly; `ADMIN_API_TOKEN` matches both the file and
PocketBase's process environment and is accepted by `/api/admin/unbind-code`.

---

## BLANK-VPS DEPLOY — SEVEN BUGS FIXED (2026-09-19)

Validated `v5/server/setup.sh` end-to-end against a **fresh** DigitalOcean
Ubuntu 22.04 droplet (`170.64.196.179`, 1 vCPU / 512MB / Sydney). This box is
now the live hub for `networkingguides.duckdns.org`. Previously the scripts had
only ever been exercised against an already-provisioned host, so several bugs
that only bite on a *blank* box went unnoticed. Every one below was reproduced,
fixed, re-run, and verified.

Summary: (1) `00-env.sh` memory guard rejected 512MB droplets; (2)
`seed-pb.py` used a PocketBase admin API that does not exist in 0.22, so **no
schema was ever created**; (3) `06-pocketbase.sh` downgraded that failure to a
warning; (4) `08-firewall.sh` used a 6-conns/30s `ufw limit` that locked out
deployment automation; (5) `smoke-test.sh` reported false tc failures via
`grep -q` SIGPIPE; (6) `update_config` duplicated on every deploy; (7) unknown
codes returned HTTP 500 instead of 404/not-found in all four hooks.

### 1. `00-env.sh` — 512MB droplets could never pass the memory guard

The guard compared `MemTotal` against `524288` KB (512 MiB) and hard-failed:
```
[00-env][FAIL] Less than 512MB RAM. Minimum 512MB required.
```
A "512MB" droplet reports **464972 KB (454MB)** because the kernel reserves a
slice — so the check was impossible to satisfy on the exact hardware it was
written for. Deployments aborted before module 01.
**Fix:** compare against `MIN_MEM_KB=380000` (~371MB), which still catches
256MB-class boxes. Also warn (not fail) on absent/small swap, with the exact
`fallocate`/`mkswap`/`fstab` recipe, since no module creates swap and a
transient spike can OOM a 512MB host.

### 2. `seed-pb.py` — wrong PocketBase admin API; nothing was ever seeded

```
Superuser creation returned: The requested resource wasn't found.
Could not obtain admin token. Trying existing creds...
```
The script POSTed to `/api/collections/_superusers/records` to create the first
admin. **That endpoint does not exist in PocketBase 0.22.x.** 0.22 uses the
legacy `_admins` table and `/api/admins/auth-with-password`, and offers **no
public API at all** for creating the first admin — only the CLI
(`pocketbase admin create <email> <pass>`). Consequence: no admin ⇒ no token ⇒
**zero collections created**, while `06-pocketbase.sh` downgraded the failure to
a warning and still printed "✓ Setup complete". The hub served 500s to every
client and looked successfully deployed.
**Fix:** create the first admin via the CLI; authenticate against
`/api/admins/auth-with-password` with a `_superusers` fallback so the same
script works on 0.22.x and 0.23+. Added a `SCHEMA_ERRORS` counter that exits
non-zero if collections or tiers fail to seed.

### 3. `06-pocketbase.sh` — bootstrap failure was only a warning

`PocketBase bootstrap encountered errors — check output above` was followed by
`✓ PocketBase setup complete`. A hub with no schema is not a successful deploy.
**Fix:** treat a failed bootstrap as fatal (`fail`), and replace the fixed
`sleep 3` after start with a 30-second health-poll (the fixed sleep was racy on
512MB hosts where first-start migration is slow).

### 4. `08-firewall.sh` — SSH rate limit locked out legitimate automation

`ufw limit 22/tcp` hardcodes **6 new connections / 30s per IP** (see
`backend_iptables.py`: `--seconds 30 --hitcount 6` — there is no config knob).
Back-to-back deploy commands tripped it, producing repeated
`ssh: connect to host ... port 22: Connection refused` mid-deployment.
**Fix:** drop the limiter entirely and use **fail2ban** (`/etc/fail2ban/jail.d/
locus-sshd.local`, 6 failures/10m → 1h ban, `banaction=ufw`), keeping UFW as
plain allow/deny. UFW's own limiter cannot be re-tuned — patching
`user.rules` is futile because `ufw reload` re-renders it from the hardcoded
value.

> **Warning — do not attempt to re-add a custom limiter chain.** An earlier
> attempt injected `recent`-based rules into `/etc/ufw/before.rules`. This
> locked SSH out **completely** (port 22 timed out while 80/443 kept serving)
> and required a provider-console recovery. `after.rules` is worse: referencing
> `ufw-user-input` there fails with `Problem running '/etc/ufw/after.rules'`
> because UFW creates that chain *after* processing the file. Both approaches
> are recorded here so they are not retried.

Also normalised `/etc/ufw/ufw.conf` `ENABLED=no` → `yes`: some images ship the
flag off while rules are already loaded, which makes `ufw status` report
"active" while `ufw reload` silently does nothing ("Firewall not enabled
(skipping reload)").

### 5. `smoke-test.sh` — `grep -q` SIGPIPE caused false tc failures

```
⚠️  WARN: tc Stealth class (1:20) not found
```
on a host where `tc class show` clearly listed it. `tc ... | grep -q` makes
`grep` exit at the first match, SIGPIPE-ing `tc`; under `set -o pipefail` the
pipeline then reports **141** and the `if` takes the failure branch. The same
latent pattern existed in the UFW and `ss` checks.
**Fix:** capture command output once (`TC_NOW=$(...)`, `UFW_NOW=$(...)`,
`SS_NOW=$(...)`) and match against the captured text.

### 6. `update_config` duplicate rows on every deploy

`seed-pb.py` POSTed a new `update_config` record unconditionally, so a second
`setup.sh` run left **2 rows** (client reads the first, so mostly harmless, but
rollout state becomes ambiguous and rows grow per deploy).
**Fix:** same idempotent upsert used for `tier_configs` — delete older
duplicates, PATCH the newest, create only if none exists.

### 7. `findFirstRecordByData` throws — unknown codes returned HTTP 500

Affected **`code_lookup.pb.js`** (new), `activation.pb.js`, `heartbeat.pb.js`,
`admin_unbind.pb.js`. Every file guarded the lookup with `if (!rec)` as if the
DAO returned null. It does not — it **throws** `sql: no rows in result set`, so
the guard was unreachable and a well-formed but unknown code escaped to the
catch-all:
```
POST /api/code-lookup  →  500 {"status":"unknown","message":"sql: no rows in result set"}
```
For a student who mistypes a code this is the worst possible signal: the client
treats 5xx as "cannot tell" and retries, instead of saying "not recognised".
**Fix:** wrap each lookup in `try/catch` and treat a throw as not-found
(lookup → `200 not_found`; activate/heartbeat/unbind → `404`).

### Verification performed on the live hub

| Check | Result |
|---|---|
| `setup.sh` full run, then **complete re-run** | exit 0, all 8 modules ✓ (idempotent) |
| `smoke-test.sh` | **23 passed / 0 failed / 0 warnings** |
| TLS (Let's Encrypt, Caddy) | valid, 2026-09-19 → 2026-12-18 |
| `GET /api/health`, `/update.json` over HTTPS | 200 |
| `POST /api/activate` (all 3 tiers) | 200 + correct tier password/port, `udp_relay:false` |
| `POST /api/code-lookup` unbound / bound-same / bound-other / bad-checksum / unknown | `unbound` / `bound_this_device` / `bound_other` / `Invalid code format` / `not_found` |
| `POST /api/heartbeat` (valid + unknown) | 200 `status:ok` / 404 |
| **Real HTTP through each tier** (`sslocal` → `curl https://example.com`) | **200 + real content on 8443, 8444, 8445** |
| Wrong tier password | correctly fails (no data) |
| BBR + tc caps | BBR default; classes `1:10` 5Mbit, `1:20` 100Mbit, `1:30` 200Mbit |
| Reboot persistence | caddy, pocketbase, 3× ss, 3× tc, ufw, fail2ban, backup timer all enabled |
| `tier_configs` / `update_config` row counts after re-runs | 1 per tier / 1 total |

**Deploy notes:** the hub must be seeded with codes (`codes.json` holds the
`RQ-` set) — a fresh DB has an empty `codes` collection. `SKIP_DNS_CHECK=1` is
required until DNS points at the new host; Caddy only obtains a certificate
once the A record resolves here.

**Deploy-flow trap found during final verification:** `setup.sh` installs hooks
from the **staging copy at `/root/server/pb_hooks/`**, not from the repo. After
fixing a hook in the repo, re-running `setup.sh` with a stale staging dir
silently reverts the live fix (the `code-lookup` 500 reappeared this way after a
clean re-run). Always update `/root/server/pb_hooks/` (or re-`scp` the tree)
before re-running setup, then `systemctl restart pocketbase`.

---

## CODE FORMAT MIGRATION — MYVPN- → RQ- (2026-08-17)

**All activation codes now use `RQ-XXXX-XXXX-XXXX-C` (15 chars) instead of
`MYVPN-XXXX-XXXX-XXXX-C` (18 chars).** The Luhn-mod-N checksum covers the whole
body INCLUDING the prefix, so every code's check digit changed with the prefix.

Changed (live code + docs):
- `v5/client/internal/activation/luhn.go`: `CodePrefix = "RQ"`; `CodeTotalLen`
  is derived (now 15). Client-side validation rejects old `MYVPN-` codes with
  the prefix error.
- `ActivationScreen.vue`: placeholder `RQ-XXXX-XXXX-XXXX-C`, auto-format
  segments `2-4-4-4-1`, full-body length check 15.
- `v5/server/pb_hooks/activation.pb.js` + `heartbeat.pb.js` + `hiddify.pb.js`:
  canonical-lookup normalization now formats `2-4-4-4-1` (15 chars).
- `v5/server/scripts/seed-live.py` `make_test_code()`: `RQ` body + `2-4-4-4-1`.
- `v5/server/scripts/codes.json`: all 15 codes migrated — random segments
  preserved, prefix + checksum recomputed (e.g. `MYVPN-8FAU-DJBA-DBA7-H` →
  `RQ-8FAU-DJBA-DBA7-N`). Same bodies as the previously seeded cards.
- `scripts/generate_codes.sh` + `scripts/README.md`: `PREFIX="RQ"` (generator
  is prefix-length agnostic otherwise).
- Docs: README.md, API.md, ARCHITECTURE.md, BACKEND-API.md, CLIENT-GUIDE.md,
  IMPLEMENT.md, OPS.md, POCKETBASE-SETUP.md, UI-AESTHETICS.md,
  WAILS-MIGRATION.md — code examples/format updated.
- New `luhn_test.go` locks in the RQ format, validates the migrated codes.json
  codes + the doc example, and rejects legacy MYVPN codes.

NOT changed (intentional): `v4/` and `v5/docs/history/` are historical records
and keep the old format; `LOCUS_DEBUG`/`LOCUS_LOG_LEVEL`/`MYVPN_AGE_KEY` are
app-infra env/secret names, not code prefixes; app name remains "Locus".

**Deploy (REQUIRED, breaks old codes):**
1. The live PocketBase `codes` collection still holds the MYVPN codes — update
   each record to its RQ form (same random segments, new checksum) or delete +
   reseed from the new `codes.json` (`seed-live.py` skips existing codes, so
   plain re-run ADDS duplicates — it does NOT replace).
2. Existing bound activations keep working server-side (binding is by
   fingerprint; the heartbeat/activation hooks now normalize to the RQ form),
   BUT already-issued MYVPN cards can no longer be activated: new client
   validation rejects the prefix. Reissue/print cards with the RQ codes.
3. Deploy the three pb_hooks edits (activation/heartbeat/hiddify) with the DB
   update — the hooks are forward/backward tolerant, but the DB must match.

---

## CLIENT APP PASS — UPDATE FLOW WIRED, CODE NORMALIZATION, STORAGE RECOVERY (2026-08-17)

Client-side fix pass (no server re-deploy needed for the client fixes; the two
hook edits are hardening and deploy with the next hook sync):

**1. Update flow was a dead end — now wired end-to-end.**
The `updater` package (crash-safe two-phase swap) was fully implemented but
NOTHING called `PerformUpdate`: the UI's "Update vX available" button only
re-ran `CheckForUpdate` and never downloaded anything. Students would see the
button forever with no update ever applying.

- `app.go`: heartbeat + manual check now record the update signal
  (`recordUpdateSignal` → `a.lastUpdate`), and a new frontend-bound
  `App.ApplyUpdate()` runs `PerformUpdate` in the background (5 min timeout,
  serialized by `upMu`), emitting `update:status` events
  (`downloading → verifying → applying → applied | failed`). On success the
  app quits ~1.5s later so the forked new binary takes over. Re-applying the
  already-running version (staged rollouts re-advertise every heartbeat) is
  rejected with "Already running the latest version".
- Frontend: `bridge.applyUpdate()`, `update:status` listener, store
  `updatePhase`/`updateMessage` state, and the MainScreen button now applies
  the update with a live spinner + "Downloading…/Restarting…" label.
- Note: after an update the app does NOT auto-reconnect the tunnel (the new
  binary starts disconnected; student clicks Connect). Follow-up candidate.

**2. Activation-code normalization (heartbeat 404 risk).**
The server looks up codes formatting-sensitively (`findFirstRecordByData` on
the seeded hyphenated form), but the client stored whatever the user typed.
The frontend auto-formats with hyphens so the happy path worked, but any
unformatted/lowercase stored code (old clients, direct pastes) 404'd every
heartbeat — killing suspension checks and update signals.

- `activation.NormalizeCode()` added; `App.Activate` stores + heartbeats the
  canonical `MYVPN-XXXX-XXXX-XXXX-C` form; `Startup` self-heals legacy stored
  codes by rewriting storage.
- Hardening: both `activation.pb.js` and `heartbeat.pb.js` now normalize the
  lookup code to the canonical form before `findFirstRecordByData`.

**3. Storage: corrupt file now recovers from backups instead of deactivating.**
`storage.New` previously moved a corrupt `storage.json` aside and started
fresh — silently deactivating the device (and the code is then "bound to
another device" server-side, a support nightmare). The `.bak.0..2` rotation
existed but was never used for recovery.

- `New()` now tries `tryRestoreBackups()` (newest valid `.bak.N` first, JSON +
  consistency checked) before the fresh-start fallback; restored via the same
  atomic save path.
- `RestoreFromBackup()` simplified to reuse the same logic.
- New tests: `storage_test.go` (corrupt→restore keeps activation, no-backup→
  fresh start moves file aside, invalid backups skipped, rotation pruning).

**4. Frontend fixes.**
- `MainScreen.vue`: removed dead `await emit('connect')` error handling —
  Vue 3 `emit()` returns void, so the "failed connect" branch could never
  fire. Connect failures surface through the global error toast (store path),
  which already worked. Also removed the unused `check-update` emit/handler.
- `app.go`: bound methods (`Activate`, `Connect`, `GetStatus`, `IsActivated`,
  `GetDiagnostics`, `disconnect`, `startHeartbeatLoop`) now nil-guard via
  `notReady()` when Startup fails part-way — a broken startup shows a clear
  message instead of panicking inside the Wails binding.

**Verified:** `go build ./...` + `go vet` + `go test ./...` (incl. new storage
tests) green on linux, windows-amd64 and darwin-arm64 cross-compile;
`vue-tsc --noEmit` + `vite build` clean; frontend/dist rebuilt and committed.

**Deploy:** client fixes ship with the next client build. The two `pb_hooks`
edits are safe to deploy anytime (normalization is a no-op for canonical
codes); restart pocketbase or rely on hook hot-reload.

---

## GAMING UDP — P0 CODE LANDED (UNTESTED) (2026-08-14)

The P0 transport change from `v5/docs/GAMING-UDP.md` is implemented but
**untested** (no test environment). What landed:

- `v5/server/modules/02-shadowsocks.sh`: gated `ENABLE_UOT=1` section —
  installs sing-box server (v1.12.1) with a Strike-creds shadowsocks inbound
  on `UOT_PORT` (default 8446), `udp_over_tcp: true`, systemd unit
  `sing-box-uot.service`. Additive; 8443/8444/8445 and TCP path untouched.
- `v5/server/scripts/seed-live.py` + `seed-pb.py`: with `ENABLE_UOT=1`,
  strike tier_configs gains `"uot_port"` + `udp_relay=true`; otherwise
  identical to before (raw UDP).
- Client (`process.go`, `app.go`, `heartbeat.go`, `activation.go`,
  `storage.go`): when the tier advertises `uot_port` (UDPRelay &&
  ServerPortUOT > 0), the generated config adds a `proxy-uot` outbound
  (`udp_over_tcp: true`) and a `network: udp` route rule; TCP stays on the
  standard outbound. No advertised port → config byte-identical to before.
- Verified: `go build`/`go vet` green on linux + windows cross-compile.
  Shell modules NOT executed (no VPS).

**Deploy (when testing resumes):** `ENABLE_UOT=1` on setup.sh re-run +
`ENABLE_UOT=1` on seed-live.py. Disable: stop unit + drop uot_port from
tier_configs. Full validation checklist in `v5/docs/GAMING-UDP.md` §P1.

---

## GAMING UDP — CONSOLIDATED CHANGE PLAN (2026-08-14)

The Strike-tier gaming-UDP failure chain (raw SS UDP → hostile school-network
UDP policy; UoT removed because shadowsocks-rust RSTs SagerNet UoT) is
consolidated into an implementation plan at `v5/docs/GAMING-UDP.md`.

**Core change (P0, not yet implemented):** carry game UDP inside the TCP
tunnel via SagerNet UDP-over-TCP, with a **server that implements it**
(additive sing-box server instance on a new port is the leading option —
shadowsocks-rust can't do UoT and RSTs it). The server choice is an open
decision with a tradeoff matrix in the plan (§5a: sing-box server vs Xray
SS+uot vs shadowsocks-rust+udp2raw vs raw-UDP status quo); the deciding
interop test can't run until testing resumes. P1 validation experiments
(VPS-direct byte-exact LiteNetLib probe, packet-rate ladder, server
aliveness) are deferred until testing is possible. See the plan doc for the
full file-change map and sequencing.

---

## MACOS RE-ENABLED — UNSIGNED BUILD (2026-08-14)

Round-2 culling dropped macOS (unsigned builds were deemed unusable).
Revisited when it became clear macOS is ~50% of the target market; decision:
ship the **unsigned** build with a documented Gatekeeper workaround (right-click
→ Open, or `xattr -cr /Applications/MyVPN.app`). Full end-user instructions
will live on the separate download site.

What was restored (all previously removed in the culling commits):
- `tunnel.go`: `darwinTUN`, `killSwitchDarwin` (pfctl), `setDNSDarwin`
  (networksetup), darwin switch cases + interface assertion
- `activation/fingerprint_darwin.go`: self-contained darwin fingerprint
  (networksetup / ioreg), mirroring the linux/windows layout
- `updater/update_darwin.go`: darwin binary swap + fork (`swapDarwin`/`forkDarwin`)
- `darwin_link.go`: `UTType` cgo shim (Wails + Xcode 26 SDK linker fix)
- `manager/process.go`: elevation case back to `linux, darwin`
- `updater.go` + `heartbeat.go`: `DownloadURLMacOSIntel/ARM`,
  `UpdateMacOSIntel/ARM` fields + darwin case in `PlatformDownloadURL`
- `server/pb_hooks/heartbeat.pb.js`: `update_macos_intel`/`arm` field emission
- `.github/workflows/build.yml`: macOS-amd64 + macOS-arm64 matrix entries,
  sing-box darwin download case, release-table macOS rows + unsigned note
- `v5/client/Makefile`: `build-macos-intel` / `build-macos-arm` + build-all

**Verified:** `go build ./...` and `go vet` pass for darwin arm64 + amd64
(cross-compile), plus linux + windows. The full `wails build` (frontend
embed) must be validated on a macOS runner (CI) before shipping a release.

---

## FRESH VPS DEPLOY 2026-08-14 — BUGS FOUND & FIXED (134.199.155.166)

First deploy to the new VPS (Ubuntu 22.04, 1vCPU/1GB, DigitalOcean syd1).
DNS for networkingguides.duckdns.org now points at the new IP. The deploy
surfaced a stack of real bugs — all fixed in this round:

| # | Area | Bug | Fix |
|:-:|------|-----|-----|
| D1 | `00-env.sh` | Hard RAM gate (<1GB FAIL) rejected a 957MB-class VPS that runs the whole stack fine | Fail only <512MB; <1GB is a warning |
| D2 | `05-caddy.sh` | `list-modules | grep -q` under `set -euo pipefail`: grep -q closes the pipe → caddy SIGPIPE → pipefail turns the check into a FALSE NEGATIVE (a good binary was rejected) | Use `grep -c` (consumes all input); fixed in 3 places |
| D3 | `05-caddy.sh` | caddyserver.com download API ignores the version pin (asked 2.8.4, got 2.11.4) and occasionally returns a build WITHOUT the plugin | Added deterministic xcaddy fallback build; keeps API fast-path |
| D4 | `07-backups.sh` | `ensure_sqlite3` called before its definition → "command not found", setup died | Function moved above the Main section |
| D5 | `setup.sh` | Post-module service-start list only started shadowsocks-* — tc-*-cap services (and sing-box-uot) were never started on fresh deploys → caps silently unenforced | Start list now includes tc-*-cap + sing-box-uot (when ENABLE_UOT=1) |
| D6 | `02-shadowsocks.sh` UoT | sing-box REJECTS `"network": "tcp_and_udp"` (shadowsocks-rust syntax) and rejects `"udp_over_tcp"` on the INBOUND (unknown field) | Both omitted — sing-box defaults to tcp+udp and handles UoT magic-domain connections automatically |
| D7 | `seed-pb.py` | `api()` never sent the Authorization header → admin token "verification" always failed, then old `/api/admins` endpoint (removed in PB 0.22) → bootstrap never created collections | Global TOKEN threaded through api(); PB 0.22 `_superusers` endpoints; saved-password re-auth fallback |
| D8 | `seed-live.py` | Same PB 0.22 admin migration needed (`/api/admins` → `_superusers`) | Same fix |
| D9 | seed schemas | `update_config` still declared `download_macos_*` fields | Removed (macOS dropped) |
| D10 | `scripts/generate_codes.sh` | `double=!$double` in bash assigns the literal string `!false` → EVERY generated code had a wrong Luhn checksum → all codes rejected as "Invalid code format" by the client AND the activation hook | Proper toggle `if $double; then double=false; else double=true; fi` |
| D11 | code generator docs | Instructions passed `/root/.admin_api_token` (app-level token) — PocketBase 0.22 rejects it (401). The generator needs the PB admin JWT | Docs + setup.sh hint now use `grep PB_TOKEN /root/.pb_admin_creds \| cut -d= -f2` |

**Deployment notes (streamlined process):**
- Update DNS BEFORE running setup.sh — `00-env.sh` hard-fails if the domain doesn't resolve.
- `scp -r v5/server age-key.txt root@VPS:/root/server/` — age-key.txt must sit alongside setup.sh.
- `ENABLE_UOT=1 DOMAIN=… ./setup.sh` installs the sing-box UoT endpoint (port 8446) + seeds `uot_port` via seed-pb.py.
- Caddy per-IP rate limits are tight for school NAT (activate 5/10min/IP, heartbeat 1/10s/IP, api 100/10s/IP) — fine for the current client cadence; revisit if multiple students share one egress IP.
- The old VPS's PocketBase DB (activation codes) remains in B2 (`vpsvpnbackup`, backups up to 2026-08-10 02:00 UTC) — decision 2026-08-14: fresh start, do NOT restore.

**Verified live:** 23/23 smoke checks; activation 200 with `uot_port:8446`; heartbeat 200 with config refresh; TLS 200; all 10 services active; backup timer hourly.

---

## STRIKE GAMING LATENCY — BUFFERBLOAT CONTROL (2026-08-14)

Gaming speed = latency + jitter, not Mbps (school RTT 51ms is already good).
The real server-side win was bufferbloat: the HTB tier classes used plain
FIFO queues, so when a tier ran at full bandwidth (e.g. someone downloading
at 200 Mbps), queuing added latency spikes to game traffic on the same tier.

Changes (applied live + in modules):

- `04-tc.sh` helper: **fq_codel leaf qdisc under every HTB class**
  (parent 1:10/1:20/1:30, handles 10:/20:/30:) — per-flow fair queuing +
  CoDel AQM keeps latency flat under load (the standard BBR pairing).
  UoT traffic (port 8446, default class 1:30) benefits too.
- `01-bbr.sh` 91-tcp-tune.conf: `tcp_slow_start_after_idle = 0` (games that
  alternate quiet/burst don't re-slow-start every round) +
  `tcp_mtu_probing = 1`.
- `04-tc.sh` module bugs fixed while doing this:
  - helper script is now ALWAYS rewritten (was skip-if-exists → a stale
    helper persisted on deployed VPSs and helper updates never applied)
  - filter flush moved to the module main (once, before applying all three
    tiers) — the per-call flush design would have left only the LAST tier's
    filter; repeated adds without any flush duplicate filters (both hit
    during the 2026-08-14 apply)

Verified live: 3 filters, fq_codel under all 3 classes, sysctls applied.

Expected effect: latency stays flat on Strike when the tier is saturated;
game RTT (already ~51ms school → Sydney) unchanged at low load.

---

## NEW VPS — VOYAGER-PROBLEM REGRESSION TEST (2026-08-14)

Tested 134.199.155.166 against every failure class that plagued the old
Voyager VPS. Results:

| Old-Voyager problem | Test | Result |
|---------------------|------|:------:|
| IP drift (domain pointed at dead IP, cached clients broke) | duckdns resolves to 134.199.155.166 from VPS + public resolvers | ✅ |
| tc caps evaporate on kernel update / reboot | **Full reboot** → all 10 services active, exactly 3 tc filters attached post-boot | ✅ |
| "Can't validate the SS leg on the VPS" (TUN captured own egress) | sing-box client on the VPS → :8445 → curl example.com → **HTTP 200 in 89ms** | ✅ |
| UoT RST'd by shadowsocks-rust (~300ms) | sing-box client `udp_over_tcp` → :8446 → DNS query to 1.1.1.1:53 → **reply returned (2 answers)** — first working UoT round-trip on this stack | ✅ |
| Backups silently stopped (old box's last backup 2026-08-10) | B2 shows 24 fresh `.db.gz` backups from Aug 13-14; timer active; checksum verified | ✅ |
| `source <(age -d …)` secrets silently failing | setup.sh decrypts via age-key.txt temp-file path; tier passwords match configs + tier_configs | ✅ |
| PB hooks broken → generic 400 on activation | Activation returns proper "Missing code" 400; heartbeat 200; health 200; TLS 200 | ✅ |
| "Empty tier_configs" red herring | tier_configs populated (eco/stealth/strike, strike has uot_port) | ✅ |
| B2 CLI syntax drift / restore-path bugs | b2 CLI 4.7.1 installed; backup upload + checksum verification working | ✅ |
| Admin-password drift after restore | PB admin JWT in /root/.pb_admin_creds valid (seeds + API calls succeed) | ✅ |

**Remaining (needs the user's environment):** real SCP:SL game session on
the school network (P1 in GAMING-UDP.md). Also note: school network shapes
downloads per-flow (~2.5 Mbps/flow; 25 Mbps aggregate with parallel flows,
113 Mbps up) — a last-mile policy, not a VPS defect.

---

## HISTORY ARCHIVE — CURATED ORIGINALS RESTORED (2026-08-14)

After the culling rounds 1–2 deleted `originals/` wholesale, review flagged
that the business model and N4L threat research were unique knowledge with no
living-doc equivalent. The still-relevant docs were restored from git history
(`72eec8a^:originals/`) into a clearly-labelled archive:

- `v5/docs/history/Business-Plan.md` — middleman distribution, market vs xVPN
- `v5/docs/history/attacker-perspective.md` — insider-threat / covert-telemetry analysis
- `v5/docs/history/defender-perspective.md` — defense-in-depth research
- `v5/docs/history/comprehensive-vpn-blocklist.md` — N4L/Palo Alto blocklist research
- `v5/docs/history/school-vpn-blocking-implementation.md` — N4L network-level blocking
- `v5/docs/history/Architectural-Plan.md` — original v3 Hysteria 2 plan (context)

**Not restored** (stays in git history only): `Actionable-Plan.md` (4,771-line
superseded action plan), `V2-ACTION-PLAN.md`, `V2-BOTTLENECK-REFERENCE.md`,
and the entire `v3/` + `simplified/` alternative doc sets.

`v5/docs/history/README.md` indexes everything and gives the `git show`
commands for anything still only in history.

---

## REPO CULLING ROUND 2 — STREAMLINED LAYOUT (2026-08-13)

Continuation of the 2026-08 cleanup (round 1 removed `v3/`, `simplified/`,
`originals/`, `v5/legacy/`). Round 2 consolidates duplicates and drops dead
platform support. All removed files are recoverable from git history.

| # | Change | Rationale |
|:-:|--------|-----------|
| C1 | `modular-vps/` deleted | Stale duplicate of `v5/server/` (untouched by commits since 2026-07-27). `v5/server/` is the one and only server deployment directory. Its `hiddify.pb.js` hook was moved into `v5/server/pb_hooks/`. |
| C2 | Root `CONTEXT.md` deleted | Superseded by `v5/CONTEXT.md` — one context document. |
| C3 | `v5/scripts/` deleted | Duplicate of root `scripts/` (which holds the newer hardened `generate_codes.sh`). Root `scripts/` is canonical; all docs updated to `./scripts/…`. |
| C4 | macOS (darwin) support dropped from client | Unsigned builds are unusable on macOS (no Apple signing/notarization). **REVERSED 2026-08-14** — macOS was ~50% of market, so the unsigned build is shipped with a documented Gatekeeper workaround (right-click → Open / `xattr -cr`). See "MACOS RE-ENABLED" below. |
| C5 | Fingerprint files made self-contained | `fingerprint.go` (shared) + `fingerprint_darwin.go` deleted; shared logic (hashSources, UUID fallback, ValidateFingerprint, caching) folded into `fingerprint_linux.go` and `fingerprint_windows.go`. `update_unix.go` → `update_linux.go` (linux-only, macOS gone). |
| C6 | `POCKETBASE-SETUP.md` leaked admin token | Five hardcoded `ADMIN_API_TOKEN` values replaced with `YOUR_ADMIN_TOKEN` placeholder. |

**Docs updated:** root `README.md`, `v5/README.md`, `v5/CONTEXT.md`,
`v5/docs/ARCHITECTURE.md`, `BACKEND-API.md`, `CLIENT-GUIDE.md`, `CI-CD.md`,
`API.md`, `OPS.md`, `DEPLOY.md`, `POCKETBASE-SETUP.md`, `IMPLEMENT.md`,
`WAILS-MIGRATION.md`, `.github/workflows/build.yml`, `v5/client/Makefile`.

---

## DEPLOY/RESTORE — PLUG-N-PLAY HARDENING (2026-08-01)

Validated a full blank-VPS deploy + B2 restore live (114.23.136.59). The deploy
path was already one-command; the restore path had gaps, all fixed:

| # | Area | Issue | Fix |
|:-:|------|-------|-----|
| R1 | `restore.sh` | Latest-backup detection used `b2 ls --long "$BUCKET" backups/` — plain bucket names fail on the current CLI ("Invalid B2 URI") | `b2 ls --recursive "b2://$BUCKET/backups/"` filtered to `.db.gz`, sorted |
| R2 | `restore.sh` | Download/checksum used deprecated `b2 download-file-by-name` / `b2 file-info` (exit 1 + error output on success paths) | Current syntax: `b2 file download b2://…` / `b2 file info b2://…` |
| R3 | `restore.sh` | Restored `data.db` over a fresh-seeded DB without removing `-wal`/`-shm` — stale WAL can be replayed against the restored file | `rm -f data.db-wal data.db-shm` before chown |
| R4 | `restore.sh` | Restored DB's admin password may predate the secrets file (old auto-generated era) → login fails with documented creds | Auto-run `./pocketbase admin update "$PB_ADMIN_EMAIL" "$PB_ADMIN_PASS"` (while PB stopped) |
| R5 | `restore.sh` | `pocketbase-backup.timer` has `Requires=pocketbase.service` — stopping PB (part of restore) **stops the timer too** (Requires propagates stops, not starts); it stayed inactive after restore | Explicit `systemctl restart pocketbase-backup.timer` + `is-active` verification after start |
| R6 | `restore.sh` | Codes-count check used the API with a nonexistent token file → always 0/unknown (collections are admin-only) | Direct `sqlite3 … "SELECT COUNT(*) FROM codes"` |
| R7 | `restore.sh` | No hook-load verification after restore | POST `/api/activate {}` must return 400 "Missing code" (generic 400 = hook load error) |
| R8 | `07-backups.sh` | `sqlite3` not installed → backup script silently used the direct-copy fallback (inconsistent backups under write load) | Module now installs `sqlite3` so `.backup` (safe) is always used |
| R9 | `07-backups.sh` | Timer enable/restart swallowed errors with `|| true` | Loud enable/restart + `is-active` verification with manual-fix instructions |

**Docs updated:** `DEPLOY.md` (post-deploy checklist now includes backup timer +
backup log + update.json + the Requires= gotcha; disaster-recovery section
describes the full one-command flow), `OPS.md` (restore runbook rewritten with
the verified manual steps + current b2 syntax + admin password alignment +
timer restart), `v5/CONTEXT.md` (agent rules 7–9: timer gotcha, admin password
alignment, b2 URI syntax).

**Verdict:** a blank Ubuntu 22.04 VPS + repo + `age-key.txt` is fully plug-n-play:
`setup.sh` for fresh deploy (auto-runs the smoke test), `restore.sh` for
migration/disaster recovery — both validated live 2026-08-01.

---

## STEALTH TIER — TCP BRUTAL REMOVED, BACK TO BBR (2026-08-01)

**Decision:** Stealth no longer uses the `tcp-brutal` kernel module + LD_PRELOAD
wrapper. The aggressive rate-based CC filled buffers and caused bufferbloat/jitter
on the school network when it throttled, and the Linux `tcp-brutal` module is not
as robust or powerful as Hysteria 2's built-in Brutal CC. Stealth now runs plain
**BBR** like the other tiers, with a **tc HTB cap of 100 Mbps** (class `1:20`,
port 8444) replacing the old 48 Mbps Brutal target rate.

**Changes:**
- `v5/server/modules/03-brutal.sh` — **deleted** (kernel module build/load,
  DKMS registration, `brutal-wrap.so` LD_PRELOAD wrapper, 48 Mbps target).
- `v5/server/modules/02-shadowsocks.sh` — removed `write_brutal_dropin()`
  (the `LD_PRELOAD=/usr/local/lib/brutal-wrap.so` drop-in for the Stealth
  systemd service); Stealth service description is now "BBR, 100 Mbps tc cap".
- `v5/server/modules/04-tc.sh` — Stealth now gets a tc class `1:20` @ `100mbit`
  and a `tc-stealth-cap.service` oneshot (alongside Eco `1:10` and Strike `1:30`).
- `v5/server/setup.sh` — `03-brutal.sh` removed from the module chain; summary
  line updated to "Stealth port: 8444 (BBR, 100 Mbps tc)".
- `v5/server/scripts/smoke-test.sh` — Brutal module check replaced with a
  Stealth tc class (`1:20`) check; tc verification now covers 8443/8444/8445.
- Docs updated: root `README.md`, `v5/CONTEXT.md`, `v5/docs/ARCHITECTURE.md`,
  `v5/docs/DEPLOY.md`, `v5/docs/OPS.md` (tier tables, diagrams, deploy
  checklist, ops checks, kernel-update guidance).
- Historical references in `v3/`, `v4/`, `simplified/`, `originals/`,
  `modular-vps/` and root `CONTEXT.md` are left untouched (reference-only).

**Ops note:** no kernel modules are maintained anymore. After a kernel update,
re-apply the caps with `systemctl restart tc-eco-cap tc-stealth-cap tc-strike-cap`.
The old Brutal entries below this one are historical records of the earlier design.

---

## CI LINT — ERRCHECK FIXES (Wails Migration)

Found when the Wails CI pipeline first ran `golangci-lint` on the client module.
All fixes are behavior-neutral (`_ =` acknowledges best-effort/cleanup errors).

| # | File | Line | Fix |
|:-:|------|:----:|-----|
| L1 | `v5/client/internal/storage/storage.go` | 159 | `f.Sync()` → `_ = f.Sync()` (best-effort fsync) |
| L2 | `v5/client/internal/updater/updater.go` | 165 | `u.restoreBackup(...)` → `_ = ...` (cleanup after swap failure; original error already returned) |
| L3 | `v5/client/internal/updater/updater.go` | 364 | `os.Chmod(...)` → `_ = ...` (post-revert permission set) |
| L4 | `v5/client/internal/manager/process.go` | 261 | `m.cmd.Wait()` → `_ = ...` (graceful shutdown goroutine) |
| L5 | `v5/client/internal/manager/process.go` | 409 | `cmd.Wait()` → `_ = ...` (startup probe after immediate exit) |
| L6 | `v5/client/internal/tunnel/tunnel.go` | 140 | `iptables -F OUTPUT` → `_ = ...Run()` (best-effort rule removal) |
| L7 | `v5/client/internal/tunnel/tunnel.go` | 163 | `pfctl -F all` → `_ = ...Run()` (best-effort) |
| L8 | `v5/client/internal/tunnel/tunnel.go` | 164 | `pfctl -d` → `_ = ...Run()` (best-effort) |
| L9 | `v5/client/internal/tunnel/tunnel.go` | 257 | `t.Stop()` → `_ = t.Stop()` (rollback on TUN setup failure) |
| L10 | `v5/client/internal/tunnel/tunnel.go` | 294 | `t.Stop()` → `_ = t.Stop()` (rollback on TUN setup failure) |

Also fixed siblings not flagged by the linter: `f.Close()` in the same storage block,
and the `linuxTUN.Stop()` best-effort cleanup loop.

---

## CI LINT — REMAINING ERRCHECK + UNUSED FIXES (2026-07-31)

`golangci-lint run ./...` (v2.12.2, default linters — the CI `lint` job) still
failed after the L1–L10 batch. 26 more issues fixed, all behavior-neutral
(`_ =` acknowledgements; two genuinely dead functions removed):

| # | File | Fix |
|:-:|------|-----|
| L11 | `v5/client/app.go` | `a.mgr.Stop()` → `_ = ...` (disconnect) |
| L12 | `v5/client/app.go` | `a.store.SetHeartbeat(...)` → `_ = ...` (heartbeat callback) |
| L13 | `v5/client/app.go` | `a.store.SetHeartbeatFailure(...)` → `_ = ...` (heartbeat callback) |
| L14 | `v5/client/internal/activation/activation.go` | `defer resp.Body.Close()` → `defer func() { _ = ... }()` |
| L15 | `v5/client/internal/heartbeat/heartbeat.go` | `defer resp.Body.Close()` → closure |
| L16 | `v5/client/internal/heartbeat/heartbeat.go` | **Removed dead `getInterval()`** (unused) |
| L17 | `v5/client/internal/manager/process.go` | `defer conn.Close()` → closure (helper IPC client) |
| L18 | `v5/client/internal/manager/process.go` | `os.Remove(configPath)` → `_ = ...` (stop cleanup) |
| L19 | `v5/client/internal/storage/storage.go` | `os.Remove(tmpPath)` → `_ = ...` (save failure cleanup) |
| L20 | `v5/client/internal/storage/storage.go` | `os.Remove(oldest)` → `_ = ...` (backup rotation) |
| L21 | `v5/client/internal/tunnel/tunnel.go` | `netsh ... delete rule` → `_ = ...Run()` (Windows kill switch off) |
| L22 | `v5/client/internal/tunnel/tunnel.go` | `networksetup ... Ethernet` → `_ = ...Run()` (macOS DNS fallback) |
| L23 | `v5/client/internal/tunnel/tunnel.go` | `ifconfig down` → `_ = ...Run()` (darwinTUN.Stop) |
| L24 | `v5/client/internal/updater/updater.go` | 5× `defer {Body,f}.Close()` → closures (download/checksum/copy) |
| L25 | `v5/client/internal/updater/updater.go` | 13× `os.Remove(...)` → `_ = ...` (cleanup paths in PerformUpdate/downloadBinary/CheckOnStartup) |
| L26 | `v5/client/internal/updater/updater.go` | **Removed dead `cleanupSentinelFiles()`** (unused) |
| L27 | `v5/client/internal/updater/recover.go` | 6× `os.Remove(...)` → `_ = ...` (sentinel/marker cleanup) |
| L28 | `v5/client/internal/updater/update_windows.go` | 2× `os.Remove(oldPath)` → `_ = ...` (.old cleanup in swapWindows) |

Also ran `gofmt -w` across the module (pre-existing alignment drift in const/var/struct
blocks — cosmetic only) and synced `frontend/package.json` to the committed
`package-lock.json` (`vite ^6.4.3` → `^5.4.21`) so CI `npm install` is
deterministic. Verified green locally: `golangci-lint run ./...` (0 issues),
`go vet ./...`, `go build -tags frontend` (linux/windows/darwin amd64+arm64),
`go test ./...`, and a clean `npm ci && npm run build`.

---

## CI/RUNTIME — MISSING WAILS BUILD TAGS + BROKEN JS BRIDGE (2026-07-31)

**Symptom:** Windows binary built by CI showed the Wails error dialog
*"Wails applications will not build without the correct build tags"* and
opened https://wails.io/docs/guides/manual-builds/.

**Root cause 1 — build tags:** Wails v2 selects its app implementation with
build tags (`internal/app/app_default_*.go` is `//go:build !dev && !production
&& !bindings`). The CI workflow compiled with only `-tags frontend` (which just
selects the embedded asset FS), so the *stub* implementation shipped: a binary
that shows the error dialog. `wails build` adds `desktop,production` itself;
raw `go build` must pass them explicitly.

**Fix:** `.github/workflows/build.yml` now uses `-tags "frontend desktop production"`
in the lint job (`go vet`, compile check) and the 4-platform build matrix.

**Root cause 2 — broken frontend bridge:** `frontend/src/lib/bridge.ts` called
`window.runtime.Call('GetVersion', …)`, but Wails v2.9's runtime does NOT expose
`Call` on `window.runtime` (only Log/Window/Events/etc.), and method names must
be qualified (`main.App.GetVersion`) per the binding DB. Every UI action would
have thrown after the tags were fixed.

**Fix:** `bridge.ts` now calls `window.go.main.App.<Method>(…)` (the bindings map
the backend injects at startup — the generated `wailsjs/` files are only optional
IDE helpers) and keeps `window.runtime.EventsOn/EventsOff` for events.

**Verified:** Windows binary built with the real tags embeds
`-tags=frontend,desktop,production` (per `go version -m`) and no longer contains
the stub error string. Frontend rebuilds cleanly. Linux desktop build requires
WebKitGTK headers locally (CI installs `libgtk-3-dev libwebkit2gtk-4.1-dev` —
includes `pkg-config`); macOS builds require CGO on a macOS runner (matrix
already sets `cgo: "1"` for macOS).

### Follow-up (2026-07-31) — Linux lint failure: missing `webkit2_41` tag

The first green push still failed the `Lint & Vet` job on ubuntu-latest. Cause:
Wails' Linux desktop cgo code selects the WebKitGTK version via a build tag —

```c
#cgo !webkit2_41 pkg-config: webkit2gtk-4.0
#cgo webkit2_41 pkg-config: webkit2gtk-4.1
```

ubuntu-latest (24.04) ships only WebKitGTK **4.1** (`libwebkit2gtk-4.1-dev`),
so the tag-less build looked for `webkit2gtk-4.0` and failed at the pkg-config
step. The `wails` CLI does not auto-add this tag in v2.9.1 — it must be passed
manually. **Fix:** `.github/workflows/build.yml` adds `webkit2_41` to the lint
job (`go vet` / compile check) and the Linux matrix entry; macOS/Windows keep
`frontend desktop production` (the tag is linux-only). Docs and `main.go`
comments updated to mention `-tags "frontend desktop production webkit2_41"`
for Ubuntu 24.04+ builds.

---

## WINDOWS RUNTIME — BLANK POWERSHELL FLASH + INVISIBLE APP (2026-07-31)

**Symptom:** launching the Windows exe popped a blank PowerShell window, then
"nothing happened".

**Root cause 1 — PowerShell console flash:** the device fingerprint collector
(`internal/activation/fingerprint_windows.go`) spawns `powershell.exe` 3×
(Get-NetAdapter, Win32_DiskDrive, Win32_ComputerSystemProduct). A GUI-subsystem
parent spawning console-subsystem children gets a **visible console window per
child** on Windows.

**Fix:** all PowerShell spawns now run via `runHidden()` with
`syscall.SysProcAttr{HideWindow: true}`.

**Root cause 2 — invisible app:** `main.go` set `StartHidden: true`, but Wails
v2.9.1 has **no system tray API** (verified: no `SystemTray` in `pkg/runtime` /
`pkg/options`) and this app never creates a tray icon — the `tray:show` /
`tray:quit` listeners in `setupSystemTray` are dormant hooks nothing emits.
Result: the app ran completely invisible ("nothing happened"). Wails v2.9 also
has no close-to-hide interception, so closing the window quits.

**Fix:** removed `StartHidden` (window is shown on launch); documented the
dormant tray hooks and close-quits behaviour in code comments and docs.

**Also fixed:** `manager/process_windows.go` `newProcAttr()` now sets
`HideWindow: true` so sing-box (console subsystem) doesn't flash a console when
the user connects. Docs (`ARCHITECTURE.md`, `CLIENT-GUIDE.md`,
`UI-AESTHETICS.md`) updated to match.

---

## MACOS BUILD — UNDEFINED `_OBJC_CLASS_$_UTType` (2026-07-31)

**Symptom:** both macOS matrix jobs (`Build macOS-amd64`, `Build macOS-arm64`)
failed at the "Build client app" step with a final-link error:

```
Undefined symbols for architecture arm64: "_OBJC_CLASS_$_UTType"
```

**Root cause:** Wails v2.9.1's darwin frontend uses `UTType` for file dialogs
(`WailsContext.m:575,659`, `UTType typeWithFilenameExtension:`) but its cgo
LDFLAGS only link `Foundation`, `Cocoa`, `WebKit`. On older SDKs the
`UniformTypeIdentifiers` framework was re-exported transitively; the
Xcode 26 / macOS 26 SDK on `macos-latest` removed that implicit linkage.

**Fix:** `v5/client/darwin_link.go` (`//go:build darwin`) adds the missing
framework to the final link:

```go
#cgo LDFLAGS: -framework UniformTypeIdentifiers
```

cgo flags from the main package are included in the final link step, so this
covers both `wails build` and manual `go build` on amd64/arm64. Non-darwin
builds are unaffected (build tag). Upstream Wails v2.10+ fixes this properly
in the darwin package itself.

---

## WINDOWS RUNTIME — BLACK WINDOW FLASHES THEN APP DIES (2026-07-31)

**Symptom:** the window (black) flashes for ~1 second then closes; no process
remains in Task Manager. Wails runs `OnStartup` in a goroutine with no
recovery, and `wailsruntime.LogFatal` calls `os.Exit(1)` — for a GUI build
(no console) any startup failure or panic dies **completely silently**.

**Likely trigger:** a corrupt `storage.json` (e.g. left by the earlier
invisible/crashed sessions) → `storage.New` failed → `LogFatal` → `os.Exit(1)`
about one second after launch (the visible gap = the hidden PowerShell
fingerprint calls). The black window is the WebView mid-load when the process
dies.

**Fixes (make failures impossible to hide):**
1. `internal/storage/storage.go` — `New()` is now **self-healing**: an
   unreadable/corrupt `storage.json` is moved aside
   (`storage.json.corrupt-<unix-ts>`) and a fresh store is created; config-dir
   failures fall back to the OS temp dir. A bad JSON file can no longer brick
   startup.
2. `main.go` — `os.Stderr` and the standard logger are redirected to
   `%APPDATA%\locus\myvpn.log` (rotated at 1MB). Panics, `log.Fatal` and
   `log.Printf` output are now captured on GUI builds with no console.
3. `app.go` — `Startup` wraps its body in `recover()` (logs the panic to
   `myvpn.log` and keeps the window alive) and the storage failure path uses
   `LogError` instead of `LogFatal` (no more `os.Exit(1)`).
4. `main.go` — `wails.Run` errors still exit, but the message lands in
   `myvpn.log` instead of a null console.

**Diagnosis path for future Windows issues:** run the exe, then read
`%APPDATA%\locus\myvpn.log` — any panic stack or startup error will be there.

### Follow-up (2026-07-31) — exit-path instrumentation

With the log in place, a fresh CI build (run 42 — all jobs green incl. macOS)
still flashed and died with ONLY `MyVPN starting (version main)` in the log —
no panic, no `wails.Run` error. Since go-webview2 `log.Fatalf`s on WebView2
env/controller failure (which would have been logged), WebView2 init succeeded;
the death is either a native crash, a browser-process failure, or an external
kill (school-managed machines: AV/AppLocker). Added stage markers to the log:

- `DOM ready — webview loaded the UI` (new `OnDomReady` hook in `main.go`)
- `Startup complete (activated=...)` (end of `App.Startup`)
- `Shutting down MyVPN...` (`App.Shutdown` — present iff the app exited via
  the normal window-close path)

Combined with the Windows Event Viewer (Application log → "Application Error"
for `myvpn.exe`, showing the faulting module), the next run identifies the
exact dying stage.

### Follow-up 2 (2026-07-31) — WAILS v2.9.1 → v2.12.0 (go-webview2 crash fixes)

The instrumented log showed `MyVPN starting` → `DOM ready — webview loaded the
UI` and then **silent death**, with `Startup complete` and `Shutting down`
never logged — i.e. the process died right when the Vue app started calling
bound Go methods over the WebView2 JS↔Go IPC. Wails v2.9.1 bundles
`go-webview2 v1.0.10` (2023-10), which predates the upstream crash fixes:

| go-webview2 | Fix |
|-------------|-----|
| v1.0.12 | infinite recursion fix |
| v1.0.13 | overlapped I/O error on long JS scripts |
| v1.0.16 | **panic when sending long data from JS to Go** |
| v1.0.19 | COM error handling |
| v1.0.20/21 | **random crashes** |

**Fix:** upgraded `github.com/wailsapp/wails/v2` **v2.9.1 → v2.12.0** (needs
only Go 1.22, so CI is unchanged) which bundles **go-webview2 v1.0.22** with
all of the above. Verified: Windows build (real tags), golangci-lint 0 issues,
`go vet`, `go test` all green. The darwin `UTType` shim and the linux
`webkit2_41` tag remain required in v2.12.0 (confirmed in its source).

---

## SERVER-SIDE — STALE/EMPTY tier_configs REJECTS CLIENTS (2026-07-31)

**Symptom:** with the app running (as admin), Connect worked but the tunnel
died instantly: `inbound/tun[tun-in]: ... wsasend: An existing connection was
forcibly closed by the remote host` (both upload and download) to
`114.23.136.59:8445`. TCP connects fine (all ss ports open) — the ssserver
**rejects the Shadowsocks handshake**: wrong password/method.

**Investigation (verified from outside):**
- DNS: `networkingguides.duckdns.org` → **114.23.136.59** (older docs say
  .47 — the VPS IP changed; docs updated).
- All three tier passwords from `secrets.env.age` were tested against the live
  ss servers with a real sing-box client — **all three authenticate**
  (HTTP 200 through the tunnel).
- **Correction (with SSH access, the story simplified):** the live
  `tier_configs` collection was **correct all along** — all three records
  match `/etc/shadowsocks/*.json` exactly, one record per tier. The earlier
  "empty collection" conclusion was wrong: the collection's API rules are
  admin-only (`@request.auth.admin = true`), so unauthenticated list/create
  requests returned misleading empty/generic-400 responses.
- **The real problem is the CLIENT's stale stored config**: the device was
  activated on the PREVIOUS PocketBase instance (before the Jul 28 re-setup
  replaced the data dir and rotated the ss passwords). The current client
  never refreshes stored connection parameters:
  - the heartbeat hook returns `server_config`, but the client ignored it,
  - re-activation short-circuits client-side ("Already activated" returns
    before any server call),
  - so the app kept connecting with an old password → ssserver RST.

**Fixes (repo + deployed to VPS):**
- `v5/server/pb_hooks/activation.pb.js` + `heartbeat.pb.js` — always pick the
  **newest** `tier_configs` record; the "Already activated" response now
  includes `server_config`. NOTE: `{:param}` binding is unreliable in
  `findRecordsByFilter` on PB 0.22 — filters use sanitized inline values.
- `v5/server/scripts/seed-pb.py` — tier seeding is an **idempotent upsert**
  (delete older duplicates, PATCH existing or POST new).
- `v5/server/scripts/fix-tier-configs.py` — repair script for the VPS
  (ground-truth passwords from `/etc/shadowsocks/*.json`).
- Client (`v5/client/app.go`): heartbeat responses now **apply
  `server_config`** (self-heal within one heartbeat once a client with this
  fix runs).
- Client (`v5/client/internal/manager/process.go`): sing-box stderr captured
  and surfaced in Connect errors ("TUN interface creation was denied — run
  Locus as administrator: ... Access is denied.").

**Live-VPS actions performed (2026-07-31, authorized):** deployed both hooks
to `/opt/pocketbase/pb_hooks/` (+ `/root/server/pb_hooks/`), restarted
PocketBase, verified the heartbeat route serves the correct `server_config`
(end-to-end test with a real code: eco → `networkingguides.duckdns.org:8443`
+ matching password). Client unblock: run the new build (heartbeat refresh)
or delete `%APPDATA%\locus\storage.json` and re-enter the activation code.

---

## WINDOWS RUNTIME — CONNECT SPINS FOREVER (Process.Signal(0) BROKEN) (2026-08-01)

**Symptom:** after the server fix, Connect starts sing-box (correct password,
TUN created) but the button spins for minutes with no new log lines — while
the tunnel actually runs in the background.

**Root cause:** the manager checked process liveness with
`cmd.Process.Signal(syscall.Signal(0))` everywhere (startup probe, IsRunning,
State, healthLoop, already-running check). On **Windows**, `Process.Signal`
only supports `Kill` (TerminateProcess) — any other signal returns
`syscall.EWINDOWS` ("not supported by windows") — verified in Go's
`src/os/exec_windows.go`. So every check reported "dead":
- the 500 ms startup probe concluded sing-box exited, called `cmd.Wait()`,
  which **blocks while sing-box is alive** → `Connect()` never returns → the
  UI spinner runs forever;
- closing the app then spawned a second `cmd.Wait()` in `stopLocked`, which
  errors immediately ("Wait was already called"), skipping the kill → an
  **orphaned sing-box.exe** kept running.

**Fix (`v5/client/internal/manager/process.go`):** replaced ALL signal-based
liveness checks with a cross-platform **exited channel**: a goroutine owns
`cmd.Wait()` and `close(exited)` when the process exits; `processAlive()`
selects on that channel. Applied to the startup probe (select vs 500 ms
timer), IsRunning, State, healthLoop (incl. the restart path — each restarted
process gets a fresh channel), stopLocked (waits on the existing channel; no
second Wait), and Start's already-running check.

**Tests added (`internal/manager/process_test.go`):** `TestLifecycle` (start →
running → auto-exit detected → crashed → stop → stopped) and
`TestImmediateExit` (probe surfaces sing-box stderr, e.g. "Access is denied").
Both pass. Windows build, golangci-lint, vet all green.

**Note for users of the previous build:** after closing the hung app, kill any
leftover `sing-box.exe` in Task Manager before running the new build.

---

## WINDOWS RUNTIME — CONNECTS BUT NO TRAFFIC (dial i/o timeout) (2026-08-01)

**Symptom:** with the fixed build, the UI reports Connected, but
whatismyip.com still shows the real IP and sing-box logs repeated
`inbound/tun[tun-in]: dial tcp 114.23.136.59:8445: i/o timeout` (5 s dial
timeouts every ~60-130 s). The heartbeat (direct, not via TUN) still works.

**Server verified healthy from an outside host:** all three ss ports open;
strike password authenticates (HTTP 200 through the tunnel in ~60 ms).
So the server is NOT the problem.

**Most likely cause — stale host state, not config:** the same sing-box config
reached the server in an earlier session (RST after connect = connection
established). The sessions in between (old builds with the broken shutdown)
**orphaned sing-box.exe processes and left the `locus0` TUN adapter with
stale routes/WFP filters**; a fresh sing-box then routes its own dial into the
dead TUN state → timeout. (The config already sets
`route.auto_detect_interface: true`, which binds the outbound to the physical
interface — so a clean host should not loop.)

**Diagnostics added (`app.go`):** the report now includes
`Server: <addr> reachable|UNREACHABLE (...)` — a 3 s TCP dial to the configured
server from the app (before/without the tunnel). This distinguishes:
- `UNREACHABLE` ⇒ the network blocks the ss port (e.g. different WiFi);
- `reachable` while the tunnel still times out ⇒ stale TUN/routes on the host
  (cleanup below).

**User-side cleanup (one-time, after the old builds):**
1. Close Locus; in Task Manager end ALL `sing-box.exe` processes.
2. As admin: `netsh interface show interface` → find `locus0` →
   `netsh interface delete interface locus0` (if present).
3. Reboot (clears routes + WFP filters), then Connect again.
4. Check Diagnostics: `Server: … reachable` + `Engine: running`.

### Follow-up — strict_route disabled (2026-08-01)

New diagnostics (VPN ON) showed the app's OWN dial failing at
`lookup networkingguides.duckdns.org: i/o timeout` — i.e. with the TUN up,
**DNS and everything else through the tunnel dies**, while direct Shadowsocks
(Hiddify, no TUN) works and the server is verified healthy. The prime suspect
was `strict_route: true` in the generated sing-box config: on Windows it
installs WFP filters that "strictly block all connections not from the TUN" —
a misfiring filter also blocks sing-box's own outbound and DNS, which matches
"connects but cuts out the internet" exactly.

**Fix:** `generateConfig` now sets `strict_route: false` (omitted from JSON;
`auto_route` + `auto_detect_interface` remain — still a full tunnel). If the
tunnel still fails after the cleanup+reboot, set `LOCUS_DEBUG=1` before
launching (switches sing-box to debug logging) and send the log — it shows the
dial's interface binding and route decisions.

### Follow-up 2 — DNS loop: final must be dns-tunnel (2026-08-01)

After disabling strict_route the tunnel egressed (ping 1.1.1.1 worked) but
**domains never resolved**. Root cause: `generateConfig` set
`dns.final = "dns-direct"`, whose server had `detour: "direct"` — the direct
outbound's DoH traffic is routed back into the TUN by auto_route → **DNS loop**
("dial tcp: lookup …: i/o timeout" while the tunnel itself was fine). This
also explains "the app has never worked": the proxy outbound was fine all
along; DNS was always looping.

**Fix:** `dns.final` is now `"dns-tunnel"` (DoH via the default proxy outbound
→ through the tunnel); `dns-direct` is kept but unused. Verified: Windows
build, lint, vet, manager tests green; docs (ARCHITECTURE.md, CLIENT-GUIDE.md)
updated to match.

### Follow-up 3 — DNS query loopback: explicit detour required (2026-08-01)

With `final: dns-tunnel` (no detour), sing-box flooded
`DNS query loopback in transport[dns-tunnel]` for every query. Verified in
`sing-box v1.10.0 route/router.go`: a DNS server with an **empty detour**
dials via `dialer.NewRouter(router)` — the transport's own connection is
re-routed by the route rules back into the DNS handler, and sing-dns's
context-based loop detection fires (`sing-dns client.go`).

**Fix:** `dns-tunnel` now has an explicit **`detour: "proxy"`** — the DoH dial
goes straight through the Shadowsocks outbound (`dialer.NewDetour`), bypassing
the router entirely. `TestGeneratedConfig` asserts the detour; all tests,
Windows build, lint, vet green.

### Follow-up 4 — THE missing piece: `sniff: true` on the TUN inbound (2026-08-01)

Verified empirically on the VPS (root, real TUN): without sniffing, DNS
queries were routed to the **shadowsocks outbound** (`outbound connection to
10.0.0.2:53` via proxy) — the `{"protocol": "dns", "outbound": "dns-out"}`
rule NEVER matched. Source: `route/router.go:856` gates ALL sniffing (which
sets `metadata.Protocol` for rule matching) behind
`metadata.InboundOptions.SniffEnabled` — i.e. the TUN inbound's `"sniff": true`
option. Every official sing-box TUN example includes it; our config never did
— this single omission explains every DNS failure variant (direct detour loop,
transport loopback, and the plain "no DNS at all" behavior).

With `"sniff": true` the VPS test confirmed the full interception pipeline:
`sniﬀed packet protocol: dns` → `match protocol=dns => dns-out` →
`dns: exchange example.com`. (The ss leg could not be validated on the VPS
itself — a TUN client on the ss server host captures the server's own egress —
but the tunnel was already proven working from remote hosts.)

**Fix:** TUN inbound now sets `"sniff": true`. `TestGeneratedConfig` asserts
it. Windows build, lint, vet, tests green; docs updated.

### Follow-up 5 — server-domain resolution loop (sing-box issue #2207) (2026-08-01)

Even with sniff + detour, every query failed instantly with
`DNS query loopback in transport[dns-tunnel]`. Root cause (confirmed via
sing-box issue #2207, closed as fixed upstream but still present in v1.10):
the Shadowsocks outbound's server is the **domain**
`networkingguides.duckdns.org` — when the DNS transport dials the DoH via the
proxy, the proxy must resolve that domain, and the resolution re-enters the
DNS system → the transport's own context (tagged `dns-tunnel`) hits the loop
detection in sing-dns.

**Fix (the reporter-confirmed workaround):** a DNS rule sending ALL
sing-box-initiated (outbound) resolution DIRECT:
`{"outbound": ["any"], "server": "dns-direct"}`. Plus a route rule excluding
the resolved VPN-server IP from the tunnel (`ip_cidr: <server>/32` → direct;
resolved at config generation, before the TUN exists) so a captured ss
connection egresses physically instead of looping.

**VPS validation (root, real TUN, exact config):** `sniffed protocol: dns` →
`match => dns-out` → exchange; DoH routed via the ss outbound; the server
domain resolved DIRECT (`lookup succeed: 114.23.136.59`, NOERROR) — **zero
loopback errors**. (The ss leg itself can't be validated on the VPS — the TUN
on the ss-server host captures the server's own egress — but the tunnel is
proven working from remote hosts.)

### Follow-up 6 — bind_interface: force the ss dial onto the physical NIC (2026-08-01)

With the DNS pipeline fixed, every exchange still timed out at 10 s — the DoH
through the tunnel never completes on the user's Windows machine, while the
same path works from remote hosts in ~350 ms (verified: DoH query through the
tunnel returns HTTP 200 + a valid DNS response) and Hiddify (no TUN) works on
the same machine. Conclusion: with the TUN up, sing-box's OWN Shadowsocks
dial is still being captured by auto_route on Windows — `auto_detect_interface`
is not sufficient there.

**Fix:** the proxy outbound now gets an explicit **`bind_interface`** — the
physical NIC is detected at Connect time, BEFORE the TUN exists
(`interface_windows.go`: hidden PowerShell `Get-NetRoute` for the 0.0.0.0/0
alias; `interface_unix.go`: `ip route show default`). A socket bound to the
physical interface cannot be captured by the TUN, so the ss connection always
egresses directly. `app.go` Connect passes the detected interface into
`manager.Config.BindInterface`; `generateConfig` emits
`"bind_interface"` on the shadowsocks outbound.

### Follow-up 7 — sing-box 1.10.0 → 1.12.1: THE fix (2026-08-01)

Despite every config safeguard (sniff, DNS detour, outbound:any rule,
server-IP rule, bind_interface), the ss connection STILL timed out with the
TUN up — on both the user's Windows machine AND the Linux VPS. Meanwhile
**Hiddify (also sing-box + TUN + Shadowsocks) worked on the same machine** —
its core runs a much newer sing-box.

**Validation on the VPS with sing-box 1.12.1** (root, real TUN, the 1.12
config format): `curl https://example.com` through the tunnel returned
**HTTP 200 in 84 ms** — the first end-to-end success ever, even in the
self-host scenario.

**Changes:**
- Engine bumped **1.10.0 → 1.12.1** (`.github/workflows/build.yml`,
  `engines/README.md`). 1.11 added "Improve tun compatibility" (3 fixes);
  1.12 refactored the DNS servers and route rules.
- Config rewritten for the 1.12 format: DNS servers use
  `{"type": "https", "server": "1.1.1.1", "server_port": 443, "detour": ...}`
  (no more `"address"`), the `dns-out` special outbound is gone (replaced by
  the route rule action `{"protocol": "dns", "action": {"type": "dns"}}`), and
  the server-IP exclusion uses `{"action": {"type": "route", "outbound":
  "direct"}}`.
- `TestGeneratedConfig` updated and passing; Windows build, lint, vet green.

### Follow-up 8 — REMOVED bind_interface: align with Hiddify exactly (2026-08-01)

The 1.12 config parsed and ran, but the ss dial STILL timed out with
`bind_interface: "Wi-Fi"`. Evidence review: the ONLY session where the ss dial
reached the server (23:35) had NO bind; Hiddify (works on the same machine)
uses NO bind — only `auto_detect_interface`; and sing-box's Windows interface
name resolution for `bind_interface` is unverified (an unresolvable name
silently breaks every dial — the exact observed symptom).

**Fix:** `bind_interface` is REMOVED from both outbounds. The direct outbound
is kept non-empty via `"connect_timeout": "10s"` (1.12 rejects DNS detours to
empty direct outbounds). The generated config now matches the Hiddify pattern:
no binds, `auto_detect_interface`, `default_domain_resolver`, `hijack-dns`
action, sniff, DoH through the tunnel. Verified: parses and runs on the VPS
(process alive, TUN up, zero FATAL), tests pass, Windows build + lint green.

### Follow-up (2026-07-31) — macOS link failure: missing `UniformTypeIdentifiers` framework

Linux/Windows builds then passed, but both macOS matrix jobs failed at the
final link with:

```
Undefined symbols for architecture arm64:
  "_OBJC_CLASS_$_UTType"
ld: symbol(s) not found for architecture arm64
```

Wails v2.9.1's darwin code (`WailsContext.m`) uses `UTType` but only links
`-framework Foundation -framework Cocoa -framework WebKit`. On older SDKs,
UniformTypeIdentifiers was re-exported transitively and the symbol resolved
implicitly; **Xcode 26 / macOS 26 SDK removed that implicit linkage**, so the
link broke. (Checked: v2.13.0 does not add the framework either — and it
requires Go 1.25, so bumping Wails was not an option on Go 1.22.)

**Fix:** added `v5/client/darwin_link.go` — a `//go:build darwin` cgo shim in
the main package with `#cgo LDFLAGS: -framework UniformTypeIdentifiers`. cgo
flags from the main package are passed to the final link, so this fixes both
macOS architectures and both `wails build` and manual `go build` paths. No
workflow change needed.

---

### Follow-up 9 — REMOVED udp_over_tcp: sing-box's UDP-over-TCP is proprietary (2026-08-01)

**Symptom:** client says Connected and DNS resolves, but pages hang ("nothing
loads properly"). The log shows successful DNS exchanges and DoH through the
tunnel (`outbound/shadowsocks[proxy]: outbound connection to 1.1.1.1:443`),
but every UDP/QUIC flow dies:

```
outbound/shadowsocks[proxy]: outbound UoT packet connection to 142.251.12.84:443
connection: packet download closed: read tcp ...->114.23.136.59:8445:
  wsarecv: An existing connection was forcibly closed by the remote host.
```

(Repeated ~300ms apart — Chrome/Edge QUIC retry storm. Hiddify mobile VPN
mode and Hiddify desktop System Proxy work with the same server; Hiddify
desktop VPN mode fails the same way.)

**Root cause:** `generateConfig` emitted
`"udp_over_tcp": { "enabled": true, "version": 2 }` for tiers with
`udp_relay` (Strike). But sing-box's UDP-over-TCP is a **proprietary SagerNet
protocol** — magic domains `sp.udp-over-tcp.arpa` (v1) / `sp.v2.udp-over-tcp.arpa`
(v2) — **not** the Shadowsocks SIP003 UDP-over-TCP. The deployed server is
shadowsocks-rust, which does not implement it and RSTs every UoT connection.
The TCP data path was fine all along; the failure was dead UDP plus browser
QUIC retry loops. (This was also observed earlier the same day in
`seed-live.py`'s notes: "every UoT conn RST after ~300ms".)

**Fix:**
- `v5/client/internal/manager/process.go` — `udp_over_tcp` is no longer
  emitted for any tier. UDP goes **raw** (standard ss UDP; Strike server runs
  `tcp_and_udp`), works wherever the network allows UDP, and browsers fall
  back to TCP where it doesn't (N4L school WiFi). `UDPOverTCPConfig` type and
  `Outbound.UDPOverTCP` field removed; `Config.UDPRelay` is now informational
  only.
- `v5/server/scripts/seed-pb.py`, `fix-tier-configs.py` — strike seeds
  `udp_relay: false` (it historically flipped on client UoT). `seed-live.py`
  already had this.
- **Live DB (pending):** tier_configs → strike must have `udp_relay: false`.
  Clients self-heal via the heartbeat server_config refresh within one beat
  (no rebuild needed for existing installs).
- Revisit UoT only if a **sing-box server** is deployed for a tier.

---

### Follow-up 10 — Pre-flight guard: refuse a second sing-box on the same TUN (2026-08-01)

**Symptom:** after the UoT fix, the client still misbehaved ("a variety of
issues"). The log showed TWO engine startups in one session:

```
23:55:42 sing-box log level: debug          ← leftover sing-box.exe (orphaned
23:55:45 sing-box log level: debug          ← fresh Connect spawn
         inbound/tun[tun-in]: started at locus0   ← ×2 — two instances, one TUN
```

**Root cause:** orphaned `sing-box.exe` processes from earlier broken builds
(and/or other VPN apps like Hiddify) survive on Windows. A fresh Connect then
spawns a second engine; both instances share the `locus0` wintun adapter —
packets are delivered to both, routes fight, and failures look random.

**Fix (`v5/client/internal/manager/`):**
- New `foreignSingBoxRunning()` (Windows: `tasklist /FI "IMAGENAME eq sing-box.exe"`;
  Unix: `pgrep -x sing-box`) — true when an untracked sing-box process exists.
- `Start()` now refuses with a clear message ("another sing-box process is
  already running — close it (Task Manager) and retry") instead of stacking a
  second engine. Only runs on user-initiated Connect (never in the health-loop
  restart path, and never when our own process is tracked and alive).
- Note: this also blocks Connect while Hiddify's sing-box core is running —
  intentional (two TUN VPNs at once is invalid), the message says so.

**One-time user cleanup (still required for machines with orphaned engines):**
1. Task Manager → end ALL `sing-box.exe`.
2. `netsh interface show interface` → if `locus0` exists:
   `netsh interface delete interface locus0` (admin).
3. Reboot (clears stale routes/WFP filters).

---

## VPS TESTING — ISSUES FOUND & FIXED (2026-07-26)

These were discovered during real deployment to a Voyager VPS (Ubuntu 22.04)
and are now fixed in `v5/server/`.

### S1. Caddy Systemd Service Missing

| Severity | 🔴 Caddy wouldn't start |
|----------|------------------------|
| **File:** | `v5/server/modules/05-caddy.sh` |
| **Issue:** | The module downloaded a custom Caddy binary with rate_limit support, but never created a systemd service file. `systemctl restart caddy` failed with "Unit not found." |
| **Fix:** | Added `create_systemd_service()` that: creates `caddy` user if missing, creates `/var/log/caddy` and `/var/www/html`, writes a proper systemd unit file, and grants `cap_net_bind_service+ep` to the binary so it can bind :80/:443 as non-root. |

### S2. Caddy TLS Certificate Provisioning Failed

| Severity | 🔴 HTTPS broken |
|----------|-----------------|
| **File:** | `v5/server/modules/05-caddy.sh` |
| **Issue:** | Caddy ran as user `caddy` which had no home directory. Caddy tried to write TLS storage to `/home/caddy/.config/caddy/` but got "permission denied." |
| **Fix:** | Removed `ProtectSystem=full` from the service file. Created `/var/lib/caddy` for Caddy's runtime data. Added `Environment=XDG_CONFIG_HOME=/var/lib/caddy` and `Environment=XDG_DATA_HOME=/var/lib/caddy` so Caddy stores TLS certs in a writable location. |

### S3. PocketBase Service Failed with NAMESPACE Error

| Severity | 🔴 PocketBase wouldn't start |
|----------|------------------------------|
| **File:** | `v5/server/modules/06-pocketbase.sh` |
| **Issue:** | The service had `ProtectHome=true`, `ProtectSystem=full`, and `PrivateTmp=true`. On this VPS kernel (5.15.0-161-generic), these caused `status=226/NAMESPACE` errors — the kernel rejected namespace operations. |
| **Fix:** | Removed all systemd security hardening directives (`ProtectHome`, `ProtectSystem`, `ReadWritePaths`). The service now uses only `NoNewPrivileges=true` and `PrivateTmp=true`. |

### S4. Admin Token Extraction Failed (Shell Escaping)

| Severity | 🟡 Admin created but collections not seeded |
|----------|---------------------------------------------|
| **File:** | `v5/server/modules/06-pocketbase.sh` |
| **Issue:** | The admin creation API response was piped through `echo "$RESP" | python3 -c "..."` which broke when the JWT contained special characters (dots, dashes). The token came back empty, so the script thought "Admin created but could not extract token." |
| **Fix:** | Wrote the API response to a temp file before parsing. Also added a fallback auth flow (`/api/admins/auth-with-password`) if token extraction fails. Then refactored the entire bootstrap into `scripts/seed-pb.py` which uses Python's `json.dump()` to write temp files for all API calls, avoiding shell escaping entirely. |

### S5. Collection Creation Failed with Bash Heredocs

| Severity | 🟡 Collections not created |
|----------|---------------------------|
| **File:** | `v5/server/modules/06-pocketbase.sh` |
| **Issue:** | The shell heredocs (`cat > file << 'JSON'`) for the collection schema JSON were inconsistently parsed when passed through SSH. The JSON contained nested quotes and special characters that bash mangled. |
| **Fix:** | Moved all collection creation logic into `scripts/seed-pb.py`. Python writes the JSON payload to a temp file using `json.dump()`, then passes `@file` to curl. This is bulletproof — no shell escaping issues. |

### S6. `generate_codes.sh` Bash Bug (!$double)

| Severity | 🔴 Code generation produced errors |
|----------|-----------------------------------|
| **File:** | `v5/scripts/generate_codes.sh` |
| **Issue:** | The Luhn-mod-N checksum used `double=!$double` to toggle a boolean. In bash, `!$double` triggers history expansion (or fails with `!false: command not found` when history expansion is off). |
| **Fix:** | Replaced with `if $double; then double=false; else double=true; fi`. |

### S7. PocketBase 0.22 JS Hook Compatibility

| Severity | 🔴 Activation endpoint returns 400 |
|----------|-----------------------------------|
| **File:** | `v5/server/pb_hooks/activation.pb.js`, `heartbeat.pb.js`, `admin_unbind.pb.js` |
| **Issue:** | PocketBase 0.22.21 changed the JavaScript VM API. The hooks were originally written for the 0.21 API (`$app.dao()`, `$apis.requestInfo()`). On 0.22.21, `$app.dao()` still works via compatibility shim, but `$app.dao().db().exec()` and `$app.dao().findRecordsByFilter()` with `{:param}` syntax had degraded behavior. |
| **Fix:** | **✅ Fixed 2026-07-26.** Hooks updated to use working API patterns for PocketBase 0.22.21: |
| **Changes made:** | • `$apis.requestInfo(c).data` → `$apis.requestInfo(e).data` (compat shim still works)<br>• `$app.dao().db().exec()` → `$app.dao().db().newQuery().execute()`<br>• `$app.dao().findRecordsByFilter()` with `{:param}` → `$app.dao().findFirstRecordByData()` for single-record lookups<br>• `$app.dao().findCollectionByNameOrId()` + `new Record()` + `$app.dao().saveRecord()` for creating records<br>• `e.remoteIP` → `e.request().remoteAddr.split(":")[0]` (remoteIP not exposed in 0.22.21 router events)<br>• `const`/`let` → `var` (goja compatibility)<br>• Helper functions must be defined inside the `routerAdd` callback (goja doesn't hoist declarations into callback scope)<br>• Rate limiting filter: inline string with sanitization instead of `{:param}` syntax (filter parser changed)<br>• `sqlite_pragmas.pb.js` removed — `on()` hook API removed in 0.22, WAL mode set automatically by PocketBase |

### S8. Smoke Test IP Detection Off by One

| Severity | 🟡 False positive warning |
|----------|---------------------------|
| **File:** | `v5/server/scripts/smoke-test.sh` |
| **Issue:** | The smoke test detects the VPS IP using `ip route show default | awk '{print $3}'` which returns the **gateway** IP (e.g., `114.23.136.1`), not the interface IP (`114.23.136.47`). The DNS match check then fails even when DNS is correct. |
| **Fix:** | Changed to `ip -4 addr show | grep -oP 'inet \K[0-9.]+' | grep -v '^127\\.' | head -1` to get the actual interface IP. |

---

### S9. Secrets Decryption via Process Substitution Fails Silently

| Severity | 🔴 All SS passwords auto-generated instead of using stable secrets |
|----------|-------------------------------------------------------------------|
| **File:** | `v5/server/setup.sh`, `v5/server/restore.sh` |
| **Issue:** | Both `setup.sh` and `restore.sh` used `source <(age -d ...)` (bash process substitution) to load decrypted secrets. In non-interactive SSH sessions — specifically when running via `ssh root@host "/root/server/setup.sh"` — the process substitution would appear to succeed (exit 0, "✓ Secrets decrypted" printed) but **the variables would not actually be loaded into the current shell**. This caused all downstream modules to fall through to auto-generated passwords instead of using the stable credentials from `secrets.env.age`. The symptom: `DOMAIN` was empty despite "Secrets decrypted" having printed, causing a hard fail at the DOMAIN check. |
| **Root cause:** | `source <(command)` is a bash extension that works unreliably in certain SSH environments. The process substitution forks a subprocess, and `source` may read from an already-closed pipe under some shell configurations. Ubuntu's `/bin/sh` is dash, which does not support process substitution at all — but even under bash the behavior was inconsistent. |
| **Fix:** | Replaced `source <(age -d ...)` in both files with a two-step temp-file approach: `age -d ... > /tmp/.secrets-$$.env` followed by `source /tmp/.secrets-$$.env`. This is POSIX-compatible, works in all shell environments, and is easy to debug (the temp file can be inspected). Also added an `age` binary check + auto-install via `apt-get` before decryption, so a fresh VPS without `age` pre-installed will still work. |
