# Fixes

```
audience:    builder
status:      live
authoritative-for: the dated defect log — what broke, what fixed it, and the rule it produced
verified-against: docs/state.toml (version/release facts)
```

Every defect below is a dated entry, newest first, naming the release it went out
in. **Treat entries as historical** — later entries sometimes correct earlier ones.
For what is *still* broken, read [`STILL-OPEN.md`](STILL-OPEN.md); for the
commercial gaps, [`../business/17-not-built.md`](../business/17-not-built.md).

> **The server-side halves do not reach the live host until `setup.sh` re-runs.**
> `setup.sh` deploys from `/root/server/` on the VPS, not from this repo, so every
> hook and script change here is inert until that re-run. A heartbeat that lacks
> `enforcement_version` in its response is proof the hook is still stale.

> **This log is a summary index, not the transcript.** Each entry keeps what broke,
> what fixed it, and the rule it produced. The full investigation behind any entry
> is in git: `git log -S'<heading phrase>' -- docs/reference/FIXES.md` finds the
> commit that introduced it.

---

## THE ENGINE PIN WAS HALF-WIRED, SO THE ENGINE DRIFTED AND EVERY BUILD STOPPED (2026-10-29, release 3.3.4)

**This is the real reason `v3.3.4` first produced nothing.** The geo-database
404 above was the first failure; once repinned, the builds still failed, at a
different file, for an unrelated reason.

- **What broke.** Every `build` job died with:

  ```
  "verge-mihomo" does not match its pin:
    got 07e79c1eb24cab62, lock pins 90529d00b84df144
  ```

  The pin was **right** and the download was **right**: they described different
  engines. `sidecar.lock.json` recorded `"engine_versions": {"mihomo": null}`, and
  `getLatestReleaseVersion()` reads a null pin as "no pin — ask the network":

  ```js
  const pinned = lockedEngineVersion('mihomo')
  if (pinned) { META_VERSION = pinned; return }   // taken for alpha…
  // …falls through to releases/latest for the stable engine
  ```

  So the *hash* was pinned to the bytes of **v1.19.32** while the *version* was
  re-resolved from `latest` on every run. When upstream published v1.19.33 the
  two disagreed and the build refused to proceed. The alpha core was pinned
  (`alpha-6780eee`) the whole time; only the stable one was left null.
- **Established by direct download, not by inference.** Fetching
  `mihomo-linux-amd64-v2-v1.19.32.gz` hashes to `90529d00b84df144…` — exactly the
  recorded pin — while `v1.19.33.gz` hashes to `07e79c1e…`. The pinned binary
  reports `v1.19.32` from its own `-v`. Three independent facts agree.
- **Why it looked fine for months.** Any machine with a warm
  `node_modules/.verge/` cache short-circuits before the pin is consulted, so
  local builds passed while every cold CI runner failed. That is the same
  "a local green run is evidence about your working tree" trap `RELEASING.md`
  §3.3.1 already records.
- **The fix.** `engine_versions.mihomo` is now `"v1.19.32"`, so the lock decides
  which engine to fetch and the network only decides where to get it — the
  behaviour the code's own comment already claimed.
- **Observed failing, then passing.** With the lock temporarily pinned to
  `v1.19.33` and the sidecar deleted (the CI condition), prebuild reproduced the
  CI error exactly and left the previous binary in place; with the correct pin it
  downloads and verifies cleanly. A guard that cannot fail would have shown the
  same output for both.
- **Rule kept.** *A pin has to pin the thing that is chosen, not just the bytes
  that arrive.* The repository already knew this for one engine and not the
  other, which is why the defect survived: half a pin reads like a pin.

## THE PINNED GEO DATABASES 404'D, AND THE BUILD STOPPED RATHER THAN SHIP NEW DATA (2026-10-29, release 3.3.4)

- **What broke.** Both `Build (linux)` and `Build (windows)` failed at "Fetch
  sidecars and geo data". Not a code defect: all four assets pinned in
  `server/data/sidecar.lock.json` returned **404**. MetaCubeX republished the
  rolling `latest` release on 2026-10-28 (release name `Release 2026-10-28 09:30`),
  and a republish mints new asset IDs and deletes the old ones.
- **The pin worked, in the sense that matters.** It refused to build instead of
  silently downloading whatever `latest` pointed at. That is the property the
  lock was built for, and this is the first time it earned it.
- **What the pin could not tell us.** Whether the republish was cosmetic or a
  real data change. Answering it required fetching the new bytes and hashing
  them: three of four were re-uploaded **byte-identical** (sha256 still equal to
  the old pins), and only `geosite.dat` had changed — 4,307,247 → 4,353,539 bytes,
  a genuine upstream routing-data update.
- **The fix.** All four asset IDs repinned and `resources_release` advanced to
  `Release 2026-10-28 09:30`. Three sha256 values are unchanged, so the diff
  isolates the one file whose content moved. Re-downloaded through
  `prebuild.mjs`, which reported "on disk does not match its pin — re-downloading"
  and then verified the new bytes; `sidecar-lock.py --check` is green.
- **Rule kept.** `STILL-OPEN.md` had already named the residual risk two days
  earlier — "an asset ID is only as durable as the repository that hosts it" —
  and it was realised immediately. A predicted risk that is written down is
  cheaper than one that is discovered, but only if it also says what to do. It
  now says: expect to repin whenever `resources_release` moves, and mirroring
  these four files into this project's own release assets is a defect to fix,
  not an optional hardening.

