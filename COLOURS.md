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

**Permanent Dawn, decided 2026-11-19.** Chrysalis has exactly one appearance. There is
no theme registry, no theme id, no light/dark switch, no user-selectable palette
and no theme hook — the value of every colour in the app is a constant, and the
machinery that could produce a second one has been deleted rather than disabled.

This supersedes the **monochrome reset of 2026-11-11** (recorded in §11). That
decision reduced every Chrysalis-owned colour to grey so a real colour design could
start from a neutral base. This is that design, and it is the only one.

What "permanent Dawn" means concretely:

- **One mode: dark.** There is no light appearance. `CHRYSALIS_LIGHT`, the
  `prefers-color-scheme` blocks in the app, the landing page and the console, and
  every `mode === 'light'` branch are gone. A student on a bright laptop cannot
  choose a light UI; that is the cost of the decision, not an oversight.
- **The OS follows the app.** The native window is painted the app's ground on
  every system, and the title-bar hint is a constant. A light-mode machine no
  longer gets a light frame around a dark app.
- **Stored choices are dropped, not migrated.** An existing `verge.yaml` may still
  carry `theme_id` or `theme_mode`; `IVerge` has no `deny_unknown_fields`, so both
  keys are ignored on read and nothing converts them. A student who had chosen
  `sepia` now sees Dawn.
- **The landing page and the operator console are dark-only too** (§5, §7). A
  light marketing page beside a permanently dark product would be two products.
- **`theme_setting` is gone too (deleted 2026-11-23).** It was the per-field
  override of the palette inherited from Clash Verge Rev, and it had **no
  reader**: `services/theme.ts` builds the one theme with no argument. Earlier
  revisions of this file described it as a surviving *preference* that still
  applied; that was false, and the paragraph asserting it is what kept the field
  in `IVerge` for four days after the registry around it was removed. There is now
  no input to the palette of any kind.

**Status of this section:** the *decision* is a fact about the tree. Its
consequences above were read from the code, not seen on a screen.

---

## 1. How colour flows, and the one rule that must not be broken

One chain, one input, built once at module load:

```
CHRYSALIS_COLORS                     _theme.tsx   → the palette, the only one
   └─> defaultTheme              _theme.tsx   → the legacy { primary_color, … } shape
        └─> createTheme(...)     services/theme.ts → `chrysalisTheme`, the MUI theme
             └─> ThemeProvider   main.tsx     → every screen, including the gate
```

`services/theme.ts` also writes the window-chrome CSS variables
(`--divider-color`, `--scrollbar-*`, `--background-color`, `--selection-color`)
and the global stylesheet, once, through `applyThemeWhenReady()`. It is a function
rather than a module side effect so importing the module has no DOM requirement —
the palette test imports it under Node.

**The rule: a future colour pass edits `CHRYSALIS_COLORS` and stops there.** It does
not edit `services/theme.ts`, and it does not edit a component. A colour written
into a component is the defect the guard in §10 catches.

There is no override. `theme_setting` used to be described here as
"`setting.X || defaultTheme.X` inside the projection, so a student who set a
custom colour keeps it" — that sentence was wrong when it was written, because the
projection never took a `setting` argument, and the field it named was deleted on
2026-11-23. A future colour pass therefore has exactly one input: the palette.

### The three paint layers, and why there are still three values for one colour

The window is painted three times before the app is themed, and none of the three
can import the others:

| Layer | Painted by | Source of truth |
|---|---|---|
| **1. The native window** | the OS, before any web content exists | `client/src-tauri/src/utils/resolve/window.rs` |
| **2. The document** | HTML parsed before any bundle runs | `client/src/index.html`, then `client/src/assets/styles/index.scss` |
| **3. The app** | the module-level theme | `client/src/pages/_theme.tsx` → `services/theme.ts` |

There is a fourth, the pre-bundle bootstrap script
(`client/src-tauri/src/utils/resolve/window_script.rs`), which paints before
layer 2's stylesheet is live and cannot import anything.

All four carry the same ground (§2). They are kept in step by
`client/tests/theme-colors.test.ts`, which reads every one of them — the copies
cannot be removed (layer 2 cannot import layer 3, layer 1 is Rust), so the test is
what keeps them honest. **Changing layer 3 alone shows the old colour for a frame
on every launch.**

