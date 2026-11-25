# The maintenance tunnel

```
audience:    builder + human-operator
status:      live
authoritative-for: the free-tier tunnel used for hub calls and plan establishment on a network that
                   blocks the hub by name, and what is not yet proven about it
verified-against: client/src-tauri/src/chrysalis/activation_tunnel.rs, client/src-tauri/src/chrysalis/tier.rs,
                  client/src-tauri/src/chrysalis/hub/client.rs, client/src-tauri/build.rs
```

## 1. What it is

A shadowsocks connection to the hub's **free tier** (`:8443`, `ECO_PASS`), dialled
directly to the hub's address with credentials compiled into the binary. It needs
no discovery handshake and no hub round-trip to form, which is the whole point: it
is the one route available to a device that cannot reach the hub's name.

Split scope: it carries **this process's hub-bound traffic only** — activation,
heartbeat, the update download, and the plan tunnel's own outbound. It never
carries a student's traffic; user traffic goes through the plan tunnel.

## 2. The failure it addresses

A Palo Alto appliance inspects HTTPS and kills the connection to the hub's domain,
so anything that names it is blocked. Ports are not blocked, and a tunnel formed
before joining the network keeps working — which is the proof that the transport
is fine and only *discovery* is broken. The maintenance tunnel bypasses discovery
rather than fixing it.

> ### ✅ "Ports are not blocked" is CORRECT — measured on that network, 2026-11-19.
>
> An earlier revision of this section asserted the opposite. It was wrong, and the
> way it was wrong is worth keeping: it tested **plaintext `GET`s** and inferred
> that the destination was unreachable.
>
> **What the appliance actually does.** A plaintext `GET` to `:8443` or `:8445`
> returns `HTTP/1.1 503` carrying the filter's own *"category of high-risk"* page,
> because plaintext is something it can read. A `GET` to a port nothing serves
> returns no HTTP status at all. So it answers in place **on the ports it has
> classified** — and classification is of the *content it can read*, not of the
> port number.
>
> **What the tunnel does.** A Shadowsocks stream presents no name and no
> plaintext, so there is nothing to classify. Driving this tunnel's exact
> generated config (`ss` / `2022-blake3-aes-256-gcm` / `ECO_PASS` / the hub
> address / `:8443`) through the vendored engine **from that network**:
> `/api/health` returned the path filter's `403` (decrypted, path refused —
> exactly right) and `POST /api/activate` returned **`400`**, PocketBase's own
> invalid-code answer. The plan port behaved the same way: `:8445` with
> `STRIKE_PASS` carried `https://example.com` to **`http=200`**.
>
> **So this tunnel works on the network it was built for.** A24 in
> [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5 is the measurement. What is
> still unknown is why the *app* does not form the same connection when an
> equivalent hand-built raw-address config does — see §8.

## 3. The switch rule

```rust
if error.status().is_none() { retry_through_maintenance() }  // nothing answered
else { /* the hub answered; a 4xx/5xx is not a blocked network */ }
```

**The classifier is not the trigger.** `hub/client.rs:392`
(`retryable_over_tunnel`) matches `ResetDuringHandshake | Refused | Dns`. TLS
inspection truncates the stream, which arrives as a decode error and classifies as
`Transport` — so that gate never fires on the network this exists for. The rule is
"no HTTP status at all", recorded once per session and never re-tested.

## 4. Lifecycle

**Owned by a state machine**, not by a two-branch `if`. See
[`TUNNEL-SUPERVISOR.md`](TUNNEL-SUPERVISOR.md) for the states, the constants and
the reasoning; the short version is that the engine is stopped only after the plan
tunnel has carried traffic for a **sustained** `TUNNEL_HOLD`, not on the first
sample that says so.

Warm at app setup (`lib.rs`, `setup`); driven from `chrysalis_status` via
`activation_tunnel::supervise`, which is the only hook that observes both
transitions. The 60 s idle TTL in `endpoint()` governs the per-request activation
engine, **not** this one — they occupy separate slots, so the maintenance engine is
never idle-killed.

**Changed 2026-11-19.** This section used to say the engine "is stopped when the
plan tunnel is carrying traffic", which was true but unstable: it acted on one
sample. The measured consequence was a start/stop cycle across every TUN-up on the
network this exists for.

Nothing auto-reconnects (`use-connection.ts:315` is the only `chrysalisConnect`
caller), so a dropped tunnel is re-warmed within one poll of the drop — up to 15 s
on the idle interval. Do not expect the drop itself to wake maintenance.

## 5. Why the plan dials an address, not a chain

The plan's `server` comes from the hub as a **hostname**. Once the session has seen
a transport failure, `tier.rs::dial_server` substitutes the hub's compiled address
for the name; both the plan proxy and the UoT proxy (`UOT_PROXY_NAME`,
`Chrysalis-UoT`) take it, since they share one builder.

Chaining via `dialer-proxy` was rejected for two independent reasons: it severs
the plan's TCP when the maintenance engine stops at TUN-up (a tunnel that dies the
moment it succeeds), and `config/runtime.rs:65-68` strips every `dialer-proxy`
when the profile is enhanced, so the emitted chain never survives to the engine
anyway. Substitution is one field and has neither problem.

## 6. Operating it

**The key.** `CHRYSALIS_FREE_TIER_KEY` is the hub's `ECO_PASS` from
`server/secrets.env.age`, injected at build time (`build.rs`) and compiled in.

```bash
gh secret set CHRYSALIS_FREE_TIER_KEY --repo <owner>/<repo>
```

A tagged build **fails** without it (`.github/workflows/client.yml`); a branch
build simply has no maintenance tunnel.

**School-network test, three scenarios:**

