# 15. Continuity — what survives what

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
  normal config change.
- The **5 GB allowance** is a number the client holds. Changing it means
  telling existing clients the new number through the same config channel.
- A client that is offline during the change keeps the old number until it
  heartbeats.

**Implication:** the allowance should be a *server-provided value* read on
heartbeat, not a constant compiled into the client. This is a design
constraint on the free tier's implementation, and it is the cheapest way to
keep the free tier adjustable without shipping a release.

## 15.3 Related reading

- The risks this addresses → [`14-risks.md`](14-risks.md)
- The project that would close the gap → [`18-open-items.md`](18-open-items.md)
- The console actions involved → [`09-console.md`](09-console.md)
