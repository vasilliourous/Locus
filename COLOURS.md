# COLOURS — every colour in the project, in one place

```
audience:    builder
status:      live
authoritative-for: every colour value, every palette, and every rule that governs colour, across the client, the hub and the landing page
verified-against: client/tests/theme-colors.test.ts (values + contrast floors), server/scripts/check-consistency.sh §49 (no colour outside this file)
```

> **This is the only file in the repository allowed to contain a colour value, a
> palette, a gradient, or a present-tense statement about how something looks.**
> Every other document, code comment and spec links here and states nothing.
>
> That is not tidiness. A hex value written into a sentence cannot be recomputed,
> and this project has already paid for that class of defect: `THEMES.md` carried
> the pre-3.3.0 palette as current for a release, `FRONTEND.md` listed ten themes
> and named eight that do not exist, and `UI-AESTHETICS.md` described a green-black
> identity the client had already left behind. Each was true when written. None of
> them could fail. **A colour stated in a second place is a colour that will
> disagree with the first.**

---

## 0. The state of colour right now

**Monochrome reset, decided 2026-11-11 and not yet superseded.** Every surface
Locus owns — the app's default appearance, its tier badge, its status roles, the
landing page, the operator console, the native window's first frame, the wordmark
and the application icon — is **neutral grey**. There is no brand colour.

The reason is a product report, not a taste: clients looking at the landing page
read it as unfinished. Rather than iterate on a palette that was never agreed,
every Locus-owned colour was reduced to grey so a real colour design can start
from a neutral base instead of being tuned against a guess.

**What survived the reset, deliberately:**

- **The machinery.** Colour is data projected through one chain
  (`§1`). Nothing in that chain was removed, so re-colouring the product is a
  change to data, not to behaviour.
- **The seven themes that are not Locus's own.** `slate`, `dawn`, `ember`,
  `linen`, `sepia` and `contrast` keep the colours they were authored with, and
  so do the three decoration presets that belong to them (`blueprint`,
  `risograph`, `signal-noise`). They are alternative looks a student may pick;
  they are not the brand. **`moss` and its `forest-glow` preset are neutral now**
  because both were green — see §4.
- **The shape.** Corner radii are not colour and were not touched.

**Status of this section:** the *decision* is a fact about the tree. Whether the
result looks right **on a real screen is unverified** — this change was made
against computed contrast figures and has not been rendered on a physical display
on any of the three platforms.

---

## 1. How colour flows, and the one rule that must not be broken

There is exactly one chain, and every surface is painted by it:

```
theme_id (stored, may be empty)
   └─> resolveTheme(id)            _themes.ts   → a ThemeSpec { palette, shape, decoration }
        └─> themeDefaultFor(spec)  _themes.ts   → the legacy { primary_color, … } shape
             └─> createTheme(...)  use-custom-theme.ts → the MUI palette every component reads
```

**The rule: a future colour pass edits the palette data in `§3`–`§6` and stops
there.** It does not edit `resolveTheme`, `themeDefaultFor`, `use-custom-theme`,
or any component. Those are the mechanism; the palette is the input. A change to
the mechanism to get a colour is a change that will have to be undone to get the
next one.

`use-custom-theme` resolves each MUI field as `theme_setting.X || projected.X`, so
a student who has set a custom colour keeps it. The palettes below are **defaults,
not overrides** — the same rule the pre-reset palette shipped with.

### The three paint layers, and why there are three values for one colour

The window is painted three times before the app is themed, and none of the three
can import the others:

| Layer | Painted by | Source of truth |
|---|---|---|
| **1. The native window** | the OS, before any web content exists | `client/src-tauri/src/utils/resolve/window.rs` |
| **2. The document** | HTML parsed before any bundle runs | `client/src/index.html`, then `client/src/assets/styles/index.scss` |
| **3. The app** | the theme hook | `client/src/pages/_theme.tsx` → `_themes.ts` |

All three carry the same pair of grounds (§2). They are kept in step by
`client/tests/theme-colors.test.ts`, which reads all three files — the copies
cannot be removed (layer 2 cannot import layer 3, and layer 1 is Rust), so the
test is what keeps them honest. **Changing layer 3 alone shows the old colour for
a frame on every launch.**

---

## 2. The base palette — Locus's own appearance

