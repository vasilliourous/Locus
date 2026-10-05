# 4. Tiers

```
audience:    human-operator
status:      live
authoritative-for: the tier table, what each tier is in the code, and the free tier's design
verified-against: server/modules/04-tc.sh, server/modules/02-shadowsocks.sh, docs/state.toml
```

The product now has **two tiers**: a permanent **free** tier with a data
allowance, and **one** paid tier. This file describes what each one is in the
code, what changed from the legacy model, and the free tier's design in full.

> **There is no ladder any more, and that is the decision.** The three-tier
> ladder ($0/$4/$7) was replaced by a single paid plan at $5. The reasoning is
> in §4.2.4 and [`05-pricing.md`](05-pricing.md); the short version is that the
> two paid tiers were never different enough to be worth the confusion, and the
> only thing a student ever actually wanted to buy was *the fast one*.

---

## 4.1 The current, code-verified tier table

| Plan | TCP port | UDP (UoT) | Hard cap (tc) | Kernel | Data | What the student gets |
|---|:--:|:--:|:--:|---|---|---|
| **Free** | 8443 | ✓ 8447 | **1 Mbps** (both TCP and UDP) | BBR | 10 GB/mo, then throttled | Chat, browsing, **and light gaming** (Roblox, Minecraft). |
| **Full** | 8445 | ✓ 8446 | **100 Mbps** TCP; UDP uncapped | BBR | Unmetered | Everything, **including heavy FPS titles** and 4K streaming. |

> **UDP is on both plans; the rate is the difference.** That is the whole ladder
> now. The free plan carries UDP-over-TCP on its own listener (8447) with its own
> credentials, capped at the hub to the same 1 Mbps as its TCP tunnel; the paid
> plan carries it on 8446, uncapped, behind a 100 Mbps TCP tunnel. A student on
> Free can play Roblox or Minecraft at 1 Mbps — badly, but it works, which is the
> point — and a student who wants a competitive FPS needs the 100× rate.

Source of truth for the caps: `server/modules/04-tc.sh` (the `apply_tc_now` and
`create_tc_service` calls); for the UoT listener,
`server/modules/02-shadowsocks.sh`. The free tier's 1 Mbps cap **is in the tree**
(`04-tc.sh`, port 8443) and the allowance is sent by the hub on every heartbeat
as `free_allowance_mb` (`server/pb_hooks/heartbeat.pb.js`); both are held in
agreement by `check-consistency.sh` §23, which since this change holds the
**paid** tier's cap and UoT endpoint to the same standard (§4.7).

> **Both UDP listeners are separate, and that is a credential requirement.**
> A sing-box shadowsocks inbound is bound to **one** password, so serving both
> plans from one listener is impossible without handing one plan the other's
> credential. The free listener therefore serves `ECO_PASS` on 8447 and the paid
> one serves `STRIKE_PASS` on 8446. It also allows them to be shaped
> independently, which is the security half of this change (§4.10).
>
> **The paid tier is called `strike` in the database and `Full` on the card.**
> Its endpoint, its password and its systemd units all keep the `strike` name
> (`/etc/shadowsocks/strike.json`, `tc-strike-cap.service`), because a code in
> the field already carries the string `strike` and the hub resolves a code's
> tier by that string. The customer-facing name is **Full**. This is the same
> asymmetry §4.5 describes for `eco`/`free`, for the same reason — and it is why
> **no wire name changed** in this merge (§4.7).

Source of truth for the caps: `server/modules/04-tc.sh` (the `apply_tc_now` and
`create_tc_service` calls); for the UoT listener,
`server/modules/02-shadowsocks.sh`. The free tier's 1 Mbps cap **is in the tree**
(`04-tc.sh`, port 8443) and the allowance is sent by the hub on every heartbeat
as `free_allowance_mb` (`server/pb_hooks/heartbeat.pb.js`); both are held in
agreement by `check-consistency.sh` §23.

> **There is ONE row for the free plan: `free`.** The hub used to seed a second
> row named `eco` against the same port 8443, kept in case a code minted in the
> Eco era still carried that string. That guard was retired on evidence
> (2026-10-06): the live hub holds **no** code with `tier: "eco"`, so the row was
> a duplicate of `free` that showed up in the console's tier list as a second
> plan sharing one port. Removed from `seed-pb.py` and `fix-tier-configs.py`.
>
> **The on-box names stay `eco` and that is not an inconsistency.** The free
> plan's infrastructure — `shadowsocks-eco.service`, `tc-eco-cap.service`,
> `sing-box-uot-eco.service`, `/etc/shadowsocks/eco.json`, and the `ECO_PASS`
> credential — keeps its name because renaming it means new ports and new units
> for no product change. The *tier row* is what went, not the plan's plumbing.
> See §4.5.