---

## 2. The one palette

Authority: `client/src/pages/_theme.tsx` (`CHRYSALIS_COLORS`), projected into MUI by
`client/src/services/theme.ts`. This table is the single source for every value in
the product, the landing page, the console and every native window frame.

| Role | Value | Notes |
|---|---|---|
| `background` | `#1A1B26` | The window |
| `surface` | `#24283B` | Cards and panels; one visible step above the window |
| `surfaceHover` | `#2F3349` | Interactive surfaces; also the `::selection` ground |
| `border` | `#3B4261` | Dividers, input borders, window border |
| `textPrimary` | `#C0CAF5` | Body text |
| `textSecondary` | `#9AA5CE` | Labels and hints |
| `accent` | `#7AA2F7` | Primary action, active indicators, **and the wordmark** |
| `accentHover` | `#9AB8FF` | |
| `success` | `#9ECE6A` | Connected |
| `warning` | `#E0AF68` | Connecting, degraded, renewal warnings |
| `error` | `#F7768E` | Disconnected, failures |
| `secondary` | `#BB9AF7` | Non-primary emphasis |
| `info` | `#7DCFFF` | Informational, not a status |

### The legacy projection

Authority: the same file, `defaultTheme`. These are the MUI fields the chain reads;
every one is overridden by a student's own `theme_setting` when they have one.

| Field | Value | Role |
|---|---|---|
| `primary_color` | `#7AA2F7` | the accent |
| `secondary_color` | `#BB9AF7` | non-primary emphasis |
| `primary_text` | `#C0CAF5` | body |
| `secondary_text` | `#9AA5CE` | labels |
| `info_color` | `#7DCFFF` | informational |
| `error_color` | `#F7768E` | failure |
| `warning_color` | `#E0AF68` | degradation |
| `success_color` | `#9ECE6A` | connected |
| `background_color` | `#1A1B26` | the window |

### The surface ladder

`background` and `surface` are deliberately two different values: MUI's shadow
scale is zeroed app-wide, so a card is separated from the page by a one-step-up
surface **and** its 1px border. They must not collapse to one value, which is what
the old default-dark did — the border alone is what reads as unfinished.
`theme-colors.test.ts` asserts the step exists.

### Shape

`--card-radius` is `12px` and `--control-radius` is `8px` (`DEFAULT_SHAPE` in the
same file). Shape is not colour and is not a theme field: there is one set.

### Status colour

**Status is carried by hue *and* by a word and a glyph** — the redundancy rule the
tier badge follows (§6). The three status roles are separated in both hue and
value, which is the property the previous palette could not claim: under the
monochrome reset the three roles were three greys within 32/255 levels of each
other, so a connected dot and an error dot differed only by their shadow ring.

---

## 3. There is one appearance

There is no registry. `client/src/pages/_themes.ts` was deleted on 2026-11-19,
together with `THEMES`, `THEME_IDS`, `resolveTheme`, `isThemeId`,
`themeDefaultFor`, `DEFAULT_THEME_ID`, `DECORATIONS` and the `ThemeSpec` /
`ThemePalette` / `DecorationSpec` types. The Account screen no longer has an
Appearance row or a Theme row, and `docs/state.toml [client.theme_ids]` is gone
with the checker that recomputed it (`check-consistency.sh` §9).

**Do not reintroduce a theme layer "for contrast" or "for light mode".** Re-adding
one means re-adding the registry, the resolver, the dropdown, the decoration
injector and the second paint path — all of which is what §0's decision removed.
If a second appearance is ever wanted, it is a new decision with its own dated
entry here, not a palette entry.

`theme-colors.test.ts` asserts that none of those names survives in the source
tree, and that the registry file does not exist. Both were demonstrated failing
before being kept.

---

## 4. Decorations

None. The five decoration presets (`forest-glow`, `ember-pit`, `blueprint`,
`risograph`, `signal-noise`) were part of the theme layer and were deleted with it.
The two `radial-gradient` washes that lived on the activation gate and the
suspension screen — the only other decorative paint in the product — were removed
at the same time: three screens carrying the same gradient is a placeholder, not a
design.

The `<style id="chrysalis-theme-decoration">` element that `use-custom-theme` wrote no
longer exists, because the hook does not. Nothing injects decoration CSS, so there
is no scope for a preset to escape into — the `[data-theme-skin]` scoping rules and
the tests that guarded them went with the mechanism.

