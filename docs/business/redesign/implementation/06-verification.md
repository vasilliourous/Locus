# 6. Verification — the commands, and what each one proves

```
audience:    builder
status:      design-record
authoritative-for: the verification commands and what each proves
verified-against: docs/reference/DEVICE-IDENTITY.md, docs/reference/FIXES.md
```

> What to run, what it establishes, and — more usefully — what it does not.

---

## 6.1 The full suite

```bash
# ── Repository guards ──
server/scripts/check-consistency.sh

# ── Everything parses ──
for f in server/pb_hooks/*.js; do
  node -e "new Function(require('fs').readFileSync('$f','utf8'))" || echo "FAIL $f"
done
for f in server/scripts/*.sh server/modules/*.sh; do bash -n "$f" || echo "FAIL $f"; done
for f in server/scripts/*.py; do python3 -c "import ast;ast.parse(open('$f').read())" || echo "FAIL $f"; done

# ── Client ──
cd client
cargo test --offline
cargo clippy --offline --all-targets --features clippy -- -D warnings
pnpm typecheck && pnpm test && pnpm lint

# ── Console ──
cd server/console
./node_modules/.bin/tsc --noEmit -p tsconfig.json
./node_modules/.bin/vite build --logLevel error
```

## 6.2 What each guard actually protects

### `check-consistency.sh`

The project's mechanism for the failure class that has cost it the most: a value
that must agree across Rust / goja JS / Python / shell with nothing enforcing
agreement. Its header names four real incidents, each of which was silent.

| Group | Checks | What a failure means |
|---|---|---|
| 1 | Platform keys identical across every hook; `contract.rs` constants | A platform would be silently omitted from a release or an offer |
| 2 | Frozen wire names (`uot_port`, `download_*`/`update_*`) | **Previously caused a fleet-wide UDP outage** — a silent no-op |
| 3 | **Every hook-written field exists in the schema** | The `unbound_at` class: PocketBase silently discards a write to a non-existent column, so an audit trail *looks* implemented and records nothing |
| 4 | The cipher is defined once (`SS_METHOD`) | A duplicated cipher string drifts |
| 5 | No tracked path contains `:` | A colon breaks the **Windows checkout entirely** while looking green on Linux |
| 6 | PocketBase hook traps | File-scope helpers; `findRecordsByFilter` (returns zero rows, silently); token comparison; **headers read from the wrong place** — a fallback that had never run; **a raw fingerprint written to a binding** (must be normalised, or the index lookup cannot find the row) |
| 7 | Version agreement (informational) | The three manifests, and prose that names a stale version |
| 8 | Documentation cross-references | Every markdown link's target file exists, and every `#fragment` matches a real heading slug |

