# Overrides and eviction: releasing a code that is still in use

```
audience:    builder
status:      live
authoritative-for: the override model (why status stays derived), the eviction
                   sequence, and the frozen-wire reason eviction is delivered as
                   a suspension response
verified-against: server/pb_hooks/heartbeat.pb.js,
                  server/pb_hooks/admin_console.pb.js,
                  server/pb_hooks/activation.pb.js,
                  server/console/src/views/codes/model.ts
```

## The defect this replaces

`codes.unbind` cleared `activated_at` with no liveness check, and the heartbeat
never read `activated_at`. So releasing a code a device was still using left the
hub and the client disagreeing **forever**: the hub showed it free, the live
client's next beat returned `200`, and it kept serving a code that could be sold
to someone else. Nothing evicted it and nothing said so.

That was not a missing guard at the moment of release — it was a fact nobody
checked. Release is now `codes.release`, and it carries the eviction the old
action never had.

## Why there is no "in use" test

`last_seen_at` proves the device **reached the hub**, not that a student is at it
(`livenessLabel` in `model.ts` is careful about exactly this). A release of a
"busy" code is therefore a **decision an operator makes**, never a condition the
hub detects. That is why it is recorded as an override with an author, rather
than refused by a guard that would be refusing on a guess.

## Status stays derived; the override is an input

```
main_status =
  force_state === 'released' ? 'released'      ← the override, highest precedence
  : suspended                ? 'suspended'
  : expires_at < now         ? 'expired'
  : activated_at             ? 'redeemed'
  :                            'available'
```

An override never becomes the status. It is an input the derivation reads, so
**removing the override restores the prior derived state exactly** — which is what
makes `codes.reactivate` a delete rather than a reconstruction.

The observed axis (`livenessOf`: `live`/`stale`/`absent`/`unknown`) is **not an
input to this derivation at all**, by construction. It is shown beside the status
and, where the two disagree, as a conflict marker (`conflictOf`). That marker is a
pure render: it cannot write, so an observation cannot silently become a status.

## The eviction sequence

1. Operator presses **Mark unused** in the inspector. The console sends
   `codes.release { code }` as a DIRECT action — there is no drawer, no preview,
   no typed confirmation and no reason field. `codes.release { dry_run: true }`
   is still supported for scripts and still computes the consequences **from the
   row**, including whether a device will be evicted; the console simply no longer
   asks for it first.
2. Hub writes the `code_overrides` audit row first (a hard failure aborts the
   action), then one save: `activated_at=null`, `force_state='released'`,
   `evict_requested_at`, `evict_reason`, `override_id`.
3. The device learns at its next heartbeat, which answers `403`.

`will_evict` is true for `live` **and** `stale` (a device inside the 7-day window
that may come back at any beat), and false for `absent`/`unknown` — a code nobody
has touched in a week is very likely genuinely free, and a false suspension on a
code about to be resold outlives the mistake.

## The frozen-wire trap: the response says `suspended`, the column does not

Deployed clients decide reversibility with a substring test:

```rust
fn is_suspension(reason: &str) -> bool { reason.to_ascii_lowercase().contains("suspended") }
```

`runtime.rs`. The **false** branch deletes the credential — a permanent lock-out
for what may be an operator's mistaken release. So the eviction response carries
`suspended: true` and the true sentence in `suspended_reason`, plus additive
`evicted: true` for builds that understand it.

**This is a response field, not the column.** `codes.suspended` is read by
`activation.pb.js` (:351, :381), so setting it would refuse the **next** holder and
break the resale that releasing exists for. There is one stored predicate —
`evict_requested_at` — and only the heartbeat reads it.

Do not "tidy" either half of this. The wire shape and the column separation are
both load-bearing.

## Idempotency

- The console mints one `override_id` per confirmed action; a retry reuses it and
  the handler returns the prior result with no second row and no second save.
- Repeated beats after eviction return `403` each time and perform **zero hub
  writes** (the `last_seen_at` stamp happens first, as it always does).
- Two admins releasing the same code produce two audit rows and one final state;
  overrides never touch `release_count`.

## The override is not a status value (read this before changing anything)

`status` is a **four-value partition** — `available`, `redeemed`, `suspended`,
`expired` — and `console-card-consistency.js` asserts the four buckets sum to the
total. The filter bar, the facet counts and `dashboard` all enumerate exactly
those four.

A release therefore does **not** change `status`. A released code is
`status: "available"` (it genuinely is available to the next holder — that is
what `activation.pb.js` does) and carries `force_state: "released"` as a
**separate field**, rendered by `overrideNote()` beside the badge.

The first implementation got this wrong and added `released` as a fifth value.
It was silent in three ways at once: `?status=available` stopped listing released
stock, released codes vanished from every facet count (no `<option>` rendered the
new bucket), and `dashboard` and the list disagreed about the same code. Guarded
now by §66 assertions 6 and 8, each demonstrated failing first.

**The general rule:** before adding a member to an enumerated vocabulary, grep
every consumer that partitions on it. A partition is only exhaustive if all
consumers agree on its members.

## A successful activation consumes the override

This is not obvious and it broke the feature's whole purpose on the first pass.
A released code arrives at activation's **first-redemption arm**, because the
release cleared `activated_at`. If that arm left `evict_requested_at` set, the
new holder would activate fine and then be refused at their **next heartbeat** —
the resale works for one beat and then cuts off the student who just paid.

