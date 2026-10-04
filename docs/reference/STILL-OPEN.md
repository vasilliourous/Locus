# Still open after this work

```
audience:    builder
status:      live
authoritative-for: what is unfinished, unverified, or deliberately deferred
verified-against: docs/STATE.md
```

What I did **not** finish, and what a successor should know before picking it up.
Ordered by whether I could have validated it here.

---

## Open, and blocked in this environment

### macOS auto-update is now the right SHAPE, but has never run on a Mac

**Added 2026-10-04, for v3.2.27.** macOS was reported as "the update wasn't
offered" (see `FIXES.md`). Two things were wrong, and only one of them is
verified fixed.

**Verified:**

- The offer is no longer silently suppressed. Every path that skips an offer now
  logs at `warn`/`info` (they were all `debug`, and the default level is `Info`,
  so a dropped offer left no trace), and the reason is surfaced on the Account
  page. The `auto_check_update` case — the likely trigger, and the one a student
  can fix — gets its own wording and a one-tap repair.
- The published macOS payload is now a `*.app.tar.gz` holding the ad-hoc-signed
  bundle, and the three sites that name it agree (CI's manifest,
  `fetch-release.py` `PLATFORMS`, `publish-release.sh` `PLATFORMS`), enforced by
  `check-consistency.sh` §24. Every §24 assertion was observed failing against
  the defect.
- The client refuses a bare Mach-O as the wrong artifact kind rather than
  falling through as an unknown container, and accepts a gzip.

**NOT verified, and this is the important part:**

- **That a macOS client installs the new payload.** No Mac was involved. The
  tarball has never been consumed by a real updater, and the *signature over the
  bundle* has never been checked by macOS. The change is verified as far as "the
  artifact has the shape the plugin documents" — a **structural argument**, not a
  measurement.
- **That the tarball's ad-hoc signature survives the repack.** CI verifies the
  bundle inside the rebuilt `.dmg` before packaging, but nothing verifies the
  bundle *inside the tarball* the way macOS will on a student's machine.
- **That the stale-setting theory is the actual cause of this report.** It is
  the most likely explanation given what the student saw, and it is now
  diagnosable and fixable — but the original 3.2.24 machine was never inspected,
  so it remains a hypothesis. The fix does not depend on it being right: the
  reason is now visible either way.

**The specific observation that would settle it.** On a Mac running 3.2.26:
confirm the Account page reports no obstacle, install 3.2.27, and let the app
update itself. Then repeat from 3.2.24, which is the version in the report.
Until that runs, the honest claim class for macOS auto-update is **structural
argument plus CI guard**, and it should not be recorded as working.

---

### The free tier's throttle is decided, stored — and applied to nothing

**Added 2026-10-04, updated for 3.2.26.** This is the largest remaining gap in
the free tier, and it is worth stating precisely because the field looks wired.

What exists: the hub sends `free_throttle_mbps`; the client parses it (tolerantly
— a malformed value does not fail the beat), persists it (`store::store_throttle_mbps`)
and can read it back (`store::throttle_mbps`). `check-consistency.sh` §23 asserts
that reader exists.

What does **not** exist: anything that lowers a running Core's speed from that
value. The **decision** to throttle works end to end; the **act** does not.

So today's honest behaviour for a student past their allowance is:

- the usage bar says "slowed until your allowance resets" — which is the
  committed copy, and is therefore **currently untrue**;
- the connection is **not** actually slowed, beyond the server's 1 Mbps `tc` cap
  that applies to the tier regardless.

That is a documentation-shaped problem as much as a code one: 3.2.26 shipped the
sentence before the mechanism. The `td` cap on the hub means no student is worse
off than before — 1 Mbps is the free tier's ceiling either way — but the app is
claiming something it does not do.

**The specific observation that would settle it.** A free code, past its
allowance, on a machine with a client: measure throughput and compare against a
fresh free window. They are identical today. Until the applying step exists,
`04-tiers.md` §4.4.5's "throttled further" is a design intention rather than a
behaviour, and the banner's wording should be read as aspirational.

---

### The free tier's hub half is not deployed, and no free code has been exercised

**Added 2026-10-04.** The free tier's quota now exists end to end in the tree: a
1 Mbps `tc` cap, a heartbeat-provided allowance, a client that counts its own
30-day window (`client/src-tauri/src/locus/usage.rs`), and — as of 3.2.26 —
clamping and skew guards on the advisory inputs.

**Verified:**

- The rule. Fifteen tests cover the window rollover, the 80% line, the
  at-allowance boundary, saturation, the skew repair, the allowance clamp, and
  "no allowance never throttles". `warn_fraction_matches_the_integer_comparison`
  was **observed failing** when the boundary was moved to 90%, so the integer
  comparison and the documented fraction cannot drift apart.
- The wire shape on both sides, tolerantly: a malformed advisory field no longer
  fails the beat, and a well-formed one (including zero) still parses.
- 597 lib tests, clippy `-D warnings`, `tsc`, `lint`, 96 frontend tests, the web
  build and `check-consistency.sh` (101 checks) all pass. Every §23 assertion was
  observed failing against its defect.

**NOT verified:**

- **That the hub half is live at all.** Nothing was redeployed. `setup.sh` deploys
  from `/root/server/`, so the `tc` cap, the `free` tier row and the heartbeat
  keys are **inert until it re-runs**. A heartbeat response that lacks
  `free_allowance_mb` is proof the hook is still stale.
- **That a `free`-tier code activates and connects.** No code was minted against
  the new row; the row was added to the seed, not exercised.
- **That the guards fire on real data.** The clamp only triggers on a hub sending
  an implausible value, and no deployed hub sends one — so the failure path is
  tested but never encountered.

See the entry above for the throttle's store-but-not-apply gap.

---

### The macOS Gatekeeper bypass is argued from how macOS classifies signatures, never observed on a Mac

**Added 2026-10-03.** The macOS install path changed: the `.dmg` no longer carries
a `READ ME FIRST.txt` telling the student to run `xattr -cr` in Terminal, and the
bundle inside it is now **ad-hoc signed** (`codesign --force --deep --sign -`),
with a `.zip` of the signed `.app` published as a human download alongside it.

The claim the change rests on is a claim about **macOS**, not about this tree:

> An unsigned, quarantined `.app` fails Gatekeeper as *corrupt* — *"…is damaged and
> can't be opened"* — and that dialog offers no "Open Anyway". An ad-hoc-signed
> bundle fails as *unverified* instead, because the signature exists but is not a
> Developer ID, and **that** dialog is surfaced in System Settings → Privacy &
> Security with a working **Open Anyway**.

**What is verified here:**

- the signing step runs, verifies (`codesign --verify --deep --strict`) and prints
  the identity; a failure fails the build rather than warning and continuing;
- the signing/repack step is macOS-gated on `matrix.label`;
- the repack re-mounts the rebuilt image and re-verifies the bundled `.app`;
- the zip is asserted to contain `Locus.app/` and `Locus.app/Contents/MacOS/` by
  name, and to **extract to a bundle that still verifies as signed**;
- the zip can never enter the update path — CI's manifest and the hub's
  `PLATFORMS` both refuse a `.zip`, asserted by `check-consistency.sh` §1c, and
  each of those assertions was observed failing against the defect it catches.

**What is NOT verified, and cannot be from here:**

- **That macOS actually draws the distinction above.** The whole design turns on
  Gatekeeper treating ad-hoc-signed-but-unnotarized as an identity problem rather
  than corruption. That is read from Apple's documented behaviour and from how the
  same distinction is described for `spctl`, **not measured**. No Mac was involved.
- **That the "Open Anyway" button appears** for this build, on the macOS versions
  students run. On macOS 15+ a first right-click → Open may be needed to force the
  dialog before the Settings pane offers the button; that interaction is untested.
- **That the zip extracts to a launchable app on a real machine.** `ditto -x -k`
  is the documented-correct extractor and the signature survives it — but only a
  Mac confirms the app then opens.
