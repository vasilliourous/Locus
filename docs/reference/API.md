# Chrysalis API Reference

```
audience:    builder
status:      live
authoritative-for: the hub's HTTP API contracts (activation, heartbeat, releases, code lookup)
verified-against: server/pb_hooks/*.pb.js
```

> Complete API contracts for the Chrysalis server, as implemented by the
> PocketBase JS hooks and Caddy reverse proxy.

---

## Base URL

`https://networkingguides.jadedns.uk` — the canonical hub, and the default
compiled into the client (`chrysalis::contract::HUB_URL`).

**Two names serve this API.** The hub's previous name,
`https://networkingguides.duckdns.org` (alias `contract::LEGACY_HUB_URL`), is on
the **same Caddy site block** and answers every endpoint identically — not by
redirect. Every build before 3.3.2 has it compiled in, and the only channel that
could tell a device otherwise (the heartbeat) needs a working hub to reach, so
removing it strands those devices permanently at the activation gate. Both names
are in `05-caddy.sh`; `check-consistency.sh` §32 asserts it.

All endpoints are served through Caddy reverse proxy to PocketBase on `127.0.0.1:8090`.

---

## 1. Activation

### `POST /api/activate`

Activates a code. One code is **one machine at a time**. The first activation
stamps `codes.activated_at`; a later activation of the same code is allowed
**only if the device holding it signed out afterwards**
(`released_at > activated_at`, written by `/api/release-entitlement`), and
answering it consumes that sign-out by re-stamping `activated_at`.

Everything else — a code still signed in elsewhere — is refused with `409`. The
reinstall path stays open because the student's own sign-out is what reopens it;
there is no device identity to recognise them by, so that sign-out is the only
proof available, and it is also the only proof an honest student can give.

> **Why not simply re-issue to any presenter.** That was the behaviour until
> 2026-11-27, and it made one purchase serve any number of machines for the whole
> remaining term with nothing on the hub recording a second device. Clearing
> `activated_at` on a sign-out instead was rejected for a different reason: the
> endpoint is authenticated by possession of the code, and codes are printed on
> cards, so clearing it would let the holder of a photo free a paying student's
> code. See `docs/reference/FIXES.md`, "One code signed in on every machine".

**Rate limited:** 5 attempts per 10 minutes per rate key (Caddy + JS hook double-gated).

**Request:**
```json
{
  "code": "RQ-ABCD-EFGH-JKMN-T",
  "fingerprint": "a1b2c3d4e5f6...abcdef"
}
```

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `code` | string | ✅ | Activation code with hyphens |
| `fingerprint` | string | ❌ | A rate-limit and support correlator, **not an identity**. Non-alphanumerics are stripped for the attempt key and the value is truncated in the attempt log; it is compared against nothing |

**Response `200` (success):**
```json
{
  "code": 200,
  "message": "Activation successful",
  "tier": "free",
  "device_fingerprint": "a1b2c3d4e5f6...abcdef",
  "server_config": {
    "server": "networkingguides.jadedns.uk",
    "server_port": 8443,
    "password": "...",
    "method": "2022-blake3-aes-256-gcm",
    "uot_port": 8447
  },
  "udp_relay": true,
  "expires_at": "2026-05-14T00:00:00.000Z"
}
```

> **`expires_at` is the entitlement's end as an instant**, or `null` when the
> code has no expiry. It is returned on both success paths — a first activation
> and a repeat activation — so a newly activated client can show the date
> immediately rather than waiting for its first heartbeat.
>
> **It is computed from the code's TERM at the moment of activation**, not from
> a date fixed when the card was minted. A code carrying `term_days` of 70
> activated today expires in 70 days; the same card sold in a month expires a
> month later. A code with no term keeps whatever absolute expiry it carries,
> and never `null`-ing that is deliberate: clearing it would silently turn an
> un-migrated code into a permanent one.
>
> `null` and "the empty string" are **different**: the client distinguishes "no
> date recorded" from "the date is blank", and only a *known past* date lapses.
> A `null` must therefore never be read as expired.

> **`method` is `2022-blake3-aes-256-gcm`** — Shadowsocks 2022, not the legacy
> AEAD (`aes-256-gcm`) it replaced. SS2022 is a different wire protocol with an
> encrypted fixed-length header and replay protection. The `password` is the
> **base64 encoding of a 32-byte key**, not a passphrase; a client given a key
> of the wrong length fails at handshake, not at parse time.
>
> The cipher is defined once, in `server/modules/00-env.sh` (`SS_METHOD`), and
> every hook, seed script and config template reads it from there. Clients pass
> the value straight through to the engine — mihomo builds all `2022-blake3-*`
> ciphers — so no client logic depends on the specific string.
>
> **There is no legacy fallback.** A client from before this migration speaks
> `aes-256-gcm` and will not connect; it must be updated. The server does not
> serve both.

