# The pre-activation activation tunnel

```
audience:    builder + human-operator
status:      live
authoritative-for: why enrolment is blocked on a censored network, what the activation tunnel is, and what is not yet proven about it
verified-against: server/modules/09-activation-tunnel.sh, server/modules/05-caddy.sh, server/modules/08-firewall.sh, client/src-tauri/src/locus/hub/client.rs
```

> **Status of this work: built and guarded in the tree; the client half does not
> exist yet, and nothing here has been tested from a school network.** Read
> "What is verified" and "What is not" before treating any of it as working.

---

## 1. The failure, as measured

A student on a school network could not activate. The client reported:

```text
Could not reach the Locus hub at https://<hub>/api/activate:
  error sending request for url (https://<hub>/api/activate).
```

That message names no cause, and the reason is a defect recorded below in §5.
The cause was established by running `curl` **on the school network**, not by
reading code. Two traces, both reproduced on 2026-10-29:

### 1.1 Port 443 — reset during the TLS handshake

```text
*   Trying <address>:443...
* ALPN: curl offers h2,http/1.1
* TLSv1.3 (OUT), TLS handshake, Client hello (1):
* Recv failure: Connection reset by peer
* OpenSSL SSL_connect: Connection reset by peer in connection to <hub>:443
curl: (35) Recv failure: Connection reset by peer
```

Read the order: DNS **resolved** (`Trying <address>`), TCP **connected**, the
ClientHello **was sent**, and the reset arrived **after** it. This is not DNS
blocking and not a closed port. It is an inline appliance reading the SNI —
visible in cleartext in the ClientHello — and resetting the flow before any
certificate is presented.

### 1.2 Port 8443 — the block is by name and category

```text
< HTTP/1.1 503 Service Unavailable
<title>Application Control Violation</title>
The website <hub>:8443/ has been deemed unsuitable by your school administrator
Reason: The category of high-risk has been blocked by your school administrator.
```

This is the filtering appliance **answering in place of the server**. It read
`Host: <hub>:8443` from a plaintext HTTP request and denied it by category.

### 1.3 What the two traces together mean

Both blocks are the same decision: **the destination is blocked when its name is
visible.** TLS exposes the name in SNI; plain HTTP exposes it in `Host`.

This is the whole reason a student's tunnel works while enrolment does not. A
Shadowsocks stream is AEAD ciphertext end to end — it presents **no hostname** to
anything on the path. So the block is not a property of a *port*: it is a
property of whether a name is exposed. That distinction matters, because an
earlier reading of this incident concluded the reverse and reasoned from it.

### 1.4 Two inferences that were wrong, and are recorded so they are not repeated

- **"Port 8453 is filtered by the network."** It was not. `curl` to 8453 timed
  out because `08-firewall.sh` never opened it — `ufw default deny incoming`
  with no rule. A timeout with no firewall rule behind it says nothing about the
  network. The port is now opened so the question can be asked meaningfully.
- **"8443 proves an SS port passes."** It proves plaintext HTTP on 8443 is
  categorised and blocked. The evasion is the absence of a readable name, not a
  blessed port number.

---

## 2. Why the client cannot fix this alone

Three mechanisms exist for "the hub's default name is unreachable", and none of
them can help a device that has never activated:

| Mechanism | Why it cannot rescue a new device |
|---|---|
| `contract::resolve_hub_url` — an operator-set override | The value is stored **on the device**. A device that has never run has no override, and nothing can deliver one to it. |
| The `hub_alias` offered over the heartbeat | Requires a successful heartbeat — which requires the hub to already be reachable. |
| `contract::LEGACY_HUB_URL` | Same class: every build before 3.3.2 has it compiled in, and the only channel that could carry a new name needs a working hub. |

The pattern is the same in all three: **the escape hatches are delivered over the
connection they exist to repair.** Activation is what *issues* the tunnel
credentials — `server`, `server_port`, `password`, `method`, `uot_port` all
arrive in the `/api/activate` response — so there is no tunnel to ride before
activating.

