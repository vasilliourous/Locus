# Egress readiness — why the button says "connecting"

```
audience:    builder
status:      live
authoritative-for: the readiness chain from probe to rendered phase, and how to localise a stall
verified-against: client/src-tauri/src/core/manager/probe.rs, client/src/components/connection/phase.ts
```

**What this is.** A field manual for one symptom: *the tunnel is carrying traffic
and the button still says "connecting".* It names every hop in the chain, the
value each hop produces, what each failure looks like, and the exact command that
localises the break. Read it before touching anything in the readiness path.

**What this is not.** It does not restate the diagnosis history — that is
[`FIXES.md`](FIXES.md) (the 2026-10-01 egress entries, the second of which corrects
the first). It does not restate the method — that is
[`DEBUGGING-METHOD.md`](DEBUGGING-METHOD.md). This document is the *map of this
specific chain*, which neither of those carries in one place.

---

## 1. The chain, in one picture

Read top to bottom. Every arrow is a place the chain can break, and the failure at
each hop is named beside it.

```text
  client/src/pages/connection.tsx
    renders the button label from `phase`
            ▲
  client/src/components/connection/phase.ts   phaseFromStatus(status)
    if !activated            -> 'unknown'      (activation gate owns the UI)
    if connected && !ready   -> 'connecting'   ◄── THE STALL LANDS HERE
    if ready                 -> 'connected'
    else                     -> 'disconnected'
            ▲
  client/src/services/locus.ts   locus_status()
    { activated, connected, ready, coreUp, trafficFlowing, subscription, … }
            ▲
  client/src-tauri/src/cmd/locus.rs  locus_status()
    ready:   readiness.is_ready()      // Readiness::Ready
    core_up: readiness.is_core_up()    // Ready | Connecting
            ▲
  core/manager/mod.rs  observe_readiness()
    outcome = probe::probe_core_api(running)          // control API /version
    egress  = observe_egress(outcome == Serving)      // through-tunnel check
    traffic = traffic_probe::outcome()                // /traffic stream, pushed
    decide(outcome, egress, traffic, latch_active)
            ▲
  core/manager/probe.rs  decide(outcome, egress, traffic, latch_active)
    (_, Ok)                    -> Ready   <- a measured round trip
    (_, _, Flowing)            -> Ready   <- bytes actually moving (see §6)
    (NotRunning | any, .., inactive latch) -> Stopped  -> 'disconnected'
    (otherwise)                -> Connecting  ◄── renders as 'connecting'
            ▲
  core/manager/probe.rs  probe_egress(core_serving)
    egress_attempt_once() -> for each name in tier::OUTBOUND_NAMES:
        delay_proxy_by_name(name, EGRESS_TEST_URL, 5)
        -> Ok(ProxyDelay { delay }) ; accept if delay_is_a_measurement(delay, 5)
    up to EGRESS_ATTEMPTS (2) inside EGRESS_DEADLINE (12 s)
            ▲
  mihomo  GET /proxies/<name>/delay?url=…&timeout=5
    -> Ok({"delay": N})   (also on failure, as N = 0 — see §4)

  core/manager/traffic_probe.rs   the OTHER proof, and the one that cannot miss
    writers: the tray rate task (macOS, when enabled) and this module's own
    websocket: mihomo /traffic, one sample a second, up/upTotal/down/downTotal
    ready route:  current sample has bytes  AND  a sample arrived recently
                  (never upTotal/downTotal — see §6)
```

**The load-bearing fact.** *The Core is not answering* and *the Core answered but
no packet got through* both render as **connecting** — and since 2026-10-02 they
are also the *same backend state* (`Readiness::Connecting`). The screen cannot
tell them apart, and neither can the client: they differ in remedy, not in what
the student should do next. Any diagnosis must therefore start from the **log
line**, the Core's own answer, and the `trafficFlowing` field — not from the
screen.

**The second load-bearing fact.** `ready` has exactly two sources, and both are
evidence that a byte moved: a delay test that **returned a measurement**, or the
Core **reporting bytes on its own stream**. Neither is the Core answering its
control API, neither is the readiness latch, and neither is a lifetime counter.
If a change makes `ready` derivable from anything cheaper, the button starts
lying in the "connected with no internet" direction this file has already paid
for twice.

