# Locus API Reference

```
audience:    builder
status:      live
authoritative-for: the hub's HTTP API contracts (activation, heartbeat, releases, code lookup)
verified-against: docs/STATE.md
```

> Complete API contracts for the Locus server, as implemented by the
> PocketBase JS hooks and Caddy reverse proxy.

---

## Base URL

`https://networkingguides.duckdns.org`

All endpoints are served through Caddy reverse proxy to PocketBase on `127.0.0.1:8090`.

---

## 1. Activation

### `POST /api/activate`

Activates a device with an activation code and binds it to the hardware fingerprint.

**Rate limited:** 5 attempts per 10 minutes per IP (Caddy + JS hook double-gated).

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
| `fingerprint` | string | ✅ | SHA256 of MAC + disk serial + motherboard UUID |

**Response `200` (success):**
```json
{
  "code": 200,
  "message": "Activation successful",
  "tier": "eco",
  "device_fingerprint": "a1b2c3d4e5f6...abcdef",
  "server_config": {
    "server": "networkingguides.duckdns.org",
    "server_port": 8443,
    "password": "...",
    "method": "2022-blake3-aes-256-gcm"
  },
  "udp_relay": false,
  "expires_at": "2026-05-14T00:00:00.000Z"
}
```

> **`expires_at` is the entitlement's end as an instant**, or `null` when the
> code has no expiry. It is returned on both success paths — a fresh bind and a
> same-device re-activation — so a newly activated client can show the date
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

**Response `200` (re-activation — same device):**
```json
{
  "code": 200,
  "message": "Already activated",
  "tier": "eco",
  "device_fingerprint": "a1b2c3d4e5f6...abcdef",
  "server_config": { "...": "as above" },
  "udp_relay": false,
  "expires_at": "2026-05-14T00:00:00.000Z"
}
```

> Re-activating on the same device is a **success, not an error**, and it returns
> the current tier config so a client can refresh stale connection parameters
> (a rotated password, a moved server). The fingerprint is compared in
> normalised form, so a device re-activating its own code is never mistaken for
> a different one.

**Error responses:**

| Status | Code | Message | Meaning |
|:------:|:----:|---------|---------|
| 400 | 400 | "Missing code" | No code provided |
| 400 | 400 | "Missing device fingerprint" | No fingerprint provided |
| 400 | 400 | "Invalid code format" | Luhn-mod-N validation failed |
| 403 | 403 | "Code bound to another device" | Fingerprint mismatch |
| 403 | 403 | "Code suspended" | Admin suspended the code |
| **409** | **409** | **"This device is already activated on another Locus code."** | **One code per device. See below.** |
| 404 | 404 | "Code not found" | Code doesn't exist in DB |
| 410 | 410 | "Code expired" | Past expires_at date |
| 429 | 429 | "Too many attempts" | Rate limit hit |

> Messages are the exact strings `activation.pb.js` returns; the client matches
> on status code, but a support conversation will quote the text, so it is kept
> verbatim here.

> **The 409 is new, and it is not a 403 for a reason.** 403 already covers BOTH
> "suspended" and "bound to another device", and the client tells them apart by
> looking for the substring `suspended` in the message — a contract pinned by
> `activation_contract_test`. Reusing 403 for a third condition would make a
> deployed client report the wrong account state. The 409's message must
> therefore also **avoid the word "suspended"**, because the oldest builds have
> no 409 arm and render the text verbatim through their `ServerError` path.
>
> **The fingerprint in the request is normalised** to `[a-zA-Z0-9]` server-side
> before it is stored or compared. Clients should send it as-is; the hub does
> the stripping, and doing it in two places is how the two halves drift.

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
| `fingerprint` | string | ❌ | Device fingerprint (binds the check-in to this device) |

