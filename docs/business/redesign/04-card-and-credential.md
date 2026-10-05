# 4. The card and the credential

```
audience:    human-operator
status:      design-record
authoritative-for: the card/credential design (client half built; binding half removed)
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **SUPERSEDED IN PART 2026-10.** The requirement this file opened with — *"the
> code needs to be stored somewhere"* so a student who threw the card away stays
> logged in — is **built**: the client mirrors the code to a machine-scoped store
> and re-adopts it if the config loses it. The *other* half of the proposed
> answer, "forever bound to a device", was **rejected**: a code is single-use and
> is not tied to a device.
>
> See [`../../reference/DEVICE-IDENTITY.md`](../../reference/DEVICE-IDENTITY.md).
> The status tables in §4.5 are updated; the surrounding narrative describes the
> design as it was written, and §3 (device binding) is superseded.

> **The reported problem:** *"they should be FOREVER bound to a device, and
> people are likely to throw their cards away, so they need to remain logged in
> always and the code needs to be stored somewhere."*
>
> This file separates two things the current design conflates, which is what
> makes a thrown-away card fatal today.

---

## 4.1 The conflation

[`../03-product.md`](../03-product.md) §3.3 states the object's nature:

> *"a code is a **bearer token that is also a physical card**. It is generated
> in bulk, printed, handed to a middleman, sold for cash, and activated once."*

One string is doing three jobs at once:

| Job | Needs | Lifetime |
|---|---|---|
| **Proof of purchase** | To identify who paid, and for what | Forever, in principle |
| **Proof of entitlement** | To fetch a tier config and be authorised | While the subscription runs |
| **Device identity** | To be the thing a code is bound to | Life of the machine |

Because they are one value, **losing the card loses all three**, and there is
nothing left to recover from. That is the defect.

## 4.2 What actually happens today when a card is lost

Traced against the code, because the answer is not obvious:

1. The student loses the card, but **their installed client keeps working.**
   The code is stored on their machine — `store::store()` writes
   `activation_code` into the app config (via `store::store`), and
   `store::read()` reads it back. So in the common case
   **nothing is lost at all** for a student who keeps using the same machine.

2. What is lost is **recoverability**:
   * **Reinstall or new machine.** The stored code is gone with the old
     install, and the card was the only other copy. The student now has an
     entitlement they cannot prove, and their only route is back through the
     middleman who sold it.
   * **Support.** When the student phones the middleman, the middleman has no
     way to look up "which card did I sell Kahu?" — the paper record is theirs
     and may be as lost as the card.
   * **Renewal.** The operator cannot renew a code they cannot identify
     ([`01-identity-without-pii.md`](01-identity-without-pii.md) §1.3).

> **So the honest framing is: a lost card is *not* an immediate outage — it is
> a permanent loss of the ability to prove the purchase, which becomes an
> outage the first time anything changes.**

That is still a serious defect, because "anything changes" includes a reinstall,
which is exactly when a student is most likely to need help.

## 4.3 The design: the hub remembers the entitlement, the card remembers nothing

The separation, stated as a rule:

> **A code is a voucher. A subscription is an entitlement. The card proves
> nothing after the first activation — the hub proves it.**

Concretely:

| Actor | Holds | If lost |
|---|---|---|
| **The card** | The code string, once | Nothing is lost *after* first activation — it is a spent voucher |
| **The device** | Its fingerprint, persisted durably ([`archive/03-device-binding.md`](archive/03-device-binding.md) §3.2 Part A) | Recoverable: see below |
| **The hub** | The subscription, keyed by *fingerprint*, with its term and history | Recoverable by an operator |
| **The operator/middleman** | The paper/`label` link from person → code | Copyable, and the fallback |

The critical change is the third row. In the current model, the hub's record of
"who is entitled" is a field *on the code*, so losing the code loses the
pointer. In this design the entitlement is addressed by **fingerprint** — a
value the device keeps producing on its own, independently of any card. A
student who reinstalls and whose fingerprint is stable ([`archive/03-device-binding.md`](archive/03-device-binding.md))
is recognised immediately, **with no card and no operator involvement.**

## 4.4 What "forever bound" should actually mean

The report asks for codes to be *forever* bound to a device. Taken literally
that is dangerous, so this design states the intended property precisely:

> **The binding persists until an operator changes it.** Not until the term
> ends, not until the code is suspended, not until any expiry — an entitlement's
> *device* and its *time* are independent.

This matters because the current code accidentally ties them together:

* `store::clear()` wipes the code, tier, expiry,
  heartbeat state **and** leaves the refusal reason — and it is called from
  `enforce_grace_period` in `runtime.rs` when the hub has been
  unreachable for a week.
* So a student who cannot reach the hub for a week loses their stored code, and
  on a machine whose fingerprint has drifted, that is unrecoverable without an
  operator.

Under the new model, a lapsed or unreachable entitlement must expire **the
subscription**, never **the binding**. Re-activating on the same device should
be a no-op that re-establishes the link, not a fresh bind that risks the
`boundFp !== fp` refusal.

## 4.5 Keeping a student logged in "always" — the requirements

The report's practical requirement. Three properties must hold:

| Requirement | Mechanism | Status |
|---|---|---|
| The client survives a restart without re-entry | `store::read()` reads the code back from config — **already works** | **Built** |
| A reinstall does not lose the entitlement | The client mirrors the code to a machine-scoped store; the hub keys nothing to a device | **Built 2026-10.** [`../../reference/DEVICE-IDENTITY.md`](../../reference/DEVICE-IDENTITY.md) |
| An offline student is not silently de-entitled | Grace period is bounded at 7 days (`GRACE_PERIOD` in `heartbeat.rs`); on lapse the session is cleared but **the code is kept** (`store::record_lapsed_grace`) | **Built 2026-10** — see §4.4 |

The third row is worth an explicit decision, because it is a deliberate
trade-off that the report's "always" framing sits awkwardly against. The
existing rationale (the grace-period rationale in `runtime.rs`) is sound:

> *"a device which simply never reaches the hub again does not run forever on an
> entitlement the hub has never confirmed — and so a lapsed or refunded code
> eventually stops even if the refusal never arrives."*

The design keeps the 7-day bound but changes what it costs: **it must clear the
*entitlement*, not the *binding*.** A student who is offline for a week, and
whose code is still valid, must be able to reconnect the moment the network
returns — without an operator, and without re-typing a code they no longer have.

## 4.6 The recovery paths, ranked

When something is lost, in order of how cheap it should be:

1. **Nothing lost** — same machine, same install. *Already the case.*
2. **Reinstall, stable fingerprint** — automatic. *Blocked on §3.2 A, the client-side fingerprint persistence, which is deliberately deferred.*
3. **New machine, same person** — operator re-bind, found by name.
   *Built: `codes.rebind` plus name search.*
4. **Card lost, person known, code unknown** — operator searches by the name in
   `label`, finds the code, re-binds. *Built — the search now reads `label`.*
5. **Everything lost** — the middleman's paper record, or nothing. This remains
   the irreducible floor, and it is why `label` should be filled in at mint.
   It is also the argument for the middleman habit described in
   [`../08-distribution.md`](../08-distribution.md) §8.3.

Path 5 is the one that justifies a *behavioural* recommendation rather than a
technical fix: **write the student's name into `label` when the code is sold.**
The system cannot enforce it (no PII, by decision), so it belongs in the
middleman's operating routine.

## 4.7 What this file does not do

* It does not add a customer record. Identity stays on paper and in
  `label`/`notes` ([`01-identity-without-pii.md`](01-identity-without-pii.md)).
* It does not introduce a "recovery code" or any second secret the student must
  keep — that would recreate the same problem in a new form.
* It does not remove the grace period. It makes the grace period stop costing
  the binding.

## 4.8 Related reading

* The binding this depends on → [`archive/03-device-binding.md`](archive/03-device-binding.md)
* The identity decision → [`01-identity-without-pii.md`](01-identity-without-pii.md)
* Where the operator does the recovery → [`06-console-and-operator.md`](06-console-and-operator.md)
