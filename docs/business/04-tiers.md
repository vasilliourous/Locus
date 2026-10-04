# 4. Tiers

```
audience:    human-operator
status:      live
authoritative-for: the tier table, what each tier is in the code, and the free tier's design
verified-against: server/modules/04-tc.sh, server/modules/02-shadowsocks.sh, docs/state.toml
```

The product now has **three tiers**: a permanent **free** tier, and two paid
tiers. This file describes what each one is in the code, what changed from
the legacy model, and the free tier's design in full.

---

## 4.1 The current, code-verified tier table

| Tier | Port | Hard cap (tc) | Kernel | UDP (UoT) | Data | What the student gets |
|---|:--:|:--:|---|---|---|---|
| **Free** | 8443 | 1 Mbps | BBR | ✗ | 5 GB/mo, then throttled | Chat, IM, email, light browsing. |
| **Stealth** | 8444 | 100 Mbps | BBR | ✗ | Unmetered | Fast streaming and downloads. |
| **Strike** | 8445 | 200 Mbps | BBR | ✓ (8446) | Unmetered | Streaming **and** low-latency UDP for gaming. |

Source of truth for the caps: `server/modules/04-tc.sh` (the `apply_tc_now` and
`create_tc_service` calls); for the UoT listener,
`server/modules/02-shadowsocks.sh`. The free tier's 1 Mbps cap **is in the tree**
(`04-tc.sh`, port 8443) and the allowance is sent by the hub on every heartbeat
as `free_allowance_mb` (`server/pb_hooks/heartbeat.pb.js`); both are held in
agreement by `check-consistency.sh` §23.

> **`eco` and `free` are the same endpoint.** The hub seeds **both** names against
> port 8443 with the Eco password — `free` for codes minted now, `eco` so codes
> already in the field keep resolving. The customer-facing name is **Free**; the
> on-box names (`shadowsocks-eco.service`, `tc-eco-cap.service`, `/etc/shadowsocks/eco.json`)
> stay `eco` on purpose. See §4.5.

---

## 4.2 What changed from every legacy document

### 4.2.1 The Brutal-CC differentiation is gone

Both legacy business documents sold Stealth on a "Brutal" congestion-control
kernel module (`tcp-brutal` + `LD_PRELOAD`). **That module was removed on
2026-08-01** (`docs/reference/FIXES.md`). All tiers now run plain BBR.

Consequences:

- The old "Stealth's jitter makes gaming unplayable → upgrade to Strike"
  argument no longer has a mechanism behind it. There is no jitter generator.
- Stealth's real cap is **100 Mbps**, not the legacy "48 Mbps Brutal target".
- **Tier differentiation is now: the tc cap, the data allowance, and whether
  UoT is on.** That is the entire ladder.

### 4.2.2 Eco is replaced by Free

The legacy ladder had a $2 "Eco" tier at 5 Mbps. That tier is **removed** and
replaced by a free tier at 1 Mbps with a 5 GB monthly allowance
(decision, this session).

Why this is better than keeping a cheap paid anchor:

- **Free is a funnel, not a price point.** A student who would never pay $2
  will still install a free app, which makes word-of-mouth work through a
  much larger population ([`11-growth.md`](11-growth.md)).
- **Free makes the paid upsell easier, not harder.** The jump from
  *1 Mbps / 5 GB* to *100 Mbps / unmetered* is enormous, and it costs the
  student $4 — a decision that does not need to be deliberated.
- **It removes the "is $2 worth it?" question entirely.** There is no longer
  a tier a student has to *justify*; the only question is whether they want
  fast.

### 4.2.3 The paid gap is small on purpose

