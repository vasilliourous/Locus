# Themes — the decorative skin layer

```
audience:    builder
status:      live
authoritative-for: the theme registry's mechanics — what a theme may change, how a selection resolves, and how to add one
verified-against: docs/state.toml, client/tests/theme-colors.test.ts
```

> **This document owns the *layer*. `/COLOURS.md` owns every colour.** No palette,
> value or description of how a surface looks appears here — if you are looking
> for what a theme *is* rather than what it *may do*, read `/COLOURS.md` §§3–4.
> The history of that split is in `../reference/FIXES.md` and in `/COLOURS.md` §11.

---

## 1. What this layer is, and what it is not

The client has a named appearance (**a theme**) a student picks from a dropdown,
on top of a three-way `theme_mode` (`system` / `light` / `dark`). This document
describes that layer.

The design constraint that shapes everything here: the layer **sits on top** of
the shipped appearance and must not restructure it.

| The theme layer MAY change | The theme layer MUST NOT change |
|---|---|
| The palette (accent, surfaces, borders, text, status roles) | The two-tab shape, or anything in `_navigation-meta.ts` |
| Corner radii, via two CSS variables | Tier identity — see §3 rule 4 |
| A **multi-layer decoration** from a named preset, scoped to `[data-theme-skin]` | Any component's structure, props or behaviour |
| Anything a decoration can paint **without changing layout** | Whether a state is shown, or what it says |

It is deliberately not a redesign. The Connection screen's layout, its state
machine and its message priority (`client/docs/FRONTEND.md` §§2–4) are untouched
by a theme, and a theme that changes what a student reads is a bug in the theme,
not a feature.

### Decoration is a named preset, not injected CSS

A preset is a `DecorationSpec`: `short` (root declarations) plus an ordered
`layers` array. That structure exists because a *rice* is inherently layered — a
base wash, a directional treatment, a texture pass and a vignette — and a single
flat string cannot say it.

The safety properties are asserted directly by `theme-colors.test.ts`: every
brace balances, no construct introduces a document-level rule, and no preset can
load a remote or inline asset.

**Every layer must stay compositor-only.** `background-*`, `box-shadow`,
`border-*`, `opacity`, `filter`, `mix-blend-mode` and custom-property declarations
are fine, because none can change layout. Nothing in a preset may set `width`,
`height`, `margin`, `padding`, `position`, `display`, `flex`, `grid`, `gap`,
`order` or `font-size` on anything. A decoration that could resize or move an
element is a decoration that can move the Connect control, which is the one thing
this app must not do.

That is a **review obligation**, not a checked property — §10 records it as
unguarded rather than claiming the suite covers it.

The fork also carries a `theme_setting.css_injection` field. **Do not reuse it as
a theme mechanism.** A theme that can emit arbitrary CSS can restyle any
component, which is the interference property this layer exists to avoid — and it
makes a theme change unreviewable, because the diff can contain anything.

---

## 2. The resolution chain

One function decides what a student sees: `resolveTheme()` in
`client/src/pages/_themes.ts`. The order is not negotiable:

```
1. the selected theme's palette        (verge.theme_id -> THEMES[id])
2. legacy per-field colour overrides   (theme_setting.primary_color, ...)
3. the legacy light/dark choice        (theme_mode, only when no theme is set)
4. the built-in default theme          (default-dark)
```

Read as: **a selected theme is the base, and the legacy fields are an override of
it.** Every field keeps the precedence it has today — `setting.X || spec.X`. That
is the least invasive form of the change: no stored value is reinterpreted, no
user's custom colour stops applying, and the change is a substitution of the base
rather than a new precedence rule.

### `theme_id` absent means the pre-theme behaviour, exactly

A device with no `theme_id` resolves through `theme_mode` as it always has. This
is what makes the layer additive: with the field unset — which is every install
that never opened the dropdown — the app behaves exactly as it did before the
layer existed. `theme-colors.test.ts` pins that.

### An unknown or missing id never errors

An id that is absent, empty, or not in the registry resolves to `default-dark`.
This is the same rule the subscription union follows (`FRONTEND.md` §4: never
invent a state) — a theme id is a decoration, and a decoration that cannot be read
degrades to the default rather than blanking the screen or throwing.

**Do not "simplify" the resolver to `THEMES[id]`.** The id comes from a file a
user can edit, so an unknown value is a real possibility rather than a programming
error, and `THEMES[id]` would surface as a crash several frames later with nothing
pointing back at the cause.

### One mode per theme

**A theme carries its own light/dark mode.** There is deliberately no
theme-plus-mode matrix: selecting a theme gives the appearance that theme defines,
whatever `theme_mode` says.

