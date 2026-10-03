import { LOCUS_COLORS, LOCUS_LIGHT } from '@/pages/_theme'

/**
 * The theme registry — the six looks a student can pick between.
 *
 * `docs/reference/THEMES.md` is the design record and the authority on *why*
 * each theme exists; this file is the authority on *what* each theme is.
 * `check-consistency.sh` §9 recomputes the id list from here and fails when it
 * disagrees with `docs/state.toml`, so a theme cannot be added in one place.
 *
 * # Debugging a theme problem
 *
 * `THEMES.md` §9 is written as symptoms ("I picked a theme and nothing
 * changed"), each naming the one place to look. The fastest first move is
 * usually to read the resolved state off the document root, because almost every
 * theme bug is either "the id did not round-trip" or "the palette is not
 * reaching the component":
 *
 *     document.documentElement.getAttribute('data-theme-id')
 *     getComputedStyle(document.documentElement).getPropertyValue('--card-radius')
 *
 * An attribute that is missing means the hook never ran; one showing
 * `default-dark` when you selected something else means `resolveTheme` hit its
 * fallback — which is what an *absent* id looks like, so check the write path
 * before assuming the registry is wrong.
 *
 * # The rules, in short (full version: THEMES.md §3)
 *
 *   1. Text on its own surface, and the accent used as text, must clear the
 *      contrast floors the test suite applies.
 *   2. `mode` must match the palette direction — a `dark` theme has light text
 *      on a dark surface. This is the check that catches a copy-paste error
 *      between two similar themes, which is how a registry entry really gets
 *      authored wrong.
 *   3. `surface` is a visible step from `background`, `border` is visible
 *      against both, `textSecondary` is dimmer than `textPrimary` but readable.
 *   4. Tier colours are NOT themed. See `TIER_COLORS` in `_theme.tsx`.
 *
 * # Why the defaults are re-exported rather than restated
 *
 * `default-dark` and `default-light` are built from `LOCUS_COLORS` and
 * `LOCUS_LIGHT` directly, not from copies of their hex values. The shipped
 * palette has one home, and a theme that restated it would be a second home that
 * can drift — which is the exact defect class `theme-colors.test.ts` exists to
 * catch. The registry is a *superset* of the shipped appearance, provably,
 * because the two default entries are literally the same objects.
 */

/**
 * A shell of a theme: what the layer is allowed to change, and nothing else.
 *
 * `decoration` names a **preset** from `DECORATIONS`, never CSS text. A theme
 * that could emit arbitrary CSS could restyle any component, which is the
 * interference property this layer exists to avoid, and it would make a theme
 * change unreviewable. See THEMES.md §1.
 */
export interface ThemeSpec {
  /** Stable id. Persisted in `verge.theme_id`; renaming one is a migration. */
  id: ThemeId
  /** i18n key for the dropdown label. Added beside the other `theme*` keys. */
  labelKey: string
  /** The theme carries its own mode — one mode per theme, by decision. */
  mode: 'light' | 'dark'
  palette: ThemePalette
  shape: ThemeShape
  /** A named preset from `DECORATIONS`, or omitted for none. */
  decoration?: DecorationId
}

/**
 * The colours a theme owns.
 *
 * Deliberately the *whole* palette rather than a delta against a base: a theme
 * that inherits most of its values from another theme cannot be read on its own,
 * and "which theme does this actually derive from" is not a question anyone
 * should have to answer while looking at a palette.
 *
 * Not exported: the test suite reaches it through `THEMES`, and exporting a
 * second name for the shape would invite a consumer to type against it instead
 * of against the registry — which is the thing that is actually guarded.
 */
interface ThemePalette {
  background: string
  surface: string
  surfaceHover: string
  border: string
  textPrimary: string
  textSecondary: string
  accent: string
  accentHover: string
  success: string
  warning: string
  error: string
  /** Secondary/accent-adjacent colour for non-primary emphasis. */
  secondary: string
  /** Informational (not a status) accent. */
  info: string
}

/** Corner radii, written as CSS variables so `_surfaces.ts` can read them. */
export interface ThemeShape {
  /** Cards and panels. Today's value is 12. */
  cardRadius: number
  /** Controls: buttons, selects, inputs. */
  controlRadius: number
}

