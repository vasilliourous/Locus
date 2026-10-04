# 17. What is NOT built

```
audience:    human-operator
status:      live
authoritative-for: the anti-drift guard — what is absent, and must not be described as working
verified-against: docs/STATE.md
```

Claims that appeared in the legacy business documents or have been floated in
planning, and which **do not exist in the current code**.

Verified by grep: `referred_by`, `referral`, `warp`, `trial`, `free_week`
across `server/`, `scripts/`, and `client/src-tauri/src/locus/` return
nothing; `"protocol"`, `v2ray`, `trojan` in the hooks and Locus modules
return nothing.

> **Updated `9a91da1`, corrected 2026-10.** The term model, renewal and
> search-by-name are **built** and documented in
> [`10-lifecycle.md`](10-lifecycle.md) and [`09-console.md`](09-console.md).
> **Device binding and one-code-per-device were removed, not finished** — the
> live model is a single-use, unbound code; see
> [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md). This
> file is the anti-drift guard, so it is updated *with* the change rather than
> after it: anything below is still genuinely absent.

## 17.0 Still not built

| Thing | Status |
|---|---|
| **Free-tier quota that the operator controls** | **NOT BUILT.** The allowance is counted client-side and is not configurable from the console ([`09-console.md`](09-console.md) §9.3, [`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)). |
| **A middleman ledger** | **NOT BUILT.** Renewal detail records optional `price`/`middleman`, but nothing aggregates them ([`09-console.md`](09-console.md)). |
| **Renewal prompting / notification** | **NOT BUILT.** Nothing tells the operator a given student is due; the worklist is passive ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md) §1.2). |
| **A verified live migration** | **NOT DONE.** The term backfill script exists with a dry-run, but it has never been run against the deployed hub ([`redesign/05-migration-and-live-data.md`](redesign/05-migration-and-live-data.md)). |
| **A verified end-to-end renewal** | **NOT DONE.** The arithmetic and the wire handling are tested; a renewal reaching a *running* client is not. |
| **Per-user traffic accounting** | **NOT BUILT — and structurally blocked.** One code per tier shares a single Shadowsocks password, so free users are indistinguishable on the wire ([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)). This is what forces the client-side quota. |

| Claim | Verdict |
|---|---|
| **Brutal-CC tier differentiation** | **DEAD** — module removed 2026-08-01. All tiers are BBR ([`04-tiers.md`](04-tiers.md#421-the-brutal-cc-differentiation-is-gone)). |
| **Referral cards** (`referred_by`, discount/free-week) | **NOT BUILT — and rejected.** Does not fit a code-gated, physical-distribution product; the middleman already is the referral mechanism ([`11-growth.md`](11-growth.md#112-referral-cards--re-derived-and-rejected)). |
| **Tier bundling** (Strike → free code) | **NOT BUILT — reconsider.** Survives the carrier critique; kept as proposal P6 ([`18-open-items.md`](18-open-items.md)). |
| **Warp Lite / Cloudflare Warp tier** | **NOT BUILT** (no `warp` anywhere). |
| **Protocol swapping via heartbeat** | **NOT BUILT** ([`15-continuity.md`](15-continuity.md#151-the-gap-stated-plainly)). |
| **Server-side free-tier quota** | **NOT BUILT** — no per-user accounting exists; the quota is client-side ([`04-tiers.md`](04-tiers.md#443-why-enforcement-is-client-side-and-what-that-means)). |
| **Fake protocol names in UI** | *Partially honoured* — the UI simply shows tier names; no rename logic needed. |
| **Client-side bandwidth throttling** | **Changed** — caps are enforced **server-side** by `tc`. The *quota* is the one client-side control. |

## 17.1 The free tier's build status

The free tier is **partly built and partly not** — this table is the boundary.
The honest summary: the *enforcement* works end to end; the *purchase* does not.

| Piece | Status |
|---|---|
| The 8443 cap at 1 Mbps | **BUILT** — `04-tc.sh`, both the applied class and the reboot unit, held in agreement by `check-consistency.sh` §23. |
| The `free` tier row | **BUILT** — seeded alongside the legacy `eco` row. |
| The allowance, sent by the hub | **BUILT** — `free_allowance_mb` / `free_throttle_mbps` on the heartbeat, gated to the free/eco tiers. |
| Client-side counting of the 5 GB allowance | **BUILT** — `client/src-tauri/src/locus/usage.rs`, pure and fake-clock tested (11 tests). The window, the rollover and the 80% line are one implementation. |
| The usage bar | **BUILT** — `components/connection/usage-bar.tsx`, wired into the connection screen. Renders nothing for a paying tier. |
| The 80% warning and the throttle banner | **BUILT** — as the bar's colour and its summary line. |
| **The upgrade route beside the throttle state** | **BUILT (3.2.26)** — the throttle line is followed by a sentence naming the action (contact the middleman about Stealth). There is no self-serve checkout to link to; a code is a physical card ([`08-distribution.md`](08-distribution.md)). |
| **Applying the throttle to a running Core** | **NOT BUILT.** The hub sends `free_throttle_mbps`, the client parses and *stores* it (`store::throttle_mbps`), and nothing yet lowers a live Core's speed from it. This is the honest boundary: the **decision** to throttle works, the **act** does not exist — see [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md). |
| **Operator control of the allowance from the console** | **NOT BUILT.** The value is in the hook, so it is changeable without a release but not from the console UI ([`09-console.md`](09-console.md) §9.3). |
| **Whether the throttle actually slows a real connection** | **NOT VERIFIED.** The classification is tested; the effect on a running tunnel is not — see [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md). |

> **All of this is inert on the hub until `setup.sh` re-runs**, because the hub
> deploys from `/root/server/` rather than from this repo. A heartbeat whose
> response lacks `free_allowance_mb` is proof the hook is still stale. The
> *client* half is live as soon as the matching version installs.

See [`18-open-items.md`](18-open-items.md) for the implementation items.
