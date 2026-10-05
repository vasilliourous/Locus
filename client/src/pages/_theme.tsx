import getSystem from '@/utils/get-system'

const OS = getSystem()

/**
 * Locus's colour system.
 *
 * These are the **defaults**, not a hard override: `use-custom-theme` reads each
 * field as `setting.X || dt.X`, so a student who has customised a colour keeps
 * it. Changing this file changes what a fresh install — and anyone who never
 * opened the old colour picker — sees.
 *
 * # Where these values come from
 *
 * `docs/archive/UI-AESTHETICS.md` defines the brand: a dark green-black theme
 * with Locus green as the single accent. That spec was written for the retired
 * client and never ported, which is why this fork still shipped Clash Verge Rev's
 * iOS blue (`#007AFF`) and grey (`#2E303D`) — an identity belonging to a
 * different product, on every screen a student sees.
 *
 * # The one departure from the spec
 *
 * The spec defines **only** the dark theme; it was written for a client that
 * shipped dark-only. This fork has a light mode, so the light palette below is
 * derived rather than quoted: the same green accent and the same structural
 * relationships, inverted for a light surface. Documented as an addition so
 * nobody later mistakes it for part of the original spec.
 *
 * # The 3.3.0 palette pass — what changed and why
 *
 * The values were re-derived against three rules, and the guards in
 * `client/tests/theme-colors.test.ts` now assert all three:
 *
 * 1. **No pure white and no pure black.** `#F4F8F5`/`#FFFFFF` and `#06130C`
 *    were the old light and dark grounds. Pure white on a full-window panel is
 *    a light source rather than a surface — it glares, and it makes text halo
 *    for exactly the people who need a high-legibility theme. Pure black
 *    crushes the surface ladder: at `#06130C` the card sat a 1.008 contrast
 *    ratio above the page, which is a different colour on paper and the same
 *    flat plane on screen.
 * 2. **The surface ladder must be visible.** Every theme now keeps its card a
 *    deliberate step above its page (the registry guard asserts >= 1.03), so the
 *            1px border reinforces separation instead of doing all of it alone.
 * 3. **The accent must carry text and the logo.** The accent is used for
 *    `button` labels and now paints the wordmark, so it clears 3:1 against both
 *    the page and the surface in every theme — asserted, not hoped for.
 *
 * # Why the light accent is darker than the dark one
 *
 * The dark accent (`#4FBF84`) on a dark ground is comfortable, but the same
 * green on a light ground is roughly 1.9:1 — unusable for body copy. Light mode
 * therefore uses a genuinely darker green (`#2A6E4C`, ~5.6:1 on its surface)
 * for anything textual. Using one green for both would have meant either failing
 * contrast in light mode or dulling the brand in dark, which is the mode that
 * matters most.
 */

/** Locus green, and the surfaces it sits on. Single source for the brand. */
export const LOCUS_COLORS = {
  /**
   * The window. A dark slate with a green cast, not a near-black.
   *
   * This was `#06130C`, which measured L=0.003 — close enough to black that the
   * page and the cards had almost nothing to separate them (a 1.008 contrast
   * step), and harsh on an OLED panel at night. The hue is unchanged in spirit
   * (green, ~h158) but the value is lifted into the comfortable band, which is
   * what gives the surface ladder somewhere to go.
   */
  background: '#0D1512',
  /**
   * Cards and panels: one visible step up from the window.
   *
   * The step is the point. `#0C1711` was a 1.008 contrast ratio against the old
   * background — technically a different colour, visually the same flat plane.
   * A card that cannot be seen does no work, and the 1px border was left
   * carrying separation it should only be reinforcing.
   */
  surface: '#1B2A24',
  /** Hover state for interactive surfaces. */
  surfaceHover: '#22332C',
  /** Dividers, input borders. */
  border: '#2C3F35',
  /** Body text on the dark surface. */
  textPrimary: '#E4EDE7',
  /** Labels and hints. */
  textSecondary: '#9BAFA3',
  /** The brand accent. Buttons, active indicators. */
  accent: '#4FBF84',
  /** Accent hover. */
  accentHover: '#6ED29C',
  /** Connected. */
  success: '#4FBF84',
  /** Disconnected, failures. */
  error: '#D97070',
  /** Connecting, degraded, renewal warnings. */
  warning: '#D9A94E',
} as const