/**
 * A named decoration preset.
 *
 * # Why this is a structure and not one string
 *
 * A preset began as a single flat declaration block, and the guard enforcing
 * that forbade any `{` or `}`. That was true to the original design — a preset
 * was one `radial-gradient` — but it cannot express a *rice*, which is
 * inherently layered: a base wash, a directional sheen, a grain or scanline
 * pass, and a vignette, composited in a defined order.
 *
 * So a preset is now a small structure with the layers spelled out:
 *
 *   - `short`   — declarations applied to the skin root itself.
 *   - `layers`  — further declaration blocks, painted in order on top. Each is
 *                 still a *declaration block*, never a selector: the scoping
 *                 rule is unchanged.
 *
 * # The constraint that has NOT changed
 *
 * Every layer must remain compositor-only. `background-image`, `background-*`,
 * `box-shadow`, `border-*`, `opacity`, `filter`, `mix-blend-mode` and the
 * `--var` declarations a component may read are all fine, because none of them
 * can change layout. **Nothing here may set `width`, `height`, `margin`,
 * `padding`, `position`, `display`, `flex`, `grid`, `gap`, `order` or `font-size`
 * on anything** — a decoration that could resize or move an element is a
 * decoration that can move the Connect button, which is the one thing this app
 * must not do. That rule is a review obligation, not something a test can prove;
 * `THEMES.md` §8 says so.
 *
 * Multi-layer presets that draw at the *edges* of the skin element are what make
 * a rice read as a rice. They work here only because the skin element is the
 * layout root and already fills the window; a preset that assumed its own box
 * could paint outside the app.
 */
export interface DecorationSpec {
  /** Declarations on the skin root. Empty string is valid. */
  short: string
  /** Ordered declaration blocks, painted over `short`. Omit for a flat preset. */
  layers: string[]
}

/** The closed set of decorative presets. Adding one is a code change. */
export type DecorationId =
  | 'forest-glow'
  | 'ember-pit'
  | 'blueprint'
  | 'risograph'
  | 'signal-noise'

export type ThemeId =
  | 'default-dark'
  | 'default-light'
  | 'slate'
  | 'dawn'
  | 'ember'
  | 'moss'
  | 'linen'
  | 'sepia'
  | 'contrast'

/**
 * The decoration presets.
 *
 * Each is a self-contained block scoped to `[data-theme-skin]`, which is set on
 * the layout root only while that theme is active. Nothing here targets a
 * component, a class, or the document: a preset that could reach outside the
 * skin attribute would defeat the point of naming presets at all.
 *
 * Every technique used is painted by the compositor and needs no asset. That is
 * what lets a decoration be additive: `use-custom-theme` writes this into one
 * `<style id="locus-theme-decoration">` element scoped to `[data-theme-skin]`,
 * and removing that element removes every trace of the layer.
 */
