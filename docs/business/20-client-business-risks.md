# 20. What in the client gets in the way of the business

```
audience:    human-operator
status:      live
authoritative-for: client-side commercial risks, ranked, with the evidence for each
verified-against: client/src/, client/src-tauri/src/locus/, server/pb_hooks/ (2026-11-04)
```

> **How to read this.** Each item is one of the four claim kinds this repo uses
> (`../operate/CLAIMS.md` §2 and root `AGENTS.md` rule 4), and says which:
>
> - **verified** — established from this checkout, with the file and the
>   mechanism named. Re-checkable by reading the code.
> - **structural** — follows from the code, but needs a box or a device to confirm.
> - **unverified** — believed, not checked. Says what would settle it.
> - **world claim** — about the deployed system; would live in `CLAIMS.md` §5.
>
> Ranked by *money at risk per unit of effort*, not by severity alone. The two
> changes shipped on 2026-11-04 (the support route, and sign-out) close the first
> two; they are listed with what is left.

---

## 1. A student who cannot activate now has a route; a student who cannot *buy* has none

**Verified.** `server/site/index.html` contains no contact route of any kind —
no `mailto:`, no address, no form (`grep -ci "mailto\|contact\|email"` → `0`).
The landing page is the top of the funnel and it is the one page with no way to
ask a question.

**What changed.** The support block now exists on the activation gate, the
Connection screen and Account (`client/src/components/shared/support-block.tsx`),
so a student who is *already in the app* can reach a person. That was the larger
hole: the gate is where a stuck student actually is.

**What is left.** Someone deciding whether to buy has no way to ask what they are
buying. For a product sold through middlemen this may be deliberate — the
middleman *is* the sales channel — but then the page should say so, and it says
nothing at all. **Cost:** one sentence of copy, or one mailto. **Risk:** every
pre-purchase question becomes a lost sale or, worse, a support email from someone
who is not yet a customer.

---

## 2. Sign-out now covers the deliberate move; a *lost* device still needs the operator

**Verified.** The only path that released a key server-side was
`POST /api/admin/unbind-code` (`server/pb_hooks/admin_unbind.pb.js`), which
requires `ADMIN_API_TOKEN`. A student cannot have one.

**What changed.** "Sign out on this device" (Account page) forgets the key
locally — including the machine-scoped mirror that survives an uninstall
(`locus/credential.rs`) — and records the release on the hub.

**What is left, and why it matters commercially.** Sign-out requires the student
to *press a button on the machine they are leaving*. The case that generates the
support load is the machine that is gone: lost, stolen, wiped, dead disk. That
student has no device to sign out from, so they still email, and the operator
still needs `ADMIN_API_TOKEN` by hand.

That is acceptable at current volume — one operator, a few a month — but it does
**not** scale, and it is invisible in the product: nothing on the Account page
says "lost your computer? ask us to move your key." **Cost:** one sentence plus a
console action that already exists (`codes.unbind`). **Cheap, and it converts a
support conversation into a self-service one.**

> **Do not "fix" this by letting a code-holder release any code.** The sign-out
> endpoint is deliberately restricted to an audit record
> (`docs/reference/API.md` §5c): releasing a redemption from a request
> authenticated only by a code printed on a card would let one student reset
> another's. See `FIXES.md`, 2026-11-04.

---

## 3. The free tier's throttle is described in the UI and never applied

**Verified.** `locus_throttle_mbps` is parsed from the heartbeat
(`locus/heartbeat.rs:174`), persisted (`locus/runtime.rs:519`), and read by
nothing. `usage.rs` says so itself, and `docs/reference/STILL-OPEN.md` carries it
as open.

**Why this is a business risk and not just a TODO.** The free plan's *stated*
value proposition is "1 Mbps after the allowance"; the *delivered* one is
unthrottled after the allowance, because the cap depends on the client choosing to
apply it, and the client does not. So:

- the limit is enforced by a client the student controls (`usage.rs`: "a modified
  client could ignore the cap"), and
- the cap the honest client would apply is not even applied.

A paying student's complaint — "the free one is just as fast" — is therefore
**correct on the current build**, and it is the single most corrosive thing that
can be true about a two-tier product. **Cost:** the tc class already exists on the
hub; this needs the client-side application path. **This is the largest
unaddressed commercial defect in the client.**

---

## 4. Supporting one OS costs more than the others, and nothing says so

**Verified in the tree, unverified on devices.** The release process carries four
platforms (`docs/operate/RELEASING.md`), and `FIXES.md` records repeated
platform-specific breakage: macOS got nothing offered and nothing said why
(2026-10-04), the Windows installer was unsigned (2026-10-01), Windows TLS failed
with an expired root (2026-10-18), macOS shipped a terminal-only remedy (2026-10-04).

**The business shape.** Every platform multiplies the support surface, and the
operator is one person. Two platforms well is worth more than four badly — but
there is no decision recorded anywhere about which are *supported* and which are
*best-effort*, so every platform bug is treated as equally load-bearing.

**What would settle it:** a stated support matrix in `docs/business/`, and the
download page matching it. **Cost:** a paragraph. **Effect:** it makes "we do not
officially support that" an answer the operator can give, instead of a feeling.

---

## 5. The app cannot tell a student "this is your network, not your purchase"

**Verified.** `locus/hub/client.rs` and `locus/activation_tunnel.rs` already
distinguish censorship-shaped failures (a reset during the TLS handshake) from
ordinary ones, and the activation tunnel exists precisely because a school filter
can block the hub by name.

**The gap.** That distinction is used to *retry*, not to *explain*. Where it
surfaces to a student it is as "could not reach the Locus hub at …", which reads
as "the product is broken" to someone whose school is blocking it. That student
then emails, and the operator's reply is "try your phone's hotspot" — a correct
answer delivered late, one student at a time.

**Cost:** medium (one sentence, on the failure path that already exists). **Risk
if left:** every school-network block becomes a support email, and the operator's
reply is identical each time.

---

## 6. `describe_outcome` is English-only in a translated app

**Verified.** `client/src-tauri/src/cmd/locus.rs:791` — `describe_outcome` — every activation refusal
("This code has already been activated on a different device…", "You will need a
new one") is a Rust string literal. The UI is translated into 13 locales; these
sentences are not.

**Risk:** modest, and mostly fairness rather than money — a student reading the app
in Arabic or Japanese hits English at the most confusing moment of their
experience. It is listed because it is cheap to see and easy to forget, and
because the refusal sentences are exactly the ones that need to be understood.

---

## 7. Deliberately *not* on this list, having checked

Recorded so a successor does not re-litigate them:

- **"Connected but nothing loads" is not currently a one-source claim.** Readiness
  reaches `Ready` on **either** proven egress **or** observed traffic
  (`core/manager/probe.rs:284` `decide`), so the month-long dead egress probe
  (fixed on 2026-11-04, `docs/reference/FIXES.md`) did not leave "connected"
  resting on a single source. The earlier worry was overstated — the second source
  is independent and real.
- **The free allowance being client-side is a stated decision, not an oversight**
  (`locus/usage.rs` module docs; `docs/business/04-tiers.md` §4.4.3). It is soft
  by design, with the threat model written down. Item 3 above is about the
  *throttle*, which is a different mechanism and is not applied at all.
- **Single shared per-tier passwords** — same trade-off, same recorded reasoning.
  Counting per student would need per-user SS2022 credentials and a server-side
  usage table; that is a project, and it is not implied by any of the above.

---

## What I would do, in order

1. **Apply the throttle** (§3). It is the only item where the product currently
   fails to deliver something it advertises, and the failure favours the free tier.
2. **Say on the Account page what to do when the computer is gone** (§2). One
   sentence; the console action already exists.
3. **Put a contact route on the landing page, or say the middleman is the
   channel** (§1). One sentence either way.
4. **State the support matrix** (§4), and make the download page agree.
5. The rest when it is annoying enough.

> **Nothing in items 1–4 needs a client release to *decide*** — except item 3's
> copy if it goes in the app, and item 1's mechanism. Items 1 and 3 are worth
> planning as one release rather than two.
