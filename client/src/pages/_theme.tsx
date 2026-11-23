import getSystem from '@/utils/get-system'

const OS = getSystem()

/**
 * The Chrysalis palette. **Every value here is documented in `/COLOURS.md` §2**,
 * which is the only file allowed to describe or justify one. Do not restate a
 * value or a rationale here; change it there first.
 *
 * # There is exactly one appearance
 *
 * Chrysalis is permanently Dawn. There is no theme registry, no theme id, no
 * light/dark switch and no theme hook — the appearance a student gets is the one
 * constant below, and the decision that made it permanent is recorded in
 * `COLOURS.md` §0. Do not reintroduce a second palette "just for contrast" or
 * "for light mode": the machinery that made a second one possible was deleted on
 * purpose, and re-adding it means re-adding the whole layer.
 *
 * # There is no runtime input at all
 *
 * `theme_setting` — the per-field override of the palette inherited from Clash
 * Verge Rev — was the last survivor of the layer and was deleted on 2026-11-23.
 * It never had a reader: this file's `defaultTheme` is projected by
 * `services/theme.ts` with no argument, so no stored preference could change a
 * colour. Three documents claimed otherwise, which is what kept it alive.
 *
 * So the values below are the *only* ones a student will ever see, and the answer
 * to "can a user change the palette?" is no, not "not yet".
 */

/**
 * Dawn's palette — the only one.
 *
 * The field names are the registry's old vocabulary (`background`, `surface`,
 * `accent`, …) rather than MUI's (`primary`, `background.paper`), because this
 * object is the source the MUI theme is *projected* from, and the projection is
 * the one place the two vocabularies meet.
 */
export const CHRYSALIS_COLORS = {
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
  /** Secondary/accent-adjacent colour for non-primary emphasis. */
  secondary: '#BB9AF7',
  /** Informational (not a status) accent. */
  info: '#7DCFFF',
} as const

/**
 * The font stack, shared by the MUI theme and the injected `body` rule.
 *
 * **Latin-first.** The inherited stack led with `Microsoft YaHei UI`, a CJK face,
 * which meant a Windows install set every English label in a font chosen for
 * Chinese. It arrived with the Clash Verge fork and nobody decided it.
 *
 * A CJK locale does not lose anything by its absence: the OS fallback chain
 * supplies a CJK face for the glyphs a Latin face lacks. Keeping one in the
 * *primary* position changes Latin rendering, which is the only thing it does.
 *
 * There is no web font here on purpose. The landing page cannot load one (the
 * hub's CSP is `default-src 'none'`), and the app should not depend on a network
 * fetch to render its own chrome. The Twemoji face on Windows is an emoji
 * fallback shipped in `font.scss`, not the UI face.
 */
export const chrysalisFontFamily = `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif, "Apple Color Emoji"${
  OS === 'windows' ? ', twemoji mozilla' : ''
}`

/**
 * The MUI-shaped projection of {@link CHRYSALIS_COLORS}.
 *
 * These are the fields `theme.ts` reads: everything else in the MUI palette is
 * derived from them. The shape is the retired override's vocabulary (it was what
 * `theme_setting` used to produce), kept because `theme.ts` and
 * `tests/theme-colors.test.ts` both read these names — but there is no longer a
 * `setting.X || dt.X` precedence, because there is no `setting.X`. Each field is
 * the value below, always.
 */
export const defaultTheme = {
  primary_color: CHRYSALIS_COLORS.accent,
  secondary_color: CHRYSALIS_COLORS.secondary,
  primary_text: CHRYSALIS_COLORS.textPrimary,
  secondary_text: CHRYSALIS_COLORS.textSecondary,
  info_color: CHRYSALIS_COLORS.info,
  error_color: CHRYSALIS_COLORS.error,
  warning_color: CHRYSALIS_COLORS.warning,
  success_color: CHRYSALIS_COLORS.success,
  background_color: CHRYSALIS_COLORS.background,
  font_family: chrysalisFontFamily,
}

/**
 * Corner radii, written as CSS variables so `_surfaces.ts` can read them.
 *
 * Not a theme field any more — there is one shape. `cardRadius` is what
 * `--card-radius` carries and `controlRadius` what `--control-radius` carries;
 * the values are set once in `theme.ts` and never at runtime.
 */
export const DEFAULT_SHAPE = { cardRadius: 12, controlRadius: 8 } as const

/**
 * Tier badge colours. The KEYS are frozen wire names; see `/COLOURS.md` §6.
 *
 * **Deliberately kept** when the theme layer was removed, and not a second
 * appearance: the badge's tint reinforces a glyph and a label rather than
 * carrying meaning, so it is redundant encoding, not a palette. A tier with no
 * entry renders through `TIER_FALLBACK` — correct, just generic — which is what a
 * shipped build does for a row added after it. That is why a new tier must be
 * added here in the same change that seeds it: otherwise the first students on it
 * see a badge reading the bare wire name.
 */
export const TIER_COLORS: Record<
  string,
  { color: string; background: string }
> = {
  strike: { color: '#FFFFFF', background: 'rgba(255, 255, 255, 0.20)' },
  budget: { color: '#F0F0F0', background: 'rgba(240, 240, 240, 0.19)' },
  free: { color: '#DADADA', background: 'rgba(218, 218, 218, 0.18)' },
  eco: { color: '#DADADA', background: 'rgba(218, 218, 218, 0.18)' },
}

/** The fallback for a tier this build does not know. */
export const TIER_FALLBACK = {
  color: '#BDBDBD',
  background: 'rgba(189, 189, 189, 0.18)',
}
