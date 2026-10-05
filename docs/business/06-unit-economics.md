# 6. Unit economics and cost base

```
audience:    human-operator
status:      live
authoritative-for: the cost base, breakeven, and which estimates are unvalidated
verified-against: docs/state.toml
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

Breakeven is **2 paying Full users**. This is the most important
economic fact in the directory: the business is cash-positive almost
immediately, and the risk is not running out of money — it is that the
operator stops caring.

## 6.2 Cost does not set price

Because cost is cents per user, **price is purely willingness-to-pay**
([`05-pricing.md`](05-pricing.md#51-what-sets-the-price-and-what-does-not)). Do not cost-plus this product.

## 6.3 The free tier's cost

Free users have no revenue but real cost. At 1 Mbps / 10 GB:

| Free users | Max monthly transfer at full quota | Notes |
|:--:|:--:|---|
| 100 | ~1 TB | Trivially fine. |
| 1,000 | ~10 TB | **This is the number to watch.** |

- **Per user, free is cheap.** 10 GB is small.
- **In aggregate, free is a bandwidth question, not a hardware one.** A free
  user can never exceed 1 Mbps on *either* transport, so they cannot crowd out a
  Full gamer on rate; the exposure is the droplet's monthly transfer allowance.
- **Free UDP is capped at the hub, not just in the client.** The 8447 listener
  carries the same 1 Mbps `tc` class as the free TCP tunnel
  ([`04-tiers.md`](04-tiers.md#410-the-free-plans-udp-abuse-surface-accepted-and-bounded)),
  so the free plan's UDP ceiling is real even against a modified client. This is
  what keeps the table above meaningful now that UDP is on both plans: the
  ceilings are per-plan *hub* limits, not client promises.
- **Mitigation if it grows:** tighten the cap or the allowance. The allowance is
  a hook value, so it changes without a client release; the cap takes a
  `setup.sh` re-run. Removing the tier is the last resort, not the first.

> **The allowance doubled from 5 GB to 10 GB in the same change that merged the
> paid tiers** ([`05-pricing.md`](05-pricing.md#55-why-the-free-allowance-is-10-gb-and-not-5)),
> so the ceiling in this table doubled with it. Nothing about the *shape* of the
> cost changed: it is still a bandwidth exposure, still bounded by the tc cap,
> still something to watch rather than something to fear at this scale.

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

1. **The removal of the free cap** (1 Mbps → 100 Mbps). This is what most
   of the $5 buys.
2. **The removal of the data allowance** (10 GB → unmetered). Closely tied to
   (1).
3. **UDP / low latency**. This is the one thing the free tier cannot do and a
   free VPN cannot do here — it is the reason the paid tier exists at all
   ([`13-competition.md`](13-competition.md)).

That ordering is why the merge worked: the 200 Mbps cap that used to sit at the
top of this list was the **only** item no student could feel, and it is the item
that was deleted. Everything a paying student actually experiences — speed,
unmetered data, and UDP — is still there, at $5 instead of $7.