- **The `.dmg` repack itself.** `hdiutil attach`/`create` and `codesign` run only
  on the macOS runners, so neither the signing nor the rebuild has executed
  anywhere in this environment.

**The specific observation that would settle it.** On a real Mac: download the
`.dmg` from a tagged release in a browser, drag the app to Applications, launch
it, and record **which** dialog appears and whether System Settings offers **Open
Anyway**. Then repeat with the `.zip` from the console's Releases page. Record both
on `CLAIMS.md` §5 (a new claim — A6/A7 cover Windows only and are unaffected).

**Until then**, the honest claim class for the whole macOS change is **structural
argument plus CI guard**, and the old README's implicit claim — *"this is expected
and here is what to type"* — is **not** replaced by anything verified on hardware.

NOTE: this work is on the `rice/themes` branch and is uncommitted-then-committed
per change; nothing here has been tagged, so no student has received it.

---

### The egress probe judges the tunnel by the group's selected member, and a 504 is unreachable by the classifier

**Added 2026-10-02. FIXED the same day** — the probe now asks each outbound by name
instead of the group. Kept here because the *verification* is what is still missing;
see the section above.

### The traffic proof is tested as a rule, never against a live Core (2026-10-02)

**Added 2026-10-02. The fix is applied; this is what is NOT proven about it.**

`Readiness::Ready` now accepts **either** a measured delay-test round trip **or** the
Core reporting bytes moving on its own `/traffic` stream
(`core/manager/traffic_probe.rs`). The motivation is in `FIXES.md`; the map is
`EGRESS-READINESS.md` §6.

**Verified:**

- The rule. `observed_traffic_alone_is_ready`, `traffic_that_stops_ends_readiness`,
  `a_serving_core_carrying_nothing_is_not_connected`, and the sink's own window/total
  guards all pass, and each was **observed failing** against the code it catches
  (a one-condition `decide`, and a `return true`-on-lifetime-total sink).
- `cargo clippy --lib` and `cargo build --lib` are warning-free; 564 lib tests pass;
  `check-consistency.sh` exits 0; the phase unit tests pass.
- The wiring is read, not run: `observe_readiness` → `traffic_probe::ensure_stream`
  → `outcome()` → `decide`, and `locus_status` → `trafficFlowing` → the TS type.

**NOT verified:**

- **That a real `/traffic` stream ever feeds the sink on a real machine.** No live
  Core was run here, so the claim "the Core reports bytes and the app says connected"
  is a structural argument from the plugin source (`ws_traffic` → `/traffic`, the
  same socket the on-screen graph uses), not a measurement. This is the gap that
  matters: the whole fix rests on that stream delivering.
- **That this was the reporter's cause.** The delay test's false-negative mechanism
  is argued from the report's own screenshot (traffic numbers moving on a screen that
  says "connecting"); the reporter's machine has not been instrumented.
- **macOS tray coexistence.** That the tray rate task and the app's own stream never
  both connect — `ensure_stream` asks `Tray::speed_task_running` first, but the two
  writers were never exercised together on macOS hardware.
- **The stream's reconnect loop.** `TRAFFIC_STALE_TIMEOUT` is copied from the tray
  task's own choice rather than measured, and the reconnect path has not been run
  against a Core that dies mid-stream.

**What to do with a live machine:** connect, confirm `trafficFlowing` is `true` in
`locusStatus()` while the graph moves, then kill the Core and confirm it goes `false`
within `TRAFFIC_ACTIVE_WINDOW` (3 s) rather than staying green on the last sample.

### Two engine tests read `ok` while never running, and there was no way to tell (2026-10-02)

**Found while pre-emptively testing the traffic-readiness change. This is a
*verification* defect, not a product one — and it is the reason the previous three
"stuck on connecting" fixes could ship with the engine suite green.**

`tests/egress_probe_engine.rs` and `tests/tier_profile_engine.rs` report a
situation they could not test by printing `SKIP: …` to stderr and returning. A
returning test is a **passing** test, so `cargo test` prints `ok` either way.

**Reproduced, not theorised.** With `a_reachable_member_yields_a_usable_delay`'s
assertion changed from `last.0 == 200` to the impossible `last.0 == 59999`, the
test still reports:

```
SKIP: no network egress from this runner; the sidecar could not complete a round trip …
test a_reachable_member_yields_a_usable_delay ... ok
test result: ok. 1 passed; 0 failed
```

That is a green tick guarding nothing. On this runner the case that matters — a
**completed** round trip through the engine — has never executed, and the same is
true of every CI runner without egress. The four cases that do run here all assert
*failure* shapes (a dead member, an all-fail group), so the suite currently proves
only that a broken tunnel reads as broken, never that a working one reads as
working. **That is precisely the direction the three shipped defects failed in.**

**What changed:** both files now skip through a `skip(reason)` helper that honours
`LOCUS_REQUIRE_LIVE_ENGINE=1`, turning every skip into a failure. Verified: with
the variable set, `a_reachable_member_yields_a_usable_delay` FAILS on this runner
instead of passing. On a machine with a real engine and egress, that is the run
worth trusting; without the variable, behaviour is unchanged.

**Still open:**

- **Nothing sets `LOCUS_REQUIRE_LIVE_ENGINE` in CI.** The mechanism exists and is
  proven, but no workflow uses it, so the default `cargo test` is still skip-silent.
  Decide where it belongs — a job with egress, or a required release-time check.
- **No egress on this runner**, so the positive case is still unrun here. Whatever
  machine closes `docs/reference/STILL-OPEN.md` §"the per-member egress probe" needs
  to run it with the variable set.
- The same `eprintln!("SKIP")` pattern may exist in other suites; only these two
  engine files were swept.

### Binding-key migration: the hook must ship before (or with) the client, or already-bound devices 403

**Added 2026-10-02.** Codes no longer bind to the re-derived hardware fingerprint;
they bind to the device's durable `device_id` (see the `FIXES.md` entry above). A
device bound *before* the change still has the old fingerprint on its code row, and
`activation.pb.js` now migrates that row when the incoming request proves the same
device via its `verifier`.

- **Not verified:** the `403 → migrate → 200` path end to end. It needs a real code
  bound to an old fingerprint plus a device presenting the matching identity. No
  live code was touched — it is production data — so this is a **structural**
  argument from the hook source.
- **Not verified:** that a real Windows/macOS update-and-reinstall now keeps its
  code. This environment has no such hardware.
- **The deployment-order hazard.** The client half is active the moment the new
  version installs. The hub half is **inert until `setup.sh` re-runs** (`pb_hooks/`
  deploys from `/root/server/`, not this repo). If the client reaches a device
  *before* the hook is deployed, that device presents its new `device_id` to a hub
  that still only knows the old fingerprint — and 403s with its own code. **Deploy
  the hook before or with the client.** A hub without the migration is not merely
  "not yet improved"; it is the failure mode reintroduced for every already-bound
  device.

### Activation never registers a device identity — SUPERSEDED (2026-10)