**Response `200` (re-activation — a redeemed code):**
```json
{
  "code": 200,
  "message": "Already activated",
  "tier": "free",
  "server_config": { "...": "as above" },
  "udp_relay": true,
  "expires_at": "2026-05-14T00:00:00.000Z"
}
```

> This body is returned when the student's own sign-out is consumed, and the
> message is `"Activation successful"` for that reason: it is a first redemption
> on this device. It returns the current tier config so a client can refresh
> stale connection parameters (a rotated password, a moved server).
>
> **It is not what a second machine gets.** A code held elsewhere is `409` —
> see the error table below. There is no fingerprint check either way: nothing
> is bound to a device, and `/api/release-entitlement` is what distinguishes the
> student moving machines from a stranger holding their card.
>
> **`device_fingerprint` is echoed on the first-activation path only**, because
> deployed 3.2.x clients read the key. It no longer means "the device this code
> is bound to"; nothing is bound, and current clients ignore it.

**Error responses:**

| Status | Code | Message | Meaning |
|:------:|:----:|---------|---------|
| 400 | 400 | "Missing code" | No code provided |
| 400 | 400 | "Invalid code format" | Luhn-mod-N validation failed |
| 403 | 403 | "Code suspended" | Admin suspended the code |
| 409 | 409 | "This code is already in use on another device. Sign out on that device first, then enter the code here" | Redeemed, with no sign-out after that redemption. Also carries `already_in_use: true`. **Not** 403: a client that read a shared code as a suspension would show the kick-out screen to a paying student |
| 404 | 404 | "Code not found" | Code doesn't exist in DB |
| 410 | 410 | "Code expired" | Past expires_at date |
| 429 | 429 | "Too many attempts" | Rate limit hit |
| 500 | 500 | hub message | Unexpected failure |

> Messages are the exact strings `activation.pb.js` returns; the client matches
> on status code, but a support conversation will quote the text, so it is kept
> verbatim here.

> **403 means suspended on this route, and the client recognises it by the frozen
> substring `suspended`.** That wording is a contract pinned by
> `activation_contract_test`; the operator's own `suspended_reason` is appended on
> the heartbeat, never substituted here.
>
> **There is no device BINDING, and no authorisation on the fingerprint.** Binding
> was removed in 2026-10: `device_fingerprint` is a rate-limit and support key, not
> an identity, and nothing is compared against it on this route.
>
> **That is not the same as "activation always succeeds", and an earlier revision of
> this paragraph said it was.** Since 2026-11-27 the one-machine rule is enforced by
> the SIGN-OUT, not by an identity: a code whose `released_at` is not later than its
> `activated_at` is refused here with `409` (row above), because the only credential
> in this system is possession of the code and a sign-out is the only proof an honest
> student can give. Do not read "no binding" as "re-activation is always allowed" —
> it is allowed exactly when the previous holder signed out.

---

## 2. Heartbeat

### `POST /api/heartbeat`

Periodic health check. Returns suspension status, the subscription expiry, an
update signal and a refreshed tier config.

**Rate limited:** 1 request per 10 seconds per IP.

**Request:**
```json
{
  "code": "RQ-ABCD-EFGH-JKMN-T",
  "fingerprint": "a1b2c3d4e5f6...abcdef"
}
```

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `code` | string | ✅ | Full activation code |
| `fingerprint` | string | ❌ | Hardware fingerprint. A **rate-limit and support key only** — it is logged truncated and compared against nothing |
| `token` | string | ❌ | **Retired.** A frozen wire field; the hub no longer mints or accepts one |

> **`code` is the only credential.** It used to be possible to authenticate with
> a session token minted at device recognition, for a device the hub had restored
> without a code. Recognition and the token are both removed: a client always
> holds its activation code, and keeps it across a reinstall itself (see
> [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md)).
>
> `token` is still accepted in the request shape so a deployed 3.2.x client's
> body is not rejected, but it resolves to nothing: a request with no `code`
> answers `400 Missing code`.
>
> A `401` is treated as a **refusal**, like `403`, `404` and `410`. It used to
> be the token path's "renew me" signal; with tokens gone it means the hub will
> not accept this code.

**Response `200` (strike tier — carries a UoT endpoint):**
```json
{
  "status": "ok",
  "server_time": "2026-09-19T10:09:42.945Z",
  "tier": "strike",
  "server_config": {
    "server": "networkingguides.jadedns.uk",
    "server_port": 8445,
    "password": "...",
    "method": "2022-blake3-aes-256-gcm",
    "uot_port": 8446
  },
  "udp_relay": true
}
```

> **`uot_port` is a frozen wire key.** The tier config JSON is passed through
> verbatim, so the UoT endpoint arrives under its stored name. Do **not** rename
> it to `server_port_uot`: clients already in the field read `uot_port`, and an
> unrecognised key is a silent no-op rather than an error — which is exactly how
> UDP-over-TCP was dead fleet-wide until 2026-09-19 (FIXES.md 29). `udp_relay`
> and `uot_port` are **both** required before the client sends UDP over TCP.