---

## 5. The landing page

Authority: `server/site/style.css` `:root`. Served statically by Caddy with no
build step, so it is a **by-value copy of what the stylesheet compiles to** rather
than an import, and it is not enforced by the TypeScript test — it is enforced by
whoever changes the palette remembering this section exists.

| Variable | Value |
|---|---|
| `--bg` | `#1A1B26` |
| `--surface` | `#24283B` |
| `--surface-hover` | `#2F3349` |
| `--border` | `#3B4261` |
| `--text` | `#C0CAF5` |
| `--text-secondary` | `#9AA5CE` |
| `--accent` | `#7AA2F7` |
| `--accent-hover` | `#9AB8FF` |

The `prefers-color-scheme: dark` block and the light `:root` are gone: the page
is dark-only, like the product. A light marketing page beside a permanently dark
app would be two products.

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

The console is dark-only and is **not** yet recoloured to the Dawn palette: it
still carries the neutral set below, because it is an operator surface served
from the hub and its stylesheet is a separate build. This section records that
honestly rather than claiming an agreement that does not exist.

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

**Open:** the console reads as a second product beside the app. Recolouring it is
a follow-up with its own change, gated on the console's build and deploy path
(`docs/operate/DEPLOY.md`), and it must not be done by inventing values here — it
implements `§2`.

Not colour, and not owned here: the console's routes, its auth, and the
`/admin/` path.

---

## 8. Marks: the wordmark, the glyph, the app icon, the tray

| Mark | Source | Colour |
|---|---|---|
| Wordmark | `client/src/assets/image/logo.svg` | `fill: currentColor` |
| Glyph (app mark) | `client/src/assets/image/glyph.svg` | the artwork's own gradients — see below |
| Nav glyphs | `client/src/assets/image/itemicon/*.svg` | `fill: currentColor` |
| Application icon | generated by `client/scripts/generate-icons.mjs` | the glyph on `background` |
| Tray icons | generated by the same script, from `src/assets/image/mark/status_*.svg` | FlClash's own state art — see below |
| Landing-page icon | `server/site/icon.svg` | the glyph |
| Landing-page wordmark | `server/site/logo.svg` | `fill: currentColor` |

The nav glyphs and the wordmark resolve `currentColor` from their parent, which
`layout-sidebar.tsx` sets to `theme.palette.primary.main` — the accent. The glyph
does not: it carries its own gradients, so it is the one mark whose colour is
fixed by its artwork rather than inherited. There is one asset per mark, never one
per appearance.

`icon_light.svg` was **deleted** with the theme layer. The sidebar used to pick
between the light and dark glyph from the resolved mode; with one appearance that
choice collapsed, and the file had no remaining reference.

**The application mark is the FlClash glyph (2026-11-23).** It was the Clash Verge
Rev cat in every release up to 3.4.4 — a grey-to-black gradient with a white cat,
shipped in the window, the taskbar, the dock, the tray and the favicon while the
product called itself something else. `client/scripts/generate-icons.mjs` now
renders each size at its own resolution from `glyph.svg`, on §2's `background`,
using FlClash's own app-icon framing. The glyph's three gradients are **the
artwork, not a palette**: they are copied unaltered from FlClash's vector master
and appear on every surface, which is what adopting that icon set means. They are
the one family of colour in this file's marks that §2 does not supply.

The ground is read out of `_theme.tsx`, and `check-consistency.sh` §54 fails if a
packaged icon is not one the generator writes **or** if the bytes on disk disagree
with a fresh render. Which files are generated, and which two still need macOS
tooling, is in `client/src-tauri/icons/README.md`.

**The tray is FlClash's own state set.** It used to be three filled discs
recoloured to §2's status roles. It is now FlClash's four `status_*` assets, copied
verbatim into `client/src/assets/image/mark/`. FlClash's mapping
(`lib/common/tray.dart`) is `1` idle or macOS-templated, `2` running with TUN off,
`3` running with TUN on, `4` safe mode. Our three `IconKind`s map to `1`, `2` and
`3`; `status_4` ships unrendered because this client has no safe-mode branch.

