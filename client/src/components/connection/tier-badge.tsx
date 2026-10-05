import { Box, Typography } from '@mui/material'
import { useTheme } from '@mui/material/styles'

import { TIER_COLORS, TIER_FALLBACK } from '@/pages/_theme'

/**
 * The tier badge.
 *
 * From `docs/archive/UI-AESTHETICS.md` §7 — the "tier sells itself" principle:
 * a student on the paid plan should be able to see that without opening a
 * settings page. Each tier gets a colour, a small glyph, and nothing else.
 *
 * The glyph is decorative (`aria-hidden`) because it duplicates the label: a
 * screen reader announcing "lightning bolt full" is worse than "full". The
 * colour is likewise redundant — it reinforces the word rather than carrying
 * meaning on its own, which is the rule the whole palette follows.
 *
 * ## Two tiers, two frozen wire names and their labels
 *
 * The hub sends `tier` as a **frozen wire name** — a code carries the string it
 * was minted with, and the hub resolves it against `tier_configs`. So this
 * component is the ONE place a wire name becomes a student-visible label, and
 * every mapping lives here rather than being spread across call sites:
 *
 *   `eco`     → shown as "Free"   (the legacy slot the free tier reuses)
 *   `free`    → shown as "Free"
 *   `strike`  → shown as "Full"   (the merged paid tier)
 *
 * `strike` deliberately does NOT display as "Strike", and `Full` is not a wire
 * name: renaming the row would strand every paid code in the field. See
 * `docs/business/04-tiers.md` §4.6.
 */

/** The tier identity, including the unknown case. */
const tierIdentity = (tier: string | null | undefined) => {
  const key = (tier ?? '').trim().toLowerCase()
  const glyph = key === 'strike' ? '\u26A1' : '\u25CB'
  const palette = TIER_COLORS[key] ?? TIER_FALLBACK

  // An unknown tier shows the raw name rather than a guessed label: if the hub
  // ever adds a tier this build predates, showing "Full" for it would be a
  // lie, and showing nothing would hide a plan the student is paying for.
  const label =
    key === 'strike'
      ? 'Full'
      : key === 'free' || key === 'eco'
        ? 'Free'
        : (tier ?? '')

  return { glyph, palette, label }
}

interface Props {
  tier: string | null | undefined
  /** Rendered smaller, for inline use beside a status line. */
  compact?: boolean
}

export const TierBadge = ({ tier, compact = false }: Props) => {
  // Reads the palette only for the text colour, so the badge sits correctly on
  // both the dark surface and the light one without a second colour table.
  const theme = useTheme()
  const { glyph, palette, label } = tierIdentity(tier)

  if (!label) return null

  return (
    <Box
      component="span"
      sx={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 0.5,
        px: compact ? 1 : 1.25,
        py: compact ? 0.125 : 0.25,
        borderRadius: '20px',
        bgcolor: palette.background,
        border: `1px solid ${palette.color}40`,
        color: palette.color,
        lineHeight: 1.6,
        verticalAlign: 'middle',
      }}
    >
      <Typography
        component="span"
        aria-hidden="true"
        sx={{ fontSize: compact ? 10 : 11, lineHeight: 1 }}
      >
        {glyph}
      </Typography>
      <Typography
        component="span"
        sx={{
          fontSize: compact ? 11 : 12,
          fontWeight: 600,
          letterSpacing: '0.02em',
          color: 'inherit',
          // `color: inherit` above keeps the tier's own colour rather than the
          // theme's text colour; the `theme` read is what makes this component
          // re-render on a mode change.
          textShadow: theme.palette.mode === 'light' ? 'none' : undefined,
        }}
      >
        {label}
      </Typography>
    </Box>
  )
}

export default TierBadge
