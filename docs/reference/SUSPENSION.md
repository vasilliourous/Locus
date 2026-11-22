# The entitlement states, and the invariant between them

```
audience:    builder
status:      live
authoritative-for: the four states a device can be in, which surface each selects,
                   and the invariant that a suspension is only ever true of a device
                   that still holds its code
verified-against: client/src-tauri/src/chrysalis/store.rs,
                  client/src-tauri/src/chrysalis/runtime.rs,
                  client/src/hooks/use-entitlement.ts,
                  client/src-tauri/src/cmd/chrysalis.rs
```

**What this is.** The four states the gate can be in, what moves a device between
them, and the one rule that must never be broken. Read this before touching
`handle_refusal`, `clear_entitlement`, `sign_out` or the gate in `main.tsx` — they
are four places holding one fact apart.

---

## 1. The four states

| State | Stored | Gate shows | Moved out by |
|---|---|---|---|
| **Activated** | code + tier + expiry | the app | a refusal, or a sign-out |
| **Suspended** | the same, **plus** `chrysalis_suspended` (then `locus_suspended`) | `SuspendedScreen` | the hub answering 200 again |
| **Refused** | no code; `chrysalis_refusal_reason` (then `locus_refusal_reason`) kept | `ActivationScreen`, with the reason | a new activation |
| **Signed out** | no code, and no history | `ActivationScreen` | a new activation |
| **Lapsed grace** | code kept; tier/expiry cleared | the app, unconnected | a successful beat |

`Suspended` and `Lapsed grace` are the only two that are *not* terminal, which is
why they are the only two where the student is told to wait rather than act.

## 2. The invariant

> **A suspension is a statement about a code. It cannot outlive the code.**

A suspension is the one refusal an operator can reverse, so it keeps the
credential: that is what makes unsuspending need no re-entry. The cost of that
choice is a state — `suspended`, with a code still on disk — that is one field
away from nonsense, and the defect that produced this document was a build where
the field survived the code.

Enforced three times, deliberately:

1. **On write** — `store::clear_entitlement` and `store::clear_device_state` both
   clear `chrysalis_suspended` (then `locus_suspended`). Any future path that removes a credential must go
   through one of them; `clear_entitlement` is the only function that deletes the
   code.
2. **On read** — `store::is_suspended` returns `false` unless an activation is
   also readable. This is what protects a device whose `verge.yaml` was written by
   an affected build, which no fix can retract.
3. **At the gate** — `gateScreen(activated, suspended)` checks `!activated`
   *before* `suspended`. Cheap, and it is the last line between a bad flag and a
   screen with no controls.

Belt, braces, and a third belt. That is proportionate because the failure mode is
the worst in the product: `SuspendedScreen` is rendered **without controls** on
purpose (its remedy is a conversation with whoever sold the code), so reaching it
by mistake leaves the student with nothing to press.

## 3. Why a suspended device keeps beating

The screen promises "ask them to lift it and you will be back in without
re-entering your code or restarting the app". That sentence is only true if
something is still asking the hub. So a refusal no longer ends the heartbeat loop:
it switches to `SUSPENDED_INTERVAL` (2 minutes) until the hub says 200, at which
point `handle_success` clears the flag and emits `chrysalis://entitlement-changed`.

`runtime::resume()` is the companion: idempotent, it starts a loop from the stored
activation when none is running, so a device that was suspended when the app quit
is still being watched when it reopens. It cannot resurrect a loop for a signed-out
device, because there is no code to read.

The poll floor is five minutes and the push triggers are focus, `pageshow`,
`online`, `visibilitychange` and two events. The interval is the *last* resort —
not, as it was, the only mechanism.

## 4. The deployment half

The enforcement chain lives in `server/pb_hooks/`, which runs on the box and is
deployed from `/root/server/` — **not from this checkout**. So a client fix is
inert until the hub is redeployed, and "an operator suspended a device and it kept
working" has two indistinguishable causes: the console never set the flag, or the
hub is running hooks older than enforcement version 2.

`heartbeat.pb.js` answers the question on every beat (`enforcement_version`). The
client records it and reports `HubEnforcement`: `unreported` (no beat yet — a
silence, not a verdict), `current`, or `stale { hub }`. A stale hub renders one
caption on the Account page and nothing else; it is a diagnostic, never a gate.

`server/scripts/hooks-sync.sh --check` is the command that actually compares repo
against host. Until that is clean, treat any claim about live enforcement as
**unverified**.
