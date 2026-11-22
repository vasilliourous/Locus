import { Box, Typography } from '@mui/material'
import { useTranslation } from 'react-i18next'

import { SupportBlock } from '@/components/shared/support-block'
import type { ChrysalisStatus } from '@/services/chrysalis'

/**
 * The screen an operator's suspension lands on.
 *
 * # Why this is a screen and not a notice
 *
 * A suspension is not a warning about something that might happen — it is the
 * app saying "this device may not be used right now". Every lesser treatment was
 * wrong in a specific way:
 *
 *  * A toast or a badge is missed by a student who is not looking at the window,
 *    which on a background VPN is most of them.
 *  * The activation gate is worse than useless: a suspended device still HOLDS
 *    its code (that is what makes unsuspending work without re-entry), so
 *    offering a code prompt would invite the student to type a code the hub
 *    already knows and already refuses.
 *  * The normal connection screen with a red strip lets them press Connect and
 *    get a tunnel that cannot carry anything, which reads as a broken app rather
 *    than a withdrawn account.
 *
 * # What it deliberately does NOT offer
 *
 * No "sign out", no "try another code", no "retry". The remedy is a conversation
 * with whoever took their money, and the screen's job is to start it while saying
 * so precisely enough that the middleman can act. The one thing that does change
 * this screen is an operator undoing it, which the backend reports and the gate
 * re-reads — the student does not need to do anything, and a "check again" button
 * would suggest otherwise.
 *
 * **A screen with no controls is only safe while the state behind it cannot be a
 * dead end**, and that was not true when this was written: a sign-out of a
 * suspended device left the flag set and the credential gone, so the student
 * arrived here with nothing to press and no way out but wiping the app's data.
 * The guarantee now comes from the state, not from this component — a suspension
 * is only ever true of a device that still holds its code
 * (`docs/reference/SUSPENSION.md` §2), and a device that holds no code is sent to
 * the activation gate instead. If that guarantee is ever weakened this screen
 * becomes a trap again, which is why the invariant is enforced in three places and
 * guarded rather than assumed here.
 *
 * # Why the copy is in the locale bundle
 *
 * It used to be English string literals in this file, which made the one screen
 * an operator's action is announced on the only screen no translator could
 * reach. `main.tsx` mounts the theme provider above this screen as well, so
 * there is no longer any reason — theming and translation were skipped together
 * here, and the two are unrelated.
 */
interface Props {
  /**
   * The status the gate already read, or `null` if it has not settled.
   *
   * Passed in rather than fetched here. This screen used to issue its own
   * `chrysalisStatus()` on mount, which made it a third independent reader of a fact
   * two other surfaces also read — and the one that disagreed was the one the
   * student was looking at. The gate's read is the authoritative one, so this
   * renders from it and owns no state at all: there is nothing here that can go
   * stale relative to the screen that chose to show it.
   */
  status: ChrysalisStatus | null
}

const SuspendedScreen = ({ status }: Props) => {
  const { t } = useTranslation()

  // `state: 'refused'` is the shape the backend uses for every withdrawal of
  // access, and it carries the hub's own sentence. A suspension and a lapse can
  // both reach it in principle; the difference the student cares about is whether
  // they can fix it themselves, which is why 'lapsed' is distinguished.
  const subscription = status?.subscription
  const reason = subscription?.state === 'refused' ? subscription.reason : null
  const lapsed = subscription?.state === 'lapsed'

  return (
    <Box
      sx={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        bgcolor: 'background.default',
        color: 'text.primary',
      }}
    >
      <Box
        sx={{
          width: 460,
          maxWidth: 'calc(100vw - 48px)',
          textAlign: 'center',
        }}
      >
        <Typography variant="h5" sx={{ fontWeight: 600, mb: 1.5 }}>
          {lapsed
            ? t('home.components.connection.suspended.lapsedTitle')
            : t('home.components.connection.suspended.suspendedTitle')}
        </Typography>

        <Typography
          variant="body2"
          color="text.secondary"
          sx={{ lineHeight: 1.7, mb: 2 }}
        >
          {/*
            The hub's own sentence is shown VERBATIM when there is one — an
            operator's wording ("contact your middleman") is an instruction, not a
            description, and paraphrasing it would drop the one actionable part.
          */}
          {reason ??
            (lapsed
              ? t('home.components.connection.suspended.lapsedBody')
              : t('home.components.connection.suspended.suspendedBody'))}
        </Typography>

        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', lineHeight: 1.6, mb: 2.5 }}
        >
          {/*
            The second sentence is the one that was missing from every other
            surface: a suspension is REVERSIBLE, and the code is kept. Without
            saying so, a student assumes they have lost their purchase and goes
            looking for a refund instead of asking the person who can undo it.

            It is now also TRUE on a timescale the student can see. The backend
            keeps asking the hub whether the suspension has been lifted, so
            "without restarting the app" is a statement about the code rather than a
            hope; before that, a suspended device stopped beating and waited forever.
          */}
          {lapsed
            ? t('home.components.connection.suspended.lapsedAction')
            : t('home.components.connection.suspended.suspendedAction')}
        </Typography>

        <SupportBlock showMiddleman />
      </Box>
    </Box>
  )
}

export default SuspendedScreen
