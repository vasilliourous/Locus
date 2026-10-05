# 2. Term model and renewal

```
audience:    human-operator
status:      design-record
authoritative-for: the term model and renewal decisions
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **Decisions (operator):** expiry is a **term length measured from
> activation**; renewal **extends from the existing expiry**, so paying early
> never loses days.
>
> This file specifies the model, the exact arithmetic, and the audit trail.

---

## 2.1 Why term-relative, not absolute

### What the code did before this work

> **The pre-`9a91da1` state.** Kept because it is the premise the redesign
> overturns. For what the code does *now*, see
> [`implementation/02-term-and-renewal.md`](implementation/02-term-and-renewal.md).

`codes.expires_at` was a `date` (see the `codes` schema in `seed-pb.py`) set at
**mint time** from the console's form (the `codes.generate` action in
`admin_console.pb.js`).

`codes.activated_at` was written at **bind time** (the bind path in
`activation.pb.js`).

**Nothing connects the two.** Verified by grep across `server/`: the only reads
of `activated_at` are

* in the `codes.list` row mapping, to display it, and
* in the `codes.list` sort, as a sort key.

There is no arithmetic anywhere that derives an end date from an activation
date. The only expiry is the one typed in at mint.

### Why that is wrong for this business

A card printed in February and sold in May grants the buyer whatever remains of
a February-to-February window — **not** a year, and not a term. The operator
cannot honestly print "10 weeks" on a card, because the card's clock started
when it was minted, not when it was sold.

Worse, it makes the operator's own stock decay. Unsold cards silently lose
value on the shelf, which is a real cost that shows up as "the batch I printed
last term is worth less now".

### The replacement

A code carries a **term**: a duration. The entitlement's end is computed from
**when the device activated**, not when the card was printed. Nothing on the
card decays while it sits in a drawer, and "10 weeks" means ten weeks.

## 2.2 The representation, and the one trick that makes it tractable

`expires_at` is read in **four** places on the hub, plus the console and the
client wire:

* the expiry gate in `activation.pb.js` — the 410 expiry gate,
* the expiry gate in `heartbeat.pb.js` — the 410 expiry gate on every beat,
* the pre-check in `code_lookup.pb.js` — the read-only pre-check,
* the `expiryForWire` helper in `activation.pb.js`, the `expires_at` response field in `heartbeat.pb.js` — the `expiryForWire` helpers
  that send the date to the client,

and on the client by `locus/expiry.rs` in its entirety — the `Expiry` enum, the
`is_lapsed` enforcement rule, `as_state`, and `EXPIRY_WARNING_DAYS`.

> **Design rule: `expires_at` remains the single enforcement field, and remains
> an instant.**
>
> The term is only *how the instant is computed*. It is materialised into
> `expires_at` when the entitlement changes (activation, renewal).

This is the decision that keeps the change tractable. Because the enforcement
and display paths keep reading an instant from the same field with the same
shape, **none of them change**:

| Path | Changes? |
|---|---|
| The 410 gates in `activation.pb.js` / `heartbeat.pb.js` | **No** — still compares an instant to `Date.now()` |
| `expiryForWire` → the client's `expires_at` | **No** — still an instant string, or `null` |
| `locus/expiry.rs` — parse, `is_lapsed`, `ExpiryState` | **No** — identical contract |
| The connect gate `subscription_allows_connect` (`subscription_allows_connect` in `cmd/locus.rs`) | **No** |
| The client's Account row and the `rename_all_fields` wire shape | **No** |

The alternative — teaching every consumer to compute `activated_at + term` — would
have touched four hooks, the wire format, the whole client expiry module, and
the v3.2.4 wire-shape test that pins `daysRemaining`/`expiresAt`
(`the_active_wire_shape_is_camel_case`). Materialising the instant avoids all of it, and
avoids creating a second source of truth that could disagree with the first.

## 2.3 The arithmetic, stated exactly

New fields on the code (additive; see [`05-migration-and-live-data.md`](05-migration-and-live-data.md)):

| Field | Type | Meaning |
|---|---|---|
| `term_days` | number | Length of one purchase, in days. `0` or absent = never expires. |
| `term_kind` | text | `"month"` · `"term"` · `"year"` · `""`. Display/labelling only — **never** arithmetic. |

`term_kind` is deliberately **not** used to compute anything. A rule like
"month = 30 days" that lives in two places will disagree in one of them;
`term_days` is the number, and `term_kind` is a label the operator sees.

**On activation** (`activation.pb.js`, the bind path at line ~168):

```
activated_at = now
expires_at   = now + term_days days,   if term_days > 0
             = null,                   if term_days == 0 or absent
