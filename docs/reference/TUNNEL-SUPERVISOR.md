# The maintenance tunnel supervisor

```
audience:    builder
status:      live
authoritative-for: the state machine that decides when the maintenance tunnel is up,
                   and why a transition may not drop the app's traffic
verified-against: client/src-tauri/src/cmd/chrysalis.rs, client/src-tauri/src/chrysalis/activation_tunnel.rs,
                  client/src-tauri/src/core/manager/probe.rs, client/src-tauri/src/core/manager/traffic_probe.rs
```

## 1. The requirement

> The tunnel is active in the background for **all** the app's traffic, without
> exception, until a proper connection is established.

Two words carry the weight: *all* (not just hub calls), and *established* (not
"the Core says it is ready").

## 2. What the current code does, and why it is unstable

`cmd/chrysalis.rs` decided the lifecycle with a two-branch `if`, evaluated on every
status poll:

```rust
if readiness.is_ready() { stop_maintenance() } else { warm_maintenance() }
```

`readiness` is not the naive signal it looks like. `probe::decide` reaches
`Ready` only on **evidence that payload moved** — a completed egress round trip,
or bytes reported on the Core's `/traffic` stream. Core-up is explicitly *not*
evidence (`probe.rs:266`). So the gate is better than "TUN is up".

It is still the wrong rule, for three reasons that are all the same reason:

1. **It is instantaneous, not sustained.** A single `Flowing` sample at poll *n*
   stops the engine. A student whose page load finishes a second later, or whose
   delay test landed during a lull, drops back to `Connecting` — and the engine
   is already dead. The next poll restarts it. **That is the flapping you asked
   about**: the rule is evaluated faster than the evidence it consumes, so any
   jitter in a sample becomes a start/stop cycle.

2. **The stop is abrupt.** `stop_maintenance()` sets the slot to `None` with no
   drain. A request already handed to that SOCKS port fails mid-stream. Worse for
   us since 2026-11-19: the retry failsafe now *restarts* the engine on a
   transport failure, so during a flap a request can trigger a restart that the
   next poll immediately reverses.

3. **It does not cover "all traffic".** The engine is only ever consulted by
   `hub_client_for`, so activation, heartbeat and updates ride it — and nothing
   else does. Any other hub-bound call the app makes goes direct, which on a
   network that blocks the domain is a guaranteed failure.

## 3. The state machine

Four states. The engine exists in all of them except `Stopped`.

```text
                 ┌──────────────── (plan tunnel carries traffic
                 │                   for HOLD seconds)
                 ▼
   Warming ──▶ Carrying ──▶ Handover ──▶ Stopped
      ▲            ▲            │            │
      │            └────────────┘            │
      │            (traffic stopped:         │
      │             abort the handover)      │
      └──────────────────────────────────────┘
                    (plan tunnel dropped,
                     or a hub call failed)
```

| State | Engine | App traffic | Leaves when |
|---|---|---|---|
| `Warming` | starting | direct, then the engine once listening | engine listens → `Carrying`; `Ready` already held → `Stopped`; start fails → `Warming` + backoff |
| `Carrying` | **up** | **all** through it | `Ready` sustained for `HOLD` → `Handover` |
| `Handover` | **still up** | **all** through it | `Ready` held for `HOLD` → `Stopped`; `Ready` lost → `Carrying` (engine untouched) |
| `Stopped` | down | direct | `Ready` lost, or a transport failure → `Warming` |

Implemented as `activation_tunnel::Supervisor`, driven by `supervise()` from
`cmd/chrysalis.rs`. The transition rule is a pure function of two booleans and an
`Instant`, so every edge below is tested at a fake clock with no Core, no engine
and no network.

### 3.1 The two constants

- **`HOLD` = 10 s.** How long `Ready` must persist before the engine is stopped.
  Chosen against the poll interval (15 s idle, 750 ms while connecting): 10 s is
  longer than the connecting poll by an order of magnitude, so a single sample
  cannot cause a transition, and shorter than the idle poll, so the handover
  completes within one idle tick.
- **`RESTART_BACKOFF` = 5 s.** A failed start does not retry in a tight loop. The
  existing `start_quietly` already logs the reason.

### 3.2 Why `Handover` is a separate state and not a timer

A timer owned by `Carrying` would have to be cancelled by the same function that
sets it, and the cancel path would be "any poll that is not `Ready`". A distinct
state makes the cancel explicit and testable: `Handover` → `Carrying` **without
stopping the engine** is the one transition that proves no traffic was dropped,
and it is asserted as its own test.

### 3.3 What "all app traffic" means concretely

While `Carrying` or `Handover`, every hub request the app makes goes through the
engine — not only the three call sites that use `hub_client_for` today. The
implementation is one accessor that the remaining call sites adopt, so the rule
lives in one place rather than in each caller's judgement.

**This does not put user traffic through the free tier.** The student's own
traffic is carried by the plan tunnel once it is up; before that there is no
plan tunnel to carry it, and the app has no business proxying it at 1 Mbps.

## 4. What this does not promise

- It does not make a blocked network work. If the appliance classifies an opaque
  stream on `:8443`, no state machine helps. Measured 2026-11-19: it does not.