So the first-redemption save clears `force_state`, `evict_requested_at` and
`evict_reason` in the same write as `activated_at`. Guarded by §66 assertion 7.

## The client half is pinned by one test, end to end

`client/src-tauri/src/chrysalis/runtime.rs` carries
`the_release_eviction_the_hub_sends_reaches_the_suspension_path`: it reads the
eviction sentences OUT of `heartbeat.pb.js`, feeds them through the real
`classify_response` and `is_suspension`, asserts the store read the gate uses
(`is_suspended`, including the `suspended && !activated` refusal), and leaves the
final "which screen" step to `gateScreen` (unit-tested) and to
`client/.harness/verify-states.mjs`, which renders the stopped page with the hub's
sentence. The sentences are not retyped in the test, so a hub-side reword fails it
rather than silently sending every deployed client down the credential-deleting
branch. It was observed failing by rewording both sentences to drop "suspended".

## A released code stays protected from deletion

`codes.delete` used to refuse only while `activated_at` was set, and its message
said "Release it first". After a release that column is null, so the guard
stopped protecting the row — and the message guided the operator toward releasing
a sold code in order to delete it, destroying `first_seen_at` and the audit trail.
It now refuses on "was ever issued" and no longer suggests releasing.

## Desync: how a change reaches every reader

There is one stored truth (the `codes` row) and three readers — the console, the
client's heartbeat, and `session_probe`. No reader has its own copy of status;
`fetchCode` calls the same `codes.list` handler, so the Inspector and the list
cannot disagree.

**Every state change is exactly one `saveRecord`.** Release, reactivate and the
activation override-clear each write all their fields in one call, so the hub is
never observed half-changed. Guard 10 asserts this and was shown failing against
an injected split save.

**The `claim_token` is still enforced when a caller sends one.** It is the
`activated_at` a preview was computed against; a mismatch returns 409. The console
no longer previews, so its direct release omits the token — a direct action is
taken against the state as it is at the moment it is pressed. `suspend`/`kick`
deliberately have no token either: they are idempotent set operations, so a stale
call converges rather than diverging.

**An already-released code is a no-op, not a 400.** It used to answer "That code
has not been used yet", which was false and read like a failure to a second
operator or a retried request.

### The client-side invariant that must never be broken

`classify_response` takes the refusal reason from the JSON **`message`** field —
not the `suspended` boolean — and `handle_refusal` decides reversibility with

```rust
reason.to_ascii_lowercase().contains("suspended")
```

So **the eviction message must contain the word "suspended"**. The first version
said "This code was released by your provider — …", which reads better and sends
the client down the branch that deletes the credential: a permanent lock-out and a
card re-entry, for what may be an operator's mistaken release.

The message therefore leads with the frozen `Account suspended —` prefix and
carries the truth after it. Guard 9 checks every message variant, and also asserts
the client still classifies by `message`, because if that changes the guard's
rationale changes too.

## The other direct override: a pending sign-out → in term

`codes.override { to_state: "in_term" }` is the console's **Mark in term**. It
exists for the one state a release cannot express: a code whose holder pressed
Sign out (`released_at` later than `activated_at`) while the claim on the machine
is still the one the operator wants in force.

It is a REAL state change, not a label: clearing `released_at` is what
`activation.pb.js` compares, so a second machine is refused again and the current
holder keeps the claim. `force_state` is deliberately NOT set — the derivation
already agrees once the date is gone, and a pin that restates the derived answer
would be a second source of truth for it. `release_count` is left alone (the
sign-out happened; this withdraws only the pending claim), and the action writes
one `code_overrides` row with `aspect: "in_term"` before its single save.

It does not have an undo: the operator's correction is "the sign-out is void", and
the student can sign out again. A repeated call is a truthful no-op.

## The probe sees the override

`session_probe.pb.js` reports `override` (force state, id, eviction) and adds
`operator_override` to `divergence` whenever `force_state` or
`evict_requested_at` is set. It reads the same predicate the heartbeat enforces,
so a run after a console override answers "did it take?" — and the console shows
it as an alert. Without this the flag would be written by one action and read by
none of the diagnostic surfaces, which is the failure R3 names.

## Fields an override must never write

`last_seen_at` (writing it makes liveness unfalsifiable), `released_at` /
`release_count` (the client's own statement about itself), `first_seen_at` /
`first_seen_fingerprint` (provenance, and the probe's anti-oracle gate), `code`
(frozen on printed cards), `used` (vestigial).

## Undo

`codes.reactivate` clears `force_state`, `evict_requested_at`, `evict_reason` and
`override_id` in one save. It does **not** restore a cleared `activated_at` (see
above), so "undo" on a release removes the eviction and the pin — the freed claim
stays freed. The inspector offers it as **Undo release** and says so only by being
a direct, audited action rather than promising a full rollback. It deliberately does **not** restore `activated_at`: if
the code was re-redeemed by someone else, that claim is a real fact and survives.
The evicted device is still beating (a suspension-shaped refusal keeps the code),
so it is back in at its next beat with no re-entry.

`codes.unsuspend` does **not** clear an override, and says so in its response — a
code can be both released and unsuspended, and clearing only the suspension would
be re-evicting the device at its next beat.