export const DECORATIONS: Record<DecorationId, DecorationSpec> = {
  /**
   * `forest-glow` — a soft green wash from above, for `moss`.
   *
   * The light source is the top edge, which is the conventional reading of a
   * window in a wall. Kept at a low alpha against a desaturated palette, so it
   * reads as depth rather than as a coloured panel.
   */
  'forest-glow': {
    short: `
      background-image:
        radial-gradient(
          ellipse 120% 60% at 50% -10%,
          rgba(143, 191, 159, 0.10),
          rgba(143, 191, 159, 0) 70%
        );
      background-repeat: no-repeat;
    `,
    layers: [],
  },

  /**
   * `ember-pit` — a warm light source from below, for `ember`.
   *
   * Deliberately bottom-anchored: every other wash in this registry comes from
   * the top, which is the generic "product hero gradient". Light from underneath
   * reads as a fire, which is the palette's whole conceit.
   */
  'ember-pit': {
    short: `
      background-image:
        radial-gradient(
          ellipse 100% 55% at 50% 112%,
          rgba(224, 137, 95, 0.16),
          rgba(224, 137, 95, 0) 68%
        );
      background-repeat: no-repeat;
    `,
    layers: [],
  },

  /**
   * `blueprint` — a cold grid, for `slate`.
   *
   * Drawn with repeating gradients rather than an image so it needs no asset and
   * cannot shift layout. The lines are extremely low-alpha on purpose: a grid
   * that is visible at a glance competes with the UI on top of it, and the point
   * of a rice's texture is that it is felt rather than read.
   */
  blueprint: {
    short: `
      background-image:
        linear-gradient(rgba(136, 192, 208, 0.045) 1px, transparent 1px),
        linear-gradient(90deg, rgba(136, 192, 208, 0.045) 1px, transparent 1px),
        radial-gradient(
          ellipse 120% 70% at 50% -15%,
          rgba(129, 161, 193, 0.14),
          rgba(129, 161, 193, 0) 70%
        );
      background-size: 32px 32px, 32px 32px, auto;
      background-repeat: repeat, repeat, no-repeat;
    `,
    layers: [],
  },

  /**
   * `risograph` — print misregistration, for `sepia`.
   *
   * Two offset flat tints that read as over-inked plates. Kept to a very low
   * alpha and a small offset: the effect is meant to be noticed only once you
   * look for it, and a stronger version would be a bug report about a colour
   * cast rather than a style choice.
   */
  risograph: {
    short: `
      background-image:
        radial-gradient(
          ellipse 90% 60% at 22% 8%,
          rgba(47, 100, 128, 0.07),
          rgba(47, 100, 128, 0) 62%
        ),
        radial-gradient(
          ellipse 90% 60% at 82% 96%,
          rgba(150, 96, 42, 0.08),
          rgba(150, 96, 42, 0) 62%
        );
      background-repeat: no-repeat;
    `,
    layers: [],
  },

  /**
   * `signal-noise` — a phosphor glow with a fine scanline, for `dawn`.
   *
   * The glow is the second layer and the scanline the first, so the glow sits
   * *behind* the line pattern — which is what makes it read as light coming off
   * the screen rather than as an overlay on the text.
   *
   * **The scanline is the one preset here that can cost legibility**, because it
   * is a pattern over the entire window rather than a static wash. It is
   * therefore limited to a 1px line on a 4px period at a very low alpha, and it
   * is deliberately *not* on `contrast` — the theme whose whole job is
   * legibility. If text ever reads as thin or shimmering under this theme, this
   * preset is the first thing to remove.
   */
  'signal-noise': {
    short: `
      background-image:
        repeating-linear-gradient(
          to bottom,
          rgba(0, 0, 0, 0.10) 0px,
          rgba(0, 0, 0, 0.10) 1px,
          rgba(0, 0, 0, 0) 1px,
          rgba(0, 0, 0, 0) 4px
        );
      background-repeat: repeat;
    `,
    layers: [
      `
      background-image:
        radial-gradient(
          ellipse 120% 80% at 50% 42%,
          rgba(122, 162, 247, 0.10),
          rgba(122, 162, 247, 0) 58%
        );
      background-repeat: no-repeat;
      `,
    ],
  },
}

/** Today's radii. Kept as named values so a theme reads as a delta from them. */
export const DEFAULT_SHAPE: ThemeShape = { cardRadius: 12, controlRadius: 8 }

/**
 * `default-dark` — the shipped dark appearance.
 *
 * Built from `LOCUS_COLORS`, so it is byte-identical to what ships today. This
 * is the default a fresh install sees, and the fallback for an id that is
 * absent, empty or unknown (THEMES.md §2).
 */
const defaultDark: ThemeSpec = {
  id: 'default-dark',
  labelKey: 'home.components.connection.account.themeDefaultDark',
  mode: 'dark',
  palette: {
    background: LOCUS_COLORS.background,
    surface: LOCUS_COLORS.surface,
    surfaceHover: LOCUS_COLORS.surfaceHover,
    border: LOCUS_COLORS.border,
    textPrimary: LOCUS_COLORS.textPrimary,
    textSecondary: LOCUS_COLORS.textSecondary,
    accent: LOCUS_COLORS.accent,
    accentHover: LOCUS_COLORS.accentHover,
    success: LOCUS_COLORS.success,
    warning: LOCUS_COLORS.warning,
    error: LOCUS_COLORS.error,
    secondary: '#7FB8A0',
    info: '#7FB0C9',
  },
  shape: DEFAULT_SHAPE,
}

/**
 * `default-light` — the shipped light appearance.
 *
 * The accent is the *darker* green because this value is used for text-bearing
 * controls, where the dark accent on an off-white page fails the body-copy rule.
 * That is a property of this theme, not a special case in the code.
 */