**Response `200` (with an update advertised):**
```json
{
  "status": "ok",
  "server_time": "2026-09-19T10:09:42.945Z",
  "tier": "free",
  "update_available": "<version>",
  "update_url": "https://networkingguides.jadedns.uk/updates/<version>/chrysalis-linux-amd64",
  "update_sha256": "abc123...",
  "update_linux": "https://.../updates/<version>/chrysalis-linux-amd64",
  "update_windows": "https://.../updates/<version>/installer-Chrysalis_<version>_x64-setup.exe",
  "update_macos_intel": "https://.../updates/<version>/chrysalis-darwin-amd64.app.tar.gz",
  "update_macos_arm": "https://.../updates/<version>/chrysalis-darwin-arm64.app.tar.gz",
  "update_sha256_linux": "abc123...",
  "update_sha256_windows": "def456...",
  "update_sha256_macos_intel": "ghi789...",
  "update_sha256_macos_arm": "jkl012..."
}
```

Update fields are present when the `update_config` collection has a single
**active** record. There is no rollout gate: publishing *is* offering, and
`active` is the only off switch (the `rollout_percent` column still exists in
the schema but is inert — nothing reads it). The client refuses any advertised
version that is not strictly newer, so a stale or rolled-back row cannot
downgrade a device. See `../operate/UPDATE-SYSTEM.md` §2.

`update_url` / `update_sha256` are the **legacy single-platform** fields and
describe the linux binary only. They exist so clients predating per-platform
support still update. A client that finds its own platform's field missing falls
back to them — which means a record missing `download_<platform>` silently
offers Windows and macOS a Linux executable. The `update_<platform>` fields come
from the record's `download_<platform>` keys — a rename the hook performs on the
way out (`server/pb_hooks/heartbeat.pb.js`), frozen because deployed clients read
`update_<platform>` (FIXES.md 31).

**Error responses:**

| Status | Code | Message | Meaning |
|:------:|:----:|---------|---------|
| 400 | 400 | "Missing code" | No `code` in the request body |
| 403 | 403 | `"Account suspended — "` + the operator's reason | Code is suspended |
| 404 | 404 | "Code not found" | Code doesn't exist or is unparseable |

> **Expiry is enforced on every heartbeat, not just at activation.** A code that
> passes its `expires_at` stops working here even on a device that is already
> bound, which is what makes the 7-day grace period bounded.

> ### The suspension 403 carries two extra keys, and they are ADDITIVE
>
> ```json
> {
>   "code": 403,
>   "message": "Account suspended — chargeback",
>   "suspended": true,
>   "suspended_reason": "chargeback"
> }
> ```
>
> The `message` prefix `Account suspended — ` is **frozen**: the client classifies
> a 403 by the substring `suspended`, and deployed builds that cannot be updated
> depend on it. The operator's own `suspended_reason` is **appended**, never
> substituted. A hub that replaced the sentence would make every deployed client
> report a suspension as some other, wrong refusal.
>
> `suspended` and `suspended_reason` are for clients that would rather switch on a
> flag than parse a sentence — the kick-out screen renders the reason in its own
> layout. An older client ignores both and falls back to the message, which is why
> adding them is safe.
>
> ### Suspension is REVERSIBLE, and that is a contract
>
> The client **keeps** `codes.activated_at`, its stored code, tier and expiry when
> this 403 arrives. That is deliberate: an unsuspend then restores the session with
> no re-entry and no restart, which is what makes a kick-out an operator action
> rather than a support call. Every **other** refusal (404 unknown, 410 expired)
> still clears the credential client-side, because there is nothing to come back
> to.
>
> The trade, stated: a suspended device holds a live bearer credential on disk. It
> cannot be used (the tunnel is down and the heartbeat loop is stopped), and a
> sign-out still removes it entirely. See `chrysalis::store::record_suspension`.

---

## 3. Device Recognition

### `POST /api/device-recognise` — RETIRED

Answers a uniform `unknown` for every request. It is kept as a **tombstone**, not
as a feature: deployed 3.2.x clients call it once per launch, and without the
route they would get a `404` and log a transport error on a path that no longer
matters. `unknown` is the answer those clients already handle — fall through to
the code prompt.

It once asked whether *this device* already held a live entitlement, keyed on a
`verifier` and returning a session `token`, so a reinstalling student could skip
the code prompt. Device identity and the token are both removed; a student's
recovery path is the activation code, which the client keeps durably on the
device. See [`DEVICE-IDENTITY.md`](DEVICE-IDENTITY.md).

**Response `200` (always):**
```json
{"status": "unknown", "message": "This device is not recognised"}
```

The endpoint does **no** work: no database read, no rate limit, no token. It
cannot leak, because it cannot look anything up. `activation_contract.rs` asserts
the absence of `device_identities`, `token_hash`, `verifier` and any lookup in
the hook, so the retired machinery cannot quietly return.

There is no rate limit any more — nothing is looked up to limit.

---

