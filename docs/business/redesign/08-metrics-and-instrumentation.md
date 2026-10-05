# 8. Metrics and instrumentation

```
audience:    human-operator
status:      design-record
authoritative-for: the metrics instrumentation specification
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> [`../16-metrics.md`](../16-metrics.md) §16.2 already *defines* the metrics this
> business needs. This file specifies what must be stored for them to become
> **computable**, and is candid about which ones still will not be.

---

## 8.1 The problem being fixed

`../16-metrics.md` §16.2 lists derived metrics including:

| Metric | Stated definition | Was it computable? | Now |
|---|---|---|---|
| Active subscribers | bound − suspended − expired | **No** — "subscriber" is not an entity. Counted *codes*. | **Yes**, with a caveat — §8.2.2 |
| Retention | renewals ÷ expiries in the window | **No** — renewal did not exist | **Yes** — §8.2.1 |
| Effective ARPU | gross ÷ active paid subscribers | **Partly** — "gross" needs a price | **Partly** — needs `price` recorded on renewals |
| Middleman concentration | top middleman's codes ÷ total | **Yes** — `middlemen.list` | **Yes** |
| Free → paid conversion | paid codes minted ÷ free users | **No** — no cohort, no time series | **Still no** — §8.3 |

So two of the five were real, one was half-real, and two were definitions of
things that could not be measured. The file was honest about the last one
(§16.3: *"the hardest to measure here"*), but "Retention" and "Active
subscribers" were presented as available when they were not.

**Both of those are now computable**, because renewal became a real operation
that writes an event, and the term model made "active" a precise definition.
What each still cannot say is stated with it.

## 8.2 What the redesign makes computable

Both are **by-products of features that had to exist anyway** — there is no
separate instrumentation project here.

### 8.2.1 Retention — from the renewal event

**Built.** `codes.renew` writes the event below, so the numerator is a count
over rows that already exist.

[`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4 writes a `code_events`
row per renewal:

```
event = "renewed"
detail = "2027-03-14 → 2027-06-22 (term_days=100 by term)"
```

So retention becomes a count over an existing table:

```
retention = COUNT(code_events WHERE event = "renewed" in window)
          ÷ COUNT(code_events WHERE event = "expired"   in window)
```

**The denominator is derived, not recorded.** Nothing writes an `"expired"`
event — expiry is *computed* on read (the expiry gate in `activation.pb.js`
and in `heartbeat.pb.js`), never stored. That is deliberate: see the table
below. "How many codes lapsed in this window" is a date filter over rows that
already exist.

The options considered, and why deriving won:

| Option | Where | Assessment |
|---|---|---|
| On the heartbeat 410 | `heartbeat.pb.js` | Reaches real devices, but a lapsed device stops beating — so most expiries would only be seen if the device happens to beat after lapsing. |
| On the activation/lookup 410 | `activation.pb.js`, `code_lookup.pb.js` | Reliable and low-volume, but adds a write to a read path. |
| **Derived** (no event) — **chosen** | query `codes` by `expires_at` | **Cheapest and sufficient for a rate.** Exact at this scale, and needs no new writes. |

With tens to hundreds of codes
([`../12-scale-and-ceiling.md`](../12-scale-and-ceiling.md) §12.1), a query over
`codes` is exact. Add a recorded event only if a real time-series is ever
needed. This made retention computable **the moment renewal shipped**, with no
extra work — which was the point of choosing it.

### 8.2.2 Active subscribers — from the term model

With the term model, "active" has a precise, single definition:

```
active = codes where expires_at is null          (never expires)
       OR      expires_at > now                   (in term)
       AND NOT suspended
```

This is nearly what `dashboard` already computes
(the `dashboard` `expiringSoon` computation) — its `available` / `bound` / `suspended` /
`expired` buckets already partition exactly this way. The change is
**definitional, not mechanical**: the Console's "Activated on a device" card
should be understood (and ideally relabelled) as "codes in term", so the
operator does not read a code count as a headcount.