**Group 3 is the one this work leaned on.** It confirms that `term_days` and
`term_kind` (and, at the time, the `device_bindings` fields) are declared. **Read
the count from the guard's own output** — `bash server/scripts/check-consistency.sh
| grep 'hook-written'` — rather than from a number here: it is derived and it
moves as hooks change. If a future change adds a hook write without a schema
field, this fails.

### The Rust tests

The workspace runs the client's whole test set across all suites. **Read the
current figure from the runner** (`cd client && cargo test`) rather than from
this sentence: it is a derived count and it changes with every commit. The ones
this work touched or added:

| Test | Pins |
|---|---|
| ~~`maps_the_one_code_per_device_refusal_to_its_own_outcome`~~ | **REMOVED 2026-10** — the 409 match arm is kept but no longer fires; a code is not device-tied |
| `the_active_wire_shape_is_camel_case` | `rename_all_fields` on `SubscriptionStatus` — removing it fails the test |
| `activation_contract_test` | The 403 message-substring contract against the live hook (the 409 half is historic) |
| `version_consistency.rs` | The three manifests agree |
| `user_facing_messages_have_no_collapsed_whitespace` | No run of 3+ spaces in a message a student sees |

**Verified, not assumed:** the 409 test was checked by removing its `match` arm
and watching it fail — a test that cannot fail is not a guard. *(The arm it
pinned is gone from the live path as of 2026-10; the test remains as the record
of the discipline, which is the durable part.)*

**Correction (2026-10-01).** This table listed `activation_contract_test` as an
existing guard, but no such test existed — the name was cited in six documents
and two code comments and nothing enforced the contract. It now exists at
`client/src-tauri/tests/activation_contract.rs`, with the same
verified-by-failing discipline: the suspension wording was mutated in
`activation.pb.js` and the test was watched to fail before the hook was restored
byte-identical.

### The console build

`vite build` is the real check on the `.vue` files — `tsc` covers the script
blocks, but the SFC template is only compiled by the build. Afterwards, confirm
the new surface actually reached the bundle:

```bash
grep -o "codes.renew\|codes.rebind\|codes.set-term\|device.get\|devices.list\|expiring_within_days" \
  server/console/dist/assets/*.js | sort -u
```

> **2026-10 note.** After the device-binding removal, the live action set is
> `codes.renew`, `codes.set-term`, `codes.unbind` and the worklist key
> `expiring_within_days`. `codes.rebind` still appears in the bundle but answers
> **410** (a tombstone, not a feature), and `device.get`/`devices.list` are gone
> from the console. Grep for the tombstones and expect them — do not "restore"
> them.

## 6.3 The checks that are not in the suite

Some properties have no test runner in this repo, so they were exercised
directly. Reproduce them with:

### Term arithmetic

Extract `expiryFromTerm` from `activation.pb.js` and check:

| Case | Expected |
|---|---|
| `70` days from a fixed instant | exactly 70×24h later |
| `0`, absent, `null`, negative, `"abc"`, `""` | **`null`** — never a date |
| `"30"` (string) | works (PocketBase may return a number as a string) |
| 100 days across a DST boundary | exactly 100×24h (UTC arithmetic) |

### Renewal arithmetic

| Case | Expected |
|---|---|
| Renewing early (14 days left) | extends **from the existing expiry** |
| Renewing a lapsed code | starts from **now** |
| Renewing exactly at expiry | a full term |
| **Invariant** | the new expiry is `>= max(now, old expiry)` — a renewal can only move a date forward |

### The worklist window

Against a 7-day window: a code 3 days out is **included**, one exactly 7 days
out is **included** (the boundary), and **excluded** are: 20 days out, already
lapsed, no expiry (permanent), and suspended.

### The binding lifecycle

Five scenarios, which is how the force-delete bug was found:

### The binding lifecycle — REMOVED 2026-10, kept as the record

> The five scenarios below describe **device binding**, which was removed. They
> no longer describe live behaviour; a code is single-use and untied to any
> device, so scenarios 1–5 are all superseded by one rule: *re-activating any
> code succeeds and restores the entitlement*. Kept because the force-delete bug
> (scenario 4) is the origin story of a still-live guard.

1. Device binds A, then tries B → **refused**. *(superseded)*
2. A *different* device tries a code already bound → **403**, not 409. *(superseded)*
3. Unbind frees the device to bind elsewhere → **succeeds**. *(superseded)*
4. Force-delete a bound code, then bind another → **succeeds** (the fix). *(superseded)*
5. Re-activating the same code → **idempotent**. *(still true — this is the live rule)*

### The backfill

Dry-run against the real PocketBase space-separator date format
(`2027-09-19 00:00:00.000Z`, which strict RFC3339 rejects), plus:

| Input | Expected |
|---|---|
| activated + expires, sane span | derives the term |
| never activated | **declines** — mint-relative, no term provable |
| already has a term | **skips** |
| a ~4-year span | **declines** — looks mint-relative |

## 6.4 What none of this can establish

The limit of the suite, stated so no one mistakes green for working:

* **Nothing has met a real database.** See
  [`05-not-yet-true.md`](05-not-yet-true.md).
* ~~**PocketBase's unique index has not been observed rejecting a duplicate.**
  That is the guarantee behind one-code-per-device, and it is unverified.~~
  **REMOVED 2026-10** — no uniqueness constraint is in play.
* **No renewal has reached a running client.**
* ~~**The fingerprint's reinstall behaviour** needs real Windows/macOS hardware.~~
  **REMOVED 2026-10** — nothing depends on fingerprint stability any more.
* **goja's behaviour** differs from V8 in ways this project has been bitten by
  repeatedly (no hoisted file-scope functions, `findRecordsByFilter` returning
  zero rows, `new Date("... ...Z")` returning `NaN`). Syntax checking a hook in
  node proves it *parses*; it does not prove it *runs*.

## 6.5 The checks to run first after deploying

In order, because each depends on the one before:

1. `check-consistency.sh` — confirms the schema and the hooks agree.
2. **The unique index rejects a duplicate** — the whole guarantee rests on it.
3. **A renewal on a test code** — before→after in the response, and a `renewed`
   event in the history.
4. **The second-code refusal** — a 409, and the message a student would read.
5. **The migration dry-run** — read the diff before applying anything.
6. **A real client's expiry moving** after a renewal.