## 4. Admin Release (the legacy `unbind-code` endpoint)

### `POST /api/admin/unbind-code`

Clears the single-use stamp (`codes.activated_at`), returning the code to the
unused pool so it can be activated by a **different** student. It is not how a
student moves to a new machine: a redeemed code already re-activates and
restores access without operator help.

It does **not** touch the tier or the expiry, because releasing must not change
what a code entitles its holder to. It records `unbound_at` and `unbind_reason`;
`first_seen_at` deliberately survives, so the console can tell "never activated"
from "issued once and released".
Requires a valid admin API token.

> The console's `codes.unbind` action calls the same operation through
> `/api/admin/console`; this route is the legacy SSH-era endpoint and does not
> accept the console's `X-Admin-Token` header.

**Rate limit:** None (admin endpoint).

**Request:**
```json
{
  "admin_token": "your-secure-token",
  "code": "RQ-ABCD-EFGH-JKMN-T",
  "reason": "Device lost/stolen"
}
```

| Field | Type | Required | Description |
|-------|------|:--------:|-------------|
| `admin_token` | string | ✅ | Must match `ADMIN_API_TOKEN` env variable on VPS |
| `code` | string | ✅ | Code to unbind |
| `reason` | string | ❌ | Optional reason for the audit log; defaults to `"Requested by admin"` |

**Response `200`:**
```json
{
  "code": 200,
  "message": "Code released successfully.",
  "tier": "free",
  "middleman": ""
}
```

**Error responses:**

| Status | Code | Message |
|:------:|:----:|---------|
| 400 | 400 | "Missing code" |
| 400 | 400 | "Code has not been used yet" |
| 403 | 403 | "Invalid admin token" |
| 404 | 404 | "Code not found" |
| 500 | 500 | "ADMIN_API_TOKEN not configured" |

---

## 5. Health

### `GET /api/health`

Standard PocketBase health check.

**Rate limit:** 100 requests per 10 seconds.

**Response `200`:**
```json
{
  "message": "API is healthy.",
  "code": 200
}
```

---

## 5b. Code Lookup (the pre-activation check)

### `POST /api/code-lookup`

A **read-only** pre-check: "is this code ready to activate?" It binds nothing and
never activates. The activation screen calls it before `/api/activate` so a
typo or a lapsed code is answered without spending one of the five activation
attempts.

**Response `200`** — `status` is one of:

| `status` | Client treats as | Meaning |
|---|:---:|---|
| `ok` | ready | Code exists and is ready |
| `unbound` | ready | Code has not been redeemed yet |
| `bound_this_device` | **ready** | Redeemed, and a sign-out is waiting (`released_at > activated_at`): the next activation will be admitted |
| `bound_other` | blocked | Redeemed and held by a machine that has not signed out: activation would answer `409` |
| `suspended` | blocked | Operator suspended the code |
| `expired` | blocked | Past `expires_at` |
| `not_found` | blocked | No such code |
| *anything else* | **ready** | Unknown status — proceed to `/api/activate`, which is the authority |

> ### `ready` is advisory, and the message must not be reassuring on its own
>
> The client shows `message` and then calls `/api/activate` **regardless of
> `ready`** for every status above except the four blocked ones. So the lookup is
> not a gate and cannot become one — `/api/activate` is the authority, and a
> client that refused on a lookup would strand every reinstalling student.
>
> That makes the message load-bearing in a way the status is not. There are two
> redeemed states and each message must name its own fact, because the student's
> next move differs: `bound_this_device` says the code was **signed out** and is
> ready here; `bound_other` says it is **in use on another device** and to sign
> out there first, since there is no device to name and no remote release. A
> message about what re-activating *does* for the caller, with no statement about
> the code, reads exactly like an untouched code — that is how "a client can use
> an already redeemed code" first looked like the hub not checking at all. Pinned
> by `a_redeemed_code_says_it_is_already_in_use` and
> `the_lookup_and_activation_agree_about_a_held_code` in
> `activation_contract.rs`.