const defaultLight: ThemeSpec = {
  id: 'default-light',
  labelKey: 'home.components.connection.account.themeDefaultLight',
  mode: 'light',
  palette: {
    background: LOCUS_LIGHT.background,
    surface: LOCUS_LIGHT.surface,
    surfaceHover: LOCUS_LIGHT.surfaceHover,
    border: LOCUS_LIGHT.border,
    textPrimary: '#1B2A23',
    textSecondary: '#55665D',
    accent: LOCUS_LIGHT.accent,
    accentHover: LOCUS_LIGHT.accentHover,
    success: '#2A6E4C',
    warning: '#8A5A00',
    error: '#A33226',
    secondary: '#4F6B5E',
    info: '#2C6480',
  },
  shape: DEFAULT_SHAPE,
}

/**
 * The registry. Order here is the order of the dropdown.
 *
 * Read it as a table: every entry is a full, self-contained look. See
 * `docs/reference/THEMES.md` §4 for why each one exists and what it must not be
 * confused with — a theme with no rationale is a palette nobody can safely
 * change later.
 */
export const THEMES: Record<ThemeId, ThemeSpec> = {
  'default-dark': defaultDark,
  'default-light': defaultLight,

  /**
   * `slate` — cool blue-grey, low stimulus.
   *
   * Scheme: **split-complementary** on a ~h220 blue-grey base. The accent
   * (`#88C0D0`, a frost blue) sits a quarter-turn from the surfaces, and the
   * status colours swing across to the warm side (`#D08770` orange, `#EBCB8B`
   * straw) so a warning is never mistaken for a link.
   *
   * Mood: calm and recessive. Every colour is low-saturation and close in value,
   * so nothing competes with the Connect control. This is the theme for someone
   * who wants the app to stop shouting.
   */
  slate: {
    id: 'slate',
    labelKey: 'home.components.connection.account.themeSlate',
    mode: 'dark',
    palette: {
      background: '#1E222A',
      surface: '#272C36',
      surfaceHover: '#323845',
      border: '#3E4553',
      textPrimary: '#D8DEE9',
      textSecondary: '#A9B4C4',
      accent: '#88C0D0',
      accentHover: '#A3D4E2',
      success: '#A3BE8C',
      warning: '#EBCB8B',
      error: '#D08770',
      secondary: '#81A1C1',
      info: '#5E9FD8',
    },
    shape: DEFAULT_SHAPE,
    decoration: 'blueprint',
  },

  /**
   * `dawn` — indigo night with a lit horizon.
   *
   * Scheme: **analogous** on an indigo base (h~230), with the accent a step
   * toward blue-cyan and the warm accents (peach, coral, violet) acting as
   * complements. Four hues in the accent family, deliberately: this is the most
   * colourful theme, and the variety is the point.
   *
   * Mood: nocturnal but not gloomy — the palette of a screen in a dark room at
   * 3am. It is the highest-chroma dark theme in the registry, which is what makes
   * it the "vibrant alternative" rather than another grey.
   */
  dawn: {
    id: 'dawn',
    labelKey: 'home.components.connection.account.themeDawn',
    mode: 'dark',
    palette: {
      background: '#1A1B26',
      surface: '#24283B',
      surfaceHover: '#2F3349',
      border: '#3B4261',
      textPrimary: '#C0CAF5',
      textSecondary: '#9AA5CE',
      accent: '#7AA2F7',
      accentHover: '#9AB8FF',
      success: '#9ECE6A',
      warning: '#E0AF68',
      error: '#F7768E',
      secondary: '#BB9AF7',
      info: '#7DCFFF',
    },
    shape: DEFAULT_SHAPE,
    decoration: 'signal-noise',
  },

  /**
   * `ember` — warm terracotta, lamplight.
   *
   * Scheme: **complementary** on a warm terracotta base (h~25). The accent
   * (`#E0895F`) is the base hue at full strength, and the cool counterweight is
   * carried by `info` (`#8FB0C9`) rather than by the accent — which is what keeps
   * a warm theme from turning muddy.
   *
   * Mood: warm and energetic without being loud. Warm light is easier on the eye
   * at night than a blue-white screen, and this is the theme for a cold room.
   * The blue channel is kept low throughout so it reads as lamplight, not as a
   * tinted grey.
   */
  ember: {
    id: 'ember',
    labelKey: 'home.components.connection.account.themeEmber',
    mode: 'dark',
    palette: {
      background: '#211A17',
      surface: '#2E2521',
      surfaceHover: '#3B302A',
      border: '#4E4038',
      textPrimary: '#EDE0D8',
      textSecondary: '#C0ABA0',
      accent: '#E0895F',
      accentHover: '#F0A17C',
      success: '#9CB380',
      warning: '#E0A85F',
      error: '#E0706B',
      secondary: '#C98F9E',
      info: '#8FB0C9',
    },
    shape: DEFAULT_SHAPE,
    decoration: 'ember-pit',
  },

  /**
   * `moss` — desaturated deep green, the quiet one.
   *
   * Scheme: **analogous** on a green base (h~145), a half-turn away from the
   * brand green in value rather than in hue. Every colour is muted, and the
   * range between the lightest and darkest surface is small.
   *
   * Mood: restful and organic. This is the closest relative of the default in the
   * registry, and it exists for people who like the default's character but want
   * it softer — the difference is saturation and lift, not identity.
   */
  moss: {
    id: 'moss',
    labelKey: 'home.components.connection.account.themeMoss',
    mode: 'dark',
    palette: {
      background: '#1B2320',
      surface: '#243029',
      surfaceHover: '#2E3D35',
      border: '#3A4A41',
      textPrimary: '#DCE8DF',
      textSecondary: '#A3B5A8',
      accent: '#8FBF9F',
      accentHover: '#A8D4B6',
      success: '#8FBF9F',
      warning: '#D9B382',
      error: '#D98C8C',
      secondary: '#7FA8B8',
      info: '#8FB8C9',
    },
    shape: { cardRadius: 14, controlRadius: 8 },
    decoration: 'forest-glow',
  },

  /**
   * `linen` — cool neutral light.
   *
   * Scheme: **analogous** on a neutral-cool base (h~210). Near-monochrome
   * surfaces with a single blue accent, so the one saturated thing on screen is
   * always interactive.
   *
   * Mood: clean and clinical. The off-white ground (`#F4F7FA`, L=0.93) is chosen
   * rather than white so the card edge is a relationship rather than a glare —
   * see the note in `_theme.tsx` on why `#FFFFFF` was removed.
   */
  linen: {
    id: 'linen',
    labelKey: 'home.components.connection.account.themeLinen',
    mode: 'light',
    palette: {
      background: '#EAEEF2',
      surface: '#F4F7FA',
      surfaceHover: '#DFE5EB',
      border: '#C0CAD4',
      textPrimary: '#1E2733',
      textSecondary: '#55636F',
      accent: '#2F6F9F',
      accentHover: '#255A82',
      success: '#3F6C45',
      warning: '#8A5A00',
      error: '#A33226',
      secondary: '#5B6E85',
      info: '#255E8A',
    },
    shape: DEFAULT_SHAPE,
  },

  /**
   * `sepia` — warm paper, low glare.
   *
   * Scheme: **analogous** on a warm paper base (h~35), with a burnt-orange accent
   * the same hue at full strength. The text is a warm near-black rather than a
   * neutral one, which is what makes the whole surface feel like paper.
   *
   * Mood: calm and analogue. Designed for a bright room or beside a window, where
   * a cool white screen competes with the daylight. The warm ground also reduces
   * the blue-light load for evening reading.
   */
  sepia: {
    id: 'sepia',
    labelKey: 'home.components.connection.account.themeSepia',
    mode: 'light',
    palette: {
      background: '#EDE4D3',
      surface: '#F6F0E3',
      surfaceHover: '#E0D5BF',
      border: '#C4B69C',
      textPrimary: '#2B2620',
      textSecondary: '#6B6152',
      accent: '#96602A',
      accentHover: '#7A4D20',
      success: '#4A6B3A',
      warning: '#8A5A00',
      error: '#9E3A2A',
      secondary: '#6B6152',
      info: '#2F6480',
    },
    shape: DEFAULT_SHAPE,
    decoration: 'risograph',
  },

  /**
   * `contrast` — maximum legibility, still not a glare.
   *
   * Scheme: a **neutral ramp** with a single green accent. Not a colour scheme at
   * all, deliberately: this theme's job is legibility, and hue variety would
   * work against it. What separates it from the other dark themes is *range* —
   * text at ~13:1 on the surface, borders at 2.40:1, and the largest surface step
   * in the registry.
   *
   * It replaces the old pure-white-on-pure-black `high-contrast`, which met the
   * contrast numbers by every measure and was still the wrong answer: `#FFFFFF`
   * on `#000000` is the highest-glare combination possible, and for the people
   * who need this theme most — those with astigmatism or light sensitivity —
   * glare is the symptom, not the fix. `#F0F3F5` on `#242A30` is still ~13:1 and
   * does not halo.
   */
  contrast: {
    id: 'contrast',
    labelKey: 'home.components.connection.account.themeContrast',
    mode: 'dark',
    palette: {
      background: '#14171A',
      surface: '#242A30',
      surfaceHover: '#2E353C',
      border: '#5A646E',
      textPrimary: '#F0F3F5',
      textSecondary: '#C2CAD2',
      accent: '#6FD08C',
      accentHover: '#8FDFA6',
      success: '#6FD08C',
      warning: '#E8C46A',
      error: '#F08A8A',
      secondary: '#9AB4CC',
      info: '#8FC4E8',
    },
    shape: DEFAULT_SHAPE,
  },
}