---

## 2. Localise it in five commands

Run these in order. Stop at the first that disagrees with expectation; that is the
broken hop.

### Hop 1 — what does the backend actually report?

```sh
# In the running app's devtools console, or against the command surface:
locusStatus()
```

Read **`connected`**, **`ready`** and **`trafficFlowing`** separately. They are
different questions:

| `connected` | `ready` | `trafficFlowing` | Meaning |
|---|---|---|---|
| `true` | `true` | `true` | Working, and proven by the student's own bytes. |
| `true` | `true` | `false` | Working, proven by a delay test (machine idle). |
| `true` | `false` | `false` | **The stall.** Core is up; neither proof landed. |
| `false` | `false` | `false` | Core not up at all — a different bug. |

If `connected: true, ready: false`, the defect is downstream of the Core: either
the egress check or the traffic stream. Hop 2 covers the first, Hop 5 the second —
and **Hop 5 first** is usually the faster of the two, because one look at the
student's own traffic numbers answers it.

### Hop 2 — is the Core answering, and does the tunnel carry a packet?

The three observations behind `observe_readiness` are taken separately on purpose,
so ask them separately here.

```sh
# The Core's control API (probe_core_api):
curl -s http://127.0.0.1:<external-controller>/version
# -> 200 {"version":…}     the Core is up
# -> refused / timeout      ProbeOutcome::Unresponsive -> NotReady -> 'connecting'

# The through-tunnel check, exactly as the probe makes it (probe_egress):
curl -s -G "http://127.0.0.1:<external-controller>/group/Locus%20Auto/delay" \
     --data-urlencode "url=http://cp.cloudflare.com/generate_204" \
     --data-urlencode "timeout=5"
# -> 200 {"Locus":1234}     a member reported a round trip
# -> 200 {"Locus-UoT":5000} a member HIT ITS BUDGET (see §4 — this was the bug)
# -> 504 "get delay: all proxies timeout"   every member timed out
# -> 200 {}                 no member reported at all
```

`<external-controller>` is the address the app configured (see `clash-verge.yaml`
/ the run state). The group name must be **`Locus Auto`** — the *group*, not the
`Locus` proxy. See §5.

### Hop 3 — is the group the traffic is using the group being probed?

```sh
curl -s http://127.0.0.1:<external-controller>/proxies/Locus%20Auto
# -> {"now":"Locus-UoT", "all":["Locus-UoT","Locus"], …}
```

`now` is the member the group's `select` has chosen — and therefore the member the
delay test dials. Compare it against the proxy the student's traffic actually exits
through. **If they differ, the probe is testing a path the traffic is not using.**
For Strike, `tier.rs` deliberately lists `Locus-UoT` first precisely because a
`select` group defaults to its first member.

### Hop 4 — does the classifier accept what the engine said?

Take the numeric value from Hop 2 and apply the rule by hand:

```
delay > 0 && delay <= 100_000      (probe.rs::delay_is_a_measurement)
```

- `0` -> not egress. The test genuinely failed.
- `1..=100_000` -> **egress**. This includes `5000`, a member that hit its budget.
- `> 100_000` -> error sentinel, not egress.

If Hop 2 returned `5000` (or any value at/above the budget) and `ready` is `false`,
you are looking at the §4 defect class — check the classifier first, before the
network. But check Hop 5 first of all: if the student's own traffic is moving, none
of the above matters.

### Hop 5 — is the Core reporting bytes moving?

The second proof (§6), and the one to check first when the student can see traffic
but the button disagrees. It is a single field:

```sh
# In the devtools console:
(await locusStatus()).trafficFlowing
# -> true   bytes are moving right now; `ready` must be true
# -> false  no bytes, or no recent sample — the delay test is the only proof left
```

It is fed by mihomo's own `/traffic` stream, which is the same socket the traffic
graph on screen reads. So when the graph's numbers move and this is `false`, the
stream is not reaching the sink — check the log for the stream connecting, and on
macOS check whether the tray rate task has taken the subscription instead
(`enable_tray_speed`). Both writers publish into the same place, so exactly one of
them should be connected, never both and never neither.

