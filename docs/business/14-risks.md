# 14. Risks

```
audience:    human-operator
status:      live
authoritative-for: the actionable risks, and for each one what mitigates it
verified-against: docs/state.toml
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
| **Mitigation** | Monitor the droplet's transfer allowance; tighten the cap or the allowance if it grows. The allowance is a hook value (`heartbeat.pb.js`), so it changes without a client release; the 1 Mbps cap takes a `setup.sh` re-run. |

At 1,000 free users, full quota is ~10 TB/month
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
falls ([`05-pricing.md`](05-pricing.md#541-how-this-compares-to-the-ladders-before-it)). **That bet is unmeasured.**

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

## 14.8 The tier merge strands a live endpoint, and nothing removes it

| | |
|---|---|
| **Likelihood** | **High, if the operator step is skipped** — the code will not do it. |
| **Impact** | Medium — an uncapped, unsold endpoint keeps working for anyone who already has the password. |
| **Mitigation** | An explicit operator step: disable and remove the retired `stealth` service and config ([`04-tiers.md`](04-tiers.md#47-what-this-merge-costs-at-deploy-time)). |

Merging Stealth into Full **retires** the `stealth` service, and the deployment
scripts are deliberately *skip-if-exists* — they exist so a re-run is idempotent,
not so they can delete things. So `/etc/shadowsocks/stealth.json` and
`shadowsocks-stealth.service` survive a `setup.sh` re-run and keep serving 8444
with a working password, while `04-tc.sh` no longer shapes that port.

That is a **bandwidth risk, not a security one**: the password was never secret
from students who bought Stealth, but the tier is now unsold, unlisted in the
console, and unmetered. The exposure is exactly the same as §14.2's — some bytes
off the droplet — with none of the revenue.

**Action:** after the merge, `systemctl disable --now shadowsocks-stealth` and
remove the unit and config. Verify with the read-only probe in
[`15-continuity.md`](15-continuity.md#153-what-this-merge-changed-about-continuity).

---

## 14.9 Free UDP is the widest abuse surface the free plan has

| | |
|---|---|
| **Likelihood** | Medium — it requires deliberately modifying the client, which is closed-source. |
| **Impact** | Low per user, medium in aggregate — real bytes with no revenue, on a plan whose only bound on usage is a client-side counter. |
| **Mitigation** | The free UDP listener carries its own **server-side 1 Mbps `tc` class** (8447), so the rate is enforced by the hub regardless of the client. `check-consistency.sh` §27(h) fails the build if that class is removed. |

UDP is on both plans now ([`04-tiers.md`](04-tiers.md#431-why-udp-is-given-away-rather-than-sold)),
which means the free plan carries game and voice traffic from the largest
population it has ever had. Its *quota* remains client-counted and soft.

**What the hub enforces:** the rate. A tampered client cannot exceed 1 Mbps of
UDP no matter what it claims. **What it does not enforce:** the 10 GB allowance,
which a tampered client can exceed indefinitely at that rate.

**Action:** recognise the symptom rather than hunt for it — a sustained ~1 Mbps
egress on 8447 from one source is the shape. The line between "one enthusiastic
Roblox player" and "one abuser" is thin at this rate, which is *why* the
tolerance is affordable: the worst case is roughly one continuous video stream's
worth of bandwidth. Per-user accounting (P4) is the real fix and is not yet worth
building.

---

## 14.10 What the risks have in common

Four of the original six are solved by *the same project*: the ability to change what
the product is, remotely, without a client update
([`15-continuity.md`](15-continuity.md)). That is the highest-leverage piece
of work available to this business.

The two identity-shaped risks share a different shape: **both are consequences of
storing no identity**, and both are mitigated by operator discipline rather
than by code. That is the price of the no-PII decision, and it is worth paying
— but it should be paid deliberately, which is why both are written down here
rather than left implicit.

§14.8 is a third shape, and worth naming because it recurs: **a merge removes
something from the code but not from the box.** Any future retirement of a tier
follows the same recipe — the tree stops creating it, and a human has to stop
the running one.