The only escape is a transport that exists **before** entitlement.

---

## 3. What the activation tunnel is

A Shadowsocks listener on the hub that carries a **closed set of enrolment
endpoints**, and nothing else.

```text
client ──SS (no readable name)──▶ :8453 ssserver
                                    │  decrypts, forwards to 127.0.0.1:18090
                                    ▼
                          path filter (allowlist + rate limit)
                                    │
                                    ▼
                          127.0.0.1:8090 PocketBase
```

| Piece | Where it lives |
|---|---|
| The listener and forwarder | `server/modules/09-activation-tunnel.sh` |
| The opened port | `server/modules/08-firewall.sh` (`ACTIVATION_TUNNEL_PORT`, default 8453) |
| Module ordering | `server/setup.sh` — after `02` (needs `ssserver`) and `06` (needs `:8090`), before `08` |
| The guards | `server/scripts/check-consistency.sh` §40 |

### 3.1 The allowlist is the security boundary

The forwarder reaches PocketBase **directly, bypassing Caddy** — and Caddy is
where `/api/admin/*`, `/admin/` and `/updates/` are separated, and where the
activation rate limit lives. The bootstrap credential is compiled into the
client, so it must be assumed public. Everything that makes that acceptable is
the allowlist:

```
/api/activate
/api/code-lookup
/api/heartbeat
```