Deliberately **not** part of this check: `upTotal` / `downTotal`. Those are
lifetime totals and reading them as "connected" reinstates the latch bug this file
exists to prevent. See §6.

---

## 3. Which hop produces which screen

Use this to map a live symptom onto a hop without a debugger.

| Symptom | Most likely hop | Check |
|---|---|---|
| Stuck connecting **forever**, traffic flowing | the traffic proof is not reaching `decide` | **Hop 5 first**, then 3, 4 |
| Stuck connecting, **no** traffic | Core control API or the tunnel itself | Hop 2 |
| Connecting then settling to **disconnected** | `ProbeOutcome::Unresponsive` with a dead latch | Hop 2 |
| Reverts to connecting **intermittently** after working | one transient miss; retries exhausted inside the deadline | Hops 2, 4 |

**Hop 5 first, when traffic is flowing.** If the bars move and `whatsmyip` shows
the exit node, the tunnel works, and since 2026-10-02 that is *itself* a proof the
readiness rule accepts — so a stall with traffic flowing means the proof is not
arriving, not that it is missing. That is §6, and it is one field to check.

If `trafficFlowing` is `true` and the button is still on "connecting", the break is
between that field and `decide` — a code fault, not a network one. If it is `false`
while the student's panel moves, the stream is not reaching the sink.

**Then Hop 3**, which is the pre-2026-10-02 diagnosis and still applies when
nothing is moving: the question is not "is it up" but "which outbound did the probe
ask about, and what did that one answer". Confirm the selected member and ask the
probe's own question by hand:

```sh
# which member the group is currently using (may not be the one carrying traffic)
curl -s "http://127.0.0.1:<controller>/proxies/Locus%20Auto" | jq .now

# the probe's question, per outbound — note 200-with-delay:0 means "member failed"
curl -s "http://127.0.0.1:<controller>/proxies/Locus/delay?url=<enc(EGRESS_TEST_URL)>&timeout=5"
curl -s "http://127.0.0.1:<controller>/proxies/Locus-UoT/delay?url=<enc(EGRESS_TEST_URL)>&timeout=5"
```

A `200` carrying a usable `delay` on **either** outbound means the probe should be
reporting ready; if it is not, the fault is in the client's classification, not the
tunnel. If both answer `delay: 0`, no outbound can carry a packet and *connecting* is
the honest answer.

---

## 4. The classifier trap (the 2026-10-01 recurrence)

This is the defect class this document exists to prevent a third time.

**The mistake.** Deriving the *acceptance window* from the *timeout budget*:

```rust
// WRONG: a member that hits its budget is classified as a non-measurement
let timeout_ms = timeout_secs.saturating_mul(1000);
delay > 0 && delay < timeout_ms && delay <= IMPLAUSIBLE_DELAY
```

The engine is given the same number and reports it back when a member runs out of
time. So the rule rejects its own answer — and **raising the budget moves the
rejection threshold rather than widening tolerance**, making the check *more*
likely to fail on exactly the slow links it was meant to tolerate.

**The rule to keep.** Only two values are not measurements:

- `0` — the test failed;
- `> 100_000` — an error sentinel.

Everything else positive is egress. "Slow" is not "dead".

**Why it can look like a deliberate decision.** The rule was introduced to *agree*
with the frontend's `classifyDelay`, and a test was written asserting it. Both were
wrong together: `classifyDelay` answers *how fast is this node?* for a latency list,
where `'timeout'` is the right badge. The probe answers *did a packet move at all?*.
**They must agree on the sentinel set, not on what a slow node means.** If you find
yourself making the two identical, stop — that is the bug being reintroduced.

---

## 5. Facts that are easy to get wrong

- **Probe each OUTBOUND, not the group.** Since 2026-10-02 the probe asks
  `/proxies/<name>/delay` for every name in `tier::OUTBOUND_NAMES`, and never the
  group. The group route tests only the group's *currently selected* member, and
  the tier's group is a `select` whose default is its **first** member — on a UDP
  tier that is `Locus-UoT`, a different outbound from the `Locus` proxy ordinary
  TCP traffic uses. Asking the group therefore lets the app decide the tunnel is
  dead by consulting an outbound the student's traffic never touches.