/**
 * The default theme, and the fallback for an unusable id.
 *
 * Exported separately because it is used in two places that must agree: the
 * resolver, and the guard that checks a stored id against the registry.
 */
export const DEFAULT_THEME_ID: ThemeId = 'default-dark'

/** Every id, in dropdown order. Recomputed against `state.toml` by check-consistency.sh §9. */
export const THEME_IDS = Object.keys(THEMES) as ThemeId[]

/**
 * Whether a value is a usable theme id.
 *
 * Deliberately a type guard over the registry rather than a cast: a stored id
 * comes from a file a user can edit, so "is this a theme" is a question with a
 * real answer, not an assumption.
 */
export const isThemeId = (value: unknown): value is ThemeId =>
  typeof value === 'string' && Object.hasOwn(THEMES, value)

/**
 * Resolve a stored id to a theme. Never throws, never returns nothing.
 *
 * An absent, empty or unknown id resolves to `default-dark`. This is the same
 * rule the subscription union follows (`FRONTEND.md` §4, never invent a state):
 * a theme id is decoration, and a decoration that cannot be read degrades to the
 * default rather than blanking the screen or throwing.
 *
 * **Do not "simplify" this to `THEMES[id]`.** The id comes from a file a user can
 * edit by hand, so an unknown value is a real possibility rather than a
 * programming error — and `THEMES[id]` would return `undefined`, which then
 * surfaces as a crash several frames later with nothing pointing back here. The
 * registry test pins the fallback by asserting `resolveTheme({})`, `undefined`,
 * `null` and a nonsense string all return `DEFAULT_THEME_ID`.
 */
