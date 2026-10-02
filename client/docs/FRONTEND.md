# The client front-end — the surfaces a student meets

```
audience:    builder
status:      live
authoritative-for: the shipping two-tab UI and the rules each surface obeys
verified-against: docs/STATE.md
```

> The front-end was replaced in the 3.1.0 rework: the inherited Clash Verge
> screens were deleted and Locus's own two-tab application was built in their
> place. This file describes what those surfaces **are**, so the next change does
> not have to rediscover the rules from the code.
>
> `UPSTREAM-CHANGES.md` §4 lists *what was removed* from upstream. This file
> describes *what replaced it*. Where this document and the code disagree, the
> code is right — fix it in the same change.

---

## 1. The shape: two tabs, and a gate

A student sees **two destinations**, plus a first-run gate that stands in front of
everything:

| Surface | Route | What it is |
|---|---|---|
| Connection | `/` | The product. One Connect control, the status, the traffic, the subscription. |
| Account | `/account` | The device and tier, the subscription in detail, diagnostics. |

The navigation is defined in `src/pages/_navigation-meta.ts` and consumed by
`_navigation.tsx` and `_routers.tsx`. It was four tabs (Home, Proxies, Logs,
Settings); three were Clash Verge surfaces a student should never meet — a node
picker for a one-server-per-tier product, a raw core log, and a settings page
exposing core choice, TUN, system proxy, ports and DNS.

### The activation gate owns everything behind it

`src/main.tsx` renders `Shell`, which polls `locusStatus()` and branches:

```
activated === null  -> a spinner          (first read, nothing else)
!activated          -> <ActivationScreen> (the only thing on screen)
activated           -> RouterProvider -> Layout -> the two tabs
```

This is a **gate, not a redirect**: an unactivated device cannot reach the router,
the profile machinery or the connect controls by any route. "Render nothing else
until activated" is a property that is easy to hold; "every page remembers to
check" is not.

Three rules live here and each was a real bug:

- **The gate re-polls** (on `SUBSCRIPTION_POLL_MS`). The entitlement can be
  withdrawn by a background heartbeat at any moment — a suspension, an expiry, a
  refund — and reading it once meant the app kept showing a connection screen for
  a device that no longer had one. The workaround a student found was to close and
  reopen the app.
- **A read *failure* only forces "not activated" on the FIRST read.** A transient
  failure while running must not tear a working connection screen away
  mid-session. `setActivated((current) => current ?? false)`.
- **The initial loading overlay is removed here, in `Shell`**, not in `Layout`.
  `Layout` is unreachable on an unactivated device, so removing the overlay there
  left it covering the activation screen permanently — a blank window over a
  working app.

### The gate reads the status, and nothing mutates it out of band

`Shell` renders `ActivationScreen` until `locus_status` reports `activated`, and
re-reads that on an interval (`ENTITLEMENT_POLL_MS`) so a background refusal — a
suspension, an expiry, a refund — moves the student off a working connection
screen rather than leaving them on one until they restart.

There used to be a second writer: a **device-recognition** check
(`locus_recognise()`) that asked the hub whether it already knew the device, and
STORED the entitlement as a side effect. Because `activated` is not what that
command returns, the successful case had to bump an `entitlementRestored` counter
to make the status effect re-read — otherwise a returning student sat on the code
prompt for a full poll interval while the app knew they had a working device.

**All of that is gone.** Device recognition was removed, so the status read is
the only writer and there is no counter to keep in step.

**The rule to carry forward:** if a command ever mutates a polled value again,
the poll must be told. Join the two at the call site; nothing in the type system
or the linter can see the omission, because both halves are individually correct.
See the 2026-10-01 entry in `docs/reference/FIXES.md`.

### Routing is a HASH router — do not "simplify" it

`createHashRouter`, not `createBrowserRouter`. In production Tauri serves `dist/`
over the `tauri://localhost` custom protocol, which has **no server behind it**: a
browser router requests a real document at `/proxies`, nothing answers, and
navigation and reload break. The hash router keeps the route in the fragment
(`#/proxies`), so the document path never changes.

---

## 2. The connection state machine

