import { Box, Link, Typography } from '@mui/material'
import type { SxProps, Theme } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { LOCUS_COLORS } from '@/pages/_theme'

/**
 * The support route: the one place a student is told who to email and what to
 * say.
 *
 * # Why this is a component and not copy pasted onto three screens
 *
 * Three screens needed it (activation, connection, account) and each had drifted:
 * the connection page had a bare one-line footer with the address and no
 * instruction, the account page had a hint that named the Device ID but no
 * address, and the activation gate had nothing at all — with a comment saying
 * that was deliberate.
 *
 * Copy that exists three times is copy that will disagree. The two facts that
 * must never disagree are the address and the reply promise: a student who
 * emails an address the app no longer reads, or who was promised a reply in a
 * window the operator no longer holds, is worse off than one who was told
 * nothing. So both live in the locale bundle exactly once and are rendered by
 * this component.
 *
 * # Why it takes a palette rather than reading the theme
 *
 * The activation gate renders OUTSIDE `ThemeModeProvider` (see `main.tsx`): it is
 * the pre-activation screen, so it cannot read the app theme and gets a bare
 * `createTheme()`. Reading `text.secondary` there would paint the LIGHT theme's
 * grey on a dark page — the white-on-white class of defect this project already
 * has a story about. `palette: 'brand'` selects the fixed brand constants that
 * screen is already written against; the default follows the app theme, light
 * mode included.
 *
 * # What it is telling the student, and in what order
 *
 * 1. The middleman first, because they can usually fix it on the spot and they
 *    hold the paper trail between the student and the code.
 * 2. The address, as a `mailto:` action — "copy this out of a rendered label"
 *    produces mistyped addresses and support emails that never arrive.
 * 3. What to include, because "describe the problem" alone produces reports with
 *    no error message in them, which is the report that cannot be acted on.
 * 4. The reply promise, so silence is not read as being ignored.
 * 5. One clause of why, because a student who understands it is one person
 *    answering email is far more likely to send the version that can be fixed.
 */
export const SupportBlock = ({
  /** Hide the "ask your middleman first" line where it does not apply. */
  showMiddleman = true,
  /** Show the Device ID instruction. On where a student can actually see it. */
  showDeviceIdHint = false,
  /** `brand` = fixed dark constants, for screens outside the theme provider. */
  palette = 'theme',
  sx,
}: {
  showMiddleman?: boolean
  showDeviceIdHint?: boolean
  palette?: 'theme' | 'brand'
  sx?: SxProps<Theme>
}) => {
  const { t } = useTranslation()

  const brand = palette === 'brand'
  const strongest = brand ? LOCUS_COLORS.textPrimary : 'text.primary'
  const body = brand ? LOCUS_COLORS.textSecondary : 'text.secondary'
  const accent = brand ? LOCUS_COLORS.accent : 'primary.main'

  return (
    <Box
      sx={[
        {
          boxSizing: 'border-box',
          width: '100%',
          textAlign: 'left',
          px: 1.75,
          py: 1.5,
          borderRadius: 1.5,
          border: '1px solid',
          borderColor: brand ? LOCUS_COLORS.border : 'divider',
          bgcolor: brand ? LOCUS_COLORS.surface : 'background.paper',
        },
        ...(Array.isArray(sx) ? sx : [sx]),
      ]}
    >
      <Typography
        variant="body2"
        sx={{ color: strongest, fontWeight: 600, mb: 0.75, lineHeight: 1.5 }}
      >
        {t('home.components.connection.supportBlock.title')}
      </Typography>

      {showMiddleman && (
        <Typography
          variant="caption"
          sx={{ display: 'block', color: body, lineHeight: 1.6 }}
        >
          {t('home.components.connection.supportBlock.middleman')}
        </Typography>
      )}

      <Typography
        variant="caption"
        sx={{ display: 'block', mt: 0.75, color: body, lineHeight: 1.6 }}
      >
        {t('home.components.connection.supportBlock.emailLead')}{' '}
        <Link
          href={`mailto:${t('home.components.connection.supportEmail')}`}
          sx={{
            color: accent,
            fontWeight: 600,
            textDecoration: 'underline',
            textUnderlineOffset: '2px',
            wordBreak: 'break-word',
          }}
        >
          {t('home.components.connection.supportEmail')}
        </Link>{' '}
        {t('home.components.connection.supportBlock.emailNote')}
      </Typography>

      {showDeviceIdHint && (
        <Typography
          variant="caption"
          sx={{ display: 'block', mt: 0.5, color: body, lineHeight: 1.6 }}
        >
          {t('home.components.connection.supportBlock.deviceIdHint')}
        </Typography>
      )}

      {/* The promise. Its own line and a weight above the surrounding hint text,
          because it is the sentence that decides whether a student bothers to
          write at all. */}
      <Typography
        variant="caption"
        sx={{
          display: 'block',
          mt: 0.75,
          color: strongest,
          fontWeight: 600,
          lineHeight: 1.6,
        }}
      >
        {t('home.components.connection.supportBlock.replyPromise')}
      </Typography>

      <Typography
        variant="caption"
        sx={{ display: 'block', mt: 0.25, color: body, lineHeight: 1.5 }}
      >
        {t('home.components.connection.supportBlock.why')}
      </Typography>
    </Box>
  )
}
