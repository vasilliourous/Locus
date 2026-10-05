# 5. Pricing

```
audience:    human-operator
status:      live
authoritative-for: the price ladder, and the reasoning behind each step
verified-against: docs/state.toml
```

> **Status: decided (this change).** The offer is **two plans**: **Free at $0**
> and **Full at $5/month**. The three-step ladder ($0/$4/$7) that this file used
> to describe is gone, and the reasoning below is kept so the number can be
> re-justified rather than merely re-stated.

---

## 5.1 What sets the price (and what does not)

- **Cost does NOT set the price.** Cost per user is cents
  ([`06-unit-economics.md`](06-unit-economics.md)). Pricing on cost would be
  absurd. **Price is set by willingness-to-pay**, and the ceiling is *what a
  high-school student has in pocket*.
- **The value proposition sets the ceiling.** Full is the only thing on this
  network that plays games *well*. Note what changed: it is no longer that Full
  is the only plan that plays games at all — both do, because UDP is a given
  ([`04-tiers.md`](04-tiers.md#431-why-udp-is-given-away-rather-than-sold)). $5
  buys **100× the rate**, which is the difference between a game that connects
  and a game worth playing. That is a weaker-sounding but more honest claim, and
  it is the one the product can actually keep.
- **The floor is a sanity check only:** the paid plan must clear its share
  of the ~$6/mo VPS many times over. At $5, two paying students pay for the hub.

---

## 5.2 The offer

There is no ladder. There are **two plans and one price**:

| Plan | Price | What it is |
|---|:--:|---|
| **Free** | **$0** | 1 Mbps, 10 GB/month, then throttled, **UDP included**. The funnel — an install, not a destination. See [`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full). |
| **Full** | **$5** | 100 Mbps, unmetered, **UDP included**. The whole paid product. |

> **Why one paid tier, and not two.** Stealth and Strike were the same plan at
> two rates — the same kernel, the same unmetered data, differing only by the
> UDP endpoint and a 200 Mbps cap no game could use
> ([`04-tiers.md`](04-tiers.md#424-stealth-and-strike-are-merged-into-one-paid-tier)).
> A second paid tier asked the student to make a comparison that had no real
> answer, so the merge deletes the question instead of answering it.

---

## 5.3 The decisive principles

### 5.3.1 Revenue comes from volume, not margin

> **Revenue comes from volume in the cheap plan, not margin at the top.**
> Optimising a premium tier optimises the tier that matters least.

This is why Full is **$5** and not $12. A high premium price buys margin on a
minority of users while risking the thing that actually sells the product, and
there is no longer a second tier for that margin to hide behind.

> **The risk this raise creates, stated plainly.** Giving UDP away removed the
> one *binary* reason a light user had to pay — a student who only played
> Roblox now has a working free option. The bet is that a working free game
> creates the *desire* for a good one, which is a conversion argument rather
> than a capability argument, and it is unmeasured. If conversion collapses,
> §5.4.2's remedy is a price change, not re-gating UDP: a plan whose games are
> broken is worse at its only job than a plan that is merely slow.

### 5.3.2 The gap that matters is free → paid, and it is the only gap

With one paid tier there is exactly **one** decision a student makes: pay or
don't. That is the honest shape of this product — there was never a meaningful
second choice — and it means every pricing effort belongs on the free→paid step:

- **Make Free genuinely useful**, so the app stays installed
  ([`11-growth.md`](11-growth.md)).
- **Make the step obvious**, which the 1 Mbps → 100 Mbps jump does on its own.

> **This supersedes "the paid gap is small on purpose."** That principle was
> correct for a two-tier paid ladder. With one paid tier it is not small — it
> does not exist.

### 5.3.3 The real anchor is $0

The competition's price anchor is **free** (the free VPNs,
[`13-competition.md`](13-competition.md)), not the legacy $8. Every paid
price is judged against "I could just not pay." Free is also now *our* offer, so
the comparison the student makes is not "Locus vs a free VPN" but "Lose the data
limit, or keep it" — a comparison the product controls.

### 5.3.4 The free gap is huge, and that is the point

1 Mbps + 10 GB → 100 Mbps unmetered is a step change, not a speed bump.
That size is what makes $5 an easy conversion, and it is the only lever that
does not require the paid price to move. (UDP used to be on this list and is not
any more — both plans carry it, so the *rate* is doing the whole job.)

---

## 5.4 Sensitivity

Revenue at several scale points on the **Free / Full** offer. Only paid users
contribute, so the free mix matters as much as the headcount.

| Total users | Mix (Free/Full) | Paid users | Gross/mo | After ~25% middleman | Net/mo |
|:--:|:--:|:--:|:--:|:--:|:--:|
| 25 | 60 / 40 | 10 | $50 | ~$38 | **~$32** |
| 50 | 55 / 45 | 23 | $115 | ~$86 | **~$80** |
| 100 | 50 / 50 | 50 | $250 | ~$188 | **~$182** |
| 250 | 45 / 55 | 138 | $690 | ~$518 | **~$512** |
| 1,000 | 40 / 60 | 600 | $3,000 | ~$2,250 | **~$2,244** |

*(Gross = paid users × $5. Middleman cut 20–30%; 25% used here. Net subtracts
the ~$6 VPS.)*

### 5.4.1 How this compares to the ladders before it

Same 50 users, three generations of this file:

| Offer | Paid users at 50 | Net/mo | Per paying user |
|---|:--:|:--:|:--:|
| Legacy $2/$5/$11 | ~35 | ~$164 | $4.7 |
| Previous Free/$4/$7 | ~25 | ~$88 | $3.5 |
| **Now: Free/$5** | **~23** | **~$80** | **$3.5** |

**Per head, this earns roughly half what the legacy ladder did**, and that is
the deliberate trade this directory has made twice now:

- **Lower revenue per user, in exchange for a funnel that reaches students no
  price point could.**
- **The bet is that headcount grows faster than the per-head revenue falls.**
  The 1,000-user row shows what that looks like if it works: ~$2,200/mo net,
  with no price above $5.

> **The merge makes this slightly worse, not better.** Two paid tiers at $4 and
> $7 collected $15 from every three paying students who split across them; one
> $5 tier collects $15 from the same three only if all three pay the same price —
> and it collects **$4 from the student who used to buy Stealth**, which is the
> real cost of deleting a tier. The compensation is that the student who used to
> buy Strike now pays $5 instead of $7, so the tier is easier to sell and there
> is one fewer thing to explain. Whether that nets out is **unmeasured**.

> Whether any of this pays is **unmeasured** — there is no retention or
> conversion data ([`16-metrics.md`](16-metrics.md)). Treat the mix column as
> assumptions to be validated, not forecasts. The mixes here are *more*
> pessimistic than the previous file's, because the Free→Full step is now the
> only step and it is the hardest one.

### 5.4.2 What would make this wrong

- **If free users never convert**, headcount grows and revenue does not: the
  free tier becomes a bandwidth cost with no upside
  ([`14-risks.md`](14-risks.md#143-free-to-paid-conversion--the-strategic-risk)).
- **If the free allowance is too generous**, the same thing happens with data as
  the currency instead of conversion. 10 GB is now a *number worth advertising*
  rather than a token, so this risk is larger than it was at 5 GB — the cap still
  binds first for almost every user (§5.5), and that is what keeps it safe.
- **If $5 is above what a student will pay for one plan.** The merge removed the
  $4 option that was the volume seller. If the conversion rate at $5 collapses,
  the fix is a price cut, not a new tier.

---

## 5.5 Why the free allowance is 10 GB and not 5

The free plan's allowance was raised from 5 GB to 10 GB in the same change that
merged the paid tiers. Three reasons:

1. **It is the marketing number.** "Free with a 10 GB monthly limit" is a
   sentence a student can repeat; "5 GB" is not obviously different from the
   other free VPNs' fine print.
2. **The speed binds first, so 10 GB is cheaper than it looks.** At 1 Mbps, 10 GB
   is about 22 hours of continuous saturated traffic. A student using Free for
   chat and browsing will not reach it; one trying to stream will, which is
   exactly the intended boundary.
3. **It widens the funnel without weakening the paid case.** The paid tier is
   still 100×  the speed and unmetered, so the step remains enormous — see §5.3.4.

**What it costs** is in [`06-unit-economics.md`](06-unit-economics.md#63-the-free-tiers-cost):
at 1,000 free users the ceiling is ~10 TB/month. That is an infrastructure cost,
not a product one, and §5.4.2 names it as the thing to watch.

---

## 5.6 Term passes

NZ school terms run ~10 weeks, four per year. Term passes improve cash flow,
cut collection frequency, and are holiday-proof
([`07-billing.md`](07-billing.md)).

| Plan | Monthly ×3 (10 wks) | **Term pass** | Saving | Effective monthly |
|---|:--:|:--:|:--:|:--:|
| Full | $15 | **$12** | $3 | ~$4.00 |

The discount is deliberately modest — enough to reward commitment and pull
cash forward, not enough to cannibalise the monthly price. There is no term
pass for Free, by definition.

> **These are now sellable, and they were not before.** Until `9a91da1` a code
> stored one absolute `expires_at` fixed at *mint*, so "$12 per term" had no
> mechanism behind it — the only expressible product was a single date. A term
> pass is now a code minted with `term_days ≈ 70`; a month is the same code with
> `term_days ≈ 30`. The ladder above is therefore a choice of *number*, not of
> machinery ([`07-billing.md`](07-billing.md)).
>
> The term is measured from **activation**, not from mint, which matters for
> pricing honesty: a card printed in advance is worth its full term whenever it
> is sold, so a batch can be printed before a term starts without decaying.
>
> **This is also the console's default.** `Codes.vue`'s mint form defaults to
> `genTermDays = 70`, so the term pass is what the operator gets without thinking
> about it — which is the intended default ([`07-billing.md`](07-billing.md) §7.3).

---

## 5.7 The middleman cut

- Typical cut: **20–30%** (legacy figure; still a reasonable default). §5.4
  assumes 25%.
- This is the largest single variable in the operator's net. Paying a higher
  cut to keep a good middleman is usually better than losing the volume,
  especially given the concentration risk
  ([`08-distribution.md`](08-distribution.md#84-middleman-economics)).
- **The cut applies to paid plans only.** A middleman earns nothing on a free
  code — which means free codes are pure cost to whoever distributes them,
  and should be handed out by the operator, not pushed onto middlemen as
  stock. See [`08-distribution.md`](08-distribution.md#85-how-the-free-tier-changes-middleman-work).
- **The merge changes the pitch, not the cut.** `08-distribution.md` §8.5's
  script — *"try it free; when you want it fast, it's $5"* — is now the whole
  sales conversation. There is no second paid option for a middleman to explain,
  which makes this the simplest version of the offer the business has ever had.

---

## 5.8 Related reading

- What each plan *is* → [`04-tiers.md`](04-tiers.md)
- What it costs to serve → [`06-unit-economics.md`](06-unit-economics.md)
- How it gets collected → [`07-billing.md`](07-billing.md), [`08-distribution.md`](08-distribution.md)
- What is decided but not built → [`17-not-built.md`](17-not-built.md)
