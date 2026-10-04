# 6. Unit economics and cost base

```
audience:    human-operator
status:      live
authoritative-for: the cost base, breakeven, and which estimates are unvalidated
verified-against: docs/STATE.md
```

## 6.1 The cost base

| Item | Cost | Source |
|---|---|---|
| Live VPS | DigitalOcean, 1 vCPU / 512 MB + 1 GB swap, Sydney — **~$6/mo** | `docs/operate/DEPLOY.md` |
| Alt. hosts | Hetzner CCX13 ~$8/mo; Vultr ~$12/mo | `docs/operate/DEPLOY.md` |
| Payment processing | **$0** — cash via middlemen | by design |
| Advertising | **$0** — organic + middlemen | by design |
| Salaries, office, inventory, debt | **$0** | by design |
| **Total fixed cost** | **≈ $6/month** | |

Breakeven is **1–2 paying Stealth users**. This is the most important
economic fact in the directory: the business is cash-positive almost
immediately, and the risk is not running out of money — it is that the
operator stops caring.

## 6.2 Cost does not set price

Because cost is cents per user, **price is purely willingness-to-pay**
([`05-pricing.md`](05-pricing.md#51-what-sets-the-price-and-what-does-not)). Do not cost-plus this product.

## 6.3 The free tier's cost

Free users have no revenue but real cost. At 1 Mbps / 5 GB:

| Free users | Max monthly transfer at full quota | Notes |
|:--:|:--:|---|
| 100 | ~500 GB | Trivially fine. |
| 1,000 | ~5 TB | **This is the number to watch.** |

- **Per user, free is cheap.** 5 GB is small.
- **In aggregate, free is a bandwidth question, not a hardware one.** A free
  user can never exceed 1 Mbps, so they cannot crowd out a Strike gamer on
  *rate*; the exposure is the droplet's monthly transfer allowance.
- **Mitigation if it grows:** tighten the cap or the allowance. Removing the
  tier is the last resort, not the first.

See [`04-tiers.md`](04-tiers.md#446-the-bandwidth-cost-of-free-users) and [`14-risks.md`](14-risks.md).

## 6.4 The estimate that must stay labelled

> **UNVALIDATED ESTIMATE.** The legacy figure "50–100 concurrent users per
> VPS on 1 vCPU" is **not measured anywhere in this repo**. Nothing
> benchmarks it. Treat it as a guess and validate it empirically before
> relying on it for anything.

This matters more now than it did before the free tier: a free tier is
*designed* to push concurrency up, and the ceiling that governs it is exactly
the number nobody has measured. If it is wrong, it is wrong in the direction
that hurts free users first.

## 6.5 What the paid tiers actually pay for

A useful reframe for pricing decisions: a paid user is buying three things,
in this order.

1. **The removal of the free cap** (1 Mbps → 100/200 Mbps). This is what most
   of the $4 buys.
2. **The removal of the data allowance** (5 GB → unmetered). Closely tied to
   (1).
3. **UDP / low latency** (Strike only). This is what the extra $3 buys.

That ordering is why Stealth at $4 is the volume tier and Strike is the
premium add-on.
