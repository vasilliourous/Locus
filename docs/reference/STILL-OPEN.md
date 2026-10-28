# Still open after this work

```
audience:    builder
status:      live
authoritative-for: what is unfinished, unverified, or deliberately deferred
verified-against: docs/state.toml, docs/operate/CLAIMS.md
```

What is not finished, and what a successor should know before picking
it up. **Grouped by what is blocking, not by when it was found**, and each entry
says what would close it.

**Read this as a status page.** A gap closes by being *run*, not by being
re-argued; the evidence for each is a world claim in
[`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5, never a sentence repeated here.

---

## The Free/Full tier merge — written, verified in the tree, never run on the hub

**Added 2026-10-05.** The paid ladder was merged from two tiers into one: Free
($0, 1 Mbps, 10 GB) and Full ($5, 100 Mbps unmetered + UDP). Stealth was retired;
the survivor keeps the on-box name `strike`. See
[`../business/04-tiers.md`](../business/04-tiers.md) §4.2.4 and §4.7.

**Verified in the tree.** `check-consistency.sh` §27 asserts the agreement across
`04-tc.sh` (applied class *and* reboot unit), `seed-pb.py` (the row *and* its
`udp_relay` gating), the console placeholder, `state.toml` and the `README.md`
prose, and each new assertion was **observed failing** against the defect it
catches (the 200 Mbit cap, the `stealth` seed row, the retired-tier placeholder,
the restated caps) before being kept.

**NOT verified here, and not verifiable from a checkout:**

- **That the hub has taken it.** A11. The tree being green says nothing about
  the box; `/root/server/` is a separate copy and `setup.sh` has to re-run.
- **That a paid code connects on the merged tier.** A12 — the merge's central
  claim. Structural argument only: the row name is unchanged, so a code that
  resolved yesterday resolves today; nobody has run one.
- **Whether any live code carries `tier: "stealth"`.** A10. This is a live-data
  question with a real consequence — if any do, the row must be re-pointed
  rather than deleted, or a paying student is stranded.
- **That the retired `stealth` service is actually stopped.** A11. `04-tc.sh`
  disables `tc-stealth-cap.service` best-effort, but
  `/etc/shadowsocks/stealth.json` and `shadowsocks-stealth.service` are
  *skip-if-exists* and survive a re-run by design — so 8444 can keep serving with
  a working password on a box that looks merged. That is an operator step
  ([`../business/14-risks.md`](../business/14-risks.md) §14.8).

### The free plan's UDP path — built to match the paid one, never run

**Added 2026-10-05.** UDP became a given on both plans: the free plan now has its
own sing-box UoT listener on 8447 (its own credentials, its own 1 Mbps `tc`
class) beside the paid one on 8446.

**Verified here:** the tree agrees with itself. §27(h) asserts that every row
advertises UDP, that each names its own `uot_port`, the free port's 1 Mbps class
and its reboot unit, and that the paid UDP port is *not* shaped — plus that the
firewall opens 8447. Each assertion was observed failing first.

**Verified on 2026-10-05 by the deploy** (`docs/operate/CLAIMS.md` §5, A13):

- Both listeners are `active` and `enabled` on TCP+UDP, both reachable from
  outside the school network, and both tier rows resolve their own `uot_port`
  (`eco`/`free` → 8447, `strike` → 8446). The free plan's UDP is shaped at 1 Mbps
  on `1:20`; the paid plan's is unshaped on 8446.

**NOT verified:**

- **That a client tunnels real game traffic at the shaped rate.** Nothing has
  connected a real machine and played. This is the remaining gap and it is a
  hardware observation, not a repo one.
- **That a free client's game traffic actually flows.** The client needs no
  change (it keys UoT off its own `uot_port`+`udp_relay`, never the tier name), so
  this is a hub-side claim — but "no client change was needed" is a structural
  argument about `contract.rs`, not an observation.
- **Whether 1 Mbps is playable.** The user's premise is that Roblox/Minecraft
  work badly at 1 Mbps and that this is acceptable as a demo. Nobody has measured
  it. If it is *worse* than "badly" — unplayable — then the free plan's games are
  still broken and §4.3.1's argument loses its empirical leg.

> **Two traps on an upgraded box, both now guarded.** The old single-listener
> unit bound :8446, the same port as its replacement; its retirement is best-effort
> and only runs on a re-run, so confirm with `systemctl is-active sing-box-uot`
> (expect: not-found) after any upgrade. Both defects that deploy exposed are the
> same class — *a step that reports success without doing the thing*: the units
> were enabled but never started (`systemctl enable` only takes effect at boot),
> and `04-tc.sh` warned "Filter not found" on a correctly shaped interface because
> it grepped for `sport` where `tc filter show` prints a hex u32 match.

**Two things this work deliberately did NOT change, so nobody reads them as
closed:**

- **`free_throttle_mbps` is still stored and never applied.** The client parses
  the throttle speed and persists it; nothing lowers a running Core's rate from
  it. The merge renamed the plan the throttle banner points at; it did not make
  the throttle act.
- **The free allowance is still counted client-side** and is still not
  console-editable. It moved 5 GB → 10 GB, which is a hook value, not a
  mechanism.

**What to do with a live hub:** run the read-only probe in
[`../business/15-continuity.md`](../business/15-continuity.md) §15.3.1, answer
A10 from the console, then re-run `setup.sh` and enable/disable the two
shadowsocks units by hand.

---

## Open, and blocked in this environment

### Three field reports from the 2026-10 investigation: what is fixed, and what is not

**Added 2026-10. Released in 3.3.1 on 2026-10-17** — building is not the same as
working, and the entries below are the difference. Three reports arrived together
and only one of them turned out to be a defect this repository could see. Recorded
here so the next person does not re-derive the split.

**Fixed and shipped in 3.3.1** (CI run 39076038836: `verify`, all four platform
builds and `release` green; 21 assets, `manifest.json` reads `3.3.1` with every
platform signed):

- *"Configuration validation is already running"* reported as a refusal. The
  connect loop re-entered the config-apply path per poll; every poll after the
  first met a guard and was shown to the student as a refused configuration. Now
  separated into apply-once and poll-readiness, with `is_refusal()` deciding what
  may be called a refusal. See `FIXES.md`.
- *"Failed to compensate proxy apply failure … lock preferences"*. On macOS the
  whole proxy configuration is one machine-wide lock taken with a **zero-second
  wait**, and this client has several legitimate writers. Two writers overlapping
  meant one reported a failure, then a *second* write to compensate met the same
  lock. The write now waits for the lock and retries (3 attempts, 250 ms apart),
  and only for that error — `commit`/`apply proxy configuration` failures still
  fail once, because repeating them cannot help.
- The macOS bundle's `com.apple.security.inherit` and its upstream app-group.
  Removed, guarded by `check-consistency.sh` §30.
- The free plan's verifier could not see the free plan at all: it checked the
  retired single UoT listener. It now asserts each plan's listener against that
  plan's own key and cap (§31), and `02-shadowsocks.sh` no longer rewrites
  `/root/.tier_passwords` when it is not deploying from those values.
- A stored hub override, delivered through the heartbeat, plus a Caddy alias
  block, so a fleet behind a filtered hostname can be repointed without a release
  (§32).

> **What 3.3.1 does NOT prove.** `Build (macos-intel)` succeeded, which means the
  bundle compiles, gets ad-hoc signed and packages — **not** that the reported
  Intel symptom is gone. Nobody has installed 3.3.1 on the Intel Mac that reported
  it. The same is true of the free-plan and N4L fixes: all three need a device or a
  box, and each is a separate claim. See the per-item entries below for the exact
  command that would close each one.

**NOT verified — each needs a machine or a box, and each is a different claim:**

- **That the Intel Mac's bundle fails to validate.** This is the hypothesis the
  entitlement fix rests on, and it has not been observed. Read on the machine:
  `codesign -dv --verbose=4 /Applications/Locus.app` and
  `spctl -a -vvv -t install /Applications/Locus.app`. If both report the same as
  on the silicon machine, the entitlement change is a *correctness* fix (it is
  still correct) and is **not** the cause of the Intel difference — look to
  `sysopt` timing next.
  **3.3.1 status: unrun.** The Intel build compiles and signs; nobody has installed
  it there. The most useful single artifact now is
  `installer-Locus_3.3.1_amd64.zip` from the v3.3.1 release, installed on that
  machine, plus the two commands above.
- **That the free plan's UoT listener is running on the hub, with credentials
  matching the seeded row.** `sing-box-uot-eco.service` must be `active` (not
  merely `enabled`), `/etc/sing-box/eco.json`'s password must equal `ECO_PASS`, and
  8447 must be shaped by `1:20`. `locus_status.udp` now reports what the *client*
  believes; it cannot report what the hub is doing.
  **Run `server/scripts/verify-ss2022.sh` on the box** — it now checks exactly
  this pairing, which it previously could not.
- **What the free-plan crash actually was.** No crash text was captured, and a
  fix for it has not been attempted. This is the single most valuable missing
  artifact: the macOS crash report or the log lines around the failure.
- **That N4L will classify the new hostname favourably.** See below.

### The hub hostname is blocked on a school network, and no client change can fix it

**Added 2026-10.** A school network filtered the hub's then-canonical duckdns
name as "unknown / uncategorised", blocking activation, heartbeat **and** the
update path — all three on one name, so the device could not fetch the build that
would relieve it.

This is a **world claim about a third party's classification**, not a code
property, and it must be treated as one: it can be true today and false after any
reclassification, and it can differ per school.

**Fixable here:** which hostname the client dials. `locus::contract::resolve_hub_url`
now honours a validated stored override (absolute https, no path), so a hub move is
one operator-set value plus a Caddy site block — no release.

**Not fixable here:** the categorisation itself. A dynamic-DNS subdomain
(`*.duckdns.org`) is structurally likely to be classified as uncategorised or
dynamic-DNS. A durable fix needs a hostname under a domain that is registered,
has a reputation, and can be recategorised on request — that is an operator
purchase, and it is **unverified** until a device on the school network has
actually resolved and reached it.

**Re-verify with:** a `curl` to `/api/release` from the school network. Success is
the only evidence that counts; a successful `curl` from anywhere else says nothing.

### Activation on a Windows machine with an old root store is fixed, not observed fixed

**Added 2026-10-18.** Activation failed on Windows, on every attempt, with
`invalid peer certificate: Expired` for a hub certificate valid until 2027. The
mechanism (a bare `reqwest` client trusting only the Windows store, whose chain
needs `ISRG Root X2`) and the fix (a shared client with a bundled-roots fallback)
are in `FIXES.md`.

**Verified:** the fallback verifies the real hub — `the_bundled_roots_verify_the_live_hub`
fetches it end to end — and the platform path verifies it from a machine with a
current store.

**NOT verified:** that a Windows machine with the missing root now activates. No
such machine was available, so the causal chain is a structural argument. What
would settle it: on the failing machine, activate again and check the log for
`the platform certificate store rejected the hub; using the bundled roots` — the
fallback saying it engaged. Confirming the root is genuinely absent there is
`Get-ChildItem Cert:\LocalMachine\Root | ? { $_.Subject -match 'ISRG Root X2' }`
returning nothing.

### The domain moved on 2026-10-18 — and N4L's classification is still unverified

**Added 2026-10-18.** The hub moved to a name under a domain the operator owns;
`contract::HUB_URL`, the script defaults, the tier rows and `state.toml` all name
it. The old name is **still served** from the same Caddy site block
(`LEGACY_DOMAIN`), because every pre-3.3.2 build has it compiled in — see
`../operate/CLAIMS.md` A16/A17 and `contract::LEGACY_HUB_URL`.

**Verified here (the tree and the box, by running things):**

- Both names serve `/api/health` 200 and present certificates valid to
  **2027-01-15/16**; the new name's certificate was issued during the change.
  Recorded as A16, dated.
- A throwaway `free` code activated through the **public** `/api/activate` on the
  new name and returned `server_config.server` = the new name on 8443 with
  `udp_relay: true`; the heartbeat returned the same plus a self-consistent
  `update_*` block. The code was deleted afterwards (one code, never a bulk
  delete, never a live student's).
- All four tunnel ports (8443/8445/8446/8447) accept connections on the new name.

**NOT verified, and this is the part that matters:**

- **That N4L now permits it.** The whole point of the move is a classification
  this repository cannot observe. Nothing here has been tested from a school
  network, and a `curl` from anywhere else is not evidence. The choice of a name
  under a registered domain is a *structural argument*, not a measurement.
- **That a pre-3.3.2 client survives.** It should, because the old name still
  answers — but no device on an older build has been driven through activation
  since the change, and the failure mode if the legacy block is ever dropped is
  permanent (the device cannot reach the update that would fix it).
- **Which name the 3.3.1 fleet is actually failing on.** The reported error said
  "invalid peer certificate: Expired" for `networkingguides.duckdns.org`, whose
  origin certificate was valid throughout (issued Oct 17). The mismatch was never
  explained — the only hop not observable from either end is the network between
  the student and the box. Moving the canonical name may route around it without
  anyone having established the cause.

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
  bundle, and every site that names it agrees (CI's manifest, `fetch-release.py`
  and `publish-release.sh` `PLATFORMS`), enforced by `check-consistency.sh` §24 —
  each assertion observed failing against the defect first.
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

**Verified:** the client-side rule (window rollover, the 80% line, the
at-allowance boundary, saturation, skew repair, the clamp, "no allowance never
throttles") has tests, and the wire shape is tolerant on both sides — a malformed
advisory field does not fail the beat. Every §23 assertion was observed failing
against its defect first. Run the suites; do not read counts here.

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

**Verified in the build:** the signing step verifies and fails the build rather
than warning; the repack re-mounts the image and re-verifies the bundled `.app`;
the zip extracts to a bundle that still verifies as signed; and the zip can never
enter the update path (§1c, observed failing against the defect).

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

**Verified:** the rule has tests (traffic alone is ready; traffic that stops ends
readiness; a serving Core carrying nothing is not connected), each observed failing
against the code it catches. The wiring was read, not run:
`observe_readiness` → `traffic_probe::ensure_stream` → `outcome()` → `decide`.

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

**Reproduced, not theorised.** With an assertion changed to an impossible value
(`last.0 == 59999`), the test still reports `ok` — a green tick guarding nothing.
The case that matters, a **completed** round trip through the engine, has never
executed, and the same is true of every CI runner without egress. The four cases that do run here all assert
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
- **No egress on this runner**, so the positive case is still unrun here. Whichever
  machine closes "the per-member egress probe" must run it with the variable set.
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
- **Unknown, and worth naming:** how many Windows clients are on 3.2.17 — the hub
  stores no version per device, deliberately, so this is a world claim.
- **Not observed:** a 3.2.18 → 3.2.19 update. That is the first transition the
  fixed guard makes possible, and it will not happen until 3.2.19 exists.
- **Reconstructed, not measured:** the raw `locus-windows-amd64.exe` is no longer
  published, so the fixture proving the post-fix constant still refuses it carries
  the *documented* `NullsoftInstaller` offset rather than a measured one.

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
  system proxy. Reaching it needs a displaced or transport-lost Service owner on a
  machine with the system proxy enabled and the tray app running.
- **Not observed:** that the macOS arm really was unreachable — the argument is
  that `clear_active_service_session()` runs three lines above the clear. The
  failure is logged rather than silent, so a log from a real macOS recovery would
  settle it; nobody has looked at one.

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
(`theme_mode`, 3 options) and Theme (`theme_id`, one option per entry in the
registry — nine today; `check-consistency.sh` §9 recomputes the id list). When a
theme is set, the first shows a value the app is not honouring. `THEMES.md` §7 records this as
deliberately deferred rather than overlooked — merging them is a UI change with
its own migration question (what does an existing "Dark" choice become?) and is
not needed for themes to work. **The point where this becomes confusing to a
student is the point to do it,** and that point is probably the same change as the
entry above.

---

## Open, and blocked in this environment (continued)

### RESOLVED — the publish guard called `atob`, which goja does not have

**Added 2026-09-30. Resolved: the fix is committed; only the live restart is
left.** Kept in place, not moved to `## Closed`, because the *hub* still needs
PocketBase restarted for it to take effect and that has not been done (a repo fix
is inert until the piece that runs it is redeployed — see `AGENTS.md`).

**State in the tree.** `server/pb_hooks/admin_console.pb.js` carries a hand-written
`base64ToBinary()` and uses it in `isPlausibleMinisignSignature()`; there is no
`atob`/`btoa` call left anywhere under `server/pb_hooks/` (checked with
`grep -rn 'atob\|btoa'`). Decoding a real four-line `.sig` file from
`release-artifacts/` round-trips byte-for-byte to four lines with the expected
`untrusted comment: ` / `trusted comment: ` headers.

**Guarded by** `server/scripts/smoke-signature-encoding.sh`, which extracts the
predicate from the shipped hook rather than trusting a pattern — so the regression
this entry describes (a decoder that returns `false` for every input) fails a check
instead of quietly refusing every release.

**Still open:** the hub's copy has not been re-deployed and PocketBase has not been
restarted, so the live console's behaviour after this fix is **unverified**.

<details>
<summary>The original diagnosis, kept for the reasoning</summary>


**Added 2026-09-30.** The admin console refuses to publish or activate **any**
release, reporting for all four platforms *"signature not base64 of a minisign
.sig file"* — a message that wrongly blames the fetch. The full diagnosis is the
2026-09-30 entry in `FIXES.md`.

In one line: `isPlausibleMinisignSignature()` in
`server/pb_hooks/admin_console.pb.js` decodes with `atob`, and **PocketBase
0.22.21's goja defines no `atob`, `btoa`, `Buffer` or `TextDecoder`** — so the
call throws, the function's own `catch` returns `false`, and every value is
judged invalid, correct ones included.

**The fix was proven on the live hub, then reverted.** A pure-JS base64 decoder
decoded all four real signatures to minisign's 4-line shape. The version that was
reverted was never committed; it has since been re-applied and committed (see the
resolved note above).

To finish it on a hub: deploy the hook, then **restart PocketBase** before
testing — it caches `pb_hooks` at startup, and a stale-cached hook will make a
correct patch look like it did not apply.

Two things a successor should know:

1. `atob`/`btoa` should be checked across the whole hook tree
   (`grep -rn 'atob\|btoa' server/pb_hooks/`) — this class of bug is only visible
   at runtime, never at parse time, and the Rust/Python contract test cannot see it.
2. A guard that returns `false` for *every* input is indistinguishable from a
   total data outage in its output. It was not noticed because nobody published a
   release in the window.

</details>
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

### CI now runs the tests — and it has since been watched doing so

**Added 2026-09-29. RESOLVED — kept as the record.** `cargo test`,
`cargo clippy -D warnings`, `cargo fmt --check` and `pnpm test` run in the
`verify`/`build` jobs, and the workflow has been watched running on GitHub
(`CLAIMS.md` §5 **A4**). That A4 is a dated observation rather than a standing
property is the point of the register; re-check with
`gh run list --workflow=client.yml`.

> **Correction, 2026-10-29.** This entry said the job was "watched doing so", and
> that was true of an earlier workflow — but the job could not go green from
> 2026-10-26 onward: the §39 guard proof needs the mihomo sidecar and `verify`
> never fetched it, so every run failed at that step (the `v3.3.3` tag produced
> no release because of it). Fixed in 3.3.4 by fetching the build inputs in
> `Verify`. The lesson is the entry's own: a green check on a previous commit is
> evidence about that commit. See `FIXES.md`, 2026-10-29.

### No update has ever *successfully* been installed

**Open since 2026-09-29; the closest gap to closed.** Everything up to and
including signature verification is proven — the decode chain is reproduced
against the real crate behaviour and a real signature from `.locus-keys/`,
including the tamper case. What has never run is **the installer handoff**:
the plugin launches NSIS and the app exits, so "success" looks like the process
disappearing. That needs Windows or macOS hardware.

This is the single most valuable thing left to do, and the suggested first test
is a signed release published with `active = false`, flipped to `true`, watched
from one Linux client — the only platform a headless Linux box can verify.

Still stranded from this period: **any build compiled with the bare-key pubkey**
fails verification on every future update and needs a manual reinstall. Check what
is actually in the field before relying on an update to reach it.

## Closed — kept for the reasoning, not the state

Finished items, collected here so the live sections hold only live gaps. Nothing
here needs work.

- **Fork versioning (settled 2026-09-26).** The client's own line began at
  **3.0.0**, not Clash Verge's inherited `2.5.5`. Three version sites must agree at
  runtime (`version_consistency.rs`); `pnpm release-version` writes five. The list
  is `[client.version_sites]` in [`../state.toml`](../state.toml) — link it, never
  restate the count. **Bump with `pnpm release-version`, never by hand**
  ([`../operate/RELEASING.md`](../operate/RELEASING.md)).
- **The Verge UI and the `proxies` page (3.1.0).** Deleted, not rebranded: two
  tabs remain (Connection, Account). The backend was untouched — profiles,
  `enhance`, the service-vs-sidecar decision and `locus::tier` still drive
  `locus_connect`.
- **`publish-update.sh` and the `updatecfg` comment (2026-09-23).**
  `legacy/publish-update.sh.broken` was never live, and both writers of
  `update_config` write `download_*`. What made it worth recording is the rule:
  **there was no single definition of the `update_config` record**, so nothing
  could notice two scripts describing it differently. `internal/updatecfg` is that
  definition, and its three frozen field-name layers (record `download_*` →
  response `update_*` → client struct) are the contract.

---

### Delete my backup files once the guard is confirmed good

*Operator housekeeping left on the host, not a code item — recorded so it is not
mistaken for one.*

```
/root/data.db.pre-updateconfig-fix-1789795235
/root/admin_console.pb.js.pre-guard-1789795273
/root/admin_console.pb.js.staged-pre-guard-1789795273
```

---

## Things I checked and deliberately left alone

Recording these so they are not re-investigated:

- **Theme visuals are unverified by machine.** The registry suite proves contrast
  floors, mode/palette direction and preset scope; it proves **nothing** about
  whether a theme *looks* right — `forest-glow`'s wash, the cold-start
  transition, `midnight` on an OLED panel. Eyes-only; a green suite does not
  cover it. See [`THEMES.md`](THEMES.md) §8 and §9.
- **`forest-glow` is only ever used against a dark palette, and nothing enforces
  it.** A future light theme naming it would pass every guard, because the preset
  test checks the decoration's *shape*, not its contrast against the palette using
  it. See [`THEMES.md`](THEMES.md) §10 item 3.
- **The lint-suppressed `recognised` binding** (fixed 2026-10-01). The right fix
  was to finish the feature: the state was never rendered, **and** nothing re-read
  the status after a successful recognition, so a returning student waited up to
  `ENTITLEMENT_POLL_MS` on a screen telling them their device was registered. The
  second half was not lint-visible — trace what happens after `locus_recognise`
  returns, do not trust the linter to find it.
- **`shadowsocks-eco` WARN lines** — `decrypt length failed` from AWS-range IPs.
  Internet scanners probing open ports; they cannot complete the AEAD handshake.
  Benign, and the volume is low.
- **A 502 in the Caddy log** — my own PocketBase restart during the hook deploy.
- **`/update.json`** — a stale placeholder written by `05-caddy.sh`. The updater
  reads `update_config`. Not a fault, but do not use it as a health check.
- **The truncated fingerprint** in the `live-data-do-not-delete` memory note — I
  flagged it but did not correct the memory. A device fingerprint is always a
  full 64-character SHA-256; if a note shows 16, it is a prefix, and testing with
  it makes `/api/code-lookup` answer `bound_other` and `/api/activate` 403 for the
  device that actually activated. Print the stored value before theorising.
- **Making it so a future deploy cannot advertise unresolvable update URLs** — I
  added a guard for the *record* (`releases.set` checks URLs and hashes are
  present and version-matched), but the hook cannot stat the filesystem
  (PocketBase exposes only `$os.getenv`), so it cannot confirm the files exist.
  Publishing still verifies served bytes (`publish-release.sh`); that is the real
  guarantee, and the guard is defence in depth for the console path.

---

## The key recovery path is documented but has never been run

**Added 2026-10-05.** [`../operate/RELEASE-RECOVERY.md`](../operate/RELEASE-RECOVERY.md)
was written after the 3.3.0 rotation, and its commands are derived from
`secrets-decrypt.sh`'s interface and `client/docs/SIGNING.md` §6 — **not executed
against a real recovery**. Specifically unverified:

- That `LOCUS_UPDATE_KEY_B64` decodes to bytes `minisign` accepts. The encoding
  is base64 of the whole file *because* the raw form breaks `source`; the decode
  path has not been round-tripped since the runbook was written.
- That a key recovered this way, once set as `LOCUS_UPDATE_KEY`, produces a CI
  signature a client accepts. §2's round trip proves the private and public halves
  match; it cannot prove the pair is the one the fleet trusts.
- That a reinstalled 3.3.0 client then auto-updates. This is the same open item as
  "no update has ever successfully been installed" above, seen from the other
  side: the rotation gave that item a second population — every device still on
  the old key — and nothing has re-run it.

**What would close it:** decrypt the bundle on a throwaway host, decode the key,
run §2's sign/verify round trip, then set the secret on a test branch and read the
`build` job's output. No tag, no device, no fleet.

## The contract lock records names, not shapes — two gaps, both real

**Added 2026-10-26**, when [`CONTRACT.md`](CONTRACT.md) and its lock were created.
The lock guards the class of defect this project keeps re-earning (a wire name
that must agree across a boundary with nothing enforcing agreement), and its five
injected-defect tests are run by CI. Two adjacent gaps are **not** covered, and
saying so is part of the guard being honest about its scope.

**Gap 1 — a field's meaning can change while its name stays.** `uot_port` renamed
to something else fails; `uot_port` keeping its name and its meaning changing from
"port" to "port + 1" passes. Closing this means asserting values in fixtures, not
names in a lock, and there is no fixture for the hub's responses today.

**Gap 2 — the hub's actual response shape is not checked against the client's
structs.** The lock asserts `POST /api/heartbeat` exists on both sides. It does
**not** assert that what `heartbeat.pb.js` returns deserialises into
`HeartbeatResponse`. A field the hook stops sending is caught only if it also
disappears from `contract.rs` — and it will not, because the client is the half
nobody edits during a hub fix. This is the `enforcement_version` failure mode
(written by a hook against columns that did not exist) one layer out.

**Closing both contract gaps** needs the same artifact: a directory of recorded hub responses
captured from the live host or a replay fixture, deserialised into the client's
structs in a Rust integration test. That is a real piece of work, it needs the
live host or a captured transcript, and it is deliberately not started here.

## The build still needs the network

**Added 2026-10-26.** `server/data/sidecar.lock.json` pins the sha256 of every
downloaded build input, and `prebuild.mjs` verifies each one against it. That
closes the *silent wrong binary* path: a substituted or corrupted engine now
aborts the build instead of shipping.

It does **not** close the network dependency. The archives are still fetched from
GitHub on every build that does not already have them; pinning only decides
*which* bytes are acceptable, not whether they can be obtained offline.

Two things follow, and neither is fixed:

- **A build on a machine with no network, or on a day GitHub is unreachable,
  cannot proceed.** For CI this is normal and acceptable.
- **A pinned artifact can be deleted upstream**, which no lock can prevent — this
  is exactly what happened to the alpha engine (previous entry). For the stable
  engine the risk is lower (release assets are kept), but it is not zero.

**What would close it:** vendor the archives into the repository (or a release
attachment owned by this project) and have `prebuild.mjs` read them locally,
falling back to the network only when the vendored copy is absent. The cost is
repository size — the stable engine alone is ~59 MiB per platform, so a four-
platform vendoring is roughly 240 MiB. That is a deliberate trade nobody has
made, and it is **not** done here.


## The alpha engine: vendored, and the geo databases now pinned by asset ID

**Resolved on 2026-10-26 by vendoring.** The record below is kept because the
*failure mode* is the point, and it recurs for anything else published as a
rolling prerelease.

**What happened.** `tauri.conf.json` lists `sidecar/verge-mihomo-alpha` in
`externalBin`, so the client cannot compile without it. MetaCubeX publishes the
mihomo alpha as a **single rolling prerelease** and deletes the previous build,
so `mihomo-linux-amd64-v2-alpha-94b13ae.gz` returned **404** while the alpha it
replaced was unrecoverable from anywhere. `prebuild.mjs`'s error path then deleted
the only remaining copy. The build stopped working, permanently, for a file no
command could fetch.

**What was done.** The current alpha (`alpha-6780eee`, sha256
`8ef31967…`) is committed to the repository at
`client/src-tauri/sidecar/verge-mihomo-alpha-x86_64-unknown-linux-gnu`. Two
`.gitignore` files were changed to un-ignore that one path and nothing else;
`server/data/sidecar.lock.json` marks it `"vendored": true`; `prebuild.mjs`
verifies it against the pin and **does not fetch it**, including under `--force`.

**Two bugs this exposed, both fixed:**

- `prebuild.mjs` deleted the existing sidecar on *any* failure. That is what
  turned a transient 404 into permanent loss. It now leaves the previous binary
  in place and says so.
- `--force` produced a sidecar with the right bytes and mode `0644`, because the
  `.gz` path chmods inside a stream handler that can run after the rename. A
  sidecar that exists but cannot execute reports as "not found" all the way
  downstream. The mode is now set once, after extraction, for every shape.

**Resolved — the geo databases are now pinned by release-asset ID.**

They came from `meta-rules-dat/releases/download/latest`, and that repository
publishes **only** a moving `latest` release with no dated tag. Observed live on
2026-10-26: `Country.mmdb` changed between two runs minutes apart (7,646,989 →
7,649,827 bytes) and the pin caught it — but the lock could only report the
change, not fetch a chosen older set back.

The fix is a different immutable handle. A GitHub **release asset ID** names one
upload whose bytes never change, and is readable at
`GET /releases/assets/<id>` with `Accept: application/octet-stream`. All four
databases are now recorded by asset ID plus sha256 in `sidecar.lock.json`,
`prebuild.mjs` requests the ID rather than `latest`, and every download is
verified against the pin. Re-downloading restores identical bytes.

Cost: none in repository size. The assets stay where they are; only the
*address* changed from a moving pointer to a permanent one.

**Residual risk, stated:** an asset ID is only as durable as the repository
that hosts it. If MetaCubeX deletes the release, the ID 404s and the bytes are
gone — the same class as the alpha engine, minus the repo of last resort. That
is the argument for eventually mirroring the four files into this project's own
release assets, and it remains **not done**.

> **This risk was realised on 2026-10-28, two days after it was written.**
> MetaCubeX republished the rolling `latest` release (new release name
> `Release 2026-10-28 09:30`), which **deleted every asset ID the lock held**.
> All four returned 404, so `prebuild.mjs` died and both `Build (linux)` and
> `Build (windows)` failed on "Fetch sidecars and geo data" — the `v3.3.4` tag
> produced no release, for a reason that had nothing to do with the code.
>
> Repinned in 3.3.4. Three of the four files were re-uploaded byte-identical
> (their sha256 still matches the old pins); only `geosite.dat` genuinely
> changed, from 4,307,247 to 4,353,539 bytes. The pin did its job — it refused
> to build rather than silently ship different data — but it could not tell
> anyone *which* of those two situations it was in until the bytes were fetched
> and hashed.
>
> **What this changes.** "Pinned by asset ID" is durability, not immortality: it
> survives a rebuild only if the republished upload is byte-identical. Expect to
> repin whenever `resources_release` in `sidecar.lock.json` moves. **Mirroring the
> four files into this project's own release assets is now a defect, not an
> improvement** — it is the only fix that does not depend on another project's
> release hygiene.

## If you are the next agent here

1. **Read the "never met a database" section above first** — the term/renewal
   work is the largest unverified surface in the repo, and its detail is in
   `docs/business/redesign/implementation/05-not-yet-true.md`.
2. Before testing against the live hub, read this file's "deliberately left
   alone" section — it records the false diagnoses already paid for (a truncated
   fingerprint, a Luhn-invalid test code).
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