## CI COULD NEVER PASS, BECAUSE THE GUARD PROOF NEEDED A FILE CI NEVER FETCHED (2026-10-29, release 3.3.4)

**Found by cutting the release, not by reading the workflow.** `verify-live.sh`
claim **A4** ("a release is published") was red, and the reason is the whole
entry: the run for `v3.3.3` failed, so no release was ever created for that tag
and the hub went on serving 3.3.2.

- **What broke.** `test-contract-guard.sh` proves the §39 build-input pin can
  FAIL by substituting a mihomo core that is still a valid gzip and asserting
  `sidecar-lock.py --check` rejects it. The test needs that sidecar on disk. The
  `Verify` job never fetched one — the sidecars are a gitignored download that
  only the four `build` jobs performed — so every run printed

  ```
  MISSED a substituted engine (§39) — the sidecar is not present, cannot test
  ```

  and the script exits 1 on any `MISSED`. **The release path could not produce a
  green run at all**, on any commit, and nothing said so: the last *successful*
  run predates this test.
- **Reproduced before fixing.** Hiding the sidecar locally reproduced the exact
  `MISSED` line and exit 1; restoring it (lock check green) made the full proof
  pass, 10 assertions caught. The fix is therefore aimed at the observed failure,
  not at a guess about it.
- **The fix.** `Verify` now caches and fetches the build inputs before the proof,
  and the two steps are ordered **after `pnpm install`** — `prebuild.mjs` imports
  `adm-zip`, `glob`, `https-proxy-agent` and `tar` at module scope, so placed
  earlier it fails with `Cannot find module`, which `continue-on-error` would
  then have rendered identical to the missing-sidecar case this step exists to
  repair.
- **Rule kept.** *A guard that cannot run is not a green guard — and a test that
  skips when its input is absent is a check that cannot fail.* The §39 branch
  already reported `MISSED` rather than skipping, which is why the defect was
  visible at all; the missing half was giving it the input.

**A second CI gap, found the same way: the suite CI ran was not the suite.**

- **What broke.** `sysopt.rs` quoted a log line a student actually saw, indented
  four spaces:

  ```
  ///     Failed to compensate proxy apply failure (… lock preferences) with direct mode.
  ```

  Rustdoc reads a four-space-indented block as Rust, tried to compile it as an
  expression, and failed on the `…` character. The crate's doc-tests therefore
  did not compile **at all**.
- **Why nothing caught it.** CI runs `cargo test --all-targets`, which excludes
  doc-tests. That is cargo's documented behaviour, not a mistake in the line —
  but it meant a broken doc-test could sit in a green tree indefinitely, and this
  one did. The 2026-10-26 entry above claimed the suite "Verified"; what was
  actually run was `--lib` and `--all-targets`, and neither reaches a doc-test.
  The claim was true about what it ran and misleading about what it covered.
- **The fix.** The quote is now a fenced `text` block, so rustdoc renders it and
  does not compile it. `cargo test --doc` is green.
- **The guard.** `Doc-tests` is a new linux-only step in `build (linux)`, next to
  clippy and `cargo fmt --check`. Without it the fix is a one-off: the next
  indented quote in any doc comment reintroduces the same failure silently.
- **Rule kept.** *A suite is only as complete as the command that runs it.*
  `--all-targets` reads like "everything"; it excludes doc-tests, and the gap
  between a flag's name and its behaviour is where this defect lived.

## 3.3.3 — THE WINDOWS TLS FIX SHIPS, AND THE BUILD BECOMES REPRODUCIBLE (2026-10-26, release)

- **What went out.** The `locus::hub` TLS fix from 2026-10-18, which sat in the
  tree labelled *unreleased* for eight days and three commits. The student it was
  written for was still affected; the fix was correct and never shipped. That gap
  is the one this entry exists to name: work is not done when it is in `main`.
- **Proven before tagging, not after.** Both live-hub tests pass on 2026-10-26 —
  the bundled-roots fallback verifies `networkingguides.jadedns.uk/api/health`,
  and the platform-store path does too, so the probe is not proving a tautology.
  A **Windows** machine was still not available: that this fixes *the reporter's*
  specific machine remains structural reasoning, not an observation.
- **Build inputs are now data.** The mihomo engine, the service bundle and the
  four geo databases are pinned by sha256 in `server/data/sidecar.lock.json`;
  `prebuild.mjs` takes the version *from the lock* rather than from
  `releases/latest`. The four databases are pinned by immutable release-asset ID,
  because `meta-rules-dat` publishes only a moving `latest` tag — observed live
  when `Country.mmdb` changed between two runs minutes apart.
- **The alpha engine is vendored.** `tauri.conf.json` requires
  `sidecar/verge-mihomo-alpha` in `externalBin`, and upstream deletes old alphas,
  so its URL is not stable. It is committed and hash-verified. Before this the
  client would not compile at all with a cold cache.
- **Rule kept.** A build that cannot be reproduced from a cold cache is a build
  that has never been reproduced. Two of the three inputs here were only
  discoverable by clearing the cache and trying, which is why the failure was
  invisible for as long as it was.

## THE GUARDS THEMSELVES HAD FOUR DEFECTS, AND ONE OF THEM WAS BLIND (2026-10-26, unreleased)

A run of the full check suite from a cold tree found no defect in the product and
four in the things that check the product. Recorded together because they share a
shape: **a check that reads correct and does not do what its comment says.**