Deliberately excluded: `/api/admin/*` (privileged — the console's channel, never
a student's), `/api/release`, `/updates/*`, `/admin/`, `/_/`.

Refusals return `403` with an **empty body**. A bootstrap endpoint must not
describe what it protects.

### 3.2 The rate limit is reproduced, not assumed

Caddy limits `/api/activate` to 5 events per 10 minutes per source
(`05-caddy.sh`, the `zone activation` block). This path bypasses Caddy, so the
forwarder enforces the same limit itself. `check-consistency.sh` §40 reads
**both** numbers and fails when the tunnel's is laxer — a second, looser limit on
the endpoint that mints entitlements would be the weakest link.

### 3.3 The bootstrap key

The listener's password is a fixed value shipped in the client build. It does
**not** rotate, on purpose: rotating it would strand every deployed client, and
the channel that could deliver a new one is the very call this tunnel exists to
make possible.

Where it lives:

| | |
|---|---|
| Source of truth | `ACTIVATION_TUNNEL_KEY` in `server/secrets.env.age` (see [`../operate/SECRETS-MANAGEMENT.md`](../operate/SECRETS-MANAGEMENT.md)) |
| On the box | `/etc/locus/activation-tunnel.key`, mode `0600`, directory `0700` |
| In the repo | **nowhere** — a tracked credential is a build failure (`check-consistency.sh` §29) |

If the key is absent from the secrets bundle the module **generates one and warns
loudly** that it will match no shipped client. That is deliberate: a deploy must
not stop because a key is missing, but a silent mismatch would waste a whole test
round.

**The bundle does not contain it yet, and that is a step nobody has taken.** The
row above describes where the value *belongs*; as of 2026-10-29
`server/secrets.env.age` predates this feature, so a deploy today takes the
generate-and-warn path and the listener's key matches no build. Closing it is:

1. Generate a 32-byte key (`head -c 32 /dev/urandom | base64`) and put it in the
   plaintext env the bundle is built from — the one `secrets-encrypt.sh` encrypts.
   It is not in this repository and cannot be reconstructed from here.
2. Re-encrypt: `server/scripts/secrets-encrypt.sh` (it needs the passphrase, which
   is likewise not in the repo — see [`../operate/SECRETS-MANAGEMENT.md`](../operate/SECRETS-MANAGEMENT.md)).
3. **Compile the same value into the client** before any build that is expected to
   use the tunnel. There is no client code for it yet (§6), so this lands with that
   work, not before.
4. Re-run `setup.sh` and confirm the module logs *"Using ACTIVATION_TUNNEL_KEY from
   the decrypted secrets"* rather than the warning.

Until 3 exists, steps 1–2 are still worth doing: they make the key stable across
deploys, which is a prerequisite for a client ever matching it.

> **Not written down here, on purpose.** The value is a credential; this document
> says where it lives and what shape it is, which is everything a reader needs.
> Do not paste it into a document, an issue, or a chat — a credential that travels
> through a conversation has to be treated as public afterwards.

---

## 4. What is verified, and what is not

### Verified (by running things, on the tree)

- The allowlist predicate was executed against 13 paths and correctly **refused**
  `/api/admin/fetch-release`, `/api/activate/../../admin/x`,
  `/api/%2e%2e/%2e%2e/admin/fetch-release`, `/updates/...`, `/admin/`, `/_/`, and
  an upper-case variant.
- The rate limiter was executed: attempts 1–5 allowed, the 6th and 7th refused,
  and a different peer address unaffected.
- `check-consistency.sh` §40 passes, and each of its assertions was **observed
  failing** against the defect it catches: an admin path added to the allowlist, a
  rate limit of 50 instead of 5, the tunnel moved onto 8443, and the firewall rule
  removed.
- The hub answers on 443 from an uncensored network with a valid chain; the TLS
  interception above is a property of the school network, not of the hub.

### Not verified — and these are the ones that matter

- **That the tunnel is reachable from the school network.** Nothing has been run
  from a school network since 8453 was opened. The next measurement is
  `curl -v --max-time 10 http://<hub>:8453/`: an immediate
  `Established connection` followed by a stall or garbage is the pass condition
  (SS says nothing sensible to HTTP); a connection *timeout* now genuinely means
  the network drops it, because the firewall rule exists.
- **That the appliance tolerates the port.** A connection proves reachability,
  not that the inspector leaves an opaque stream alone. Observed behaviour of the
  student's own tunnel is the reason to expect it will, and that is an argument,
  not a measurement.
- **That a real activation completes over it.** The client cannot use this
  listener yet — see §6.
- **That any of this reaches a deployed device**, until `setup.sh` re-runs on the
  hub. See [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md).

---

## 5. Three client defects this incident exposed

Recorded here because they turned a locatable fault into a mystery, and each is a
bug in its own right.

1. **The error is truncated before it is shown.** `locus/activation.rs` formats
   `"{error}"`, where `reqwest::Error`'s `Display` is the generic outer message
   ("error sending request for url …"). The cause is in the `source()` chain and
   is discarded. `"{error:#}"` would have named DNS, connect, or TLS.
2. **`locus::hub::diagnose()` has no callers.** The one function written to
   distinguish a root-store failure from a reachability failure is `pub` and
   never invoked, so the diagnostic does not reach a support log.
3. **`Failure::Transport` is a catch-all.** A DNS failure, a refused connection
   and a reset during the handshake classify identically, and `remedy()` returns
   `None` for all of them — the student is told nothing, and the log records
   nothing that separates them.

A `Failure` variant for "the session was reset during the handshake" is also
what the client-side fallback in §6 must key on. It must **not** fire on DNS
failures or certificate errors, which a different transport cannot fix.

---

## 6. What does not exist yet

**There is no client code that uses this tunnel.** As built, the repository has
the server half only.

To finish it, the fallback belongs at the two call sites that already resolve the
hub through `contract::hub_url()` and share one HTTP client:

- `locus/activation.rs::post_json` — `/api/activate`, `/api/code-lookup`
- `locus/heartbeat.rs::beat` — `/api/heartbeat`

Scope is **activation and heartbeat**, matching what the tunnel carries. Tunnelling
the update path is a separate decision with its own signature surface and is not
part of this work.

---

## 7. The durable answer, which is not code

The appliance blocked the hub by **category**, stated in its own response. That is
a policy decision held by a third party, and a hostname visible on any port will
be blocked on that decision. Recategorisation is the only fix that does not
depend on the filter's behaviour, and it cannot be performed from this repository.

Until it is resolved, every device on that network depends on a bootstrap channel
that exists to work around a classification nobody here controls — which is
exactly the kind of thing [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md)
exists to keep visible rather than let a green build imply has been solved.