1. **Fresh start** — cold launch on the school network, enter a code. Expected:
   enrolment succeeds, and the activation screen shows *"Using the Chrysalis backup
   route to reach the server."*
2. **Tunnel drop** — press Disconnect, then Connect. Expected: the plan tunnel
   re-establishes without leaving the network.
3. **Home regression** — on an unfiltered network, connect normally. Expected:
   unchanged behaviour, no maintenance engine started.

**Expected log lines:**

```
[maintenance] engine started proxy=socks5h://127.0.0.1:<port>
[maintenance] no key, direct only        (a build without the free-tier key)
[hub] direct failed (no status), switching to maintenance
[hub] retry via maintenance ok           (or: failed err=None, meaning nothing answered)
[plan] server=<ip> (prefer_maintenance)  (or: <domain> (default))
```

`err=None` means the retry also produced no HTTP status; a status there means the
hub answered and the route works but the call did not.

## 7. Known limitations

- **The app does not form this connection on that network, although the
  tunnel itself demonstrably works there (§2, §8).** That gap is a client defect
  and is the open item.
- The engine's successful start is not covered by a test: it needs a build-time
  key a test binary does not carry.
- The hub's `:8443` **has now been dialled from that network and WORKED**
  (2026-11-19) when driven directly. This line used to say "never dialled".
- The poll intervals live in React and cannot be asserted from Rust.
- The switch wiring is reviewed, not exercised end to end; the rule it calls is
  tested against a real HTTP server.

## 8. The open defect: the app does not form what the config does

**Measured 2026-11-19, from the school network, in one session.** The tunnel's
own generated config, run by hand through the vendored engine, carries
`/api/activate` to the hub and `https://example.com` end to end. A raw-address
Shadowsocks profile in Clash Verge Rev does the same. **The app does not.**

So every premise this document rests on holds on that network, and the failure
is in the app's own path. The candidates, none yet eliminated:

- **The engine is never warm when it is needed.** `warm_maintenance` runs from
  `lib.rs` setup and from `chrysalis_status`; enrolment can be submitted before
  either has finished loading a 60 MB sidecar.
- **The switch never fires.** The rule is "no HTTP status at all"; the direct
  call must actually produce that, and `retry_over_maintenance` must be reached
  before the response is parsed.
- **The build carries no free-tier key.** `CHRYSALIS_FREE_TIER_KEY` is
  `option_env!`; `Bootstrap::maintenance()` returns `None` silently and the
  routing rule takes the direct branch forever. The Actions secret exists
  (2026-11-13) but the *installed* binary is only as good as the build that
  produced it.

**The next measurement is one support log**, and it is decisive: the startup line
for the maintenance engine, then `[hub] direct failed (no status), switching to
maintenance`. Absent the first, the build or the warm path is at fault; absent
the second, the switch rule is. Both are logged already — nothing new needs
building to find out which.

## 9. Failsafes, added 2026-11-19

Four defects of one family were found by measuring on the school network: **every
one of them made the route silently unavailable in the situation it exists for.**
Each is fixed with a guard that was shown failing against the old code.

### 9.1 The retry was refused before anything could start the engine

`should_retry_over_maintenance` was
`error.status().is_none() && maintenance_proxy().is_some()`. The second clause
asked whether an engine was warm *at that instant* — and on a cold launch it is
not, because the background warm is still racing a 60 MB sidecar load. So the one
transport a censored network allows was skipped, the request went direct, and it
failed with exactly the failure that triggered it.

The decision now depends on the **build** (`maintenance_tunnel_available()`), not
on the clock. Guard:
`a_retry_is_decided_by_the_build_not_by_whether_an_engine_is_warm`.

### 9.2 Nothing started the engine when it was needed

`hub_client_for` only ever read a warm engine. `hub_client_for_retry` starts one
on demand (`maintenance_proxy_or_start`), and it is used **only** by the two retry
call sites — a direct call must never pay a 60 MB load on a network that does not
need it. When it cannot start, it says which of the three reasons applies: no key
in this build, the start failed, or one is now warm.

### 9.3 One failure disabled the tunnel for the whole session

A single boolean meant the first failure blocked every later attempt. On this
network, a mistyped activation code fails the advisory `/api/code-lookup`, sets
the flag, and then **the corrected code is refused before it is attempted** — the
student sees the same error twice and must restart the app.

Now: up to `TUNNEL_MAX_ATTEMPTS` (3) failures are allowed, separated by
`TUNNEL_RETRY_BACKOFF`, and a success clears the count — so the cap means "three
in a row", which is what a broken network looks like, and cannot accumulate across
a working session. Guard:
`a_first_failure_does_not_disable_the_tunnel_for_the_session`.

### 9.4 The startup line reported only the activation key

`build_stamp()` named the activation tunnel alone, so a build could ship with the
maintenance tunnel dead and the log would still say "available" — true about the
wrong tunnel. It now reports both. Guard:
`the_build_stamp_reports_the_maintenance_tunnel_too`.

`CHRYSALIS_FREE_TIER_KEY` also gained a `_PRESENT` build stamp
(`build.rs`), the same shape the activation key has had since three releases went
out keyless.

### 9.5 The screen said nothing during the slow part

`ChrysalisStatus.bootstrapping` required a warm engine, so during the on-demand start
— precisely the window where the student is waiting — the caption *"Using the
Chrysalis backup route to reach the server"* was absent. It now reports the session's
preference plus whether the build carries a key.

### 9.6 What is still not guaranteed

These remove the ways the route could **decline to try**. They do not make a
blocked network work: if the appliance ever starts classifying an opaque stream
on `:8443`, no client-side change helps. That was tested on 2026-11-19 and it
does not, today — A24 in [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5.
