# 11. Growth

```
audience:    human-operator
status:      live
authoritative-for: the growth channels, and the referral verdict
verified-against: docs/state.toml
```

## 11.1 What exists, and what carries the weight

| Channel | Status | Notes |
|---|---|---|
| **Middleman recruitment** | **LIVE — the engine** | Each middleman is a distribution node *and* the referral mechanism (§11.2). Recruit continuously. |
| **The free tier** | **NEW — the funnel** | A free app can reach students no price point could. See [`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full). |
| **Word of mouth** | LIVE (organic) | The product working well is the marketing. |
| **Riding free-VPN blocks** | LIVE (opportunistic) | When school IT kills a free VPN, displaced students need a replacement. |
| **Tier bundling** (Full → free code) | **NOT BUILT — reconsider §11.4** | Survives the carrier critique; kept as proposal P3 ([`18-open-items.md`](18-open-items.md)). |
| **Referral cards** | **REJECTED — §11.2** | Legacy mechanic. Does not fit the distribution model. |

## 11.2 Referral cards — re-derived, and rejected

The legacy plans treated referral as the scalable growth channel: a student
gives a friend a card, the friend gets a discount, the referrer gets a free
week. Re-examined against *this* project's distribution, it does not fit.

**The carrier problem is fatal.** A referral needs a redeemable token that
travels referrer → referee → redemption. Here, redemption is a code typed
into a desktop app, and **every code must come from a human middleman** —
there is no self-signup ([`03-product.md`](03-product.md)). So the discount
would have to be printed on a physical card, traced back to a referrer, and
reconciled by hand at the scale of this business. That is overhead, not
leverage.

**The middleman already *is* the referral mechanism.** A middleman earns
20–30% cash ([`08-distribution.md`](08-distribution.md#84-middleman-economics)) for exactly the
work a referral programme would add. Referral would layer a *second*,
weaker incentive on top of one that already works — with a worse fraud
surface (friendship networks).

**Verdict:** **not applicable to this distribution model.** Do not build
`referred_by`. Recorded in [`17-not-built.md`](17-not-built.md).

> **What would change the verdict.** The free tier changes one input: there
> is now something to *give away* without a discount mechanism. That is
> bundling (§11.4), not referral.

## 11.3 The free tier as the acquisition engine

The free tier changes the growth model in three ways:

1. **It removes the price objection from the first install.** A student who
   would never ask a parent for $5 will install a free app today.
2. **It makes the block-window response instant.** When a free VPN dies, the
   operator can hand out free codes immediately and centrally
   ([`02-market.md`](02-market.md#24-the-timing-window), [`08-distribution.md`](08-distribution.md#85-how-the-free-tier-changes-middleman-work)).
3. **It gives word-of-mouth something to spread.** "It's free" travels
   further than "$5/month".

**But it also creates the new risk:** free adoption that never converts is a
bandwidth cost with no upside ([`14-risks.md`](14-risks.md),
[`06-unit-economics.md`](06-unit-economics.md#63-the-free-tiers-cost)). The growth metric that
matters is therefore not free installs — it is
**free → paid conversion** ([`16-metrics.md`](16-metrics.md)).

## 11.4 Tier bundling — reconsider, don't dismiss

Bundling (a Full buyer gets a free code to give away) is **not** the same
as referral, and survives the carrier critique because the *buyer* carries
the code they already have:

- It spreads the product for free via social proof ("everyone's on it").
- A free user experiences the 1 Mbps cap → a real upgrade path.
- Marginal cost is close to zero — free is a cap, not a new server.

**But it is not free:** free users consume bandwidth and a Full ticket
subsidises them, and it is unbuilt. **Verdict: the one legacy growth mechanic
worth a second look**, kept as proposal P3
([`18-open-items.md`](18-open-items.md)).

## 11.5 The honest growth position

> **Recruit and retain middlemen, and let the free tier widen the top of the
> funnel. That is the growth strategy.**

Everything else is either organic (word of mouth, riding blocks) or an
experiment. This matches [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md):
a side project at pocket-money scale does not need a growth machine — it
needs enough middlemen that losing one does not hurt.
