# 4. Tiers

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

Source of truth for the caps: `server/modules/04-tc.sh` (the
`create_tc_service` calls); for the UoT listener, `server/modules/02-shadowsocks.sh`.
The 1 Mbps free cap and the 5 GB allowance are **proposed changes**, not yet
in the tree — see §4.4.

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

So the quota is counted **by the client**, which already heartbeats and knows
its tier. The honest consequences:

- **It is soft.** A modified client could ignore the cap. The client is
  closed-source, so tampering is non-trivial, but it is not impossible.
- **It is good enough.** The threat model is a student who wants free fast
  internet, not a determined adversary — and the alternative (per-user
  SS2022 credentials + a server-side usage table) is real work for little
  return at this scale.

This is recorded as a decision, not a gap: see
[`18-open-items.md`](18-open-items.md) for the "do it properly" option if the
softness ever becomes a problem.

### 4.4.4 What the free user sees

The client must make the constraint visible at all times (decision, this
session):

| Signal | When |
|---|---|
| **Usage bar** | Always, on the connection screen. |
| **Warning** | At 80% of the allowance. |
| **Persistent banner** | Once throttled, for the rest of the month. |
| **Upgrade CTA** | In the banner — one tap to Stealth. |

The point is that a throttled student **knows why**, and knows exactly what
fixes it. A silently slow app reads as a broken product; a visibly throttled
one reads as a working free tier with a clear upgrade.

### 4.4.5 What "throttled further" means

Not cut off — slowed. The free tier keeps working after 5 GB, at a much
lower speed, until the month resets. This is deliberately kinder than a hard
stop: it keeps the app *useful* (so the student does not uninstall it) while
making the paid tier obviously worth it. It is not yet implemented in
`04-tc.sh`; the mechanism would be a second tc class the client switches to
on exhaustion, or a server-side per-tier cap change.

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

Eco is currently a live shadowsocks service on port 8443 with its own
password and systemd unit (`tc-eco-cap.service`,
`shadowsocks-eco.service`).

**Decision (this session): reuse Eco's slot in configuration only.** Do not
rename the services. Keep port 8443 and its instance; stop selling Eco codes;
mint free codes against that `tier_configs` row with the cap dropped to
1 Mbps.

Rationale: a true rename means editing `04-tc.sh` and `02-shadowsocks.sh`
(new service names, new port) and **redeploying the hub**. That is a live
system serving paying students, and the rename buys nothing a config change
does not. It is also exactly the kind of cross-file inconsistency that has
caused real defects here before (see `docs/CONTEXT.md` on cross-file
agreement).

The legacy `eco` name therefore survives in the tree while the customer-facing
tier is called **Free**. That asymmetry is intentional and should be
documented wherever it could confuse — see
[`03-product.md`](03-product.md) on the code being the truth.

---

## 4.6 Related reading

- Why the ladder is shaped this way → [`05-pricing.md`](05-pricing.md)
- What free costs to run → [`06-unit-economics.md`](06-unit-economics.md)
- How free feeds growth → [`11-growth.md`](11-growth.md)
- The tier-separation risk → [`14-risks.md`](14-risks.md)