`src/components/connection/phase.ts` — a **pure function**, deliberately in its
own module importing only a type, so the rule can be unit-tested without mounting
React:

```
not activated                        -> 'unknown'      (the gate owns the UI)
activated, core not running          -> 'disconnected'
activated, core running, not ready   -> 'connecting'
activated, core running and ready    -> 'connected'
```

**Both `connected` and `ready` are consulted, and their disagreement IS the
connecting phase.** `connected` means the engine process exists; `ready` means it
has bound its ports and can carry a packet — a gap of seconds on a real
connection. `ready` is **observed**: the backend asks the Core's own control API
(`CoreManager::observe_readiness`) and refuses to report `ready` for a Core it
cannot get an answer from, so a machine with no usable network reports
`connected && !ready` and renders as `connecting`, never as a false green.

`use-connection.ts` owns the state machine, the polling intervals (15 s idle, 750 ms
while connecting) and `toggle()`. Two rules there matter:

- **A poll never stomps an in-flight transition with a stale reading**, but a
  reading that *matches* the transition must be allowed to advance or end it —
  otherwise the button strands on "connecting" after a connect fails.
- **`connecting` is reachable two ways** — a connect the student started, and a
  half-dialled core found on app open. A press means "cancel" in the first case
  and "take the half-formed tunnel down" in the second; `connectInFlightRef`
  distinguishes them, because `locusCancelConnect` only raises a flag for a
  running command to notice.

---

## 3. What the connection screen says, and in what order

`src/pages/connect-notice.ts` is a second pure function, for the same reason as
`phase.ts`: the *priority* between messages is the thing that was wrong.

```
1. the hub refused this device   -> show the hub's own sentence, verbatim
2. an error from this attempt    -> show it, verbatim
3. phase === 'unknown'           -> the generic "find your code on the card"
4. otherwise                     -> no notice
```

The refusal is checked **first and unconditionally**. A refused device is
*unactivated* — a refusal withdraws the entitlement — so the screen used to ask a
suspended student for the code on their card, with no mention that anything had
happened. The refusal sentence is the whole answer to "why can I not connect", and
it is rendered as the hub wrote it: an operator's wording ("contact your
middleman") is an instruction, not a description, and paraphrasing it would
replace an action with a description.

The same rule backs the **activation screen** (`activation.tsx`), which is what a
refused device renders and therefore the only place the reason can be told.

---

## 4. The status the UI reads

`locus_status` (`cmd/locus.rs`) is the single poll the UI is built on. It is
deliberately small and never fails — a status query that can error forces the UI
to invent a state, and the honest answer when storage cannot be read is "not
activated".

| Field | Meaning |
|---|---|
| `activated` | This device has a complete, usable entitlement record. |
| `tier` | The tier name (`eco`/`stealth`/`strike`), when known. |
| `deviceId` | The fingerprint, **truncated** — enough for support, not a shareable id. |
| `connected` | The core works. The UI derives its phase from this **and** `ready`. |
| `ready` | The core can carry a packet (observed, not latched). |
| `subscription` | A pre-parsed tagged union — see below. |
| `lastConfirmedAt` | Unix seconds of the last accepted heartbeat, or `null`. |

### The subscription is a closed set of cases, not an optional date

`SubscriptionStatus` is one of `unknown` / `lapsed` / `active { daysRemaining,
urgent, expiresAt }` / `refused { reason }`. The rule that matters is **never
invent a date**: an absent, empty or unparseable value is `unknown`, and the UI
renders nothing — a guessed expiry is a false claim about someone's account, and
`unknown` deliberately **allows** a connect (refusing on a guess would end valid
subscriptions).

`refused` is checked ahead of the unactivated fallback, because a refusal always
clears the code — testing `activated` first would make the state unreachable and
the reason invisible.

One shared query (`useSubscription`) feeds three surfaces so they cannot disagree:
the **sidebar chip** (`layout-subscription.tsx` — a permanent chip that reads
*unconfirmed* before any beat, *expires in N days* / *expires today*, turns amber
inside the warning window and red when lapsed, and shows the hub's own words on a
refusal), the **connection screen** warning, and **Account**. The activation gate in `main.tsx`
shares the *interval constant* with it rather than the cache (it renders outside
`SWRConfig`), so the two can in principle differ for one poll cycle — acceptable,
and noted in `docs/reference/STILL-OPEN.md`.

