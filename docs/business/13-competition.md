# 13. Competition and the moat

```
audience:    human-operator
status:      live
authoritative-for: the competitive set, and what the moat actually is
verified-against: docs/state.toml
```

## 13.1 The competitive set

| Competitor class | Why they lose *on this network* |
|---|---|
| **Free VPNs (xVPN/Everest, Psiphon, Lantern, Hotspot Shield)** | Detectable and repeatedly blocked; slow on congested WiFi; free tiers useless for gaming. |
| **Big paid VPNs** | Need a credit card; not tuned for N4L; oversold servers. |

## 13.2 The moat, precisely

- **Shadowsocks over TCP** has no TLS fingerprint for the Palo Alto to match,
  and game/voice UDP rides *inside* that TCP stream via UDP-over-TCP rather than
  depending on the network admitting raw UDP
  ([`02-market.md`](02-market.md#23-the-wedge), [`../GAMING-UDP.md`](../GAMING-UDP.md)).
- **The VPS IP is fresh and unlisted**, unlike the public blocklists free
  VPNs sit on.
- **A dedicated VPS** is not oversold.

> **The moat is "it works here and free ones don't" — not price, not speed.**
> That is a real moat for one school and a fragile one against a firewall
> update ([`14-risks.md`](14-risks.md)).

## 13.3 Why the free tier does not weaken the moat

It is worth stating clearly, because a free tier looks like competing with
the free VPNs on their own terms — and it is not:

- **The free VPNs are free *and* unreliable here.** The free tier is free
  *and works here*. That is a different proposition.
- **The free tier is deliberately too limited to substitute for the paid one.**
  1 Mbps / 10 GB cannot stream, and can only game the games that tolerate it
  (Roblox, Minecraft) — badly. It carries UDP, so it is *not* broken for games;
  it is simply 100× too slow for a competitive one. That is a narrower gap than
  "it cannot game at all", and it is the honest one: the free plan is a
  demonstration of a working tunnel, not a destination.
- **The free tier exists to place the app**, and the app is the thing that
  works when others do not.

## 13.4 The anti-moat: what we do not have

- **No brand.** Nobody knows the name; word of mouth is local and personal.
- **No protocol advantage that is permanent.** Every obfuscation is
  eventually detectable ([`14-risks.md`](14-risks.md#141-protocol-detection--highest-impact)).
- **No switching cost.** A student can leave and use a free VPN the moment
  the free VPN works again.

The last point is why the free tier is strategically useful: it makes Locus
the *default installed app* in a market with no switching cost.

## 13.5 The competitive question that matters

> When the school blocks the free VPNs — and it will, because the blocking
> guide is a deliberate part of the strategy — **who is already installed on
> the student's laptop?**

That is what the free tier is for.
