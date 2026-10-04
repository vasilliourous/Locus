# 8. Distribution — the middleman network

```
audience:    human-operator
status:      live
authoritative-for: how codes reach students, and the middleman's economics
verified-against: docs/STATE.md
```

## 8.1 The model

```
Operator (hub owner)
   │  mints code batches in the admin console,
   │  hands them to middlemen, keeps the hub
   │
   ├── Middleman A ── sells cards to students in their circle, takes a cut
   ├── Middleman B ── same
   └── Middleman C ── same
```

## 8.2 Why friends-as-middlemen works here

- **Trust gap closed.** The operator knows the middlemen. Cash collections
  are reliable, feedback is direct, and fraud risk is low.
- **Middlemen are the delivery channel for something intangible.** A paper
  card with a code on it is the physical artifact of a digital product.
- **Middlemen have no app access and no admin panel.** They hold stock, not
  system access.

## 8.3 What the system actually supports (verified)

- **Attribution is real and set at mint time.** `codes.generate` accepts a
  `middleman` field, and every code row stores it.
- **`middlemen.list`** returns the distinct middleman labels in use, with a
  code count per label — a ready-made "who sold how many" report.
- **A first-batch stamper exists** (`stamp-batch.py`) for producing cards.
- **Reporting is a filter.** There is no middleman dashboard beyond
  `middlemen.list`; per-middleman detail is a `codes.list` filter.
- **Renewals can carry attribution too** (`9a91da1`). `codes.renew` records the
  middleman and price in the audit detail when the operator supplies them.
  Worth supplying: once a code can be renewed, a code *count* no longer tells
  you what a middleman collected, because the same code can be sold repeatedly.

## 8.3.1 The middleman's one job the system cannot do for them

**Write the buyer's name into the code's `label` at the point of sale.**

This is now load-bearing. Because the hub stores no personal data
([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md)),
the link between a student and their code lives on **the middleman's paper and
in `label`**. When a student calls — "my code stopped working" — the operator
finds them by searching that name. A code sold without a label is a code whose
owner cannot be found, which is precisely when it will matter.

`label` is set at mint time in the console (`codes.generate` accepts it), so
this can be done for a whole batch, and `codes.update` can amend it later.

## 8.4 Middleman economics

- Typical cut: **20–30%** ([`05-pricing.md`](05-pricing.md#56-the-middleman-cut)).
- **Revenue concentration is the structural risk** — if 80% of revenue comes
  from 2–3 people, losing one (graduation, moving, getting caught) cuts
  revenue 30–40%.
- **Mitigation: always keep 3+ active middlemen**, and keep recruiting.

## 8.5 How the free tier changes middleman work

This is new, and it is important.

- **A middleman earns nothing on a free code.** The cut is a percentage of a
  paid price, and free has no price. If free codes were pushed onto middlemen
  as stock, they would be distributing unpaid work.
- **So free codes are the operator's to give, not stock to sell.** Free
  should be handed out by the operator (or by whoever is seeding adoption),
  and the middleman should only ever hold *paid* stock.
- **A free code in a middleman's hands is a foot-in-the-door for the paid
  sale.** The natural pitch is: *"try it free; when you want it fast, it's
  $4."* The middleman converts, they do not seed.
- **The post-block window is the operator's moment, not the middleman's.**
  When a free VPN dies, free codes should flow immediately and centrally
  ([`02-market.md`](02-market.md#24-the-timing-window), [`11-growth.md`](11-growth.md)).

> **Net effect:** the free tier increases the *addressable* population a
> middleman can sell into, while lowering the average transaction the
> operator sees per head. The middleman's job shifts from "sell the cheapest
> thing" to "convert the free install", which is a harder sell but a bigger
> market.

## 8.6 Guardrails

- **Never bulk-delete codes or `code_events`.** The live hub holds real paid
  codes in active use. Use suspend / unbind instead
  ([`10-lifecycle.md`](10-lifecycle.md)). *(The count and the distributor's name
  are deliberately not recorded — they decay and they identify a customer; see
  [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §4.)*
- **Tier passwords are deliberately not console-editable.** Changing one
  instantly breaks every issued client. It is a documented operation only.
