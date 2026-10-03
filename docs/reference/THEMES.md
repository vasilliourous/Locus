# Themes — the decorative skin layer

```
audience:    builder
status:      live
authoritative-for: the theme registry, what a theme may change, and how a selection resolves
verified-against: docs/state.toml
```

> **Read this before adding a theme, or before changing anything in
> `client/src/pages/_themes.ts`.** The registry is a data table with a guard
> behind it (`check-consistency.sh` §9 recomputes the id list from the source).
> A theme added by editing only the registry, or only `state.toml`, fails the
> build on purpose.

---

## 1. What this layer is, and what it is not

Locus shipped two appearances: **Light** and **Dark**, chosen by a three-way
`theme_mode` (`system` / `light` / `dark`). This document describes the layer
added on top of that: a set of **named themes** a student picks from a dropdown,
each of which is a complete look.

The design constraint that shapes everything here is that this layer **sits on
top** of the shipped appearance and must not restructure it:

| The theme layer MAY change | The theme layer MUST NOT change |
|---|---|
| The palette (accent, surfaces, borders, text, status colours) | The two-tab shape, or anything in `_navigation-meta.ts` |
| Corner radii, via two CSS variables | Tier identity (`TIER_COLORS`) — see §3 |
| A **multi-layer decoration** from a named preset, scoped to `[data-theme-skin]` | Any component's structure, props or behaviour |
| Anything a decoration can paint **without changing layout** | Whether a state is shown, or what it says |

It is deliberately not a redesign. The Connection screen's layout, its state
machine and its message priority (`client/docs/FRONTEND.md` §§2–4) are
untouched by a theme, and a theme that changes what a student reads is a bug in
the theme, not a feature.

### What changed in 3.3.0, and why

Before 3.3.0 a decoration was a **single flat string**, and the guard enforcing
that rejected any `{` or `}` in a preset. That was true to the original design —
a preset was one `radial-gradient` — but it could not express a *rice*, which is
inherently layered: a base wash, a directional treatment, a texture pass and a
vignette, composited in a defined order.

A preset is now a `DecorationSpec` — `short` (root declarations) plus an ordered
`layers` array. The safety properties are unchanged; the **enforcement** is now
direct rather than a proxy. The old check banned braces, which:

- **permitted** a stray unbalanced `}` in a preset, which closes the scoped block
  early and leaves the rest of the text loose at the top level of the injected
  `<style>` — a real escape the old guard did not detect; and
- **rejected** balanced nested blocks, layered gradients and `@supports`, none of
  which can escape anything.

`theme-colors.test.ts` now asserts the properties that actually matter:
every brace balances, no construct introduces a document-level rule, no preset
can load a remote or inline asset, and no preset can leave the skin scope. Each
of those assertions was demonstrated failing before it was kept — including the
early-close case above, which the previous guard could not have caught.

### The rule that replaced "no braces"

The constraint that has not changed, and which a test **cannot** enforce, is that
every layer stays **compositor-only**. `background-*`, `box-shadow`, `border-*`,
`opacity`, `filter`, `mix-blend-mode` and custom-property declarations are all
fine, because none can change layout. Nothing in a preset may set `width`,
`height`, `margin`, `padding`, `position`, `display`, `flex`, `grid`, `gap`,
`order` or `font-size` on anything. A decoration that could resize or move an
element is a decoration that can move the Connect button, which is the one thing
this app must not do.

That is a **review obligation**, and §10 records it as an unguarded gap rather
than claiming the suite covers it.

### Why decoration is a named preset and not injected CSS

The fork already carries a `theme_setting.css_injection` field, and the
temptation is to reuse it as free-form CSS per theme. **Do not.** A theme that
can emit arbitrary CSS can restyle any component, which is exactly the
"interference" property this layer exists to avoid, and it makes a theme change
unreviewable — a diff that can contain anything.

Instead a theme names a **decoration preset** from a closed set defined in
`_themes.ts`. Adding a new decoration is a code change with a name, an owner and
a review; selecting one is data. The existing `css_injection` field keeps working
exactly as it does today for an advanced user — it is not a theme mechanism.

---

## 2. The resolution chain

