import { LOCUS_COLORS, LOCUS_LIGHT } from '@/pages/_theme'

/**
 * The theme registry — the named looks a student picks between.
 *
 * **Every colour in this file is documented in `/COLOURS.md` §§3–4**, which is
 * also where the rationale for each theme and each decoration preset lives. This
 * file is the data; do not restate a value or a design note here.
 *
 * `docs/reference/THEMES.md` owns the *layer's* mechanics: what a theme may and
 * may not change, how an id resolves, and how to add one.
 *
 * `check-consistency.sh` §9 recomputes the id list from `THEMES` below and fails
 * when it disagrees with `docs/state.toml`, so a theme cannot be added in one
 * place.
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
 * The colours a theme owns. Deliberately the *whole* palette rather than a
 * delta against a base: a theme that inherits most of its values from another
 * theme cannot be read on its own.
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
 * A named decoration preset: `short` declarations on the skin root, plus
 * ordered `layers` painted over them. Each is a declaration block, never a
 * selector. Values: `/COLOURS.md` §4; the safety rules: `THEMES.md` §1.
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
 * The decoration presets, scoped to `[data-theme-skin]`.
 *
 * Every technique here is painted by the compositor and needs no asset, so the
 * layer is additive: `use-custom-theme` writes this into one
 * `<style id="locus-theme-decoration">` element, and removing that element
 * removes every trace. What each one paints: `/COLOURS.md` §4.
 */
export const DECORATIONS: Record<DecorationId, DecorationSpec> = {
  'forest-glow': {
    short: `
      background-image:
        radial-gradient(
          ellipse 120% 60% at 50% -10%,
          rgba(153, 153, 153, 0.10),
          rgba(153, 153, 153, 0) 70%
        );
      background-repeat: no-repeat;
    `,
    layers: [],
  },

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

/** The shipped dark appearance: a projection of `LOCUS_COLORS`, not a copy. */
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
    secondary: '#A6A6A6',
    info: '#D0D0D0',
  },
  shape: DEFAULT_SHAPE,
}

/** The shipped light appearance: `LOCUS_LIGHT` grounds plus the light roles. */
const defaultLight: ThemeSpec = {
  id: 'default-light',
  labelKey: 'home.components.connection.account.themeDefaultLight',
  mode: 'light',
  palette: {
    background: LOCUS_LIGHT.background,
    surface: LOCUS_LIGHT.surface,
    surfaceHover: LOCUS_LIGHT.surfaceHover,
    border: LOCUS_LIGHT.border,
    textPrimary: '#151515',
    textSecondary: '#565656',
    accent: LOCUS_LIGHT.accent,
    accentHover: LOCUS_LIGHT.accentHover,
    success: '#151515',
    warning: '#151515',
    error: '#151515',
    secondary: '#565656',
    info: '#565656',
  },
  shape: DEFAULT_SHAPE,
}

/**
 * The registry. Order here is the order of the dropdown.
 *
 * Read it as a table: every entry is a full, self-contained look. Palette values
 * and the rationale for each entry: `/COLOURS.md` §3.
 */
export const THEMES: Record<ThemeId, ThemeSpec> = {
  'default-dark': defaultDark,
  'default-light': defaultLight,

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
   * `moss` — neutral since the 2026-11-11 reset; keeps the larger card radius.
   */
  moss: {
    id: 'moss',
    labelKey: 'home.components.connection.account.themeMoss',
    mode: 'dark',
    palette: {
      background: '#1E1E1E',
      surface: '#2A2A2A',
      surfaceHover: '#333333',
      border: '#3D3D3D',
      textPrimary: '#E6E6E6',
      textSecondary: '#ABABAB',
      accent: '#C8C8C8',
      accentHover: '#DCDCDC',
      success: '#C8C8C8',
      warning: '#B4B4B4',
      error: '#A6A6A6',
      secondary: '#ABABAB',
      info: '#B4B4B4',
    },
    shape: { cardRadius: 14, controlRadius: 8 },
    decoration: 'forest-glow',
  },

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
 *
 * `font_family` is deliberately not a *theme* field -- it is a preference, not a
 * look, and no theme in the registry sets one.
 */
export const themeDefaultFor = (spec: ThemeSpec, fontFamily: string) => ({
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