- **The name is `Locus Auto` (the group); `Locus` and `Locus-UoT` are the proxies
  inside it.** mihomo refuses a group whose name matches a member (see
  `client/AGENTS.md`), so the group and its members can never share a name. If you
  add a third outbound, add it to `OUTBOUND_NAMES` **and** to the group —
  `every_outbound_the_probe_asks_about_is_one_the_profile_defines` pins the pair.
- **The probe tests general egress, not the hub.** `EGRESS_TEST_URL` is a
  Cloudflare 204 on purpose: a tunnel that reaches only the Locus hub has not
  proven it can carry the student's traffic.
- **The group route and the member route fail *differently*, and the difference is
  the fix.** `delay_group` raises any non-2xx as an `Err` (`ret_failed_resp!` in
  the plugin), so "every member failed" reaches the caller as an indistinguishable
  transport error with no delay map to inspect. `delay_proxy_by_name` instead maps
  a non-2xx to `Ok(ProxyDelay { delay: 0 })` — the zero sentinel — so a dead member
  is an ordinary `Ok(0)` the classifier already rejects. That is what lets the loop
  tell "this member failed" from "the tunnel is down", and therefore try the next
  member instead of giving up on the tier. Verified in the plugin source
  (`mihomo.rs`) and against the real sidecar; pinned by
  `a_dead_member_answers_with_a_non_measurement_not_an_error`.
- **Bytes moving is a proof, a lifetime total is not.** `trafficFlowing` comes from
  the *current* `/traffic` sample's `up`/`down`. `upTotal`/`downTotal` are a Core
  lifetime counter and are never evidence — reading them as connected is the latch
  bug with a number in front of it. See §6.
- **Two proofs, either sufficient.** A delay-test measurement **or** observed
  traffic means Ready. A change that requires *both* re-strands a working tunnel
  whenever the machine is idle (no traffic) or the delay test misses; a change that
  accepts anything else (the Core answering, the latch, a total) makes the button
  lie. `observed_traffic_alone_is_ready` and
  `a_serving_core_carrying_nothing_is_not_connected` pin the two directions.
- **The egress result is cached for 1.5 s.** `observe_egress` serialises the check
  behind a lock and reuses the result, so the 750 ms status poll and the 250 ms
  connect loop collapse onto one round trip. A reading can therefore be up to
  1.5 s old; do not read a single poll as instantaneous truth.
- **`Connecting` and `Stopped` are the only non-Ready states, on purpose.** The
  removed `NoEgress`/`NotReady` split claimed to distinguish "the Core is up but
  your network is not" from "the Core is still starting". Nothing consumed the
  difference — `phaseFromStatus` rendered both as *connecting* — so carrying two
  states bought no behaviour and cost a defect that hid in the untested one for two
  releases. Do not reintroduce a state the UI cannot use; if a diagnosis genuinely
  needs distinguishing, log it.

---

## 6. The other proof: traffic the Core reports moving

**Added 2026-10-02, after the third "stuck on connecting while traffic flows"
report.** The delay test was the *only* route to `ready`, and the previous two
repairs to it shipped within days of each other with the symptom surviving both.
That is the signature of a wrong premise, not of a wrong constant: a delay test is
a **question we put to the Core**, and anything we ask can be answered "no" about a
tunnel that is carrying the student's traffic perfectly well.

So there is now a second, independent observation, and it is not a question
(`core/manager/traffic_probe.rs`):

- the Core pushes `/traffic` once a second — `{up, down, upTotal, downTotal}`;
- a sample with `up > 0 || down > 0`, **arriving recently**, is proof the tunnel
  carried a byte, with no request of ours in the loop;
- either proof is sufficient for `ready`.

### The trap this section exists to prevent

mihomo reports four numbers and **two of them are a latch with a counter in
front of it**:

| Field | Meaning | Safe as proof? |
|---|---|---|
| `up` / `down` | bytes **per second**, for this sample | **yes** — traffic now |
| `upTotal` / `downTotal` | lifetime totals since the Core started | **no** |

