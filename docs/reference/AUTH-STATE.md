# The auth state model, and why the two sides disagreed

```
audience:    builder
status:      live
authoritative-for: the canonical auth states on each side, the two axes
                   (entitlement × liveness), and the named desync cases
verified-against: server/pb_hooks/heartbeat.pb.js,
                  server/pb_hooks/session_probe.pb.js,
                  server/console/src/views/codes/model.ts,
                  client/src-tauri/src/chrysalis/heartbeat.rs
guard:       server/scripts/check-consistency.sh §58, §59
```

**Read this before touching `last_seen_at`, `stateLabel`, `livenessOf`, or
anything that renders whether a client is signed in.**

---

## 1. The defect this document exists for

> A client that is actually logged in shows as signed out on the hub.

It was not stale state, a lost event, or a failed refresh. **The hub had no
concept of a client being logged in at all**, and the console answered a liveness
question with an entitlement fact.

The facts, from the code:

| Fact | Where |
|---|---|
| `POST /api/heartbeat` wrote **nothing**. It was a pure read. | `heartbeat.pb.js` — no `saveRecord`, no `set` |
| The `codes` collection had no column recording when a device last checked in | `seed-pb.py` |
| The console derived its badge from `activated_at` / `released_at` | `views/codes/model.ts: stateLabel` |
| `released_at` means "a student pressed **Sign out** since the last activation" | `device_release.pb.js` |
| Nothing else ever writes `released_at` | API.md §5c |

So "beat 30 seconds ago" and "killed three weeks ago" produced **byte-identical
hub state**, and the console filled that hole with `released_at` — a different
fact, about a different thing — and printed `signed out`.

Two consequences, and both still shape the design:

1. **The hub could not detect absence.** Without a trace, a silent client and a
   dead one are the same row. No amount of console logic could fix that.
2. **A stale `released_at` renders as "signed out" for a beating device.** A
   reinstall that re-activated from the credential mirror, a sign-out recorded on
   one device while the student signed in on another, or any claim nobody spent —
   all produce it.

---

## 2. The two axes

The mistake to avoid is a single span-both "auth state", because the two sides
own different facts and **neither can answer the other's question**.

```
            ┌─────────────────────────────────────────┐
            │  entitlement: may this code be used?    │   THE HUB answers —
            │  values: active · released-unclaimed    │   it is the only side
            │          suspended · expired            │   that can refuse
            │          never-activated · unknown      │
            └─────────────────────────────────────────┘

            ┌─────────────────────────────────────────┐
            │  liveness: is this device alive?        │   THE CLIENT answers —
            │  values: live · stale · absent          │   it beats, and only
            │          unknown                        │   it observes itself
            └─────────────────────────────────────────┘
```

**Source of truth, stated once:**

> The hub is authoritative for **entitlement**. The client is authoritative for
> **liveness**. Neither may answer the other's question by inference.

The justification is capability, not preference: only the hub can refuse service,
and only the client can observe its own process. The original bug was a
violation of exactly this rule.

The console renders the **pair**. `entitlement=active, liveness=absent` reads
*"in term · away 9d"* — which is true. It never reads *"signed out"*, because
that is a statement about entitlement and the entitlement is active.

### `liveness: unknown`

`last_seen_at` null means **unknown**, never `absent`. A code that has not beaten
since the column existed has no evidence either way, and rendering it as "away"
— still less as "signed out" — accuses a device on no evidence.

It is a first-class value in the vocabulary on both sides, and the console renders
it as `—` with a tooltip saying so. §58 fails if it is rendered as a state.

---

## 3. Client-side states

These are already the product's states; see [`SUSPENSION.md`](SUSPENSION.md) for
the invariant that a suspension cannot outlive its code.

| State | Stored | Gate shows | Left by |
|---|---|---|---|
| Activated | code + tier + expiry | the app | a refusal, or a sign-out |
| Suspended | the same, plus `chrysalis_suspended` | `SuspendedScreen` | the hub answering 200 |
| Refused | no code, reason kept | `ActivationScreen` with the reason | a new activation |
| Signed out | no code, no history | `ActivationScreen` | a new activation |
| Lapsed grace | code kept, tier/expiry cleared | the app, unconnected | a successful beat |