Authority: `client/src/pages/_theme.tsx` (`LOCUS_COLORS`, `LOCUS_LIGHT`). The
registry's `default-dark` and `default-light` are **projections of these objects**,
not copies, so this table is the single source for both.

### Dark (`LOCUS_COLORS`) — the default a fresh install sees

| Role | Value | Notes |
|---|---|---|
| `background` | `#141414` | The window |
| `surface` | `#202020` | Cards and panels; one visible step above the window |
| `surfaceHover` | `#2A2A2A` | Interactive surfaces |
| `border` | `#4A4A4A` | Dividers, input borders |
| `textPrimary` | `#E8E8E8` | Body text |
| `textSecondary` | `#A6A6A6` | Labels and hints |
| `accent` | `#C4C4C4` | Buttons, active indicators, **and the wordmark** |
| `accentHover` | `#DEDEDE` | |
| `success` | `#E8E8E8` | Connected |
| `error` | `#D6D6D6` | Disconnected, failures |
| `warning` | `#C8C8C8` | Connecting, degraded, renewal warnings |

### Light (`LOCUS_LIGHT`)

| Role | Value |
|---|---|
| `background` | `#ECECEC` |
| `surface` | `#F7F7F7` |
| `surfaceHover` | `#E6E6E6` |
| `border` | `#C0C0C0` |
| `accent` | `#4C4C4C` |
| `accentHover` | `#404040` |

The light palette has no status colours of its own; the light projections in §3
supply them.

### The legacy shape, and the six status roles

Authority: the same file, `defaultTheme` (light) and `defaultDarkTheme` (dark).
These are the MUI fields the chain reads; every one is overridden by a student's
own setting when they have one.

| Field | Light | Dark | Role |
|---|---|---|---|
| `primary_color` | `#4C4C4C` | `#C4C4C4` | the accent |
| `secondary_color` | `#565656` | `#A6A6A6` | non-primary emphasis |
| `primary_text` | `#151515` | `#E8E8E8` | body |
| `secondary_text` | `#565656` | `#A6A6A6` | labels |
| `info_color` | `#565656` | `#D0D0D0` | informational |
| `error_color` | `#151515` | `#D6D6D6` | failure |
| `warning_color` | `#151515` | `#C8C8C8` | degradation |
| `success_color` | `#151515` | `#E8E8E8` | connected |
| `background_color` | `#ECECEC` | `#141414` | the window |

**Status colour in a monochrome product is carried by position, weight and
colour, not by hue alone** — the same redundancy rule the tier badge follows
(`§6`). The three status roles are deliberately separable in value so a warning
is still distinct from a success without colour.

---

## 3. The registry — nine themes

Authority: `client/src/pages/_themes.ts` (`THEMES`). The list of ids is data in
`docs/state.toml [client.theme_ids]`, recomputed from this registry by
`check-consistency.sh` §9. Dropdown order is registry order.

One mode per theme: a theme carries its own light/dark, and selecting one
supersedes `theme_mode` until it is cleared.

### Themes that are projections of `§2` — neutral

| id | Mode | Source |
|---|---|---|
| `default-dark` | dark | `LOCUS_COLORS` |
| `default-light` | light | `LOCUS_LIGHT` + the light columns of `§2` |

### Themes that keep their authored colour

Bodies from `client/src/pages/_themes.ts`; each is a full self-contained palette
with the fields `background, surface, surfaceHover, border, textPrimary,
textSecondary, accent, accentHover, success, warning, error, secondary, info`.