---

## 4.2 What changed from every legacy document

### 4.2.1 The Brutal-CC differentiation is gone

Both legacy business documents sold Stealth on a "Brutal" congestion-control
kernel module (`tcp-brutal` + `LD_PRELOAD`). **That module was removed on
2026-08-01** (`docs/reference/FIXES.md`). All tiers now run plain BBR.

Consequences:

- The old "Stealth's jitter makes gaming unplayable → upgrade to Strike"
  argument no longer has a mechanism behind it. There is no jitter generator.
- **Tier differentiation is now: the data allowance and whether UoT is on.**
  That is the entire ladder — two tiers, one paid.

### 4.2.2 Eco is replaced by Free

The legacy ladder had a $2 "Eco" tier at 5 Mbps. That tier is **removed** and
replaced by a free tier at 1 Mbps with a 10 GB monthly allowance.

Why this is better than keeping a cheap paid anchor:

- **Free is a funnel, not a price point.** A student who would never pay $2
  will still install a free app, which makes word-of-mouth work through a
  much larger population ([`11-growth.md`](11-growth.md)).- **Free makes the paid upsell easier, not harder.** The jump from
  *1 Mbps / 10 GB* to *100 Mbps / unmetered* is enormous, and it costs the
  student $5 — a decision that does not need to be deliberated.
- **It removes the "is $2 worth it?" question entirely.** There is no longer
  a tier a student has to *justify*; the only question is whether they want
  fast.

### 4.2.3 ~~The paid gap is small on purpose~~ — superseded by §4.2.4

The $4→$7 gap was deliberately small, and the reasoning was sound for a
**two-tier paid ladder**: once a student is paying anything, the step up is
trivial. It was superseded when the ladder became one tier, because a gap
between two paid tiers stops being a question once there is only one paid tier
to buy.

### 4.2.4 Stealth and Strike are merged into one paid tier

**Decision (this change): two tiers — Free and Full — not three.**

This is not a rename. Read together, `04-tc.sh` and `02-shadowsocks.sh` show
that Stealth and Strike were **the same plan at two rates**:

| | Stealth | Strike |
|---|---|---|
| Kernel | BBR | BBR |
| Data | Unmetered | Unmetered |
| UDP | ✗ | ✓ (UoT on 8446) |
| Cap | 100 Mbps | 200 Mbps |

The **only** structural difference was the UDP-over-TCP endpoint. The 200 Mbps
cap was pure positioning: this directory's own numbers say no game needs it
(Valorant ~5 Mbps, Fortnite ~20 Mbps — §4.3), so the second paid tier charged $3
for a number no student could use, to separate itself from a tier that did the
same job.

Three consequences, and each is a reason this is the better product:

- **One paid decision, not two.** The student's question becomes "do I want it
  fast?" instead of "which of these two do I want?". The §4.2.2 argument that
  killed the $2 anchor applies to a second paid tier too.
- **The paid price drops from $7 to $5**, and the tier gets *better* for the
  student who was previously buying the cheaper one: Full is Strike's UDP path at
  Stealth's 100 Mbps, for $1 more than Stealth cost.

> **What the merge gives up.** A 200 Mbps headline. That is the trade, and it is
> deliberate: the cap exists to stop one user saturating the droplet (§4.3), not
> to be advertised. Retiring it also removes an entire tc class (§4.7).

> **What did not change.** The free tier, its 1 Mbps cap, its client-side
> counting, and the whole of §4.4. Free was already the funnel; this change only
> makes the thing at the top of the funnel one product instead of two.

---

## 4.3 What still makes the plans work together

The differentiator is **rate**, and only rate.