With Free at the bottom, Stealth ($4) and Strike ($7) sit close together.
This reverses the earlier "widen the gaps" steer, and the reason is in
[`05-pricing.md`](05-pricing.md#53-the-decisive-principles): **the gap that matters is free→paid, not
paid→paid.** Once a student is paying anything at all, moving to Strike is a
trivial step.

---

## 4.3 What still makes the tiers work together

- **Free exists to place the app on the device.** It is deliberately usable
  but constrained — enough to prove it works, not enough to live on.
- **Stealth is the volume seller.** 100 Mbps unmetered for $4 is the tier
  the business is built on.
- **Strike is the real product.** UDP + the 200 Mbps cap is the only tier
  that plays games on school WiFi. A 200 Mbps cap is far above any game's
  needs (Valorant ~5 Mbps, Fortnite ~20 Mbps) but stops one user saturating
  the link.

---

## 4.4 The free tier, in full

### 4.4.1 Parameters

| Parameter | Value |
|---|---|
| Price | **$0** |
| Speed cap | **1 Mbps** (tc) |
| Monthly allowance | **5 GB** |
| After the allowance | **Throttled further** (not cut off) |
| Enforcement | **Client-side counting** |
| Port | 8443 (reuses the old Eco slot) |

### 4.4.2 Why a data allowance, not a user cap

A user cap protects *instantaneous* load (concurrent users on the pipe). A
data allowance protects *aggregate* cost (bytes off the droplet) and
self-limits naturally: a free user who streams video burns their allowance
and stops; a free user who only chats uses almost nothing for a month and
costs the business almost nothing.

This is the better fit for a 5 GB-at-1-Mbps tier, and it means the free tier
does not need an operator watching a user count
([`09-console.md`](09-console.md)).

### 4.4.3 Why enforcement is client-side (and what that means)

The hub has **no per-user traffic accounting**. Each tier is one shadowsocks
instance with a **single shared password**, so every free user is
indistinguishable on the wire. There is nowhere to count bytes per user.

So the quota is counted **by the client** (`client/src-tauri/src/locus/usage.rs`),
which already heartbeats and knows its tier. The honest consequences:

- **It is soft.** A modified client could ignore the cap. The client is
  closed-source, so tampering is non-trivial, but it is not impossible.
- **It is good enough.** The threat model is a student who wants free fast
  internet, not a determined adversary — and the alternative (per-user
  SS2022 credentials + a server-side usage table) is real work for little
  return at this scale.

**The allowance itself comes from the server**, on every heartbeat, as
`free_allowance_mb` (and `free_throttle_mbps`). That is deliberate: it lets the
operator change the allowance — or switch it off by sending nothing, which the
client reads as "no allowance" rather than as zero — **without shipping a client
release**. The *counting* is local; the *number* is not.

This is recorded as a decision, not a gap: see
[`18-open-items.md`](18-open-items.md) for the "do it properly" option if the
softness ever becomes a problem.

### 4.4.4 What the free user sees

The client must make the constraint visible at all times (decision, this
session):

| Signal | When | Built? |
|---|---|---|
| **Usage bar** | Always, on the connection screen, once an allowance applies. | ✅ `components/connection/usage-bar.tsx` |
| **Warning** | At 80% of the allowance. | ✅ the warning colour plus a one-line notice |
| **Persistent banner** | Once throttled, for the rest of the month. | ✅ the bar's summary line states it, in the error colour |
| **Upgrade CTA** | Beside the throttle state — one tap to Stealth. | ⚠️ **NOT BUILT** — the copy names the throttle but there is no purchase link yet (see [`18-open-items.md`](18-open-items.md)) |

The point is that a throttled student **knows why**, and knows exactly what
fixes it. A silently slow app reads as a broken product; a visibly throttled
one reads as a working free tier with a clear upgrade.

> The component renders **nothing at all** for a paying tier — `unlimited` is its
> own case, not "metered with zero", so a Strike student is never shown a data
> bar. The frontend does no arithmetic of its own: it renders the backend's
> classification, so the warning line and the throttle line cannot drift between
> Rust and TypeScript.

### 4.4.5 What "throttled further" means

Not cut off — slowed. The free tier keeps working after 5 GB, at a much
lower speed, until the month resets. This is deliberately kinder than a hard
stop: it keeps the app *useful* (so the student does not uninstall it) while
making the paid tier obviously worth it.

**How it works now.** The `tc` class on port 8443 is the hard ceiling at 1 Mbps
either way; the "further" throttle is the speed the client drops to once the
allowance is spent, sent by the hub as `free_throttle_mbps` and applied by the
client. The 30-day window, the 80% warning line and the classification all live
in one pure module (`client/src-tauri/src/locus/usage.rs`) so the warning and the
throttle cannot disagree about where the line is.

**The window is 30 days, not a calendar month.** The client has no trustworthy
timezone and the hub stores no billing date per student, so a fixed 30-day window
is the one rule that cannot disagree with itself across a daylight-saving
boundary. A student who returns after three months gets a **fresh** allowance,
not three months of accumulated debt — there is nothing to spend unused allowance
on, and punishing the casual user is the opposite of the point.

### 4.4.6 The bandwidth cost of free users

Every free user consumes real bandwidth on the same 1 vCPU / 454 MB droplet
and the same N4L link as paying users. At 1 Mbps and 5 GB, an individual
free user is cheap; the risk is *volume*.

- **5 GB × 1,000 free users = 5 TB/month**, which is not cheap and not small.
- The cheap tiers protect themselves: a free user cannot exceed 1 Mbps, so
  they can never crowd out a Strike gamer on bandwidth *rate*.
- The remaining exposure is **the droplet's monthly transfer allowance**,
  which is the thing to monitor if free adoption is large. See
  [`14-risks.md`](14-risks.md).

> **This is the free tier's real risk, and it is an infrastructure cost, not
> a product one.** If free adoption is large, the fix is a tighter cap or a
> tighter allowance, not removing the tier.

---

## 4.5 The deploy-time choice: renaming Eco → Free

Eco is a live shadowsocks service on port 8443 with its own password and
systemd unit (`tc-eco-cap.service`, `shadowsocks-eco.service`,
`/etc/shadowsocks/eco.json`).

**Decision: reuse Eco's slot, rename nothing on the box.** Keep port 8443, its
instance and its unit names; stop selling Eco codes; mint free codes against a
**`free`** `tier_configs` row that points at that same endpoint, with the cap
dropped to 1 Mbps.

**What that means in the tree**, and it is three files rather than one:

| File | What changed | Why |
|---|---|---|
| `server/modules/04-tc.sh` | the 8443 cap is `1mbit` (was `5mbit`), in **both** the applied class and the `tc-eco-cap.service` oneshot | a reboot rebuilds the class from the unit; the two must agree or a reboot silently restores the old cap |
| `server/scripts/seed-pb.py` | seeds **both** `free` and `eco` rows against 8443 | `free` is what new codes carry; `eco` must survive so field codes still resolve |
| `server/pb_hooks/heartbeat.pb.js` | sends `free_allowance_mb` for the free/eco tiers | the allowance is server-provided so it changes without a release |

`check-consistency.sh` **§23** asserts the agreement across all of them, and
across the client's reader for the key. It was observed failing against each
half-deployed state before it was kept.

Rationale: a true rename means new service names and a new port, i.e. editing
`04-tc.sh` and `02-shadowsocks.sh` and **redeploying the hub**. The rename buys
nothing a config change does not, and it is exactly the kind of cross-file
inconsistency that has caused real defects here before (see `docs/CONTEXT.md` on
cross-file agreement).

The legacy `eco` name therefore survives in the tree while the customer-facing
tier is called **Free**. That asymmetry is intentional and should be documented
wherever it could confuse — see [`03-product.md`](03-product.md) on the code
being the truth.

> **One student-visible consequence.** Existing `eco`-tier codes keep the tier
> name they were minted with, so they get the 1 Mbps cap too. This is the
> intended downgrade: the free tier *replaced* Eco as a product, and Eco was never
> a paid tier's promise. A student on an old Eco code who expected 5 Mbps does not
> have a bug — they have the free tier.

---

## 4.6 Related reading

- Why the ladder is shaped this way → [`05-pricing.md`](05-pricing.md)
- What free costs to run → [`06-unit-economics.md`](06-unit-economics.md)
- How free feeds growth → [`11-growth.md`](11-growth.md)
- The tier-separation risk → [`14-risks.md`](14-risks.md)