> ### Both redeemed statuses are FROZEN NAMES, and `already_used` must not be sent
>
> An **available** redeemed code reports `bound_this_device`; one held by a
> machine reports `bound_other`. Both names are leftovers from the device-binding
> era, and both are kept because a client's `LookupStatus` is an allow-list whose
> `#[serde(other)]` catch-all is `Unknown`. `bound_this_device` is the **only**
> status every deployed build maps to `ready: true` for a used code, and
> `bound_other` is the third entry in the same enum, which every deployed build
> already decodes as *bound to another device* — which is now literally the
> situation.
>
> (`bound_this_device` used to describe every redeemed code, because activation
> admitted every presenter. It now describes only the case where that promise is
> still true: a sign-out is waiting. Both statuses must move together; sending
> `bound_this_device` for a held code promises a `ready` the hub refuses.)
>
> A **new** status string therefore reaches a deployed client as `Unknown`, and an
> older client read `Unknown` as *not ready* — so it refused to
> call `/api/activate` at all. This shipped once (2026-10-03): the hub began
> sending `already_used` for a redeemed code and every reinstalling student was
> stranded at the code prompt, unable even to update out of it because the
> updater runs behind this gate. The remedy was an operator releasing the code.
>
> So: **do not rename this status, and do not add a new one for an existing
> meaning.** A rename here is not cosmetic — it is a dead end for clients that
> cannot update their way out of it. Current clients tolerate unknown statuses by
> proceeding, but deployed ones do not.
>
> The client side is pinned by `activation_contract.rs`:
> `a_redeemed_code_reports_a_status_deployed_clients_accept` (the hub emits the
> frozen name) and `an_unknown_lookup_status_does_not_block_activation` (the
> client proceeds on an unknown status).
>
> **Current clients also *accept* the alias** `already_used` — `contract.rs`
> declares `#[serde(alias = "bound_this_device")] AlreadyUsed` — so a hub that has
> learned to send the descriptive name will not break a current build. That
> tolerance is **not** a licence to send it: it exists so the eventual migration
> is possible, not so it can be done now, because the deployed installs that
> cannot update their way out of the gate are the ones the frozen name protects.

---

## 5c. Device Release (the student sign-out record)

### `POST /api/release-entitlement`

A student pressed "Sign out on this device". The client forgets the code locally
(`client/src-tauri/src/chrysalis/signout.rs`) and this endpoint records that the code
was signed out of. **It is what frees the code**: `/api/activate` admits a
redeemed code only when `released_at > activated_at`, and this is the only
endpoint that writes `released_at`. A student who signs out and then installs
elsewhere gets in without an operator; a second machine anywhere else gets `409`.

**It writes a claim, and does not clear anything.** The distinction matters, and
it is why this is not simply the admin release:

- It does **not** clear `codes.activated_at`. Clearing it would free the code for
  anyone holding the card — a photo of a friend's, an unsold batch in a middleman's
  drawer — because the only credential here is possession of the code. Writing
  `released_at` frees it for **the next activation**, which is the student's own
  next move, and that activation spends the claim by re-stamping `activated_at`.
- It does **not** change `expires_at`, `tier`, or `suspended`. Consuming a claim
  moves `activated_at` and leaves the expiry alone, so signing out and in again
  gives the student the clock they already had rather than a new term.
- A failure here leaves the device signed out locally and the code still looking
  held on the hub. The client therefore reports "signed out, not confirmed"
  rather than failing the sign-out. **This is the one failure in the whole flow
  that costs the student something**: they will be refused elsewhere until an
  operator releases the code (§ `codes.unbind`), which is why the sign-out
  screen says so plainly.

**Authentication:** possession of the code, exactly like `/api/code-lookup`. No
admin token — a student cannot have one, and requiring it would make the feature
unreachable by the only person it is for.

**Rate limit:** two limits, both keyed on the requester. Caddy allows 10 per
10 minutes per IP (`release` zone); the hook enforces the same bound in its own
`release_` bucket in `activation_attempts`. The bucket is **deliberately separate
from `activate_`**: signing out is not an activation attempt, and consuming that
budget would lock a student out of the screen that explains their code.

**Request:**
```json
{
  "code": "RQ-ABCD-EFGH-JKMN-T",
  "fingerprint": "…"
}
```

`fingerprint` is the same rate-limit and support correlator every other call
carries. It is compared against nothing.

**Response `200` — recorded:**
```json
{ "code": 200, "message": "Release recorded — the code can be used on another device now", "released": true, "count": 2 }
```

**Response `404` — nothing to release:**
```json
{ "code": 404, "message": "Nothing to release", "released": false }
```

Five situations answer this way, deliberately indistinguishable: the code does
not exist, it was never redeemed, it is suspended, it has expired, **or it is
already released and not yet claimed**. Telling the
caller which would make this an unauthenticated **code-existence oracle** — the
request is authenticated by the very value being probed. The client maps `404` to
"nothing to release", which is a successful no-op for the student (their local
sign-out has already happened and is unaffected).

**Fields written:** `codes.released_at` (latest), `codes.release_count`
(monotonic), and one `code_events` row with `event = "released"`. The log line
and the event row record the code **prefix** and a truncated fingerprint only —
a code is a bearer credential and logs are things people paste.

> ### Why a claim, and not a cleared `activated_at`
>
> The obvious implementation is to clear `activated_at` here and be done. It is
> rejected for two reasons, and both are still true of the claim that replaced it:
>
> 1. **A student could reset their own paid term.** `expires_at` is derived from
>    the term on a first redemption. Consuming a claim re-stamps `activated_at`
>    and leaves `expires_at` alone, so signing out and in again is not a purchase.
> 2. **Anyone holding the code could free it.** The only credential on this route
>    is possession of the card. A claim is spendable exactly once and only by an
>    activation, so the holder of a photo gains nothing while somebody is signed
>    in — the honest student's sign-out is what empties the slot, and their next
>    activation fills it.
>
> The operator-visible side is `released_pending` in `codes.list` (the same
> comparison, so the console cannot disagree with the hub), the code detail's
> "Next activation" line, and an Overview check for a sign-out nobody claimed.
> Releasing a code **for someone else** — the lost-laptop case — remains a
> separate, deliberate operator action: `codes.unbind`, below.

