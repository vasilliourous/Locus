# 19. Cross-reference index

```
audience:    human-operator
status:      live
authoritative-for: question → file, and the one canonical home per topic
verified-against: docs/ (the link graph itself)
```

## By question

| Question | File |
|---|---|
| What is this business? | [`01-overview.md`](01-overview.md) |
| Who buys it and why? | [`02-market.md`](02-market.md) |
| What am I actually selling? | [`03-product.md`](03-product.md), [`04-tiers.md`](04-tiers.md) |
| **How does a student keep access / pay again?** | [`07-billing.md`](07-billing.md#75-the-renewal-routine--the-operational-job-this-creates), [`10-lifecycle.md`](10-lifecycle.md) |
| **What does "a term" mean, and how do I renew?** | [`10-lifecycle.md`](10-lifecycle.md), [`07-billing.md`](07-billing.md#75-the-renewal-routine--the-operational-job-this-creates) |
| **My student changed laptops — what do I do?** | [`10-lifecycle.md`](10-lifecycle.md#100-one-code-one-activation--and-almost-no-support-cost) — usually *nothing*; the code is not device-bound |
| **What happens if a student loses their card?** | [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md) — the client keeps the code, so they usually still have access |
| What does the free tier do? | [`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full), [`11-growth.md`](11-growth.md#113-the-free-tier-as-the-acquisition-engine) |
| What do I charge? | [`05-pricing.md`](05-pricing.md), [`07-billing.md`](07-billing.md) |
| What does it cost me? | [`06-unit-economics.md`](06-unit-economics.md) |
| How does it reach students? | [`08-distribution.md`](08-distribution.md) |
| How do I run it? | [`09-console.md`](09-console.md), [`10-lifecycle.md`](10-lifecycle.md) |
| How does it grow? | [`11-growth.md`](11-growth.md) |
| How big can it get? | [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md) |
| Why won't competitors kill it? | [`13-competition.md`](13-competition.md) |
| What could end it? | [`14-risks.md`](14-risks.md) |
| What survives a firewall change? | [`15-continuity.md`](15-continuity.md) |
| What can I actually measure? | [`16-metrics.md`](16-metrics.md) |
| What's missing? | [`17-not-built.md`](17-not-built.md) |
| What should I decide? | [`18-open-items.md`](18-open-items.md) |

## Where the design rationale lives

The term model and renewal were designed in the `redesign/` corpus and
implemented in `9a91da1`; their **live** description is now the numbered files
above ([`10-lifecycle.md`](10-lifecycle.md), [`07-billing.md`](07-billing.md),
[`09-console.md`](09-console.md)). The corpus itself is the **design record**,
including the device-binding design that was later **removed** — see
[`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md) for the
model that actually shipped. Read the corpus for *why*, never as a spec for
*what is*: [`redesign/00-README.md`](redesign/00-README.md) carries its own
status.

## By decision

| Decision | Where it is justified |
|---|---|
| Free replaces Eco | [`04-tiers.md`](04-tiers.md#422-eco-is-replaced-by-free) |
| Stealth and Strike merge into one paid plan | [`04-tiers.md`](04-tiers.md#424-stealth-and-strike-are-merged-into-one-paid-tier) |
| The plan is $0 / $5, with no ladder | [`05-pricing.md`](05-pricing.md#52-the-offer) |
| The survivor is called `strike` on the box | [`04-tiers.md`](04-tiers.md#46-the-same-choice-made-again-for-stealth--merged-into-full) |
| The free allowance is 10 GB | [`05-pricing.md`](05-pricing.md#55-why-the-free-allowance-is-10-gb-and-not-5) |
| Free is a data cap, not a user cap | [`04-tiers.md`](04-tiers.md#442-why-a-data-allowance-not-a-user-cap) |
| Free quota is enforced client-side | [`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means) |
| Free throttles, it does not cut off | [`04-tiers.md`](04-tiers.md#445-what-throttled-further-means) |
| Eco is reused in config only | [`04-tiers.md`](04-tiers.md#45-the-deploy-time-choice-renaming-eco--free) |
| Referral is rejected | [`11-growth.md`](11-growth.md#112-referral-cards--re-derived-and-rejected) |
| Bundling is worth reconsidering | [`11-growth.md`](11-growth.md#114-tier-bundling--reconsider-dont-dismiss) |

## By the three sentences

| Sentence | Files it drives |
|---|---|
| "We are the option that still works on this network." | [`02-market.md`](02-market.md), [`13-competition.md`](13-competition.md) |
| "Revenue comes from volume, not margin." | [`05-pricing.md`](05-pricing.md), [`06-unit-economics.md`](06-unit-economics.md) |
| "This is a side project that pays for itself." | [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md) |

---

*Supersedes `docs/history/Business-Plan.md` (pre-V5) and
`legacy/v4/BUSINESS.md` (V4), both background only. Every checkable claim in
this directory rests on the tree at the commit recorded in
[`../STATE.md`](../STATE.md); where the legacy prose disagrees, the code wins.*