The disc decision recorded here previously — "a disc reads at 16 px where a
detailed glyph does not" — is **superseded**. The glyph was chosen deliberately,
and it survives the size: its 16 px frame still resolves both bars and the dot.

**The one white that is not a surface: the macOS template ink.** The template set
(`tray-icon*-mono*.ico`) is drawn in pure white, `#FFFFFF`, and that is the only
colour in the marks that does not come from §2. It cannot affect what a student
sees — a template image is tinted by the OS from its alpha channel, so the RGB is
discarded — and white is the format's convention. It is stated here, rather than
left as a literal in the generator, because that generator is on §10's allowlist
and an allowlisted file with an unexplained value is how a second home starts.


---

## 9. Contrast rules the palette must satisfy

These are not preferences. Each is asserted by `client/tests/theme-colors.test.ts`,
so a change to the palette cannot ship unreadable.

| Rule | Floor | Why |
|---|---|---|
| `textPrimary` on the surface **and** on the page | **4.5:1** | body copy; both grounds, because most text sits on a card |
| `textSecondary` on the surface | **4.5:1** | labels and hints are still text a student must read |
| `accent` on the page | **3:1** | it paints the wordmark and the primary action; 3:1 is the floor for a non-text UI mark |
| `success`, `warning`, `error` on the surface | **3:1** | the status dot is the whole signal; an invisible one is worse than none |
| surface against page | **> 1.15:1** | the ladder must exist: with shadows zeroed, the border alone is what reads as unfinished |
| no pure white, no pure black | — | pure white on a full-window panel is a light source, not a surface; pure black crushes the ladder |

Two things a contrast figure cannot express, and a reader should know are
therefore **unverified**:

- **Whether it looks right.** A ratio says two values are distinguishable, not that
  they are pleasant. That needs a person, a dark room and a bright one.
- **Whether a status reads at a glance.** Hue separation is asserted as a ratio;
  colour-blindness is not modelled by any figure here. The words and glyphs beside
  each status are the fallback, which is why they are not optional.

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
palette has to be expressible in code, so `§2`–`§8` name the file that holds each
value. That is the *implementation* of what this file describes, and the pair is
guarded in both directions — the doc names the file, and
`client/tests/theme-colors.test.ts` asserts the file's values are what this
document says, and that the four paint layers agree with each other.

**The guard:** `server/scripts/check-consistency.sh` §49 fails when a file outside
the allowlist carries a colour literal. The allowlist is: this file, and the seven
sources below. It is *asserted* rather than trusted — each path must exist, so a
renamed module fails here rather than silently widening the allowlist to nothing.

| Allowlisted source | Holds |
|---|---|
| `client/src/pages/_theme.tsx` | `CHRYSALIS_COLORS`, `defaultTheme`, `TIER_COLORS` |
| `client/src/index.html` | the pre-bundle document ground |
| `client/src/assets/styles/index.scss` | the pre-theme stylesheet fallback |
| `client/src-tauri/src/utils/resolve/window.rs` | the native window ground |
| `client/src-tauri/src/utils/resolve/window_script.rs` | the pre-bundle bootstrap ground |
| `client/scripts/generate-icons.mjs` | one value only: the macOS template ink (§8), which the OS discards |
| `server/site/style.css` | the landing page |
| `server/console/src/styles.css` | the operator console |

**What the guard does not cover, stated so nobody reads it as coverage:** a
component that hardcodes a colour is caught only by the palette test's per-file
scan of the five main screens, not by §49; and a *shape* decision (radius, weight,
spacing) that changes how a surface reads is not colour at all and this file has
nothing to say about it.

---

## 11. Superseded, and where the old values went

Four decisions have been superseded, and none of their values are reproduced
anywhere. In order:

| Date | Decision | Superseded by |
|---|---|---|
| — | the original green-black specification | the redesign below |
| — | the 3.3.x palette | the monochrome reset |
| 2026-11-11 | **monochrome reset** — every Chrysalis-owned surface neutral, so a colour design could start from a neutral base | **permanent Dawn (§0)** |
| 2026-11-19 | **nine-theme registry** — `default-dark`, `default-light`, `slate`, `dawn`, `ember`, `moss`, `linen`, `sepia`, `contrast`, five decoration presets and a theme hook | **one appearance (§0, §3)** — the registry, the resolver, the hook, the dropdown and the decorations were deleted |

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