```

**On renewal** (new `codes.renew`, see §2.4):

```
base       = max(now, expires_at if expires_at is a future date, else now)
expires_at = base + term_days days
```

Both are computed **server-side, in goja**, because that is where the
enforcement field already lives.

### The boundary cases, decided

| Case | Result | Reasoning |
|---|---|---|
| `term_days == 0` / absent | `expires_at = null` → **never expires** | A permanent code is a legitimate product. The gates already treat `0`/`NaN` as "no expiry" (the expiry-gate comment in `heartbeat.pb.js`), so this needs no new handling. |
| Renewing a **lapsed** code | `base = now` | There is no remaining time to preserve; the student gets a fresh full term. |
| Renewing an **active** code | `base = existing expires_at` | The core decision: **paying early never costs days.** A student renewing two weeks before term end gets their new term *added*, not substituted. |
| Renewing a **never-expiring** code | `base = now`, result still effectively permanent only if `term_days` is 0 | Renewing a permanent code with a term is how an operator converts it to a dated one. Warn in the console; do not silently shorten. |
| `expires_at` unparseable | `base = now` | Consistent with the hub's existing "an unreadable date must not lock anyone out" rule (the expiry-gate comment in `heartbeat.pb.js`). |

## 2.4 The renew operation

A new admin action, alongside the eleven existing `codes.*` actions
(the action dispatch in `admin_console.pb.js`):

```
action: "codes.renew"
body:   { code, term_days?, middleman?, note? }
```

### What it must do

1. **Resolve the code** with the existing `canonical()` helper and
   `findFirstRecordByData`, throwing-safe (the pattern every other action uses).
2. **Compute the new expiry** by the rule in §2.3, from the *stored* expiry.
3. **Write it**, then write a `code_events` row recording
   `event: "renewed"`, with the **previous and new expiry in `detail`**.
4. **Return the old and new expiry** so the console can show what changed.

### What it must NOT do

| Must not | Why |
|---|---|
| **Clear `suspended`** | A renewal is a *payment* event, not an abuse pardon. Silently lifting a suspension on payment would let a suspended student buy their way back without the operator noticing. If an operator wants both, they perform both — two audited actions, two clear records. |
| **Touch `bound_fingerprint`** | Renewal extends time; it does not move a device. Changing the binding is `codes.unbind` plus a re-activation, and conflating them would make the audit trail lie about what happened. |
| **Change `tier`** | A renewal buys more *time*, not a different tier. A tier change is a separate decision with separate pricing. |

### Why an optiona `term_days` override

`term_days` in the body overrides the code's own term for this one renewal.
This is needed for the real case the operator will hit on day one: a student
pays for a *different* span than the code was minted with (a month instead of a
term, or two terms at once). Without the override the only way to express that
is to mint a new code, which is exactly the friction this redesign removes.

The override **must be recorded in the event detail**, because "why is this
code's expiry different from its term" is otherwise unanswerable later.

### The audit trail

`code_events` already exists (the `code_events` schema in `seed-pb.py`) and is written by
`logEvent()` in the admin hook. `codes.renew` appends:

| Field | Value |
|---|---|
| `code` | the code |
| `event` | `"renewed"` |
| `detail` | e.g. `"2027-03-14 → 2027-06-22 (term_days=100 by term)"` |
| `actor` | `"console"` (matching the existing convention, `logEvent` in `admin_console.pb.js`) |

This is what makes [`../16-metrics.md`](../16-metrics.md) §16.2's
**"Retention = renewals ÷ expiries"** computable for the first time: a renewal
becomes a countable row rather than an inferred delta. See
[`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md).

## 2.5 What the student sees

**No client change is required to *show* a renewal**, which is a benefit of
keeping `expires_at` as an instant. The next heartbeat
(the `expires_at` response field in `heartbeat.pb.js`) carries the new date; `handle_success` records it
(`handle_success` in `runtime.rs`); the Account row re-renders from
`SubscriptionStatus::Active { days_remaining, expires_at }`.

Two details worth pinning, because they are where a renewal would visibly fail:

1. **`store::record_expiry` only writes when the hub sent something.**
   `cmd/locus.rs:~450` guards with `if expires_at.is_some()`. This must stay:
   an older hub sends `null`, and writing `None` would wipe a date the client
   legitimately holds. A renewal always sends a value, so it passes the guard.
2. **The client must accept a *later* date without a restart.** The heartbeat
   path already re-records on every beat, so this works — but it is exactly the
   kind of claim that should be a test rather than a belief. Specified in
   [`11-phased-implementation.md`](11-phased-implementation.md) phase 4.

The renewal does **not** require the student to re-enter their code, and does
not interrupt the tunnel. That is the whole point: a renewal is invisible to a
connected student, which is what makes it sellable.

## 2.6 Related reading

* Where the term numbers come from commercially → [`../05-pricing.md`](../05-pricing.md), [`../07-billing.md`](../07-billing.md)
* The migration that must not shorten a live term → [`05-migration-and-live-data.md`](05-migration-and-live-data.md)
* The button that triggers this → [`06-console-and-operator.md`](06-console-and-operator.md)
* The wire names that must not change → [`07-wire-contract-and-compatibility.md`](07-wire-contract-and-compatibility.md)