One function decides what a student sees. It is `resolveTheme()` in
`client/src/pages/_themes.ts`, and the order is not negotiable:

```
1. the selected theme's palette        (verge.theme_id -> THEMES[id])
2. legacy per-field colour overrides   (theme_setting.primary_color, ...)
3. the legacy light/dark choice        (theme_mode, only when no theme is set)
4. the built-in default theme          (default-dark)
```

Read as: **a selected theme is the base, and the legacy colour fields are an
override of it.** Every field keeps the precedence it has today —
`setting.X || spec.X`, where today's `spec` was the fixed `defaultTheme` /
`defaultDarkTheme` and is now the resolved theme. This is the least invasive
form of the change: no stored value is reinterpreted, no user's custom colour
stops applying, and the change is a *substitution of the base*, not a new
precedence rule.

### `theme_id` absent means today's behaviour, exactly

A device with no `theme_id` set resolves through `theme_mode` as it always has.
This is what makes the layer additive: with the field unset — which is every
existing install — the app behaves byte-identically to the version before this
document existed. `default-light` and `default-dark` are byte-identical to
today's `LOCUS_LIGHT` and `LOCUS_COLORS`, and `theme-colors.test.ts` pins that.

### An unknown or missing id never errors

An id that is absent, empty, or not in the registry resolves to `default-dark`.
This is the same rule the subscription union follows (`FRONTEND.md` §4: never
invent a state) — a theme id is a decoration, and a decoration that cannot be
read degrades to the default rather than blanking the screen or throwing.

### One mode per theme

**A theme carries its own light/dark mode.** There is deliberately no
theme-plus-mode matrix: selecting `Midnight` gives a dark appearance whatever
`theme_mode` says, because the theme *is* the mode.

The consequence, stated so nobody later reports it as a bug: **a student cannot
have a light variant of a dark-only theme.** Nine themes are nine palettes, not
nine palettes with two variants each. This was a deliberate simplification — it
keeps the registry readable and the contrast checking tractable — and if a
theme is wanted in both modes it is added as two entries with two names.

Because a theme carries its mode, **selecting a theme supersedes `theme_mode`
for as long as it is set.** The existing Appearance control on the Account
screen still reads and writes `theme_mode` and still governs behaviour when no
theme is selected; the theme dropdown is the control that wins when it is used.
The two are not merged in this pass — see §7.

---

## 3. What every theme must satisfy

Four rules, each enforced by a test in `client/tests/theme-colors.test.ts`:

1. **Contrast.** Text on its own surface and the accent used as text on the
   theme's background must clear the floors in §4. The floors are per-theme mode,
   because `#2EA86A` on white is ~3.0:1 — fine for large text and UI shapes, short
   of the 4.5:1 body-copy rule — while the same green on `#06130C` is comfortable.
   A light theme therefore needs a *darker* accent, and that is a property of the
   theme, not a special case in the code.
2. **A theme's `mode` must match its palette direction.** A theme declaring
   `mode: 'dark'` must have light text on a dark surface. This is the check that
   catches a copy-paste error between two similar themes, which is the realistic
   way a registry entry is authored wrong.
3. **Structural relationships hold.** `surface` is a visible step from
   `background`; `border` is visible against both; `textSecondary` is dimmer than
   `textPrimary` but still readable. These are the relationships the shipped
   palette was built on, and a theme that violates one looks broken in a way
   contrast maths alone will not catch.
4. **Tier colours are not themed.** `TIER_COLORS` and `TIER_FALLBACK` are
   **brand identity, not decoration** — the badge "sells itself"
   (`client/docs/FRONTEND.md` §5), and a theme that repainted `strike` gold into
   something else would be making a claim about a product tier. They stay fixed
   across every theme. If a tier colour is ever themed, that is a product
   decision, and it is not this layer's.

---

## 4. The nine themes

Every palette is designed against the rules in §3, and each is described by its
**colour scheme** rather than only its colour names, because the scheme is the
part that is checkable and the part a future theme should be derived from.

