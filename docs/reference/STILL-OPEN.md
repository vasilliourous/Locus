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

## `main`'s red `Build (linux)`: a harness race, fixed — one symptom still unproven

**Added 2026-11-08 from the failing run; diagnosed and fixed 2026-11-09.** The
claim is **A4** in [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5, last verified
for `v3.3.4` — read the register for what A4 is. This entry is the observation,
the measured cause, and what is still not proven.

> **`main` is green again (observed 2026-11-09).** The greeting fix landed in
> `4acac2a` and the scheduled run that followed is `success`. The header above
> says "red" because it names the run this entry was written from, not the state of
> the branch — read the run list, not this heading. What is *not* closed is the
> `502` and the UoT datagram timeout, both stated below.

Run `42437559393` (push of `4865855`) is `failure`. Not the macOS work: `Verify`,
`Build (macos-arm)`, `Build (macos-intel)` and `Build (windows)` all passed.
`Build (linux)` failed on one test:

```
test tcp_crosses_a_real_shadowsocks_2022_listener ... FAILED
assertion `left == right` failed: a request through the tunnel must reach the target
  left: "502"   right: "204"
```

**It was a test defect, and it was reproduced.** The harness waited for mihomo's
**controller** port and then connected to the **mixed** port without waiting at
all — two listeners mihomo opens ~76 ms apart, which its own debug log shows.
Waiting for the right port was still not enough: `TcpStream::connect(..).is_ok()`
is satisfied by the kernel accepting into a backlog, before the proxy serves, so
under `cargo test`'s default parallelism the request after it was refused or reset.
The harness now waits for a real SOCKS5 `[0x05, 0x00]` greeting.

**Measured, back to back on one machine:** the bare-connect wait failed **4 runs
in 25**; the greeting wait **0 in 25**. At ordinary load the old rate was 1–3 in
40. The change and its numbers are in the docblock of
`client/src-tauri/tests/tunnel_end_to_end.rs`.

**What is NOT proven, and is stated so nobody reads this as closed:**

- **The `502` reproduced on 2026-11-12**, on `main`'s run `43139506588` (push of
  `fdc06b1`, a test-only change) and then locally — about 1 run in 10 when two
  `cargo test` invocations overlap on one machine. **So it is not closed, and the
  greeting wait is not offered as its cure.** What changed is that it can now be
  diagnosed: the harness ran mihomo at `log-level: warning`, which emits
  *nothing*, so every earlier report attached an empty mihomo log and only the
  peer's log — and the peer, by construction, has nothing to say about a `502`.
  It now runs at `debug` and prints **both** logs
  (`src-tauri/tests/tunnel_end_to_end.rs`: `client_yaml`, the assertion message,
  and `Harness::client_log`).
- **What the first capture shows, and what it does not.** In the captured run
  mihomo logged its API bind, its mixed-port bind and `Auth success from …`, and
  then **no dial attempt at all**; the peer showed one inbound connection with no
  corresponding outbound. That is not "a dial failed", it is "no dial was
  logged". **It does not yet distinguish** a request that never reached mihomo's
  SOCKS handler from a dial that failed below the level a debug build logs —
  closing that needs `curl -v` in the failure message or mihomo's own `-d`
  debug flag, and neither has been added.
- **A `UDP-over-TCP` datagram still times out at the 10 s `recv_from`.** Seen in
  the same 2026-11-12 runs, alongside the `502`. It reproduced on the pre-fix
  harness too, so it predates the greeting change and is a **separate open
  item** — the one piece of this story still unexplained. The two co-occurring is
  a reason to suspect one cause for both.
- **The new wait is not proven under CI's conditions.** It has run locally only,
  and the 2026-11-12 local runs were *deliberately* loaded (two suites at once),
  which is not how CI runs it.

**What would close the rest.** A green `Build (linux)` on a run with the same
matrix shape, and a `502` capture that includes mihomo's log covering the moment
of the request — which the harness can now produce. If the datagram timeout
recurs, sing-box's log line `sp.v2.udp-over-tcp.arpa` is the first thing to read:
its absence means the negotiation never happened, its presence means the datagram
was lost downstream.

---

## CLOSED 2026-11-08 — the macOS runner. Both slices start and are publishable

