# The client front-end — the surfaces a student meets

```
audience:    builder
status:      live
authoritative-for: the shipping two-tab UI and the rules each surface obeys
verified-against: client/src/pages/, client/src/components/
```

> The front-end was replaced in the 3.1.0 rework: the inherited Clash Verge
> screens were deleted and Chrysalis's own two-tab application was built in their
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

`src/main.tsx` renders `Shell`, which reads `useEntitlement()` and branches on its
pure decision function, `gateScreen(activated, suspended)`:

```
activated === null  -> a spinner                       (first read, nothing else)
!activated          -> <ActivationScreen>              — BEFORE `suspended`
suspended           -> <SuspendedScreen status=…>      (the kick-out page)
otherwise           -> RouterProvider -> Layout -> the two tabs
```

This is a **gate, not a redirect**: an unactivated device cannot reach the router,
the profile machinery or the connect controls by any route. "Render nothing else
until activated" is a property that is easy to hold; "every page remembers to
check" is not.

**The order of the last two branches is load-bearing**, and it is why the decision
is a pure function rather than an `if` chain in the component. A suspension keeps
the credential (that is what makes unsuspending need no re-entry), so `suspended`
implies `activated` — but the pair `activated: false, suspended: true` was
*reachable*, because signing out removed the code and left the flag. The kick-out
screen deliberately offers no controls, so that pair left a student with nothing to
press and no exit but wiping the app's data. Testing `!activated` first means the
kick-out page can only ever be shown to a device that holds a code. It is enforced
three times over — on write (`store::clear_entitlement`), on read
(`store::is_suspended`) and here — and `gateScreen`'s five cases are pinned in
`hooks/use-entitlement.test.ts`. The full state model is
`docs/reference/SUSPENSION.md`.

Four rules live here and each was a real bug:

- **Everything that can change the answer asks again, immediately.** The hook
  re-reads on the backend's `chrysalis://entitlement-changed` event, on the local
  `entitlement-refresh` signal, and on window focus, `pageshow`, `online` and
  `visibilitychange`. The interval (`SUBSCRIPTION_POLL_MS`) survives as a **floor**,
  not as the mechanism: it is what keeps "event-driven" from meaning
  "event-dependent", since every event above is something that might not fire. The
  original defect was the opposite shape — the poll was the only trigger, so a
  withdrawal reached the screen up to five minutes late and the workaround a
  student found was to close and reopen the app.
- **A read *failure* only forces "not activated" on the FIRST read.** A transient
  failure while running must not tear a working connection screen away
  mid-session. The same branch must never *clear a suspension*: keeping what we
  have is the safe direction, because the other one silently re-admits a device an
  operator just kicked out.
- **The initial loading overlay is removed here, in `Shell`**, not in `Layout`.
  `Layout` is unreachable on an unactivated device, so removing the overlay there
  left it covering the activation screen permanently — a blank window over a
  working app.
- **The kick-out screen takes the status it renders.** `SuspendedScreen` used to
  issue its own `chrysalisStatus()`, making it a third independent reader of a fact the
  gate and the sidebar also read — and the one that disagreed was the one on
  screen. It is now a pure render of the gate's read.

### The gate reads the status, and nothing mutates it out of band

`useEntitlement()` owns the read. There are two signals that say "the account
changed" — `chrysalis://entitlement-changed` from the backend and
`services/entitlement-refresh.ts` from the renderer — and the guidance for a new
surface is to read the status from the hook, never to fetch it again:

- **A backend change** (a suspension on a background beat, a reinstatement, a
  sign-out) emits `chrysalis://entitlement-changed`. The payload is a label for logs;
  the authoritative state is always the re-read, so a missed event delays the
  screen rather than making it lie.
- **A student action** (sign-out) calls `requestEntitlementRefresh()` from the
  renderer *and* the backend emits on its own path. Both land in the hook, so the
  gate leaves the app immediately instead of after a poll.

There used to be a second writer: a **device-recognition** check
(`chrysalis_recognise()`) that asked the hub whether it already knew the device, and
STORED the entitlement as a side effect. Because `activated` is not what that
command returns, the successful case had to bump an `entitlementRestored` counter
to make the status effect re-read — otherwise a returning student sat on the code
prompt for a full poll interval while the app knew they had a working device.

**All of that is gone.** Device recognition was removed. The general rule it
produced still applies, and the hook is now the place it is enforced:

**The rule to carry forward:** if a command ever mutates the entitlement, it must
announce it — `runtime::notify_entitlement_changed(reason)` from Rust, or
`requestEntitlementRefresh()` from the renderer. Nothing in the type system or the
linter can see the omission, because both halves are individually correct. See the
2026-10-01 entry in `docs/reference/FIXES.md`.

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
`connected && !ready` and renders as `connecting`, never as a false positive.

`use-connection.ts` owns the state machine, the polling intervals (15 s idle, 750 ms
while connecting) and `toggle()`. Two rules there matter:

- **A poll never stomps an in-flight transition with a stale reading**, but a
  reading that *matches* the transition must be allowed to advance or end it —
  otherwise the button strands on "connecting" after a connect fails.
- **`connecting` is reachable two ways** — a connect the student started, and a
  half-dialled core found on app open. A press means "cancel" in the first case
  and "take the half-formed tunnel down" in the second; `connectInFlightRef`
  distinguishes them, because `chrysalisCancelConnect` only raises a flag for a
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

## 3b. The support route, and signing out

### Every stuck screen offers a way to reach a person

`components/shared/support-block.tsx` is the one implementation of "here is who
to email and what to say", rendered on:

| Screen | When |
|---|---|
| Activation gate | Always. The student is stuck by definition, and this is the only screen a person who has not activated will ever see. |
| Connection | Only when something is wrong: `connectNotice(...).kind === 'refused'`, a lapsed subscription, a live `error`, or `!canConnect`. See `connectionNeedsSupport` in `connect-notice.ts` — a pure rule, because a screen that quietly stops offering the block is a dead end nothing else in the build notices. |
| Account | Always, with the Device ID hint. This page holds the two things a report needs: the Device ID and the app version. |

Two facts live in exactly **one** place — the address (`supportEmail`) and the
reply promise (`supportBlock.replyPromise`) — and `check-consistency.sh` §41
fails if the address is duplicated or the promise stops naming its window.

**The reply promise is a world claim, not a code property.** "Answered within 12
hours, not counting nighttime" is a statement about the operator's behaviour; it
is recorded in [`../../docs/operate/CLAIMS.md`](../../docs/operate/CLAIMS.md) §5
with a re-verify, and the app renders it because the operator made it. Changing
the sentence in the bundle changes what students are promised, so it is a product
decision, not a copy edit.

