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

### The term model, renewal and device binding have never met a database

**Added 2026-09-29.** `9a91da1` + `6a422bc` added the term model, `codes.renew`,
`codes.rebind`, `codes.set-term`, one-code-per-device (a unique index plus a 409),
name search, the renewal worklist, and `device.get`/`devices.list`.

**Every one of those is unrun against PocketBase.** They parse, the guards are
green, the console builds, and the arithmetic was exercised directly — but no
hook has ever executed on the deployed build, and the schema changes have never
been applied to the live database. The full list of what is unverified, with the
reason each matters, is in
`docs/business/redesign/implementation/05-not-yet-true.md`.

The three to check first, in order:

1. **The unique index actually rejects a duplicate.** The whole
   one-code-per-device guarantee rests on it, and it has not been observed
   enforcing anything. (U4 in that file.)
2. **The migration preserves the paying codes.** `server/scripts/backfill-terms.py`
   has a `--dry-run` and **never writes `expires_at`**, but it has never run
   against the live hub, which holds real paid codes in active use — no count is
   recorded here on purpose (`operate/CLAIMS.md` §4). Read the diff before applying.
3. **A renewal reaching a running client.** The propagation is reasoned and
   unit-tested in pieces; the chain has never been run end to end.

Also unbuilt, deliberately: the **client-side fingerprint persistence**. The hub
now keys the entitlement by fingerprint, but the client still stores its
fingerprint only in its own config, so a reinstall presents a new identity and is
refused at 409. Deferred because the fix's core property ("survives a reinstall")
cannot be verified without real Windows and macOS hardware, and an unverified
storage change could make drift worse.

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

### The readiness probes have never been run against a live Core

**Added 2026-09-28 (second round); extended 2026-09-29 for the egress check.**

Readiness now asks **two** questions, and neither probe has talked to a real
mihomo. `probe_core_api` asks the Core's control API; `probe_egress` makes a real
request *through* the tunnel (`delay_group`).

`probe_core_api` is the fix for claiming "connected" with a Core that had not
started. Its *rule* is pinned by unit tests (only a serving Core is `Ready`; a
failed probe revokes the latch). `probe_egress` is the fix for the case
`probe_core_api` could not catch — **a Core that is up and answering locally on a
machine with no usable uplink**, where mihomo binds its control port and answers
`/version` whether or not a packet can leave. That is the school-wifi report. Both
are unit-tested as rules; the probes themselves have never talked to a real core:

- **Does `delay_group("Locus Auto", …)` return a usable delay through the tunnel?**
  It goes through the mihomo plugin, so it should exercise the same path the delay
  UI uses — but that is assumed, untested here. If the group name, the tier config
  or the plugin transport is wrong, every connect would stall into "no egress" and
  the button would say **connecting** forever. Fail-safe (it never says "connected"
  wrongly) but still wrong, and it would look like the bug we just fixed.
- **Is the 3 s egress budget right on a school network?** A real tunnel on a slow
  link (or one whose first request pays a cold start) could exceed it, falsely
  reporting no egress on a working tunnel. This is the most likely false negative —
  watch it on a genuinely slow link before trusting it.
- **Does `get_version()` actually answer over the configured transport?** It goes
  through the plugin, so it should use the same channel as every other Core call —
  but that assumption is untested here. If the address/secret/socket path is wrong
  in a way that only shows at runtime, the probe would report `Unresponsive` for a
  healthy Core, and the button would say **connecting** forever. That failure is
  fail-safe (it never says "connected" wrongly) but it is still wrong, and it would
  look exactly like the bug we just fixed.
- **Is 400 ms enough on a loaded machine?** Chosen short because the status command
  is polled at 750 ms while connecting. If a real Core on a slow school laptop takes
  longer to answer, every connect would stall into the "still starting" branch.
  (The egress check has its own, larger budget — 3 s — so the two do not share this
  risk.)
- **The no-wifi case end to end.** With the Core running, disconnect the network and
  confirm the button leaves `connected` and settles on **connecting** — the core
  answers `/version` but `probe_egress` fails — rather than hanging on a socket that
  never answers or falsely reporting connected.

Worth checking first: the `client/src-tauri/sidecar/verge-mihomo-*` binary is
present, so `cargo test --test tier_profile_engine` exercises a real core for the
*profile*, and could be extended to exercise this probe the same way.

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
