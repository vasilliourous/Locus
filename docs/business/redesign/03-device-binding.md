# 3. Device binding

> **Decisions (operator):** fix all three defects — one code per device,
> explicit re-binding, and fingerprint stability. A second code activated on an
> already-bound device is **refused outright**.
>
> This file specifies each, and is honest about which parts cannot be verified
> without real hardware.

---

## 3.1 Defect one: a device can bind to many codes

### The mechanism, verified

`bound_fingerprint` is a field **on the code** (see the `codes` schema in
`seed-pb.py`). Its only write is in `activation.pb.js`:

```js
rec.set("bound_fingerprint", fp);
```

There is no reverse index. Nothing anywhere asks "which codes does this
fingerprint already hold?". Verified by grepping every read of the field — the
complete list is:

| Site | Use |
|---|---|
| `activation.pb.js` | read `boundFp`, compare to the incoming `fp` |
| `activation.pb.js` (the bind path) | write it on bind |
| `admin_console.pb.js` | console display, unbind, dashboard counts |
| `admin_unbind.pb.js` | operator unbind |
| `code_lookup.pb.js` | readiness pre-check |

**Not one of these looks at other codes.** So a device may activate code A,
then code B, then code C, and each succeeds independently. It now holds three
entitlements, and the hub has no idea.

### Why it matters commercially

* **Theft is undetectable.** A student who obtains several codes — from a
  friend's discarded card, a cancelled order, a batch that leaked — can stack
  them. Nothing flags it, because there is nothing to flag against.
* **The dashboard lies.** `bound` counts *codes with a fingerprint*
  (the `dashboard` counts in `admin_console.pb.js`). Three codes on one machine read as three
  activated devices, so the operator's own headcount is inflated.
* **The free tier makes it worse.** If free codes are handed out
  ([`../04-tiers.md`](../04-tiers.md) §4.4), the same machine can farm them.

### The fix: uniqueness, enforced at bind

Enforce **one live code per fingerprint**. On activation, before binding:

1. Look up whether this `fp` already holds a binding on a *different* code.
2. If it does — and that code is not suspended or expired — **refuse**.

The refusal must be a distinct, legible outcome, not a generic 403. The client
already has a vocabulary for exactly this shape of problem: `ActivationOutcome`
(`ActivationOutcome` in `locus/activation.rs`) has separate arms for `BoundToAnotherDevice`,
`Suspended`, `Expired`, `NotFound`, `RateLimited`, `ServerError`, and
`describe_outcome` (`cmd/locus.rs:~490`) gives each one a sentence written for a
student. **Add a new arm, e.g. `DeviceAlreadyActivated`**, rather than
overloading an existing one — overloading is how the `403` ambiguity in §3.3
happened.

### How to find the other binding

Two options, with a real trade-off:

| Option | How | Cost |
|---|---|---|
| **Scan** | `findRecordsByExpr("codes", $dbx.exp("bound_fingerprint = {:f}", {f: fp}))` | No new collection; a full scan of `codes` on every activation. Fine at hundreds of rows; not at scale. |
| **Index table** | New `device_bindings` collection: `{fingerprint, code, subscription, bound_at}` with **unique fingerprint** | One extra write per bind; O(1) lookup; survives code deletion; can carry per-device history. |

**Recommendation: the index table. BUILT as recommended** — the collection is
`device_bindings` with a UNIQUE `fingerprint` field
([`implementation/01-data-model.md`](implementation/01-data-model.md) §1.3,
[`implementation/03-device-binding.md`](implementation/03-device-binding.md)).
Note the `subscription` field in the sketch above was **not** built; the row
carries `tier` instead, because there is no subscription table to reference
([`implementation/01-data-model.md`](implementation/01-data-model.md) §1.6).

The scan is tempting, but it cannot
express the state we actually need — "this fingerprint is bound, and to which
code *by id*" — and it breaks the moment a code is deleted
(`codes.delete` exists, `codes.delete` in `admin_console.pb.js`) because the binding
evaporates with it. The index also gives
[`06-console-and-operator.md`](06-console-and-operator.md) its "what does this
device hold?" view, which is otherwise unanswerable.

Note the schema caution: a *unique* index on `fingerprint` is what actually
enforces the rule. Application-level checks have already failed once here —
that is the current defect.

