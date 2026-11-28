# ADR 0003 — No outbox: the facts here are already idempotent scalars

```
audience:    builder
status:      reference
authoritative-for: why no durable outbox exists, and what would have to change
verified-against: server/pb_hooks/heartbeat.pb.js,
                  server/pb_hooks/session_probe.pb.js,
                  server/console/src/views/codes/model.ts
```

## Context

The question was asked: for a device that signs out while offline, should there
be a durable local outbox with idempotency keys, bounded size, retention, backoff,
batch-on-reconnect and a drop-order when full?

## Decision

**No outbox is built.** The sign-out already survives offline, because the fact
it carries is a **scalar stamp, not an event stream**.

## Why

An outbox exists to make an *event* reliably delivered. The only event that
matters here is "signed out at T", and it is already modelled as `released_at`
— a timestamp the next activation compares against `activated_at`. Replaying a
stamp is a no-op by construction:

* `released_at` is last-writer-wins over one column; a duplicate write converges.
* `release_count` is monotonic; replaying it under `max` semantics converges.
* `activated_at` is likewise a stamp, and `/api/activate` spends a claim by
  re-stamping it — replay is self-cancelling.

So an outbox would **duplicate a mechanism that already exists and is already
idempotent**, and pay for it with: a new file format, a new corruption mode
(disk full, mid-write), a migration, and unbounded growth on a device offline
for weeks.

The existing degradation is already correct and is stated in the code: when the
hub cannot be reached, `signout.rs` reports **"signed out, not confirmed"** rather
than failing the sign-out. That is the honest behaviour an outbox would be built
to avoid needing.

## What is done instead

* Liveness needs no delivery at all — it is a lease that expires (ADR 0002).
* A sign-out that cannot reach the hub is retried by the **student's next
  activation**, which spends the claim. No queue.
* `codes.unbind` is the operator's repair, and it is idempotent (`SET NULL` on
  `activated_at`) and audited (`code_events`).

## Consequences

* If a future requirement introduces a fact that is **not** expressible as a
  scalar stamp — a genuinely ordered event stream where losing the middle
  matters — this decision must be revisited rather than quietly extended. The
  drop-order question ("what is dropped first when the buffer fills") only has an
  answer once such a queue exists, and building it speculatively would be
  answering a question nobody has asked.
* One thing retained from the brief regardless: **every new message is
  idempotent and safe to replay.** The probe is a `SELECT`; the beat's
  `last_seen_at` write is last-writer-wins.

## Rejected alternatives

| Alternative | Failure it prevents | Verdict |
|---|---|---|
| Durable outbox with idempotency keys | A lost sign-out | The loss does not exist — the stamp is already durable locally and the claim is spent by re-activation |
| Client-side retry queue for sign-out | Same | Same; and `ReleaseOutcome::NotConfirmed` already tells the student |
| Queue for liveness pings | Same | Liveness is a lease by design; queuing stale pings would *falsify* it |
