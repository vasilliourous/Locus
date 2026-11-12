import type { SxProps, Theme } from '@mui/material'

import { LOCUS_COLORS } from '@/pages/_theme'

/**
 * Shared surface styling, so every card in the app is the same object.
 *
 * A card is a one-step-up surface with a 1px border and a radius. MUI's shadow
 * scale is zeroed app-wide (`shadows: Array(25).fill('none')` in
 * `use-custom-theme`), so a card that relied on `elevation` to separate itself
 * would be invisible against the page; the border is what does the work.
 *
 * Defined once because the alternative is what this fork had: each screen
 * hand-rolling its own `sx`, which is how the old app ended up with upstream
 * greys in some places and brand colours in others.
 *
 * The radius is a variable and `12px` is the fallback: it is what an unthemed
 * app renders, and what renders in the window before `use-custom-theme` has run,
 * so a card cannot flash a different shape on startup.
 *
 * Colours: `/COLOURS.md` §2.
 */
export const cardSx = (theme: Theme): SxProps<Theme> => ({
  borderRadius: `var(--card-radius, 12px)`,
  border: `1px solid ${
    theme.palette.mode === 'light' ? theme.palette.divider : LOCUS_COLORS.border
  }`,
  bgcolor: theme.palette.background.paper,
})

/**
 * A card with a coloured accent, for state-bearing panels.
 *
 * Used by the Connection screen's status panel, where the border and wash carry
 * the connection state. Colours: `/COLOURS.md` §2.
 */
export const accentCardSx = (theme: Theme, accent: string): SxProps<Theme> => ({
  borderRadius: `var(--card-radius, 12px)`,
  border: `1px solid ${accent}47`, // ~28% alpha
  bgcolor: `${accent}0A`, // ~4% alpha
})

/** Monospace, for identifiers a student may be asked to read aloud or type. */
export const monoFontFamily =
  'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace'