| id | Mode | Palette | Decoration | Shape |
|---|---|---|---|---|
| `slate` | dark | `#1E222A` · `#272C36` · `#323845` · `#3E4553` · `#D8DEE9` · `#A9B4C4` · `#88C0D0` · `#A3D4E2` · `#A3BE8C` · `#EBCB8B` · `#D08770` · `#81A1C1` · `#5E9FD8` | `blueprint` | default |
| `dawn` | dark | `#1A1B26` · `#24283B` · `#2F3349` · `#3B4261` · `#C0CAF5` · `#9AA5CE` · `#7AA2F7` · `#9AB8FF` · `#9ECE6A` · `#E0AF68` · `#F7768E` · `#BB9AF7` · `#7DCFFF` | `signal-noise` | default |
| `ember` | dark | `#211A17` · `#2E2521` · `#3B302A` · `#4E4038` · `#EDE0D8` · `#C0ABA0` · `#E0895F` · `#F0A17C` · `#9CB380` · `#E0A85F` · `#E0706B` · `#C98F9E` · `#8FB0C9` | `ember-pit` | default |
| `linen` | light | `#EAEEF2` · `#F4F7FA` · `#DFE5EB` · `#C0CAD4` · `#1E2733` · `#55636F` · `#2F6F9F` · `#255A82` · `#3F6C45` · `#8A5A00` · `#A33226` · `#5B6E85` · `#255E8A` | — | default |
| `sepia` | light | `#EDE4D3` · `#F7F0E4` · `#E2D6C0` · `#C9B79A` · `#2B2118` · `#5C4C3B` · `#8A5A2B` · `#6E4622` · `#3F6C45` · `#8A5A00` · `#A33226` · `#6B5641` · `#31556F` | `risograph` | default |
| `contrast` | dark | `#14171A` · `#242A30` · `#2E353C` · `#5A646E` · `#F0F3F5` · `#C2CAD2` · `#6FD08C` · `#8FDFA6` · `#6FD08C` · `#E8C46A` · `#F08A8A` · `#9AB4CC` · `#8FC4E8` | — | default |

The field order in that table is the object's own order: **background, surface,
surfaceHover, border, textPrimary, textSecondary, accent, accentHover, success,
warning, error, secondary, info.** Read a row against the header, not by
guessing a column.

### The theme that was neutralised

| id | Mode | Palette | Decoration | Shape |
|---|---|---|---|---|
| `moss` | dark | `#1E1E1E` · `#2A2A2A` · `#333333` · `#3D3D3D` · `#E6E6E6` · `#ABABAB` · `#C8C8C8` · `#DCDCDC` · `#C8C8C8` · `#B4B4B4` · `#A6A6A6` · `#ABABAB` · `#B4B4B4` | `forest-glow` | card 14 / control 8 |

`moss` keeps its identity as the theme with a larger card radius; what it lost is
the green. Its palette was literal greens in the registry rather than a projection
of `§2`, which is why it needed its own row here.

---

## 4. Decorations

Authority: `client/src/pages/_themes.ts` (`DECORATIONS`). A preset is a
**structure** — `short` declarations plus ordered `layers` — never free-form CSS,
scoped to `[data-theme-skin]` and injected into one `<style>` element by
`use-custom-theme`. Removing the element removes every trace of the layer.

| Preset | Used by | What it paints | State |
|---|---|---|---|
| `forest-glow` | `moss` | a soft radial wash from the top edge | **neutral** — `rgba(153,153,153,0.10)` → transparent |
| `ember-pit` | `ember` | a radial wash from below, 112% down | as authored |
| `blueprint` | `slate` | a 32px repeating grid plus a cold wash | as authored |
| `risograph` | `sepia` | two offset flat tints | as authored |
| `signal-noise` | `dawn` | a 1px scanline on a 4px period under a phosphor glow | as authored |

**Compositor-only is a rule, not a checked property.** `background-*`,
`box-shadow`, `border-*`, `opacity`, `filter`, `mix-blend-mode` and custom-property
declarations are permitted because none can change layout. **Nothing in a preset
may set `width`, `height`, `margin`, `padding`, `position`, `display`, `flex`,
`grid`, `gap`, `order` or `font-size`** — a decoration that can resize an element
can move the Connect control, which is the one thing this app must not do. No test
can tell `box-shadow` from `margin-top`, so this is the first thing to check in a
theme diff.

A preset's safety properties *are* checked (`client/tests/theme-colors.test.ts`):
braces balance, no construct introduces a document-level rule, and no preset can
load a remote or inline asset.

---

## 5. The landing page

Authority: `server/site/style.css` `:root`, and its `prefers-color-scheme: dark`
block. Served statically by Caddy with no build step, so it is a **by-value copy of
§2** rather than an import, and it is not enforced by the test suite — it is
enforced by whoever changes §2 remembering this section exists.

