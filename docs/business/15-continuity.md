# 15. Continuity — what survives what

```
audience:    human-operator
status:      live
authoritative-for: what survives a change to each part of the system
verified-against: docs/STATE.md
```

| Event | What actually happens | Needs |
|---|---|---|
| **Tier config changes** (port, cap, UoT) | Clients pick it up on next heartbeat (~5 min). No app update. | edit `tier_configs` in the console ([`09-console.md`](09-console.md)) |
| **Free-tier cap / allowance changes** | **Only if client-side** — the client must be told the new values. Affects every free client. | a config value the client reads; see §15.2 |
| **New code batches** | Minted in the console instantly. | operator |
| **Spare VPS failover** | Update DNS + `tier_configs`. | spare VPS kept warm ([`14-risks.md`](14-risks.md#144-school-it-blocks-the-vps-ip)) |
| **New protocol needed** | **A client update.** No in-product swap exists. | a release, and the updater has never installed one |
| **Operator stops** | VPS bill ends; codes stop working. Low exit cost ([`12-scale-and-ceiling.md`](12-scale-and-ceiling.md#124-the-honest-ambition)). | nothing |

## 15.1 The gap, stated plainly

The legacy "no app update needed, clients swap protocols on heartbeat"
continuity plan is **aspirational, not built**. State it as a proposal
([`18-open-items.md`](18-open-items.md)), never as a current capability.

What *is* built is narrower and worth being precise about:

- Clients refresh **connection config** on heartbeat — so a server-side change
  to ports, caps, or UoT propagates within ~5 minutes.
- Clients do **not** acquire a new *protocol* on heartbeat. A different
  protocol is a different client.

## 15.2 The free tier's continuity question (new)

Because the free quota is enforced **client-side**
([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)), changing the free cap or allowance is
**not** a pure server-side operation:

- The cap itself is a `tc` value on the hub — that part propagates as a
  normal config change (but note it is fixed in `04-tc.sh`, so it takes a
  `setup.sh` re-run rather than a console edit).
- The **5 GB allowance** is a number the client holds. Changing it means
  telling existing clients the new number through the same config channel.
- A client that is offline during the change keeps the old number until it
  heartbeats.

**Resolved: the allowance is a server-provided value**, sent on every heartbeat
as `free_allowance_mb` and read by the client. That is what keeps the free tier
adjustable without shipping a release, and it means there is one place to change
the number. **Sending nothing is itself the off switch** — the client reads an
absent allowance as "no allowance applies", never as zero, so the operator can
end the quota without throttling anyone by accident.

**Still true:** a client that is offline during a change keeps the old value
until its next successful beat. The grace period makes that at most a week, and
in the meantime the old number is what that student was promised.

## 15.3 Related reading

- The risks this addresses → [`14-risks.md`](14-risks.md)
- The project that would close the gap → [`18-open-items.md`](18-open-items.md)
- The console actions involved → [`09-console.md`](09-console.md)
