# 2. Term and renewal — the arithmetic, exactly

> The arithmetic decides how much time a paying student gets. It is stated here
> in full, with the boundary cases, so it can be checked without reading goja.

---

## 2.1 Producing an expiry from a term

`expiryFromTerm(termDays, fromMs)` in `server/pb_hooks/activation.pb.js`:

```
days = Number(termDays)
if (!isFinite(days) || days <= 0) return null      // never expires
return new Date(fromMs + days * 86400000).toISOString()
```

**Three properties, each deliberate:**

| Property | Why |
|---|---|
| `<= 0` yields `null`, never a date | "Never expires" must be expressible. A sentinel far-future date would be a lie the UI renders as a real date, and it would make `is_lapsed` meaningless. |
| Non-numeric yields `null` | Same reasoning as the goja date-parsing bug that let codes never expire: an unreadable value must not be silently converted into a *decision*. Here the safe answer is "no expiry", not "expired". |
| Arithmetic is in milliseconds since epoch | `2026-01-01 + 70d` is exactly 70×24h regardless of DST or timezone, because the hub stores and compares UTC instants. Verified across a DST boundary. |

`Number("30")` is `30`, so a PocketBase number field that arrives as a string
works. Verified.

## 2.2 Where the term is applied — exactly two places

| Operation | What happens |
|---|---|
| **Activation** (`activation.pb.js`, the bind path) | `expires_at = now + term_days`, **only when the code has a term** |
| **Renewal** (`codes.renew`) | `expires_at = max(now, existing_expiry) + term_days` |

That is the whole surface. There is no third place that computes an expiry from
a term, which is what makes the arithmetic auditable.

### The activation guard, and why it is a guard

```js
var termExpiry = expiryFromTerm(termDays, nowMs);
if (termExpiry) rec.set("expires_at", termExpiry);
```

The `if` matters. A code with **no** term keeps whatever `expires_at` it
already carries — the pre-term behaviour, which every un-migrated row depends
on. Without the guard, activating an old code would **clear** its expiry, and
a student would silently become permanent. That is a bug in the generous
direction, which is the kind that goes unnoticed for a long time.

Note also that the guard cannot *shorten* anything: the only value it can write
is `now + term`, and it only writes when a term exists.

## 2.3 Renewal, and the property that makes it sellable

```js
var prevMs = parsePBDate(rec.get("expires_at"));
var prevValid = !isNaN(prevMs) && prevMs > 0;
var baseMs = (prevValid && prevMs > nowMs) ? prevMs : nowMs;
var newMs = baseMs + (termDays * 86400000);
```

| Case | Base | Result |
|---|---|---|
| Code still running | `existing expires_at` | **The new term is added on top.** A student who pays two weeks early keeps those two weeks. |
| Code already lapsed | `now` | A fresh full term. There is no remaining time to preserve. |
| Code expires exactly now | `now` | A full term. No day is lost either way. |
| `expires_at` unparseable | `now` | Consistent with the hub's standing rule that an unreadable date must not lock anyone out. |

> **The invariant, stated so it can be tested: the new expiry is always greater
> than or equal to both the existing expiry and now.** A renewal can only ever
> move a date forward. Verified — [`06-verification.md`](06-verification.md) §6.3.

This is the property that makes "renew before it lapses" something an operator
can honestly ask a student to do.

## 2.4 The term to apply, and the override

```
termDays = body.term_days if supplied, else the code's own term_days
if (!isFinite(termDays) || termDays <= 0) → 400
```

The override exists because the real case on day one is a student paying for a
span other than the code's own term — a month rather than a term, or two terms
at once. Without it, expressing that means minting a new code, which is the
friction renewal exists to remove. The override is recorded in the event detail
so "why is this code's expiry different from its term" stays answerable.

A code with **no** term and no override is refused with a 400 rather than
guessed at. There is nothing to renew by, and inventing a default would be the
system deciding a commercial question.

## 2.5 What a renewal deliberately does NOT do

| Must not | Why |
|---|---|
| **Clear `suspended`** | A payment is not an abuse pardon. Lifting a suspension as a side effect of taking money would let a suspended account buy its way back without the operator ever deciding that. An operator who wants both performs both — two audited actions, two clear records. |
| **Touch `bound_fingerprint`** | Renewal extends time; it does not move a device. The audit trail must not say a binding changed when it did not. |
| **Change `tier`** | A renewal buys more time, not a different tier. |
| **Recompute `activated_at`** | It is not touched. The expiry is materialised, so the term running now is safe from a later recomputation. |

## 2.6 The result shape

```
{ ok, code, previous_expires_at, expires_at, term_days, extended }
```

`previous_expires_at` and `expires_at` are returned so the console can show
`current → new` before and after the write, and `extended` tells the caller
whether the base was a live expiry or `now` — the difference between "added
onto what you had" and "a fresh term", which is what the operator repeats to the
middleman.

## 2.7 How the change reaches a running student

**No client change was required, which is the payoff of materialising the
instant.**

1. `codes.renew` writes the new `expires_at` on the code row.
2. The next heartbeat (`heartbeat.pb.js`) sends it as `expires_at`, unchanged in
   shape.
3. `handle_success` (`runtime.rs`) calls `store::record_expiry`.
4. The Account row re-renders from `SubscriptionStatus::Active`.

The client does **not** re-enter a code, does not restart, and the tunnel is not
interrupted. The heartbeat interval is 5 minutes at minimum, jittered ±10%.

> **One guard that must stay:** `cmd/locus.rs` writes the stored expiry only
> `if expires_at.is_some()`. An older hub sends `null`, and writing that would
> wipe a date the client legitimately holds. A renewal always sends a value, so
> it passes.

**This propagation has not been observed on a real client** — see
[`05-not-yet-true.md`](05-not-yet-true.md).