| Theme | Mode | Scheme | Base hue | Mood |
|---|---|---|---|---|
| `default-dark` (Locus) | dark | analogous | h~158 green | the brand |
| `default-light` | light | analogous | h~150 green | the brand, inverted |
| `slate` | dark | split-complementary | h~220 blue-grey | calm, recessive |
| `dawn` | dark | analogous + complements | h~230 indigo | vivid, nocturnal |
| `ember` | dark | complementary | h~25 terracotta | warm, energetic |
| `moss` | dark | analogous | h~145 green | restful, organic |
| `linen` | light | analogous | h~210 neutral-cool | clean, clinical |
| `sepia` | light | analogous | h~35 paper | calm, analogue |
| `contrast` | dark | neutral ramp | — | maximum legibility |

### 4.1 `default-dark` — Locus · *default*

The brand appearance: a dark slate with a green cast (`#0D1512`) rather than a
near-black, Locus green as the single accent, and a card a deliberate step above
the page. It is the default because it is the mode the brand was designed in.

*Must not be confused with:* `moss`, which is the same hue family with the
saturation and the surface step pulled down. The difference is energy, not hue.

### 4.2 `default-light` — Locus (light)

The same structure inverted, with the darker accent (`#2A6E4C`) that the
body-copy contrast rule requires on a light ground.

### 4.3 `slate` — cool blue-grey

**Split-complementary** on a h~220 base. The frost-blue accent sits a
quarter-turn from the surfaces, and the status colours swing warm so a warning
is never mistaken for a link. Every colour is low-saturation and close in value,
so nothing competes with the Connect control.

*Why it exists:* it is the theme for someone who wants the app to stop shouting.

### 4.4 `dawn` — indigo night

**Analogous** on an indigo base (h~230) with a blue accent and warm complements
(peach, coral, violet). The highest-chroma dark theme in the registry, with a
phosphor glow and a fine scanline (`signal-noise`).

*Why it exists:* it is the vibrant alternative — a screen in a dark room that is
alive rather than greyed out.

### 4.5 `ember` — warm terracotta

**Complementary** on a h~25 base, with the accent at full strength and the cool
counterweight carried by `info` rather than by the accent — which is what keeps a
warm theme from turning muddy. Lit from below (`ember-pit`), so it reads as a
fire rather than as the conventional top-down hero gradient.

*Why it exists:* warm light is easier on the eye at night than a blue-white
screen.

### 4.6 `moss` — desaturated green

**Analogous** on a green base (h~145), close to the brand in hue and a long way
from it in saturation, with the largest corner radius in the registry and a soft
green wash (`forest-glow`).

*Why it exists:* it is the default's character made softer — for someone who
likes the brand green but wants it quiet.

### 4.7 `linen` — cool neutral light

**Analogous** on a neutral-cool base (h~210): near-monochrome surfaces with a
single blue accent, so the one saturated thing on screen is always interactive.
The off-white ground is `#F4F7FA` (L=0.93), not white.

### 4.8 `sepia` — warm paper

**Analogous** on a h~35 paper base with a burnt-orange accent, warm near-black
text, and two offset flat tints (`risograph`) reading as over-inked plates.

*Why it exists:* for a bright room or beside a window, where a cool white screen
competes with daylight.

### 4.9 `contrast` — maximum legibility

A **neutral ramp** with a single green accent — deliberately not a colour scheme,
because this theme's job is legibility and hue variety would work against it.
What separates it is *range*: ~13:1 text on the surface, 2.40:1 borders, and the
largest surface step in the registry.

*It replaces pure-white-on-pure-black.* That combination met the contrast numbers
by every measure and was still the wrong answer: `#FFFFFF` on `#000000` is the
highest-glare pairing possible, and for the people who need this theme most —
astigmatism, light sensitivity — glare is the symptom, not the fix.

---

## 5. The three paint layers, for nine themes

`client/docs/FRONTEND.md` §5 describes three places the window background colour
exists and cannot share code:

1. the **native window** (`src-tauri/src/utils/resolve/window.rs`)
2. the **document** (`client/src/index.html`)
3. the **app** (`_theme.tsx` via `use-custom-theme`)

Layers 1 and 2 are painted **before any bundle runs**, so they cannot know which
theme is selected by reading the registry — and they must still paint something
that is not a visible flash of the wrong colour.

**The decision: layers 1 and 2 stay pinned to the two legacy backgrounds
(`default-dark` and `default-light`) and are not extended per theme.**