---

## 5. Appearance: the Locus palette, and the theme layer on top

`src/pages/_theme.tsx` holds `LOCUS_COLORS` (dark) and `LOCUS_LIGHT`, both
exported and both **defaults, not overrides** — `use-custom-theme` reads each
field as `setting.X || dt.X`, so a student who customised a colour keeps it.

- The brand is a green-black window with Locus green (`#2EA86A`) as the single
  accent. It comes from `docs/archive/UI-AESTHETICS.md`, which was written for the
  retired client and never ported — which is why the fork shipped Clash Verge's
  iOS blue and grey on every screen.
- **Light mode is derived, not quoted.** The spec is dark-only. The light accent is
  a *darker* green (`#1E7A4A`, ~4.6:1 on white) for text-bearing controls, because
  `#2EA86A` on white is ~3.0:1 and fails the body-text contrast rule.
- **The window's background colour exists in three layers that cannot share code**
  — the native window (`utils/resolve/window.rs`), the document (`src/index.html`),
  and the theme (`_theme.tsx`). They are pinned against each other by
  `tests/theme-colors.test.ts` instead of by a shared constant, because layer 2
  cannot import the theme module and layer 1 is Rust. All three must change
  together or the window flashes the wrong colour on startup.
- **Tier colours** (`TIER_COLORS`) are per-tier so the badge "sells itself";
  `TIER_FALLBACK` covers a tier this build does not know, so an unknown tier still
  renders.

### The named themes are a layer over this, not a replacement for it

`src/pages/_themes.ts` holds a registry of six named themes (`default-dark`,
`default-light`, `midnight`, `paper`, `high-contrast`, `forest`) that a student
picks from a dropdown on **Account**. The design record is
[`../../docs/reference/THEMES.md`](../../docs/reference/THEMES.md) — read it before
touching the registry.

The three properties that make it safe to have alongside this section:

- **It is additive.** `verge.theme_id` unset — every existing install — resolves
  through `theme_mode` exactly as before. `default-dark` and `default-light` are
  built from `LOCUS_COLORS`/`LOCUS_LIGHT` rather than restating them, so the
  shipped appearance is a *member* of the registry, not a copy that can drift.
- **`setting.X || dt.X` is unchanged.** A selected theme changes only which base
  `dt` is; the custom-colour precedence above still applies field by field.
- **Shape and decoration travel by CSS variables** (`--card-radius`,
  `--control-radius`) and one `<style id="locus-theme-decoration">` element scoped
  to `[data-theme-skin]`. `cardSx` reads `var(--card-radius, 12px)` — the explicit
  fallback is what an unthemed app and the pre-hook startup window render.

A theme **cannot** change structure, tier identity, or what a state says. A
decoration is a **named preset**, never injected CSS: a theme that could emit
arbitrary CSS could restyle any component, and the registry test asserts no preset
contains `{`, `}` or `url(`.

`src/pages/_surfaces.ts` defines the shared card styling, so every card is the
same object. Note the app zeroes MUI's shadow scale app-wide and uses a 1px border
instead — a card that relied on `elevation` would be invisible.

---

## 6. What is deliberately absent from the front-end

| Not present | Why |
|---|---|
| Profiles UI (`pages/profiles.tsx`, `components/profile/`) | A student activates a code; they do not manage subscriptions. The profiles **engine** remains load-bearing — only the UI was removed. |
| Connections table, Rules browser, Unlock checker | Tools for choosing between nodes, or unrelated to a VPN. |
| Node picker, core selector, TUN/system-proxy switches, ports, DNS | One server per tier, one core, decided by Locus and the hub. Exposing them is the interface the product exists to remove. |
| Deep links entirely | They existed solely to import subscription URLs. |
| A Locus logo asset | `src-tauri/icons/*` is still the Clash Verge Rev mark. `UI-AESTHETICS.md` §4 asks for a 48×48 `#2EA86A` shield; drawing a brand mark needs a human decision, so the activation screen shows the wordmark alone. |
