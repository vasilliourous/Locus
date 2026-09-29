# 17. What is NOT built

Claims that appeared in the legacy business documents or have been floated in
planning, and which **do not exist in the current code**.

Verified by grep: `referred_by`, `referral`, `warp`, `trial`, `free_week`
across `server/`, `scripts/`, and `client/src-tauri/src/locus/` return
nothing; `"protocol"`, `v2ray`, `trojan` in the hooks and Locus modules
return nothing.

> **Updated `9a91da1`.** The term model, renewal, one-code-per-device and
> search-by-name are **now built** and have moved out of this file — see
> [`10-lifecycle.md`](10-lifecycle.md) and [`09-console.md`](09-console.md).
> This file is the anti-drift guard, so it is updated *with* the change rather
> than after it: anything below is still genuinely absent.

## 17.0 Still not built after `9a91da1`

| Thing | Status |
|---|---|
| **Fingerprint durability across a reinstall** | **NOT BUILT.** The binding is now enforced (one code per device), but the *client's fingerprint* still falls back to a value persisted only in its own config, so a reinstall can still present a new identity. Deferred deliberately: the fix's core property cannot be verified without real Windows/macOS hardware ([`redesign/10-open-questions.md`](redesign/10-open-questions.md) §10.2). |
| **A middleman ledger** | **NOT BUILT.** Renewal detail now records optional `price`/`middleman`, but nothing aggregates them ([`09-console.md`](09-console.md)). |
| **Renewal prompting / notification** | **NOT BUILT.** Nothing tells the operator a given student is due; the worklist is passive ([`redesign/01-identity-without-pii.md`](redesign/01-identity-without-pii.md) §1.2). |
| **A verified live migration** | **NOT DONE.** The backfill script exists with a dry-run, but it has never been run against the deployed hub, which holds real paying codes ([`redesign/05-migration-and-live-data.md`](redesign/05-migration-and-live-data.md)). |
| **A verified end-to-end renewal** | **NOT DONE.** The arithmetic and the wire handling are tested; a renewal reaching a *running* client is not. |

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

The free tier is a **planning decision with a small build**, not a shipped
feature:

| Piece | Status |
|---|---|
| Reuse port 8443 / the Eco slot with a 1 Mbps cap | **Not yet applied** — a config + `tc` change ([`04-tiers.md`](04-tiers.md#45-the-deploy-time-choice-renaming-eco--free)). |
| The 5 GB allowance, counted client-side | **Not built.** |
| The usage bar, 80% warning, throttle banner, upgrade CTA | **Not built** ([`04-tiers.md`](04-tiers.md#444-what-the-free-user-sees)). |
| Progressive throttling after the allowance | **Not built** ([`04-tiers.md`](04-tiers.md#445-what-throttled-further-means)). |

See [`18-open-items.md`](18-open-items.md) for the implementation items.
