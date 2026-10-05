# Locus — Terms, Renewal and Device Binding: Implementation Reference

```
audience:    builder
status:      design-record
authoritative-for: how the term/renewal/device-binding code actually works
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> **Built and committed.** This directory describes what the code
> **actually does** as of commit `6a422bc` (written against client `3.2.5`). It is the companion
> to the design record in the parent directory: `../` explains *why* the model
> is shaped this way, this explains *how it works and what to do with it*.
>
> **Deployment has not happened.** Nothing here has run against the live hub.
> The migration, in particular, is written and dry-runnable but **unexecuted** —
> see [`04-operations.md`](04-operations.md) §4.6 and
> [`05-not-yet-true.md`](05-not-yet-true.md).

---

## What this directory is for

Three audiences, three files:

| You are | Read |
|---|---|
| **Implementing against this** — hooks, console, client | [`01-data-model.md`](01-data-model.md), [`02-term-and-renewal.md`](02-term-and-renewal.md) |
| **Operating it** — the day-to-day job | [`04-operations.md`](04-operations.md) |
| **Auditing or debugging it** | [`03-device-binding.md`](03-device-binding.md), [`06-verification.md`](06-verification.md) |
| **Deciding whether to ship it** | [`05-not-yet-true.md`](05-not-yet-true.md) |

The **reasoning** behind the model is not repeated here. It is in the design
record at [`../`](../), and duplicating it is how two documents start
disagreeing. This directory states only what is true of the code today.

---

## The change in one paragraph

A code used to be one absolute `expires_at` fixed when the card was **minted**,
and it was both the purchase and the credential. It now carries a **term**
(`term_days`) measured from **activation**, which is materialised into
`expires_at` at the moment a device activates; the customer's entitlement is
addressed by the **device fingerprint** rather than by the card; one device
holds **one** code, enforced by a unique index; and the operator can **renew**
a code when a middleman reports payment (`codes.renew`), which extends from the
later of today and the current expiry.

`expires_at` deliberately kept its type and meaning throughout, which is why
the enforcement checks, the client's whole expiry module and the wire format
needed no changes at all.

---

## The files

| # | File | What it covers |
|---|---|---|
| 1 | [`01-data-model.md`](01-data-model.md) | Every field that was added, what writes it, what reads it, and the invariants |
| 2 | [`02-term-and-renewal.md`](02-term-and-renewal.md) | The arithmetic, exactly, with the boundary cases and where it is applied |
| 3 | [`03-device-binding.md`](03-device-binding.md) | The binding index, the 409 refusal, and the lifecycle state machine |
| 4 | [`04-operations.md`](04-operations.md) | What the operator actually does, in order, including the migration |
| 5 | [`05-not-yet-true.md`](05-not-yet-true.md) | **What has not been verified or run.** Read before trusting any of it |
| 6 | [`06-verification.md`](06-verification.md) | The commands, what they prove, and what they cannot |

---

## The commit trail

| Commit | What it did |
|---|---|
| `9a91da1` | The schema, the term model, renewal, re-bind, uniqueness, search, the backfill script, the client's 409 handling |
| `33b3901` | Caught the live business plan up to the implementation |
| `6a422bc` | The renewal worklist, and made the binding index readable (`device.get`, `devices.list`, the Devices page) |

Commits **before** `9a91da1` describe a system where a code had one fixed
expiry date. A document that says that is stale, not an alternative design.

---

## The rule this directory is held to

From `docs/README.md`, applied strictly: **the code wins.** Every claim was read
out of the tree, and every file/line reference can be checked. Where something
is *planned* rather than *built*, it says so in
[`05-not-yet-true.md`](05-not-yet-true.md) instead of being described as if it
worked — the discipline [`../../17-not-built.md`](../../17-not-built.md) exists
to enforce.
