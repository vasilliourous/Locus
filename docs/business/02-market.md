# 2. Market

```
audience:    human-operator
status:      live
authoritative-for: who the customer is, and why the incumbents fail them
verified-against: docs/state.toml
```

## 2.1 Who the customer is

Students at N4L-managed schools. The live market is Macleans College.

They want, in rough order of frequency:

1. **To play online games** during breaks and free periods.
2. **To watch video** (YouTube, TikTok) that is bandwidth-starved or blocked.
3. **To reach sites** the school firewall restricts.

The first is the highest-value need and the hardest for anyone else to serve
on this network — it is the entire reason the paid tier exists
([`04-tiers.md`](04-tiers.md#424-stealth-and-strike-are-merged-into-one-paid-tier)).
Note the shift the merge made: games used to justify the **premium** (Strike at
$7); with one paid plan they justify **the paid plan itself**, which is a
stronger position — the paid tier now has a reason to exist that a free VPN
cannot copy here, rather than merely being the faster of two paid options.

## 2.2 Why the incumbents fail them

| Competitor class | Why they lose *on this network* |
|---|---|
| **Free VPNs** (xVPN/Everest, Psiphon, Lantern, Hotspot Shield) | Detectable and repeatedly blocked; slow on congested WiFi; free tiers useless for gaming. |
| **Big paid VPNs** | Need a credit card (most students lack one); not tuned for N4L; oversold servers. |
| **Self-hosting** | Needs technical skill students do not have. |

## 2.3 The wedge

The school's firewall (N4L's Palo Alto) filters by **TLS fingerprint** and by
**UDP**. A Shadowsocks-over-TCP stream is neither, so it passes. Free VPNs
that rely on TLS-shaped or UDP protocols do not.

> **We are not cheaper than free, and we are not faster than a good VPN. We
> are the option that still works on this specific network.**

This is also the moat's single point of failure — see
[`13-competition.md`](13-competition.md) and
[`14-risks.md`](14-risks.md).

## 2.4 The timing window

Two events concentrate demand, and both are worth planning around:

- **When a free VPN gets blocked.** Students who relied on it need a
  replacement *that day*. Being present at that moment is worth more than
  any advertising. See [`11-growth.md`](11-growth.md) for how the blocking
  guide and the free tier interact.
- **Term boundaries.** Demand spikes at the start of each of the four NZ
  school terms, which is also when cash is collected. See
  [`07-billing.md`](07-billing.md).

## 2.5 What the market does *not* contain

There is no evidence in the repository of a second school, a second country,
or an adult market. Every figure in this directory assumes **one school**.
Treat any plan that needs a second market as unproven
([`12-scale-and-ceiling.md`](12-scale-and-ceiling.md)).
