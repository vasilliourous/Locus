# The Locus update system

```
audience:    builder
status:      live
authoritative-for: how a released build reaches an installed client, end to end
verified-against: server/pb_hooks/release.pb.js, client/src-tauri/src/locus/update/
```

> How a released build reaches an installed client, end to end. Read this with
> `client/docs/SIGNING.md` (key custody and signing) and
> `client/docs/UPDATE-ARCHITECTURE.md` (why the updater is hub-mediated).

---

## 1. The shape of it

```
you publish                                                    client
─────────────                                                  ──────────────
CI builds 4 raw binaries + manifest.json + .sig
        │
        ▼
GitHub Release (tag v<version>)  ──────────┐
                                           │  fetch by version
console: "Fetch & publish" ────────────────┤
  locus-fetch pulls from GitHub,           │
  verifies format + SHA-256 + signature    │
        │                                  │
        ▼                                  │
update_config row                          │
  version, active, download_*,             │
  sha256_*, signature_*                    │
        │                                  │
        ▼                                  │
Caddy serves /updates/<version>/<file> ────┘
        │
        ▼
GET /api/update?version=…&platform=…  ────────► { version, url, signature }
                                                   │
                                                   ▼
                                              download, verify,
                                              installer runs
```

**Two entry points, one source of truth.** The hub decides *what* to offer; the
client decides *whether to accept*; the installer does the installing.

---

## 2. Publishing (the operator's side)

Publishing is **one action**, from the console:

> **Releases → enter the version → Fetch & publish**

That runs `locus-fetch` (`server/scripts/fetch-release.py`), which:

1. Resolves the four platform payloads **and their `.sig` files** from the GitHub
   Release tagged `v<version>`.
2. Downloads each one, enforcing a size floor (a payload under 1 MB is a
   truncated download or a Git LFS pointer).
3. Checks each payload's **format** matches its platform slot — a Windows `.exe`
   in the Linux slot is refused.
4. Checks each `.sig` is a plausible minisign signature (four lines, base64
   payloads).
5. Writes `update_config` with the URL, SHA-256 **and signature** for every
   platform, and sets `active = true`.

A release missing **any** platform, or missing a signature for any platform, is
refused. A partial publish is worse than no publish: it looks complete from the
operator's seat while some clients silently cannot update.

### What each platform's payload actually is

**These are not all "the binary".** Each slot carries whatever that platform's
updater can install, and the two are not interchangeable:

| Slot | Payload | Why that one |
|---|---|---|
| `linux` | `locus-linux-amd64` (ELF) | The plugin replaces the AppImage/installs the package directly. |
| `windows` | `installer-Locus_<v>_x64-setup.exe` (NSIS) | The plugin ShellExecutes **any** PE as an installer, so the raw app would make a client re-execute itself (`FIXES.md`, 2026-09-30). |
| `macos_intel` / `macos_arm` | `locus-darwin-<arch>.app.tar.gz` | The plugin's macOS path is `GzDecoder` + `tar::Archive` over an `.app` bundle — **a bare Mach-O is not installable there**. |

> **The macOS row is a fix, not a detail (2026-10-04).** Both the CI manifest and
> the hub named the bare `locus-darwin-arm64` for the macOS slots, so a macOS
> client downloaded and verified an artifact its installer could never apply.
> macOS auto-update had therefore never worked. Four sites name this payload and
> `check-consistency.sh` **§24** asserts they agree — nothing checked it before,
> which is why the mismatch survived every green pipeline.
>
> **A release that predates that fix is now refused, not degraded (2026-10-05).**
> The fetch route used to fall back to the bare Mach-O when a release carried no
> `.app.tar.gz`, log a WARNING and publish anyway — which is how `v3.2.26` reached
> the live hub pointing `macos_arm` at a payload no macOS client can install. It
> now raises `FetchError`. Degrading to an uninstallable payload is worse than
> refusing: a refusal is visible at publish time, a broken row is not. See
> `../reference/FIXES.md`, 2026-10-05, and `RECOVER-MACOS-UPDATE.md`.

### There is no rollout percentage

Removed deliberately (2026-09). Publishing **is** offering.

The percentage existed to limit how many devices saw a bad build, because the
client has no rollback beyond the installer's own atomicity. Its real-world
effect was mostly misdiagnosis: on a small fleet, a 5% rollout means your own
test device is almost certainly *outside* the bucket, so a working updater looks
broken and gets "fixed" when nothing was wrong.

