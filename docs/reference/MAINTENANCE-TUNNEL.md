# The maintenance tunnel

```
audience:    builder + human-operator
status:      live
authoritative-for: the free-tier tunnel used for hub calls and plan establishment on a network that
                   blocks the hub by name, and what is not yet proven about it
verified-against: client/src-tauri/src/locus/activation_tunnel.rs, client/src-tauri/src/locus/tier.rs,
                  client/src-tauri/src/locus/hub/client.rs, client/src-tauri/build.rs
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

Warm at app setup (`lib.rs`, `setup`); started and stopped from `locus_status`,
which is the only hook that observes both transitions. The 60 s idle TTL in
`endpoint()` governs the per-request activation engine, **not** this one — they
occupy separate slots, so the maintenance engine is never idle-killed.

Nothing auto-reconnects (`use-connection.ts:315` is the only `locusConnect`
caller), so a dropped tunnel is re-warmed on the next poll — up to 15 s on the
idle interval. Do not expect the drop itself to wake maintenance.

## 5. Why the plan dials an address, not a chain

The plan's `server` comes from the hub as a **hostname**. Once the session has seen
a transport failure, `tier.rs::dial_server` substitutes the hub's compiled address
for the name; both the plan proxy and `LocusUoT` take it, since they share one
builder.

Chaining via `dialer-proxy` was rejected for two independent reasons: it severs
the plan's TCP when the maintenance engine stops at TUN-up (a tunnel that dies the
moment it succeeds), and `config/runtime.rs:65-68` strips every `dialer-proxy`
when the profile is enhanced, so the emitted chain never survives to the engine
anyway. Substitution is one field and has neither problem.

## 6. Operating it

**The key.** `LOCUS_FREE_TIER_KEY` is the hub's `ECO_PASS` from
`server/secrets.env.age`, injected at build time (`build.rs`) and compiled in.

```bash
gh secret set LOCUS_FREE_TIER_KEY --repo <owner>/<repo>
```

A tagged build **fails** without it (`.github/workflows/client.yml`); a branch
build simply has no maintenance tunnel.

**School-network test, three scenarios:**

1. **Fresh start** — cold launch on the school network, enter a code. Expected:
   enrolment succeeds, and the activation screen shows *"Using the Locus backup
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

- The engine's successful start is not covered by a test: it needs a build-time
  key a test binary does not carry.
- The hub's `:8443` has never been dialled from a real school network.
- The poll intervals live in React and cannot be asserted from Rust.
- The switch wiring is reviewed, not exercised end to end; the rule it calls is
  tested against a real HTTP server.
- `LOCUS_FREE_TIER_KEY` is not yet set as an Actions secret.
