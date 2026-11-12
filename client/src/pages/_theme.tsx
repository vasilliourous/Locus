import getSystem from '@/utils/get-system'

const OS = getSystem()

/**
 * The Locus palette. **Every value here is documented in `/COLOURS.md` §2**,
 * with the contrast rule each one satisfies and the state the palette is in.
 * Do not restate a value or a rationale in this file; change it there first.
 *
 * These are defaults, not overrides: `use-custom-theme` reads each field as
 * `setting.X || dt.X`, so a student who has customised a color keeps it.
 */

/** The dark appearance. Single source for `default-dark`. */
export const LOCUS_COLORS = {
  background: '#141414',
  surface: '#202020',
  surfaceHover: '#2A2A2A',
  border: '#4A4A4A',
  textPrimary: '#E8E8E8',
  textSecondary: '#A6A6A6',
  accent: '#C4C4C4',
  accentHover: '#DEDEDE',
  success: '#E8E8E8',
  error: '#D6D6D6',
  warning: '#C8C8C8',
} as const

/** The light appearance. Single source for `default-light`'s grounds. */
export const LOCUS_LIGHT = {
  background: '#ECECEC',
  surface: '#F7F7F7',
  surfaceHover: '#E6E6E6',
  border: '#C0C0C0',
  accent: '#4C4C4C',
  accentHover: '#404040',
} as const

const fontFamily = `-apple-system, BlinkMacSystemFont,"Microsoft YaHei UI", "Microsoft YaHei", Roboto, "Helvetica Neue", Arial, sans-serif, "Apple Color Emoji"${
  OS === 'windows' ? ', twemoji mozilla' : ''
}`

/** The legacy MUI-shaped projection for light mode. */
export const defaultTheme = {
  primary_color: LOCUS_LIGHT.accent,
  secondary_color: '#565656',
  primary_text: '#151515',
  secondary_text: '#565656',
  info_color: '#565656',
  error_color: '#151515',
  warning_color: '#151515',
  success_color: '#151515',
  background_color: LOCUS_LIGHT.background,
  font_family: fontFamily,
}

/** The legacy MUI-shaped projection for dark mode. */
export const defaultDarkTheme = {
  primary_color: LOCUS_COLORS.accent,
  secondary_color: '#A6A6A6',
  primary_text: LOCUS_COLORS.textPrimary,
  secondary_text: LOCUS_COLORS.textSecondary,
  info_color: '#D0D0D0',
  error_color: LOCUS_COLORS.error,
  warning_color: LOCUS_COLORS.warning,
  success_color: LOCUS_COLORS.success,
  background_color: LOCUS_COLORS.background,
  font_family: fontFamily,
}

/**
 * Tier badge colours. The KEYS are frozen wire names; see `/COLOURS.md` §6.
 *
 * The three rungs step in VALUE, not hue (the badge is monochrome): Full is
 * brightest, Budget sits between it and Free, Free is dimmest. A tier with no
 * entry renders through `TIER_FALLBACK` — correct, just generic — which is what
 * a shipped build does for a row added after it. That is why a new tier must be
 * added here in the same change that seeds it: otherwise the first students on
 * it see a grey badge reading the bare wire name.
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
