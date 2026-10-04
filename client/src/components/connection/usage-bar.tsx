import { Box, LinearProgress, Typography } from '@mui/material'
import { useTranslation } from 'react-i18next'

import type { AllowanceStatus } from '@/services/locus'

import { usageDisplay, usageSummary } from './usage-bar-model'

/**
 * The free tier's usage bar.
 *
 * The decided behaviour (`docs/business/04-tiers.md` §4.4.4) is that the free
 * tier's constraint is **visible at all times**: a student who is being throttled
 * must know why and know what fixes it. A silently slow app reads as broken; a
 * visibly throttled one reads as a working free tier with a clear upgrade.
 *
 * Renders **nothing** for a paying tier — `usageDisplay` returns `none` for
 * `unlimited`, and that is the point of the distinction: a Strike student must
 * never see a data bar.
 *
 * The colours are the theme's own warning/error roles rather than literal
 * values, so the bar follows a theme change without a second palette table.
 */
export const UsageBar = ({ allowance }: { allowance: AllowanceStatus }) => {
  const { t } = useTranslation()
  const display = usageDisplay(allowance)
  if (display.kind === 'none') return null

  const { percent, warning, throttled, usedLabel, allowanceLabel } = display

  // Warn and full are the theme's roles, not this component's invention: they
  // already carry the correct contrast on both the light and dark surfaces.
  const color = throttled ? 'error' : warning ? 'warning' : 'primary'

  return (
    <Box sx={{ width: '100%' }} data-testid="usage-bar">
      <Box
        sx={{
          display: 'flex',
          alignItems: 'baseline',
          justifyContent: 'space-between',
          gap: 1,
          mb: 0.5
        }}
      >
        <Typography variant="caption" sx={{ fontWeight: 600, letterSpacing: '0.02em' }}>
          {t('home.components.connection.freeData')}
        </Typography>
        <Typography variant="caption" color="text.secondary">
          {usedLabel} / {allowanceLabel}
        </Typography>
      </Box>

      <LinearProgress
        variant="determinate"
        value={percent}
        color={color}
        // The bar is a redundant encoding of the sentence below it, so it is
        // hidden from assistive tech: announcing "progress bar 100 percent" then
        // the sentence would say the same thing twice. The sentence carries the
        // meaning; the bar carries the glance.
        aria-hidden="true"
        sx={{
          height: 6,
          borderRadius: '3px',
          '& .MuiLinearProgress-bar': { borderRadius: '3px' }
        }}
      />

      {/* The sentence, in the live region, because it is the accessible form of
          the bar. `polite` rather than `assertive`: it changes on a poll, and
          interrupting a screen reader mid-sentence to say "4.3 of 5.0 GB" is
          worse than saying it when the reader next pauses. */}
      <Typography
        variant="caption"
        color={throttled ? 'error' : warning ? 'warning.main' : 'text.secondary'}
        role="status"
        aria-live="polite"
        sx={{ display: 'block', mt: 0.5 }}
      >
        {usageSummary(display, {
          used: t('home.components.connection.freeDataUsed'),
          nearlyOut: t('home.components.connection.freeDataNearlyOut'),
          throttled: t('home.components.connection.freeDataThrottled')
        })}
      </Typography>

      {/* The upgrade route, shown ONLY while throttled.
          This is the one moment the student has a reason to read it: a free user
          under their allowance does not need to be sold anything, and a
          permanent advert on a working free tier is the thing that makes a free
          tier feel like an advert rather than a product.

          It deliberately names the ACTION rather than offering a purchase. There
          is no self-serve checkout — a code is a physical card bought from a
          middleman (docs/business/08-distribution.md), so a "Buy now" button
          would have nowhere to go. The copy points at the person who can
          actually sell them the upgrade, which is what the rest of the app does
          for renewals too. */}
      {throttled && (
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', mt: 0.25, lineHeight: 1.5 }}
        >
          {t('home.components.connection.freeDataThrottledUpgrade')}
        </Typography>
      )}
    </Box>
  )
}

export default UsageBar