What replaced it: **`active`**, a single on/off switch. Stopping only stops
*offering* the update — clients that already installed it stay on it, and there
is still no server-driven downgrade. The fix for a bad build is a higher
version, so test on a real machine before turning it on.

---

## 3. The endpoint the client polls

`GET /api/update?version=<running>&platform=<key>`

Served by `server/pb_hooks/update.pb.js`. The client points the Tauri updater at
this URL at runtime, so **no static manifest is ever consulted**.

### Response shapes

| Situation | Status | Body |
|---|---|---|
| An update is available | `200` | `{ "version": "2.4.0", "url": "…", "signature": "…", "sha256": "…" }` |
| Up to date, nothing published, or the release is incomplete | `204` | *(empty)* |
| `version` or `platform` missing/unknown | `400` | `{ ok: false, message }` |
| Internal error | `500` | `{ ok: false, message }` |

**`204` is load-bearing.** The Tauri plugin treats `204 No Content` as "no
update" and returns cleanly, but treats a `200` with an unparseable body as an
**error** it logs. Returning `200 {}` for "you are up to date" would make every
up-to-date client log a failure on every check — noise that hides a real fault.

### Why it is unauthenticated

The plugin's HTTP client sends no activation code, and more importantly this
route must work for exactly the clients that *cannot* authenticate:

- a build broken badly enough that activation fails,
- a device that has never activated,
- a code that is suspended or expired.

In all three the heartbeat is unreachable. If updating required the heartbeat,
those clients could never be given the fix that resolves their problem — the
failure would prevent escaping the failure. (This is the same deadlock
`/api/release` exists for.)

It therefore exposes nothing that is not already public: a version, and a URL
that `/updates/<version>/` serves openly to anyone. No codes, no fingerprints,
no fleet counts.

### Why it refuses to advertise an incomplete release

Every platform needs a URL, a SHA-256 **and** a signature, or the hook returns
`204`. The signature is not optional: the Tauri updater verifies a minisign
signature mandatorily and offers no bypass, so an unsigned artifact cannot be
installed by any client. Advertising it would make every client download tens of
megabytes only to fail verification.

When it refuses, it logs **why** — an operator who published without signing sees
it in the PocketBase journal, rather than discovering "updates silently never
arrive", which is the hardest class of fault to diagnose.

### The `signature` field is base64 of the WHOLE `.sig` file

This is the one field whose *encoding* is a trap, and getting it wrong shipped a
live release that no client could install (3.2.6; see `FIXES.md`).

`tauri-plugin-updater` calls `base64_to_string()` on the `signature` field
**before** parsing the result as minisign text:

```
Signature::decode(base64_to_string(signature))   // what the plugin does
```

So the field must be `base64(<artifact>.sig file text)`, encoded **exactly once**.
The three neighbouring encodings are all wrong:

| `signature` field contains | Result on the client |
|---|---|
| `base64(whole .sig text)` | **verifies** ✅ |
| the `.sig` text verbatim | `base64_to_string` fails — `Invalid byte …, offset N` (the file has spaces/newlines) |
| the signature line only | `base64_to_string` fails (binary, not UTF-8) |
| `base64(signature line only)` | `Signature::decode` fails (loses the trusted comment) |

That offset error is the *symptom*; it names the **signature** field, but an
equally common cause is a bad `plugins.updater.pubkey` (the plugin decodes the
pubkey first, then the signature — see `client/docs/SIGNING.md` §3). Both values
are `base64(<file text>)`.

The encoding is applied once, in `server/scripts/fetch-release.py`, when the `.sig`
is folded into `artifacts[<platform>].signature`. `releases.publish` and
`releases.set` both validate the shape with the same predicate
(`isPlausibleMinisignSignature`), and `check-consistency.sh` §1b fails if any of
this drifts.

### Repairing a row already published with a bad signature

A bad row is data, not code — fixing the script does not repair it. The signatures
are recoverable from the `.sig` files the release already serves:

```sh
# Re-run the publish for the live version; it re-fetches the .sig files and
# PATCHes update_config with the correct base64 encoding.
server/scripts/publish-release.sh <version>
```

Then verify against the live hub (this is the check that would have caught it):

```sh
curl -s "https://<hub>/api/update?version=<older>&platform=linux" \
  | python3 -c 'import sys,json,base64; s=json.load(sys.stdin)["signature"]; \
      t=base64.b64decode(s, validate=True).decode(); \
      assert t.startswith("untrusted comment: "), t; print("signature field OK")'
```

A raw-text field fails that `b64decode(validate=True)` with
`Only base64 data is allowed`; a correct one decodes to the four-line minisign
text.

---

## 4. What the client does

