# 10. Lifecycle operations

The legacy plans had almost none of this. The live system does.

> **The lifecycle is now a CYCLE, not a line.** A code is minted with a *term*,
> activated, and then **renewed for as long as the student keeps paying**
> (`9a91da1`). Before that, a code had one fixed end date and the only way to
> sell more time was to mint another code. That is the difference between the
> business plan's "$10 per term" and the code's original single expiry.

| Operation | Mechanism | Commercial use |
|---|---|---|
| **Term** | `term_days` on the code; the expiry is computed from **activation** and stored in `expires_at` | What a student actually buys. A ~70-day term is a term pass; ~30 days is a month. |
| **Renewal** | `codes.renew`; extends from the **later of now and the existing expiry** | **Taking money for another term.** The operation the business runs on. |
| **Expiry** | `expires_at` reached → HTTP **410** | Enforces the subscription; drives renewal. |
| **Suspension** | `suspended` flag; → HTTP **403**. Reversible. | Refunds, abuse, "the card bounced". **Not cleared by a renewal** — a payment is not an abuse pardon. |
| **Release** | Clears `codes.activated_at` (the single-use stamp); the code is available to a different student | Support without reissuing a code. **Not needed to move a student to a new machine** — a code is not tied to a device. |
| **Expiry edit** | `codes.expire` sets or clears the date | Goodwill extensions. An absolute override, not the normal path. |
| **Term edit** | `codes.set-term` — sets the term, **never moves an expiry** | Fixing a code minted without a term; changing what future renewals produce. |
| **Audit trail** | `code_events` — append-only. Renewals record `before → after`. | Answers "what happened to this code" without guessing. |

**Support posture:** because the trail exists, support is *evidence-based*
rather than *memory-based* — a real operational maturity the legacy business
did not have.

## 10.0 One code per device, and what it costs in support

A device may hold **one** live code. Activating a second on the same machine is
refused (HTTP 409) with a message naming the fix
([`redesign/03-device-binding.md`](redesign/03-device-binding.md)).

The consequence needs stating plainly, because it is a real operational cost:

> **A refused student cannot fix this themselves.** Their route is the person
> who sold them the code, who contacts the operator, who performs an audited
> **re-bind**. That is the same friction that forced the operator to unbind
> their own test machine when a client's fingerprint changed.

So the escape hatch is not optional polish. Finding the right code must be
fast — which is why the console's search now matches the **name** in `label`,
not just the code string ([`09-console.md`](09-console.md)).

**When a student replaces a laptop**, use **Move to another device**
(`codes.rebind`), *not* Unbind. Unbind clears `activated_at`, which is the
input a term is measured from; a re-bind deliberately leaves the expiry
untouched so the student keeps the time they paid for.

## 10.1 How this applies to the free tier

Free codes are ordinary codes, so every lifecycle operation works on them:

- **Expiry is how free access is bounded in time.** A free code with a
  `expires_at` far in the future is a standing free tier; a short one is a
  trial ([`18-open-items.md`](18-open-items.md)).
- **Suspension is the abuse lever.** If a free code is being used to hammer
  the hub, suspend it — no need to delete anything.
- **The audit trail covers free codes too**, so a free user's history is as
  answerable as a paying one's.

## 10.2 The guardrails the operator must respect

- **Never bulk-delete codes or `code_events`.** The live hub holds **real paid
  codes in active use** (count and distributor deliberately not recorded — see
  [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §4). Use **suspend / unbind**,
  not delete.
- **Tier passwords are deliberately not console-editable.** Changing one
  instantly breaks every issued client. It is a documented operation only.
- **The rollout-percentage gate was removed.** An active release is offered
  to *every* client; `active` is the single off switch.

## 10.3 Related reading

- What the operator can do from the console → [`09-console.md`](09-console.md)
- What survives a bigger change → [`15-continuity.md`](15-continuity.md)