**Error responses:**

| Status | Code | Message | Meaning |
|:------:|:----:|---------|---------|
| 400 | 400 | "Missing code" | No `code` in the body |
| 429 | 429 | "Too many attempts" | Rate limited; retry later |
| 500 | 500 | hub message | Unexpected failure |

> **The 500 arm is not a student-facing outcome.** The client's sign-out is
> best-effort at the hub and reports `notConfirmed` for anything it cannot
> interpret, so a hub fault here costs an audit row, never a student's access.

---

## 5d. Session Probe (the desync diagnostic)

### `POST /api/session-probe`

Answers **"is this code in term, and is the device holding it alive?"** as two
separate facts. It exists because the console answered the second question from
the first question's data, and a client that was heartbeating every five minutes
was displayed as signed out. The state model behind that is
[`AUTH-STATE.md`](AUTH-STATE.md); the procedure that uses this endpoint is
[`../operate/RUNBOOK-DESYNC.md`](../operate/RUNBOOK-DESYNC.md).

**Strictly read-only.** It writes nothing — not even `last_seen_at`. Every write
belongs to the flow that owns it (the beat writes liveness, `/api/release-entitlement`
writes `released_at`, activation writes `activated_at`). Repair is a separate,
audited, idempotent operator action (`codes.unbind`).

**Authentication:** possession of the code **and** the matching redacted device
fingerprint. A caller who cannot present the machine that first redeemed the code
gets the same bare `404` an unknown code gets, so this cannot be used as a
code-existence oracle. See `server/pb_hooks/session_probe.pb.js`.