**Response `200` (strike tier — carries a UoT endpoint):**
```json
{
  "status": "ok",
  "server_time": "2026-09-19T10:09:42.945Z",
  "tier": "strike",
  "server_config": {
    "server": "networkingguides.duckdns.org",
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
  "tier": "eco",
  "update_available": "2.1.0",
  "update_url": "https://networkingguides.duckdns.org/updates/2.1.0/locus-linux-amd64",
  "update_sha256": "abc123...",
  "update_linux": "https://.../updates/2.1.0/locus-linux-amd64",
  "update_windows": "https://.../updates/2.1.0/locus-windows-amd64.exe",
  "update_macos_intel": "https://.../updates/2.1.0/locus-darwin-amd64",
  "update_macos_arm": "https://.../updates/2.1.0/locus-darwin-arm64",
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
| 403 | 403 | "Account suspended — contact your middleman" | Code is suspended |
| 404 | 404 | "Code not found" | Code doesn't exist or is unparseable |

> **Expiry is enforced on every heartbeat, not just at activation.** A code that
> passes its `expires_at` stops working here even on a device that is already
> bound, which is what makes the 7-day grace period bounded.

---

## 3. Admin Unbind

### `POST /api/admin/unbind-code`

Clears the device fingerprint from a code, allowing re-activation on a new device.
Requires valid admin API token.

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
| `reason` | string | ❌ | Optional reason for audit log |

**Response `200`:**
```json
{
  "code": 200,
  "message": "Code unbound successfully. Can now be activated on a new device.",
  "tier": "eco",
  "middleman": ""
}
```

**Error responses:**

| Status | Code | Message |
|:------:|:----:|---------|
| 400 | 400 | "Missing code" |
| 400 | 400 | "Code is not bound to any device" |
| 403 | 403 | "Invalid admin token" |
| 404 | 404 | "Code not found" |

---

## 4. Health

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

## 5. Update Check

### `GET /api/update?version=<running>&platform=<key>`

The endpoint the desktop client's updater polls. **Unauthenticated by design** —
see `../operate/UPDATE-SYSTEM.md` §3 for why, and why that exposes nothing new.

`platform` is one of `linux`, `windows`, `macos_intel`, `macos_arm`. Both
parameters are **required**: without them the hub could not answer for the right
device, and guessing is how a Windows client is handed a Linux binary.

**Response `200` — an update is available:**

```json
{
  "version": "2.4.0",
  "url": "https://<domain>/updates/2.4.0/locus-windows-amd64.exe",
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

## 6. Release Manifest (public)

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
  "version": "2.4.0",
  "platforms": {
    "linux":       { "url": "https://<domain>/updates/2.4.0/locus-linux-amd64",       "sha256": "abc123…", "signature": "…" },
    "windows":     { "url": "https://<domain>/updates/2.4.0/locus-windows-amd64.exe", "sha256": "def456…", "signature": "…" },
    "macos_intel": { "url": "https://<domain>/updates/2.4.0/locus-darwin-amd64",      "sha256": "ghi789…", "signature": "…" },
    "macos_arm":   { "url": "https://<domain>/updates/2.4.0/locus-darwin-arm64",      "sha256": "jkl012…", "signature": "…" }
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

## 7. Update Manifest (static placeholder)

### `GET /update.json`

Static file served by Caddy. A placeholder — the updater does NOT read it; it reads
`update_config` via `/api/update` and the heartbeat. Informational only.

**Response `200`:**
```json
{
  "version": "1.0.0",
  "rollout_percent": 0,
  "windows": null,
  "linux_amd64": null
}
```

---

## 8. PocketBase Admin UI

### `GET /_/`

PocketBase admin interface at `https://networkingguides.duckdns.org/_/`.

---

## 9. Client→Server Protocol Summary

```
Activation:    POST /api/activate            ─── JSON body (PocketBase hook)
Code lookup:   POST /api/code-lookup         ─── JSON body, read-only pre-check (PB hook)
Heartbeat:     POST /api/heartbeat           ─── JSON body (PB hook)
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

* **`codes.renew` does not clear a suspension, move the device, or change the
  tier.** A renewal is a payment event. Those are separate, separately-audited
  actions, and an operator who wants two of them performs two.
* **Renewal extends from the later of now and the current expiry**, so paying
  early never loses days. The response carries `previous_expires_at` and
  `expires_at` so the change can be quoted exactly.
* **`codes.rebind` is not `codes.unbind`.** Unbind clears `activated_at`, which
  a term is measured from, so it can silently reset paid time. Rebind moves the
  device and leaves the expiry alone. Use rebind to move a student.

---

## 10. Error Response Format

All error responses follow this structure:

```json
{
  "code": 400,
  "message": "Human-readable error description"
}
```

HTTP status code matches the `code` field in the JSON body.

---

## 11. Rate Limiting

| Endpoint | Limit | Window | Mechanism |
|----------|:-----:|:------:|-----------|
| `/api/activate` | 5 | 10 minutes | Caddy + JS hook |
| `/api/code-lookup` | 10 | 10 minutes | JS hook (own bucket) |
| `/api/heartbeat` | 1 | 10 seconds | Caddy |
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