- **`cargo clippy -- -D warnings` failed on two lints, so the gate could not
  pass.** `logging_error!` expanded to a bare `if let` statement with a trailing
  semicolon, which is rejected in expression position
  (`semicolon_in_expressions_from_non_local_macros`) — the macro's only use there
  was a `match` arm. And `AtomicU64::fetch_update` is deprecated in favour of
  `try_update`. The macro now expands to a block, which is correct in both
  positions; the atomic uses a CAS loop, because `try_update` is newer than the
  crate's `rust-version = "1.95"` floor and the loop compiles on both.
  *Rule:* a macro used as an expression must expand to a block. A `-D warnings`
  gate is only a gate if it has been run against the toolchain CI actually uses.
- **`test-contract-guard.sh` judged its own patch step by the wrong status.**
  Both helpers ran `python3 … <<PY` and then tested `[ $? -ne 0 ]` — but `$?` at
  that point was the heredoc **assignment**, not the patch, so a patch that
  silently failed to apply was reported as caught. It now captures the status
  immediately.
- **The same script's restore check could pass on files it never checked.** The
  fingerprint was `grep "  $f\$"`, which treated the path as a regex (dots
  match anything) and reported OK for an unmatched file; there was also a dead
  `want=` line whose value was computed and never used. It now snapshots the
  files to a temp directory *before* touching them and compares with `cmp`, so a
  file that was not snapshotted fails loudly instead of passing. **Observed
  failing**: skipping one restore produced three `BAD` lines, where the old code
  produced one.
- **`check-consistency.sh` §7 declared `TRIAGED_BASIS` — the documented,
  per-path list of files whose older version is correct as written — and never
  applied it.** The filter was `grep -vE "$TRIAGED_BASIS"`, which is an Extended
  regex, so `\.` matched any character and the anchors behaved as alternations of
  substrings. It did not crash; it silently did not exclude what the comment above
  it says it excludes. Replaced with an exact-path list and `grep -vF`, plus the
  container-directory prefixes as a regex. The report is unchanged on this tree,
  which is the point: the list is the mechanism, and it was inert.
- **Nothing checked `cargo fmt`.** `client/rustfmt.toml` has been the style since
  the fork and no workflow ran it, so 82 files were unformatted. All were
  formatted here and a `cargo fmt --check` step was added next to clippy.
  *Rule:* formatting is a function of the toolchain and must be bound to the same
  toolchain, or it drifts into a commit nobody can review.
- **`prebuild` did not restore the executable bit on a sidecar that already
  matched its pin.** The chmod lived at the bottom of the download path only, so
  every early return — "matches its pin, skipping download", "already exists
  (UNPINNED)", and the vendored path, which chmods *after* its return — left the
  file as it was. `verge-mihomo` was found with the pinned sha256 to the byte and
  mode 0644. The bytes were right, so the lock check passed; the file could not
  execute, so all five engine integration tests panicked with
  `PermissionDenied` and the two engine suites read as failures in a tree whose
  content was correct. Mode is not content, so the hash pin cannot cover it; it
  is now asserted on every path, by `ensureExecutable()`.
- **The engine tests' skip check tested `is_file()` and then `spawn`ed.** A
  present-but-0644 sidecar passed the check and panicked inside `start_engine`,
  blaming the test. Both suites now treat "not executable" as "not fetched",
  which prints the `SKIP` naming `prebuild.mjs` — observed skipping after
  `chmod 644`, and observed passing (5/5, 10.2s) after the bit was restored.
  *Rule:* a precondition a test relies on must be checked in the form the failure
  takes. `spawn` needs an executable, not a file.
- **No guard existed for the goja builtin trap.** `admin_console.pb.js` refused
  every release because it called `atob`, which PocketBase's goja does not
  define; the crash was caught and turned into `false` for every input. The fix
  was already committed but nothing prevented it recurring. §6(f) now fails on
  `atob`/`btoa`/`TextDecoder`/`TextEncoder`/`Buffer.*` in any hook, ignoring
  comments (the file explains the trap in prose at the call site). Observed
  failing against a reintroduced `atob(`. `STILL-OPEN.md` was corrected in the
  same change — it still described the fix as uncommitted.



- **What broke.** A 3.3.1 client failed activation with `invalid peer certificate:
  Expired`; the certificate had not expired. The response to the N4L block had never
  been finished — `hub_alias` was opt-in and empty, so every client was still
  pinned to the blocked hostname.
- **The fix.** The canonical hub moved; the legacy name is served, not redirected;
  the console, the heartbeat and the seeder agree on the new host. §32 turns the
  two-hostname rule into a guard.
- **Rule kept.** A defect whose mechanism cannot be observed from either end is
  recorded as *not established*, not as a theory that was convenient.

## ACTIVATION FAILED ON WINDOWS WITH "INVALID PEER CERTIFICATE: EXPIRED", ON EVERY ATTEMPT (2026-10-18, release 3.3.3)

- **What broke.** A Windows student could not activate, on either plan, on every
  attempt: `client error (Connect): invalid peer certificate: Expired` against a
  hub certificate issued that morning and valid until 2027-01-16. The log said
  `failed to verify TLS certificate: invalid peer certificate: Expired` four
  times — once per verification attempt.
- **What it was not.** The hub. Measured: `/api/health` returns 200, the leaf is
  valid, the served chain is complete (leaf → `YE1` → `Root YE` → `ISRG Root X2`
  → cross-signed by `ISRG Root X1`), and both TLS 1.2 and 1.3 verify against
  OpenSSL and Python on a machine with a current trust store.
