# 9. The operator console

```
audience:    human-operator
status:      live
authoritative-for: the console's actions, and what it deliberately does not do
verified-against: server/pb_hooks/admin_console.pb.js
```

The real operator surface is the **admin console**, not any legacy "admin
panel" description.

- **Location:** `/admin/` — a Vue 3 + Vite SPA in `server/console/`.
- **API:** a single authenticated `POST /api/admin/console` with an `action`
  discriminator, in `server/pb_hooks/admin_console.pb.js`.
- **Auth:** `ADMIN_API_TOKEN` in the JSON body or the `X-Admin-Token` header.

## 9.1 Actions that exist today (verified)

| Group | Actions |
|---|---|
| Overview | `dashboard` |
| Codes | `codes.list` · `codes.generate` · `codes.suspend` · `codes.unsuspend` · `codes.renew` · `codes.set-term` · `codes.expire` · `codes.unbind` · `codes.history` · `codes.update` · `codes.delete` · `codes.deleteBatch` |
| Middlemen | `middlemen.list` |
| Devices | `devices.list` |
| Tiers | `tiers.list` · `tiers.update` |
| Releases | `releases.get` · `releases.fetchLink` · `releases.publish` · `releases.set` |

> **`codes.rebind` is retired, not listed.** The action name still exists in
> `admin_console.pb.js`, but it now returns **HTTP 410** — *"Rebinding is no
> longer needed: codes are not tied to a device."* It is kept so an operator
> following an old runbook gets a sentence instead of a silent no-op. See
> [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md).

**The three that run the business** (`9a91da1`):

* **`codes.renew`** — the payment operation. Extends from the later of today
  and the existing expiry, so paying early never loses days. Records
  `before → after` in the audit trail.
* **`codes.set-term`** — set how long one purchase lasts, without moving an
  expiry already running.
* **Search** — `codes.list`'s query now matches `label`/`notes`/`middleman` as
  well as the code. Finding a student by name was impossible before, which made
  every renewal start with guessing.

## 9.2 `codes.generate` — the business's minting press

Accepts: `count` (1–500), `tier`, **`term_days`**, `term_kind`, `expires_at`,
`middleman`, `label`, `notes`.

So **attribution, term and expiry are set at mint time** — the business decides
all three when it creates stock. This is the single most important console
action commercially: every free code, every paid batch, and every middleman's
inventory starts here.

**Prefer the term over the absolute date.** `term_days` starts the clock at
*activation*, so a card does not decay on the shelf; `expires_at` starts it at
*mint*. The form defaults to 70 days (~10 weeks, a term pass). A code may carry
both — the term is then authoritative, because activation recomputes the
expiry from it.

## 9.3 What the console does *not* do

Worth recording, because it bounds what the operator can delegate:

- **No per-user traffic view.** There is no usage table
  ([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)).
- **No revenue or billing view.** Revenue is inferred from code counts and
  the price ladder, by hand ([`16-metrics.md`](16-metrics.md)).
- **No middleman ledger.** `middlemen.list` gives counts, not money owed;
  remittance is reconciled manually ([`07-billing.md`](07-billing.md#74-cash-handling)).
  **This matters more now** that renewal creates repeating revenue: before, "who
  owes what" was derivable from a count of codes sold. It no longer is, which
  is why the renewal action records the optional `price`/`middleman` in the
  audit detail — the raw material a ledger would need, captured now rather than
  lost ([`redesign/08-metrics-and-instrumentation.md`](redesign/08-metrics-and-instrumentation.md) §8.4).
- **No renewal prompting.** The console shows an *Expiring in 30 days* count
  and a 7-day worklist, but nothing tells the operator a specific student is
  due. That is a consequence of storing no personal data
  ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md) §1.2)
  and it is the largest operational risk in the current design
  ([`14-risks.md`](14-risks.md)).
- **No free-tier quota control.** The allowance is enforced client-side and
  is not configurable from here.

## 9.4 The rollout-percentage gate was removed

An active release is offered to *every* client; `active` is the single off
switch. (Rationale: a low percentage usually left the operator's own test
device outside the bucket, so a working updater looked broken.) See
[`15-continuity.md`](15-continuity.md).