## 3.2 Defect two: a code can silently change hands

### The mechanism, verified

Binding is **overwrite**. the bind path in `activation.pb.js`:

```js
// Bind device
rec.set("bound_fingerprint", fp);
rec.set("activated_at", new Date().toISOString());
$app.dao().saveRecord(rec);
```

The only guard is the branch above it (line 128-165):

```js
if (boundFp) {
    if (boundFp !== fp) return e.json(403, {code:403, message:"Code bound to another device"});
    ...
}
```

So a *different* fingerprint **is** correctly refused at 403 — when it reaches
this hook with the right code. What is missing is everything around it:

* **No record that an attempt happened** beyond the generic
  `activation_attempts` row.
* **No operator-visible state** distinguishing "one device, stable" from
  "the code has changed hands".
* **No re-bind path.** Moving a code to a new machine requires `codes.unbind`
  first — which is correct, but is currently the *only* route, and §3.3
  explains why that route is being hit more often than it should be.

### The reported failure — and what actually caused it

> *"I was actually forced to unbind my test code for my test machine because it
> no longer recognised my device as the one originally bound despite nothing
> changing."*

This is the important one, and the cause is **fingerprint instability**, not a
binding bug. `bound_fingerprint` correctly held the *old* value; the device
simply presented a *different* `fp`, so the `boundFp !== fp` guard fired — the
guard working exactly as designed against an input that should never have
changed.

### Why the fingerprint changes

`locus/device.rs` computes the fingerprint by `sha256` over whatever hardware
identity is available, degrading through weaker combinations, and **finally to
a random value**:

```rust
fn compute() -> String {
    match platform_sources() {
        Some(fingerprint) => fingerprint,
        None => random_fingerprint(),
    }
}
```

On Linux (the Linux fingerprint sources in `device.rs`) the chain is:

```
machine_id + first NIC MAC   →   machine_id + hostname   →   machine_id
```

and `combine()` requires each candidate to be ≥ `MIN_ENTROPY` (8) bytes or it
falls through. So the fingerprint changes if **any** of these is true:

| Trigger | Effect |
|---|---|
| `/etc/machine-id` differs (reimage, some VM clones, container rebuild) | Primary source changes → new fingerprint |
| machine-id **absent/unreadable** and the NIC MAC changes | Falls back to `machine_id + hostname`; if machine-id is empty this is just hostname, so a **hostname change** alters it |
| All sources empty (bare container/VM) | `random_fingerprint()` — random bytes, persisted **only in the client's own config** |
| Config file lost, reset, or AppData wiped | A `random_fingerprint()` value is **not re-derivable**, so a new random one is generated |

That last row is the killer. The random value is cached for the process
lifetime (`OnceLock`) but its persistence is the client's own state file. Lose
that file and the device becomes a stranger to the hub — permanently, and with
no self-service recovery.

### The fix, in three parts

**Part A — persist the fingerprint where a reinstall cannot lose it.**

Today the fingerprint is written into the app's own config (the same store that
holds `activation_code`, `locus_tier`, etc., the Locus fields on `IVerge` in `config/verge.rs`). It
must instead live somewhere that survives the app being removed and
reinstalled, and is machine-scoped rather than user-scoped:

* **Windows:** a machine-wide location outside the roaming profile.
* **macOS:** a machine-scoped preference.
* **Linux:** `/etc/machine-id` is already stable; the risk here is the
  random fallback, so persist it in a machine-scoped path.

This alone removes the common failure: a student who reinstalls the client
keeps their entitlement.

**Part B — prefer durable sources, and make the fallback stable.**