**Superseded 2026-10.** The whole device-identity mechanism was removed rather
than deployed: recognition never worked in the field, and a code is now
single-use and not tied to a device. There is no `device_identities` row to
write, no `registerIdentity`, and no `/api/device-recognise`. See the 2026-10
entry in `FIXES.md` and [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

The entry below is kept as the historical record of the fix that was applied
before the removal.

**Added 2026-10-02. Fix applied the same day.** `grep -rln device_identities
server/pb_hooks/` returned `heartbeat.pb.js` and `device_recognise.pb.js` — both
readers. Nothing created the row. `/api/activate` now calls `registerIdentity` on
both success paths, and the client sends the `verifier` it already computes.

- **Verified:** the guards fail against the pre-fix code (both calls removed →
  red; `locus_activate` reverted to plain `activate()` → red). `activation.pb.js`
  parses; 584 Rust tests pass.
- **Not verified:** that a device, a code and a reinstall now work end to end. No
  live hub call has been made from here.
- **Not yet true:** the hook is **inert until `setup.sh` re-runs on the host** —
  `pb_hooks/` deploys from `/root/server/`, not from this repo. A device already
  bound registers on its next activation or recognition attempt; one that never
  calls `/api/activate` again still has no row.
- **Do not "fix" it by clearing `bound_fingerprint` on reinstall.** That dissolves
  the one-code-per-device guarantee. The hub is correctly asking who is calling.

### The per-member egress probe is verified structurally, never through a real tunnel

**Added 2026-10-02. Fix applied the same day — this is what is NOT proven about it.**

The probe no longer asks the `select` group (which answers only for its *currently
selected* member) and instead asks each outbound in `OUTBOUND_NAMES` by name. The
reasoning is sound and the asymmetry it rests on is read from the plugin source:

- `delay_group` raises any non-2xx as `Err` — "every member failed" arrives as an
  indistinguishable transport error with no map to inspect;
- `delay_proxy_by_name` maps a non-2xx to `Ok(ProxyDelay { delay: 0 })`, so a dead
  member is an ordinary value the classifier rejects and the loop moves on.

**Not proven:**

- **That this was the reporter's cause.** The symptom (bars moving, `whatsmyip` on
  the exit node, button amber) is consistent with it, and the mechanism reproduces
  against a local sidecar — but no Locus tunnel on a school link has been measured.
  The commands to confirm are in `EGRESS-READINESS.md` §3.
- **That a real tunnel now reports ready.** Confirming needs a device on a working
  tier, which this environment does not have.
- **That the per-member route is faster or slower.** It can now cost up to
  `len(OUTBOUND_NAMES)` round trips per attempt instead of one. On a healthy tunnel
  the first member answers, so the cost is unchanged; on a dead one the check is
  bounded by `EGRESS_DEADLINE` as before, but the *distribution* of latency has not
  been measured on a real link.

**A trap for whoever verifies this:** `tests/egress_probe_engine.rs`'s positive case
**SKIPs green** when the runner cannot complete a round trip — on the machine this
was written on, it does. "The engine test passed" there means it did not run its
assertion. Check for the `SKIP:` line, not the green tick.

### Every installed 3.2.17 Windows client is stranded, and the fix cannot reach it