- It does not survive the engine being killed by something outside the app.
- `HOLD` is a guess calibrated on reasoning, not on a measurement from the school
  network. If the handover still flaps there, the number in this file is the
  thing to change, and it should be changed only after a log shows the cycle.

## 4a. What the critical review of the first version found

The first cut of this supervisor was reviewed against the whole path rather than
against its own tests, and **four defects were found in it**, plus one that had
been missed at a call site. All are fixed; the point of recording them is that
each was invisible to a green test run.

1. **The drained stop blocked the status poll.** `supervise()` is awaited inside
   `chrysalis_status`, which the UI polls every 750 ms. Awaiting a 45 s drain there
   would stall the status command — a worse student-visible fault than the
   handover it was making graceful. Start and stop are now **dispatched**, not
   awaited, and each is re-entrancy guarded.

2. **`warm_maintenance` was still called directly from `lib.rs`**, bypassing the
   state machine entirely, so an engine could be started without the supervisor
   ever entering `Carrying`.

3. **The update download still used `hub_client_for`**, not
   `hub_client_for_retry`. It was the last of three retry sites needing the warm
   check, so an update failed where a heartbeat succeeded, for no observable
   reason.

4. **A drain could leave `draining` set forever** if its task panicked, silently
   disabling the route for the life of the process — the same one-way-latch
   failure this work already removed at the request level, reintroduced one
   level down. It is now cleared by a `Drop` guard.

5. **A guard that could not fail.** The first drain test called the real
   accessor, which reads an empty slot in a test binary, so every branch returned
   `None` — the assertion passed *even with the gate deleted*. The gate is now a
   pure `admit(prefer, warm, draining)` so a warm proxy can be supplied, and it
   was re-verified failing.

6. **`warm_maintenance` answered the wrong question after a refactor.** Extracting
   the refusals turned every refusal into `false`, including "already warm" — so
   a working engine reported itself absent. It returns whether a **route exists**,
   not whether one was started.

## 5. The guards

Each is a test, and each was **run red** against the rule it replaces before
being kept — see the "falsified" note in each.

| Test | The defect it pins |
|---|---|
| `a_flapping_ready_signal_never_stops_the_engine` | 60 s of alternating samples at the real 750 ms cadence, asserting no `Stop` and no departure from `Carrying`. Fails against the old instantaneous rule; a sample-*count* rule would also pass a naive version of this, so it is driven at real poll intervals off a fake clock. |
| `a_sustained_ready_signal_completes_the_handover` | The hold is measured from the first true sample of a run, and one millisecond short is still `Keep`. Also pins that the stop happens exactly once. |
| `traffic_resuming_mid_handover_cancels_it_without_stopping` | The `Handover → Carrying` edge, asserting `Action::Keep` — nothing stopped, nothing started. This is the transition that proves no traffic is dropped. |
| `an_engine_already_running_is_adopted_not_restarted` | `Warming` must not replace an engine the on-demand retry already started. |
| `a_proven_plan_tunnel_without_an_engine_starts_nothing` | The launch-while-connected edge, which would otherwise start an engine the next poll stops. |
| `a_draining_engine_takes_no_new_requests` | The gate itself, driven through the pure `admit` with a warm proxy supplied. **This is the one that was first written so it could not fail** — see §4a.5. |
| `the_in_flight_count_returns_to_zero_after_each_request` | A leaked count, which would make every later handover wait out the full 45 s. |
| `the_start_budget_is_reachable_and_bounded` / `a_successful_start_clears_the_attempt_budget` | The start cap: reachable, and reset on success so it means "three in a row". |
| `a_warm_engine_reports_a_route_even_though_nothing_was_started` | §4a.6 — a status answer must not be turned into a work answer. |

## 6. Known limits

- **`HOLD` is reasoned, not measured.** 10 s sits between the 750 ms connecting
  poll and the 15 s idle poll. If the handover still cycles on the school
  network, this constant is the thing to change — after a log shows the cycle, so
  the number is not adjusted to hide a different fault.
- **The engine start is not bounded by a session budget.** Each failed start
  costs `STARTUP_TIMEOUT` (10 s) and retries on the 15 s idle poll, so a network
  that refuses the bootstrap produces a start attempt roughly every 25 s for the
  life of the session. That is far less than the old per-beat spawn, and it is
  bounded by `TUNNEL_MAX_ATTEMPTS` on the request path — but the supervisor's own
  `Warming` loop has no cap.
- **Nothing here survives the engine being killed from outside the app.**
- **The drain abandons a request older than `DRAIN_TIMEOUT` (45 s).** An update
  download can legitimately run for up to 600 s, so a handover during one still
  cuts it — just after 45 s rather than instantly, and with a warning naming the
  count. Waiting longer would pin a 60 MB process on a student's machine for ten
  minutes; the trade is deliberate and this is the line to revisit if downloads
  are observed failing across a handover.
- **`HOLD` and `DRAIN_TIMEOUT` have never been observed on the school network.**
  Both are reasoned from the poll intervals, and both are the values to change if
  a log from that network shows the cycle.
