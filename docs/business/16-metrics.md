# 16. Metrics

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
> With one code per device enforced, that is a close approximation, not a
> headcount, and the console should not imply precision it lacks.
>
> **Free → paid conversion remains unmeasurable** — unchanged, and see §16.3.

## 16.3 The one metric that now matters most

With the free tier, **free → paid conversion is the number that validates or
kills the pricing strategy** ([`05-pricing.md`](05-pricing.md#541-how-this-compares-to-the-old-ladder),
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

**So conversion must be computed by hand**, from code counts over time. If it
becomes important, that is the first thing worth instrumenting
([`18-open-items.md`](18-open-items.md)).

## 16.4 What is *not* measured at all

- Time-series / trend of any kind.
- Churn.
- Per-user bandwidth.
- Revenue (inferred from code counts and the price ladder, by hand).
- Money owed to or by a middleman ([`07-billing.md`](07-billing.md#74-cash-handling)).

> **There is no analytics beyond the dashboard.** If a metric matters, it is
> computed by hand from `dashboard` or a `codes.list` filter.