**Added 2026-11-07; closed 2026-11-08.** Kept because the entry below it was
titled *"Blocking"* for a day and two documents then described a blocker that no
longer existed. Evidence is world claim **A22** in
[`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5 — do not restate the findings
here, they would drift. **What it means now:** v3.3.7 built and published on all
four platforms from `macos-15-intel` / `macos-15`, with both darwin binaries at
minOS 11.0.0. Nothing is blocking a release. Read the rest of this entry for the
diagnosis, not for a status.

**The cause was a retirement, not an account problem.** GitHub retired every
macOS 14 image from Actions on **2026-11-02** (`actions/runner-images` #13518,
#15226). The bare `macos-14` label resolved to a configuration that no longer
exists, which is why both jobs were cancelled with zero steps. The matrix now
names one image per architecture — `macos-15-intel` for the x64 slice and
`macos-15` for arm64 — because **a bare macOS label is the arm64 machine**.

**What the consequence WAS, while it was open.** Both macOS jobs were refused,
so `locus-darwin-amd64` and `locus-darwin-arm64` were never produced.
`server/scripts/fetch-release.py` requires all four payloads plus their `.sig`
files and **refuses a partial publish deliberately** — an update nobody can
install looks like it worked from the operator's seat — so the rule was: do not
tag until a dispatched run shows both macOS jobs running steps. **That rule has
now been satisfied once** (v3.3.7); it is still the right pre-flight for any tag,
because it costs one dispatch and the failure it prevents is silent.

**The two numbers that closed it**, kept because they are the ones to read again
if the matrix ever moves: the macOS jobs' **step counts** (non-zero means the
label is alive) and each darwin `.app`'s `LC_BUILD_VERSION` minOS, which must not
exceed the `minimumSystemVersion: "11.0"` the bundle declares — the macOS 26
image was avoided for exactly this reason, but a *newer* runner's SDK can still
lift the floor. v3.3.7 read minOS 11.0.0 / SDK 15.5.0 on both slices.

**Do not spend effort re-deriving this.** These have already been tried and are
recorded so they are not tried again:

- Asking GitHub support or Billing which macOS configurations are entitled. That
  was the recommendation for a day, and it was wrong — the images were retired.
- `macos-14-x64`. **It is not an image name.** The jobs sat *queued with no
  runner* — a worse failure than the rejection, because it burns the timeout
  instead of failing fast. Only `-intel` is a real x64 suffix.
- Attributing it to the build steps. A refused runner never runs a step, so no
  step's change can affect it. Read the job **annotations**, not the conclusion.

**What is true regardless of this closing.** The pipeline is green on the other
two platforms, a whole run is ~21 minutes rather than 39, a tag-only pre-flight
(`Probe the matrix runners against recent history` in `client.yml`) fails a tag in
seconds with the named cause, and `check-consistency.sh` §45 fails before that if
the two slices ever name mismatched images. **`macos-15-intel` is the last x86_64
image Actions will provide, available until August 2027** — after that
`locus-darwin-amd64` needs a decision, and nothing in the tree enforces the date.

---

## Budget, the third tier — written, verified in the tree, never run on the hub

**Added 2026-11-12.** A middle rung: Free ($0, 1 Mbps, 10 GB), **Budget**
($2, 25 Mbps, unmetered), Full ($5, 100 Mbps unmetered). Both of Budget's
listeners are capped server-side — TCP on 8448 (`1:40`) and its own
UDP-over-TCP listener on 8449 (`1:50`) — because the plan sells a *rate*, so an
unshaped UDP port would be a route around the thing being sold. That asymmetry
with Full's deliberately unshaped 8446 is intended and asserted (§27(j2)), not
an oversight. See [`../business/04-tiers.md`](../business/04-tiers.md) §4.2.5.

**"Stealth" did not come back.** That name was branding over the same
Shadowsocks transport, and the tier was merged away because it shared Full's
endpoint and differed only by a number. Nothing in this change adds a transport:
censorship resistance lives in the client's host/address substitution and the
maintenance tunnel, which are a separate, unverified body of work.

**Verified in the tree.** `check-consistency.sh` §27(j) asserts fifteen
agreements — applied cap, shaped UDP, both reboot units, the seed row with its
key and port, the UoT listener, both firewall rules, the console list, the
colour, the badge label, and the negative that Budget is not on the retired
8444 slot. Every one was **observed failing** against the defect it catches
before being kept, including two that had to be re-shaped: the `uot_port`
two-way conditional (which would have handed a third plan the free listener)
and a double-escaped `$` that made the listener check report BAD on a correct
tree. `verify-ss2022.sh` now verifies Budget's key and its shaped listener;
`smoke-test.sh` asserts both `tc` classes.

**NOT verified here, and not verifiable from a checkout:**

- **That the hub has taken it.** The tree being green says nothing about the
  box. The operator list is
  [`../business/18-open-items.md`](../business/18-open-items.md) P2.0.
- **That a Budget code activates and connects.** No code has been minted against
  `budget`, and no device has been handed a `server_config` on 8448.
- **That 25 Mbps is the rate a student actually sees.** `tc` is asserted to
  apply a `25mbit` class; that a transfer measured through the tunnel matches it
  has not been run. The class is the ceiling, and nothing here measures what
  throughput lands underneath it.
- **The cost of 25 Mbps unmetered at adoption.** Unmeasured; recorded as a risk
  ([`../business/14-risks.md`](../business/14-risks.md) §14.11), not a fact.

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
- ~~**That a free client's game traffic actually flows.**~~ **The mechanism is
  now proven; the rate is not.** `client/src-tauri/tests/tunnel_end_to_end.rs`
  (2026-10-29) runs a **real** `sing-box` Shadowsocks-2022 listener and the real
  mihomo sidecar against each other on loopback, sends a datagram through the
  `Locus-UoT` outbound, and gets it back — with the server logging
  `to sp.v2.udp-over-tcp.arpa:0`, which is the v2 revision `tier.rs` pins. So the
  negotiation, the framing and the cipher all agree across the boundary for the
  first time. This test was **observed failing** against the 3.3.x shape (the UoT
  outbound without `udp-over-tcp`) and against the pre-SS2022 cipher, so it is not
  a check that cannot fail.
  **Which sing-box, since 2026-11-08:** the peer is the version the hub installs
  (`SING_BOX_VERSION` in `server/modules/02-shadowsocks.sh`), read from that file
  by the workflow rather than restated in it — it used to say `1.14.4` against a
  hub on `v1.12.1`, and that drift is now `check-consistency.sh` §47. Read as a
  claim about *that binary*, not about every sing-box.
  **Still not proven:** that the *live hub's* listener is reachable, that the
  1 Mbps class shapes UDP as intended, or that anything is playable. Those need
  the box and a real network. And because `install_singbox` skips when
  `/usr/local/bin/sing-box` already exists, the running deployed peer is not
  guaranteed to be the pin.
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

### Four fixes from the 2026-11-06 field reports: proven in the tree, none observed on the machine that reported them

**Added 2026-11-06.** Four defects were fixed from five reports (see
[`FIXES.md`](FIXES.md) for each one's mechanism and guards). Every guard was
observed failing against its pre-fix code, so the *tree* claim is strong. The
**field** claim is not made: none of the four was reproduced, and three of them
can only be closed on a specific machine.

**Verified in the tree (Linux, unit tests only):**

- A damaged stored code is not offered as a credential, does not get adopted from
  the mirror, and cannot block the local half of a sign-out.
- `Failure::Dns` and `Failure::Refused` classify from `io::ErrorKind` and carry
  remedies; `Dns` is not retried over the tunnel.
- A missing privileged helper raises `SERVICE_HELPER_MISSING` naming the file,
  ahead of the elevation classifier.
- A deferred start after a restart is reported as a deferred start, not as a
  broken Core.
- `bash server/scripts/check-consistency.sh` exits 0; `cargo clippy-all` is clean;
  659 Rust and 110 frontend tests pass.

**NOT verified — each needs a machine, and each is a different claim:**

- **That the reported Windows machine lacked `VCRUNTIME140.dll`, and that the
  service installer was the binary that needed it.** This is the operator's
  report plus a loader-behaviour argument: a binary that cannot load cannot draw
  its own dialog, so Windows' loader drew it. **The PE import table of
  `clash-verge-service-install.exe` was never read** — the v2.7.4 release archive
  could not be downloaded from this checkout (refused by policy, not retried).
  Read it on a Windows host: `dumpbin /dependents clash-verge-service-install.exe`
  beside `VCRUNTIME140.dll`, or `strings` it for the DLL name. Until then the
  fix is correct regardless — it names whatever file or runtime is missing — but
  *which* prerequisite the report hit is unconfirmed.
- **That a student on that school network now sees the DNS sentence.** The fix
  guarantees a school-DNS failure is classified as `Dns` and carries a remedy;
  it does **not** make the hub reachable. Closing this needs one support log line
  from that network, or a status dump showing the new sentence.
- **That a damaged-code sign-out completes on macOS.** Reported on macOS arm,
  proven on Linux. The code is platform-independent; the *observation* is not.
- **That a deferred-start restart is what the `core did not become ready after
  restart` report actually was.** The mechanism is located and the branches are
  now reported apart, but nobody has reproduced the confused state. What would
  close it: a log line `the restart did not produce a usable core (mode after
  start: …)` from a machine that hit it, which the new `warn` now emits.

### The activation tunnel: both halves built, nothing run on a school network

**Updated 2026-11-06 — three of the four "built" claims below were not true of the
shipped code.** A school device never used the tunnel, and the reason was not that
the mechanism is wrong: the fallback did not fire on a `Refused`, the outbound
dialled the client's own loopback, and the bootstrap key was absent from every
release. All three are fixed and guarded; see §"the three defects" below and
[`ACTIVATION-TUNNEL.md`](ACTIVATION-TUNNEL.md) §6.1–6.1b. **A green tree said
"built" for all three, which is the lesson worth keeping** — "built" was true of
the intent and false of the artifact.

**Added 2026-10-29.** A student on a school network could not activate at all.
Measured from that network: 443 resets the TLS handshake after the ClientHello
(the SNI is read and the flow killed) and 8443, when spoken to in plaintext HTTP,
gets the filter's own `503 Application Control Violation` naming the block as a
**category** decision. Both blocks fire when a **hostname is visible**; a
Shadowsocks tunnel presents none, which is why the student's tunnel works and
enrolment does not. The mechanism and the traces are
[`ACTIVATION-TUNNEL.md`](ACTIVATION-TUNNEL.md).

**Built and guarded in the tree** (`server/modules/09-activation-tunnel.sh`,
opened by `08-firewall.sh`, guarded by `check-consistency.sh` §40): a Shadowsocks
listener on 8453 forwarding three enrolment endpoints through an allowlist and a
rate limit that mirrors Caddy's.

> **Updated 2026-11-07 — the hub half is now actually deployed, finally.**
> A fourth defect made the module impossible to run, and it is the worst of the
> set because it made the deploy *lie*: `09-activation-tunnel.sh` sourced
> `00-env.sh`, which ends in `exit 0`, and sourcing a file that calls `exit`
> terminates the caller. The module exited 0 having installed nothing, while
> `setup.sh` printed "✓ Module 09-activation-tunnel completed successfully". With
> the three earlier defects and this one, the tunnel had **four** independent
> reasons never to work (`FIXES.md`). All four are fixed and the module has now
> run on a hub: `:8453` accepts a connection from an uncensored network where the
> 2026-11-06 measurement timed out, `:18090` is loopback-only, both units are
> active, and the key the box runs hashes to the same sha256 as the bundle's.
> **This closes the hub-side blocker and nothing more** — the items below stand,
> and one measurement from the school network is still the only evidence that
> counts.

**NOT verified, and not verifiable from a checkout:**

- **That 8453 is reachable from the school network.** Nothing has run there since
  the port was opened. Note that an earlier timeout on this port was misread as
  censorship when the cause was `ufw default deny incoming` with no rule for it —
  the port was never open, so the network was never asked.
- **That the appliance tolerates an opaque stream on that port.** Reachability is
  not tolerance.
- **That a device could have used it at all, before 2026-11-06.** Three defects
  each independently made the tunnel inert: `Failure::Refused` did not trigger the
  fallback (and a filtering appliance answers either way depending on how far it
  gets); the generated outbound dialled `127.0.0.1`, i.e. the student's own
  machine; and `LOCUS_ACTIVATION_TUNNEL_KEY` was absent from every shipped build,
  with nothing in the artifact saying so. All three are fixed, each with a guard
  shown failing first. **Not verified on hardware** — see the re-verification
  lines above.

- ~~**That a real activation completes over it, because no client code uses the
  tunnel yet.**~~ **CLOSED 2026-11-01: the client half now exists.** The fallback
  lives in `locus/activation_tunnel.rs` and is wired at `activation.rs::post_json`
  and `heartbeat.rs::beat`. It fires on `ResetDuringHandshake`, `Refused`, and —
  since 2026-11-09 — `Dns`. See
  [`ACTIVATION-TUNNEL.md`](ACTIVATION-TUNNEL.md) §6.
- **That the tunnel now fires on the failure the school network actually
  produces.** Added and fixed in the tree on 2026-11-09, three defects, each
  independently sufficient and each guarded (`FIXES.md`): a reset arriving
  **after** the request was written classified as `Transport` and never reached
  the tunnel; `Dns` was excluded on reasoning that stopped being true the moment
  the outbound gained an address candidate; and every tunnelled client shared one
  rate-limit bucket because PocketBase is reached on loopback. **Structural, not
  observed** — no machine has run the result. The first two are proven by unit
  tests shown failing pre-fix; the third by `§40.8`.
- **That a deployed device activates over it end to end.** The mechanism is
  proven on loopback, against a stand-in for the hub's filter, by
  `client/src-tauri/tests/activation_tunnel_end_to_end.rs` — and that test was
  shown to fail against a broken outbound and against an open allowlist. What it
  does not cover is the deployed hub's own Python filter, a real machine on a
  school network, or the address candidate reaching a listener that answers.
- **That the three defects found on 2026-11-06 are gone ON HARDWARE.** Each is
  fixed with a guard that was shown failing, and each fix is *structural* — but
  no machine has run the result:

  | Defect | What it made impossible | Fixed by |
  |---|---|---|
  | `Refused` was not in `retryable_over_tunnel` | the fallback never fired when the filter answered with a refusal instead of a reset | `Failure::retryable_over_tunnel` now covers both; `a_refusal_must_reach_the_activation_tunnel` |
  | the SS outbound dialled `127.0.0.1` | the tunnel dialled the student's own machine, so it could only work against a local listener | `contract::ss_hub()`; `the_config_dials_the_hub_and_not_the_local_machine` |
  | the bootstrap key was absent from every shipped build | nothing could have used the tunnel however well it worked | `build.rs` re-emit + `rerun-if-env-changed`, a startup stamp, and a CI failure on a tag |

  **Re-verify with** a device on the school network, one support log: the startup
  line `activation tunnel: available …` (not `NOT COMPILED IN`), then a line
  naming the direct failure as `Refused` or `ResetDuringHandshake` followed by
  `retrying … through the activation tunnel`. Two lines, both required — the first
  proves the build can, the second proves it tried.
- **That `hub:8453` is even listening.** Measured from an uncensored network on
  2026-11-06, it **timed out** while 8443 connected in milliseconds. That is about
  the hub, not the school: either the module has never run there or `ufw` still
  drops the port, and both are one `setup.sh` re-run. Do not read it as a school
  finding.
- **That any of it reaches a deployed device** until `setup.sh` re-runs on the
  hub from `/root/server/`.

**A defect this work found, and what it means for whoever deploys it.** Building
the client half showed that the module's `local_addr`/`local_port` keys are inert:
`ssserver` has no inbound forwarding and dials whatever destination the client
names. The forward keys are removed and `check-consistency.sh` §40.6 now rejects
them. The consequence for the operator is that **the client must be built with a
`FILTER_PORT` that matches the hub's `ACTIVATION_FORWARD_PORT`** (default 18090);
§40.7 fails the build when they disagree. Establishing that took a running
`ssserver` and its log, not a reading of the config — a sentence in a comment
claimed the forward existed and nothing checked it.

**The durable answer is not code.** The filter blocked the hub by category, which
is a third party's policy decision; recategorisation is the only fix that does
not depend on the filter's behaviour, and it cannot be performed from here.

**Re-verify with:** `curl -v --max-time 10 http://<hub>:8453/` from the school
network, and again from an ordinary connection. **The two must agree.** As of the
2026-11-09 measurement from home they do not: an ordinary connection gets
`Established connection`, uploads the request, and then
`Recv failure: Connection reset by peer` — which is the client's
`ResetDuringHandshake`, i.e. the fallback failing exactly like the direct path.
Nothing that answers a reset is carrying an allowlisted request, so before asking
the school question, settle it on the box:
`systemctl status locus-activation-tunnel locus-activation-forwarder` and
`ss -lntp | grep 8453`. The detail, and why this is now the more important open
item, is [`ACTIVATION-TUNNEL.md`](ACTIVATION-TUNNEL.md) §6.6.

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
  (§32). Since 2026-11-07 the alias also **serves** `/updates/*` rather than
  redirecting it — without that it could reach the API and not download a
  release. Pinned by §32, which also forbids any *other* handle appearing there.

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

**Update 2026-11-07 — the alias was half a fix, and the missing half is now in
code.** A Caddy alias that `308`'d everything to the canonical name made the API
reachable from a network that blocks the canonical name, but sent the *download*
to that same blocked name. A device could therefore activate and never update —
and updating is its only route out. The alias block in `modules/05-caddy.sh` now
serves `/updates/*` directly and redirects the rest; `FIXES.md` carries the entry
and the four mutations its guard is proven against. **Still not done:** no
`setup.sh` run has written that Caddyfile to a hub, and no artifact has been
fetched from the alias from any network. The assertion is `200`, not `308`, and it
is unrun.

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

### The free tier's throttle: the hub sends it, and no mechanism can apply it

**Added 2026-10-04; measured 2026-11-01.** This is the largest remaining gap in
the free tier, and it is worth stating precisely because the field looks wired.

What exists: the hub sends `free_throttle_mbps`; the client parses it (tolerantly
— a malformed value does not fail the beat), persists it (`store::store_throttle_mbps`)
and can read it back (`store::throttle_mbps`). `check-consistency.sh` §23 asserts
that reader exists.

What does **not** exist: anything that lowers a running Core's speed from that
value.

**Measured 2026-11-01, and it changes the shape of this gap.** The obvious
implementation is a `rate-limit` on the generated mihomo profile, so that was
tried against the vendored engine rather than assumed. **The engine accepts both
field spellings and enforces neither.** Throughput for an 8 MiB download over a
real Shadowsocks-2022 peer on loopback:

| Config | Measured |
|---|---|
| no rate field at all (baseline) | 233 MB/s |
| `rate-limit: 1` on the `ss` outbound | 234 MB/s |
| `max-rate: 1000000` on the `ss` outbound | 379 MB/s |
| `rate-limit: 1000000` on an inbound listener | 355 MB/s |

So this is **not** a missing three-line config change — at the profile level the
capability does not exist, and adding the field would have shipped a no-op that
looks like a working limiter. (The same trap as the audit of this repo's own
`tc` verifier: a check that cannot fail reads exactly like one that passed.)

**What closing it actually requires:** a per-OS traffic shaper acting on the
running Core's traffic — `tc` on Linux, `pfctl`/`dnctl` on macOS, WFP or a
callout driver on Windows — which is privileged, platform-specific code this
client has none of today. It is deliberately not started: it cannot be verified
on any platform from this checkout, and a defect in it degrades the tunnel for
**paying** users, not just free ones.

**What was changed instead (2026-11-01).** The banner no longer claims a speed
change. It said *"slowed until your allowance resets"*, which was the one
statement on that screen the app did not honour; it now says the allowance is
spent, which is true. A test asserts the copy contains no "slowed" and no
"resets", and was observed failing against the old wording. `04-tiers.md` §4.4.5
now says the same thing in the same words — it previously described the throttle
as an applied behaviour, which was the document side of this same defect — and
the upgrade CTA it marked NOT BUILT is in `usage-bar.tsx`.

**No student is worse off.** The hub's `tc` cap applies to the free tier all
along, so 1 Mbps is the ceiling either way; what a spent allowance does *not*
do is lower it further.

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

**CLOSED 2026-10-29, and the mechanism was pointed at the wrong thing.**

- **CI now sets it.** The Linux `build` job runs `cargo test` with
  `LOCUS_REQUIRE_LIVE_ENGINE=1` *and* `LOCUS_REQUIRE_EGRESS=1`, and installs
  `sing-box` immediately beforehand. Nothing in the engine suites is allowed to
  skip on that path; a missing peer is a red build naming it.
- **The variable was gating the wrong requirement.** It made *every* skip a
  failure, but the skips were not alike: three of `tier_profile_engine.rs`'s
  tests need the engine and **no network at all** (`mihomo -t` parses a document),
  and four of `egress_probe_engine.rs`'s are loopback-only. All seven skipped on a
  missing sidecar, so on a machine that had simply not run `prebuild` the suite
  threw away the only part of it that always works — and reported `ok` for it.
  Requiring the engine is now the default; `LOCUS_REQUIRE_EGRESS` gates the one
  requirement that genuinely depends on the runner.
- **A conflicting assertion, found by setting the variable.** The positive-egress
  case reported `504 Timeout` against `cp.cloudflare.com` from the CI mode, and
  the first "fix" — skip — was unreachable, because `LOCUS_REQUIRE_LIVE_ENGINE`
  turns skips into panics. The case now retries a **second independent host**
  before failing, and its message names both. A single third party's availability
  was never what this assertion was about.

**Still open:**

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

**In CI the same test FAILS instead of skipping, and the cause is now MEASURED.**

> **2026-11-04. The earlier entries here blamed the runner. That was wrong.**
>
> The paragraph that used to stand here said the failure was "a torn runner, not a
> torn repository", and told the next person to re-run the job. It was written from
> a green run on 2026-10-29 and three red ones on 2026-11-02, and the cause was
> never measured. Five red runs and several wrong fixes later, the measurement:
>
> | measured on the runner, 2026-11-03 | result |
> |---|---|
> | `curl http://cp.cloudflare.com/generate_204` | `204` in **10 ms** |
> | `curl https://www.gstatic.com/generate_204` | `204` in **30 ms** |
> | `getent ahosts cp.cloudflare.com` | resolves |
> | the probe, through the engine | `504 Timeout` |
> | a plain proxied GET through the engine's mixed port | `204` |
>
> **The runner has egress, and the engine can dial from it.** The last two rows are
> the ones that localise the fault: the same engine, on the same runner, reaches the
> same host through its proxy port while the delay route returns `Timeout`. The
> probe's *delay path* is what fails, not the network.
>
> **What made this take so long, recorded because it will happen again.** A `504
> Timeout` is returned by mihomo both when a dial fails and when it merely exceeds
> `timeout`. The failure message named two hosts and no mechanism, so every
> diagnosis was a guess about third-party availability, and each guess "explained"
> the symptom equally well. `client/docs/UPSTREAM-CHANGES.md` had already written
> the trap down for a sibling test — *"the engine reports that as `Timeout`,
> indistinguishable from a dead tunnel"* — and it was not applied here.
>
> **What the test now does about it.** It carries its own control: before failing,
> it prints the HTTP code of a plain GET through the engine's mixed port, and the
> last 40 lines of the engine's debug log. The next red run says which side is
> wrong in the failure text itself instead of requiring a second investigation.
> `client.yml` also gained a `Diagnose the runner's own egress` step, which `curl`s
> both hosts directly when the suite fails.
>
> **Still open:** *why* the delay route times out when a proxied GET succeeds from
> the same engine. That is unexplained, and it is the remaining question — the next
> red run's log is what will answer it. What is no longer open is the question this
> section asked for two days: **it is not the runner.**

> **CLOSED 2026-11-04, from the next red run's log, and the answer was in the request.**
> The run this section was waiting for (`41666161485`) printed the control and the
> engine log: the proxied GET returned `204` and the delay route returned
> `504 {"message":"Timeout"}` — the same evidence as before, which localised the
> fault but did not name it. The naming came from reading the engine's handler
> rather than the symptom.
>
> **The cause: a unit error.** `egress_attempt_once` passed
> `EGRESS_ATTEMPT_TIMEOUT.as_secs()` to mihomo's delay route, which reads that field
> as **milliseconds** — and as a **signed 16-bit** integer. So every delay test ran
> with a **5 ms** budget and answered `504 Timeout` in about **7 ms**. The plugin
> maps that to `Ok(ProxyDelay { delay: 0 })`, which `delay_is_a_measurement`
> correctly rejects, so **every member failed on every platform from 2026-10-02 to
> 2026-11-04**. The probe could not report egress at all; `decide` reached `Ready`
> only through `traffic == Flowing`.
>
> **Why it took a month, recorded because it will happen again.** `Timeout` is the
> same string a dead tunnel produces, so the failure named no mechanism and every
> guess "explained" it; the *test* passed a literal `5` with a parameter called
> `timeout_secs`, so the test and the code were wrong in the same direction and
> only the engine disagreed; and the one assertion that could have failed needed
> egress, so it was read as an environment problem. The tell was in hand the whole
> time: **a `504` in 7 ms did not time out.** Widening the budget to 5000 ms makes
> the positive case pass immediately, against the real engine, on the shipped route.
>
> **Reproduced, not inferred.** Locally against `verge-mihomo v1.19.32`, through a
> real Shadowsocks-2022 outbound and a local 204 responder:
>
> | `timeout` | answer | elapsed |
> |---|---|---|
> | `5` (the defect) | `504 {"message":"Timeout"}` | 0.007 s |
> | `5000` | `200 {"delay":27}` | 0.029 s |
> | `32767` | `200` (`i16::MAX` parses) | — |
> | `32768` | `400 {"message":"Body invalid"}` | — |
>
> Four guards now hold it, each **observed failing** against the defect it catches:
> the value the call site reads, a compile-time range assertion, an egress-free
> engine test on the field's `i16` boundary, and the positive round-trip case
> itself. `EGRESS-READINESS.md` §4b is the manual; its copy-paste diagnostics said
> `timeout=5` and were corrected in the same change.

> **Still open from this thread, and it is small:** the shipped probe now *can*
> report egress, but no device on a working tier has been observed reaching
> `connected` **through the egress signal** rather than through traffic — the
> per-member probe's own entry below says why. That is a hardware observation, not
> a repo one, and this fix does not change it: it removes the reason the signal
> could never fire, it does not prove a real device fires it.

`LOCUS_REQUIRE_EGRESS=1` stays set. It turns the skip into a failure by design, and
it is what stops the suite passing by not running — the flag was never the problem.

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

### RESOLVED — nothing in the UI could clear a selected theme

**Added 2026-10-01; closed 2026-11-01.**

A theme carries its own light/dark mode and **supersedes `theme_mode` while one is
set** (`THEMES.md` §2). The dropdown on Account only ever *wrote* a theme id —
every option in it was a theme — and nothing cleared the field. So after a student
picked any theme, the Appearance control above it became inert: it still stored a
value, and the value was no longer what the app honoured. "Follow the system" was
reachable only by editing `verge.yaml` by hand.

**The shape chosen** was the explicit *Default (follow system)* entry, writing
`theme_id: ''`. The empty string was already a safe sentinel — `isThemeId('')` is
false, so no registry entry can shadow it, and `hasTheme` in
`use-custom-theme.ts` is `theme_id.length > 0`, so `''` makes the app honour
`theme_mode` again. Both properties are now asserted in
`client/tests/theme-colors.test.ts`, and the assertion was written because the
alternative — a sentinel that accidentally matched a real theme — would look
exactly like a theme bug.

That leaves the second entry below, which is unchanged: the two controls are
still separate, and now the second one can be *cleared* rather than only set.

### The theme dropdown and the `theme_mode` dropdown are separate and can disagree

**Added 2026-10-01; partly addressed 2026-11-01.** Same change.

`account.tsx` has two adjacent controls in Preferences: Appearance
(`theme_mode`, 3 options) and Theme (`theme_id`, one option per registry entry plus
*Default (follow system)*; `check-consistency.sh` §9 recomputes the id list). When
a theme is set, the first shows a value the app is not honouring.

**What improved:** the Theme control can now be *cleared*, so the state above is
recoverable by the student rather than only by hand-editing `verge.yaml` — see the
entry above.

**What remains:** the two controls are still separate, so while a theme is set the
Appearance select shows a value that has no effect. Merging them is a UI change
with its own migration question (what does an existing "Dark" choice become?) and
is not needed for themes to work. `THEMES.md` §7 records it as deliberately
deferred rather than overlooked. Nothing here is wrong or unrecoverable; it is two
controls where one would be tidier.

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

### RESOLVED — three documents described behaviour the code did not have

**Found and fixed 2026-11-01**, while refreshing the docs for the activation
tunnel and theme work. Recorded because none of these had a guard, and the class
is worth naming: **a sentence is the one artefact in this repository that nothing
recomputes.**

1. **`04-tiers.md` §4.4.5 described the spent-allowance slowdown as an applied
   behaviour** — "the speed the client drops to" — when the client only stores the
   value and the engine cannot enforce a rate at all. §4.4.4 also still marked the
   upgrade CTA `NOT BUILT` and described it as "one tap to Stealth", a tier that
   was retired in the merge. Both now match `usage-bar.tsx` and the measured
   finding.
2. **`heartbeat.pb.js` said "the client switches its tc-independent local cap to
   this"** — a hub comment asserting a client behaviour that does not exist. A
   comment is not guarded, so it survived reviews that checked the field's
   presence. It now says plainly that the value is sent and not honoured.
3. **`THEMES.md` §10 item 1 and `ACTIVATION-TUNNEL.md` §4 both described gaps
   that this change closed** — a dropdown with no way to clear a theme, and a
   tunnel the client "cannot use yet". The code moved; the prose did not.

**The rule, and it is the one `CLAIMS.md` already states:** when a change makes a
document false, the document is part of the change. Items 1 and 2 had drifted for
weeks because no guard reads prose for intent — `§28` checks that live documents
are *indexed*, not that they are *true*.

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

## The student sign-out: built and green in the tree, never run on the hub

**Added 2026-11-04.** "Sign out on this device" (Account page) forgets the key
locally and records the release on the hub. The local half is exercised by tests;
the hub half is **structural only**.

**Verified in the tree (and each guard observed failing):**

- The client sequence, including "the hub is told before the key is forgotten"
  and "the entitlement is cleared last" — `locus::signout`'s tests read the real
  `sign_out` body and compare call offsets (`FIXES.md`, 2026-11-04).
- That a signed-out device does not re-adopt its code, and that clearing only
  `verge.yaml` *would* be undone by the machine-scoped mirror — the
  counter-example test in `locus::store`.
- That the `signed-out` marker blocks a mirror rewrite, which is the in-flight-beat
  race `runtime::stop()` does not by itself close.
- The countdown reads 3, 2, 1, cancel stays available for the whole window, and a
  backwards clock cannot shorten it — `pages/sign-out-timer.test.ts`.
- `gen-ts-contract.py --check`, `contract-snapshot.py --check`,
  `check-consistency.sh` (all sections, including the new §41), `cargo test
  --lib` (640), `clippy -D warnings`, `vitest` (106), `tsc`, `vite build`.

**NOT verified anywhere:**

1. **That the hub hook works.** `device_release.pb.js` has never run. PocketBase
   hooks have no local harness in this repo, and goja's semantics differ enough
   (no `atob`, function declarations not hoisted across scopes) that "it is valid
   JavaScript" is not evidence. What would close it: `hooks-sync.sh`, then a
   throwaway code activated and signed out, checking that `released_at` and
   `release_count` moved and that `activated_at`/`expires_at` did **not**.
2. **That `released_at`/`release_count` exist on the live `codes` collection.**
   `seed-pb.py` merges missing columns additively, but it has to be re-run, and
   the same "the hook is inert until the box takes it" rule applies
   (`FIXES.md` header). A release that writes a column the schema lacks is
   **silently discarded** — this project has paid for that once already with
   `unbound_at`/`unbind_reason`.
3. **That Caddy serves the new route.** `05-caddy.sh`/`templates/Caddyfile` carry
   a dedicated `release` rate-limit zone, but a Caddy reload is needed, and
   `handle` order is load-bearing (the general `/api/*` block would otherwise
   swallow it into the 100-per-10s zone — which still proxies to PocketBase, so
   the endpoint would work while the limit was the wrong one).
4. **That sign-out is usable end to end on a real device** — tunnel down, profile
   removed, activation gate shown within the three-second window rather than after
   the five-minute poll.

> **Nothing here blocks shipping the client half.** A hub that has not taken the
> hook answers 404 to `/api/release-entitlement`, which the client reads as
> `notConfirmed` and reports as "signed out here; we could not tell Locus". That
> sentence is true, and the student's local sign-out is unaffected — which is why
> the failure mode was designed to be a true sentence rather than an error.

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

## The build no longer needs the network for binaries — but four database files still are not mirrored

**Added 2026-10-26; closed for the binaries on 2026-10-29.** The entry this
replaces said vendoring was "a deliberate trade nobody has made". It has now been
made: all eight sidecars are committed under `vendor/build-inputs/` and
`prebuild.mjs` copies them into place, falling back to the network only when a
copy is absent. [`operate/VENDORED-BUILD-INPUTS.md`](../operate/VENDORED-BUILD-INPUTS.md)
owns the layout and the procedure.

**Verified by running, not by reading.** With `HTTPS_PROXY` pointed at a dead
port and `src-tauri/sidecar/` emptied, `prebuild.mjs` reported *"is vendored in
the repository and matches its pin"* for both engines and downloaded neither.
The measurement mattered: the first attempt reported the same green line while
the log also said `download finished`, because `lockedSidecarVendored` consulted
a per-entry flag that only the alpha carried. **Only a dead proxy distinguishes
"read from disk" from "happened to get the same bytes."**

**What is still open, and is not closable from this checkout:**

- **The four geo databases are pinned but not mirrored.** All four asset IDs from
  the 2026-10-28 republish now return `404`, and `latest` serves different bytes
  (`Country.mmdb` hashes `9a485c40…` against a pin of `ccec4d6f…`). Committing
  wrong bytes under a correct pin is worse than downloading right ones, and
  repinning to `latest` would reintroduce the drift the asset-ID pin stops. The
  group is declared in the lock and empty on purpose, and `prebuild` warns on
  every build that it is fetching rather than lying about it.
  **[`operate/VENDORED-BUILD-INPUTS.md`](../operate/VENDORED-BUILD-INPUTS.md) §4
  names the host that can close it:** one holding the pre-republish bytes.
- **`geosite.dat` will need a decision, not just a copy.** The republish that
  touched it was a genuine routing-data update (4,307,247 → 4,353,539 bytes). The
  bytes that passed their pin and the newest bytes are different files, and
  choosing the second silently changes routing data nobody reviewed.
- **Repository size is now 464.7 MiB of committed binary**, measured, against a
  planning estimate of "roughly 240 MiB". The estimate was low by about half
  because it counted four copies of the stable engine and one alpha; the alpha is
  four copies at the same size again. It is real cost: a fresh
  clone and every CI checkout pay it. The alternative was a build that stops when
  a third party re-rolls a tag, which happened twice in three days.

## The alpha engine: vendored, and the geo databases now pinned by asset ID

> **Recurred on 2026-10-29, and the vendoring was too narrow to cover it.**
> MetaCubeX re-rolled `Prerelease-Alpha` to `alpha-ca8d7c4`, deleting every
> `alpha-6780eee` asset. Only **linux-x86_64** was vendored, so `Build (linux)`
> was the one platform that still compiled; macOS and Windows tried the dead URL
> and failed. Vendoring one platform is what turned a fleet-wide outage into a
> single-platform survival.
>
> Repinned to `alpha-ca8d7c4` in 3.3.4, and the lock now records the alpha's
> hash for **all four** shipped triples (`aarch64-apple-darwin`,
> `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`).
> Only linux is still *vendored*; the other three are pinned and downloaded,
> which is a checkable fetch rather than a reproducible one. `--check` reports
> the absent ones as `MISSING`, which `check-consistency.sh` §39 treats as a
> warning — a decoded pin for a platform this checkout never built — while a
> hash disagreement stays `BAD`.
>
> **Closed 2026-10-29: all four alpha binaries are now vendored.**
> `vendor/build-inputs/verge-mihomo-alpha/` holds each triple, `prebuild.mjs`
> copies the one this build needs, and `sidecar-lock.py --check` verifies the
> committed copy on a fresh clone. The duplicate under
> `client/src-tauri/sidecar/` was retired in the same change — two committed
> copies of a 62 MiB binary can disagree, and the build bundled whichever the
> copy step wrote last. Proven by emptying `sidecar/` and pointing `HTTPS_PROXY`
> at a dead port: both engines reported *"is vendored … and matches its pin"* and
> neither was downloaded. Layout and procedure:
> [`../operate/VENDORED-BUILD-INPUTS.md`](../operate/VENDORED-BUILD-INPUTS.md).

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

> **Realised a second time on 2026-11-02, and this one settles the question.**
> MetaCubeX rebuilt the release again (**2026-11-01 01:21**) and deleted all four
> IDs the 3.3.4 repin held. All four 404'd (403 on one runner), failing every
> `Build (platform)` job. Unlike the 2026-10-28 event, **the content moved too** —
> `Country.mmdb` +1290 bytes, `ASN.mmdb` +18994, `geosite.dat` +997, `geoip.dat`
> −20810 — so this was a repin of *different data*, not of identical bytes.
>
> **Two things follow, and the second is the one worth acting on.**
>
> 1. *A recurring failure needs a fix, not a fourth repin.* The pattern is now
>    twice in five days: a rolling `latest` release means every upstream rebuild
>    breaks the build. Mirroring the four files into this project's own release
>    assets is still **not done**, and it is the only option that does not depend
>    on someone else's release hygiene. Until it is done, a build failure in
>    "Fetch sidecars and geo data" should be read as *this*, not as a code defect.
>
>    **The repin to the 2026-11-01 IDs survived under an hour.** MetaCubeX
>    republished again at **2026-11-02 01:42** and the four IDs were 404 before
>    the next CI run finished — so the fifth repin (to the 2026-11-02 IDs, same
>    day) is equally temporary, and there is no longer any question of whether
>    repinning is sustainable. It is not. Two deletions in one hour is what
>    settles it: **the next person to see this failure should mirror, not repin.**
>
>    The 2026-11-02 republish was benign by comparison — three of the four files
>    came back byte-identical (`ASN.mmdb`, `Country.mmdb`, `geoip.dat`) and only
>    `geosite.dat` moved (4,353,536 → 4,356,006), which is an ordinary routing-data
>    update. That is luck, not a guarantee: the 2026-11-01 rebuild changed all
>    four. Treat any repin as a possible data change and verify it as one.
>
> **CLOSED 2026-11-03 — mirrored, not repinned.** The sixth repin was made on
> 2026-11-02 and did not survive the day either: `Build (macos-arm)` could not
> fetch `geosite.dat` at all (403 from the GitHub API) and `Build (linux)` passed
> "Fetch sidecars and geo data" only because that runner's cache still held the
> bytes. The four files are now committed in `vendor/build-inputs/resources/` and
> `prebuild` reads them from disk; the lock's pins are unchanged, so this was a
> mirror and not a repin. `VENDORED-BUILD-INPUTS.md` §4 owns the detail.
>
> **What is still not mirrored:** `clash-verge-service-ipc`, the archive the three
> service helpers are unpacked from. It is downloaded with no lock entry and no
> hash check, so it is both the one remaining upstream dependency *and* the one
> input that cannot be verified on arrival. Stable upstream is the only reason it
> has not bitten. It is the next one to pin.
> 2. *Verify the new bytes do the job, not just that they are new.* Because the
>    content changed, a hash check proves only that the download is self-consistent
>    with whatever was just written into the lock. The 2026-11-02 repin was
>    accepted only after the real vendored engine loaded all four and matched
>    traffic through them — not because the sizes looked plausible.

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