> Note the honest limit: this counts **codes**, not **people**. With no customer
> entity ([`01-identity-without-pii.md`](01-identity-without-pii.md)), one
> student with two codes is two "subscribers". Since the redesign enforces one
> code per device, this is a close approximation — but it is an approximation,
> and the console should say so rather than imply precision it does not have.

## 8.3 What still cannot be measured, and why

Stated because pretending otherwise is what
[`../17-not-built.md`](../17-not-built.md) exists to prevent.

| Metric | Why not | What it would need |
|---|---|---|
| **Free → paid conversion** | Needs a *cohort* ("users who got a free code in week 1") and a *conversion event* ("one of them bought"). The hub has neither, and no time series ([`../16-metrics.md`](../16-metrics.md) §16.3). | A `minted_at` timestamp we have, plus an explicit link from a paid code to a free one. The link is the missing piece — and it would be an identity link, which the no-PII decision rules out. |
| **Per-user bandwidth** | No per-user accounting exists; free-tier quota is client-side ([`../04-tiers.md`](../04-tiers.md) §4.4). | Per-user SS2022 credentials + a usage table ([`../18-open-items.md`](../18-open-items.md) P4). Deliberately not in scope. |
| **Churn** | Needs to distinguish "stopped paying" from "graduated" from "moved school". The system knows none of these. | Nothing cheap. This is a business guess, not a measurement. |
| **Revenue** | No price is stored; it lives in the plan's price ladder and in the middleman's head. | Store a `price_paid` on the renewal event. Cheap to add — see §8.4. |

## 8.4 The one cheap addition worth making

**Record `price_paid` and `middleman` on the renewal event.**

[`../07-billing.md`](../07-billing.md) §7.4 says cash is collected by
middlemen, with the `middleman` field as *"the only record of who owes what"*.
Today that works because revenue = codes sold × price. **Renewal breaks it**:
once a code can be renewed repeatedly, "how much did middleman X collect this
month" is no longer inferable from a code count.

So the renewal event should carry:

```
event   = "renewed"
detail  = "... (term_days=100, price=10.00, middleman=Sarah)"
```

This is **one string field, no schema change**, and it is what makes
[`06-console-and-operator.md`](06-console-and-operator.md) §6.6's middleman
ledger possible later without a data migration. It costs nothing now and cannot
be recovered retroactively if omitted.

> **Recommendation: include it from the first renewal.** Reconstructing a
> month's revenue from memory is exactly the kind of operational debt this
> project has otherwise avoided.

## 8.5 What the console should show, concretely

A minimal, honest set — chosen because each maps to a decision the operator
actually makes:

| Display | Source | Decision it supports |
|---|---|---|
| Codes expiring in 7 days | `dashboard.expiringSoon`, narrowed | **"Who do I need to chase?"** — the renewal worklist |
| Codes expiring in 30 days | `dashboard.expiringSoon` (exists) | Pipeline planning |
| In-term codes, by tier | `dashboard.byTier` (exists) | Stock: how many of each to mint |
| Renewals this month | new, from `code_events` | Whether the model works |
| Codes per middleman | `middlemen.list` (exists) | Attribution |
| Suspended | `dashboard` (exists) | Abuse/refunds |

**Explicitly not added:** any trend line, cohort, funnel, or per-user usage
chart. `../16-metrics.md` §16.4 already says none of this is measured, and
inventing a dashboard the data cannot populate is worse than not having one.

## 8.6 Related reading

* The renewal event being counted → [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4
* Why identity limits conversion measurement → [`01-identity-without-pii.md`](01-identity-without-pii.md)
* Where these are displayed → [`06-console-and-operator.md`](06-console-and-operator.md)
* The metrics file being made honest → [`09-impact-on-the-live-plan.md`](09-impact-on-the-live-plan.md)