`upTotal > 0` is "the Core has moved a byte at some point" — including the first
handshake, and including traffic from *before* the student pressed Connect. Reading
it as "connected" reproduces the original latch bug exactly, with the false claim
wearing a counter instead of a bool. The probe therefore keeps **only** "did the
current sample carry bytes", and requires the sample to be recent, because a stream
that has gone quiet because the Core died and one that has gone quiet because the
student is reading a document are indistinguishable from the numbers alone.

Two guards pin this: `a_quiet_sample_after_traffic_stops_being_evidence` and
`traffic_older_than_the_window_is_not_active`. Both were **observed failing**
against a `return true`-on-total implementation before being kept.

### One stream, two readers

The tray rate display (macOS, `enable_tray_speed`) already subscribes to
`/traffic`. Rather than open a second websocket for the same numbers,
`traffic_probe` starts its own stream **only when the tray task is not already
running** (`Tray::speed_task_running`), and the tray task publishes into the same
sink (`publish_from_tray`). The stream starts lazily from `observe_readiness`, so a
machine that never reads readiness never opens the socket.

### What to check when the delay test says no

1. Look at the student's own traffic panel. If bytes are moving there and
   `ready: false`, this hop is broken — not the network.
2. Confirm the field: `trafficFlowing` should be `true` in the same status read.
3. If it is `false` while the panel moves, the stream is not being fed: check the
   log for the stream connecting, and whether the tray task (macOS) is holding the
   subscription instead.

---

## 7. What is still open here

Read [`STILL-OPEN.md`](STILL-OPEN.md) §"The readiness probes against a live Core"
for the current list. The load-bearing gaps, as of the last edit:

- **The engine test cannot exercise slow-but-working.** `tests/egress_probe_engine.rs`
  SKIPs without network and uses a `direct` outbound to a public host. The case that
  broke — a slow round trip through a *real* tunnel — has never been reproduced in
  the suite. **On the machine this document was last edited on, the positive test
  SKIPs**, so "it passes" there means only that it did not run the assertion.
- **The per-member fix is verified structurally, not end to end.** The
  group/member error asymmetry above is read from the plugin source and measured
  against a local sidecar; it has never been exercised through a real Locus tunnel
  on a school link. See `STILL-OPEN.md`.
- **No run on a real school network.** The budget is a judgement, not a measurement.

**If you change any constant in this chain** (`EGRESS_ATTEMPT_TIMEOUT`,
`EGRESS_DEADLINE`, `EGRESS_ATTEMPTS`, `EGRESS_RETRY_GAP`, `PROBE_TIMEOUT`,
`EGRESS_CACHE_TTL`), update this document in the same change — per
`docs/README.md` rule 3, a document that disagrees with the code is a bug.

---

## 8. Quick reference

| Thing | Value | Where |
|---|---|---|
| Outbounds probed, in order | `Locus`, `Locus-UoT` | `locus/tier.rs::OUTBOUND_NAMES` |
| Group (never probed directly) | `Locus Auto` | `locus/tier.rs::GROUP_NAME` |
| Egress URL | `http://cp.cloudflare.com/generate_204` | `core/manager/probe.rs::EGRESS_TEST_URL` |
| Per-attempt budget | 5 s | `EGRESS_ATTEMPT_TIMEOUT` |
| Whole-check deadline | 12 s | `EGRESS_DEADLINE` |
| Attempts | 2 | `EGRESS_ATTEMPTS` |
| Gap between attempts | 500 ms | `EGRESS_RETRY_GAP` |
| Control-API timeout | 400 ms | `PROBE_TIMEOUT` |
| Egress cache TTL | 1.5 s | `core/manager/mod.rs::EGRESS_CACHE_TTL` |
| Status poll while settling | 750 ms | `components/connection/use-connection.ts` |
| Status poll idle | 15 s | `components/connection/use-connection.ts` |
| Routes to `connected` | `egress == Ok` **or** `traffic == Flowing` | `probe.rs::decide` |
| Traffic activity window | 3 s | `traffic_probe::TRAFFIC_ACTIVE_WINDOW` |
| `/traffic` stream stale timeout | 5 s | `traffic_probe::TRAFFIC_STALE_TIMEOUT` |
| The only traffic value that counts | current sample's `up`/`down` | `traffic_probe::mark` |