- **Free exists to place the app on the device.** It is deliberately usable
  but constrained — enough to prove it works *and enough to play the games a
  13-year-old actually plays badly*, not enough to live on. Its job is installs,
  and installs are what make the next block-window matter
  ([`13-competition.md`](13-competition.md#135-the-competitive-question-that-matters)).
- **Full is the product.** 100 Mbps unmetered for $5 is the whole paid offer.
  What it sells over Free is not a capability but a *rate*: 100× the throughput,
  which is the difference between "the game connects" and "the game is worth
  playing".
- **The 100 Mbps cap is not a limitation to overcome.** It is far above what any
  game or a household's worth of streaming needs (Valorant ~5 Mbps, Fortnite
  ~20 Mbps), and it exists to stop one user saturating the droplet. It is the
  same rate Stealth charged $4 for; the merge did not lower what a paid student
  gets, it lowered the count of things they have to choose between.

### 4.3.1 Why UDP is given away rather than sold

This reverses the previous position, which sold UDP as the paid plan's
differentiator. Three reasons, and the third is the one that decided it:

1. **UDP is table stakes on a school network, not a premium feature.** N4L drops
   raw UDP, so *without* UoT a game does not merely run slowly — it does not
   connect at all. A free tier whose games are entirely broken is indistinguishable
   from a broken app, and the free tier's entire job is to place a working app on
   the device.
2. **The physics already does the selling.** At 1 Mbps, Roblox and Minecraft are
   playable-but-dreadful; a competitive shooter is not. The rate *is* the gate, so
   gating UDP as well buys nothing and costs a working demo.
3. **It removes the "is it actually working?" doubt at install time.** A student
   whose first attempt fails at the network layer blames the app, not the plan,
   and uninstalls. That is the most expensive possible outcome for a funnel whose
   product is an installed app.

> **What this costs.** Free UDP is real bandwidth from a population expected to be
> far larger than the paid one (§4.4.6, [`06-unit-economics.md`](06-unit-economics.md#63-the-free-tiers-cost)),
> and it is the widest abuse surface the free plan has (§4.10). Both are accepted
> deliberately rather than overlooked.

---

## 4.4 The free tier, in full

### 4.4.1 Parameters

| Parameter | Value |
|---|---|
| Price | **$0** |
| Speed cap | **1 Mbps** (tc) |
| Monthly allowance | **10 GB** |
| After the allowance | **Throttled further** (not cut off) |
| Enforcement | **Client-side counting** |
| Port | 8443 (reuses the old Eco slot) |

### 4.4.2 Why a data allowance, not a user cap

A user cap protects *instantaneous* load (concurrent users on the pipe). A
data allowance protects *aggregate* cost (bytes off the droplet) and
self-limits naturally: a free user who streams video burns their allowance
and stops; a free user who only chats uses almost nothing for a month and
costs the business almost nothing.

This is the better fit for a 10 GB-at-1-Mbps tier, and it means the free tier
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

Not cut off — slowed. The free tier keeps working after 10 GB, at a much
lower speed, until the month resets. This is deliberately kinder than a hard
stop: it keeps the app *useful* (so the student does not uninstall it) while
making the paid tier obviously worth it.

**How it works now.** The `tc` class on port 8443 is the hard ceiling at 1 Mbps
either way; the "further" throttle is the speed the client drops to once the
allowance is spent, sent by the hub as `free_throttle_mbps` and *stored* by the
client.

> **The honest limit, and it is unchanged by this merge.** The client stores that
> speed and **does not yet lower a running Core's speed from it**. The behaviour
> is decided, the number arrives, and the act of applying it to a live tunnel
> does not exist — see [`17-not-built.md`](17-not-built.md) and
> [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md). Because the server's
> 1 Mbps `tc` class is the hard ceiling regardless, no student is worse off than
> the free tier's promise; but the app's wording describes an intention, not a
> behaviour. The 30-day window, the 80% warning line and the classification all live
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
and the same N4L link as paying users. At 1 Mbps and 10 GB, an individual
free user is cheap; the risk is *volume*.

- **10 GB × 1,000 free users = 10 TB/month**, which is not cheap and not small.
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
| `server/scripts/seed-pb.py` | seeds **one** free row against 8443 (the `eco` duplicate is gone) | a second row on the same port read as a second plan in the console |
| `server/pb_hooks/heartbeat.pb.js` | sends `free_allowance_mb` for the free/eco tiers | the allowance is server-provided so it changes without a release |

`check-consistency.sh` **§23** asserts the agreement across all of them, and
across the client's reader for the key. It was observed failing against each
half-deployed state before it was kept.

§23(g) additionally pins the **prose** that restates the caps — the root
`README.md`'s summary line and `04-tc.sh`'s own header comment — against
`state.toml`. Both had drifted to the retired `5 Mbps` figure while every
value-check stayed green (a check on the value is not a check on what a document
says about it), so the restatement is now checked too.

Rationale: a true rename means new service names and a new port, i.e. editing
`04-tc.sh` and `02-shadowsocks.sh` and **redeploying the hub**. The rename buys
nothing a config change does not, and it is exactly the kind of cross-file
inconsistency that has caused real defects here before (see `docs/CONTEXT.md` on
cross-file agreement).

The legacy `eco` name therefore survives in the tree while the customer-facing
tier is called **Free**. That asymmetry is intentional and should be documented
wherever it could confuse — see [`03-product.md`](03-product.md) on the code
being the truth.

> **The `eco` row was removed on 2026-10-06, and no student was affected.**
> The row existed so that a code minted in the Eco era would still resolve; the
> live hub was checked and holds no such code. The old note here said an existing
> `eco`-tier code would keep its 1 Mbps cap — true, and moot, because there are
> none. If one ever appears it will not resolve, which is a deliberate trade: a
> duplicate plan in the console costs every operator, every day, while a
> stranded Eco code costs nobody today.

---

## 4.6 The same choice, made again for Stealth → merged into Full

The §4.5 decision was **reuse the slot, rename nothing on the box**, and the
paid merge (§4.2.4) is the same decision applied to the other end of the ladder:

| Property | Stealth | Full (the merged tier) |
|---|---|---|
| Customer-facing name | **Stealth** | **Full** |
| On-box name (service, config, tier row) | `stealth` | **`strike`** |
| Port | 8444 | **8445** |
| Password | `STEALTH_PASS` | **`STRIKE_PASS`** |
| tc unit | `tc-stealth-cap.service` | **`tc-strike-cap.service`** |

**Why the survivor is `strike`, not `stealth`.** The customer-facing name is new
either way, so the only question is which *string* the database keeps — and that
one is decided for us:

- A code minted before this change carries the string `strike` or `stealth` in
  its `tier` field, and `activation.pb.js`/`heartbeat.pb.js` resolve a tier by
  looking up **that string** in `tier_configs`. Keeping `strike` means every
  existing paid code resolves to the merged endpoint with **no data migration and
  no code edit**.
- The survivor has to be the tier with the **UoT listener and its credentials**.
  The sing-box UoT endpoint serves the strike tier's password
  ([`../GAMING-UDP.md`](../GAMING-UDP.md)), so keeping `strike` keeps that pair
  intact. Choosing `stealth` would mean re-pointing UoT credentials, which is the
  one change that could break gaming without breaking anything visible.

**What happens to `stealth` codes in the field.** `04-tc.sh` no longer shapes
8444 and `02-shadowsocks.sh` no longer creates the service — so a `stealth` code
must either resolve to the survivor's endpoint or it is stranded. That
disposition depends on a fact this checkout **cannot** answer (which tier strings
live codes actually carry), so it is a world claim with a live probe, recorded in
[`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5 — not a sentence here.

Critically, the same rule as §4.5 applies: **do not rename the frozen wire
name.** A deployed client's tier vocabulary is a *client-side* allow-list in
places (see the `bound_this_device` incident in
[`../reference/FIXES.md`](../reference/FIXES.md)), so the hub keeps emitting
`tier: "strike"` and the *label* is the thing that changes.

---

## 4.7 What this merge costs at deploy time

Three files define a tier, and the merge touches all three plus the scripts that
enumerate them. This is the checklist, and `check-consistency.sh` §23 asserts the
agreement rather than each file separately:

| File | What changed | Why it matters |
|---|---|---|
| `server/modules/04-tc.sh` | the 8444 class is **removed**; 8445 is `100mbit` | the stale 8444 filter is dropped by the module's existing `tc filter del … parent 1:` flush, so the retired port stops being shaped rather than silently keeping its class |
| `server/modules/02-shadowsocks.sh` | `stealth`'s config and unit are **no longer created**; `strike`'s `tcp_and_udp` mode is unchanged | the UoT path rides on strike's credentials, so they must not move |
| `server/scripts/seed-pb.py` | no `stealth` row; `strike` keeps 8445 + `udp_relay` | a code resolves by its tier string, so the row it names must exist |
| `server/scripts/check-consistency.sh` | §23 extended to the paid tier; new §27 for the console's tier list | see §4.8 |
| `setup.sh`, `smoke-test.sh`, `verify-ss2022.sh`, `write-admin-credentials.sh`, `scripts/generate_codes.sh`, `scripts/print_codes.sh` | `stealth` removed from every enumerated tier list | a script that still expects `shadowsocks-stealth` reports a false failure against a correct hub |

**Two operational facts, stated plainly because neither is automatic:**

1. **A re-run does not remove what is already on the box.**
   `write_service` and `create_tc_service` are *skip-if-exists* by design — they
   are there so a re-run is idempotent, not so they can delete things.
   `/etc/shadowsocks/stealth.json` and `shadowsocks-stealth.service` therefore
   survive a `setup.sh` re-run and would keep serving 8444 with a working
   password. **Disabling and removing them is an explicit operator step**
   (`systemctl disable --now shadowsocks-stealth`, remove the unit and the
   config). Leaving them up means an uncapped paid endpoint that nobody can sell
   and nobody can see in the console.
2. **The hub half is inert until `setup.sh` re-runs.** The tc caps, the tier rows
   and the allowance all live on the box. A heartbeat for a paid code that still
   reports a 200 Mbps class — or an `/api/health` that is fine while `tc class
   show` is not — is the proof of a stale deploy.

---

## 4.8 How the merge is held in agreement

`check-consistency.sh` **§23** already asserted the free tier's agreement across
`04-tc.sh`, `seed-pb.py`, `heartbeat.pb.js` and the client's reader. This change
extends the same treatment to the paid tier, and adds one new guard:

- The **paid tier's cap** is pinned to `100mbit` in both the applied class and
  `tc-strike-cap.service`, and the retired 8444 class must **not** come back.
- The **UoT endpoint** is asserted two-sided: `seed-pb.py` advertises `uot_port`
  for `strike`, and the client declares the key that carries it.
- **§27 (new)** holds the console's tier dropdown to the hub's actual tier rows.
  The console's list is a **hardcoded literal** in `server/console/src/views/Codes.vue`,
  not a read of `tier_configs` — so a tier can exist on the hub and be
  unmintable, or be offered and unresolvable. That is exactly the two-sources-of-
  truth defect [`../operate/OPS.md`](../operate/OPS.md) documents for hooks.
- **§23(g)** — the prose check — is extended to the paid tier, because the
  existing guard greps `README.md` for `tc caps (1/100/200 Mbps)`, which this
  change invalidates. A guard that greps a literal is doing its job when a
  deliberate change has to update it in the same commit.

---

## 4.10 The free plan's UDP abuse surface (accepted, and bounded)

Free UDP is the largest attack surface this product has, and it is worth being
explicit about why it is nonetheless acceptable.

**The exposure.** The free plan's *quota* is counted client-side
(§4.4.3) because the hub cannot tell one free user from another. Before this
change that softness was bounded by a hard `tc` ceiling on 8443 — a modified
client could ignore the counter but still could not exceed 1 Mbps. UDP widened
that: a free client that ignored its own cap could stream UDP at whatever the
link allowed.

**The mitigation, and it is the reason the listener is separate.** The free UDP
listener (8447) carries its **own 1 Mbps `tc` class**, so the hub enforces the
free rate regardless of what the client claims. The client's cap is now a
courtesy that keeps a student inside their allowance; the hub's cap is the
enforcement. A tampered client can still ignore the *counter* — burning more
than 10 GB — but it cannot exceed 1 Mbps doing it, which bounds the cost.

**What is deliberately NOT mitigated.** The byte counter is still client-side,
so a modified client can exceed the 10 GB allowance at 1 Mbps indefinitely. The
fix for that is per-user credentials and a server-side usage table, which is
[`18-open-items.md`](18-open-items.md) P4 — real work, justified only if the
softness ever actually costs money. At 1 Mbps, a permanently-maxed abuser costs
roughly one full-time stream, which is affordable and visible.

> **The rule this leaves behind:** any *new* free-tier transport must arrive with
> a server-side `tc` ceiling, or it inherits none. The client-side counter is a
> product decision; it must never be the only thing standing between the free plan
> and an unbounded bill. `check-consistency.sh` §27(h) asserts the 8447 class
> exists for exactly this reason, and fails the build if it is removed.

---

## 4.11 Related reading

- Why there is one paid tier, and why $5 → [`05-pricing.md`](05-pricing.md)
- What free costs to run → [`06-unit-economics.md`](06-unit-economics.md)
- How free feeds growth → [`11-growth.md`](11-growth.md)
- The tier-separation risk → [`14-risks.md`](14-risks.md)
- What is decided but unbuilt → [`17-not-built.md`](17-not-built.md)
