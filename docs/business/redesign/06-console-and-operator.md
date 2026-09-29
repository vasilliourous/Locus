# 6. The operator surface

> **STATUS `6a422bc`: BUILT.** Everything specified below exists — the renew
> button (row and detail panel), the 7/30-day worklists, the clickable
> Dashboard card, name search, and the device read-back. Operator instructions
> live in [`implementation/04-operations.md`](implementation/04-operations.md);
> this file keeps the reasoning.
>
> One deliverable in §6.6 is **not** built: the middleman ledger. The renewal
> detail now records the `price`/`middleman` a ledger would need, but nothing
> aggregates them.

> What must change in the admin console for this redesign to be usable. Every
> action named here exists or is specified elsewhere in this directory.

---

## 6.1 The surface before this work

> **This section is the BASELINE the redesign started from, not the current
> state.** It is kept so the delta is legible. For what exists now, see
> `../../09-console.md` (the live action list) and
> [`implementation/04-operations.md`](implementation/04-operations.md).

The console is a Vue 3 SPA at `/admin/` hitting one authenticated endpoint
(`../09-console.md`). The code lives in `server/console/src/`, and the actions
in `admin_console.pb.js`. At the time of writing there were **eighteen
actions**, and **none of them was a renewal**:

```
dashboard · codes.list · codes.generate · codes.suspend · codes.unsuspend
codes.unbind · codes.expire · codes.update · codes.history · codes.delete
codes.deleteBatch · middlemen.list · tiers.list · tiers.update
releases.get · releases.fetchLink · releases.publish · releases.set
```

`codes.expire` sets an **absolute** date by hand. It is the closest thing to a
renewal that exists, and it requires the operator to *do the date arithmetic in
their head* — which is both error-prone and exactly the manual step this
redesign removes.

## 6.2 The renew button — the headline deliverable

**Requested explicitly by the operator.** It must appear in two places, because
the operator arrives at a renewal from two directions.

### 6.2.1 On the Dashboard — "someone just paid me"

The Dashboard currently renders five count cards
(the card grid in `Dashboard.vue`):

```
Total codes · Available (unused) · Activated on a device · Suspended
Expiring in 30 days
```

The **"Expiring in 30 days"** card is already computed
(the `dashboard` `expiringSoon` computation, `expiringSoon`) and is currently **inert** — it
is a number with no action attached. It is the natural home for renewal:

* Make it **clickable**, opening the Codes view pre-filtered to codes expiring
  within the window.
* Add a **"Renew"** action to each row in that filtered view.

This turns a dead metric into the operator's renewal worklist. It is the single
highest-value change in this file, because
[`01-identity-without-pii.md`](01-identity-without-pii.md) §1.2 established that
**nothing will prompt the operator to renew**. The Dashboard card *is* the
prompt, or there is none.

### 6.2.2 On the code detail panel — "I'm looking at this code"

The console's code detail panel has with Label, Notes, Expiry, and buttons
for *Save details* / *Unbind device* / *Suspend* / *Reactivate* / *Delete*.

Add **Renew** beside **Unbind**, with:

| Element | Requirement |
|---|---|
| A term selector | Defaults to the code's `term_days`; offers the tiers' configured terms; allows a custom day count (the override in [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4). |
| A **before → after** preview | Show `current expiry → new expiry` *before* the operator confirms. This is the whole reason the action is safe: the operator sees the arithmetic rather than trusting it. |
| A middleman/reference field | Optional, for the audit trail. |
| Confirmation | The action writes real time onto a real customer. Not one-click. |

The existing **Expiry** input (the Expiry field) stays for one-off absolute
corrections, but it should be labelled as the manual override it is
("Set expiry (manual)"), so it is not mistaken for the normal path.

### 6.2.3 What the button must NOT do

Repeating [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.4 because this
is where the mistake would be made visible:

* **Must not clear a suspension.** If it did, the operator would see the
  suspend action undone and not know why.
* **Must not change the binding.**
* **Must not change the tier.**

The UI should make these non-effects obvious — e.g. the renewal dialog shows
"Tier: Strike (unchanged)" and "Device: bound (unchanged)" so the operator can
confirm nothing else moved.

## 6.3 The renewal worklist — the operational routine

Because renewals are operator-initiated, the console owes the operator a
**queue**, not just a button. Specified minimal:

* Codes whose `expires_at` falls within **7 days** — matching
  `EXPIRY_WARNING_DAYS` on the client, so the student's
  own warning and the operator's worklist agree on what "soon" means.
* Sorted by expiry, soonest first.
* Showing `code`, `label` (the operator's name for the student), `middleman`,
  and days remaining.
* With **Renew** inline, so clearing the queue is one action per row.

The 30-day Dashboard card is the wider view; the 7-day list is the worklist.
Both come from data `dashboard` already computes — this is mostly a UI change.

## 6.4 Search by name — mandatory, not optional

[`01-identity-without-pii.md`](01-identity-without-pii.md) §1.3 established the
gap: **`codes.list`'s `q` filter searches the code string only**
(the `codes.list` query filter), and `label`/`notes` are returned but never
searched.

> **Consequence: given only a student's name, the operator cannot find their
> code.** Every renewal therefore starts with the operator guessing which code
> is whose.

The fix is small and contained. Extend the `q` match
(the `codes.list` query filter) to also test `label`, `notes` and `middleman`:

```js
// today: code string only
if (q && codeVal.toUpperCase().replace(/-/g,"").indexOf(q) === -1) continue;
```

becomes a match against any of the four. Two notes for the implementer:

* **Normalise both sides.** Lowercase the haystacks for label/notes (they are
  free text, so case should not matter), and keep the code comparison
  hyphen-insensitive exactly as now.
* **Say so in the UI.** The query box's placeholder should become
  "code, name or notes", or the operator will never discover the capability
  exists.

This is the cheapest change in the redesign with the largest effect on whether
renewals actually happen.

## 6.5 The device view — what a machine holds

`device_bindings` ([`03-device-binding.md`](03-device-binding.md) §3.1) gives
the console something it has never had: **the ability to answer "what is this
device entitled to?"** Add:

* A `device.get` / `devices.list` action reading the binding index.
* On the code detail panel, show whether the device holds *other* codes (it
  should not, after uniqueness is enforced — which makes this a useful
  integrity check).
* An operator-visible distinction between **bound, stable** and **bound, then
  re-bound**, since a rebound code is the signature of either a legitimate
  laptop replacement or a card changing hands.

## 6.6 The middleman ledger — the admitted gap

[`../09-console.md`](../09-console.md) §9.3 is candid:

> *"**No middleman ledger.** `middlemen.list` gives counts, not money owed;
> remittance is reconciled manually."*

And [`../07-billing.md`](../07-billing.md) §7.4 depends on it:

> *"The `middleman` field on each code is the only record of who owes what, so
> remittance discipline depends on the operator reconciling against
> `middlemen.list`."*

The redesign does not need to build a ledger, but it **does** make it more
valuable, because renewal creates a repeating revenue event per code — and
therefore a repeating commission per code. With monthly/term renewal, "how much
do I owe the middleman this month" stops being inferable from a stock count.

**Recommendation:** once renewal is live and there is more than one renewal on
record, add a simple per-middleman view: codes sold, renewals in the period, and
the computed commission at the agreed rate. It is a `codes.list` filter plus
arithmetic — the same "reporting is a filter" pattern
[`../08-distribution.md`](../08-distribution.md) §8.3 already describes. Flagged
as a proposal rather than a requirement, in
[`10-open-questions.md`](10-open-questions.md).

## 6.7 What must not change in the console

* **The single-endpoint shape.** One `POST /api/admin/console` with an `action`
  discriminator, and the token in the body/header. Adding actions is fine;
  adding endpoints is not, because `fetch-release.py` and `Caddyfile` routing
  depend on the current shape.
* **The audit discipline.** Every mutating action writes `code_events` via
  `logEvent()` (`logEvent` in `admin_console.pb.js`). New actions must too — this is
  what makes "what happened to this code" answerable.
* **The sessionStorage token choice** (`console/src/api.ts:3-18`) — the token
  deliberately lives in `sessionStorage`, not `localStorage`, so it does not
  persist on a shared machine.

## 6.8 Related reading

* Why identity forces operator-initiated renewal → [`01-identity-without-pii.md`](01-identity-without-pii.md)
* The action being surfaced → [`02-term-and-renewal.md`](02-term-and-renewal.md)
* The binding integrity the device view checks → [`03-device-binding.md`](03-device-binding.md)
* The metrics this makes measurable → [`08-metrics-and-instrumentation.md`](08-metrics-and-instrumentation.md)