Re-order and strengthen the chain so the *most* stable source wins, and so the
random fallback is written somewhere durable the first time it is generated
(Part A's location) rather than being re-rolled. The existing cache comment
already recognises this hazard:

> *"if hardware sources are unavailable the value is random, and an uncached
> random value would differ between the activation and the first heartbeat,
> immediately unbinding the device from itself."*

That comment describes an in-process cache. The bug being reported is the
*across-install* version of the same problem.

**Part C — make re-binding an explicit, audited operator action.**

Even with A and B, a real machine replacement happens (laptop dies, student
upgrades). Today that requires `codes.unbind` then a re-activation — two steps,
and the first one **clears** `bound_fingerprint` while also writing
`unbound_at`/`unbind_reason` (the `codes.unbind` action in `admin_console.pb.js`).

Add `codes.rebind` as a first-class action:

```
action: "codes.rebind"
body:   { code, reason, expected_fingerprint? }
```

It must:

* require a **reason** (the existing unbind already does, and it lands in the
  audit trail — keep that discipline),
* **not** clear `expires_at` (**this is the critical one** — see §3.4),
* record the old and new fingerprint prefix in `code_events`,
* optionally take `expected_fingerprint` so an operator cannot fat-finger a
  rebind onto the wrong code.

### On "refuse outright"

The operator's decision is that a second code on a bound device is refused. The
implementation must be clear about the consequence:

> **A refused student cannot fix it themselves.** Their only route is to
> contact whoever sold them the code, who contacts the operator, who performs
> an audited re-bind. That is the same friction that forced the operator to
> unbind their own test machine.

So this decision is only safe if the re-bind/unbind path is **fast and
trustworthy** — searchable by name
([`01-identity-without-pii.md`](01-identity-without-pii.md) §1.3), audited, and
not silently destructive (§3.4). Documenting that dependency is part of the
design, not a caveat to it.

## 3.3 The fragile `403` contract — do not make it worse

`activation.pb.js` returns **403 for two entirely different conditions**:

* `"Code suspended"` (line 151)
* `"Code bound to another device"` (line 150)

The client disambiguates **by wording** (the 403 classifier in `locus/activation.rs`):

```rust
403 => {
    if response.message.to_ascii_lowercase().contains("suspended") {
        ActivationOutcome::Suspended
    } else {
        ActivationOutcome::BoundToAnotherDevice
    }
}
```

This is a genuine contract with deployed clients, and the code says so:

> *"that is fragile, and it is the existing contract with deployed clients: the
> hub must not reword these messages without shipping a matching client.
> `activation_contract_test` pins the exact strings against the live hook."*

**Implication for this redesign:** the new `DeviceAlreadyActivated` outcome
must use a **new status code or a new message string that the existing client
will not mistake for `Suspended`**. Do not reuse 403 with a message containing
the word "suspended". A new 409 is cleanest and cannot be confused by any
deployed build. Any such change must update `activation_contract_test` in the
same commit.

## 3.4 The destructive combination to avoid

There is a trap that a careless implementation of this redesign would fall into,
and it must be called out explicitly:

> `codes.unbind` clears `bound_fingerprint` **and `activated_at`**
> (`codes.unbind` in `admin_console.pb.js`), and the term model computes the expiry from
> `activated_at` (…if it is ever recomputed).
>
> **Therefore: unbinding must never cause a student's remaining term to be
> recomputed from a fresh `activated_at`.** A student moved to a new laptop
> must keep their existing `expires_at` untouched.

This is why §3.3 of [`02-term-and-renewal.md`](02-term-and-renewal.md) insists
`expires_at` is *materialised* rather than derived on read. Materialisation
means an `activated_at` reset cannot silently reset a paid term. The requirement
is restated in [`05-migration-and-live-data.md`](05-migration-and-live-data.md)
as a migration invariant, because it is the most likely way a live student loses
time.

## 3.5 What cannot be verified here

Stated plainly, because this is the part most likely to be assumed working:

* **That the fingerprint survives a real reinstall** on Windows and macOS. This
  needs the actual OS, a real installer, and a reinstall cycle. No test in this
  repo can prove it.
* **That a machine-scoped storage location is writable without elevation** in
  every deployment shape. This is an empirical question per platform.
* **That `machine_id`-based identity is stable** across the specific
  virtualisation and dual-boot situations students actually have.

These go in [`10-open-questions.md`](10-open-questions.md) as hardware-gated,
and the implementation plan sequences them as a phase that cannot be closed
without a real machine.

## 3.6 Related reading

* Why identity is not stored → [`01-identity-without-pii.md`](01-identity-without-pii.md)
* The term invariant unbind must not break → [`02-term-and-renewal.md`](02-term-and-renewal.md) §2.3
* The migration that protects live bindings → [`05-migration-and-live-data.md`](05-migration-and-live-data.md)
* The operator surface for rebind → [`06-console-and-operator.md`](06-console-and-operator.md)