The consequence, stated plainly because it is a real trade-off and not an
oversight: **cold-start paints `default-dark` or `default-light`.** A student
using `midnight` sees, for the brief window before the bundle runs, the
green-black default rather than true black. This is a **transition between two
dark colours**, which is not perceptible in the way the upstream-grey-to-Locus-
green flash was, and it costs nothing.

The alternative — teaching the native window and the document about nine themes —
means threading the selected theme through the Rust window resolver and a
pre-bundle script, adding a second place a theme id can be wrong, to remove an
imperceptible flash. That is not a trade worth making here. If a future theme has
a *light* background and the cold-start default is dark, this decision must be
revisited; `paper` is light, and it is the theme to test that against.

---

## 6. Adding a theme

In this order. Skipping a step fails a guard rather than producing a silent
inconsistency — which is the whole point of the ordering.

1. **Add the entry** to `THEMES` in `client/src/pages/_themes.ts`, with `id`,
   `labelKey`, `mode`, `palette`, `shape` and (optionally) `decoration`.
2. **Add the label key** to `client/src/locales/en/home.json` beside the existing
   `theme*` keys, then regenerate the i18n key list. Other locales fall back
   rather than being machine-translated.
3. **Add the section to this document** — intent, what it must not be confused
   with, and any deliberate trade-off. A theme with no rationale is a palette
   nobody can safely change later.
4. **Update `[client.theme_ids]`** in `docs/state.toml`. The registry is the
   authority and that list must agree with it, so run
   **`bash server/scripts/check-consistency.sh`** — §9 recomputes the id list from
   the registry and fails on disagreement in **either** direction. Under `sh` or
   `dash` the script aborts with "Bad substitution" and prints false `BAD` lines,
   so run it with `bash`.
5. **Run `pnpm test`** in `client/`. The suite checks §3's four rules against the
   new entry, and a new theme is the most likely thing to fail the contrast or
   mode-direction checks.
6. **Show the new test failing first** if you added one. A guard that cannot fail
   reads exactly like a guard that passed (`docs/reference/DEBUGGING-METHOD.md`).

Adding a **decoration preset** is a separate, smaller change: a named block in
the preset table, used by at least one theme, scoped to the skin attribute so it
cannot leak into components.

---

## 7. What is deliberately not in this pass

| Not done | Why |
|---|---|
| Merging the theme dropdown into the `theme_mode` control | The existing Appearance control is shipped, tested and correctly separated (`useThemeMode()` is the *resolved* value; `theme_mode` is the *stored* one — see its comment in `account.tsx`). Merging them is a UI change with its own migration question, and it is not needed for themes to work. |
| Per-theme cold-start colours in the native window | §5 — an imperceptible flash is not worth a second source of truth for theme ids. |
| Themeable tier colours | §3 rule 4 — brand identity, not decoration. |
| Light/dark variants of each theme | §2 — one mode per theme, by decision. |
| Free-form CSS per theme | §1 — a named preset keeps a theme change reviewable. |
| Shipping a theme over the hub | Themes are client-side data. Nothing about a theme reaches the hub, and no theme can change what the client reports about itself. |

---

## 7a. The logo follows the theme

Both marks in the sidebar's top-left corner — the square shield
(`icon_dark`/`icon_light.svg`) and the wordmark (`logo.svg`) — are painted in
`palette.accent`.

**There is one asset per mark, not one per theme.** The SVGs were converted from
hardcoded fills to `fill: currentColor`, and `layout-sidebar.tsx` sets `color` to
`theme.palette.primary.main` on the wrapper. "Recolour the logo per theme" is
then a *derived* consequence of the palette rather than nine duplicated files —
a per-theme asset is a file that can be forgotten when a theme is added, and the
wordmark's own history is the argument: it shipped with
`fill={isDark ? 'white' : 'black'}` for its whole life, which the SVG's internal
`.st1{fill:#15803D}` class overrode anyway, so it never changed at all.

The pairing of the two shield files is now about **glyph weight, not colour**:
the `_dark` variant is the solid mark and `_light` the outlined one, chosen by
the mode so the mark reads against its background. The colour comes from the
theme in both cases.

