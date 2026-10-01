import {
  CloudDownloadRounded,
  CloudUploadRounded,
  MemoryRounded,
  SwapVertRounded,
} from '@mui/icons-material'
import {
  Box,
  Button,
  CircularProgress,
  Paper,
  Tooltip,
  Typography,
  alpha,
  useTheme,
} from '@mui/material'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { TierBadge } from '@/components/connection/tier-badge'
import { TrafficGraph } from '@/components/connection/traffic-graph'
import { useConnection } from '@/components/connection/use-connection'
import { useTrafficSummary } from '@/components/connection/use-traffic-summary'
import { useSubscription } from '@/hooks/use-subscription'
import { accentCardSx, cardSx } from '@/pages/_surfaces'
import { connectNotice } from '@/pages/connect-notice'
import { shouldToggleFromKey } from '@/pages/connection-keys'
import { formatSpeed } from '@/utils/format-speed'

/**
 * The Connection screen — the product's primary surface.
 *
 * One decision, made obvious: the tunnel is on or off. A vendor VPN app shows a
 * single control and the evidence it is working; everything a student should not
 * have to reason about (which core, which mode, which node, which port) is
 * decided for them by Locus and the hub.
 *
 * What replaced what:
 *   - the raw "system proxy / TUN" switches  → the one Connect control
 *   - the proxy page's node picker           → nothing; one server per tier
 *   - the proxy page's core-unavailable text → the backend's own coded error
 */