The hub URL is deliberately **not** here (or anywhere in the UI except the
Account diagnostics command's own output). It is an address the client calls, not
a place a student is sent, and a hostname cannot answer "my code says it is
already used".

### Signing out

"Sign out on this device" (`components/shared/sign-out-card.tsx`) forgets the key
on this computer and tells the hub. What it must **not** do is read as "cancel my
subscription": the dialog says in words that the subscription is not cancelled and
not shortened, and that this is the control to use when moving to a new computer.
Without those two sentences nobody dares press it, and whoever does expects to
lose the time they paid for.

Two three-second promises, both owned by `pages/sign-out-timer.ts` and asserted
there:

1. **An undo window.** Confirming starts a countdown; the sign-out runs when the
   window closes, so nothing is destroyed while the student can still change their
   mind. The countdown reads 3, 2, 1 — never a final `0`, which reads as "nothing
   is happening" during the second something happens.
2. **A readable result.** The outcome stays on screen for three seconds before the
   app returns to the activation gate.

The outcome is **three sentences, not one**: the backend reports
`released | nothingToRelease | notConfirmed`, and only the first two mean the hub
knows. Collapsing them would tell a student on a dead network that their key had
been released.

After a sign-out the gate re-reads immediately rather than waiting for its
five-minute poll — `services/entitlement-refresh.ts` is the signal, and the reason
it is a module rather than a prop is in that file. The five-minute cadence is
right for a fact the *hub* changes; it is wrong for one this device just changed.

The Rust half, and why its step order is load-bearing twice over, is
[`../../client/src-tauri/src/chrysalis/signout.rs`](../src-tauri/src/chrysalis/signout.rs);
the hub half is [`../../docs/reference/API.md`](../../docs/reference/API.md) §5c.

---

## 4. The status the UI reads

`chrysalis_status` (`cmd/chrysalis.rs`) is the single poll the UI is built on. It is
deliberately small and never fails — a status query that can error forces the UI
to invent a state, and the honest answer when storage cannot be read is "not
activated".

> **The TypeScript types for this are generated, not written.** `services/chrysalis.ts`
> re-exports `ChrysalisStatus` and the eight sibling IPC types from
> `services/chrysalis.generated.ts`, which `server/scripts/gen-ts-contract.py` derives
> from the Rust types in `cmd/chrysalis.rs`. Change the Rust, run
> `pnpm gen:contract`, and commit the diff. A hand-written interface here would
> shadow the generated one; `check-consistency.sh` §33 and §38 both fail if one
> reappears. The frozen names across the *hub* boundary are
> [`../../docs/reference/CONTRACT.md`](../../docs/reference/CONTRACT.md).

| Field | Meaning |
|---|---|
| `activated` | This device has a complete, usable entitlement record. |
| `tier` | The tier name (`free`/`budget`/`strike` — customer-facing **Free**/**Budget**/**Full**), when known. A retired name is never returned. A name this build predates renders through `TIER_FALLBACK`, which is correct — do not make the label a guess. |
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

## 5. Appearance: the palette, and the theme layer on top

**Colour is not described in this document.** Every palette, value and rule lives
in [`/COLOURS.md`](../../COLOURS.md) — the single home for colour across the
client, the hub and the landing page. Read §1 there for how a value reaches the
screen, §2 for the shipped palette, and §9 for the contrast rules a palette must
satisfy.

What this section owns is the *structure* the palette flows through:

- `src/pages/_theme.tsx` holds `CHRYSALIS_COLORS` — the one palette — plus the
  MUI-shaped `defaultTheme` projection and `TIER_COLORS`. The fields are
  **defaults, not overrides**: `services/theme.ts` reads each as `setting.X ||
  dt.X`, so a student who customised a colour keeps it. A custom colour is a
  preference on top of the one appearance, not a second appearance.
- `src/services/theme.ts` builds `chrysalisTheme` **once at module load** and exposes
  `applyThemeWhenReady()`, which writes the window-chrome CSS variables and the
  global stylesheet. There is no theme hook: nothing can change the theme at
  runtime, so nothing re-renders when it does not.
- **The window's background exists in four layers that cannot share code** — the
  native window (`utils/resolve/window.rs`), the pre-bundle bootstrap script
  (`utils/resolve/window_script.rs`), the document (`src/index.html`,
  `src/assets/styles/index.scss`), and the app (`_theme.tsx`). They are pinned
  against each other by `tests/theme-colors.test.ts` instead of by a shared
  constant, because layer 2 cannot import the theme module and the Rust layers
  cannot import it at all. All four must change together or the window flashes
  the wrong colour on startup. `/COLOURS.md` §1 has the table.
- **Tier identity** (`TIER_COLORS`, `TIER_FALLBACK`) is per-tier so the badge
  reinforces the plan without being relied on, and `TIER_FALLBACK` covers a tier
  this build does not know so an unknown tier still renders.

### There are no themes

The registry, the resolver, the decoration presets, the Account dropdown and the
theme hook were **deleted on 2026-11-19**. Chrysalis has one permanent appearance
(Dawn); `/COLOURS.md` §0 records the decision and §3 records what was removed.
An archived copy of the old layer's mechanics is in
[`../../docs/archive/THEMES.md`](../../docs/archive/THEMES.md) — read it for *why*
that layer existed, never as an instruction, because every file it names is gone.

Two properties are worth keeping in mind if a second appearance is ever proposed:

- **Shape and decoration travelled by CSS variables** (`--card-radius`,
  `--control-radius`) and one `<style id="chrysalis-theme-decoration">` element. The
  variables survive — there is one set of values and `cardSx` still reads
  `var(--card-radius, 12px)` — but nothing injects a decoration any more.
- **`setting.X || dt.X` is unchanged.** It is the only runtime input to the
  palette, and it is a preference, not a mode.

`src/pages/_surfaces.ts` defines the shared card styling, so every card is the
same object. Note the app zeroes MUI's shadow scale app-wide and uses a 1px border
instead — a card that relied on `elevation` would be invisible.

---

## 6. What is deliberately absent from the front-end

| Not present | Why |
|---|---|
| Profiles UI (`pages/profiles.tsx`, `components/profile/`) | A student activates a code; they do not manage subscriptions. The profiles **engine** remains load-bearing — only the UI was removed. |
| Connections table, Rules browser, Unlock checker | Tools for choosing between nodes, or unrelated to a VPN. |
| Node picker, core selector, TUN/system-proxy switches, ports, DNS | One server per tier, one core, decided by Chrysalis and the hub. Exposing them is the interface the product exists to remove. |
| Deep links entirely | They existed solely to import subscription URLs. |
| A Chrysalis logo asset | `src-tauri/icons/*` is still the Clash Verge Rev mark. A separate 48×48 shield is a brand decision nobody has taken, so the activation screen shows the wordmark alone. Marks: [`/COLOURS.md`](../../COLOURS.md) §8. |