export const resolveTheme = (id: unknown): ThemeSpec =>
  isThemeId(id) ? THEMES[id] : THEMES[DEFAULT_THEME_ID]

/**
 * The legacy `defaultTheme` / `defaultDarkTheme` *shape*, projected from a theme.
 *
 * `use-custom-theme` builds the MUI palette from a `{ primary_color, ... }`
 * object whose fields are all optional and all overridden by
 * `theme_setting` (`setting.X || dt.X`). That precedence is the shipped
 * behaviour and is deliberately unchanged -- what changes is only *which base*
 * it falls back to: the resolved theme instead of a fixed light/dark pair.
 *
 * This exists as a function rather than by widening `ThemeSpec` because the two
 * are different things: `ThemeSpec` is the registry's own vocabulary (names a
 * theme author writes), and this is the MUI-shaped projection the hook consumes.
 * Keeping them separate means a theme author never has to know that MUI wants a
 * `background_color` where the registry says `background`.
 *
 * `font_family` is deliberately not a *theme* field -- it is a preference, not a
 * look, and no theme in the registry sets one. It is still on this object
 * because the hook reads it from the same base as everything else and falls back
 * to the built-in stack when `theme_setting.font_family` is unset, so dropping it
 * would change behaviour for an unthemed app. It is passed through from the
 * shipped defaults, unchanged.
 */
export const themeDefaultFor = (
  spec: ThemeSpec,
  fontFamily: string,
) => ({
  primary_color: spec.palette.accent,
  secondary_color: spec.palette.secondary,
  primary_text: spec.palette.textPrimary,
  secondary_text: spec.palette.textSecondary,
  info_color: spec.palette.info,
  error_color: spec.palette.error,
  warning_color: spec.palette.warning,
  success_color: spec.palette.success,
  background_color: spec.palette.background,
  font_family: fontFamily,
})