The consequence, stated so nobody later reports it as a bug: **a student cannot
have a light variant of a dark-only theme.** Themes are separate palettes, not
palettes with two variants each. If a theme is wanted in both modes it is added as
two entries with two names.

Because a theme carries its mode, **selecting a theme supersedes `theme_mode` for
as long as it is set.** The Appearance control still reads and writes `theme_mode`
and still governs behaviour when no theme is selected. The two are not merged in
this pass — see §7.

---

## 3. What every theme must satisfy

Four rules, each enforced by `client/tests/theme-colors.test.ts`. **The numeric
floors are owned by `/COLOURS.md` §9**; this section states the rules, not the
numbers, so there is exactly one place they can be wrong.

1. **Contrast.** Text on its own surface, and the accent used as text and as a
   mark, must clear the floors in `/COLOURS.md` §9 in every theme. A light theme
   generally needs a different accent from a dark one; that is a property of the
   theme, not a special case in the code.
2. **A theme's `mode` must match its palette direction.** A theme declaring
   `mode: 'dark'` must have light text on a dark surface. This is the check that
   catches a copy-paste error between two similar themes, which is the realistic
   way a registry entry is authored wrong.
3. **Structural relationships hold.** `surface` is a visible step from
   `background`; `border` is visible against the surface; `textSecondary` is
   dimmer than `textPrimary` but still readable. A theme that violates one looks
   broken in a way contrast maths alone will not catch.
4. **Tier colours are not themed.** `TIER_COLORS` and `TIER_FALLBACK` are
   **identity, not decoration**: the badge reinforces a word, and a theme that
   repainted it would be making a claim about a product tier. They stay fixed
   across every theme. If a tier colour is ever themed, that is a product
   decision and it is not this layer's.

---

## 4. The registry

`client/src/pages/_themes.ts` holds `THEMES`, `DECORATIONS`, `DEFAULT_SHAPE`,
`resolveTheme`, `isThemeId` and `themeDefaultFor`. **The palettes and the presets
are documented in `/COLOURS.md` §§3–4**; this document does not restate them.

The id list is data: `docs/state.toml [client.theme_ids]`, recomputed from the
registry by `check-consistency.sh` §9, which fails on disagreement in **either**
direction. That is why a theme cannot be added in one place.

---

## 5. The three paint layers

The window background exists in three places that cannot share code and are
painted at three different moments:

1. the **native window** (`client/src-tauri/src/utils/resolve/window.rs`)
2. the **document** (`client/src/index.html`, then
   `client/src/assets/styles/index.scss`)
3. the **app** (`_theme.tsx` via `use-custom-theme`)

Layers 1 and 2 are painted **before any bundle runs**, so they cannot know which
theme is selected — and they must still paint something that is not a visible
flash of the wrong colour. `theme-colors.test.ts` reads all three files and fails
when they disagree.

**The decision: layers 1 and 2 stay pinned to the two default grounds and are not
extended per theme.**

The consequence, stated plainly because it is a real trade-off and not an
oversight: **cold-start paints the default ground, whichever theme is selected.**
A student on a theme with a different ground sees that default for the brief
window before the bundle runs. Between two grounds of the same mode that is not
perceptible, and it costs nothing.

The alternative — teaching the native window and the document about every theme —
means threading the selected theme through the Rust window resolver and a
pre-bundle script, adding a second place a theme id can be wrong, to remove an
imperceptible flash. That is not a trade worth making here. **If a theme's
ground differs enough in *mode* from the cold-start default, revisit this:** the
light themes are the case to test it against.

---

## 6. Adding a theme

In this order. Skipping a step fails a guard rather than producing a silent
inconsistency — which is the whole point of the ordering.

1. **Add the entry** to `THEMES` in `client/src/pages/_themes.ts`, with `id`,
   `labelKey`, `mode`, `palette`, `shape` and (optionally) `decoration`.
2. **Document its palette and its reason** in `/COLOURS.md` §3 — what it is for,
   what it must not be confused with, and any deliberate trade-off.
3. **Add the label key** to `client/src/locales/en/home.json` beside the existing
   `theme*` keys, then regenerate the i18n key list. Other locales fall back
   rather than being machine-translated.
4. **Update `[client.theme_ids]`** in `docs/state.toml`, then run
   **`bash server/scripts/check-consistency.sh`**. Under `sh` or `dash` the
   script aborts with "Bad substitution" and prints false `BAD` lines, so run it
   with `bash`.
5. **Run `pnpm test`** in `client/`. The suite checks §3's rules against the new
   entry, and a new theme is the most likely thing to fail the contrast or
   mode-direction checks.
