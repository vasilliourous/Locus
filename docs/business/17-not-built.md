# 17. What is NOT built

```
audience:    human-operator
status:      live
authoritative-for: the anti-drift guard — what is absent, and must not be described as working
verified-against: server/pb_hooks/, client/src-tauri/src/locus/
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
| **Server-enforced free *quota*** | **NOT BUILT — only the free *rate* is enforced server-side.** Free UDP and TCP are both capped at 1 Mbps by `tc` at the hub, but the 10 GB allowance is still counted by the client ([`04-tiers.md`](04-tiers.md#410-the-free-plans-udp-abuse-surface-accepted-and-bounded)). A modified client can exceed the allowance at 1 Mbps indefinitely. |

| Claim | Verdict |
|---|---|
| **Brutal-CC tier differentiation** | **DEAD** — module removed 2026-08-01. All tiers are BBR ([`04-tiers.md`](04-tiers.md#421-the-brutal-cc-differentiation-is-gone)). |
| **Referral cards** (`referred_by`, discount/free-week) | **NOT BUILT — and rejected.** Does not fit a code-gated, physical-distribution product; the middleman already is the referral mechanism ([`11-growth.md`](11-growth.md#112-referral-cards--re-derived-and-rejected)). |
| **Tier bundling** (Strike → free code) | **NOT BUILT — reconsider.** Survives the carrier critique; kept as proposal P3 ([`18-open-items.md`](18-open-items.md)). |
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
| The `free` tier row | **BUILT** — seeded as the free plan's only row. |
| The allowance, sent by the hub | **BUILT** — `free_allowance_mb` / `free_throttle_mbps` on the heartbeat, gated to the free/eco tiers. |
| Client-side counting of the 10 GB allowance | **BUILT** — `client/src-tauri/src/locus/usage.rs`, pure and fake-clock tested. The window, the rollover and the 80% line are one implementation. The *number* is the hub's (`free_allowance_mb`), so the 5→10 GB change needed no client release. |
| The usage bar | **BUILT** — `components/connection/usage-bar.tsx`, wired into the connection screen. Renders nothing for a paying tier. |
| The 80% warning and the throttle banner | **BUILT** — as the bar's colour and its summary line. |
| **The upgrade route beside the throttle state** | **BUILT (3.2.26)** — the throttle line is followed by a sentence naming the action (contact the middleman about Full). There is no self-serve checkout to link to; a code is a physical card ([`08-distribution.md`](08-distribution.md)). |
| **Applying the throttle to a running Core** | **NOT BUILT.** The hub sends `free_throttle_mbps`, the client parses and *stores* it (`store::throttle_mbps`), and nothing yet lowers a live Core's speed from it. This is the honest boundary: the **decision** to throttle works, the **act** does not exist — see [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md). |
| **Operator control of the allowance from the console** | **NOT BUILT.** The value is in the hook, so it is changeable without a release but not from the console UI ([`09-console.md`](09-console.md) §9.3). |
| **Whether the throttle actually slows a real connection** | **NOT VERIFIED.** The classification is tested; the effect on a running tunnel is not — see [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md). |

> **All of this is inert on the hub until `setup.sh` re-runs**, because the hub
> deploys from `/root/server/` rather than from this repo. A heartbeat whose
> response lacks `free_allowance_mb` is proof the hook is still stale. The
> *client* half is live as soon as the matching version installs.

See [`18-open-items.md`](18-open-items.md) for the implementation items.

## 17.2 The merge's build status

The Free/Full merge is **mostly documentation and config**, and this is the
boundary between what the tree now says and what the hub now does.

| Piece | Status |
|---|---|
| `04-tc.sh` — 8444 retired, 8445 at 100mbit | **BUILT** — and the module's existing filter flush drops the stale 8444 class on its next run. |
| `02-shadowsocks.sh` — no `stealth` service | **BUILT** — a fresh box gets two shadowsocks instances, not three. |
| `seed-pb.py` — no `stealth` row | **BUILT** — the seed loop enumerates two tiers. |
| The client's tier name | **BUILT** — the badge and palette carry the merged tier's key; the hub keeps emitting `strike`. |
| `check-consistency.sh` §23 for the paid tier, and §27 for the console list | **BUILT** — asserted and observed failing against the pre-merge tree. |
| **The retired `stealth` service on a live box** | **NOT DONE — and cannot be, from the tree.** `setup.sh` is skip-if-exists and will not remove it. This is an operator step ([`14-risks.md`](14-risks.md#148-the-tier-merge-strands-a-live-endpoint-and-nothing-removes-it)). |
| **Existing `stealth` codes in the field** | **UNKNOWN — a world claim.** Whether any code carries `tier: "stealth"` is not answerable from this checkout; the probe and the re-verify command are in [`../operate/CLAIMS.md`](../operate/CLAIMS.md) §5. Until it is answered, the merge ships with a known unknown about real students' codes. |
| **That the merged tier connects on the redeployed hub** | **NOT VERIFIED.** The cap, the row and the client's reader agree *in the tree*; that a student's client reaches 8445 and gets UDP on the live hub has not been run here. |

> **Nothing in the hub half is live until `setup.sh` re-runs.** A paid code whose
> heartbeat still resolves a 200 Mbps class, or a `shadowsocks-stealth` that is
> still `is-active`, is proof the box has not taken the merge
> ([`15-continuity.md`](15-continuity.md#153-what-this-merge-changed-about-continuity)).

See [`18-open-items.md`](18-open-items.md) for the implementation items.
