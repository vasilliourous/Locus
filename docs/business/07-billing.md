# 7. Billing

> **The term pass is now a product the system can sell.** It could not before:
> the code stored one absolute `expires_at` fixed at **mint** time, so there was
> no way to express "10 weeks from when the student activates", and no way to
> renew. As of `9a91da1` a code carries a **term** (`term_days`, measured from
> activation) and the operator can **renew** it when a middleman reports
> payment. See [`redesign/02-term-and-renewal.md`](redesign/02-term-and-renewal.md).

## 7.1 The two models

Both are cash, both run through middlemen
([`08-distribution.md`](08-distribution.md)).

| Model | Pros | Cons |
|---|---|---|
| **Monthly** | Low commitment; easy sell. | 12 collections/yr; churn every month; the summer break kills revenue. |
| **Term pass** | Upfront cash; 4 collections/yr; holiday-proof; less churn. | Higher sticker price; needs the discount to land. |

**Recommendation:** lead with the **term pass** and keep monthly as the entry
offer. The 6-week summer break (Dec–Jan) is the reason — a monthly model
loses roughly 15% of the year's revenue to a holiday, and term 1 starts
fresh in February.

**In the system, both are the same mechanism.** A "term pass" is a code minted
with `term_days ≈ 70`; a month is the same code minted with `term_days ≈ 30`.
The difference is the number, not the machinery — which is why offering both
costs nothing to build. The console's mint form defaults to 70 days.

## 7.1.1 How a term is measured, and why that matters commercially

The clock starts at **activation**, not at mint. This is the change that makes
a printed card worth what it says:

* A card printed in February and sold in May now grants its **full term** to the
  buyer, rather than whatever remained of a February window.
* **Unsold stock stops decaying.** Before, a batch printed last term was worth
  less this term. Now a card in a drawer is worth exactly what it says whenever
  it is sold, which is what makes printing a batch in advance safe.

Renewal measures from the **later** of today and the existing expiry, so a
student who pays a week early keeps that week and has the new term added on
top. Paying early is never punished — which is the property that makes
"renew before it lapses" a thing you can actually ask a student to do.

## 7.2 The term structure

NZ school terms run about 10 weeks each, four per year
([`05-pricing.md`](05-pricing.md#55-term-passes)):

| | Approx. dates | Notes |
|---|---|---|
| Term 1 | Feb – Apr | Fresh cash, fresh students. |
| Term 2 | May – Jul | |
| Term 3 | Aug – Sep | |
| Term 4 | Oct – Dec | Leavers; the last term is short. |
| **Summer** | Dec – Jan (~6 weeks) | **No revenue on a monthly model.** |

## 7.3 Why the term pass matters more with a free tier

The free tier changes the economics of billing:

- **A free user has no billing event at all**, so the term pass is the first
  and sometimes only transaction the business has with a converted student.
- **Term boundaries are the natural conversion moment.** A student whose free
  allowance reset at the start of term, and who now wants to game with
  friends, is the most likely convert — and that is exactly when the
  middleman is collecting anyway.
- **Fewer collection events matter more as the paid tier gets cheaper.**
  Collecting $4 monthly costs more middleman effort per dollar than
  collecting $10 per term.

> **Practical implication:** the term pass should be positioned as the
> *default* paid product, with monthly as the trial-friendly fallback. See
> [`08-distribution.md`](08-distribution.md) for how middlemen pitch it.

## 7.4 Cash handling

- Middlemen collect in cash and remit the operator's share.
- There are no payment processor fees and no chargebacks
  ([`06-unit-economics.md`](06-unit-economics.md)).
- The `middleman` field on each code is the only record of who owes what
  ([`08-distribution.md`](08-distribution.md#83-what-the-system-actually-supports-verified)), so remittance discipline
  depends on the operator reconciling against `middlemen.list`
  ([`09-console.md`](09-console.md)).

## 7.5 The renewal routine — the operational job this creates

**A renewal is an operator action. Nothing in the system will prompt one.**
The hub holds no identity ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md)),
so it cannot know a payment is due; it only knows a date is approaching.

The routine, in order:

1. **Watch the queue.** The console's *Expiring in 30 days* card is the wider
   view; the 7-day list is the worklist. Both are driven by data the dashboard
   already computes.
2. **A middleman reports a payment** — by phone, by message, in person.
3. **Find the student.** The console's search box now matches the **name in
   `label`** as well as the code. This is the step that was impossible before:
   `codes.list` searched the code string only, so an operator holding a name had
   to guess which code was whose.
4. **Press Renew** and enter the days purchased. The hub computes the new
   expiry and shows `current → new` before anything is written.
5. The student keeps using the tunnel. **No re-entry, no restart, no
   interruption** — the new date arrives on the next heartbeat (~5 min).

> **The single most dangerous assumption in this file:** that step 1 happens.
> If nobody watches the queue, students lapse silently and no money is
> collected. This is a deliberate, accepted trade-off of keeping the hub
> free of personal data — recorded as a risk in
> [`14-risks.md`](14-risks.md) — but it means the renewal queue is not a
> convenience feature, it is the revenue mechanism.

**Write the name into `label` when a code is sold.** It is the only way to find
the student later, and the system cannot enforce it because it deliberately
stores no personal data. It belongs in the middleman's routine
([`08-distribution.md`](08-distribution.md)), not in a setting.

## 7.6 What a renewal records

Each renewal writes an entry to the audit trail: the code, the old and new
expiry, the days purchased, and — optionally — the middleman and price. The
operator can choose to record those two because cash changes hands between
people, and once a code can be renewed repeatedly, "how much did this middleman
collect this month" stops being derivable from a count of codes.

Recording it is optional but cheap, and it cannot be reconstructed later. See
[`redesign/08-metrics-and-instrumentation.md`](redesign/08-metrics-and-instrumentation.md) §8.4.