/**
 * Light-mode surfaces.
 *
 * Derived, not quoted — see the note above. The greens keep their relationship
 * to the surfaces (surface lightest, border a visible step down) so the layout
 * reads identically in both modes.
 *
 * **The surface is deliberately not `#FFFFFF`.** It was, and it was the sharpest
 * thing in the app: a card at L=1.000 against an already-bright page reads as a
 * light source rather than a panel, and on a large window at night it is
 * genuinely uncomfortable. `#F2F6F4` is an off-white with the same green cast as
 * the dark theme, which keeps the two modes recognisably the same product.
 */
export const LOCUS_LIGHT = {
  background: '#EDF1EF',
  surface: '#F2F6F4',
  surfaceHover: '#DFE6E2',
  border: '#C3D0C9',
  accent: '#2A6E4C',
  accentHover: '#215839',
} as const

/** The font stack. System fonts: they load instantly and look native. */
const fontFamily = `-apple-system, BlinkMacSystemFont,"Microsoft YaHei UI", "Microsoft YaHei", Roboto, "Helvetica Neue", Arial, sans-serif, "Apple Color Emoji"${
  OS === 'windows' ? ', twemoji mozilla' : ''
}`

export const defaultTheme = {
  // Locus light. The accent is the darker green, because this value is used for
  // text-bearing controls (buttons, links) where `#2EA86A` on white is too thin.
  primary_color: LOCUS_LIGHT.accent,
  secondary_color: '#5B8C6F',
  primary_text: '#0B1F14',
  secondary_text: '#4A6356',
  info_color: '#2563EB',
  error_color: '#C2362B',
  warning_color: '#B45309',
  success_color: '#1E7A4A',
  background_color: LOCUS_LIGHT.background,
  font_family: fontFamily,
}

export const defaultDarkTheme = {
  primary_color: LOCUS_COLORS.accent,
  secondary_color: '#5BBF8E',
  primary_text: LOCUS_COLORS.textPrimary,
  secondary_text: LOCUS_COLORS.textSecondary,
  info_color: '#5AA9E6',
  error_color: LOCUS_COLORS.error,
  warning_color: LOCUS_COLORS.warning,
  success_color: LOCUS_COLORS.success,
  background_color: LOCUS_COLORS.background,
  font_family: fontFamily,
}

/**
 * Tier identity, from the spec's badge table.
 *
 * Colours are per-tier so the tier "sells itself" without extra UI. Kept beside
 * the palette because they are brand colours with the same rules, and because a
 * tier added on the hub must be given a colour in exactly one place.
 */
export const TIER_COLORS: Record<
  string,
  { color: string; background: string }
> = {
  // The merged paid tier. The KEY is `strike` because that is the frozen wire
  // name / `tier_configs` row the hub resolves a paid code against; the LABEL
  // shown to the student is "Full" and lives in `tier-badge.tsx`.
  strike: { color: '#EAB308', background: 'rgba(234, 179, 8, 0.20)' },
  // The free tier. `eco` is retained beside `free` because a code in the field
  // may still carry the legacy string; both render as the same Free plan.
  free: { color: '#7FB48F', background: 'rgba(127, 180, 143, 0.18)' },
  eco: { color: '#7FB48F', background: 'rgba(127, 180, 143, 0.18)' },
}

/** The fallback for a tier this build does not know, so an unknown tier renders. */
export const TIER_FALLBACK = {
  color: '#8CA596',
  background: 'rgba(140, 165, 150, 0.18)',
}
