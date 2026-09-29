# 5. Pricing

> **Status: decided (this session).** The ladder is **$0 / $4 / $7**, or
> **Free / Stealth / Strike**. The reasoning below is kept so the numbers can
> be re-justified rather than merely re-stated.

---

## 5.1 What sets the price (and what does not)

- **Cost does NOT set the price.** Cost per user is cents
  ([`06-unit-economics.md`](06-unit-economics.md)). Pricing on cost would be
  absurd. **Price is set by willingness-to-pay**, and the ceiling is *what a
  high-school student has in pocket*.
- **The value proposition sets the ceiling.** Strike is the only thing on
  this network that games. That is a strong, specific, hard-to-substitute
  benefit — it can bear a higher price than "a VPN" in the abstract.
- **The floor is a sanity check only:** every paid tier must clear its share
  of the ~$6/mo VPS many times over.

---

## 5.2 The ladder

| Tier | Price | Step from previous | Why |
|---|:--:|:--:|---|
| **Free** | **$0** | — | The funnel. See [`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full). |
| **Stealth** | **$4** | free → paid | The volume seller. Cheapest possible paid entry that still funds the business. |
| **Strike** | **$7** | 1.75× | Sole carrier of the gaming premium ([`04-tiers.md`](04-tiers.md#43-what-still-makes-the-tiers-work-together)). A small gap on purpose — see §5.3. |

---

## 5.3 The decisive principles

### 5.3.1 Revenue comes from volume, not margin

> **Revenue comes from volume in the cheap tiers, not margin at the top.**
> Optimising the top tier optimises the tier that matters least.

This is why Strike is $7 and not $12. A high Strike price buys a premium on
a minority of users while risking the thing that actually sells the product.

### 5.3.2 The gap that matters is free → paid, not paid → paid

With Free at the bottom, the funnel's job is to convert a free user into a
**paying** user. Once they are paying $4, moving to $7 is a trivial decision
— so the paid gap is deliberately **small** (1.75×).

> **This supersedes the earlier "widen the gaps" steer.** That steer was
> correct when the ladder was three paid tiers and the dead Brutal mechanism
> used to do the differentiating. With free replacing the cheap anchor, the
> paid tiers no longer need to differentiate themselves from a $2 sibling;
> they only need to be an easy yes.

### 5.3.3 The real anchor is $0

The competition's price anchor is **free** (the free VPNs,
[`13-competition.md`](13-competition.md)), not the legacy $8. Every paid
price is judged against "I could just not pay."

### 5.3.4 The free gap is huge, and that is the point

1 Mbps + 5 GB → 100 Mbps unmetered is a step change, not a speed bump. That
size is what makes $4 an easy conversion
([`04-tiers.md`](04-tiers.md#42-what-changed-from-every-legacy-document)).

---

## 5.4 Sensitivity

Revenue at several scale points on the $0 / $4 / $7 ladder. Only paid users
contribute, so the free mix matters as much as the headcount.

| Total users | Mix (Free/Stealth/Strike) | Paid users | Gross/mo | After ~25% middleman | Net/mo |
|:--:|:--:|:--:|:--:|:--:|:--:|
| 25 | 55 / 30 / 15 | 11 | $56 | ~$42 | **~$36** |
| 50 | 50 / 33 / 17 | 25 | $126 | ~$94 | **~$88** |
| 100 | 45 / 35 / 20 | 55 | $280 | ~$210 | **~$204** |
| 250 | 40 / 38 / 22 | 150 | $765 | ~$574 | **~$568** |
| 1,000 | 35 / 40 / 25 | 650 | $3,350 | ~$2,513 | **~$2,507** |

*(Gross = Σ paid users × price. Middleman cut 20–30%; 25% used here. Net
subtracts the ~$6 VPS.)*

### 5.4.1 How this compares to the old ladder

At 50 users on the same mix, the **old** $2/$5/$11 ladder produced ~$164 net;
the new Free/$4/$7 ladder produces ~$88. **Per head, the new plan earns
roughly half.**

That is the trade, and it is deliberate:

- **Lower revenue per user, in exchange for a much larger funnel.** A free
  tier can reach students no price point could.
- **The bet is that headcount grows faster than the per-head revenue falls.**
  The 1,000-user row shows what that looks like if it works: ~$2,500/mo net,
  with no tier above $7.

> Whether that bet pays is **unmeasured** — there is no retention or
> conversion data ([`16-metrics.md`](16-metrics.md)). Treat the conversion
> rates in the mix column as assumptions to be validated, not forecasts.

### 5.4.2 What would make this wrong

- **If free users never convert**, headcount grows and revenue does not: the
  free tier becomes a bandwidth cost with no upside
  ([`14-risks.md`](14-risks.md)).
- **If the free cap is too generous**, the same thing happens with data as
  the currency instead of conversion.

Both are worth watching from day one of the free tier.

---

## 5.5 Term passes

NZ school terms run ~10 weeks, four per year. Term passes improve cash flow,
cut collection frequency, and are holiday-proof
([`07-billing.md`](07-billing.md)).

| Tier | Monthly ×3 (10 wks) | **Term pass** | Saving | Effective monthly |
|---|:--:|:--:|:--:|:--:|
| Stealth | $12 | **$10** | $2 | ~$3.33 |
| Strike | $21 | **$19** | $2 | ~$6.33 |

The discount is deliberately modest — enough to reward commitment and pull
cash forward, not enough to cannibalise the monthly price. There is no term
pass for Free, by definition.

> **These are now sellable, and they were not before.** Until `9a91da1` a code
> stored one absolute `expires_at` fixed at *mint*, so "$10 per term" had no
> mechanism behind it — the only expressible product was a single date. A term
> pass is now a code minted with `term_days ≈ 70`; a month is the same code with
> `term_days ≈ 30`. The ladder above is therefore a choice of *number*, not of
> machinery ([`07-billing.md`](07-billing.md)).
>
> The term is measured from **activation**, not from mint, which matters for
> pricing honesty: a card printed in advance is worth its full term whenever it
> is sold, so a batch can be printed before a term starts without decaying.

---

## 5.6 The middleman cut

- Typical cut: **20–30%** (legacy figure; still a reasonable default). §5.4
  assumes 25%.
- This is the largest single variable in the operator's net. Paying a higher
  cut to keep a good middleman is usually better than losing the volume,
  especially given the concentration risk
  ([`08-distribution.md`](08-distribution.md#84-middleman-economics)).
- **The cut applies to paid tiers only.** A middleman earns nothing on a free
  code — which means free codes are pure cost to whoever distributes them,
  and should be handed out by the operator, not pushed onto middlemen as
  stock. See [`08-distribution.md`](08-distribution.md#85-how-the-free-tier-changes-middleman-work).

---

## 5.7 Related reading

- What each tier *is* → [`04-tiers.md`](04-tiers.md)
- What it costs to serve → [`06-unit-economics.md`](06-unit-economics.md)
- How it gets collected → [`07-billing.md`](07-billing.md), [`08-distribution.md`](08-distribution.md)
