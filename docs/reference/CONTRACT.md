# The frozen wire contract

```
audience:    builder
status:      live
authoritative-for: which wire names, endpoints and platform keys an installed client depends on
data:        contract.lock.json (this directory) — the machine-readable half
guard:       server/scripts/check-consistency.sh §37
```

> **The `contract.lock.json` beside this file is the contract. This page is how to
> read it and what to do when it changes.** Where the two disagree, the JSON is
> right and this page is the bug — same rule as everywhere else in this repo.

---

## Why this exists

Every defect class in [`FIXES.md`](FIXES.md) has one shape: a name that must
agree across a boundary, with nothing enforcing agreement.

| Incident | The boundary that was unguarded |
|---|---|
| `uot_port` vs `server_port_uot` | hub emitted one spelling, client declared another — UDP-over-TCP dead fleet-wide, both sides individually "correct" |
| `download_<platform>` vs `update_<platform>` | two publish scripts disagreed; a Windows client was handed a Linux binary |
| `bound_this_device` vs `already_used` | a status rename stranded reinstalling students on the code prompt |
| `macos_intel` / `darwin-*` | platform key and artifact filename deliberately asymmetric; "tidying" one breaks that platform's publishing |

Sections 33 and 34 of `check-consistency.sh` guard two of these boundaries — the
status struct against its TypeScript interface, and the IPC command set across
three sites. **Neither looks at the hub boundary**, which is the one an installed
client cannot renegotiate. Until this file existed, the set of names a deployed
3.3.x build depends on was recorded only in prose and in the heads of whoever
shipped it.

---

## What is frozen

| Group | Source of truth | Examples |
|---|---|---|
| **Struct fields** | `client/src-tauri/src/locus/contract.rs` | `device_fingerprint`, `server_config`, `uot_port`, `expires_at` |
| **Enum wire values** | `LookupStatus` in the same file | `ok`, `unbound`, `already_used`, `suspended`, `expired`, `not_found` |
| **Endpoints** | `routerAdd()` in `server/pb_hooks/*.js`, and the call sites in `locus/` | `POST /api/activate`, `POST /api/heartbeat`, `GET /api/release` |
| **Platform keys → artifact names** | `contract.rs` | `macos_arm` → `locus-darwin-arm64` |
| **Proxy and group names** | `client/src-tauri/src/locus/tier.rs` | `Locus` / `Locus Auto` / `Locus-UoT` |
| **Hub URLs** | `contract.rs` | canonical and legacy hub hosts |
| **IPC commands** | `cmd/locus.rs`, `lib.rs`, `client/src/**` | `locus_status`, `locus_connect`, … |

The struct fields are **snake_case on the wire**. `contract.rs` carries no
`rename_all`, which is load-bearing: `device_fingerprint` and `server_config` go
out in snake_case and the hub reads them that way. Adding
`rename_all = "camelCase"` to that struct would break every deployed client, and
the lock is what makes that visible before a release is built.

---

## The two rules

**A name the lock holds that the tree has lost is a BREAK.** Removed field, respelled
field, dropped enum value, removed endpoint, renamed platform artifact, changed
hub URL, removed IPC command. The build fails, naming it.

**A name the tree has that the lock does not hold is ADDITIVE.** Adding a wire
field is sometimes correct, so it does not fail — but it is reported, because a
field added to one side and forgotten on the other is exactly the drift this
exists to stop.

### When you intend to change the contract

```bash
# 1. Make the change in code.
# 2. Regenerate the lock. The diff IS the review artifact.
python3 server/scripts/contract-snapshot.py --write

# 3. Read the diff. Every removed or respelled name is a decision about
#    installed clients, not a side effect. If you did not intend it, you
#    have just found a bug.
git diff docs/reference/contract.lock.json

# 4. Confirm the guard still fails on the defects it claims to catch.
bash server/scripts/test-contract-guard.sh
```

`python3 server/scripts/contract-snapshot.py --explain` prints what the tree
currently says, without touching the lock.

### Removing something on purpose

You cannot simply delete it from the lock — that is the operation the guard exists
to prevent, and doing it silently is the failure. A removal needs:

1. **Proof the fleet no longer depends on it.** Check
   [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5 for the minimum client version
   still in the field, and say how you know.
2. **An entry in [`FIXES.md`](FIXES.md)** naming what was removed, which version
   stopped sending or reading it, and what breaks for a client older than that.
3. **The `--write` in the same commit**, so the diff shows the removal rather than
   hiding it among additions.

---

## What this guard does not cover

Stated plainly, because a guard's reputation outruns its scope:

- **It does not check values, only names.** A field whose meaning changed keeps its
  name and passes. Semantics are §9's job for the constants it knows, and the
  integration tests' job for the rest.
- **It does not check the hub's response *shape* against the client's structs.**
  It records that `/api/heartbeat` exists on both sides, not that what the hook
  returns deserialises into `HeartbeatResponse`. That gap needs a live or replayed
  response fixture; it is **not** covered today.
- **It does not reach the deployed host.** The lock describes this checkout. A hook
  that is correct here but not redeployed is inert — see
  [`../operate/OPS.md`](../operate/OPS.md) and `server/scripts/hooks-sync.sh`.
- **It cannot see a rename that kept both old and new.** Adding `already_used`
  beside `bound_this_device` is additive and passes; whether the old spelling is
  still needed is a judgement the guard cannot make.

The first two are the open items. Both are recorded in
[`STILL-OPEN.md`](STILL-OPEN.md).
