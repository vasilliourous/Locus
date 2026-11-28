# ADR 0002 — Liveness is a lease, not a quit message

```
audience:    builder
status:      reference
authoritative-for: why liveness is a lease that expires, not a quit message that must arrive
verified-against: server/pb_hooks/heartbeat.pb.js,
                  server/pb_hooks/session_probe.pb.js,
                  server/console/src/views/codes/model.ts
```

## Context

The requirement was phrased as "every time the app quits, send a message to the
hub". That cannot be the mechanism, and the cases say why:

| Case | Can a message be sent? |
|---|---|
| Graceful quit | yes, best-effort |
| `kill -9`, OOM, panic | **no** |
| Power loss, force-quit | **no** |
| OS sleep/hibernate | **no** — the socket drops silently |
| Network blocked at quit time | **no** |
| Offline for hours or days | **no** |
| Re-login before the old session closed | ordering is undefined |

Five of seven are impossible, and the two that work are best-effort on a network
that may be gone at that moment. A design whose correctness rests on a final
message is wrong for most of its input space.

## Decision

**Liveness is a lease, granted by the client and expired by the hub.**

* The client writes `last_seen_at` on every answered beat (`heartbeat.pb.js`),
  using **the hub's clock**.
* The hub derives `live` / `stale` / `absent` from its age, with `unknown` for a
  null column.
* The quit message is not implemented. Entitlement already survives a quit: a
  code is a scalar stamp, not a session.

**The bounds are derived from the client's own backoff:**

| Bound | Value | Derivation |
|---|---|---|
| lease (live) | 2 h 16 min | `MAX_INTERVAL` 2 h × 1.1 jitter + 15 s timeout + 60 s retry slack |
| stale | 7 days | the client's `GRACE_PERIOD` |

## Why the lease is not shorter

A client that has been failing beats backs off to `MAX_INTERVAL`, so a **healthy**
device can be legitimately silent for two hours. A lease of, say, ten minutes
would declare it absent on every recovery from an outage — turning a working
system into a permanent alarm. The bound is sized by the worst *legitimate*
silence, not by the fastest a demo could look.

## Consequences

* `kill -9` is reflected as `stale` within **≤ 2 h 16 min of last beat**, with no
  message. This is stated as the measured bound, not a tighter aspiration,
  because a tighter bound would require beating every 5 minutes forever against
  a 1-per-10s-per-IP Caddy limit — a polling storm for a diagnostic operators
  need rarely.
* "Absent" therefore means "no beat for 7 days", not "the app closed". The
  runbook says so, and the console tooltip says so, because an operator who
  expects a quit to show immediately will report the lease as a bug.
* `last_seen_at` proves the device **reached the hub**, not that the student is
  at it. The UI says "checked in", never "online".

## Rejected alternatives

| Alternative | Why rejected |
|---|---|
| Reliable quit message | Impossible in five of seven cases; the two that work are best-effort on a possibly-absent network |
| Faster heartbeats | 1-per-10s-per-IP Caddy limit; 288 writes/code/day; the lease is adequate for an operator diagnostic |
| Server-side expiry sweep daemon | Expiry is already enforced per-beat; a daemon is a second implementation of one fact |
| WebSocket / server push | Needs an inbound port on a student machine, and is defeated by the school firewall that the maintenance tunnel exists for |