**The client's own liveness is `last_heartbeat_ok` + `heartbeat_failures`,** and
it carries a 7-day `GRACE_PERIOD` during which a device whose beats are failing
still renders as authenticated. That is the *opposite* desync — the client says
signed-in with no corroboration — and it is why `STALE` on the hub side is set to
the same 7 days: both sides age a device out on one clock.

There is one state the table is missing and this work names:

| Diverged | local record says activated; the hub refuses or has no record | the app, until reconciled | the next beat |
|---|---|---|---|

---

## 4. The named desync cases

`POST /api/session-probe` distinguishes them. `reason_code` is the answer.

| # | Case | Hub | Client | `reason_code` |
|---|---|---|---|---|
| **D1** | Alive; hub said nothing about liveness (the original defect) | `live` | activated | `client_beating_hub_quiet` |
| **D2** | Grace: beats failing, tunnel still up | aging | activated, failures > 0 | `client_unreachable` until the lease, then the same |
| **D3** | **Stale `released_at` from another device or a reinstall** | `released-unclaimed` + `live` | activated | **`stale_release_claim`** |
| **D4** | Genuinely signed out, or a silent local clear | `released-unclaimed`, aging | signed-out | `genuinely_signed_out` |
| **D5** | Hub refused; client has not beaten since | `suspended`/`expired` + `live` | activated | `hub_refused_client_hasnt_noticed` |
| **D6** | Clock skew | either | either | `clock_skew` (reported, never acted on) |

**D1, D3 and D4 all used to print "signed out".** Telling them apart is the whole
value of the probe, and [`../operate/RUNBOOK-DESYNC.md`](../operate/RUNBOOK-DESYNC.md)
is the ordered procedure that does it.

---

## 5. Why liveness is a lease, not a quit message

> **Liveness is a lease granted by the client and expired by the hub. The quit
> message is an optimisation; the guarantee is that a silent client ages out.**

This follows from the cases and needs no argument beyond them: `kill -9`, OOM, a
panic, power loss and force-quit **cannot** send a message; sleep drops the
connection silently; and the client's own backoff means a healthy device is
legitimately silent for up to two hours. Any design whose correctness depends on
a final message is wrong for at least four of those.

**The bounds, derived — not chosen:**

| Bound | Value | Derivation |
|---|---|---|
| `LEASE` (live) | 2 h 14 min | `MAX_INTERVAL` 2 h × 1.1 jitter + 15 s timeout + 60 s slack = 2 h 13 min 15 s, rounded up |
| `STALE` | 7 days | the client's own `GRACE_PERIOD` |

Both values are **data**, not prose: [`../state.toml`](../state.toml) `[liveness]`
holds them, and §59 recomputes them from `heartbeat.rs` and fails when the hook,
the console, the Overview or the prose drift from that derivation. The lease was
2 h 16 min until 2026-11-28; the extra two minutes were never in the formula.

A shorter lease would declare a **healthy but unlucky** device absent the moment
it backed off to `MAX_INTERVAL` after an outage. That is the failure the number
is sized to prevent — see ADR [0002](adr/0002-liveness-is-a-lease-not-a-quit-message.md).

**Acceptance-criterion bound:** a `kill -9`'d client reads `stale` within **≤ 2 h
14 min of its last beat**, with no message. That is a property of the existing
backoff, not new machinery, and tightening it would require beating every 5
minutes forever against a 1-per-10s-per-IP Caddy limit.

---

## 6. Guard-failure evidence

§58 asserts five things. Each was observed failing against the code it catches,
before it was kept:

| Assertion | How it was made to fail | Result |
|---|---|---|
| the heartbeat writes `last_seen_at` | deleted the `record.set` from `heartbeat.pb.js` | **FAIL** "liveness would be permanently unknown and the desync unfixable" |
| it is written before the refusal | moved the write below the 410 return | **FAIL** "a suspended or expired device that is beating would be reported absent" |
| the two sides agree on the lease | set the hook's `LEASE_SECONDS` to 300 | **FAIL** "hook 300s vs console 8040s" |
| the vocabulary agrees | — | same script, per-word assertions |
| `unknown` is not a state | made `livenessLabel('unknown')` return `'signed out'` | **FAIL** "that is the exact defect this change removes" |