- **The mechanism.** `locus/activation.rs` built its own `reqwest::Client`, and a
  bare builder uses `rustls-platform-verifier`, which on Windows reads **only the
  Windows certificate store**. The chain's top anchor is `ISRG Root X2`, an ECDSA
  root that reaches Windows machines solely through root-certificate update
  servicing. `rustls` does not chase the certificate's AIA extension, so a machine
  without that root has no path to an anchor — and `webpki` reports that as
  `InvalidCertificate(Expired)`. The message accuses the hub's certificate of
  something that is false, which is why it read as a hub outage.
- **The fix.** One shared client in `locus::hub`, used by activation, heartbeat
  and the update download: try the platform store, and on a **certificate** error
  only, retry with the compiled-in `webpki-roots` store, which ships with the
  binary and needs no OS update. The failure is logged with the store that
  refused, so the next support log names the cause instead of borrowing rustls's
  word for "no path to an anchor". §36 keeps a hand-built client out of the
  hub-facing modules.
- **The fix's own near-miss.** The bundled-roots client first used
  `rustls::ClientConfig::builder()`, which **panics** unless exactly one crypto
  provider feature is enabled — this crate enables only `ring`. A live test
  caught it; without network, `both_clients_build_without_panicking` now catches
  it. The path that exists to rescue a broken machine would have crashed the
  screen it was meant to fix.
- **Proven, not assumed.** `the_bundled_roots_verify_the_live_hub` fetches the
  real hub through the fallback and passes; the platform path does too on a
  machine with a current store. **Unverified:** that this is the specific
  reporter's mechanism — no Windows machine was available, so the chain of
  inference is structural. The observation that would settle it is the absence of
  `ISRG Root X2` in that machine's root store.
- **Rule kept.** A policy copied into several places drifts, and the copy that
  drifts is the one nobody re-read. Three hub clients had diverged on the only
  setting that matters; `utils::network` had the fallback and none of the others
  did.

## 3.3.1 SHIPPED, AND THE THREE THINGS THAT STOPPED IT (2026-10-17, release)

- **What broke.** No defect in the shipped build; three defects in the road to it
  (a cache that blocked the release job among them).
- **Rule kept.** CI must be *able* to fail loudly before a release means anything.

## A REPO-WIDE SCAN: A RETRY THAT NEVER RETRIED, AND THREE CROSS-LANGUAGE GAPS (2026-10)

- **What broke.** One real defect (a retry wrapper that could never retry) plus
  three checks that could not fail.
- **Rule kept.** Capability must be called, not awaited; an await on a non-promise
  discards the return value.

## THE FREE PLAN'S VERIFIER COULD NOT SEE THE FREE PLAN (2026-10, hub scripts)

- **What broke.** `verify-ss2022.sh` checked one retired UoT listener against a
  config the per-plan split had deleted, and had no concept of the free plan's
  listener — a dead free tier passed every check.
- **The fix.** The verifier now names each per-plan unit and pairs `eco.json` with
  `sing-box-uot-eco`; §31 asserts that pairing.
- **Rule kept.** A check that cannot fail, guarding the one pairing a student
  would notice.

## THE HUB HOSTNAME WAS UNREACHABLE ON A SCHOOL NETWORK, AND NOTHING COULD REPOINT IT (2026-10, hub + client)

- **What broke.** `HUB_URL` was compiled in, and the same hostname serves
  activation, heartbeat **and** `/updates/*` — so a network that refuses that name
  removes the ability to activate *and* the ability to download the fix.
- **The fix.** A hub alias (`HUB_ALIAS_DOMAIN`, 308 to the canonical domain) plus
  a client override (`contract::resolve_hub_url`) delivered through the heartbeat,
  the only channel that reaches an installed device without a release.
- **Rule kept.** A failure that prevents escaping the failure is the worst
  topology; a second delivery path is not redundancy, it is escape.

## A CONCURRENCY GUARD WAS REPORTED AS A REFUSAL ON A FINE MACHINE (2026-10, client)

- **What broke.** "Configuration validation is already running" was surfaced as a
  refusal, abandoning the connect it was in the middle of.
- **The fix.** `apply_tier` distinguishes "no verdict" from "rejected".
- **Rule kept.** A lock held is not a verdict on the thing it locks.

## THE macOS BUNDLE CARRIED AN ENTITLEMENT IT COULD NOT HONOUR (2026-10, client packaging)

- **What broke.** `com.apple.security.inherit` was true on a bundle without
  `app-sandbox`, and `application-groups` named a team the bundle does not belong to.
- **The fix.** The plist claims only entitlements the bundle owns; §30 guards it.
- **Rule kept.** An entitlement is a claim the OS enforces; a false one is a
  signing-time lie that surfaces at runtime.

## THE CREDENTIAL RECOVERY LEFT FIVE CLAIMS FALSE, AND ONE GUARD COULD NOT FAIL (2026-10-05, docs + a guard)

- **What broke.** A recovery pass left five world claims asserting state that no
  longer held, and a guard that reported OK while checking nothing.
- **Rule kept.** A guard that crashes or short-circuits reads exactly like a guard
  that passed. Prove it fails before keeping it.

## THE DOC SET WAS GREEN AND FRAGMENTED: TWO INDEXES, LYING TITLES, RETIRED MATERIAL (2026-10-05, docs)

- **What broke.** Two competing indexes, a `status: live` document announcing its
  own retirement, and retired material living in live folders — all green.
- **The fix.** One index ([`../README.md`](../README.md)), status as structure, and
  §28 to hold both.
- **Rule kept.** Status belongs to structure, not to a banner a document writes
  about itself.

