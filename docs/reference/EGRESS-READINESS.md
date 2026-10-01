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
    { activated, connected, ready, coreUp, subscription, lastConfirmedAt }
            ▲
  client/src-tauri/src/cmd/locus.rs  locus_status()
    ready:   readiness.is_ready()      // Readiness::Ready
    core_up: readiness.is_core_up()    // Ready | NoEgress
            ▲
  core/manager/mod.rs  observe_readiness()
    outcome = probe::probe_core_api(running)          // control API /version
    egress  = observe_egress(outcome == Serving)      // through-tunnel check
    decide(outcome, egress, latch_active)
            ▲
  core/manager/probe.rs  decide(outcome, egress, latch_active)
    (Serving,    Ok)          -> Ready      <- the ONLY route to connected
    (Serving,    _)           -> NoEgress   ◄── renders as 'connecting'
    (Unresponsive, latch)     -> NotReady   ◄── renders as 'connecting'
    (Unresponsive|NotRunning) -> Stopped    -> 'disconnected'
            ▲
  core/manager/probe.rs  probe_egress(core_serving)
    egress_attempt_once()  -> delay_group("Locus Auto", EGRESS_TEST_URL, 5)
    .any(|delay| delay_is_a_measurement(delay, 5))
    up to EGRESS_ATTEMPTS (2) inside EGRESS_DEADLINE (12 s)
            ▲
  mihomo  GET /group/Locus%20Auto/delay?url=…&timeout=5
    -> Ok(HashMap<member, delay_ms>)   or an Err (non-2xx, e.g. 504)
```

**The load-bearing fact.** Two different failures — *the Core is not answering*
and *the Core answered but the egress proof failed* — both render as **connecting**.
The screen cannot tell them apart. Any diagnosis must therefore start by asking
*which* hop failed, not by reading the screen.

---

## 2. Localise it in four commands

Run these in order. Stop at the first that disagrees with expectation; that is the
broken hop.

### Hop 1 — what does the backend actually report?

```sh
# In the running app's devtools console, or against the command surface:
locusStatus()
```

Read **`connected`** and **`ready`** separately. They are different questions:

| `connected` | `ready` | `coreUp` | Meaning |
|---|---|---|---|
| `true` | `true` | `true` | Working. Button says connected. |
| `true` | `false` | `true` | **The stall.** Core is up; egress proof failed. |
| `false` | `false` | `false` | Core not up at all — a different bug. |

If `connected: true, ready: false`, the defect is downstream of the Core and is
**always** the egress check. Go to Hop 2.

### Hop 2 — is the Core answering, and does the tunnel carry a packet?

The two halves of `observe_readiness` are asked separately on purpose, so ask them
separately here.

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
network.

---

## 3. Which hop produces which screen

Use this to map a live symptom onto a hop without a debugger.

| Symptom | Most likely hop | Check |
|---|---|---|
| Stuck connecting **forever**, traffic flowing | egress classifier (`delay_is_a_measurement`) | Hop 4 |
| Stuck connecting, **no** traffic | Core control API or the tunnel itself | Hop 2 |
| Connecting then settling to **disconnected** | `ProbeOutcome::Unresponsive` with a dead latch | Hop 2 |
| Reverts to connecting **intermittently** after working | one transient miss; retries exhausted inside the deadline | Hops 2, 4 |
| Connected, but the app claims **no service** on a working link | `NoEgress` — the honest no-egress case | Hop 2 |
| Wrong member probed on Strike | group selection | Hop 3 |

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

- **Probe the group, not the proxy.** The name is `Locus Auto` (the group). `Locus`
  is the proxy *inside* it, and mihomo refuses a group whose name matches a member
  (see `client/AGENTS.md`). Probing the wrong one changes what is tested.
- **The probe tests general egress, not the hub.** `EGRESS_TEST_URL` is a
  Cloudflare 204 on purpose: a tunnel that reaches only the Locus hub has not
  proven it can carry the student's traffic.
- **A non-2xx from `delay_group` is raised as an `Err`.** `egress_attempt_once`
  collapses `Ok(Err(_))` and `Err(_)` to the same `false`. mihomo's
  `504 "get delay: all proxies timeout"` therefore looks identical to a transport
  failure. This is a known blind spot — see §6.
- **The egress result is cached for 1.5 s.** `observe_egress` serialises the check
  behind a lock and reuses the result, so the 750 ms status poll and the 250 ms
  connect loop collapse onto one round trip. A reading can therefore be up to
  1.5 s old; do not read a single poll as instantaneous truth.
- **`NoEgress` is still not `connected`.** `is_core_up()` is true for `NoEgress`,
  but `phaseFromStatus` uses `ready` alone. Do not "fix" a stall by making
  `NoEgress` render as connected without checking *why* egress failed — that would
  re-open the school-wifi false-positive the two-question design closed.

---

## 6. What is still open here

Read [`STILL-OPEN.md`](STILL-OPEN.md) §"The readiness probes against a live Core"
for the current list. The load-bearing gaps, as of the last edit:

- **Every-member-timeout is indistinguishable from a transport failure.** A `504`
  and an `Err` both become `false`; the probe cannot retry them differently.
- **The engine test cannot exercise slow-but-working.** `tests/egress_probe_engine.rs`
  SKIPs without network and uses a `direct` outbound to a public host. The case that
  broke — a slow round trip through a *real* tunnel — has never been reproduced in
  the suite.
- **No run on a real school network.** The budget is a judgement, not a measurement.

**If you change any constant in this chain** (`EGRESS_ATTEMPT_TIMEOUT`,
`EGRESS_DEADLINE`, `EGRESS_ATTEMPTS`, `EGRESS_RETRY_GAP`, `PROBE_TIMEOUT`,
`EGRESS_CACHE_TTL`), update this document in the same change — per
`docs/README.md` rule 3, a document that disagrees with the code is a bug.

---

## 7. Quick reference

| Thing | Value | Where |
|---|---|---|
| Group probed | `Locus Auto` | `locus/tier.rs::GROUP_NAME` |
| Proxy inside it | `Locus` | `locus/tier.rs::PROXY_NAME` |
| UoT outbound | `Locus-UoT` | `locus/tier.rs::UOT_PROXY_NAME` |
| Egress URL | `http://cp.cloudflare.com/generate_204` | `core/manager/probe.rs::EGRESS_TEST_URL` |
| Per-attempt budget | 5 s | `EGRESS_ATTEMPT_TIMEOUT` |
| Whole-check deadline | 12 s | `EGRESS_DEADLINE` |
| Attempts | 2 | `EGRESS_ATTEMPTS` |
| Gap between attempts | 500 ms | `EGRESS_RETRY_GAP` |
| Control-API timeout | 400 ms | `PROBE_TIMEOUT` |
| Egress cache TTL | 1.5 s | `core/manager/mod.rs::EGRESS_CACHE_TTL` |
| Status poll while settling | 750 ms | `components/connection/use-connection.ts` |
| Status poll idle | 15 s | `components/connection/use-connection.ts` |
| The only route to `connected` | `(Serving, Ok, latch)` | `probe.rs::decide` |