Two independent gates, on purpose.

### Gate 1 — the version (client-side)

`locus/update/version.rs`. The hub is not trusted to advertise only newer
versions: a stale or rolled-back `update_config` row would otherwise downgrade
the fleet, and there is no server-driven downgrade to recover from. So the
client refuses anything not **strictly newer**.

Never a string comparison. As strings, `"1.9.0" > "1.10.0"` — so a client on
1.10.0 would have accepted 1.9.0 as an upgrade. That was a real bug in the
retired client.

### Gate 2 — integrity (both sides)

| Check | Catches |
|---|---|
| **SHA-256** | A corrupted or truncated download |
| **minisign signature** | A *substituted* artifact — which a hash served by the same server could never detect, since whoever controls the response controls the hash |

SHA-256 **fails closed**: a missing checksum is a refusal, never a warning.

### Then the installer

Download, verify, hand to the platform installer.

**The download is the client's own** (`locus/update/apply.rs`): it streams to a
file in the app data root, hashing incrementally, so the artifact is never held
in memory. This matters — the plugin's own `download_and_install` reads the
**whole artifact into a `Vec<u8>`** before verifying it, and that buffering is
enough to abort the allocation on a school laptop, killing the process with no
log line and leaving the progress bar at 0%. That was a live defect (the
"Install Now → 0% → crash" report), and the fix is that the streaming downloader
is now the only path.

**The install is still the plugin's** (`Update::install`): it handles NSIS on
Windows, `.app`/`.dmg` on macOS and the Linux package formats, including `/LANG`
so the NSIS dialog does not appear mid-update, and it re-verifies the minisign
signature over the bytes we hand it. `check-consistency.sh` §12 fails the build
if the install path reverts to the buffering downloader, or if the streaming
downloader loses its only production caller — the state that produced that bug.

**Not ported from the retired client:** the hand-rolled backup / swap / fork /
sentinel-revert machinery. That existed because a portable binary had to replace
itself; an installed application does not, and it destroyed an installation once
(FIXES #22) and staged into the current working directory (FIXES #54).

---

## 5. The two user-facing surfaces

| Surface | When | What it says |
|---|---|---|
| **Prompt dialog** | On launch, when the hub has offered a newer version and it was recorded | "A new version is ready — Install now / Later" |
| **Account → "Check for updates automatically"** | Always | Whether automatic checking is on. Default **On**; off suppresses the prompt (the offer is not recorded), it does not disable manual installs |

> The first row is the only prompt. There is no separate "Settings → Check for
> updates" screen in the shipping client: the check is hub-mediated and surfaces
> through the heartbeat, so the Account toggle is the whole preference surface.
> (An earlier revision of this document described a Settings screen that the code
> never had.)

Both must be **unable to dead-end**. If the hub is unreachable, the prompt says
so and says what to do, rather than spinning. And "you are up to date" is
distinguished from "the hub could not be asked" — conflating them is how a
broken updater goes unnoticed.

Every decision branch is logged (signal not newer, no artifact for this
platform, hash missing, hub unreachable). The retired client's worst failure was
a silent no-op: an update advertised and ignored, with nothing anywhere saying
so. Observability is the client-side defence.

---

## 6. Verified vs unverified

Be precise about what has been proven.

| | Status |
|---|---|
| Signing keypair, and the plugin's own verifier accepting its signatures | **Verified** — including the negative cases (tampered bytes, wrong key) |
| Client version comparison, 14 cases incl. `1.9.0` vs `1.10.0` | **Verified** (Rust tests) |
| Hub version gate agrees with the client's | **Verified** — `server/scripts/smoke-update-endpoint.sh` extracts the logic from the shipped hook |
| Signature file validation (8 cases incl. a real signature) | **Verified** |
| Streaming download, SHA-256 fail-closed, and the hub-checksum read | **Verified** (Rust tests) |
| The crash class ("Install Now" → 0% → process dies) cannot recur unnoticed | **Guarded** — `check-consistency.sh` §12 fails if the install path returns to the memory-buffering downloader |
| `/api/update` response shapes | **Not yet run against a live host** |
| A real end-to-end update on real hardware | **Not yet done** — this is the acceptance gate |

The last two need the VPS, which is where the remaining risk lives.

---

## 7. Related

- `client/docs/SIGNING.md` — key custody, rotation, the signing pipeline
- `client/docs/UPDATE-ARCHITECTURE.md` — why the updater is hub-mediated at all
- `../operate/OPS.md` → "Client Update System" — the operator runbook
- `../reference/FIXES.md` — the defects this design avoids
