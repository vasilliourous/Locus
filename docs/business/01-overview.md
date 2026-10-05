# 1. The business in one page

```
audience:    human-operator
status:      live
authoritative-for: the business in one page, and the three sentences that drive the rest
verified-against: docs/state.toml
```

| | |
|---|---|
| **Product** | Locus — a small commercial VPN service sold to students at N4L-managed NZ schools (the live deployment is Macleans College). |
| **The job it does** | Lets a student on school WiFi reach games, video, and sites the school firewall blocks. |
| **How** | Shadowsocks 2022 over **TCP** — no TLS fingerprint for the firewall to match, no UDP dependency for the base tiers. |
| **Form** | Signed desktop app (Windows-primary; macOS/Linux builds exist), activation-code gated, no self-signup. |
| **Tiers** | **Free · Full.** Two plans, no ladder. See [`04-tiers.md`](04-tiers.md). |
| **Prices** | **$0 · $5** per month. See [`05-pricing.md`](05-pricing.md). |
| **Distribution** | Friends act as middlemen and sell paper code cards for cash. See [`08-distribution.md`](08-distribution.md). |
| **Cost base** | One ~$6/mo VPS. See [`06-unit-economics.md`](06-unit-economics.md). |
| **Ambition** | Cover costs, pay a coffee, stay a side project. See [`12-scale-and-ceiling.md`](12-scale-and-ceiling.md). |
| **The honest risk** | The firewall could learn to detect the traffic. See [`14-risks.md`](14-risks.md). |

---

## Read this next

- **What is it?** → [`02-market.md`](02-market.md), [`03-product.md`](03-product.md)
- **What does it cost / earn?** → [`05-pricing.md`](05-pricing.md), [`06-unit-economics.md`](06-unit-economics.md)
- **How does it reach people?** → [`08-distribution.md`](08-distribution.md), [`11-growth.md`](11-growth.md)
- **What could end it?** → [`14-risks.md`](14-risks.md), [`15-continuity.md`](15-continuity.md)

---

## The three sentences that matter

1. **We are not cheaper than free, and not faster than a good VPN — we are
   the option that still works on this specific network.**
2. **Revenue comes from volume in the cheap plan, not margin at the top.**
3. **This is a side project that pays for itself and a bit more; the plan is
   not built to reward chasing scale.**

Each of these drives a decision elsewhere in this directory. The first sets
the competitive position ([`13-competition.md`](13-competition.md)); the
second sets the price ladder ([`05-pricing.md`](05-pricing.md)); the third
sets the ambition ([`12-scale-and-ceiling.md`](12-scale-and-ceiling.md)).