6. **Show the new test failing first** if you added one. A guard that cannot fail
   reads exactly like a guard that passed (`docs/reference/DEBUGGING-METHOD.md`).

Adding a **decoration preset** is a separate, smaller change: a named block in the
preset table, used by at least one theme, scoped to the skin attribute so it
cannot leak into components.

---

## 7. What is deliberately not in this pass

| Not done | Why |
|---|---|
| Merging the theme dropdown into the `theme_mode` control | The Appearance control is shipped, tested and correctly separated (`useThemeMode()` is the *resolved* value; `theme_mode` is the *stored* one — see its comment in `account.tsx`). Merging them is a UI change with its own migration question, and it is not needed for themes to work. |
| Per-theme cold-start grounds in the native window | §5 — an imperceptible flash is not worth a second source of truth for theme ids. |
| Themeable tier identity | §3 rule 4 — identity, not decoration. |
| Light/dark variants of each theme | §2 — one mode per theme, by decision. |
| Free-form CSS per theme | §1 — a named preset keeps a theme change reviewable. |
| Shipping a theme over the hub | Themes are client-side data. Nothing about a theme reaches the hub, and no theme can change what the client reports about itself. |

---

## 8. Verifying it

```sh
cd client && pnpm test          # registry suite: contrast, mode direction, relationships
cd client && pnpm run typecheck # the registry's shape is typed
bash server/scripts/check-consistency.sh   # §9 registry ids, §49 no colour outside /COLOURS.md
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

Read the resolved state off the document root:

```js
document.documentElement.getAttribute('data-theme-id')
getComputedStyle(document.documentElement).getPropertyValue('--card-radius')
```

A missing attribute means the hook never ran. An attribute showing the default
when you selected something else means `resolveTheme` hit its fallback — which is
what an *absent* id looks like — so check the write path before blaming the
registry.

### "The theme applied but some element kept the old colour"

The element is reading a literal instead of the palette. `theme-colors.test.ts`
scans components for exactly this; a new offender is a one-line fix in the
component.

### "My stored theme id is ignored"

`resolveTheme` falls back on an id it does not recognise. Compare
`verge.theme_id` against `THEME_IDS`. A theme added to the registry in one place
and to `state.toml` in another fails `check-consistency.sh` §9 — which is the
point of the guard, not an obstacle to it.

### "The hero/decoration of one theme appears on another"

A preset is scoped to `[data-theme-skin]`, which is set only while a decorated
theme is active. An effect that outlives the selection means the attribute is not
being cleared — look at the decoration effect in `use-custom-theme`, not at the
preset.

---

## 10. Known gaps

Recorded here rather than left in a comment, because a gap that lives only in code
is a gap the next agent rediscovers. **Colour-specific gaps are `/COLOURS.md`'s**;
these are the layer's.

1. **~~No control clears `theme_id`.~~ Closed 2026-11-01.** The dropdown carries an
   explicit **Default (follow system)** entry that writes `theme_id: ''`, so
   `theme_mode` is reachable again after a theme has been chosen. `''` is a safe
   sentinel — `isThemeId('')` is false, so no registry entry can shadow it, and
   `hasTheme` in `use-custom-theme.ts` is `theme_id.length > 0` — and both
   properties are asserted in `client/tests/theme-colors.test.ts`. Recorded in
   [`STILL-OPEN.md`](STILL-OPEN.md) as the resolution, because it was a real gap
   students could only escape by hand-editing `verge.yaml`.
2. **The two Appearance controls can still disagree in appearance.** The theme
   dropdown and the `theme_mode` dropdown are separate and both visible on
   Account. When a theme is set, the mode control shows a value the app is not
   honouring. Clearing the theme (item 1) makes the state *recoverable*, which is
   the part that was actually harmful; the two controls being separate is a
   tidiness question, and §7 defers merging them.
3. **A decoration designed for one mode can be named by the other.** No test
   objects: the suite checks a preset's *shape* and its contrast floors, not
   whether the preset's own alphas suit the palette naming it.
4. **A decoration preset cannot be verified without eyes.** Every preset uses
   compositor-only techniques chosen because they need no asset and cannot shift
   layout. That they *look* right on four platforms is unverified, and §8 says so.
5. **Compositor-only is a review rule, not a checked one.** No test can tell
   `box-shadow` from `margin-top`; both are just declarations. A preset that
   changed layout would pass every guard in the suite. This is the direct cost of
   allowing multi-layer decorations, and it is the first thing to check in a
   theme diff.
6. **No theme has been seen on a real screen during the current palette work.**
   The figures are computed and guarded; the taste is not evidence. See
   `/COLOURS.md` §0.