## §8 WAS SILENTLY BROKEN, AND THE STALE-PATH SCAN NEVER LOOKED AT CODE (2026-10-05, guards)

- **What broke.** §8 scanned only `.md` files, so two code comments pointed at a
  moved document for a year. Widening it broke it silently: joining two generators
  with `+` raised `TypeError` and §8 reported OK while checking nothing.
- **Rule kept.** When you widen a guard's scope, re-prove that it can fail. See
  [`../reference/DOC-AUDIT.md`](../reference/DOC-AUDIT.md).

## THE DOCS DESCRIBED REMOVED MECHANISMS AS LIVE, AND TWO PROSE COPIES OF A CAP WERE WRONG (2026-10-05, docs + guards)

- **What broke.** Documents described removed mechanisms as current, and a guard
  *required* `README.md` to restate the tier caps — enforcing the duplicate it
  existed to prevent.
- **The fix.** The guard now asserts the negative: no cap string in `README.md`;
  the cap has one home backed by `state.toml`.

## A MACOS STUDENT WAS OFFERED NOTHING, AND NOTHING SAID WHY (2026-10-04, v3.2.27)

- **What broke.** The update path had no macOS payload, and the failure was silent.
- **Rule kept.** A missing remedy must say it is missing — see
  [`../operate/RECOVER-MACOS-UPDATE.md`](../operate/RECOVER-MACOS-UPDATE.md).

## THE FREE TIER'S ADVICE FIELDS COULD BREAK THE WHOLE HEARTBEAT, AND FOUR OTHER PRE-EMPTIVE FIXES (2026-10-04, v3.2.26)

- **What broke.** A malformed advice field could fail the entire heartbeat rather
  than the field, taking the paid path down with the free one.
- **Rule kept.** A degradation path must not be able to fail the path that pays.

## THE FREE TIER HAD NO ENFORCEMENT, AND THE BUSINESS PLAN READ AS IF IT DID (2026-10-04)

- **What broke.** The plan described enforcement that did not exist in the code.
- **Rule kept.** [`../business/17-not-built.md`](../business/17-not-built.md) is the
  anti-drift guard: anything not built is listed there.

## macOS SHIPPED A TERMINAL-ONLY REMEDY: A README THE STUDENT COULD NOT ACT ON (2026-10-04)

- **What broke.** The remedy for a macOS failure was a text file telling the
  student to run commands.
- **Rule kept.** A remedy a customer cannot act on is not a remedy.

## A NEW LOOKUP STATUS NAME LEFT REINSTALLING STUDENTS UNABLE TO RE-ENTER THEIR CODE (2026-10-03)

- **What broke.** A hub-side status name the client did not know left reinstalled
  students unable to re-enter their code.
- **Rule kept.** Every new status name is a wire contract: both sides must learn it
  in the same change, and the client's unknown-status default must be permissive.

## A CODE WAS BOUND TO A RE-DERIVED HARDWARE HASH, SO AN UPDATE COULD TELL A STUDENT THEIR OWN CODE BELONGED TO SOMEONE ELSE (2026-10-02)

- **What broke.** The `boundFp !== incomingFp` guard fired correctly — the device
  presented a different value, because a reinstall could not re-derive it.
- **The fix.** Device binding was **removed**, not repaired; the durable identity
  work that followed became [`client/docs/IDENTITY-MIGRATION.md`](../../client/docs/IDENTITY-MIGRATION.md).
- **Rule kept.** Never make a student's entitlement depend on a value you can only
  compute once.

## THE "CONNECTING" BUG — THE PROBE ASKED THE GROUP, WHICH ANSWERS FOR A MEMBER (2026-10-02)

- **What broke.** The readiness probe asked the *proxy group* for a delay; a group
  reports the delay of whichever member it selected, so a working tunnel could
  read as not ready, and vice versa.
- **Rule kept.** Probe the thing you are claiming about, not a handle that
  describes it.

## THE "CONNECTING" BUG CAME BACK A THIRD TIME, AND THE TEST PINNING IT WAS STALE (2026-10-02)

- **What broke.** The regression test still encoded the old probe's shape, so it
  passed against the defect it was written for.
- **Rule kept.** A test pinning a fix must be re-proved when the code it describes
  is rewritten.

## THE CODE WAS "ALREADY IN USE ON ANOTHER DEVICE" AFTER A REINSTALL, AND AUTO-SIGN-IN NEVER FIRED (2026-10-02)

- **What broke.** Reinstall produced the stale-binding refusal above, and the
  automatic sign-in path never ran.
- **Rule kept.** The recovery path for a refusal needs its own test; "the happy
  path is green" says nothing about the path a student actually takes.

## "THE UPDATE TO 3.2.18 IS NOT AN INSTALLER" — THE CLIENT REFUSED A GENUINE INSTALLER (2026-10-02)

- **What broke.** The client rejected a valid signed installer.
- **Rule kept.** An installer check must be loosened by evidence, and re-proved
  against a real signed artifact — not against a fixture.

## RECOVERY FROM A LOST SERVICE OWNER RESET THE WRONG PLATFORM'S PROXY (2026-10-01)

- **What broke.** A recovery routine keyed on the wrong platform and rescinded the
  wrong thing.
- **Rule kept.** A recovery is platform-specific; make the platform explicit at
  the call, never inferred.

## DEVICE RECOGNITION WORKED, AND THE STUDENT WAS LEFT ON THE CODE PROMPT FOR FIVE MINUTES (2026-10-01)

