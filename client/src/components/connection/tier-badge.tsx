import { Box, Typography } from '@mui/material'
import { useTheme } from '@mui/material/styles'

import { TIER_COLORS, TIER_FALLBACK } from '@/pages/_theme'

/**
 * The tier badge: a glyph, a label, and a tint. The tint is redundant by
 * design — it reinforces the word rather than carrying meaning, so the badge
 * survives a recolour unchanged. Palette: `/COLOURS.md` §6.
 *
 * The glyph is decorative (`aria-hidden`) because it duplicates the label: a
 * screen reader announcing "lightning bolt full" is worse than "full".
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
 *   `budget`  → shown as "Budget" (the middle rung, added 2026-11-12)
 *   `strike`  → shown as "Full"   (the merged paid tier)
 *
 * `strike` deliberately does NOT display as "Strike", and `Full` is not a wire
 * name: renaming the row would strand every paid code in the field. See
 * `docs/business/04-tiers.md` §4.6.
 *
 * `budget` is the one row whose wire name and label agree. That is a coincidence
 * of the tier, not a rule — `04-tiers.md` §4.2.5 chose the row string
 * deliberately, and it is still frozen: a deployed build that predates this
 * mapping renders the raw string through `TIER_FALLBACK`, which is correct and
 * generic, rather than guessing a label.
 */

/** The tier identity, including the unknown case. */
const tierIdentity = (tier: string | null | undefined) => {
  const key = (tier ?? '').trim().toLowerCase()
  // Full gets the lightning bolt; Budget and Free share the plain ring, because
  // the glyph is decorative reinforcement and two paid rungs must not look like
  // the same thing by icon alone. The LABEL carries the distinction.
  const glyph = key === 'strike' ? '\u26A1' : '\u25CB'
  const palette = TIER_COLORS[key] ?? TIER_FALLBACK

  // An unknown tier shows the raw name rather than a guessed label: if the hub
  // ever adds a tier this build predates, showing "Full" for it would be a
  // lie, and showing nothing would hide a plan the student is paying for.
  const label =
    key === 'strike'
      ? 'Full'
      : key === 'budget'
        ? 'Budget'
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
  // both the dark surface and the light one without a second table.
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
