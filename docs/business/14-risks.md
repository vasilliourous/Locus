# 14. Risks

```
audience:    human-operator
status:      live
authoritative-for: the actionable risks, and for each one what mitigates it
verified-against: docs/STATE.md
```

Only risks that call for an *action* are listed here. Background and context
live in the linked files.

---

## 14.1 Protocol detection — highest impact

| | |
|---|---|
| **Likelihood** | Low now, rising over time. |
| **Impact** | Critical — the product stops working. |
| **Mitigation today** | **Nothing automatic.** There is no in-product protocol swap ([`17-not-built.md`](17-not-built.md)). |

A firewall update that detects Shadowsocks TCP is an outage. The client can
refresh *config* on heartbeat, and the operator can edit `tier_configs` — but
a *new protocol* needs a client update, and the updater has never installed
one (`docs/reference/STILL-OPEN.md`).

**Action:** treat "swap protocol remotely" as a project, not a feature
([`18-open-items.md`](18-open-items.md)). See
[`15-continuity.md`](15-continuity.md).

---

## 14.2 Free-tier bandwidth — new, and the one to watch

| | |
|---|---|
| **Likelihood** | Medium, and rises with free adoption. |
| **Impact** | Medium — cost, not capability. Free users cannot crowd out paid users on *rate*. |
| **Mitigation** | Monitor the droplet's transfer allowance; tighten the cap or allowance if it grows. |

At 1,000 free users, full quota is ~5 TB/month
([`06-unit-economics.md`](06-unit-economics.md#63-the-free-tiers-cost)). **This is the free
tier's real risk, and it is an infrastructure cost, not a product one.**

**Action:** watch aggregate egress once free adoption grows; have a lower cap
ready.

---

## 14.3 Free-to-paid conversion — the strategic risk

| | |
|---|---|
| **Likelihood** | Unknown. Untested. |
| **Impact** | High — if free users never convert, the free tier is cost with no upside. |
| **Mitigation** | Watch the conversion metric from day one ([`16-metrics.md`](16-metrics.md)); tune the free limits if conversion is poor. |

The whole free-tier bet is that headcount grows faster than per-head revenue
falls ([`05-pricing.md`](05-pricing.md#541-how-this-compares-to-the-old-ladder)). **That bet is unmeasured.**

**Action:** instrument conversion before scaling free distribution.

---

## 14.4 School IT blocks the VPS IP

| | |
|---|---|
| **Likelihood** | Low–medium. |
| **Impact** | High — all users lose access at once. |
| **Mitigation** | Keep a spare VPS on a different provider/IP range ready. Failover = update DNS + `tier_configs`. |

---

## 14.5 Revenue concentration

Covered in [`08-distribution.md`](08-distribution.md#84-middleman-economics). **Mitigation: 3+
active middlemen, recruiting always.**

---

## 14.6 Renewal is manual — the risk the redesign introduces

| | |
|---|---|
| **Likelihood** | **High.** It is the default behaviour, not an edge case. |
| **Impact** | Medium per student, high in aggregate — lapsing students are unpaid students. |
| **Mitigation** | The console's *Expiring in 30 days* card and the 7-day worklist. **Nothing prompts the operator; they must look.** |

The hub holds no personal data by decision
([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md)),
so it cannot know a payment is due — it only knows a date is approaching.
There is no auto-renewal and no notification to the operator.

> **This is the largest operational risk in the current design.** Access ends
> when a term ends; if nobody renews, the student simply stops. A renew button
> nobody is prompted to press does not collect money.

**Action:** treat watching the worklist as a scheduled routine, not a
convenience ([`07-billing.md`](07-billing.md#75-the-renewal-routine--the-operational-job-this-creates)). If it proves unreliable, the
cheapest mitigation is an operator-facing notification, which needs an operator
contact address — not student data, so it is compatible with the no-PII rule.

---

## 14.7 Paper identity depends on a habit nobody enforces

| | |
|---|---|
| **Likelihood** | Medium — it is a human habit, not a control. |
| **Impact** | Medium — an unlabelled code cannot be found by a name, so a lost card becomes unrecoverable. |
| **Mitigation** | The console's search now reads `label`; the middleman routine must actually fill it. |

Because the hub stores no identity, the link between a student and a code lives
on **paper and in `label`/`notes`**. A code sold without a label is a code whose
student cannot be found when they call — which is exactly when they will call.
The system cannot enforce this, and that is deliberate
([`redesign/04-card-and-credential.md`](redesign/04-card-and-credential.md) §4.6).

**Action:** make "write the buyer's name in `label`" part of the middleman's
routine ([`08-distribution.md`](08-distribution.md)).

---

## 14.8 What the risks have in common

Four of the six are solved by *the same project*: the ability to change what
the product is, remotely, without a client update
([`15-continuity.md`](15-continuity.md)). That is the highest-leverage piece
of work available to this business.

The two new risks above share a different shape: **both are consequences of
storing no identity**, and both are mitigated by operator discipline rather
than by code. That is the price of the no-PII decision, and it is worth paying
— but it should be paid deliberately, which is why both are written down here
rather than left implicit.
