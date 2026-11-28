# ADR 0001 — The hub owns entitlement; the client owns liveness

```
audience:    builder
status:      reference
authoritative-for: the two authorities — who decides entitlement, who reports liveness
verified-against: server/pb_hooks/heartbeat.pb.js,
                  server/pb_hooks/session_probe.pb.js,
                  server/console/src/views/codes/model.ts
```

## Context

A client that was logged in was displayed as signed out on the hub. The
investigation found no shared fact that both sides could disagree about: the
hub's model is **entitlement** (may this code be used) and the console derived a
**liveness** answer (is this device in use) from `released_at`, an entitlement
fact. `POST /api/heartbeat` wrote nothing, so the hub could not have answered the
liveness question even if it had asked it.

## Decision

> **The hub is authoritative for entitlement. The client is authoritative for
> liveness. Neither may answer the other's question by inference.**

The console renders the **pair** and never collapses it into one label.

## Why not one authority

**Hub owns both.** It cannot: only the client can observe its own process, and
the hub is off for the periods that matter. A hub-owned liveness would need the
client to report it, which is the design we chose anyway — the difference is
whether we *pretend* the hub observed it. Pretending is how the original bug
happened.

**Client owns both.** It cannot: only the hub can refuse service, and a client
that claims its own entitlement is a client that can edit `verge.yaml` to claim
a tier. The code already states this boundary (`store.rs`: "client-side storage
is not a security boundary").

## Consequences

* Two fields everywhere, never one. `entitlement` × `liveness`, and a caller
  that wants a single badge must choose — which is the point.
* `liveness: unknown` is a first-class value on both sides, so a missing input
  cannot become a definite state. §58 fails if it is rendered as one.
* The console gained a "Last seen" column. If it had instead been folded into
  the State badge, the two facts would collapse again the next time someone
  "simplified" the table.

## Rejected alternatives

| Alternative | Why rejected |
|---|---|
| A `sessions` collection, one row per login | A second source of truth for entitlement, unbounded rows, a migration — to solve `released_at` ambiguity that one indexed column plus `code_events` already distinguishes |
| Derive liveness from tunnel traffic | Traffic is a *different* fact (the app already reports `connected` vs `ready` separately for the same reason); an idle but healthy client would read as dead |
| Ask the client on demand (support code) | Requires the student present, so it cannot diagnose the absent device — the case that matters most |