| Variable | Light | Dark |
|---|---|---|
| `--bg` | `#ECECEC` | `#141414` |
| `--surface` | `#F7F7F7` | `#202020` |
| `--surface-hover` | `#E6E6E6` | `#2A2A2A` |
| `--border` | `#C0C0C0` | `#4A4A4A` |
| `--text` | `#151515` | `#E8E8E8` |
| `--text-secondary` | `#565656` | `#A6A6A6` |
| `--accent` | `#4C4C4C` | `#D0D0D0` |
| `--accent-hover` | `#404040` | `#E2E2E2` |

The four download buttons, the canonical URL and the platform keys on this page
are **not** colour and are owned elsewhere (`docs/operate/CLAIMS.md`,
`deploy-site.sh`, `check-consistency.sh` §26).

---

## 6. The tier badge

Authority: `client/src/pages/_theme.tsx` (`TIER_COLORS`, `TIER_FALLBACK`), rendered
by `client/src/components/connection/tier-badge.tsx`.

Colour is **redundant** here by design: the badge carries a glyph and a label, and
the palette only reinforces them. That is what made the monochrome reset free —
nothing is lost that the badge was relying on.

| Wire name | Shown as | Text | Background |
|---|---|---|---|
| `strike` | Full | `#FFFFFF` | `rgba(255,255,255,0.20)` |
| `budget` | Budget | `#F0F0F0` | `rgba(240,240,240,0.19)` |
| `free` | Free | `#DADADA` | `rgba(218,218,218,0.18)` |
| `eco` | Free | `#DADADA` | `rgba(218,218,218,0.18)` |
| *unknown* | the raw name | `#BDBDBD` | `rgba(189,189,189,0.18)` |

The three rungs step in value rather than in hue, because the badge is
monochrome: Full is the brightest (`#FFFFFF`), Budget sits between it and Free
(`#F0F0F0`, the midpoint of the two neighbours), and Free is dimmest. Budget's
background alpha is its text alpha minus the same `0.01` the other paid rung uses,
so the two paid plans composite alike and Free stays visibly the dimmest.

`strike` and `eco` are **frozen wire names** — a code in the field carries that
string, and the hub resolves it against `tier_configs`. The key is the contract;
the label and the colour are not. Renaming a key strands every paid code. The
same is true of the newer `budget` and `free` rows: a row's string is what a
minted code carries forever, so a colour may change but the key may not.

Contrast is provided by the badge's own surface, not by these values alone: they
are composited over `§2 surface`. An unknown tier renders rather than vanishing,
because a plan a student is paying for must never be invisible.

---

## 7. The operator console

Authority: `server/console/src/styles.css` `:root`, plus a handful of literal
values in the same file that are not variables.

| Variable | Value |
|---|---|
| `--bg` | `#141414` |
| `--panel` | `#1C1C1C` |
| `--panel-2` | `#222222` |
| `--border` | `#3A3A3A` |
| `--text` | `#E8E8E8` |
| `--muted` | `#A0A0A0` |
| `--accent` | `#C4C4C4` |
| `--accent-dim` | `#4A4A4A` |
| `--danger` | `#D6D6D6` |
| `--warn` | `#C8C8C8` |

Non-variable literals in the same file, all neutral: the active nav label
`#FFFFFF`, the primary button's label `#141414`, the danger button's border
`var(--accent-dim)`, the three message backgrounds `#222222` (ok) /
`#1C1C1C` (err) / `#262626` (warn) distinguished by their borders, and the four
badge backgrounds `#222222` (available) / `#1C1C1C` (bound) / `#1C1C1C`
(suspended) / `#262626` (expired). Modal and toast shadows are black at alpha,
unchanged.

Not colour, and not owned here: the console's routes, its auth, and the
`/admin/` path.

---

## 8. Marks: the wordmark, the shield, the app icon, the tray

| Mark | Source | Colour |
|---|---|---|
| Wordmark | `client/src/assets/image/logo.svg` | `fill: currentColor` |
| Shield (dark / light pair) | `client/src/assets/image/icon_dark.svg`, `icon_light.svg` | `fill: currentColor` |
| Nav glyphs | `client/src/assets/image/itemicon/*.svg` | `fill: currentColor` |
| Application icon | `client/src-tauri/icons/*` (PNG, ICO, ICNS, `Assets.car`) | flat neutral mark on white |
| Tray icons | `client/src-tauri/icons/tray-icon*.ico` | unchanged — they are status indicators, not brand |
| Landing-page logo | `server/site/logo.svg` | `fill: currentColor`; `server/site/icon.svg` likewise |

