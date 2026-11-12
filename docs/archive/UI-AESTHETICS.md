# Locus UI / Aesthetics — Visual Design Spec

> **⚠️ STATUS: describes the ARCHIVED client.**
> This document describes `legacy/wails-client/` — the retired Go + Wails +
> sing-box client. **The shipping client is `client/`** (the Tauri fork of Clash
> Verge Rev, tunnelling through mihomo), which has its own docs in
> `client/docs/`. Paths below that read `legacy/wails-client/` were rewritten
> from `legacy/wails-client/` in the 2026-09-23 restructure; the content was not otherwise
> reviewed. Kept because it documents the contract the fork must reproduce —
> see `legacy/wails-client/ARCHIVED.md`.
>
> **Its colour content has been removed.** This document used to carry the
> palette, the button and badge colours, and the CSS for them. Those values are
> gone from it on purpose — they were superseded twice and were still being cited
> as a source, and a retired spec cannot be allowed to look like an authority on
> what the product looks like. **Colour is owned by `/COLOURS.md` at the
> workspace root and by nothing else.** What remains here is layout, type scale
> and behaviour, none of which is colour.

---

## 1. Design Principles

1. **One glance, one action** — The user opens the window and instantly knows
   what to do.
2. **Status without reading** — Colour, icon and position convey connection state
   at a glance. (The colours are `/COLOURS.md`'s; that state is conveyed by more
   than one channel is the rule this document owns.)
3. **Tier sells itself** — Subtle visual cues differentiate the plans without
   extra UI complexity.

**Non-goals:**
- No custom animations (CSS transitions are kept minimal)
- No custom fonts (system fonts look native and load instantly)
- No heavy graphics (SVG icons only, keep the binary small)

---

## 2. Colour

**Deliberately absent.** The palette this section used to state was superseded in
3.3.0 and again in the 2026-11-11 monochrome reset, and the value that made it
dangerous was that it *looked* current. Read `/COLOURS.md` §2 for the shipped
palette, §11 for what happened to the values that used to be written here.

What this document still owns about paint: the window background is also set
**natively**, so there is no flash while the WebView loads. That mechanism — not
its value — is the part worth keeping, and the value is `/COLOURS.md`'s.

---

## 3. Typography

| Use | Size | Weight | Align |
|-----|------|--------|-------|
| Brand title | 28px | 700 | Center |
| Card title | 18px | 600 | Left |
| Status text | 15px | 600 | Center |
| Body / labels | 13px | 400–500 | Left |
| Tier badge | 12px | 600 | Center |
| Code input | 16px | 400 (monospace) | Left |
| Diagnostics | 12px (monospace) | 400 | Left |

Font: system default stack (`-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, …`).
The activation code input uses a monospace stack (`'SF Mono', 'Fira Code', 'Cascadia Code'`).

---

## 4. Layout: Activation Screen

**Window:** 480×700 (min 380×500), Wails app. The activation screen is shown
when the device is not activated (`App.vue` → `v-if="!activated"`).