**Added 2026-10-02.** The 2026-10-02 `FIXES.md` entry ("the update to 3.2.18 is
not an installer") fixed the guard that refused a genuine NSIS installer. What is
**not** verified is that any affected machine recovers by itself:

- **Not verified, and not verifiable from here:** that a Windows client on 3.2.17
  installs 3.2.18. It cannot: the fix lives in the payload 3.2.17 refuses, so the
  bug is self-sustaining. Every client still on 3.2.17 needs a **manual** install
  of the 3.2.18 setup executable before its updater works again. Whether that has
  been done on the fleet is a world claim, not a repo fact.
- **Unknown, and worth naming:** how many Windows clients are on 3.2.17. Comparing
  `client.version` in `docs/state.toml` (3.2.18) against the installed base says
  nothing about the base — the hub stores no version per device, deliberately.
- **Not observed:** a 3.2.18 → 3.2.19 update. That is the first transition the
  fixed guard makes possible, and it will not happen until 3.2.19 exists.
- **Reconstructed, not measured:** the raw `locus-windows-amd64.exe` is no longer
  published (404 under `/updates/3.2.18/`), so the fixture used to prove the
  post-fix constant still refuses it carries the *documented* `NullsoftInstaller`
  offset rather than a measured one. The refusal is a structural argument.

If you are reading this because a student reported the same message on a build
**other than** 3.2.17, stop: the fix did not ship in that build either, and the
manual install is the only route.

---

### The service-recovery proxy clear was argued structurally, never observed

**Added 2026-10-01.** The 2026-10-01 `FIXES.md` entry ("recovery from a lost
service owner reset the wrong platform's proxy") changes
`owner_recovery_policy` so recovery resets **no** machine-wide proxy on any
reason or platform. The guard is real — it was demonstrated failing against the
pre-fix rule — but what it guards is a *decision*, and the decision rests on a
structural argument about ordering, not on a reproduction:

- **Not observed:** a Windows or Linux recovery that actually switched off a live
  system proxy. The claim is that the pre-fix `reset_system_proxy: true` arm
  attempted it; it is not a report of it happening. Reaching it needs a displaced
  or transport-lost Service owner on a machine with the system proxy enabled and
  the tray app running.
- **Not observed:** that the macOS arm really was unreachable. The argument is
  that `clear_active_service_session()` runs three lines above the clear, so
  `active_service_session()?` must fail. That reads correctly from the code and
  the failure is logged rather than silent, so a log on a real macOS recovery
  would settle it — nobody has looked at one.
- **Unmeasured:** whether the macOS clear *failed* or the retry loop exhausted
  its three attempts. Both are consistent with the code; which one occurs decides
  whether the branch is dead or merely futile.

Confirming any of these needs a machine where the privileged Service can be
displaced while the Core is running. Until then the honest claim class is
**structural argument**.

---

### The activation code's durability has never been exercised against a real machine

**Added 2026-10.** The code is now mirrored to a machine-scoped store
(`locus/credential.rs`) so a reinstall, an update or an unexpected shutdown does
not lose it. The unit behaviour is tested — round-trip, corrupt file, empty code,
config-first-then-mirror ordering — and the module is pure I/O against a scratch
directory.

What is **not** observed, and cannot be from here:

- That a real Windows or macOS install writes `/Library/Application Support/Locus`
  (or `%PROGRAMDATA%\Locus`) successfully, without elevation. The fallback is
  the app config, which does not survive an uninstall — so on a machine where the
  machine store is unwritable, the guarantee is weaker than the design intends.
  That distinction is *not* surfaced in the UI today; it should be, before this is
  called done.
- That the code survives an actual uninstall-and-reinstall cycle, and that
  `read_code_rehydrating` re-adopts it into `verge.yaml` on the next launch.
- That re-activating a redeemed code on the hub returns the tier config and
  restores a working tunnel. Both paths parse and are contract-tested, but the
  hub hooks have not run against a live database from here.

Confirming these needs a device, a hub, and a real uninstall. Do not read
"the tests pass" as covering them.

---

## Open, and needs a decision rather than work

### Nothing in the UI can clear a selected theme — `theme_mode` becomes unreachable

**Added 2026-10-01.** From the theme work (2026-10-01 entry in `FIXES.md`; design
record in [`THEMES.md`](THEMES.md)).

A theme carries its own light/dark mode and **supersedes `theme_mode` while one is
set** (`THEMES.md` §2). The dropdown on Account only ever *writes* a theme id —
every option in it is a theme — and nothing clears the field. So after a student
picks any theme, the Appearance control above it becomes inert: it still stores a
value, and the value is no longer what the app honours. "Follow the system" is
reachable only by editing `verge.yaml` by hand.

**This is a decision, not a coding task.** The obvious shape is an explicit
*Default (follow system)* entry that writes an empty/absent `theme_id`, but that
changes what the dropdown means for an existing install (it would gain an option
meaning "none of the above"), and it interacts with the 3rd gap below. Picking a
shape is a product call, so it was not made silently.

**Until it is made:** the two Appearance controls can visibly disagree, and the
honest description of the second one is "overrides the first while set".

### The theme dropdown and the `theme_mode` dropdown are separate and can disagree

**Added 2026-10-01.** Same change.

`account.tsx` now has two adjacent controls in Preferences: Appearance
(`theme_mode`, 3 options) and Theme (`theme_id`, 6 options). When a theme is set,
the first shows a value the app is not honouring. `THEMES.md` §7 records this as
deliberately deferred rather than overlooked — merging them is a UI change with
its own migration question (what does an existing "Dark" choice become?) and is
not needed for themes to work. **The point where this becomes confusing to a
student is the point to do it,** and that point is probably the same change as the
entry above.

---

## Open, and blocked in this environment

### The publish guard calls `atob`, which goja does not have — the fix is proven but not applied

**Added 2026-09-30.** The admin console refuses to publish or activate **any**
release, reporting for all four platforms *"signature not base64 of a minisign
.sig file"* — a message that wrongly blames the fetch. The full diagnosis is the
2026-09-30 entry in `FIXES.md`.

In one line: `server/pb_hooks/admin_console.pb.js`
`isPlausibleMinisignSignature()` decodes with `$os ? atob(s) : ""`, and
**PocketBase 0.22.21's goja defines no `atob`, no `btoa`, no `Buffer`, and no
`TextDecoder`** — nor does `$security` or `$os` expose base64. The call throws,
the function's own `catch` returns `false`, and every value is judged invalid,
correct ones included.

**The fix is proven on the live hub but has not been committed.** A pure-JS base64
decoder (alphabet lookup plus a bit accumulator) decodes all four real signatures
to minisign's 4-line shape. That patch was applied, tested, and **reverted** —
`/opt/pocketbase/pb_hooks/admin_console.pb.js` is byte-identical to
`/root/server/pb_hooks/admin_console.pb.js` (md5 `306e7780…`), so the live hub is
in its original, still-broken state. Nothing was left behind.

To finish it: replace that one line with a self-contained pure-JS decoder, then
**restart PocketBase** before testing — it caches `pb_hooks` at startup, and a
stale-cached hook will make a correct patch look like it did not apply.

Two things a successor should know:

1. `atob`/`btoa` should be checked across the whole hook tree
   (`grep -rn 'atob\|btoa' server/pb_hooks/`) — this class of bug is only visible
   at runtime, never at parse time, and the Rust/Python contract test cannot see it.
2. A guard that returns `false` for *every* input is indistinguishable from a
   total data outage in its output. It was not noticed because nobody published a
   release in the window.

### The term model, renewal and the single-use rule have never met a database

**Added 2026-09-29. Amended 2026-10** — one-code-per-device, `codes.rebind` and
`device.get`/`devices.list` are gone with the device-binding removal; the term
model, `codes.renew`, `codes.set-term`, `codes.unbind` (now "release") and the
renewal worklist remain.

**None of the remaining ones has been run against PocketBase.** They parse, the
guards are green, the console builds, and the arithmetic was exercised directly —
but no hook has ever executed on the deployed build, and the schema changes have
never been applied to the live database. The full list of what is unverified, with
the reason each matters, is in
`docs/business/redesign/implementation/05-not-yet-true.md`.

The two to check first, in order:

1. **A first activation followed by a re-activation.** The single-use stamp
   (`codes.activated_at`) is the whole rule now: the first activation must set it
   and report "Activation successful", and the second must report "Already
   activated" and still return the tier config. Neither path has been run live.
2. **The migration preserves the paying codes.** `server/scripts/backfill-terms.py`
   has a `--dry-run` and **never writes `expires_at`**, but it has never run
   against the live hub, which holds real paid codes in active use — no count is
   recorded here on purpose (`operate/CLAIMS.md` §4). Read the diff before applying.

Also note: the removal leaves `device_identities` and `device_bindings` in place
on an existing hub, and `codes.bound_fingerprint` on every code row. None is read
or written any more. Dropping them is a separate, deliberate operator action.

### CI now runs the tests — but nobody has watched it do so

**Added 2026-09-29.** `cargo test`, `cargo clippy -D warnings` and
`pnpm test` were added to the `verify` job. Every one was run locally and passes
(541 Rust across all suites, 49 frontend, clippy clean), and the workflow YAML parses.

What is **not** verified is the workflow actually running on GitHub: the
dependency install (`libwebkit2gtk-4.1-dev` and friends) and the
`--features clippy` flag are the plausible failure points, and a failure here is
red CI on `main` rather than a broken release. The first push to `main` after
this commit is the test.

### No update has ever *successfully* been installed

**Updated 2026-09-29.** The first real install attempt happened and **failed at
signature verification** — see `FIXES.md` ("THE UPDATE COULD NOT BE INSTALLED",
2026-09-29). Two encoding defects were found and fixed: the compiled-in `pubkey`
was the bare minisign key rather than base64 of the `.pub` file, and the hub
never checked the *shape* of `signature_<platform>`, only its presence.

That is progress on the original open question — the updater path is now proven
end to end *up to* the plugin's decode, against the real key and the real crate
versions — but **an install still has not completed**, so this stays open:

- **The decode contract is now verified; the install is not.** A test reproduces
  the plugin's exact `base64_to_string → PublicKey::decode / Signature::decode →
  verify` chain and runs a real signature from `.locus-keys/` through it,
  including the tamper case. What remains unverifiable here is everything after
  verification: the platform installer actually running.
- **Any build with the old pubkey is stranded.** The pubkey is compiled in, so a
  client shipped with the bare-key form will fail verification on every future
  update regardless of how correct the signatures are. Such builds need a manual
  reinstall; check what is actually in the field before relying on an update to
  reach it. (This does not affect a *new* install of a fixed build.)
- **It still needs a published release.** A real test requires a signed release
  on the live hub (`update_config.active = true`) with
  `download_*`/`sha256_*`/`signature_*` populated for the platform, plus a client
  running an older build.
- **The Windows path especially.** The plugin launches NSIS and the app exits, so
  "success" looks like the process disappearing. Nothing here can exercise that.
- **`installMode: "passive"` shows a progress bar the code does not control.**
  Whether that reads acceptably on a school laptop is a judgement only a real
  install can make.

Suggested first test: publish a signed release with `active = false`, flip it to
`true`, and watch one Linux client — the only platform verifiable from a headless
Linux box.

#### The *update* path served the client a raw executable (2026-10-01) — FIXED, publish pending

A Windows client on `3.2.13` was offered `3.2.14`, downloaded 100%, and refused
it: *"not an installer (51741696 bytes)"*. That size is `locus-windows-amd64.exe`.
The hub was serving the raw PE because `fetch-release.py` and
`publish-release.sh` still resolved it for the `windows` slot, while CI's
`manifest.json` had correctly named the NSIS installer since v3.2.12. See
`FIXES.md` ("NO WINDOWS CLIENT COULD UPDATE", 2026-10-01).

The client's refusal was its **own guard working** (`is_installer_payload`,
added in 3.2.12), so no client was ever handed a program it would execute. The
cost was that **no Windows client could update at all**.

Fixed in the hub scripts, with a filename cross-check against `manifest.json`
that did not exist, an NSIS check in the fetch service, an installer-signing step
in CI, and guards in `check-consistency.sh` §1a and `smoke-publish.sh` case 5b —
each shown to fail against the pre-fix code.

> **OPEN: the live hub still serves the old row.** No client sees the fix until
> `3.2.14` is re-published (a build carrying the installer signature, then
> Fetch & publish). Until then every Windows client remains offered the raw
> binary and refuses it. **`CLAIMS.md` §5 A7 is still unverified** — this
> demonstrates the hub serves the right bytes, not that a Windows machine
> installs them.

#### And the Windows *installer itself* was broken on 3.2.9 (2026-09-30)

A student ran the 3.2.9 Windows setup and got an NSIS "File not found" dialog
carrying an **Edge** logo and `ERR_FILE_NOT_FOUND` — see `FIXES.md`
("THE WINDOWS INSTALLER ABORTED", 2026-09-30). The cause was
`tauri.windows.conf.json` naming a custom `template` and an `installerHooks`
`.nsh` that `prebuild.mjs` generates inside `beforeBuildCommand` — which CI
**blanks**. The bundle built; the installer could not.

The config no longer names either file and `check-consistency.sh` §14 guards
that. **The fix is a structural argument, not a running installer**: no NSIS
bundle was produced on the machine where it was made, and nothing in CI executes
one.

**RESOLVED 2026-09-30.** `installer-Locus_3.2.12_x64-setup.exe` was run
interactively on real Windows and completed, installing Locus — the first Windows
install the project has confirmed. `CLAIMS.md` §5 **A6** is now verified for that
installer. (This was unverified when the entry was written; it is kept here rather
than deleted because the reasoning about *why* it went unverified for so long —
nothing in CI executes an installer — still holds, and still applies to A7.)

#### The *update* path put the client outside its install directory (2026-09-30)

This is the defect the two windows above were actually chasing. The artifact
advertised for a Windows client to install from itself was
`locus-windows-amd64.exe` — the raw PE executable, not an installer. The plugin
accepts any PE as an NSIS installer and ShellExecutes it, so an update relaunched
a copy of the client outside `C:\Program Files\Locus\`, with no bundled assets,
and it reported `ERR_FILE_NOT_FOUND` in a Locus window. See `FIXES.md`
("THE APP RE-EXECUTED ITSELF", 2026-09-30).

Fixed on both sides — the client refuses a non-installer payload, and the
manifest's `windows` entry names the setup executable — and guarded by
`check-consistency.sh` §15 plus Rust tests, all of which were shown to fail on the
old behaviour. **What is still unverified is the same shape as everything else
here**: no Windows machine took an update and then launched from the Start Menu.
Tracked as CLAIMS.md §5 **A7**.

#### The shipped binary embedded the CI runner's `D:` path as its asset root (2026-09-30)

The **third** report of the same `ERR_FILE_NOT_FOUND` text, and this one rules out
both earlier causes on its own facts: the installer was pulled **fresh from the
release page** (no update), and `verge.yaml` had `start_page: /` (no config path).
The log's `url=file:///D:/` is the Windows CI workspace `D:\a\Locus\Locus`,
compiled into the binary by an absolute `frontendDist` override that the "better
workflow" speed change added to `TAURI_CONFIG` — because `tauri::generate_context!()`
bakes `frontendDist` into the executable as its runtime asset root. See `FIXES.md`
("THE SHIPPED CLIENT LOOKED FOR ITS ASSETS ON THE CI RUNNER'S `D:` DRIVE",
2026-09-30).

The override is removed (the committed relative `../dist` resolves correctly, and
the bundle is already downloaded to `client/dist`) and `check-consistency.sh` §16
guards it — shown to fail against all three shapes it can be reintroduced in.

**Verified on the artifact, 2026-09-30.** Pushed as `a8b0acf` (preview, no tag); CI
run `36686775049` went green, and the built `locus-windows-amd64.exe` was read back
from the run's artifacts. The Tauri context string inside it now carries the
relative `../dist` where the broken `5ebfc89` build carried
`d:/a/Locus/Locus/client/dist`; `strings … | grep -c 'a/Locus/Locus'` returns `0`.
Tracked as CLAIMS.md §5 **A8**, now **verified for `a8b0acf`**.

**What remains open, and is the point of keeping this entry**: the check was done
**by hand**, not by CI. Nothing runs the built binary, so the next platform path
leak (macOS, Linux) ships the same way. The CI step that would close this is still
unbuilt — see the class note below.

The interactive Windows install is **no longer** part of this gap: it was run and
completed on 2026-09-30 (`CLAIMS.md` **A6**, verified). What that does **not**
cover is the app surviving an advertised *update* on that install — still
unverified (`CLAIMS.md` **A7**).

#### Nothing in CI runs either artifact — the class behind all three

Worth stating as one thing rather than three. Every check in this project verifies
that a bundle **builds**. None launches the built binary, and none executes an
installer. So:

| Release | Built | Shipped broken |
|---|---|---|
| 3.2.9 | ✓ green | an installer that could not install |
| 3.2.10 | ✓ green | an update that could not update |
| 3.2.11 | ✓ green | a fix for a cause that did not exist |
| 3.2.12 | ✓ green | a signed installer that carried a CI runner path as its asset root |

Four consecutive green runs, four releases a student could not use. A CI step
that runs the built binary and asserts its page-load URL is not `file://` would
have caught 3.2.9, 3.2.10 and 3.2.12 outright; the 3.2.11 miss was a **reasoning**
failure that no CI can catch — it came from inferring a cause from source instead
of asking for the log. The first is worth building. The second is worth
remembering.

**As of 2026-09-30, the artifact check exists but only as a hand-run.** 3.2.12's
fix was verified by downloading the built `locus-windows-amd64.exe` and reading the
Tauri context string out of it (see the entry above) — the same evidence a CI step
would produce. **The step itself is still not written.** So the answer to "does
anything check the artifact?" is now *"a person did, once"* — which is not the
mechanism this class needs. The next platform path leak would ship the same way.

> **The 3.2.12 row is the one to sit with.** A build-machine path was deliberately
> injected into a user-facing artifact for build speed, its *separators* were
> fixed when the build panicked, and it shipped signed. The defect was never the
> escaping — it was the decision, and nothing in the pipeline asks "should this
> value be in the artifact at all".

### The installed app data root and the app's own idea of it can disagree

### The suspension path has not been exercised against a live hub

**Added 2026-09-28 (third round).** The activation screen now shows a refusal, and
`main.tsx` re-reads the entitlement on an interval. What is untested is the real
sequence: suspend a code on the live hub, let a bound client beat, and watch the app
move to the activation screen **on its own** with the hub's sentence shown.

Two specific unknowns:

- **The poll latency.** The gate re-reads every five minutes, matching the
  subscription indicator. A student whose code is suspended can therefore keep a
  working tunnel for up to five minutes — longer if the heartbeat is backing off
  after failures. If that is judged too slow, the answer is a shorter interval on
  the gate (or `beat_now` on window focus), not a change to the heartbeat floor,
  which is pinned by tests and tuned against real school networks.
- **`main.tsx` polls `locusStatus()` directly, not the shared query.** It has to:
  `Shell` renders outside `SWRConfig`, so `useSubscription` is unavailable there.
  The two therefore share the *interval constant* rather than the cache, and can in
  principle disagree for one poll cycle. Acceptable, but worth knowing.

### The readiness probes against a live Core (partly closed 2026-10-01)

**Added 2026-09-28 (second round); extended 2026-09-29 for the egress check; the
engine-side contract closed 2026-10-01; the classifier corrected 2026-10-01
(second pass).**

Readiness asks **two** questions. `probe_core_api` asks the Core's control API;
`probe_egress` makes a real request *through* the tunnel (`delay_group`).

For the full chain and a localisation procedure, see
[`EGRESS-READINESS.md`](EGRESS-READINESS.md).

**Now verified against a real engine.** `cargo test --test egress_probe_engine`
starts the real sidecar with the tier's own group name (`Locus Auto`) and pins the
three facts the probe depends on:

- a **reachable** member yields HTTP 200 with a delay map carrying a usable
  measurement — the `Ok` path that lets the UI leave "connecting";
- an **unreachable** member yields a non-200 (mihomo answers `504 "get delay: all
  proxies timeout"`, and in some paths `200` with a `0` value) that the probe reads
  as `Failing`;
- the sentinels mihomo reports in the same field a measurement uses — `0` for a
  failed test and the timeout value itself for a timed-out one — are classified as
  **not** measurements, so a tunnel that just timed out cannot read as connected.

> **The third of those three now reads the other way.** mihomo's timeout value is
> classified as a **measurement** (`EGRESS-READINESS.md` §4): the engine dialled the
> target and the request left the machine, so a member that hit its budget is
> evidence of egress. Only `0` and `> 1e5` are non-measurements. The test below
> still pins what the *engine* returns; what changed is the probe's reading of it.
> The integration test's positive case does not exercise the slow-but-working path,
> so it does **not** cover this — see the gap below.

That test also settled one behavioural fact that had been assumed: mihomo's
`direct` outbound reports a delay of `0` for a **loopback** target (loopback
bypasses the proxy path), which is why the positive case uses the real egress URL
and SKIPs when the runner has no network. The two deterministic cases need no
network.

What remains genuinely unverified, because it needs a live Locus hub and a real
tunnel:

- **Does `delay_group("Locus Auto", …)` return a usable delay through a *real* Locus
  tunnel?** The engine contract is now proven; whether the tier's `ss` member carries
  the probe's request on a real school network is not. If the group name, the tier
  config or the plugin transport is wrong in the live app, every connect would stall
  into "no egress" and the button would say **connecting** forever — the report this
  work started from. Fail-safe (it never says "connected" wrongly) but still wrong.
- **Is the egress budget right on a school network?** ~~Raised from 3 s to a 5 s
  per-attempt budget within a 12 s overall deadline, with two attempts (see
  FIXES.md).~~ **Answered (2026-10-01, second pass): the budget was the bug.** A
  5 s budget inside a plain `delay < 5000` acceptance rule rejected the very answer
  the engine returned when a member hit that budget — so the probe read "slow" as
  "dead" and pinned the button on **connecting** while traffic flowed. The rule now
  accepts any measured value; the budget no longer decides what counts as egress.
  **Still unmeasured on a real school network:** whether 5 s is the right budget at
  all. It now only bounds *how long the engine waits*, not what counts as success,
  so a too-small budget degrades to a slower answer rather than a false failure —
  but nobody has watched it on a genuinely slow link. See
  [`EGRESS-READINESS.md`](EGRESS-READINESS.md) §4 and §6.
- **A group where EVERY member times out is indistinguishable from a transport
  failure.** mihomo answers `504 "get delay: all proxies timeout"`, the plugin
  raises it as `Err`, and `egress_attempt_once` collapses `Ok(Err(_))` and `Err(_)`
  to the same `false`. The probe therefore cannot retry them differently, and a
  slow group reads as a dead one. **Identified 2026-10-01 (second pass) and NOT
  fixed** — the classifier was corrected, this path was left alone deliberately.
  It is reachable on **Strike**, whose group holds both `Locus-UoT` and `Locus`:
  the delay test dials the group's *selected* member, which is `Locus-UoT`, and if
  that member times out the `504` is indistinguishable from no network. Fixing it
  means distinguishing "the whole group timed out" from "we could not ask", which
  needs a decision about what the probe should do differently. See
  [`EGRESS-READINESS.md`](EGRESS-READINESS.md) §5 and §6.
- **The engine test cannot exercise slow-but-working.** `tests/egress_probe_engine.rs`
  SKIPs when the runner has no network, and its positive case uses a `direct`
  outbound to a public host. The defect fixed on 2026-10-01 (second pass) was a
  **slow round trip through a real tunnel** — and no test in the suite reproduces
  that shape, which is why it shipped twice. A test that only proves the fast path
  cannot fail on the slow one. Closing this needs a deliberately slow in-process
  target or an injected delay map, not a network dependency.
- **Does `get_version()` actually answer over the configured transport?** It goes
  through the plugin, so it should use the same channel as every other Core call —
  but that assumption is untested here. If the address/secret/socket path is wrong
  in a way that only shows at runtime, the probe would report `Unresponsive` for a
  healthy Core, and the button would say **connecting** forever.
- **Is 400 ms enough on a loaded machine?** Chosen short because the status command
  is polled at 750 ms while connecting. A real Core on a slow school laptop that
  takes longer to answer would stall every connect into the "still starting" branch.
- **The no-wifi case end to end.** With the Core running, disconnect the network and
  confirm the button leaves `connected` and settles on **connecting** — the core
  answers `/version` but `probe_egress` fails — rather than hanging on a socket that
  never answers or falsely reporting connected.
- **The poll-storm coalescing under load.** `observe_egress` now serialises and
  caches the through-tunnel check for 1.5 s so the 750 ms status timer and the
  250 ms connect loop collapse onto one round trip. The rule is unit-shaped and
  reviewed; the live timing on a slow machine is not measured.

### Nothing has confirmed the refusal is visible on a real screen

**Added 2026-09-28 (second round).** The chip, the Account page and the
connection-screen line now all render a refusal, and "Check status now" triggers a
real beat. What is untested is the appearance and the round trip: a real suspended
code, a real beat, and a student looking at the result. The states are unit-tested;
the pixels and the timing are not.

Related: `locus_check_subscription` returns "a beat was started", not a verdict, so
the UI cannot tell the student "you are fine" immediately — it re-reads and shows
whatever the *next* status read says. On a slow network that read may land before
the beat completes, so the displayed status can be one refresh behind the check the
student just asked for. Not wrong, but possibly confusing; consider awaiting the
beat's outcome instead of firing it.

### Expiry enforcement has never been exercised against a live expired code

**Added 2026-09-28.** The four holes that let an expired code keep working are
closed (see `FIXES.md`, same date), and every one has unit tests. What has *not*
happened is the real thing: expire a code on the live hub, let a bound device beat,
and watch it be refused with 410 and have its tunnel taken down.

Two things need a live run, and neither is possible here:

- **The hub half.** The 410 check lives in `heartbeat.pb.js`, which reaches the
  live server only on a `setup.sh` re-run. Until that runs, only the client-side
  gate is active — which is enough to stop a *new* connect but not to take down a
  tunnel that is already up. Confirm the re-run happened, then confirm a heartbeat
  from a lapsed device returns 410 and not `status:"ok"`.
- **The grace path.** `enforce_grace_period` stops the tunnel once the window
  since the last good beat is spent. The arithmetic is tested; the effect (the core
  actually stopping, the entitlement actually clearing) is not, because it needs a
  device that has beaten successfully at least once and then cannot reach the hub
  for a week.

Also unverified: the sidebar badge and the connection-screen warning on a real
screen — the states and colours are unit-testable, their appearance is not.

### UoT has never carried a real game session

**Fixed 2026-09-27, in one respect:** `locus_connect` had been calling
`apply::apply_tier(&config, false)` — `udp_relay` hardcoded `false`. So the client
had never once built a UDP-over-TCP profile, no matter what the hub sent, and this
section's whole subject was unreachable in the shipping build. It now reads the
flag from storage, where activation put it.

The underlying claim remains unvalidated. The transport is proven — a SOCKS5 UDP
ASSOCIATE through both the raw 8445 and UoT 8446 paths returns a DNS answer, and
the server log shows `inbound UoT connection to 8.8.8.8:53`. What is **not**
validated is Strike's actual gaming promise: no real game has ever been run on a
school network, and now that the flag actually reaches the profile builder, that
test is meaningful for the first time. If you have access to a school network and
a game, this is the single highest-value thing left.

### The connection check needs a real school network to be trustworthy

**Updated 2026-09-28.** The check no longer compares against the tier server — that
comparison could never succeed, because the server is a hostname and the code
parsed it as an IP, so every run said "cannot confirm". It now records a
**tunnel-down baseline** (one egress reading taken while the core is known not to be
running) and reports "going through Locus" when a later reading **differs** from it.
The verdict logic is a pure function with a six-case test matrix, and a mutation
restoring the old behaviour was confirmed to fail it.

What still needs a real run, and **cannot** be exercised on this headless Linux
host: the whole sequence a student performs — disconnect, run the check to record
the baseline, connect, run it again and see the address change. In particular:

- that a reading taken *just* after the core stops is honestly tunnel-down (and so
  recorded as a baseline, not as a verdict);
- that a connected device on the school network shows a **differing** address;
- what the school actually does — a device behind transparent interception or CGNAT
  is the case the code is designed to survive, and the case the old code got wrong.

The states are unit-tested; the *live* behaviour is not.

### The connect readiness wait, cancel, and scroll are unrun visually

`locus_connect` now waits for the core to be ready (bounded at 30s) and the button
stays cancellable while connecting. The logic is tested; what has not been watched
is the *feel* — how long the button says "connecting" on a real school connection,
whether the 6s "still connecting" hint appears at roughly the right moment, and
whether the new scrollbar on the connection screen looks right against the 6px
window-resize handles it now shares those corners with.

### Linux TUN elevation — **not a gap; only unvalidated** (corrected 2026-09-26)

**This section previously said "there is no elevation path on Linux". That was true
of the retired Wails client and is FALSE of `client/`.** It was carried over from
`archive/ENGINE-SWAP-ANALYSIS.md` without re-checking the fork — the exact failure mode
this repo keeps re-learning (`docs/README.md` rule 3: the code wins).

The shipping fork is built on Clash Verge Rev, so it **inherits** Verge's elevation
path and needs no helper binary of its own. The chain is:

```
pkexec (sudo fallback)  ->  clash-verge-service-install  ->  root service
                        ->  service runs mihomo as root   ->  TUN works
```

- `crates/…/clash_verge_service_ipc` → `management.rs::elevate()` runs the installer
  under **`pkexec`**, falling back to **`sudo`** when it is absent or exits 127.
  `utils/help.rs::linux_elevator()` probes for `pkexec`. (macOS: `osascript … with
  administrator privileges`; Windows: `Start-Process -Verb RunAs`.)
- `core/service.rs::install_service()` → `invoke_service_install()` →
  `clash-verge-service-install`; the service stages and runs the core from its own
  administrator-approved directory (`stage_approved_core`, digest-pinned).
- `core/runstate/health.rs::tun_capable()` is `self.is_admin || self.service_usable()`,
  with tests `tun_is_capable_when_elevated_even_with_no_service` and
  `tun_is_capable_via_a_ready_service_without_elevation`.

This is the same mechanism that made Clash Rev Meta work in the original 2026-08-03
school test — which is why the project rebuilt on Verge rather than repairing the
retired client (whose hand-rolled elevation was buggy three times, FIXES #17/#40/#41,
and is deliberately not ported: `client/docs/LOGIC-INVENTORY.md` §10).

**The only thing open is running it** — nobody has exercised that chain on real Linux
hardware in a non-root session. See item 4 under "fixable now", which is the same
finding stated as a to-do.

---

## Open, and fixable now

### Device identity has never met a real PocketBase

**Added 2026-10-01.** The durable-identity work (see
[`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md)) is complete, tested and compiling, but
**no part of the hub half has run**. There is no PocketBase runtime in this
checkout, so the hooks are syntax-checked and covered by the
`check-consistency.sh` trap wave — the same standard as every other hook here, and
not a substitute for executing them.

Specifically unverified — after the device-identity removal, what remains is:

| Claim | Status |
|---|---|
| The hooks execute correctly on PB 0.22.21 | **Untested.** Never run. |
| A first activation sets `codes.activated_at` and a second reports "Already activated" | **Untested.** The whole single-use rule, unrun. |
| A repeat activation returns the tier config | **Untested.** |
| The term model and `codes.renew` against a real database | **Untested.** |
| The retired tombstone answers `unknown` on the deployed build | **Untested.** |

To close it: deploy (`setup.sh`), then activate a code once and again, and
confirm the first says "Activation successful" and the second "Already
activated" with a `server_config`.

### The code survives a real reinstall — needs real hardware

**Added 2026-10.** The design moves the *decision logic* out of the
"cannot verify here" bucket: `credential.rs` is pure I/O tested against a scratch
directory, and the config-first-then-mirror ordering is unit-tested. What remains
genuinely hardware-gated is whether the machine-scoped store **survives an actual
uninstall** and is **writable without elevation** in each deployment shape.

Needs a real Windows and a real macOS machine, an install -> uninstall ->
reinstall cycle per platform, and confirmation the path is writable by a
non-elevated process. The paths chosen are `/var/lib/locus` (Linux),
`/Library/Application Support/Locus` (macOS), `%PROGRAMDATA%\Locus` (Windows).

Until then, the honest position: an update no longer loses the code, and a
reinstall *should* keep it because the code is written outside the app's own
directory — but nobody has watched it happen.

Where the machine store is unwritable the code falls back to the app config,
which does **not** survive an uninstall. That is a weaker guarantee than the
design intends, and the UI does not yet say which case a device is in. It should,
before this is called done.

### The client works; a real update has never been INSTALLED

**This section used to say the client port had not started. It has, and it
shipped across the whole 3.x line (current version: `docs/STATE.md`).** `client/` has
`src-tauri/src/locus/` with activation, device
fingerprinting, a heartbeat, tier→config translation and a hub-mediated updater,
plus a first-run activation gate and a single Connect/Disconnect.

Verified against the **live hub**, not only by unit tests:

- the Luhn checksum agrees with the deployed server (a valid code reaches its
  database lookup; a checksum-broken one is refused as invalid);
- `/api/code-lookup`'s real response deserialises into the client's types, and
  `bound_other` is classified as "not ready";
- a generated tier config is accepted by the real `mihomo` binary;
- **traffic egresses from the VPS** rather than the local address, and a 5 MB
  download moved the server's `rx_bytes` by 5,135,262; *(the egress address was
  observed at the time and is deliberately not recorded here — the hub is
  addressed by name, `docs/operate/CLAIMS.md` §6)*;
- a real heartbeat returns the strike tier's `uot_port` and `udp_relay`;
- **release 3.0.0 is published**: CI built and signed all four platforms, the hub
  fetched and verified them, and both `/api/update` and the heartbeat offer it with
  a signature that verifies against the key compiled into the client.

**What is genuinely still open:**

1. **No client has ever INSTALLED an update.** Every stage is verified — build,
   sign, fetch, publish, serve, and the signature check the install path performs —
   but the installer handoff itself needs Windows or macOS hardware. This is the
   single most valuable thing left to do.
2. **The client has only been RUN on Linux.** Windows and macOS are built and
   signed by CI but have never executed on real hardware. Given four
   Windows-specific CI bugs already found from Linux, expect runtime surprises.
   **Added to this list 2026-09-27:** the app binary now declares
   `requireAdministrator` (`src-tauri/build.rs`), which is a Windows-only manifest
   change — the UAC prompt itself, the Common-Controls dependency reproduced inside
   the custom manifest, and the still-new high-privilege autostart task are all
   unexercised on Windows. They are the first things to check on the first real
   Windows run, and they are the reason "builds green" is not "works".
3. **A mid-session tier CHANGE has never been exercised.** The code path exists
   (`store::tier_changed`) but no heartbeat has carried a changed tier in practice.
4. **The Linux TUN elevation path is untested** — the fork inherits Verge's
   pkexec→sudo escalation and service, but nobody has run it. Linux is ~2% of
   clients and last in priority.

### ~~The `proxies` page is still in the client~~ — REMOVED in 3.1.0 (2026-09-26)

Resolved by deletion rather than replacement. `pages/proxies.tsx` and the whole
`components/proxy/` tree (~4,900 lines) are gone, and `proxies` is no longer a
navigation entry.

The one genuinely useful thing in there — **live latency**, which a student asking
"is my connection good?" wants — was *not* carried over, and that is the gap this
section leaves behind. The Connection screen shows live up/down speed and the traffic
graph, which answer "is it working?" but not "is it fast?". A latency readout is still
worth adding, and `test_delay` still exists to back it.

**Partly addressed in 3.2.0 (2026-09-27):** the Connection screen gained a
**Connection check** card — a manual public-IP probe that answers "is my traffic
actually going through Locus?" (the app's own "connected" state cannot, because a
tunnel can be up while nothing is carried). That closes the *is-it-working-at-all*
half of the gap. **Latency is still missing** — the check proves routing, not speed,
and a ping/round-trip readout backed by `test_delay` remains worth adding.

### ~~Deferred: the Verge UI a student still meets~~ — DONE in 3.1.0 (2026-09-26)

**This entire section is superseded.** It measured the problem — "8 of ~203 front-end
files mention Locus at all" — and posed four open questions. All four were answered and
the work is finished:

| Surface | 3.1.0 state |
|---|---|
| Activation gate | Locus, rebranded (`UI-AESTHETICS.md` palette) |
| Nav | **two tabs: Connection and Account** |
| Proxies / Logs / Settings | **deleted** — 69 files, ~17,600 lines |
| Home cards | replaced by the Connection screen |
| Locale text | zero Verge product-name strings in all 13 languages |

Answers, for the record:

1. **Node selection does not exist.** One server per tier, so the ~4,900-line proxy
   tree went. Nothing was kept "in case".
2. **Logs was deleted rather than replaced.** A "Report a problem" affordance that
   gathers what support needs is still the better answer, and is now the *only* gap
   this section leaves behind — a student with a problem has the Account screen's
   device ID and nothing to send with it.
3. **Entitlements vs internals:** TUN-on, sysproxy, core choice, ports and DNS are
   internals with **no UI at all**. Locus decides them. Only language, theme,
   update-check and refresh became student-facing, inside Account.
4. **The home cards did not shrink — they were deleted.** Live speed, the traffic graph
   and the connection state live on Connection; the session totals live on Account.

What replaced the connective tissue: `components/connection/use-connection.ts` (the
tunnel state machine), `use-traffic-summary.ts` (one place the traffic numbers are
computed) and `tier-badge.tsx`. The backend was not touched — profiles, config
generation, `enhance`, the service/sidecar decision and `locus::tier` all still drive
`locus_connect` exactly as before.

**Still missing from this area:** there is no Locus logo asset in the repo.
`src-tauri/icons/*` is the Clash Verge Rev mark; `UI-AESTHETICS.md` §4 asks for a 48×48
`#2EA86A` shield. The activation screen shows the wordmark alone rather than
substituting the old logo, deliberately — drawing a brand mark needs a human decision.

### Fork versioning — RESOLVED (2026-09-26)

This was an open decision; it is settled.

**The client is 3.0.0**, its own Locus line, not Clash Verge's `2.5.5`. The
reasoning is worth keeping, because the trap was real:

```
2.5.5 vs 2.2.6 -> strictly newer? true     (publishing 2.5.5 moves the fleet up)
2.2.9 vs 2.5.5 -> strictly newer? false    (after which every 2.2.x is REFUSED)
```

The client reported the version it inherited from Verge while the hub served
2.2.x. Publishing the inherited number would have pushed the fleet above the hub's
numbering permanently, with no server-driven downgrade to recover from — a manual
reinstall per device would be the only fix.

**Three version sites must agree**:
`client/src-tauri/Cargo.toml`, `client/package.json`,
`client/src-tauri/tauri.conf.json`. That agreement is enforced by
`client/src-tauri/tests/version_consistency.rs`, which reads them from disk. Drift
is a correctness dependency of the updater, not hygiene: a build whose reported
version disagrees with its bundled one either refuses the update that would fix it
or silently declines every release, and neither says so.

**No root `VERSION` file.** The fork versions itself in its own manifests.

**Releases are built by CI** — see `../operate/RELEASING.md` and `../operate/UPDATE-SYSTEM.md`.
Tag → CI builds, signs and releases → the hub fetches → publishes.

### ~~`publish-update.sh` (repo root) has two real defects~~ — RESOLVED BY RETIREMENT (2026-09-23)

The script is now `legacy/publish-update.sh.broken`, with both defects documented
in its header. It was never live: nothing has been published through it, and
`update_config` was at rollout 0 with empty URLs.

1. **It writes no `sha256_<platform>` columns.** Verified: zero matches. It emits
   an `assets: {…}` object, which is not an `update_config` column.
2. **Its macOS URLs point at filenames CI does not produce.**
   `locus-macos-amd64` / `locus-macos-arm64` vs CI's `locus-darwin-amd64` /
   `locus-darwin-arm64` → 404.

`publish-release.sh` is the correct path and does verify served bytes.

### ~~Correct the `internal/updatecfg` doc comment~~ — DONE (2026-09-23)

It stated that `publish-update.sh` writes `update_linux`/`update_windows` while
`publish-release.sh` writes `download_*`, causing every platform to fetch the
Linux binary. **That was not accurate**: both scripts write `download_*`, and
`publish-update.sh` contains zero `update_linux`/`update_windows` occurrences.

Corrected in three places: the `internal/updatecfg` package comment, the
`updatecfg_test.go` header, and `FIXES.md` #31 (which carried the same claim).
`publish-update.sh` itself is retired to `legacy/publish-update.sh.broken` with
the correction in its header. The package remains a valid shared contract
definition — only the stated cause of the bug was wrong.

### Delete my backup files once the guard is confirmed good

```
/root/data.db.pre-updateconfig-fix-1789795235
/root/admin_console.pb.js.pre-guard-1789795273
/root/admin_console.pb.js.staged-pre-guard-1789795273
```

---

## Things I checked and deliberately left alone

Recording these so they are not re-investigated:

- **Theme visuals — the whole appearance is unverified by machine.** **Added
  2026-10-01.** The registry suite proves a theme's palette clears the contrast
  floors, that its mode matches its palette direction, that the structural
  relationships hold, and that a decoration preset cannot escape its scope. It
  proves **nothing** about whether any theme *looks* right. Specifically
  unverified: that `forest-glow`'s radial wash reads as intended on the four
  platforms; that the §5 cold-start transition (every theme starts as
  `default-dark`/`default-light`, then resolves) is imperceptible on a real
  machine; and that `midnight`'s pure-black surfaces separate adequately on a real
  OLED panel. These are eyes-only checks. Do not read a green suite as covering
  them — see `THEMES.md` §8 and §9.
- **`forest-glow` is only ever used against a dark palette, and nothing enforces
  that.** **Added 2026-10-01.** The preset is a translucent green wash designed
  against a dark background. A future light theme naming it would pass every
  existing guard, because the preset test checks the decoration's *shape* (no
  braces, no `url(`), not its contrast against the palette using it. See
  `THEMES.md` §10 item 3.
- **~~`eslint src/main.tsx` has an unused `recognised` binding~~ — FIXED
  2026-10-01, see `FIXES.md`.** Resolved by finishing the feature rather than
  suppressing the binding: the recognition result now reaches the activation
  screen. Two things were wrong, and only one of them was lint-visible —
  the state was never rendered, **and nothing re-read the status after a
  successful recognition**, so a returning student would have waited up to
  `ENTITLEMENT_POLL_MS` (five minutes) on a screen telling them their device was
  registered. The second was found by tracing what happens after
  `locus_recognise` returns, not by the linter.
- **`shadowsocks-eco` WARN lines** — `decrypt length failed` from AWS-range IPs.
  Internet scanners probing open ports; they cannot complete the AEAD handshake.
  Benign, and the volume is low.
- **A 502 in the Caddy log** — my own PocketBase restart during the hook deploy.
- **`/update.json`** — a stale placeholder written by `05-caddy.sh`. The updater
  reads `update_config`. Not a fault, but do not use it as a health check.
- **The truncated fingerprint** in the `live-data-do-not-delete` memory note — I
  flagged it but did not correct the memory. If you rely on that value, use the
  full 64-char SHA-256 from `docs/history/SESSION-GOTCHAS.md` §1.
- **Making it so a future deploy cannot advertise unresolvable update URLs** — I
  added a guard for the *record* (`releases.set` checks URLs and hashes are
  present and version-matched), but the hook cannot stat the filesystem
  (PocketBase exposes only `$os.getenv`), so it cannot confirm the files exist.
  Publishing still verifies served bytes (`publish-release.sh`); that is the real
  guarantee, and the guard is defence in depth for the console path.

---

## If you are the next agent here

1. **Read the "never met a database" section above first** — the term/renewal
   work is the largest unverified surface in the repo, and its detail is in
   `docs/business/redesign/implementation/05-not-yet-true.md`.
2. Read `docs/history/SESSION-GOTCHAS.md` before testing against the live hub — it will
   save you the two false diagnoses a previous session made.
3. Remember `setup.sh` deploys from `/root/server/`, not from the repo. Editing a
   file here changes nothing until the staging copy is updated. `server/scripts/hooks-sync.sh`
   does the copy and verifies it; to check drift by hand:

```bash
for f in activation admin_console admin_unbind code_lookup heartbeat release update; do
  a=$(md5sum /opt/pocketbase/pb_hooks/$f.pb.js 2>/dev/null | cut -d' ' -f1)
  b=$(md5sum /root/server/pb_hooks/$f.pb.js 2>/dev/null | cut -d' ' -f1)
  [ "$a" = "$b" ] && echo "$f in sync" || echo "$f DRIFT"
done
```

(`hiddify` is gone from this list: `hiddify.pb.js` was deleted outright.)