- **What broke.** Recognition succeeded but nothing advanced the UI.
- **Rule kept.** A successful background result must have a path to the surface.

## DEVICE IDENTITY WAS NOT DURABLE — CODES "STOPPED BEING RECOGNISED", AND A REINSTALL LOST THEM (2026-10-01)

- **What broke.** The identity lived in the app config — the file an uninstall
  deletes — so a reinstall made the device a stranger to its own code.
- **The fix.** A machine-scoped stored identity, reused verbatim, with an
  app-config fallback that **reports that it does not survive** a reinstall.
- **Rule kept.** Never let the UI over-promise persistence a storage tier cannot
  deliver. Detail: [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

## A GUARD THAT WAS CITED IN SIX DOCUMENTS DID NOT EXIST (2026-10-01)

- **What broke.** `activation_contract_test` was named in six live documents and
  two code comments as pinning the 403/409 refusal contract. It did not exist.
- **The fix.** `client/src-tauri/tests/activation_contract.rs` now reads the hook
  source and asserts the literal codes and messages the client classifies on, and
  a meta-guard makes the claim self-checking.
- **Rule kept.** A guard described in prose but absent is worse than no guard: it
  stops anyone from checking.

## THE WINDOWS INSTALLER WAS NEVER SIGNED: TWO DEFECTS IN ONE STEP (2026-10-01)

- **What broke.** The signing step silently did nothing; the installer shipped
  unsigned.
- **Rule kept.** A signing step must be shown to change its input, or it is a
  comment.

## NO WINDOWS CLIENT COULD UPDATE: THE HUB SERVED THE RAW BINARY, NOT THE INSTALLER (2026-10-01)

- **What broke.** The Tauri updater executes **any** PE as an installer, so the hub
  must publish the NSIS installer for Windows — it published the raw `.exe`.
- **Rule kept.** Two-sided contracts are asserted as *agreement*, not per side. The
  ordered recovery is [`../operate/RECOVER-WINDOWS-UPDATE.md`](../operate/RECOVER-WINDOWS-UPDATE.md).

## THE BUTTON SAT ON "CONNECTING" WHILE THE TUNNEL WAS CARRYING TRAFFIC (2026-10-01)

- **What broke.** The egress check judged the tunnel on **one** 3 s round trip and
  classified `delays.values().any(|&d| d > 0)` — accepting mihomo's failure and
  timeout sentinels as measurements, and flipping a working tunnel back to
  "connecting" on a single transient miss.
- **The fix.** A bounded retry (two attempts, 5 s each, 12 s shared deadline), one
  classifier (`delay_is_a_measurement`) mirroring the frontend's `classifyDelay`,
  a 1.5 s coalesced cache keyed to `core_started`/`core_stopped`, and a re-entrancy
  guard on the status poll.
- **Rule kept.** Two implementations of one rule are free to drift; the sentinel
  case must be one function, and every sentinel gets a test **shown to fail**.
- Detail: [`EGRESS-READINESS.md`](EGRESS-READINESS.md).

## THE APP RE-EXECUTED ITSELF INSTEAD OF UPDATING, AND SHOWED AN EDGE "FILE NOT FOUND" (2026-09-30)

- **What broke.** The app installed a copy of the *running* executable outside its
  install directory and relaunched that.
- **Rule kept.** An updater must install into the location the OS will launch from,
  and the check for that is `strings … | grep -c 'a/Locus/Locus'` returning `0`.

## THE WINDOWS INSTALLER ABORTED WITH AN EDGE "FILE NOT FOUND" DIALOG (2026-09-30)

- **What broke.** A raw Windows path was interpolated into build JSON, so the
  installer referenced a file that was never produced.
- **Rule kept.** Never interpolate a Windows path into a JSON build input (§13).

## THE UPDATE DOWNLOADED INTO RAM AND KILLED THE APP (2026-09-30)

- **What broke.** The updater buffered the whole payload in memory; on a large build
  it exhausted the process.
- **Rule kept.** The update path uses the streaming downloader (§12), and an update
  payload is never read into a buffer.

## THE GUARD THAT REFUSED EVERY RELEASE: goja HAS NO `atob` (2026-09-30)

- **What broke.** A guard written against a browser-style primitive did not exist in
  the PocketBase hook runtime and refused every release.
- **Rule kept.** Confirm the runtime's real capability before building on it.

## VOLATILE FACTS WERE ASSERTED AS PROPERTIES, AND NOTHING NOTICED (2026-09-29)

- **What broke.** A present-tense IP address and similar facts had gone stale
  silently.
- **The fix.** The claim classes and the dated register —
  [`../operate/CLAIMS.md`](../operate/CLAIMS.md); §10 fails a build that inlines one.

## UPDATES UNINSTALLABLE, AND "CONNECTED" WITH NO INTERNET (2026-09-29)

- **What broke.** A false `ready` was reported on a machine with no usable egress
  (the mirror image of the 2026-10-01 entry above).
- **Rule kept.** Both directions of the readiness judgement need a test: a false
  ready and a false not-ready fail the student differently.

## TERM-BASED ACCESS, RENEWAL, AND ONE CODE PER DEVICE (unreleased)

- **What broke.** Subscriptions had no term model; the code carried no
  materialised expiry.
- **The fix.** `term_days` on the code, materialised into `expires_at`, with
  renewal and an audit trail.
- **Not shipped as designed:** the one-code-per-device half was later removed. The
  design is [`../business/redesign/`](../business/redesign/00-README.md); the live
  model is [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

## THE UPDATE COULD NOT BE INSTALLED: THE SIGNATURE WAS UNDECODABLE (2026-09-29)

- **What broke.** The signature was not the base64 of the whole `.sig` file.
- **Rule kept.** The wire format of a signature is a contract; §1b asserts it.

## EXPIRY INVISIBLE TO THE UI, AND THE UPDATE TOGGLE WIRED TO THE WRONG SETTING (2026-09-28)

- **What broke.** An expired code looked identical to a working one, and the update
  toggle wrote a setting nothing read.
- **Rule kept.** A setting with no reader is dead code wearing a switch.

## MIGRATED THE CIPHER STACK TO SHADOWSOCKS 2022 (2026-09-28)

- **What changed.** SS2022 replaced the legacy cipher stack.
- **Rule kept.** The cipher is defined once (§4); ten copies with nothing binding
  them is how the previous one drifted.

## A SUSPENDED CODE SAID NOTHING, AND THE UPDATER WAS DEAD CODE (2026-09-28, third round)

- **What broke.** Suspension was indistinguishable from a network failure, and the
  updater path was never reached.
- **Rule kept.** Every refusal state needs a distinct, actionable message.

## THE BUTTON STILL LIED, THE EXPIRY STILL DID NOTHING, AND THE CHECK CAME OUT (2026-09-28, second round)

- **What broke.** The connect state was rendered from a value that did not reflect
  the tunnel.
- **Rule kept.** Render connection state from one backend field, never from a
  frontend guess.

## SUBSCRIPTIONS NEVER EXPIRED, AND NOTHING TOLD THE STUDENT THEY WERE ABOUT TO (2026-09-28)

- **What broke.** Expiry existed in the schema and nowhere in the flow.
- **Rule kept.** A lifecycle state the UI cannot show does not exist for the user.

## FOUR HONESTY BUGS, AND THE SIDECAR THAT CAUSED TWO OF THEM (2026-09-27)

- **What broke.** Four surfaces reported something other than what was true; a
  sidecar process caused two.
- **Rule kept.** The app reports what the engine *said*, not what it hoped.

## WINDOWS CI COULD NOT CHECK OUT THE REPO — 62 FILES WITH A COLON IN THE NAME (2026-09-27)

- **What broke.** Tracked filenames contained Windows-illegal characters, so a
  Windows runner could not check out the repo.
- **The fix.** §5 fails the build on a Windows-illegal tracked path.

## THE TRAFFIC DISPLAY WAS TOO THIN, AND "CONNECTED" COULD NOT BE TRUSTED (2026-09-27, 3.2.0)

- **What broke.** Both directions of the connection surface — traffic and status —
  were guesses.
- **Rule kept.** A status indicator is a claim; it must be traceable to a
  measurement.

## CONNECT DID NOTHING, AND THE APP NEVER ASKED FOR ELEVATION (2026-09-27)

- **What broke.** The connect path needed elevation and never requested it.
- **Rule kept.** Request elevation before the operation, not after it fails.

## THE APP NOW LOOKS LIKE LOCUS, NOT LIKE CLASH VERGE REV (2026-09-26)

- **What changed.** The rebrand: the shipping two-tab UI —
  [`../../client/docs/FRONTEND.md`](../../client/docs/FRONTEND.md).

## THE STUDENT'S REPORT: "THE APP IS NOT WORKING AND THE REBRANDING IS INCOMPLETE" (2026-09-26)

- **What broke.** A customer-visible list of failures, several of them rebranding
  leftovers.
- **Rule kept.** A customer report is a specification; work from it, not from the
  changelog.

## THE BLANK WINDOW, PART 2: THE LOADING OVERLAY COVERED THE ACTIVATION SCREEN (2026-09-26)

- **What broke.** A second, independent blank-window path: the overlay never
  dismissed.

## THE CLIENT RENDERED A BLANK WINDOW — ASSET URLS RESOLVED AGAINST THE PROTOCOL ROOT (2026-09-26)

- **What broke.** Relative asset URLs resolved against the protocol root, so the
  bundle loaded nothing.
- **Rule kept.** §16: no absolute `frontendDist` is compiled into the client.

## "LOCUS HAS NO LINUX ELEVATION PATH" WAS A CLAIM ABOUT THE RETIRED CLIENT, NOT THE FORK (2026-09-26)

- **What broke.** A limitation of the retired client was read as a constraint of
  the live one.
- **Rule kept.** Always ask *which* system a sentence is about before believing it
  ([`../operate/CLAIMS.md`](../operate/CLAIMS.md) §3).

## `/api/hiddify` HAD NEVER WORKED, AND ITS `all=1` LEAK WAS NEVER EXPLOITABLE (2026-09-25)

- **What broke.** An endpoint that never worked, with a suspected data leak that
  was not reachable.
- **Rule kept.** Distinguish "broken" from "exploitable" before spending an
  afternoon on either.

## RELEASE MACHINERY REMOVED, NOT REPAIRED (2026-09, current)

- **What changed.** Root `VERSION`, `bump.sh`, the syso scripts and
  `.github/workflows/build.yml` versioned the **retired** Wails client. They were
  deleted rather than fixed.
- **Rule kept.** The client has its own version sites and its own CI —
  [`../operate/RELEASING.md`](../operate/RELEASING.md),
  [`../operate/CI-CD.md`](../operate/CI-CD.md). Never re-add a root version file.

## RELEASE TAGGING — THE .syso WENT STALE AGAIN, AND THE GATE THAT KNEW (2026-09-20)

- **What broke.** A regenerated build resource was not re-committed before tagging.
- **Rule kept.** Generated artifacts that ship must be regenerated in the same
  change that invalidates them.

## CLIENT UPDATE — THE PORTABLE BINARY COULD NOT REPLACE ITSELF (2026-09-20)

- **What broke.** A running portable binary cannot overwrite itself on Windows.
- **Rule kept.** Self-update on Windows goes through an installer, not a file copy.

## CLIENT TUNNEL/RECOVERY — NORTON KILLS THE SOCKET; THREE STATE BUGS IT EXPOSED (2026-09-20)

- **What broke.** An antivirus killed the tunnel socket, and the state machine had
  no path back from it.
- **Rule kept.** Recovery must be reachable from every state the app can enter —
  including the ones only a third party can put it in.

## RELEASE TOOLING — VERSION DRIFT AND A PUBLISH THAT WROTE AN UNUSABLE ROW (2026-09-19, later pass)

- **What broke.** The tag and the version files disagreed, and a publish wrote a
  row no client could use.
- **Rule kept.** `pnpm release-version`, then commit, then tag — in that order;
  the tag must name the version being built.

## ELEVATION — "ELEVATED COPY DID NOT HAVE PERMISSION" ON EVERY CONNECT (2026-09-19)

- **What broke.** The elevated copy lost the setup it needed on the way up.
- **Rule kept.** An elevated child must be handed its state, not rediscover it.

## TIERS PAGE — STALE WARNING CONTRADICTED THE STRIKE DESIGN (2026-09-19)

- **What broke.** A warning in the console contradicted the shipped tier design.
- **Rule kept.** Console copy is documentation; it rots the same way.

## UDP-OVER-TCP WAS NEVER CONNECTED, AND THE .syso SHIPPED STALE (2026-09-19, later pass)

- **What broke.** `uot_port` on the hub and `server_port_uot` on the client — one
  value, two names, a silent no-op. UoT was dead fleet-wide while both sides were
  individually "correct".
- **Rule kept.** Frozen wire names (§2). Two sides agreeing on a value is asserted
  as agreement, never per side.

## TWO PUBLISHERS DESCRIBED update_config DIFFERENTLY (2026-09-19, later pass)

- **What broke.** Two publish scripts wrote `update_config` inconsistently, and one
  handed a Windows client a Linux binary.
- **Rule kept.** `download_*` → `update_*` is a frozen name (§1, §35); both writers
  are asserted to agree.

## CONSOLE RELEASES — AUTOMATIC ARTIFACT VERIFICATION (2026-09-19)

- **What broke.** The console trusted that a published artifact matched its row.
- **The fix.** The console verifies the artifact automatically before showing it as
  live.

## RELEASE PIPELINE — `manifest.json` WAS NEVER PRODUCED (2026-09-19)

- **What broke.** The release pipeline named a manifest it never wrote, so the
  updater had nothing to read.

## CLIENT UPDATE SYSTEM — BUILT END-TO-END + PB HIDDEN BREAKAGE (2026-09-19)

- **What broke.** The end-to-end update path exposed PocketBase breaking on fields
  it silently discarded.
- **Rule kept.** A hook writing a column that does not exist looks implemented and
  records nothing — the `unbound_at` / `unbind_reason` defect (§3).

## SECRETS WIRING & BACKUP VERIFICATION — TWO MORE BUGS (2026-09-19, later pass)

- **What broke.** Two defects in the secrets wiring and the backup verification that
  only a restore could reveal.
- **Rule kept.** A backup is verified by restoring it, not by writing it.

## BLANK-VPS DEPLOY — SEVEN BUGS FIXED (2026-09-19)

- **What broke.** A deploy to a blank box surfaced seven defects that a re-run over
  an existing box had hidden.
- **Rule kept.** A deploy procedure is proved on a blank box; skip-if-exists steps
  hide exactly the defects a fresh install has.

## THE LOG WAS TRIMMED: THE RETIRED CLIENT'S HISTORY IS ONE SECTION NOW (2026-10-05, docs)

- **What broke.** ~1,090 lines described a client that no longer ships, in a log a
  reader takes as the record of *this* product.
- **What was done.** The eleven pre-2026-08-17 entries were replaced by the durable
  rules they produced, which are stated where they are enforced. Nothing was
  deleted from git.
- **Rule kept.** A date is not enough of a label for "retired"; the folder is — the
  same rule [`../README.md`](../README.md) rule 1 applies to documents.

---

## The rules this log produced, in one place

Each is enforced somewhere in the tree rather than remembered:

| Rule | Enforced by |
|---|---|
| Agreement between two sides, not each side | §1, §24, §31, §32, §33, §34, §35 |
| A guard must be shown to fail | [`DEBUGGING-METHOD.md`](DEBUGGING-METHOD.md) |
| No volatile fact in a sentence | §10, [`../operate/CLAIMS.md`](../operate/CLAIMS.md) |
| No statement of a fact twice | §9, [`../STATE.md`](../STATE.md) |
| The code wins over the document | §8, [`../README.md`](../README.md) rule 2 |
| A remedy the customer can act on | [`../operate/RECOVER-WINDOWS-UPDATE.md`](../operate/RECOVER-WINDOWS-UPDATE.md), [`../operate/RECOVER-MACOS-UPDATE.md`](../operate/RECOVER-MACOS-UPDATE.md) |
| A committed fix is inert until redeployed | [`DEBUGGING-METHOD.md`](DEBUGGING-METHOD.md), [`../../AGENTS.md`](../../AGENTS.md) |