The in-app marks resolve `currentColor` from their parent, which
`layout-sidebar.tsx` sets to `theme.palette.primary.main`. So **the wordmark and
shield follow the accent of whichever theme is selected, including every theme in
§3 that kept its colour.** There is one asset per mark, never one per theme: a
per-theme asset is a file that gets forgotten when a theme is added.

**The tray icons were left alone**, deliberately: they encode connection state,
not brand. Every other mark follows the theme, which is why a student on `slate`
sees a blue-grey wordmark and a student on `default-dark` sees a grey one — that
is the documented behaviour, not an inconsistency.

---

## 9. Contrast rules every palette must satisfy

These are not preferences. Each is asserted per theme by
`client/tests/theme-colors.test.ts`, so a new palette cannot ship unreadable.

| Rule | Floor | Why |
|---|---|---|
| `textPrimary` on its own surface and on the page | **4.5:1** | body copy |
| `textSecondary` on its surface | **3:1** | labels and hints |
| `accent` on surface and on page | **3:1** | it paints the wordmark and button labels; 3:1 is the floor for a non-text UI mark, and the wordmark is the largest use of the accent |
| surface against page | **1.03:1** | a card you cannot see does no work; the 1px border should reinforce separation, not carry it |
| border against surface | **1.15:1** | |
| no pure white, no pure black | — | pure white on a full-window panel is a light source, not a surface; pure black crushes the ladder |

Two rules that a contrast figure cannot express:

- **A mode must match its palette direction** — a dark theme carries light text.
  This is the check that catches a copy-paste between two similar themes.
- **An accent is a text-bearing colour, not a decoration.** If it is only legible
  as a fill, it is not usable as the accent.

---

## 10. Where colour may not be written, and the guard that enforces it

**Forbidden, everywhere except this file:**

- a colour literal of any form — `#RGB`, `#RRGGBB`, `rgb()`, `rgba()`, `hsl()`,
  or a named CSS colour used as a value;
- a gradient or a shadow described as a look;
- a present-tense claim about how any surface looks ("the window is green", "the
  accent is gold").

**Permitted:** a link to this file. That is the whole allowance, and it is what a
comment should say instead of the value.

**One exception, and it is not a second home: the palette source files.** A
palette has to be expressible in code, so `§2`–`§8` name the module that holds
each one and those modules hold the literal. That is the *implementation* of the
value this file describes, and the pair is guarded in both directions — the doc
names the file, and `client/tests/theme-colors.test.ts` asserts the file's values
are what §0–§9 say. No other code file, comment, document or spec may hold one.
The guard's allowlist is exactly: this file, and the palette modules §2–§8 name.

**The guard:** `server/scripts/check-consistency.sh` §49 fails when a live
document outside this file carries a colour literal or a colour-looking claim, and
`client/tests/theme-colors.test.ts` fails when a Locus-owned palette carries
chroma. Both were shown failing against the tree they police before being kept.

**What the guard does not cover, stated so nobody reads it as coverage:** a
component that hardcodes a colour outside the palette, and a *shape* decision
(radius, weight, spacing) that changes how a surface reads. The first is caught
only where the existing component scan looks; the second is not colour at all and
this file has nothing to say about it.

---

## 11. Superseded, and where the old values went

The pre-reset palette, the original green-black specification and the retired
client's colour table are **not reproduced anywhere**. Three documents used to
carry them and no longer do, because each had drifted or would:

| Document | What it carried | Now |
|---|---|---|
| `docs/reference/THEMES.md` | the palette tables, per-theme rationale, decoration descriptions | this file; that document keeps the *layer's* mechanics only |
| `client/docs/FRONTEND.md` §5 | the brand accent and a theme list that was stale by eight names | a pointer to this file |
| `docs/archive/UI-AESTHETICS.md` | the retired client's design spec, including the palette `THEMES.md` cited as its source | a retraction; the spec it described was never re-derived after the values changed, so no current value depended on it |

`legacy/` contains a retired client whose own files carry colours. It is
**reference only, never built**, and is not governed by this document — the guard
skips it for that reason.

**The one thing a reader should take from this section:** if you find a colour in
an old document, it is a survivor of a purge, not an authority. This file is the
authority, and `§10`'s guard exists so the next one does not survive.