**Rate limited:** 10 per 10 minutes per IP (Caddy `probe` zone, the activation
tunnel's own `probe` bucket, and the hook's `probe_` bucket — three doors, one
bound, so a support session cannot spend a student's activation budget).

**Request:**
```json
{
  "probe_version": 1,
  "code": "RQ-ABCD-EFGH-JKMN-T",
  "device_id": "a1b2c3d4e5f6",
  "client": {
    "app_version": "<version>",
    "platform": "linux",
    "auth_state": "activated",
    "last_heartbeat_ok_unix": 1765000000,
    "heartbeat_failures": 0,
    "grace_remaining_s": 590000,
    "hub_enforcement": 4,
    "recorded_refusal": null,
    "clock": { "unix": 1765000300, "utc_offset_min": 720 },
    "degradation": [],
    "transport": "direct"
  }
}
```

| Field | Required | Description |
|---|:--:|---|
| `probe_version` | ✅ | Schema version. An unrecognised value answers `409` rather than being interpreted — a half-understood probe is worse than a refused one. |
| `code` | ✅ | The activation code. |
| `device_id` | ✅ | The client's **already-redacted** 12-char fingerprint (`store::redact`). Compared to `codes.first_seen_fingerprint`; a mismatch is an indistinguishable `404`. |
| `client` | ❌ | The client's own view, when the operator is at the device. Omitted is normal and is reported as `null`, never defaulted. |

**Response `200`:**
```json
{
  "probe_version": 1,
  "server_time": "2026-11-25T10:09:42.945Z",
  "entitlement": "active",
  "liveness": "live",
  "hub_view": {
    "activated_at": "2026-11-20T08:11:02.000Z",
    "released_at": null,
    "released_pending": false,
    "suspended": false,
    "suspended_reason": null,
    "expires_at": "2027-03-01T00:00:00.000Z",
    "tier": "strike",
    "last_seen_at": "2026-11-25T10:04:12.000Z",
    "last_seen_age_s": 330,
    "first_seen_fingerprint": "a1b2c3d4e5f6"
  },
  "client_view": null,
  "lease_expires_at": "2026-11-25T12:20:12.000Z",
  "reason_code": "client_beating_hub_quiet",
  "divergence": []
}
```

**The two axes.** `entitlement` and `liveness` are never collapsed into one
label; a caller that wants a single badge has to choose, which is the point.

| `entitlement` | Meaning |
|---|---|
| `active` | In term, not suspended, no unspent sign-out claim. |
| `released-unclaimed` | `released_at > activated_at`: a sign-out is waiting to be claimed. The code is free for the next activation. |
| `suspended` | An operator stopped it. `suspended_reason` carries their words. |
| `expired` | Past `expires_at`. |
| `never-activated` | No `activated_at`. |

| `liveness` | Meaning |
|---|---|
| `live` | Beat within the lease (2 h 14 min, derived from the client's max backoff — `docs/state.toml` `[liveness]`). |
| `stale` | No beat within the lease, but within 7 days. |
| `absent` | No beat for over 7 days. |
| `unknown` | **`last_seen_at` is null.** Never render this as `absent`. |

> **`unknown` is a first-class answer.** A code that has never beaten since the
> column existed has no liveness, and reporting it as anything definite would
> accuse a device on no evidence. It is `null` in `hub_view.last_seen_at` and
> `unknown` here, and the console renders it as `—`.

**`reason_code` — what the operator should do:**

| `reason_code` | Meaning | Action |
|---|---|---|
| `in_sync` | Both sides agree. | none |
| `client_beating_hub_quiet` | The device is beating; the console's "signed out" was an inference from `released_at`. | none — a console defect, not a device one |
| `stale_release_claim` | A sign-out claim contradicts a beating device. | `codes.unbind`, or have the student sign in once |
| `hub_refused_client_hasnt_noticed` | Hub already refuses; the client has not beaten since. | wait one beat, or `beat_now` |
| `client_unreachable` | No recent beat. Killed, offline or quit — **this endpoint does not guess which**. | check the Account screen, or wait for TTL |
| `client_dropped_session` | Client lost its record; the hub still holds a live one. | student re-activates |
| `genuinely_signed_out` | Both sides agree it is signed out. | none |
| `clock_skew` | Client and hub clocks differ by >300 s. **Reported, never acted on.** | none — liveness uses the hub's clock |
| `unknown` | Nothing to conclude. | use the console |

**Error responses:**

| Status | Code | Meaning |
|:--:|:--:|---|
| 400 | 400 | `probe_version` or `code` missing |
| 404 | 404 | **Uniform.** Unknown code, unparseable code, suspended, expired, or a `device_id` that does not match the first-redeeming machine — all indistinguishable, deliberately |
| 409 | 409 | Unsupported `probe_version`; the body carries the supported one |
| 429 | 429 | Rate limited |
| 500 | 500 | Unexpected failure — never a partial `200` |

---

## 6. Update Check

### `GET /api/update?version=<running>&platform=<key>`

The endpoint the desktop client's updater polls. **Unauthenticated by design** —
see `../operate/UPDATE-SYSTEM.md` §3 for why, and why that exposes nothing new.

`platform` is one of `linux`, `windows`, `macos_intel`, `macos_arm`. Both
parameters are **required**: without them the hub could not answer for the right
device, and guessing is how a Windows client is handed a Linux binary.

**Response `200` — an update is available:**

```json
{
  "version": "<version>",
  "url": "https://<domain>/updates/<version>/installer-Chrysalis_<version>_x64-setup.exe",
  "signature": "<base64 minisign signature>",
  "sha256": "abc123…"
}
```

**Response `204` — nothing to install.** Returned when nothing is published, the
release is not active, the advertised version is **not strictly newer** than the
one supplied, or any platform is missing a URL, hash or signature.

> `204` is load-bearing: the Tauri updater treats it as "no update" and returns
> cleanly, whereas a `200` whose body it cannot parse is raised as an error it
> logs. Returning `200 {}` would make every up-to-date client log a failure on
> every check.

**Response `400`:** `version` or `platform` missing, or an unknown platform.

**There is no rollout percentage.** An active release is offered to every client.
See `../operate/UPDATE-SYSTEM.md` §2.

---

## 7. Release Manifest (public)

### `GET /api/release`

The credential-free view of the current release: version and each platform's URL,
SHA-256 and signature. It exists so a client that **cannot** reach the heartbeat —
an unactivated device, a suspended/expired code, or a build so broken that
activation fails — can still learn a fix has been published. If updating required
the heartbeat, the failure would prevent escaping the failure.

It carries **no per-device decision** and exposes nothing that is not already
served openly from `/updates/*`.

**Response `200` — a release is published:**

```json
{
  "ok": true,
  "published": true,
  "version": "<version>",
  "platforms": {
    "linux":       { "url": "https://<domain>/updates/<version>/chrysalis-linux-amd64",       "sha256": "abc123…", "signature": "…" },
    "windows":     { "url": "https://<domain>/updates/<version>/installer-Chrysalis_<version>_x64-setup.exe", "sha256": "def456…", "signature": "…" },
    "macos_intel": { "url": "https://<domain>/updates/<version>/chrysalis-darwin-amd64.app.tar.gz",      "sha256": "ghi789…", "signature": "…" },
    "macos_arm":   { "url": "https://<domain>/updates/<version>/chrysalis-darwin-arm64.app.tar.gz",      "sha256": "jkl012…", "signature": "…" }
  }
}
```

**Response `200` — nothing published:** `{"ok": true, "published": false}`.
"No release" is a normal state and must be distinguishable from an error, so it
is a `200` with an explicit marker rather than a `404`. A platform with no URL is
omitted entirely.

> `rollout_percent` is intentionally **not** returned: it is fleet policy, and
> this endpoint is unauthenticated.

---

## 8. Update Manifest (static placeholder)

### `GET /update.json`

Static file served by Caddy. A placeholder — the updater does NOT read it; it reads
`update_config` via `/api/update` and the heartbeat. Informational only.

**Response `200`:**
```json
{
  "version": "<version>",
  "rollout_percent": 0,
  "windows": null,
  "linux_amd64": null
}
```

---

## 9. PocketBase Admin UI

### `GET /_/`

PocketBase admin interface at `https://networkingguides.jadedns.uk/_/`.

---

## 10. Client→Server Protocol Summary

```
Activation:    POST /api/activate            ─── JSON body (PocketBase hook)
Code lookup:   POST /api/code-lookup         ─── JSON body, read-only pre-check (PB hook)
Heartbeat:     POST /api/heartbeat           ─── JSON body, code (PB hook)
Session probe: POST /api/session-probe       ─── JSON body, code + redacted device id (PB hook)
Device release: POST /api/release-entitlement ─── JSON body, code (PB hook)
Release manifest: GET /api/release           ─── Public, credential-free (PB hook)
Update manifest:  GET /api/update            ─── Query: version, platform (PB hook, no auth)
Admin Unbind:  POST /api/admin/unbind-code   ─── JSON body (admin_token)
Admin console: POST /api/admin/console       ─── JSON body (admin_token; Web UI actions)
Release fetch: POST /api/admin/fetch-release ─── JSON body (admin_token; hub pulls from GitHub)
Release link:  GET  /api/admin/fetch-release  ─── One-shot signed trigger link (no token)
Fetch link:    GET  /api/admin/fetch-link     ─── Mint a one-shot link (admin_token)
Health:        GET  /api/health              ─── Plain GET
Update Config: GET  /update.json             ─── Static placeholder (informational only)
Update assets: GET /updates/<version>/<file>─── Static file (no directory listing)
```

### The admin console API

One endpoint, one `action` discriminator — **not** one route per operation:

```
POST /api/admin/console
{ "action": "codes.renew", "admin_token": "…", ...action-specific fields }
```

Auth is `admin_token` in the body **or** an `X-Admin-Token` header. Both are
honoured; the console sends both, so a bodyless caller is supported too.

**The actions, in full, are listed in
[`OPS.md`](../operate/OPS.md#admin-api-actions-reference)** — that is the authoritative
table, kept next to the operator procedures it serves. It is not duplicated
here, because two lists of actions is exactly how one of them goes stale.

Three things about this API that are easy to get wrong:

* **`codes.renew` does not clear a suspension or change the tier.** A renewal is
  a payment event. Those are separate, separately-audited actions, and an
  operator who wants two of them performs two.
* **Renewal extends from the later of now and the current expiry**, so paying
  early never loses days. The response carries `previous_expires_at` and
  `expires_at` so the change can be quoted exactly.
* **`codes.rebind` is retired.** Codes are no longer tied to a device, so there
  is nothing to move between machines: a student who changes machine simply
  enters their code again and it restores their access. The action returns a
  `410` explaining that, rather than a silent no-op. Use `codes.unbind` only to
  release a code for a *different* student.

---

## 11. Error Response Format

All error responses follow this structure:

```json
{
  "code": 400,
  "message": "Human-readable error description"
}
```

HTTP status code matches the `code` field in the JSON body.

---

## 12. Rate Limiting

| Endpoint | Limit | Window | Mechanism |
|----------|:-----:|:------:|-----------|
| `/api/activate` | 5 | 10 minutes | Caddy + JS hook |
| `/api/code-lookup` | 10 | 10 minutes | JS hook (own bucket) |
| `/api/heartbeat` | 1 | 10 seconds | Caddy |
| `/api/release-entitlement` | 10 | 10 minutes | Caddy + forwarder + JS hook (own `release_` bucket) |
| `/api/session-probe` | 10 | 10 minutes | Caddy + forwarder + JS hook (own `probe_` bucket) |
| `/api/*` (general) | 100 | 10 seconds | Caddy default zone |
| `/api/admin/unbind-code` | None | — | Admin token required instead |
| `/api/admin/fetch-release` | None | — | Admin token, **or** a one-shot signed link |
| `/api/admin/fetch-link` | None | — | Admin token; mints a single-use link |
| `/updates/*` | None | — | Large one-shot downloads |

Caddy keys on `{remote_host}` (client IP). The two PocketBase hooks add a
second, fingerprint-keyed layer in the `activation_attempts` table.

**The two hook buckets are deliberately separate**, via a key prefix
(`activate_…` vs `lookup_…`). They previously shared one, and that was actively
harmful: `/api/code-lookup` exists so a student can confirm a code is recognised
*before* committing to an activation, but mistyping on the activation screen
consumed the lookup allowance — locking the student out of the affordance that
would have explained the mistake. Confirmed live 2026-09-19. See FIXES.md 33.

A successful activation clears that device's `activate_…` rows, so a student who
finds their code is not counted against themselves afterwards. Lookups are never
cleared, since they are the enumeration surface.