The accent is safe as a mark by construction: the registry suite asserts every
theme's accent clears **3:1** against both its page and its surface, which is the
WCAG floor for a non-text UI component. See §10 gap 8 for what that does *not*
cover.

---

## 8. Verifying it

```sh
cd client && pnpm test          # registry suite: contrast, mode direction, relationships
cd client && pnpm run typecheck # the registry's shape is typed
bash server/scripts/check-consistency.sh   # §9: registry ids vs docs/state.toml
```

`pnpm test` is also run by the `verify` job in `.github/workflows/client.yml`,
because the registry suite passes locally in seconds and previously nothing in CI
invoked the frontend suite at all.

**What a passing suite does not verify:** that a theme *looks* good, that its
decoration renders as intended on every platform, or that the cold-start
transition of §5 is imperceptible to a real student on a real machine. Those are
judgement calls and visual checks, and this document does not claim them.

---

## 9. Debugging a theme problem

Written as symptoms, because that is how a report arrives. Each names the *one*
place to look, and the check that distinguishes the candidate causes.

### "I picked a theme and nothing changed"

Inspect the resolved attributes on the document root, in the app's devtools
console:

```js
document.documentElement.getAttribute('data-theme-id')
getComputedStyle(document.documentElement).getPropertyValue('--card-radius')
```

- **Attribute missing entirely** → the decoration effect (§1 of
  `use-custom-theme.ts`) never ran, so the hook is not mounted. Check the theme
  resolved at all: it runs unconditionally, theme or no theme.
- **Attribute present but the wrong id** → the write path. `theme_id` is persisted
  through `patchVerge`; if the value did not round-trip, `resolveTheme` fell back
  to `default-dark`, which is exactly what a *missing* id looks like. Confirm by
  reading `verge.theme_id` back.
- **Attribute correct, colours unchanged** → the resolved theme is right and the
  palette is not reaching MUI. The likely cause is a component reading a hardcoded
  colour instead of the theme; `theme-colors.test.ts` has a test for exactly that
  ("no component hardcodes an upstream surface colour").

### "The app flashes the wrong colour on startup, then fixes itself"

Expected, within limits — see §5. The native window and the document are pinned to
`default-dark`/`default-light`, so **every theme starts as one of those two**. A
flash *between* the two defaults is a real bug (the three-layer pin test should
catch it). A flash from the default dark into `midnight` or `forest` is the
documented cold-start transition, and is a dark-to-dark change.

If the flash is from a *light* theme, §5's decision has been invalidated — that is
the case the record says must be revisited.

### "The card corners did not change, but the colours did"

`--card-radius` is written by the decoration effect, and `cardSx` reads it *with a
fallback* (`var(--card-radius, 12px)`). If the variable is unset the card silently
renders 12px. Check the computed value above, then check that the component uses
`cardSx` rather than its own `borderRadius` — a hand-rolled `sx` is how this
fork previously ended up with two different visual languages.

### "A theme I selected is not in the dropdown"

The dropdown renders `THEME_IDS`, which is `Object.keys(THEMES)`. A theme missing
from the list is missing from the registry — and because `state.toml` is checked
against the registry rather than the reverse, a half-added theme fails
`check-consistency.sh` §9 rather than shipping silently.

### "`check-consistency.sh` says `BAD client.theme_ids`"

The registry and `docs/state.toml` disagree, in either direction. The **registry is
the authority** — update `state.toml` to match it, not the other way around.
Ordering is part of the value: it is the order the dropdown renders.

### "The theme survived an upgrade but `theme_mode` seems ignored"

Not a bug — §2. A theme carries its own mode and supersedes `theme_mode` while one
is selected. To return to `theme_mode` behaviour, the field must be *cleared*, not
set to a light/dark value. There is currently **no UI control that clears it**;
see §10.

### The change that will break this layer

Adding a theme is safe and guarded (§6). The things that are *not* guarded, and
should be treated as review-blocking:

- **Making a theme reach past the skin attribute.** A preset's layers are each
  scoped to `[data-theme-skin]` by `use-custom-theme`, and the test asserts that
  every brace balances, that no layer introduces a document-level rule
  (`@import`, `@charset`, `@namespace`, `@font-face`) and that no layer can load
  a remote or inline asset. What it **cannot** stop is someone replacing that
  scoping with a raw stylesheet injection, or adding a layout-affecting
  declaration (see §1's compositor-only rule) — both are review-blocking.
- **A decoration that changes layout.** The tests check the *shape* of a preset,
  not which properties it sets. `width`, `margin`, `position` and friends would
  pass every assertion in the suite and could move a control the student has to
  press. This is the sharpest unguarded edge the layer has; it is listed in §10.
- **Deriving the accent's role from `mode` instead of from the theme.** The
  shipped light accent is darker *because it is used as text*. A future theme that
  copies the dark accent into a light palette will pass nothing — the contrast
  test will catch it — but a *new* consumer of `palette.accent` that assumes dark
  surfaces will not be caught.
- **Letting `default-dark`/`default-light` drift from `_theme.tsx`.** The
  byte-identity test is the only thing holding the "superset, not replacement"
  property. Relaxing that test converts an additive change into a visual change
  for every existing install.

---

## 10. Known gaps, and one that needs a product decision

Recorded here rather than left in a comment, because a gap that lives only in code
is a gap the next agent rediscovers. None of these has a guard.

1. **No control clears `theme_id`.** The dropdown only ever writes a value, and
   every value is a theme — so once a theme is chosen, `theme_mode` (and with it
   "follow the system") is unreachable from the UI. This is the sharpest edge of
   the one-mode-per-theme decision in §2. Clearing it needs a UI decision, not a
   code change: an explicit "Default (follow system)" entry is the obvious shape,
   but it changes what the dropdown means for an existing install, so it is not
   being made silently here.
2. **The two Appearance controls can disagree in appearance.** The theme dropdown
   and the `theme_mode` dropdown are separate and both visible on Account. When a
   theme is set, the mode control shows a value the app is not honouring. §7 defers
   merging them; the point where that becomes confusing to a student is the point
   to do it.
3. **A decoration designed for one mode can be named by the other.** `forest-glow`
   is a translucent wash designed against a dark background; `risograph` is
   designed against a light one. Nothing prevents a light theme from naming a
   dark-designed preset, and no test would object — the suite checks the preset's
   *shape* and its contrast floors, not whether the preset's own alpha suits the
   palette naming it.
4. **A decoration preset cannot be verified without eyes.** Every preset in the
   registry uses compositor-only techniques chosen because they need no asset and
   cannot shift layout. That they *look* right on four platforms is unverified and
   is stated as such in §8. The layered preset (`signal-noise`) was not rendered
   on a real screen during this change — its **validity** is asserted, its
   **appearance** is not.
5. **Compositor-only is a review rule, not a checked one.** No test can tell
   `box-shadow` from `margin-top`; both are just declarations. A preset that
   changed layout would pass every guard in the suite. This is the direct cost of
   allowing multi-layer decorations, and it is the first thing to check in a
   theme diff.
6. **All nine themes have never been seen on a real screen.** They were authored
   against computed contrast figures — every one clears the §3 floors, the
   surface-ladder rule and the accent-mark floor — but none was rendered during
   this change. The *figures* are trustworthy; the *taste* is not yet evidence,
   and this is the largest unverified surface in the redesign.
7. **`signal-noise` is a pattern over the whole window, and it is the one preset
   that can cost legibility.** It is capped at a 1px line on a 4px period at a
   low alpha, and it is deliberately not on `contrast`. If `dawn`'s text ever
   reads as thin or shimmering, remove it first.
8. **The logo's colour is the accent, which is a coupling.** The wordmark is now
   painted in `palette.accent`, and the suite asserts that clears 3:1 against the
   page and the surface in every theme. What it does *not* check is whether an
   accent reads well as a large filled wordmark — a colour can pass 3:1 and still
   be a poor choice at that size. Changing an accent changes the logo.
9. **The re-skin changes what existing installs see.** `default-dark` and
   `default-light` are no longer byte-identical to the pre-3.3.0 values: the
   grounds were lifted off pure black/white and the accent was re-derived. This
   was the point of the redesign, but it means an upgrading student sees a
   different app without asking for one.
10. **Cold-start under a light theme** is the standing risk from §5, not a defect.
    Revisit if a light default is ever shipped.
