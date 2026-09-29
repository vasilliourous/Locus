# 1. Identity without PII

> **Decision (operator):** the hub stores **no personal information**. The
> human-to-code link is kept **on paper**, and the admin console's existing
> `label` and `notes` fields carry whatever the operator chooses to record.
>
> This file specifies what that means concretely, and the one gap it exposes
> that makes the renewal feature impossible to use well until it is fixed.

---

## 1.1 What "no PII" rules out, and what it rules in

**Ruled out — must not be built:**

* A `customers` collection with names, phone numbers, emails or addresses.
* Any signup, login, password-reset or account-recovery flow for a student.
* Any field that receives data from a student about themselves.

This is not merely a preference. [`../03-product.md`](../03-product.md) §3.4
states the product's shape as *"No billing screen, no account, no password
reset, no signup flow, no dashboard of their own... this is a feature: it
removes every support surface a normal SaaS has."* Putting PII in the hub would
create exactly the obligations that positioning was chosen to avoid — a data
store to secure, a breach to disclose, and a retention question with no good
answer for a project that expects to end by switching off a VPS
([`../12-scale-and-ceiling.md`](../12-scale-and-ceiling.md) §12.4).

**Ruled in — remains the system of record:**

| Where | What it holds | Who can see it |
|---|---|---|
| **Paper** | The middleman's own record of "I sold card `RQ-…` to *this student*" | Only the middleman |
| `codes.label` | A short operator-authored tag. Verified in use for exactly this purpose: the console's generate form exposes a label input at mint time. | Operator, console |
| `codes.notes` | Operator free-text remarks. Same. | Operator, console |
| `codes.middleman` | The attribution set at mint (the `codes.generate` action in `admin_console.pb.js`) | Operator, console |

So the answer to "who is this code's owner" is: **the middleman's paper, plus
whatever the operator wrote into `label`/`notes`.** The hub is deliberately not
the source of truth for identity.

## 1.2 The consequence for renewal — read this before designing the UI

Because the hub holds no identity:

* **Every renewal is operator-initiated.** A middleman reports "Kahu paid for
  another term"; the operator finds the code and presses Renew.
* **Nothing renews automatically.** There is no scheduled job that extends an
  entitlement when payment is expected, because the hub does not know a payment
  is expected.
* **There is no self-service path.** A student cannot renew. They cannot even
  ask through the software.

This is coherent. It matches the actual money flow: cash, handed to a person,
reported to another person. But it creates an operational duty, stated plainly:

> **Someone must notice that a term is about to end and act.** If nobody does,
> access simply stops. There is no mechanism in this design that will chase
> anybody on its own.

That duty is why the console must surface *"which terms end soon"* prominently
([`06-console-and-operator.md`](06-console-and-operator.md) §6.3). A renew
button nobody is prompted to press does not collect money. This is the single
largest operational risk in the design, and it is a deliberate acceptance, not
an oversight — see [`10-open-questions.md`](10-open-questions.md) Q3 for the
alternative if it proves wrong.

## 1.3 The gap that had to be closed: finding a code by the name on paper

> **FOUND AND FIXED.** As of `9a91da1`, `codes.list`'s query matches `label` and
> `notes` as well as the code string. The section below is the diagnosis that
> made it a required deliverable — kept because the reasoning still constrains
> the design, and because anyone changing that query needs to know what it is
> for.

Here is the practical failure of the whole flow, as it was before the fix.

An operator takes a phone call: *"It's Kahu, my code stopped working."* They go
to the Codes page. `codes.list` accepted these filters:

| Filter | Matched against |
|---|---|
| `q` | **the code string only** — `codeVal.toUpperCase().replace(/-/g,"").indexOf(q)` |
| `tier` | exact tier |
| `status` | `suspended` · `expired` · `bound` · `available` |
| `middleman` | substring of the middleman field |

`label` and `notes` were **returned in every row** but **not searched or
filtered by anything.**

> **Therefore: given only a student's name, the operator could not find their
> code.** They could search by *middleman* (narrowing a batch), then read rows
> by eye. For one middleman with five codes that is tolerable. For the target of
> 50–500 students per school ([`../12-scale-and-ceiling.md`](../12-scale-and-ceiling.md) §12.1)
> it is not a workflow.

**This is why lookup-by-name was a required deliverable, not a nicety.** The
`q` filter now matches `label`, `notes` and `middleman` case-insensitively
against the same normalised needle, and the console's search box says so
("code, name or notes"). Without it, the renewal feature's cost per renewal is
"scroll and squint" — precisely the friction that means renewals do not happen.

**The constraint this leaves in place:** because the hub stores no PII, a code
sold **without** a `label` still cannot be found by name. The system cannot
enforce filling it in, which is why it is documented as a middleman routine
([`implementation/04-operations.md`](implementation/04-operations.md) §4.4)
rather than a setting.

## 1.4 What a "customer" is, in this design

Given no PII, do **not** introduce a `customers` table that implies one. Instead
the design keeps the code as the anchor and adds only what is needed to make
entitlements and renewals coherent:

| Entity | What it is | Identifier | Carries PII? |
|---|---|---|---|
| **Code** | The physical card's digital twin. A *voucher*. | `RQ-XXXX-XXXX-XXXX-C` | No |
| **Subscription** | The entitlement a code redeems into. Holds the term and its current end. | Hub-generated id | No |
| **Device binding** | "This fingerprint is entitled via this subscription." | fingerprint | No |
| **Operator label** | Free text the operator writes, e.g. a name. | on the code | **Yes — operator's choice** |

> **Built differently — see [`implementation/01-data-model.md`](implementation/01-data-model.md) §1.6.**
> The middle row is a *concept*, not a table. No `subscriptions` collection was
> created: the entitlement is realised as the term materialised into the code's
> `expires_at`, plus the binding row that names the device. Once the expiry is
> computed at activation and the device is indexed, a third entity would hold
> the same facts a second time — a second source of truth for one entitlement,
> with no reader. The reasoning below explains why the *separation* matters;
> the code achieves it without the extra table.

Note the last row. `label`/`notes` *are* where a name will end up, because the
operator needs one and it is the only place available. That is accepted: the
operator chooses what to write, the hub never solicits it, and it is no more
exposed than the console itself. The design's job is to make sure nothing
*requires* it to be filled in, so a code with no label still works end to end.

### Why the concept of a subscription is separate from the code

Because a code and an entitlement have different lifetimes, and collapsing them
is what produced the current mess:

* A **code** is minted in a batch, printed, possibly never sold, possibly
  thrown away. Its life is *physical*, and it ends when it is activated —
  it is a spent voucher from that moment.
* An **entitlement** begins at activation, is renewed repeatedly, and is what
  the hub enforces — addressed by the **device**, not by the card.

The code used to carry both, so "renew" had nowhere to write except the card's
own row — which is exactly why a lost card meant a lost account. Separating
them is what makes [`04-card-and-credential.md`](04-card-and-credential.md)
possible at all, and it is what the implementation achieves by keying the
binding on the fingerprint rather than on the code string.

## 1.5 Related reading

* The term model these entities carry → [`02-term-and-renewal.md`](02-term-and-renewal.md)
* Why the card can be lost without losing access → [`04-card-and-credential.md`](04-card-and-credential.md)
* The console work this mandates → [`06-console-and-operator.md`](06-console-and-operator.md)
