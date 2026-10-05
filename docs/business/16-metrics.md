# 16. Metrics

```
audience:    human-operator
status:      live
authoritative-for: what the console genuinely reports, and what must be computed by hand
verified-against: server/pb_hooks/admin_console.pb.js
```

Design metrics around what the console genuinely returns — do not invent
numbers the system does not have.

## 16.1 What the console reports

| Metric | Source |
|---|---|
| Total codes / available / bound / suspended / expired | `dashboard` |
| Breakdown by tier | `dashboard` |
| Expiring soon | `dashboard` |
| Codes per middleman | `middlemen.list` |
| Per-code event history | `codes.history` |

## 16.2 Derived business metrics (computed, not stored)

| Metric | How |
|---|---|
| Active subscribers | bound − suspended − expired |
| Effective ARPU | gross ÷ active paid subscribers |
| Retention | renewals ÷ expiries in the window |
| Middleman concentration | top middleman's codes ÷ total |
| **Free → paid conversion** | paid codes minted ÷ free users, over a window (§16.3) |

> **Status after `9a91da1`.** **Retention** now has a numerator: each renewal
> writes a `code_events` row with `event:"renewed"` and the before→after expiry,
> so it is countable rather than inferred. Its denominator is derived from
> `codes` by date filter, since nothing records an expiry *event* — expiry is
> computed on read, never written. That is deliberate: a date filter over rows
> that already exist is exact at this scale and needs no new writes
> ([`redesign/08-metrics-and-instrumentation.md`](redesign/08-metrics-and-instrumentation.md) §8.2.1).
>
> **Active subscribers** is now a precise definition (in term, not suspended)
> rather than a proxy, because the term model says what "active" means. Note
> the limit: this counts **codes, not people** — the hub stores no identity
> ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md)).
> With one code per student and codes not device-bound, a count of live codes is
> a close approximation of a headcount — but it is an approximation, and the
> console should not imply precision it lacks.
>
> **Free → paid conversion remains unmeasurable** — unchanged, and see §16.3.

## 16.3 The one metric that now matters most

With the free tier, **free → paid conversion is the number that validates or
kills the pricing strategy** ([`05-pricing.md`](05-pricing.md#541-how-this-compares-to-the-ladders-before-it),
[`11-growth.md`](11-growth.md#113-the-free-tier-as-the-acquisition-engine)). Everything else is bookkeeping.

It is also the hardest to measure here, because:

- The hub has **no per-user traffic accounting**
  ([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)), so free *usage* cannot be read from the
  server.
- Free users are indistinguishable from each other on the wire, so
  "an activated free user" is only knowable as "a free code that has been
  bound" (`dashboard`).
- There is **no cohort, no time-series, no funnel** in the console
  ([`09-console.md`](09-console.md#93-what-the-console-does-not-do)).

**So conversion must be computed by hand**, from code counts over time: a free
code that later appears as a paid code from the same middleman, over a window. If
it becomes important, that is the first thing worth instrumenting
([`18-open-items.md`](18-open-items.md)).

> **What the free tier did *not* fix about this.** Now that the client counts its
> own usage, the hub could in principle learn *something* — but only if the client
> reported its counter back, which it deliberately does not. Doing so would create
> exactly the per-user record this product's design refuses to keep
> ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md)).
> Conversion stays a hand-counted, code-to-code estimate, and the honest claim is
> that it is approximate.

## 16.4 What is *not* measured at all

- Time-series / trend of any kind.
- Churn.
- Per-user bandwidth.
- Revenue (inferred from code counts and the price ladder, by hand).

> **The one simplification the merge brought.** "Revenue" used to need a
> per-tier price table, because a code count told you nothing until you knew
> whether each code was Stealth or Strike. Now every paid code is worth the same
> $5, so **paid code count × $5** is the whole calculation. It is still
> hand-computed and still approximate (it cannot see refunds, suspensions, or
> term passes sold at the discount), but the arithmetic stopped being a source of
> error — a small, real operational win from deleting a tier.
- Money owed to or by a middleman ([`07-billing.md`](07-billing.md#74-cash-handling)).

> **There is no analytics beyond the dashboard.** If a metric matters, it is
> computed by hand from `dashboard` or a `codes.list` filter.
