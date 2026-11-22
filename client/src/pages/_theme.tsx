import getSystem from '@/utils/get-system'

const OS = getSystem()

/**
 * The Locus palette. **Every value here is documented in `/COLOURS.md` §2**,
 * which is the only file allowed to describe or justify one. Do not restate a
 * value or a rationale here; change it there first.
 *
 * # There is exactly one appearance
 *
 * Locus is permanently Dawn. There is no theme registry, no theme id, no
 * light/dark switch and no theme hook — the appearance a student gets is the one
 * constant below, and the decision that made it permanent is recorded in
 * `COLOURS.md` §0. Do not reintroduce a second palette "just for contrast" or
 * "for light mode": the machinery that made a second one possible was deleted on
 * purpose, and re-adding it means re-adding the whole layer.
 *
 * `theme_setting` overrides still apply where the palette reaches MUI (see
 * `theme.ts`), so a student who had set a custom colour keeps it. That is a
 * *preference*, not a theme, and it does not create a second appearance.
 */

/**
 * Dawn's palette — the only one.
 *
 * The field names are the registry's old vocabulary (`background`, `surface`,
 * `accent`, …) rather than MUI's (`primary`, `background.paper`), because this
 * object is the source the MUI theme is *projected* from, and the projection is
 * the one place the two vocabularies meet.
 */
export const LOCUS_COLORS = {
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
export const locusFontFamily = `-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif, "Apple Color Emoji"${
  OS === 'windows' ? ', twemoji mozilla' : ''
}`

/**
 * The MUI-shaped projection of {@link LOCUS_COLORS}.
 *
 * These are the fields `theme.ts` reads: everything else in the MUI palette is
 * derived from them. `theme_setting` overrides each as `setting.X || dt.X` in
 * that module, which is the shipped precedence and is unchanged — a student's own
 * colour wins, and this object is the default it falls back to.
 */
export const defaultTheme = {
  primary_color: LOCUS_COLORS.accent,
  secondary_color: LOCUS_COLORS.secondary,
  primary_text: LOCUS_COLORS.textPrimary,
  secondary_text: LOCUS_COLORS.textSecondary,
  info_color: LOCUS_COLORS.info,
  error_color: LOCUS_COLORS.error,
  warning_color: LOCUS_COLORS.warning,
  success_color: LOCUS_COLORS.success,
  background_color: LOCUS_COLORS.background,
  font_family: locusFontFamily,
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