const ConnectionPage = () => {
  const { t } = useTranslation()
  const theme = useTheme()
  const { phase, error, status, toggle, canConnect } = useConnection()
  const summary = useTrafficSummary()
  // The subscription state, from the same shared query the sidebar badge uses,
  // so the warning here and the badge there can never contradict each other.
  const { subscription } = useSubscription()

  // What this screen should say when the student cannot connect. A pure rule
  // (`connect-notice.ts`) rather than an inline condition, because the priority
  // between "the hub refused you" and "you have not activated yet" is the thing
  // that was wrong, and it is worth a test that can fail.
  const notice = connectNotice(phase, error !== null, subscription)

  const busy = phase === 'connecting' || phase === 'disconnecting'
  const connected = phase === 'connected'
  const showSlowHint = useSlowTransition(phase === 'connecting')

  /**
   * Enter and Space toggle the tunnel, from anywhere on the screen.
   *
   * This is a one-button product: a student opens the window to do one thing, and
   * reaching for the mouse is friction on the only interaction the app has. It is
   * also the difference between "usable" and "usable with a keyboard" — before
   * this, the only way to connect was to tab to a button a keyboard user cannot
   * always see is focused.
   *
   * Deliberately skipped while focus is in a text field, or a code being typed on
   * the activation screen would submit a connection instead. `isContentEditable`
   * is included for the same reason: the app has no rich-text surface today, but
   * a future one would silently break this.
   *
   * Only bound when the action is meaningful, so Enter on the Account screen
   * cannot start a tunnel the student cannot see the state of. The listener is
   * mounted here rather than globally for exactly that reason.
   */
  useEffect(() => {
    if (phase === 'checking' || phase === 'unactivated') return

    const onKeyDown = (event: KeyboardEvent) => {
      // The rule itself is a pure function so it can be tested; see
      // `connection-keys.ts` for why each exclusion exists.
      const target = event.target as HTMLElement | null
      const shouldToggle = shouldToggleFromKey({
        key: event.key,
        repeat: event.repeat,
        targetTagName: target?.tagName?.toUpperCase() ?? '',
        targetIsContentEditable: target?.isContentEditable ?? false,
      })

      // `defaultPrevented` is checked here rather than in the rule because it is
      // about the event's history, not its content: a button that already
      // handled this press sets it, and acting again would toggle twice.
      if (!shouldToggle || event.defaultPrevented) return

      // Space scrolls the page by default. Suppressed only when we are actually
      // acting, so the key still works normally everywhere else.
      event.preventDefault()
      void toggle()
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [phase, toggle])

  // While connecting, the button's job is to STAY AVAILABLE and offer a way out.
  // A disabled spinner is the retired client's failure: the tunnel never settled,
  // the control never came back, and the only recovery was to kill the app.
  //
  // `checking` and `unactivated` share the same label because they are the same
  // thing to a student — "nothing to press yet" — even though they are different
  // facts to the app. Which of the two it is decides the *notice* below, not the
  // button.
  const label =
    phase === 'checking' || phase === 'unactivated'
      ? t('home.components.connection.checking')
      : phase === 'connecting'
        ? t('home.components.connection.cancel')
        : phase === 'disconnecting'
          ? t('home.components.connection.disconnecting')
          : connected
            ? t('home.components.connection.disconnect')
            : t('home.components.connection.connect')

  // The status line answers "is it working?" before the student asks.
  //
  // The tier is deliberately NOT concatenated into this string any more: it is
  // its own badge below. "Connected · strike" read as one run-on label, and the
  // spec gives the tier its own colour so it is scannable rather than parsed.
  // The status line answers "is it working?" before the student asks, and says
  // "connecting" rather than "not connected" while the tunnel is coming up —
  // those are different facts, and the second one would be a lie a student can
  // catch by watching the panel appear a moment later.
  //
  // No read has settled yet is NOT "not connected". Saying so would be the same
  // class of lie as the activation flash, one line down.
  const statusText =
    phase === 'checking'
      ? t('home.components.connection.checking')
      : phase === 'connecting'
        ? t('home.components.connection.connecting')
        : connected
          ? t('home.components.connection.connected')
          : t('home.components.connection.notConnected')

  // Spec §5: connected green, connecting amber, disconnected grey. The dot is
  // paired with the word everywhere it appears, so colour is never the only cue.
  // `checking` stays grey like disconnected: it is an absence of information, not
  // a state of the tunnel, and colouring it would assert something we do not know.
  const accent =
    phase === 'connecting' || phase === 'disconnecting'
      ? theme.palette.warning.main
      : connected
        ? theme.palette.success.main
        : theme.palette.text.disabled

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      <Paper
        elevation={0}
        sx={{
          p: 3,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: 2,
          ...accentCardSx(theme, accent),
          transition: 'border-color 0.2s, background-color 0.2s',
        }}
      >
        {/* The state indicator is a coloured dot plus a word, not a colour alone:
            colour-only state is invisible to a colour-blind student. The dot
            pulses while connecting so "it is doing something" is visible without
            reading the button. */}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.25 }}>
          <Box
            sx={{
              width: 12,
              height: 12,
              borderRadius: '50%',
              bgcolor: accent,
              boxShadow: connected
                ? `0 0 0 4px ${alpha(accent, 0.18)}`
                : busy
                  ? `0 0 0 4px ${alpha(accent, 0.14)}`
                  : 'none',
              transition: 'background-color 0.2s, box-shadow 0.2s',
              ...(busy && {
                animation: 'locus-pulse 1.4s ease-in-out infinite',
                '@keyframes locus-pulse': {
                  '0%, 100%': { opacity: 1 },
                  '50%': { opacity: 0.45 },
                },
              }),
            }}
          />
          <Typography variant="h6" sx={{ fontWeight: 500 }}>
            {statusText}
          </Typography>
        </Box>

        {/* The tier, given its own identity rather than appended to the status.
            Only shown once the user has a tier to show — an unactivated device
            has none, and an empty badge would read as a rendering fault. */}
        {status?.tier && <TierBadge tier={status.tier} />}

        <Button
          size="large"
          variant={connected ? 'outlined' : 'contained'}
          color={connected ? 'inherit' : 'primary'}
          // Disabled ONLY when there is nothing to do. `connecting` stays
          // pressable and means "cancel"; `disconnecting` stays pressable and is
          // ignored by the hook, because a stop mid-stop is not a new intent.
          //
          // `unactivated` is included because the gate above the router should
          // make it unreachable — if it is reached, the honest state is "there is
          // nothing this button can do", and the notice below says why.
          disabled={phase === 'checking' || phase === 'unactivated'}
          onClick={() => void toggle()}
          // The shortcut is advertised rather than hidden: a keyboard affordance
          // nobody knows about is not an affordance. `title` carries it for the
          // hover case, and the accessible name below says it for a screen
          // reader, because a `title` attribute is not reliably announced.
          title={t('home.components.connection.toggleHint')}
          aria-keyshortcuts="Enter Space"
          sx={{ minWidth: 220, py: 1.6, fontSize: 16 }}
        >
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.25 }}>
            {busy && <CircularProgress size={20} color="inherit" />}
            {label}
          </Box>
        </Button>

        {/* A connect that is taking a while says so, rather than looking stalled.
            The threshold is what separates "slow" from "stuck" for the student:
            without it, a school network that needs fifteen seconds looks
            identical to a button that will never finish. */}
        {phase === 'connecting' && showSlowHint && (
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ textAlign: 'center', maxWidth: 420, lineHeight: 1.5 }}
          >
            {t('home.components.connection.takingLonger')}
          </Typography>
        )}

        {/* The backend's sentence, shown as-is.
            `LOCUS_TUN_NOT_AVAILABLE` and `SERVICE_ELEVATION_FAILED` land here;
            both are written to be actionable, and paraphrasing them would throw
            away the part that says what to do. */}
        {error && (
          <Typography
            variant="body2"
            sx={{
              color: 'error.main',
              textAlign: 'center',
              maxWidth: 420,
              lineHeight: 1.5,
            }}
          >
            {error}
          </Typography>
        )}

        {/* Only when there is no better explanation.
            `unknown` means "not activated" — which is also what a *refused*
            device looks like, since a refusal clears the entitlement. Asking
            such a student for the code on their card is the wrong instruction:
            they have one, and it was refused. The refusal sentence below is the
            real answer, so this generic prompt stands down when we have it. */}
        {connectNotice(phase, error !== null, subscription).kind ===
          'unactivated' && (
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ textAlign: 'center' }}
          >
            {t('home.components.connection.needsActivation')}
          </Typography>
        )}

        {/* The subscription, said out loud on the screen where a student would
            notice it — not only on the Account page they never open.

            Three cases, and the reason each is separate:

              - **lapsed**: the connect button will refuse, so the student must be
                told *why* before they press it, in the same words the refusal
                uses. Without this the button looks broken.
              - **urgent (last week)**: still working, but the student needs to act
                (contact the seller) before it stops. This is the window the
                renewal conversation happens in.
              - **refused**: the hub has ended this device's access and withdrawn
                the entitlement. Said here, in the hub's own words, because this
                is the screen a student returns to after the tunnel silently
                stopped working — without it they see an app that deactivated
                itself for no stated reason.
              - **comfortable**: nothing. A student whose code runs for months
                does not need a daily reminder, and showing one trains them to
                ignore the badge that matters.

            A date the hub never sent renders nothing, here as everywhere —
            except for a refusal, which is not a date. */}
        {notice.kind === 'refused' && (
          <Typography
            variant="body2"
            sx={{
              color: 'error.main',
              textAlign: 'center',
              maxWidth: 440,
              lineHeight: 1.5,
            }}
          >
            {notice.reason}
          </Typography>
        )}
        {subscription.state === 'lapsed' && (
          <Typography
            variant="body2"
            sx={{
              color: 'error.main',
              textAlign: 'center',
              maxWidth: 440,
              lineHeight: 1.5,
            }}
          >
            {t('home.components.connection.subscriptionLapsed')}
          </Typography>
        )}
        {subscription.state === 'active' && subscription.urgent && (
          <Typography
            variant="body2"
            sx={{
              color: 'warning.main',
              textAlign: 'center',
              maxWidth: 440,
              lineHeight: 1.5,
            }}
          >
            {subscription.daysRemaining <= 0
              ? t('home.components.connection.subscriptionExpiresToday')
              : t('home.components.connection.subscriptionExpiresIn', {
                  count: subscription.daysRemaining,
                })}
          </Typography>
        )}

        {/* Why the button will not work, said BEFORE it is pressed.
            //
            // The backend has always refused a connect it cannot honour, and said
            // why — but only in response to a click. So a machine that could never
            // connect looked exactly like one that simply had not been tried, and
            // the student's next move was to press a button that was always going
            // to fail. That is the "it just doesn't connect" report.
            //
            // Shown only in the connect direction: disconnecting must stay
            // reachable, because taking a broken tunnel down should never be
            // blocked by a warning about bringing one up. */}
        {!canConnect && !connected && (
          <Typography
            variant="body2"
            sx={{
              color: 'warning.main',
              textAlign: 'center',
              maxWidth: 440,
              lineHeight: 1.5,
            }}
          >
            {t('home.components.connection.requiresSetup')}
          </Typography>
        )}
      </Paper>

      {/* Live speed, and the session/engine figures beside it.

          # Why this is always rendered (changed 2026-09-30)

          It used to appear only once `connected || connecting`. Two things were
          wrong with that, and both are the "feels off" kind rather than the
          broken kind:

            1. **The screen changed height on every state change.** Pressing
               Connect grew the page by a whole card; disconnecting collapsed it
               again. That reflow is felt as the UI jumping under the cursor, and
               on a 700px-tall window it moved the button the student was about to
               press.
            2. **The disconnected screen was mostly empty**, which reads as "this
               app is missing something" rather than "nothing is running yet".

          It is now always present, in a visibly inert state when there is nothing
          to report. The figures are still honest: `summary` reports zeros for a
          dead core, and the speed readouts say "Idle" rather than showing a bare
          `0`. What changed is only that the layout no longer moves.

          Four metrics, the ones the original dashboard had: up/down *speed*
          (live), up/down *totals* (this session), core usage, and active
          connections. The retired client showed all four together; the rework
          had reduced it to two speeds, which answers "is it moving?" but not
          "what has it moved?" or "is the engine healthy?". */}
      {(() => {
        // A core is running, or coming up. Outside that the readings are all
        // zero by definition and the panel is shown inert rather than absent.
        const live = connected || phase === 'connecting'
        return (
          <Paper
            elevation={0}
            sx={{
              p: 1.75,
              ...cardSx(theme),
              // Dimmed rather than hidden, so the shape is constant and the
              // change reads as "nothing to report" instead of "gone".
              opacity: live ? 1 : 0.55,
              transition: 'opacity 0.2s',
            }}
            aria-busy={!live}
          >
            {!live && (
              <Typography
                variant="caption"
                color="text.secondary"
                sx={{ display: 'block', textAlign: 'center', mb: 1 }}
              >
                {t('home.components.connection.metricsWhenConnected')}
              </Typography>
            )}
            <Box
              sx={{ display: 'flex', justifyContent: 'space-around', mb: 1.5 }}
            >
              <SpeedReadout
                label={t('home.components.traffic.metrics.downloadSpeed')}
                bytesPerSecond={live ? summary.downSpeedBytes : 0}
                color={theme.palette.primary.main}
              />
              <SpeedReadout
                label={t('home.components.traffic.metrics.uploadSpeed')}
                bytesPerSecond={live ? summary.upSpeedBytes : 0}
                color={theme.palette.secondary.main}
              />
            </Box>

            {/* The graph is only meaningful with a running core, and it is the
                one part of this card that costs CPU, so it stays conditional —
                but the space it occupies is reserved, so its arrival does not
                move the grid below it. */}
            <Box sx={{ minHeight: live ? undefined : 0 }}>
              {live && <TrafficGraph />}
            </Box>

            {/* The four figures the original dashboard carried, as a grid of
                icon + label + value. Totals are labelled "this session" because
                mihomo's counters are per-process — a disconnect resets them, and
                presenting them as lifetime figures would be a lie a student can
                catch. */}
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: '1fr 1fr',
                gap: 1.25,
                mt: 1.75,
                pt: 1.75,
                borderTop: `1px solid ${alpha(theme.palette.divider, 0.4)}`,
              }}
            >
              <MetricTile
                icon={<CloudUploadRounded fontSize="small" />}
                label={t('home.components.connection.sessionUpload')}
                value={live ? summary.uploaded : '0'}
                unit={live ? summary.uploadedUnit : 'B'}
                color={theme.palette.secondary.main}
              />
              <MetricTile
                icon={<CloudDownloadRounded fontSize="small" />}
                label={t('home.components.connection.sessionDownload')}
                value={live ? summary.downloaded : '0'}
                unit={live ? summary.downloadedUnit : 'B'}
                color={theme.palette.primary.main}
              />
              <MetricTile
                icon={<MemoryRounded fontSize="small" />}
                label={t('home.components.traffic.metrics.memoryUsage')}
                value={live ? summary.memory : undefined}
                unit={live ? summary.memoryUnit : undefined}
                color={theme.palette.info.main}
              />
              <MetricTile
                icon={<SwapVertRounded fontSize="small" />}
                label={t('home.components.connection.activeConnections')}
                value={
                  !live || summary.activeConnections === undefined
                    ? undefined
                    : String(summary.activeConnections)
                }
                color={theme.palette.success.main}
              />
            </Box>
          </Paper>
        )
      })()}
    </Box>
  )
}

