import {
  ErrorOutlineRounded,
  ScheduleOutlined,
  WarningAmberRounded,
} from '@mui/icons-material'
import { Box, Tooltip, Typography, alpha, useTheme } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { useSubscription } from '@/hooks/use-subscription'

/**
 * A permanent, unmissable indicator of when the subscription ends.
 *
 * # Why this exists
 *
 * Expiry was only ever visible on the Account screen — a page a student has no
 * reason to open once they are connected. So a code could lapse with the student
 * seeing nothing until the tunnel simply stopped, which reads as "the app is
 * broken" rather than "your subscription ended". Renewal is a business fact, and
 * a business fact the customer cannot see is one they cannot act on.
 *
 * # Why the sidebar
 *
 * It sits in the sidebar next to the traffic readouts because that chrome is on
 * screen on every page, at every window size, without the student navigating
 * anywhere. That is the difference between "the information exists" and "the
 * student is aware of it".
 *
 * # When it renders nothing
 *
 * Deliberately silent in exactly one case:
 *
 *   - **not activated, with no refusal on record** — there is no subscription
 *     to describe and nothing has happened to this device.
 *
 * It used to be silent for `unknown` as well, and that was wrong: `unknown` also
 * covers "the hub has never confirmed a date on a device that is activated", and
 * a student whose status could not be read saw *nothing at all* — no way to tell
 * a working subscription from an unconfirmed one, which is the "no way to check
 * my status" report. A device that is activated, or that the hub has refused,
 * always has something worth saying, so it always says it.
 *
 * A comfortable, far-off expiry is shown quietly (grey); the last week turns it
 * amber; a lapsed code turns it red; a refusal is red with the hub's own words.
 * The urgency steps are the same constant the Account screen and the connect gate
 * use, so the three cannot disagree.
 */
export const LayoutSubscription = () => {
  const { t } = useTranslation()
  const theme = useTheme()
  const { subscription, activated } = useSubscription()

  // The only silent case. A refusal arrives with `activated: false` (the
  // entitlement was withdrawn), so it must be checked before this returns.
  if (!activated && subscription.state !== 'refused') return null

  const { colour, icon, label, tooltip } = (() => {
    if (subscription.state === 'refused') {
      return {
        colour: theme.palette.error.main,
        icon: <ErrorOutlineRounded sx={{ fontSize: 14 }} />,
        // The hub's own sentence is the answer, so it is the label rather than a
        // paraphrase of it. Truncated by the chip's width, fully readable on
        // hover, and shown in full on the Account screen.
        label: subscription.reason,
        tooltip: subscription.reason,
      }
    }

    if (subscription.state === 'lapsed') {
      return {
        colour: theme.palette.error.main,
        icon: <ErrorOutlineRounded sx={{ fontSize: 14 }} />,
        label: t('layout.components.subscription.expired'),
        tooltip: t('layout.components.subscription.expiredHint'),
      }
    }

    // Activated, but no date confirmed. Says so instead of vanishing.
    if (subscription.state === 'unknown') {
      return {
        colour: theme.palette.text.secondary,
        icon: <ScheduleOutlined sx={{ fontSize: 14 }} />,
        label: t('layout.components.subscription.unconfirmed'),
        tooltip: t('layout.components.subscription.unconfirmedHint'),
      }
    }

    // Active, with a known whole-day count.
    const { daysRemaining, urgent, expiresAt } = subscription
    const label =
      daysRemaining <= 0
        ? t('layout.components.subscription.expiresToday')
        : daysRemaining === 1
          ? t('layout.components.subscription.oneDayLeft')
          : t('layout.components.subscription.daysLeft', {
              count: daysRemaining,
            })

    return {
      colour: urgent
        ? theme.palette.warning.main
        : theme.palette.text.secondary,
      icon: urgent ? (
        <WarningAmberRounded sx={{ fontSize: 14 }} />
      ) : (
        <ScheduleOutlined sx={{ fontSize: 14 }} />
      ),
      label,
      // The exact date the hub sent, for a hover. Never a locally-derived one.
      tooltip: expiresAt || t('layout.components.subscription.hint'),
    }
  })()

  return (
    <Tooltip title={tooltip} arrow placement="top">
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: 0.75,
          mx: 'auto',
          mb: 0.75,
          px: 1,
          py: 0.5,
          maxWidth: 250,
          borderRadius: 1.5,
          // A tinted chip, not bare text: it has to read as a standing notice
          // rather than as part of the traffic figures above it.
          bgcolor: alpha(colour, theme.palette.mode === 'light' ? 0.12 : 0.18),
          color: colour,
          cursor: 'default',
          userSelect: 'none',
        }}
      >
        {icon}
        <Typography
          component="span"
          sx={{
            fontSize: 11,
            fontWeight: 600,
            lineHeight: 1.2,
            whiteSpace: 'nowrap',
            // A hub refusal is a full sentence, not a two-word badge, so it is
            // clipped rather than allowed to widen the sidebar past its
            // neighbour. The tooltip and the Account screen both show it whole,
            // so nothing is lost — only the standing chip is kept to size.
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            minWidth: 0,
          }}
        >
          {label}
        </Typography>
      </Box>
    </Tooltip>
  )
}