```
┌──────────────────────────────────────┐
│                                      │
│            [shield logo SVG]         │  brand icon, 48×48
│                 Locus                │  brand title, 28px bold
│          Secure School VPN           │  subtitle, 14px
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Activate                      │  │  card (max-width 380px)
│  │  Enter the code from your      │  │
│  │  activation card               │  │
│  │  ┌──────────────────────────┐  │  │
│  │  │ RQ-XXXX-XXXX-XXXX-C      │  │  │  code input, monospace, auto-format
│  │  └──────────────────────────┘  │  │
│  │  ✓ Valid code / ✗ error hint   │  │  live Luhn validation
│  │  ┌──────────────────────────┐  │  │
│  │  │        Activate          │  │  │  button, spinner while loading
│  │  └──────────────────────────┘  │  │
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  ○ Eco      — $2/mo — Browsing │  │  tier info card
│  │  ◉ Stealth  — $4/mo — Streaming│  │
│  │  ⚡ Strike   — $8/mo — Gaming  │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

**Implementation notes (ActivationScreen.vue):**
- Logo: inline SVG shield with checkmark
- Input: auto-inserts hyphens as the user types (max 23 chars), uppercases,
  and calls `ValidateCode` when complete — the hint reports valid or the error
  message
- Button: disabled until ≥16 chars, shows a CSS spinner while loading
- Errors from the server appear in an error box below the button

---

## 5. Layout: Main Screen

Shown when activated. Header + connection card + stats card + actions.

```
┌──────────────────────────────────────┐
│  ● Connected          [Strike ⚡]    │  header: StatusIndicator + label + TierBadge
│                                      │
│  ┌────────────────────────────────┐  │
│  │          [status SVG]          │  │  status circle
│  │          Connected             │  │  15px status text
│  │             Strike             │  │  tier name
│  │  ⚠ 3 days remaining           │  │  grace warning (graceDays ≤ 3, connected)
│  │  ┌──────────────────────────┐  │  │
│  │  │        Disconnect        │  │  │  primary (Connect) / danger (Disconnect)
│  │  └──────────────────────────┘  │  │  spinner while loading
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  State           running       │  │  stats card: engine state
│  │  Heartbeat       OK / 2 failures│  │  heartbeat failures
│  │  Grace Period    7 days        │  │  remaining grace
│  └────────────────────────────────┘  │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  [ Diagnostics ] [Update 2.1..]│  │  actions; update button appears only
│  └────────────────────────────────┘  │  when update:available event fires
└──────────────────────────────────────┘
```

**Status presentation:** the connection state is carried by the status circle's
role, the word beside it, and the label — never by one channel alone. The roles
and their values are `/COLOURS.md`'s.

**Diagnostics modal:** fixed overlay with a monospace `<pre>` report
(`GetDiagnostics` — version, OS, Go, activated, connected, tier, engine state,
heartbeat stats, grace days), Copy + Close buttons.

> **Note:** the original spec included a connection timer and a live speed
> progress bar. These are **not implemented** in the current client — the stats
> card (engine state / heartbeat / grace days) takes their place. A speed
> indicator remains a future enhancement.

---

## 6. Button States

```css
.btn { padding: 12px; border-radius: 8px; font-size: 15px; font-weight: 600; }
.btn:disabled { opacity: 0.4; cursor: not-allowed; }
```

| State | Label | Style |
|-------|-------|-------|
| Disconnected | "Connect" | primary |
| Connecting | spinner replaces label | disabled |
| Connected | "Disconnect" | danger |
| Loading | CSS spinner (16px ring) | disabled |

The role names are the contract; their values are `/COLOURS.md`'s.

---

## 7. Tier Badge Styling

Implemented in `TierBadge.vue`.

| Tier | Icon | Colour |
|------|:----:|--------|
| `strike` | ⚡ | `/COLOURS.md` §6 |
| `stealth` | ◉ | *(retired tier — no longer rendered)* |
| `eco` (default) | ○ | `/COLOURS.md` §6 |

Rendered as a 12px bold pill (`border-radius: 20px`, padding 4px 10px) next to
the status label in the main-screen header.

---

## 8. System Tray

The app window is **shown on launch** — Wails v2.9 has no system tray API, so
`StartHidden` was removed (a hidden window with no tray icon would make the app
permanently invisible). Closing the window quits the app. The `tray:show` /
`tray:quit` hooks in `setupSystemTray` are dormant (no tray icon exists) — a
real tray with Show/Quit is a future enhancement.

> **Note:** the tray icon assets and the right-click tray menu described in the
> original Fyne-era spec are not yet implemented in the Wails client — only the
> close-to-tray behavior and the event hooks exist. This is a future
> enhancement.

---

## 9. Visual Testing Checklist

The original checklist asked a reviewer to confirm colours by eye. That is now
`/COLOURS.md`'s problem, not this document's. What remains checkable here:

- [ ] The activation screen renders before any other surface on a fresh install
- [ ] The status readout names the state in words, not only in colour
- [ ] The tier badge shows a plan name beside its glyph
- [ ] The window paints its background before the WebView shows content
- [ ] Diagnostics copy produces a complete report

---

## 10. When to Do This

Read this document to understand how the **retired** client was laid out. Do not
take any value from it: every colour it used to state is superseded, and the
current values are `/COLOURS.md`'s.
