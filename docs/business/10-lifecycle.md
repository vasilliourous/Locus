# 10. Lifecycle operations

```
audience:    human-operator
status:      live
authoritative-for: the cycle a code goes through, and the support posture that follows
verified-against: server/pb_hooks/admin_console.pb.js, docs/reference/DEVICE-IDENTITY.md
```

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
| **Release** | Clears `codes.activated_at` (the single-use stamp); the code is available to a different student | Support without reissuing a code. **Not needed to move a student to a new machine** — a code is not tied to a device. || **Expiry edit** | `codes.expire` sets or clears the date | Goodwill extensions. An absolute override, not the normal path. |
| **Term edit** | `codes.set-term` — sets the term, **never moves an expiry** | Fixing a code minted without a term; changing what future renewals produce. |
| **Audit trail** | `code_events` — append-only. Renewals record `before → after`. | Answers "what happened to this code" without guessing. |

**Support posture:** because the trail exists, support is *evidence-based*
rather than *memory-based* — a real operational maturity the legacy business
did not have.

## 10.0 One code, one activation — and almost no support cost

A code is **single-use**: activating it stamps `activated_at` and a second
activation of the same code is refused (HTTP 409, *"Already activated"*). That
is the whole rule. The code is **not** tied to a device.

> **Corrected 2026-10.** This section previously described a device-binding
> model — one code per device, a `device_bindings` collection, `codes.rebind` —
> as the live system. It was removed, not finished: see
> [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md).
> `codes.rebind` still exists as an action name, but it now returns **HTTP 410**
> with an explanation, so an operator following an old runbook gets a sentence
> rather than a silent no-op.

**What the student does when they change laptops.** Re-enter the same code on
the new machine. The client carries the code in its machine store, so a
reinstall or an update usually restores it without any input at all; a genuinely
new machine is a fresh activation, and because the code is only *single-use*,
not device-bound, it works. **No operator involvement is required.**

That is the support cost this design removed, and it is worth naming because
the alternative was expensive: a device-bound entitlement fails precisely in the
ordinary case (the student's own laptop was wiped, repaired or replaced), and
each failure needed an operator to perform an audited re-bind.

**The one operation that remains** is **`codes.unbind` (Release)**: it clears
`activated_at`, returning the code to the unused pool so it can be activated by
a **different student**. Use it for a refund, a reissue, or a card that was sold
to the wrong person — not to move one student to a new machine, which needs no
help.

**Finding the right code must still be fast**, which is why the console's search
matches the **name** in `label`, not just the code string
([`09-console.md`](09-console.md)).

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

- **Never bulk-delete codes or `code_events`.** Use **suspend** (revoke) or
  **unbind** (reissue), not delete. On a hub holding live codes, a bulk delete
  is unrecoverable and destroys the audit trail.
- **Tier passwords are deliberately not console-editable.** Changing one
  instantly breaks every issued client. It is a documented operation only.
- **The rollout-percentage gate was removed.** An active release is offered
  to *every* client; `active` is the single off switch.

## 10.3 Related reading

- What the operator can do from the console → [`09-console.md`](09-console.md)
- What survives a bigger change → [`15-continuity.md`](15-continuity.md)
