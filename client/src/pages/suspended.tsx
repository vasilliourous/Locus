import { Box, CircularProgress, Typography, alpha } from '@mui/material'
import { useEffect, useState } from 'react'

import { SupportBlock } from '@/components/shared/support-block'
import { LOCUS_COLORS } from '@/pages/_theme'
import { locusStatus } from '@/services/locus'

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
 */
const SuspendedScreen = () => {
  /** The hub's own sentence, if we have one. */
  const [reason, setReason] = useState<string | null>(null)
  /** Why the subscription is refused, which distinguishes a suspension from a lapse. */
  const [lapsed, setLapsed] = useState(false)
  /**
   * Whether a status read has completed.
   *
   * Distinguishes "no reason recorded" from "we have not asked yet" — the first
   * renders the generic sentence, the second renders nothing rather than flashing
   * one sentence and replacing it with another.
   */
  const [read, setRead] = useState(false)

  useEffect(() => {
    let cancelled = false
    locusStatus()
      .then((status) => {
        if (cancelled) return
        // `state: 'refused'` is the shape the backend uses for every withdrawal
        // of access, and it carries the hub's own sentence. A suspension and a
        // lapse can both reach it in principle; the difference the student cares
        // about is whether they can fix it themselves, which is why 'lapsed' is
        // distinguished rather than folded in.
        const subscription = status.subscription
        if (subscription.state === 'refused') {
          setReason(subscription.reason)
        }
        setLapsed(subscription.state === 'lapsed')
        setRead(true)
      })
      .catch(() => {
        if (!cancelled) setRead(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Box
      sx={{
        width: '100vw',
        height: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        // Same reasoning as the activation gate: this renders outside
        // `ThemeModeProvider`, so the palette is stated explicitly rather than
        // inherited. A screen about a withdrawn account must not also look broken.
        color: LOCUS_COLORS.textPrimary,
        bgcolor: LOCUS_COLORS.background,
        backgroundImage: `radial-gradient(ellipse 80% 60% at 50% 0%, ${alpha(LOCUS_COLORS.error, 0.12)}, transparent 70%)`,
      }}
    >
      <Box sx={{ width: 460, maxWidth: 'calc(100vw - 48px)', textAlign: 'center' }}>
        <Typography variant="h5" sx={{ fontWeight: 600, mb: 1.5 }}>
          {lapsed ? 'Your Locus subscription has ended' : 'This device has been signed out of Locus'}
        </Typography>

        {!read && <CircularProgress size={22} sx={{ mt: 1 }} />}

        {read && (
          <Typography
            variant="body2"
            sx={{ color: LOCUS_COLORS.textSecondary, lineHeight: 1.7, mb: 2 }}
          >
            {/*
              The hub's own sentence is shown VERBATIM when there is one — an
              operator's wording ("contact your middleman") is an instruction, not
              a description, and paraphrasing it would drop the one actionable part.
            */}
            {reason ??
              (lapsed
                ? 'Your code has expired, so the tunnel has been stopped. Renewing with whoever sold you the code restores access.'
                : 'An operator has suspended this code, so the tunnel has been stopped. Your code is still saved on this computer — nothing needs to be re-entered once the suspension is lifted.')}
          </Typography>
        )}

        {read && (
          <Typography
            variant="caption"
            sx={{ color: LOCUS_COLORS.textSecondary, display: 'block', lineHeight: 1.6, mb: 2.5 }}
          >
            {/*
              The second sentence is the one that was missing from every other
              surface: a suspension is REVERSIBLE, and the code is kept. Without
              saying so, a student assumes they have lost their purchase and goes
              looking for a refund instead of asking the person who can undo it.
            */}
            {lapsed
              ? 'Contact whoever sold you the code to renew it.'
              : 'Ask them to lift the suspension — you will be able to connect again on this computer without re-entering your code or restarting the app.'}
          </Typography>
        )}

        {/* `palette="brand"`: this renders outside `ThemeModeProvider`, so it must
            read the fixed dark constants or it inherits the light theme's
            secondary colour and half the text disappears. Same reason
            `activation.tsx` passes it. */}
        <SupportBlock palette="brand" showMiddleman />
      </Box>
    </Box>
  )
}

export default SuspendedScreen
