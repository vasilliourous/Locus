# BUSINESS — Locus Operator Playbook

```
audience:    human-operator
status:      live
authoritative-for: the commercial plan and how to run the business
verified-against: docs/state.toml, docs/operate/CLAIMS.md
```

The commercial plan for the product as it actually ships (current client version
in [`../STATE.md`](../STATE.md)). Split into one file per section so each can be
read, edited, and cited on its own.

> **Status convention in this directory.** Every file here — this index, the 19
> numbered plan files and the nested READMEs — carries the standard front-matter
> block (`audience` / `status` / `authoritative-for` / `verified-against`),
> matching [`../README.md`](../README.md) rule 1. A file whose `status` is
> `design-record` is describing intent, not what is built; read
> `verified-against` to see what the file was checked against.

**The rule that governs every file here: the code wins.** Every checkable
claim was checked against the tree. Where the legacy prose disagrees with
the code, the legacy prose is the bug. Where a number is an estimate and not
a measurement, it says so. Where a legacy mechanism was never built, it says
**NOT BUILT** rather than quietly describing it as if it existed.

**Scope.** This is a *single-operator, single-school* side business. It is
not a startup, and it is not written as one. Read
[`12-scale-and-ceiling.md`](12-scale-and-ceiling.md) before proposing
anything that assumes growth.

---

## The files

| # | File | What it covers |
|---|---|---|
| 1 | [`01-overview.md`](01-overview.md) | The business in one page. Start here. |
| 2 | [`02-market.md`](02-market.md) | Who the customer is, and why the incumbents fail them. |
| 3 | [`03-product.md`](03-product.md) | What the product is: client, hub, activation code. |
| 4 | [`04-tiers.md`](04-tiers.md) | The tiers as they now are — including the free tier. |
| 5 | [`05-pricing.md`](05-pricing.md) | The price ladder, re-derived, with reasoning and sensitivity. |
| 6 | [`06-unit-economics.md`](06-unit-economics.md) | Cost base, breakeven, and the one unvalidated estimate. |
| 7 | [`07-billing.md`](07-billing.md) | Monthly vs term pass. |
| 8 | [`08-distribution.md`](08-distribution.md) | The middleman network. |
| 9 | [`09-console.md`](09-console.md) | The operator console — what the business runs on. |
| 10 | [`10-lifecycle.md`](10-lifecycle.md) | Expiry, suspension, unbind, audit trail. |
| 11 | [`11-growth.md`](11-growth.md) | Growth channels, and the referral verdict. |
| 12 | [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md) | How big this can get, and the honest ambition. |
| 13 | [`13-competition.md`](13-competition.md) | The moat, and the free-VPN competition. |
| 14 | [`14-risks.md`](14-risks.md) | Actionable risks and mitigations. |
| 15 | [`15-continuity.md`](15-continuity.md) | What survives what. |
| 16 | [`16-metrics.md`](16-metrics.md) | What the console actually reports. |
| 17 | [`17-not-built.md`](17-not-built.md) | Legacy claims that are prose only. |
| 18 | [`18-open-items.md`](18-open-items.md) | Proposals awaiting a decision. |
| 19 | [`19-cross-reference.md`](19-cross-reference.md) | Question → file index. |

---

## The design record

[`redesign/`](redesign/) holds the reasoning behind the **term model and
renewal** — designed, then implemented in `9a91da1`. Start at
[`redesign/00-README.md`](redesign/00-README.md).

**Read it for *why*, never as a spec for *what is*.** It is a design record with
its own status field, and parts of it were **superseded rather than shipped**:
its device-binding design (one code per device, a durable device identity) was
**removed** in 2026-10. The live model is a single-use, unbound code that the
client keeps in a machine store — [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md).

**Everything live in this directory is measured against a hub where a code
carries a term, a renewal is a real audited operation, and a code is single-use
and tied to nothing.** Where a file describes a fixed expiry date set at mint,
or a code bound to a device, it is stale.

---

## Where to start

- **New to the business?** [`01-overview.md`](01-overview.md) →
  [`02-market.md`](02-market.md) → [`04-tiers.md`](04-tiers.md).
- **Setting prices?** [`05-pricing.md`](05-pricing.md) →
  [`06-unit-economics.md`](06-unit-economics.md).
- **Running it day to day?** [`09-console.md`](09-console.md) →
  [`10-lifecycle.md`](10-lifecycle.md).
- **Worried about it ending?** [`13-competition.md`](13-competition.md) →
  [`14-risks.md`](14-risks.md) → [`15-continuity.md`](15-continuity.md).

---

*Supersedes `docs/history/Business-Plan.md` (pre-V5) and
`legacy/v4/BUSINESS.md` (V4), both of which remain as background only. Every
checkable claim in this directory rests on the tree at the commit recorded in
[`../STATE.md`](../STATE.md); where the legacy prose disagrees, the code wins.*
