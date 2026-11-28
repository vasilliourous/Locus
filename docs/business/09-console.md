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

## 9.0 How the console is organised (the URL redesign)

The console holds its state **in the URL**, not in a `reactive` object. Every
destination has a path, every selected code has an address, and every filter is a
query parameter. This is what makes the list refresh-safe, the back button work,
and a code linkable in a support thread. The router is `console/src/router.ts` —
a ~200-line module over the History API, **not a dependency**; Caddy already
served `/index.html` for unknown paths under `/admin/`, so the server needed no
change.

| Path | Screen | What it is |
|---|---|---|
| `/` | **Today** | Counts, the inline renewal queue, the attention checks, activity |
| `/codes` | **Codes** | The table. Filters are query params: `?q=`, `?status=`, `?tier=`, `?m=`, `?due=` |
| `/codes/<CODE>` | Codes + **inspector** | That code, in the right-hand rail |
| `/renewals` | **Renewals due** | Every renewal code as a board, by urgency |
| `/distribution` | **Distribution** | Per-middleman stock and collections |
| `/fleet/tiers` | **Tiers** | Connection settings every client receives |
| `/fleet/releases` | **Releases** | Which build the fleet is offered |
| `/fleet/maintenance` | **Maintenance** | Import, one-shot links, export |

**The code is the primary object.** Selecting a code fills a persistent
inspector beside the table; the list never covers and the panel never moves. The
inspector has no tabs — identity, verdict, the four facts (expiry, term, last
seen, middleman), the editable name/plan/middleman, the actions, the history and
the irreversible operations are one scrolling column, in the order they are
needed.

`?` toggles a **help rail** with the reasoning for the screen ("why nothing
prompts a renewal", "why lapsed is not the urgent column"). This is where the
long explanations live, instead of permanently under a table — the prose was
never the problem, its placement was.

### 9.0.1 Auth: one operator, by decision

**This console has exactly one operator and no identity model.** That is a
recorded decision, not an omission:

- One shared `ADMIN_API_TOKEN`, compared against an environment variable.
- `sessionStorage`, never `localStorage` (a shared machine must not keep the
  session), and the token is never in the built bundle.
- There is **no** per-operator token, no rate limit, and no `actor` on the audit
  trail — every `code_events` row is authored by "the console".

The consequence is stated plainly so nobody assumes otherwise: **the audit trail
cannot say WHO renewed a code or released it.** If a second person ever needs
access, this section must change first — named tokens, a hashed comparison, a
rate limit, and `actor` on `logEvent` — because the current model makes "who did
this" permanently unanswerable rather than merely inconvenient.


## 9.1 Actions that exist today (verified)

| Group | Actions |
|---|---|
| Overview | `dashboard` · `attention.list` |
| Codes | `codes.list` · `codes.generate` · `codes.suspend` · `codes.unsuspend` · `codes.renew` · `codes.set-term` · `codes.expire` · `codes.unbind` · `codes.history` · `codes.update` · `codes.delete` · `codes.deleteBatch` · `codes.import` |
| Middlemen | `middlemen.list` · `middlemen.summary` |
| Tiers | `tiers.list` · `tiers.update` |
| Releases | `releases.get` · `releases.fetchLink` · `releases.publish` · `releases.set` |

> **`device.get` / `devices.list` are retired too** — they return HTTP 410, and
> the console's "Devices" page was deleted with them (it had been a client-side
> re-filter of `codes.list`). What an operator needs there — "is this code in
> use, and until when" — is `codes.list` with `status: "redeemed"`.
>
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
- **No middleman ledger — partially closed.** `middlemen.summary` (the
  **Distribution** page) now shows, per middleman: stock by state, renewals in a
  window, and the money recorded against them. What it is **not** is a ledger:
  there is no commission rate, no remittance record and no carried balance. A
  figure is reported as a floor (`≥`) when any renewal in the window carried no
  price, because a total built from partial data would be believed. The rate
  stays a business agreement, not a stored value
  ([`07-billing.md`](07-billing.md#74-cash-handling)).
- **No renewal prompting by the SYSTEM — only by the console.** Nothing
  notifies anyone that a student is due; the hub holds no identity and therefore
  no contact route
  ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md) §1.2).
  The console answers it the only way it can: a live count on the **Renewals
  due** and **Today** nav items (codes ending within 7 days), the queue shown as
  ROWS on **Today** so it can be worked without leaving the landing page, a
  **Renewals due** board that arranges every code by urgency (lapsed · ≤7d ·
  8–30d · later · stopped), and a Today check for codes that lapsed without being
  noticed. That is still the largest operational risk in the design
  ([`14-risks.md`](14-risks.md)) — a queue only works if somebody opens it.
- **No free-tier quota control.** The allowance is enforced client-side and
  is not configurable from here.

## 9.4 The rollout-percentage gate was removed

An active release is offered to *every* client; `active` is the single off
switch. (Rationale: a low percentage usually left the operator's own test
device outside the bucket, so a working updater looked broken.) See
[`15-continuity.md`](15-continuity.md).