Restored, all 36 assertions pass.

§59 asserts the lease is **derived** rather than stated, and it was shown failing
the same way — by changing the source, not the copies:

| Assertion | How it was made to fail | Result |
|---|---|---|
| the lease follows `MAX_INTERVAL` | set `MAX_INTERVAL` to 1 h in `heartbeat.rs` | **FAIL** four BAD lines: state.toml, the hook, the Overview and §58's driver all still said 8040 s against an implied 4080 s |
| the stale bound follows `GRACE_PERIOD` | set `GRACE_PERIOD` to 3 days | **FAIL** state.toml and the console still said 7 days |

The first run of §59 also **found a real defect**: the lease had been 2 h 16 min
since it was introduced, but the derivation every copy cited
(`2 h × 1.1 + 15 s + 60 s`) yields 2 h 13 min 15 s. The constant was chosen first
and the sentence written to match it. Corrected to 2 h 14 min everywhere on
2026-11-28; the number is now `docs/state.toml` `[liveness]` `lease_s` and nothing
restates it.

---

## 7. What this document does not cover

* The probe's wire contract — [`API.md`](API.md) §5d.
* The operator procedure — [`../operate/RUNBOOK-DESYNC.md`](../operate/RUNBOOK-DESYNC.md).
* The decisions and their rejected alternatives — [`adr/`](adr/).
* The four entitlement states and their invariant — [`SUSPENSION.md`](SUSPENSION.md).

---

## 8. Schema, migration, and the test plan

### 8.1 The schema change

One column, additive, nullable, indexed by PocketBase's own date handling:

```
codes.last_seen_at   date   NULL   -- written by heartbeat.pb.js, hub clock only
```

* **No backfill.** A row that has not beaten since the column existed stays null,
  and null is `unknown` on both sides — an honest answer, not a gap to fill.
* **No migration of existing rows.** `seed-pb.py` is the source; re-running the
  schema step adds the column to a live collection.
* **The hub's own copy must be redeployed.** The hooks on the box are deployed
  from `/root/server/`, not from this repository (`hooks-sync.sh`). Until they
  are, the write is inside a `try/catch` and liveness stays `unknown` — it fails
  honestly rather than throwing on every beat. This is the "a repo fix is inert
  until the piece that runs it is redeployed" trap, and `RUNBOOK-DESYNC.md` §1 is
  where an operator meets it.

### 8.2 What was actually run

Verified, in this checkout:

| Check | Result |
|---|---|
| `server/scripts/liveness-consistency.js` | **36 assertions pass** |
| `server/scripts/probe-hook-consistency.js` | **48 assertions pass** (runs the real handler) |
| §58 in `check-consistency.sh` | **OK** (3 assertions, 84 total) |
| `cargo test` (client) | **817 pass, 0 fail**, incl. 12 probe tests |
| `cargo clippy --all-targets --features clippy -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `scripts/check-rust-deps.sh` | 0 dead dependencies |
| `tsc --noEmit` (client + console) | clean |
| `vitest run` (client) | 211 tests pass, incl. locale parity |
| `vite build` (console) | builds |

Each new guard was **shown to fail** against the code it catches — §6 above and
`server/scripts/liveness-consistency.js` carry the evidence.

### 8.3 Test plan: what is structural and what is not

**Verified** (ran it): everything in §8.2.

**Structural** (correct by construction, not executed here): the probe's
end-to-end path against a live PocketBase. No goja runtime is available in this
checkout, so the hook's SQL save, the Caddy zone and the forwarder hop are
reviewed, not run. A hub staging deploy is the first real exercise.

**Unverified, and named as such:**

* **Kill-9 timing.** The ≤ 2 h 14 min bound is arithmetic from the client's own
  constants, not a measured wall-clock observation across all four platforms.
* **The `device_id` gate against a pre-migration code.** A code with no
  `first_seen_fingerprint` fails the machine check and answers `404`. That is the
  safe direction, but it means such a code cannot be probed — the console is the
  route there, and this is stated in the response's own comment.
* **Long-offline replay.** Not exercised: it depends on the client's backoff
  reaching `MAX_INTERVAL`, which takes hours.

### 8.4 The cases that still need an integration harness

| Case | What it would assert |
|---|---|
| `kill -9` | last beat → `stale` within the lease, no message sent |
| offline-flap | `Backoff::reset` on success; no duplicate hub rows (one row per code) |
| blocked network | probe fails closed to an error, never to a fabricated `unknown` report |
| hub deploy mid-probe | `503`/socket close → error, not a partial `200` |
| expired `probe_version` | `409` carrying the supported version |
| uniform 404 | unknown / suspended / expired / wrong-device are byte-identical |

---

## 9. What mutation testing found that reading did not

This section exists because the first version of this work **passed every gate
and was still wrong in four ways**. It is kept as evidence for the practice, not
as history.

Every claim below was found by *executing* the code under a deliberately broken
version, not by re-reading it.

| # | Defect | How it was found | Why reading missed it |
|---|---|---|---|
| 1 | The hook's own comment claimed suspended/expired answered the uniform `404`. They answer `200` with detail — **correctly**, because the machine check gates first. | The harness asserted the comment and failed. | The comment and the code were written in the same session; the comment described an earlier draft. |
| 2 | The `reason_code` chain ended in `else { "in_sync" }`, so a device the hub had **never seen** was reported as "both sides agree". | A mutation that routed everything to `in_sync` passed the vocabulary check; changing the check to assert exact values failed it. | A vocabulary test proves a value is *legal*, not that it is *right*. |
| 3 | `client_dropped_session` and `genuinely_signed_out` were ordered so one shadowed the other. | Two mutants, in opposite directions. | Reading a chain of `else if`s does not make their precedence obvious; driving them does. |
| 4 | `clock_skew` was a `divergence` entry but never the `reason_code`, so a skewed client reported `client_beating_hub_quiet` beside its own contradiction. | A mutant that widened the threshold 10× survived. | The two facts lived 20 lines apart. |
| 5 | **The client** reported a device that had never beaten as `Activated`, because `remaining_grace` returns the full window for `last_ok == 0`. The hub would then call the pair `in_sync`. | The new `derive_auth_state` test, run against the old logic: `left: Activated, right: LapsedGrace`. | This is the *same bug as the original defect*, one layer down. It was reintroduced within an hour of fixing it. |

### The lesson, stated plainly

Defects 2, 3 and 4 are all **precedence** errors in one chain. A chain of
`else if`s is not reviewable by eye at this density — every branch is individually
correct, and the bug is the order. The guard that catches them executes the
chain against fixtures with expected *values*, not expected vocabulary.

Defect 5 is the one that should worry a reader most: the fix for "unknown is
never a state" reintroduced "unknown is a state" on the other side of the wire.
That is why §58 asserts the property on both sides and why
`probe-hook-consistency.js` and `liveness-consistency.js` both exist.

---

## 10. Open item: the tunnel coupling is unverified

**Status: blocked, not done.** Read this before trusting the probe's transport.

`chrysalis/probe.rs` reaches the hub by trying the direct route and then the free
tunnel (`worthy_of_tunnel`, then `hub_client_for_retry`). That logic is written
against `activation_tunnel.rs` **as it stands at commit `e887583`** — which, at
the time this section was written, is the newest such commit in this checkout.

A report of "extensive alterations to the tunnelling system" could not be located
in this repository: not in the working tree, not on `main`, not on
`origin/main`, and not in any other worktree. The tunnel source file is unchanged
since the product rename.

This matters because the probe depends on four things in that file, and any of
them moving breaks the diagnostic silently:

| The probe relies on | In `activation_tunnel.rs` |
|---|---|
| `hub_client_for` (direct / warm only) | exists |
| `hub_client_for_retry` (may **start** an engine) | exists |
| `activation_tunnel_available` (build carries a key) | exists |
| `Failure::retryable_over_tunnel` as the *starting point* for `worthy_of_tunnel` | exists |

**If the tunnel layer has been rewritten**, re-check those four names and the
`worthy_of_tunnel` fault set against the new code before deploying. The
`Transport` widening is the specific thing to preserve: it exists because a
silent blackhole is a common school-network block and the heartbeat's predicate
deliberately excludes it for loop-economics reasons that do not apply to a
one-shot probe.

Nothing else in this design depends on the tunnel, so a change there cannot
affect the liveness axis, the state model, the console or the heartbeat's
lease arithmetic.