/**
 * A live speed readout.
 *
 * Takes the raw bytes-per-second and formats it here, so the friendlier
 * presentation (sub-KB speeds shown as KB/s, zero shown as idle) lives in one
 * place — `formatSpeed` — rather than being re-derived per call site.
 *
 * # The idle case used to render as a bare `0`
 *
 * `formatSpeed` deliberately computes `idle` so a zero speed reads as "idle"
 * rather than as a fault. This component set the unit to an empty string for it
 * and emitted only the value — so the student saw a lone grey `0` with no unit,
 * which is the one thing the formatter was written to avoid: it looks like a
 * value that failed to load, not like a quiet connection. The intent was in the
 * comment and the flag, and the rendering dropped it.
 */
const SpeedReadout = ({
  label,
  bytesPerSecond,
  color,
}: {
  label: string
  bytesPerSecond: number
  color: string
}) => {
  const { t } = useTranslation()
  const speed = formatSpeed(bytesPerSecond)

  return (
    <Box sx={{ textAlign: 'center' }}>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ display: 'block' }}
      >
        {label}
      </Typography>
      {speed.idle ? (
        // Said in words, in the same visual weight as a reading, so the panel's
        // shape does not jump between idle and active.
        <Typography
          variant="h6"
          color="text.secondary"
          sx={{
            fontVariantNumeric: 'tabular-nums',
            fontWeight: 600,
            opacity: 0.7,
          }}
        >
          {t('home.components.traffic.idle')}
        </Typography>
      ) : (
        <Typography
          variant="h6"
          sx={{ color, fontVariantNumeric: 'tabular-nums', fontWeight: 600 }}
        >
          {speed.value}
          <Typography
            component="span"
            variant="caption"
            sx={{ ml: 0.5, color: 'text.secondary' }}
          >
            {speed.unit}/s
          </Typography>
        </Typography>
      )}
    </Box>
  )
}

