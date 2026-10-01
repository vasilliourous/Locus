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

/** The closed set of decorative presets. Adding one is a code change. */
export type DecorationId = 'forest-glow'

export type ThemeId =
  | 'default-dark'
  | 'default-light'
  | 'midnight'
  | 'paper'
  | 'high-contrast'
  | 'forest'

/**
 * The decoration presets.
 *
 * Each is a self-contained block scoped to `[data-theme-skin]`, which is set on
 * the layout root only while that theme is active. Nothing here targets a
 * component, a class, or the document: a preset that could reach outside the
 * skin attribute would defeat the point of naming presets at all.
 *
 * `radial-gradient` is deliberately the only technique used. It is painted by
 * the compositor, needs no asset, and cannot shift layout — a decorative layer
 * that could change the size of anything is a decoration that can move the
 * Connect button, which is the one thing this app must not do.
 */
export const DECORATIONS: Record<DecorationId, string> = {
  'forest-glow': `
    background-image:
      radial-gradient(
        ellipse 120% 60% at 50% -10%,
        rgba(46, 168, 106, 0.16),
        rgba(46, 168, 106, 0) 70%
      );
    background-repeat: no-repeat;
  `,
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
    secondary: '#5BBF8E',
    info: '#5AA9E6',
  },
  shape: DEFAULT_SHAPE,
}

/**
 * `default-light` — the shipped light appearance.
 *
 * The accent is the *darker* green because this value is used for text-bearing
 * controls, where `#2EA86A` on white is ~3.0:1 and fails the body-copy rule.
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
    textPrimary: '#0B1F14',
    textSecondary: '#4A6356',
    accent: LOCUS_LIGHT.accent,
    accentHover: LOCUS_LIGHT.accentHover,
    success: '#1E7A4A',
    warning: '#B45309',
    error: '#C2362B',
    secondary: '#5B8C6F',
    info: '#2563EB',
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
   * `midnight` — true black, for OLED panels and dark rooms.
   *
   * The accent is *lifted* (`#3FBF7F`) rather than the shipped green: on pure
   * black the shipped `#2EA86A` reads dimmer than it does on green-black, and
   * the accent has to stay the focal point. The border is lighter than the
   * surfaces by more than the default theme's, because on black the border is
   * doing all of the surface-separation work.
   *
   * Not the default: pure black loses the surface hierarchy the brand is built
   * on, and a card must still be visible against the page.
   */
  midnight: {
    id: 'midnight',
    labelKey: 'home.components.connection.account.themeMidnight',
    mode: 'dark',
    palette: {
      background: '#000000',
      surface: '#0A0A0A',
      surfaceHover: '#141414',
      border: '#2A2A2A',
      textPrimary: '#F2F2F2',
      textSecondary: '#9E9E9E',
      accent: '#3FBF7F',
      accentHover: '#5CD396',
      success: '#34D399',
      warning: '#FBBF24',
      error: '#F87171',
      secondary: '#6ECFA0',
      info: '#7CC4F2',
    },
    shape: DEFAULT_SHAPE,
  },

  /**
   * `paper` — warm, low-glare light.
   *
   * The difference from `default-light` is **temperature, not brightness**:
   * off-white (`#F6F3EC`) rather than white, and a warm-neutral text colour
   * (`#2B2620`) rather than a green-black one. A warm theme that was merely
   * dimmer than the default would be a worse default, not a distinct look.
   *
   * This is the light theme §5 of the design record names as the one to test the
   * cold-start transition against, because cold-start paints dark for everyone.
   */
  paper: {
    id: 'paper',
    labelKey: 'home.components.connection.account.themePaper',
    mode: 'light',
    palette: {
      background: '#F6F3EC',
      surface: '#FFFFFF',
      surfaceHover: '#EFEAE0',
      border: '#D8D0C0',
      textPrimary: '#2B2620',
      textSecondary: '#6B6152',
      accent: '#1F6B45',
      accentHover: '#175236',
      success: '#1F6B45',
      warning: '#8A5A00',
      error: '#A33226',
      secondary: '#6B7F52',
      info: '#255E8A',
    },
    shape: DEFAULT_SHAPE,
  },

  /**
   * `high-contrast` — accessibility.
   *
   * Maximum legibility: near-black page, near-white text, borders that are meant
   * to be *seen* rather than to separate quietly, and a bright accent. The one
   * theme where "too loud" is the point.
   *
   * Still bound by the structural rules — the surface step and the text
   * hierarchy stay intact — because a high-contrast theme that flattened the
   * hierarchy would be harder to read, not easier.
   */
  'high-contrast': {
    id: 'high-contrast',
    labelKey: 'home.components.connection.account.themeHighContrast',
    mode: 'dark',
    palette: {
      background: '#0A0A0A',
      surface: '#1A1A1A',
      surfaceHover: '#2A2A2A',
      border: '#8A8A8A',
      textPrimary: '#FFFFFF',
      textSecondary: '#D4D4D4',
      accent: '#4ADE80',
      accentHover: '#86EFAC',
      success: '#4ADE80',
      warning: '#FDE047',
      error: '#FCA5A5',
      secondary: '#93C5FD',
      info: '#93C5FD',
    },
    shape: DEFAULT_SHAPE,
  },

  /**
   * `forest` — the character theme, and the decoration seam's proof.
   *
   * Deeper and greener than `default-dark`, with a soft radial wash from the top
   * of the window (`forest-glow`) and larger radii. It exists to exercise the
   * case that would break a layer built only for colour: a theme that changes
   * palette *and* shape *and* decoration at once.
   *
   * Not to be confused with `default-dark` plus a background image: the preset is
   * a fixed, named, non-interactive wash. The user's `background_image` field
   * keeps working independently and is not part of any theme.
   */
  forest: {
    id: 'forest',
    labelKey: 'home.components.connection.account.themeForest',
    mode: 'dark',
    palette: {
      background: '#04170D',
      surface: '#0A2A19',
      surfaceHover: '#113A24',
      border: '#24543A',
      textPrimary: '#E8F5EC',
      textSecondary: '#8FBFA2',
      accent: '#34C47C',
      accentHover: '#57D894',
      success: '#34C47C',
      warning: '#F0B429',
      error: '#F0736A',
      secondary: '#6FD3A0',
      info: '#6FB6D9',
    },
    shape: { cardRadius: 16, controlRadius: 10 },
    decoration: 'forest-glow',
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
