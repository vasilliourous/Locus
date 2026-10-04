# 3. Product

```
audience:    human-operator
status:      live
authoritative-for: what the customer buys, and what the activation code is commercially
verified-against: docs/reference/DEVICE-IDENTITY.md
```

## 3.1 The three moving parts

The customer buys a signed desktop app plus an activation code. Under the
hood there are three parts, and only one of them is the customer's.

| Part | What it is | Where |
|---|---|---|
| **Client** | Tauri 2 + React desktop app, forked from Clash Verge Rev v2.5.5, tunnelling via mihomo. The Locus logic is ~5,800 lines of Rust. Ships as a 3.x release (current version in [`../STATE.md`](../STATE.md)). | `client/` |
| **Hub** | One DigitalOcean droplet running PocketBase + Caddy + the Shadowsocks/BBR/tc stack. Holds codes, tiers, releases, and the admin console. | `server/` |
| **Retired clients** | Go + Wails and Go + Fyne predecessors. Reference only — they paid for the behavioural contract the fork reproduces. | `legacy/` |

## 3.2 What the customer is allowed to see

The UI shows plan name, connection status, connection metrics, and (for the
free tier) usage against the monthly allowance. It does **not** show protocol
names, ports, or the tech stack.

This is a deliberate product decision inherited from the legacy plan (custom
tier names, no "Shadowsocks"/"BBR" in the interface), and it is enforced by
simply not shipping that text in the front-end. It matters commercially:
the product is sold as *"it works"*, not as *"it is Shadowsocks"* — see
[`13-competition.md`](13-competition.md).

## 3.3 The activation code — a commercial object

- **Form:** `RQ-XXXX-XXXX-XXXX-C` (15 characters).
- **Charset:** `ABCDEFGHJKLMNPQRSTUVWXYZ23456789` — no I/O/0/1.
- **Checksum:** Luhn-mod-N over the body **including** the `RQ` prefix. The
  client, every server hook, and the generator must agree byte-for-byte, or
  a generated code fails on the student's device.
- The pre-2026-08-17 `MYVPN-…` form is dead.

**Business meaning:** a code is a *bearer token that is also a physical
card*. It is generated in bulk, printed, handed to a middleman, sold for
cash, and activated **once**. Everything about distribution
([`08-distribution.md`](08-distribution.md)) and lifecycle
([`10-lifecycle.md`](10-lifecycle.md)) follows from this object's shape.

> **Refinement (`9a91da1`, corrected 2026-10): a code is a VOUCHER, and it is
> also the credential.** Activating it spends the card and starts a running term.
> The student then keeps access without the card in hand, because **the client
> keeps the code itself** in a machine-scoped store — it is re-read on every
> launch and survives an uninstall and a reinstall. `verge.yaml` holds the
> primary copy; `/var/lib/locus` (Linux), `/Library/Application Support/Locus`
> (macOS) and `%PROGRAMDATA%\Locus` (Windows) hold the mirror.
>
> This is what makes a thrown-away card survivable: the code was the purchase,
> and the app remembers it. **It is not addressed by a device fingerprint** —
> device identity and recognition were removed (2026-10); a code is single-use
> and tied to nothing. See
> [`../reference/DEVICE-IDENTITY.md`](../reference/DEVICE-IDENTITY.md).

The code also carries a **term** (`term_days`): how long one purchase lasts,
measured from *activation* rather than from the day the card was printed. A
~70-day term is a school term; ~30 days is a month. The term is what makes the
product repeatable — see [`07-billing.md`](07-billing.md).

> The code carries **no identity**, and nothing else is bound to it either. It
> is anonymous and unbound. That anonymity is deliberate and load-bearing: it is
> why there is no signup, no account and no PII to breach, and it is also why
> per-user traffic accounting is not possible today, which is the central
> constraint on the free tier
> ([`04-tiers.md`](04-tiers.md#44-the-free-tier-in-full)).
>
> **A code is single-use, not device-bound.** Once activated it carries an
> `activated_at` stamp and cannot be activated a second time — but it is not
> tied to which machine did it, so the same student reinstalling on the same
> laptop simply re-enters the same code. Moving back to a fresh machine is a
> re-entry, not an operator operation; `codes.unbind` (release) exists only to
> hand the code to a *different* student.

## 3.4 What the customer never touches

No billing screen, no account, no password reset, no signup flow, no
dashboard of their own. The whole relationship is *card → code → app*. This
is a feature: it removes every support surface a normal SaaS has, and it is
why the operator's real costs are bandwidth and middleman commission rather
than support ([`06-unit-economics.md`](06-unit-economics.md)).