/** One of the four dashboard figures: an icon, a label, and a value. */
const MetricTile = ({
  icon,
  label,
  value,
  unit,
  color,
}: {
  icon: React.ReactNode
  label: string
  value: string | undefined
  unit?: string
  color: string
}) => {
  const theme = useTheme()
  const { t } = useTranslation()
  const known = value !== undefined

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, minWidth: 0 }}>
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          width: 30,
          height: 30,
          borderRadius: 1.5,
          flexShrink: 0,
          bgcolor: alpha(color, 0.1),
          color,
        }}
      >
        {icon}
      </Box>
      <Box sx={{ minWidth: 0 }}>
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', lineHeight: 1.2 }}
        >
          {label}
        </Typography>
        {/* A value we do not have yet is shown as an em dash, never as "0" —
            "we do not know" and "nothing" are different facts. */}
        <Typography
          variant="body2"
          sx={{
            fontWeight: 600,
            fontVariantNumeric: 'tabular-nums',
            color: theme.palette.text.primary,
          }}
        >
          {known ? (
            <>
              {value}
              {unit ? (
                <Typography
                  component="span"
                  variant="caption"
                  sx={{ ml: 0.5, color: 'text.secondary' }}
                >
                  {unit}
                </Typography>
              ) : null}
            </>
          ) : (
            <Tooltip
              title={t('home.components.connection.metricPending')}
              arrow
            >
              <Box component="span" sx={{ color: 'text.disabled' }}>
                —
              </Box>
            </Tooltip>
          )}
        </Typography>
      </Box>
    </Box>
  )
}

export default ConnectionPage

/**
 * Whether a transition has been running long enough to be worth explaining.
 *
 * Deliberately not tied to the backend's readiness budget: this is about what
 * the student sees, and the point is to separate "slow but working" from "stuck"
 * before they give up. The timer restarts whenever the transition does.
 */
const SLOW_TRANSITION_MS = 6000

const useSlowTransition = (active: boolean) => {
  const [elapsed, setElapsed] = useState(false)

  useEffect(() => {
    if (!active) return
    const timer = window.setTimeout(() => setElapsed(true), SLOW_TRANSITION_MS)
    return () => window.clearTimeout(timer)
  }, [active])

  // Derived rather than stored: when the transition ends the answer is false
  // without a second state write, which also avoids a re-render on every toggle.
  return active && elapsed
}
