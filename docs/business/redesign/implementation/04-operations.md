# 4. Operations — the job, in order

```
audience:    builder
status:      design-record
authoritative-for: the operator runbook
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> What an operator actually does with this. Written as a runbook, because the
> reasoning is elsewhere and the sequence is what gets forgotten.

---

## 4.1 Minting stock

Console → **Codes & Clients** → the create form.

| Field | Guidance |
|---|---|
| **Term (days from activation)** | **Prefer this over the absolute date.** The clock starts at *activation*, so a card printed in advance does not decay. `70` ≈ a 10-week term, `30` ≈ a month, `365` ≈ a year, `0` = never expires. Defaults to 70. |
| Expires (absolute) | The override. Starts its clock at **mint**. Use for a fixed goodwill date, not for normal stock. |
| Label | **Write the buyer's name here when the code is sold.** This is the only way to find the student later — see §4.4. |
| Middleman | Who is holding this stock. Drives `middlemen.list`. |

A code may carry both a term and an absolute date. If it does, activation
recomputes the expiry from the **term**, because that is the moment the
student's clock starts.

## 4.2 Renewing — the recurring job

**This is the revenue operation, and nothing will remind you to do it.** The hub
stores no identity, so it cannot know a payment is due; it only knows a date is
approaching. The queue is the mechanism.

1. **Open the worklist**: Dashboard → the *Expiring in 30 days* card (it is
   clickable), or Codes & Clients → **Due in 7 days** / **Due in 30 days**.
2. **Work top-down.** The worklist is sorted by urgency, soonest first.
3. **A middleman reports a payment.**
4. **Find the student** — search by the name in `label`, or by the code.
5. **Press Renew**, enter the days purchased, and optionally the middleman and
   price.
6. The prompt shows the **current expiry**; the hub computes and reports
   `before → after`. Confirm.

**The student is not interrupted.** No re-entry, no restart. The new date
arrives on the next heartbeat (~5 minutes).

### What the days mean

| Purchase | Days |
|---|---|
| A school term (~10 weeks) | 70 |
| Two terms | 140 |
| A month | 30 |
| A year | 365 |

Renewing **adds** to the current expiry, so paying early never loses days.

### Exclusions from the worklist

* **Suspended codes are excluded.** You cut that student off deliberately;
  prompting a renewal would be asking you to undo your own decision. Renew a
  suspended code explicitly if you have also decided to lift the suspension —
  but note that **renewing does not clear a suspension.** Two decisions, two
  actions, two audit entries.
* **Already-lapsed codes are excluded.** Those are a different problem: the
  student has *lost* access and needs a catch-up, not a reminder. Find them with
  the **Expired** status filter.

## 4.3 Moving a student to a new device

**Use "Move to another device" (`codes.rebind`). Do NOT use Unbind.**

Unbind clears `activated_at`; Rebind leaves the expiry alone. Under the term
model that is the difference between the student keeping the time they paid for
and silently losing it.

You need the fingerprint the **new** machine reports (the app's diagnostics
show it; it also appears truncated in the table). A reason is required — it
goes into the audit trail.

If you pass the wrong fingerprint the student cannot activate at all. The action
takes an optional `expected_fingerprint` and refuses rather than guessing when
it does not match.

## 4.4 The routine that makes lost cards survivable

**Write the buyer's name into `label` at the point of sale.**

The system cannot enforce this and deliberately stores no personal data, so this
is a human habit. It is what turns "my code stopped working" into a findable
student, and it is the reason the escape hatch in §4.3 is usable rather than
theoretical.

A code sold without a label is a code whose owner cannot be found — which is
precisely when they will call.

## 4.5 Reading the Devices page

**Devices** lists every binding. It is read-only by design: every mutation lives
on the Codes page with its audit trail, and a second write route would only be
a second way to get the same change wrong.

| What you see | What it means |
|---|---|
| **live** | Normal. The device holds this code, and the rule is enforced against it. |
| **released** | Free to activate another code. Kept for history. |
| **code missing** (red card) | A device bound through a code that no longer exists. The uniqueness check still refuses a new activation, so **the device is stuck** until it is cleared: unbind, then re-activate. |
| **duplicate live fingerprints** (red banner) | The unique index should make this impossible. If it appears, the rule is not holding — investigate before trusting it. |

## 4.6 The migration — the one step that needs care

> **NOT YET RUN.** The script exists and is dry-runnable; it has never been
> executed against the live hub.

The live hub holds **real paid codes in active use** (count and distributor
not recorded — see `../../../operate/CLAIMS.md` §4). The migration must not
shorten anyone's remaining time.

```bash
# 1. See what WOULD change. Writes nothing. Read the output.
python3 server/scripts/backfill-terms.py <admin_token> --dry-run

# 2. Apply it, only after reading the diff.
python3 server/scripts/backfill-terms.py <admin_token> --apply
```

**What it does:** derives `term_days` only where the code's own history proves
one (both `activated_at` and `expires_at` present, and the span sane). Sets
`--term-kind` if you want the display label.

**What it never does:** it **never writes `expires_at`**. Not to set it, not to
clear it, not to correct it. A code's current expiry is untouched by this
script.

**What it leaves alone, and why:**

| Case | Behaviour | Why |
|---|---|---|
| Already has a term | skipped | Idempotent — re-running is harmless |
| No expiry recorded | left alone | May be deliberately permanent; guessing would invent a policy |
| Never activated | left alone | Its expiry is mint-relative; there is no activation to measure a term from |
| Derived span < 1 day | left alone | Meaningless arithmetic |
| Derived span > 3 years | left alone | Looks mint-relative, not a real sold span |

Every case it declines is **printed with its reason**, so the dry-run output is
actionable rather than just a count.

### The order to deploy in

| # | Step | Safe because |
|---|---|---|
| 1 | Back up `pb_data` | — |
| 2 | Dry-run the backfill; read the diff | Writes nothing |
| 3 | Re-run `setup.sh` (adds the columns and the collection) | Additive only; the reconciler never removes or retypes |
| 4 | Run the backfill | Derives only; never touches `expires_at` |
| 5 | Deploy the hooks | New actions are additive; existing ones unchanged |
| 6 | Verify | — |

**Step 3 before step 5**, or a hook reads a column that does not exist yet.

### After it runs, verify by hand

* Every bound code's **`expires_at` and `bound_fingerprint` are byte-identical**
  to before. Record the values before you start. *(Capture them from the console
  per code — do not name the codes here; `../../../operate/CLAIMS.md` §4.)*
* `server/scripts/check-consistency.sh` is green.

The full invariant list is in [`../05-migration-and-live-data.md`](../05-migration-and-live-data.md).

## 4.7 The checks an operator can run at any time

| Check | Console / command | What it proves |
|---|---|---|
| Are bindings consistent? | **Devices** page | No duplicate live fingerprints; no `code_missing` |
| Any unindexed bindings? | Codes list, `⚠ unindexed` | The rule can enforce against every bound code |
| Who is due? | **Due in 7 / 30 days** | The renewal queue |
| What happened to a code? | Detail panel → History | Every mutation, with before→after |
| Cross-language drift | `server/scripts/check-consistency.sh` | Frozen wire names, schema-vs-hook fields, hook traps |
| Version agreement | `client/src-tauri/tests/version_consistency.rs` | The three manifests agree |
