import {
  CheckCircleOutlined,
  ErrorOutlineRounded,
  LanguageOutlined,
  MemoryOutlined,
  PowerSettingsNewOutlined,
  RefreshOutlined,
  ScheduleOutlined,
  TuneOutlined,
} from '@mui/icons-material'
import {
  Box,
  Button,
  CircularProgress,
  Divider,
  MenuItem,
  Paper,
  Select,
  Tooltip,
  Typography,
  alpha,
  useTheme,
} from '@mui/material'
import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { TierBadge } from '@/components/connection/tier-badge'
import { TrafficSummaryCard } from '@/components/connection/traffic-summary'
import { useConnection } from '@/components/connection/use-connection'
import { SignOutCard } from '@/components/shared/sign-out-card'
import { SupportBlock } from '@/components/shared/support-block'
import { useI18n } from '@/hooks/use-i18n'
import { useSubscription } from '@/hooks/use-subscription'
import { useVerge } from '@/hooks/use-verge'
import { cardSx } from '@/pages/_surfaces'
import { chrysalisUpdateStatus, type UpdateStatus } from '@/services/chrysalis'
import { requestEntitlementRefresh } from '@/services/entitlement-refresh'
import { supportedLanguages } from '@/services/i18n'
import { showNotice } from '@/services/notice-service'

/**
 * The Account screen — everything a student owns, and nothing about the engine.
 *
 * Settings live here rather than behind a separate tab because there are only
 * four a student should touch, and a whole destination for four rows is worse
 * than folding them in. The distinction that matters: language, start-at-login
 * and update-checking are *preferences*; core choice, TUN, system proxy, ports
 * and DNS are *internals* Chrysalis decides, and have no UI at all. Appearance is
 * not a preference either: Chrysalis has one permanent look (`/COLOURS.md` §0).
 */
const AccountPage = () => {
  const { t } = useTranslation()
  const theme = useTheme()
  const { status, refresh } = useConnection()
  const { verge, patchVerge } = useVerge()
  const { switchLanguage } = useI18n()
  // The subscription card reads and re-checks through the shared subscription
  // query, so the sidebar chip and this page can never disagree about the same
  // device — a second source would be a second opinion.
  const { checkNow } = useSubscription()
  const onError = useCallback((err: unknown) => showNotice.error(err), [])
  const [checking, setChecking] = useState(false)
  // The Preferences refresh's own progress and its stated outcome. Separate from
  // `checking` because the two controls are different operations and a shared
  // flag would make one appear to be doing the other's work.
  const [refreshing, setRefreshing] = useState(false)
  const [refreshNote, setRefreshNote] = useState<string | null>(null)

  // The update-check state, read once on mount.
  //
  // Read locally (no network) through the same command the update prompt uses,
  // so the page and the prompt cannot disagree about whether an update exists.
  // This exists to answer "why is this device not updating?", which was
  // unanswerable before: every reason was logged at `debug` while the default
  // level is `Info`, so a suppressed offer left no trace on the machine.
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus | null>(null)
  useEffect(() => {
    let cancelled = false
    chrysalisUpdateStatus()
      .then((s) => {
        if (!cancelled) setUpdateStatus(s)
      })
      .catch(() => {
        // Failing to *describe* the update state is harmless; a broken page is
        // not. The row simply does not render.
      })
    return () => {
      cancelled = true
    }
  }, [])

  const subscription = status?.subscription ?? { state: 'unknown' as const }

  // How long ago the hub last confirmed anything, as a human phrase.
  //
  // Derived from a real timestamp the backend recorded on an accepted heartbeat,
  // never from "we just read the status" — reading local storage proves nothing
  // about the account, and showing that as a confirmation would defeat the whole
  // point of the line. `null` (never confirmed) is a distinct case from a recent
  // confirmation and renders as its own sentence.
  const confirmedAt = (() => {
    const at = status?.lastConfirmedAt
    if (at == null) return null
    const seconds = Math.max(0, Math.floor(Date.now() / 1000 - at))
    if (seconds < 60)
      return t('home.components.connection.account.confirmedJustNow')
    const minutes = Math.floor(seconds / 60)
    if (minutes < 60)
      return t('home.components.connection.account.confirmedMinutesAgo', {
        count: minutes,
      })
    const hours = Math.floor(minutes / 60)
    if (hours < 24)
      return t('home.components.connection.account.confirmedHoursAgo', {
        count: hours,
      })
    return t('home.components.connection.account.confirmedDaysAgo', {
      count: Math.floor(hours / 24),
    })
  })()

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      {/* ── Subscription ─────────────────────────────────────────────────── */}
      <Paper elevation={0} sx={{ p: 2, ...cardSx(theme) }}>
        <SectionTitle
          icon={<ScheduleOutlined fontSize="small" />}
          title={t('home.components.connection.account.subscription')}
        />

        <Row
          label={t('home.components.connection.account.plan')}
          valueNode={
            status?.tier ? (
              <TierBadge tier={status.tier} />
            ) : (
              <Typography variant="body2" color="text.secondary">
                —
              </Typography>
            )
          }
        />

        {/* Expiry renders ONLY when the hub has actually told us a date.
            An unknown subscription shows the honest sentence instead of a blank
            row, so the student is not left wondering whether it failed to load. */}
        {subscription.state === 'active' && (
          <Row
            label={t('home.components.connection.account.expires')}
            value={
              subscription.daysRemaining <= 0
                ? t('home.components.connection.account.today')
                : subscription.daysRemaining === 1
                  ? t('home.components.connection.account.oneDayLeft')
                  : t('home.components.connection.account.daysLeft', {
                      count: subscription.daysRemaining,
                    })
            }
            valueColor={
              subscription.urgent ? theme.palette.warning.main : undefined
            }
            tooltip={subscription.expiresAt}
          />
        )}
        {subscription.state === 'lapsed' && (
          <Row
            label={t('home.components.connection.account.expires')}
            value={t('home.components.connection.account.expired')}
            valueColor={theme.palette.error.main}
            icon={
              <ErrorOutlineRounded
                fontSize="small"
                sx={{ color: theme.palette.error.main }}
              />
            }
          />
        )}

        {/* A refusal explains itself here in full.
            The row above cannot: after a refusal there is no plan and no date to
            show, because the entitlement was withdrawn. Without this the page
            would render an empty subscription card, which is the "nothing on the
            client indicates this" report — a student whose access was ended with
            no stated reason. The hub's own sentence is the whole message. */}
        {subscription.state === 'refused' && (
          <Typography
            variant="body2"
            sx={{
              display: 'block',
              mt: 0.5,
              color: 'error.main',
              lineHeight: 1.5,
            }}
          >
            {subscription.reason}
          </Typography>
        )}
        {subscription.state === 'unknown' && (
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: 'block', mt: 0.5 }}
          >
            {status?.activated
              ? t('home.components.connection.account.noExpiryYet')
              : t('home.components.connection.account.activateForExpiry')}
          </Typography>
        )}

        {subscription.state === 'active' && subscription.urgent && (
          <Typography
            variant="caption"
            sx={{
              display: 'block',
              mt: 1,
              color: 'warning.main',
              lineHeight: 1.5,
            }}
          >
            {t('home.components.connection.account.expiresSoon')}
          </Typography>
        )}

        {/* The student's own way to check.
            This asks the HUB, not the local copy. The expiry is only ever the
            hub's to change, and the client learns it on a heartbeat whose
            interval is a floor of five minutes — so a plain re-read would show a
            just-renewed subscription as unchanged. `checkNow` triggers a beat and
            then re-reads, which is the difference between "the screen refreshed"
            and "the account was confirmed". */}
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            gap: 1,
            mt: 1.5,
            flexWrap: 'wrap',
          }}
        >
          <Button
            size="small"
            variant="text"
            startIcon={
              checking ? (
                <CircularProgress size={14} color="inherit" />
              ) : (
                <RefreshOutlined fontSize="small" />
              )
            }
            // Disabled ONLY while this control's own work is running.
            //
            // It used to also disable on `!status?.activated`, and `status` is
            // briefly `null` on mount — so the control flickered between enabled
            // and disabled while the page settled, which is part of the "flicks
            // from state to state" report. Whether the device is activated is not
            // this button's business anyway: asking the hub is always safe, and
            // the answer when there is nothing to confirm is a sentence, not a
            // greyed-out control. `checkNow()` already reports that case.
            disabled={checking}
            onClick={() => {
              void (async () => {
                setChecking(true)
                try {
                  // Resolves once the beat has completed and its outcome has been
                  // applied — NOT when it was merely requested. That is what makes
                  // this spinner last as long as the work it is reporting, instead
                  // of blinking for one IPC round trip.
                  const checked = await checkNow()
                  if (!checked)
                    showNotice.error(
                      t(
                        'home.components.connection.account.notActivatedToCheck',
                      ),
                    )
                } finally {
                  setChecking(false)
                }
              })()
            }}
          >
            {checking
              ? t('home.components.connection.account.checkingStatus')
              : t('home.components.connection.account.checkStatus')}
          </Button>
          {confirmedAt && (
            <Typography variant="caption" color="text.secondary">
              {t('home.components.connection.account.lastChecked', {
                time: confirmedAt,
              })}
            </Typography>
          )}
        </Box>

        <Divider sx={{ my: 1.5 }} />

        {/* Session totals, not lifetime totals — mihomo's counters reset with the
            core, so the label says so rather than implying a billing figure. */}
        <TrafficSummaryCard />
      </Paper>

      {/* ── This device ──────────────────────────────────────────────────── */}
      <Paper elevation={0} sx={{ p: 2, ...cardSx(theme) }}>
        <SectionTitle
          icon={<MemoryOutlined fontSize="small" />}
          title={t('home.components.connection.account.thisDevice')}
        />
        <Row
          label={t('home.components.connection.account.deviceId')}
          value={status?.deviceId ?? '—'}
          mono
        />
        {/* Copy the ID.
            //
            // The whole value of this row is that it ends up in an email, and
            // "select this monospace text with the mouse and get it right" is a
            // step that produces truncated IDs and reports support cannot match
            // to an install. The hint below has always told the student to quote
            // it; this is the affordance that makes quoting it possible.
            //
            // The outcome is stated ("Device ID copied") rather than left
            // silent: a copy button that says nothing is indistinguishable from
            // one that did not work — and `navigator.clipboard` genuinely can
            // fail, so silence would be a lie half the time. */}
        <Box sx={{ display: 'flex', justifyContent: 'flex-end', mt: -0.5 }}>
          <Button
            size="small"
            variant="text"
            disabled={!status?.deviceId}
            onClick={() => {
              const id = status?.deviceId
              if (!id) return
              void navigator.clipboard
                .writeText(id)
                .then(() =>
                  showNotice.success(
                    t('home.components.connection.signOut.deviceIdCopied'),
                  ),
                )
                // A rejected promise means the OS refused the clipboard (no user
                // gesture recognised, or a locked session). The student can
                // still read the ID off the row, so this is an error notice and
                // not a blocked action — and it says what to do instead.
                .catch(() =>
                  showNotice.error(
                    t('home.components.connection.signOut.deviceIdCopyFailed'),
                  ),
                )
            }}
          >
            {t('home.components.connection.signOut.deviceIdCopy')}
          </Button>
        </Box>
        <Row
          label={t('home.components.connection.account.platform')}
          value={status?.platform ?? '—'}
        />
        <Row
          label={t('home.components.connection.account.appVersion')}
          value={status ? `v${status.version}` : '—'}
        />
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', mt: 1 }}
        >
          {t('home.components.connection.account.deviceIdHint')}
        </Typography>

        {/* The one deployment fact a student can report and an operator can act on.

            "An operator suspended my device and it kept working" has two
            indistinguishable causes: the console never set the flag, or the hub is
            running hooks old enough to ignore it. The hub answers this on every
            heartbeat and nothing used to read the answer, so the operator was left
            comparing a file tree against a live server while the student was told
            nothing at all.

            Shown ONLY when the hub is behind. `unreported` renders nothing: a device
            that has not reached the hub yet knows nothing, and inventing a warning
            from silence would teach students to ignore this row. */}
        {status?.hubEnforcement.kind === 'stale' && (
          <Typography
            variant="caption"
            color="warning.main"
            sx={{ display: 'block', mt: 1 }}
          >
            {t('home.components.connection.account.hubBehind', {
              hub: status.hubEnforcement.hub,
            })}
          </Typography>
        )}

        {/* The support route, on the page that holds the two things a report
            needs: the Device ID and the app version. A student who opens Account
            is usually there to quote something to someone, and this is where
            they should find who to quote it to. */}
        <SupportBlock showDeviceIdHint sx={{ mt: 1.5 }} />
      </Paper>

      {/* ── Sign out ─────────────────────────────────────────────────────────
          Last on the page and visually last of the day-to-day controls, because
          it is the one action here that takes something away. Everything above
          (the subscription, the traffic, the preferences) is what a student
          normally comes to Account for.

          Gated on `status.activated`: an unactivated device has no key to forget,
          and offering "sign out" to someone who is not signed in reads as a
          control that does nothing. */}
      {status?.activated && (
        <SignOutCard
          // The gate re-reads at once (`services/entitlement-refresh`), so the
          // app returns to the activation screen immediately rather than after
          // the five-minute poll. This page also refreshes its own status so the
          // cards behind the dialog stop describing a signed-in device.
          onSignedOut={() => {
            requestEntitlementRefresh()
            void refresh()
          }}
        />
      )}

      {/* ── Preferences ──────────────────────────────────────────────────── */}
      <Paper elevation={0} sx={{ p: 2, ...cardSx(theme) }}>
        <SectionTitle
          icon={<TuneOutlined fontSize="small" />}
          title={t('home.components.connection.account.preferences')}
        />

        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 1 }}>
          <LanguageOutlined fontSize="small" sx={{ color: 'text.secondary' }} />
          <Typography variant="body2" sx={{ flexGrow: 1 }}>
            {t('home.components.connection.account.language')}
          </Typography>
          <Select
            size="small"
            value={verge?.language ?? 'en'}
            onChange={(e) => void switchLanguage(e.target.value).catch(onError)}
            sx={{ minWidth: 140 }}
          >
            {supportedLanguages.map((lang) => (
              <MenuItem key={lang} value={lang}>
                {lang}
              </MenuItem>
            ))}
          </Select>
        </Box>

        {/* "Check for updates automatically".
            Wired to `auto_check_update` — NOT `enable_auto_launch`. The row was
            previously bound to the start-at-login field, so this control showed
            the auto-launch state and toggling it changed whether Chrysalis started
            at login, while the label said updates. That is why the setting
            looked wrong out of the box: it was reporting a different setting.
            Default is ON (`unwrap_or(true)`), so a fresh install picks up
            releases without the student having to find this row. */}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 1 }}>
          <RefreshOutlined fontSize="small" sx={{ color: 'text.secondary' }} />
          <Typography variant="body2" sx={{ flexGrow: 1 }}>
            {t('home.components.connection.account.autoUpdate')}
          </Typography>
          <Button
            size="small"
            variant="text"
            onClick={() => {
              void patchVerge({
                auto_check_update: !(verge?.auto_check_update ?? true),
              }).catch(onError)
            }}
          >
            {(verge?.auto_check_update ?? true)
              ? t('home.components.connection.account.on')
              : t('home.components.connection.account.off')}
          </Button>
        </Box>

        {/* Why this device is (or is not) updating.
            Shown ONLY when there is a real obstacle, and only once a heartbeat
            has actually run — a fresh install has nothing to say and is not
            broken. This is the answer to a question that previously had none:
            "the update wasn't offered" is invisible otherwise, because the
            reasons were logged at `debug` and the default level is `Info`.

            The auto-checks-off case gets its own wording and a shortcut,
            because it is the one cause a student can fix without support and
            the one most likely to be stuck on from an era when this row was
            mis-wired to auto-launch. */}
        {updateStatus &&
          !updateStatus.offeredVersion &&
          !updateStatus.automaticChecksEnabled && (
            <Box
              sx={{
                display: 'flex',
                alignItems: 'flex-start',
                gap: 1.5,
                py: 1,
              }}
            >
              <ErrorOutlineRounded
                fontSize="small"
                sx={{ color: 'warning.main', mt: 0.25 }}
              />
              <Box sx={{ flexGrow: 1 }}>
                <Typography variant="body2" color="warning.main">
                  {t('home.components.connection.account.updateCheckOff')}
                </Typography>
                <Button
                  size="small"
                  variant="text"
                  sx={{ mt: 0.25, px: 0 }}
                  onClick={() => {
                    void patchVerge({ auto_check_update: true })
                      .then(() =>
                        // Re-read so the notice clears the moment it is fixed,
                        // rather than lingering until the next mount and making
                        // the toggle look like it did nothing.
                        chrysalisUpdateStatus().then(setUpdateStatus),
                      )
                      .catch(onError)
                  }}
                >
                  {t('home.components.connection.account.updateCheckOffAction')}
                </Button>
              </Box>
            </Box>
          )}

        {/* A previous update attempt that never finished.
            The backend records a stage before each install step precisely so it
            survives the crash being reported; this is the half that turns that
            record into a sentence a student can send to support. Shown only
            when the backend says the attempt was interrupted — a completed
            install clears the record, and a launched installer is the expected
            "the app closes to update" outcome, not a failure. */}
        {updateStatus?.interruptedUpdate && (
          <Box
            sx={{
              display: 'flex',
              alignItems: 'flex-start',
              gap: 1.5,
              py: 0.5,
            }}
          >
            <ErrorOutlineRounded
              fontSize="small"
              sx={{ color: 'warning.main', mt: 0.25 }}
            />
            <Typography
              variant="caption"
              color="text.secondary"
              sx={{ lineHeight: 1.5 }}
            >
              {updateStatus.interruptedUpdate}
            </Typography>
          </Box>
        )}

        {/* Any OTHER reason the offer did not appear — a version that is not
            newer, a release missing this platform's build. Support-facing, so
            it is plain text rather than a control. */}
        {updateStatus &&
          !updateStatus.offeredVersion &&
          updateStatus.automaticChecksEnabled &&
          updateStatus.noOfferReason && (
            <Typography
              variant="caption"
              color="text.secondary"
              sx={{ display: 'block', py: 0.5, lineHeight: 1.5 }}
            >
              {t('home.components.connection.account.updateBlocked', {
                reason: updateStatus.noOfferReason,
              })}
            </Typography>
          )}

        {/* "Auto Launch". A separate row because it is a separate setting.
            It had no control of its own before: the update row above was wired
            to this field, so this preference was only reachable through a
            mislabelled switch. Reusing the shared `autoLaunch` string keeps the
            key set unchanged (no new key means no locale churn). */}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 1 }}>
          <PowerSettingsNewOutlined
            fontSize="small"
            sx={{ color: 'text.secondary' }}
          />
          <Typography variant="body2" sx={{ flexGrow: 1 }}>
            {t('settings.sections.system.fields.autoLaunch')}
          </Typography>
          <Button
            size="small"
            variant="text"
            onClick={() => {
              void patchVerge({
                enable_auto_launch: !(verge?.enable_auto_launch ?? false),
              }).catch(onError)
            }}
          >
            {(verge?.enable_auto_launch ?? false)
              ? t('home.components.connection.account.on')
              : t('home.components.connection.account.off')}
          </Button>
        </Box>

        <Divider sx={{ my: 1.5 }} />

        {/* Re-read this device's status now.
            
            This is a DIFFERENT operation from "Check status now" above: that one
            asks the *hub* to confirm the account, this one re-reads what the
            client already holds. They looked identical — two text buttons,
            one of them silent — so this one now says what it did, every time.

            It used to be silent on both outcomes: a no-op while any read was in
            flight (which is every 750 ms during a connect, exactly when a student
            is most likely to press it), and an invisible success the rest of the
            time, because re-reading an unchanged status renders identically to
            doing nothing. A control whose success and whose failure look the same
            as its silence reads as broken. */}
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'flex-end',
            gap: 1,
            flexWrap: 'wrap',
          }}
        >
          {refreshNote && (
            <Typography variant="caption" color="text.secondary">
              {refreshNote}
            </Typography>
          )}
          <Button
            size="small"
            startIcon={
              refreshing ? (
                <CircularProgress size={14} color="inherit" />
              ) : (
                <RefreshOutlined />
              )
            }
            // Never disabled: a disabled control cannot explain itself, and the
            // only reason this can fail to act is "a read was already running",
            // which is a fact worth telling the student rather than hiding.
            onClick={() => {
              void (async () => {
                setRefreshing(true)
                setRefreshNote(null)
                try {
                  const outcome = await refresh()
                  // Every outcome is stated, including the one where nothing
                  // needed doing. "Already current" is an answer; silence is not.
                  setRefreshNote(
                    outcome === 'refreshed'
                      ? t('home.components.connection.account.refreshed')
                      : outcome === 'busy'
                        ? t('home.components.connection.account.refreshBusy')
                        : t('home.components.connection.account.refreshFailed'),
                  )
                } finally {
                  setRefreshing(false)
                }
              })()
            }}
          >
            {t('home.components.connection.account.refresh')}
          </Button>
        </Box>
      </Paper>

      {/* Activation state, when there is something to say about it. */}
      {status && !status.activated && (
        <Paper
          elevation={0}
          sx={{
            p: 2,
            borderRadius: 2,
            border: `1px solid ${alpha(theme.palette.warning.main, 0.3)}`,
            bgcolor: alpha(theme.palette.warning.main, 0.05),
          }}
        >
          <Box sx={{ display: 'flex', gap: 1.25, alignItems: 'flex-start' }}>
            <CheckCircleOutlined
              fontSize="small"
              sx={{ color: theme.palette.warning.main, mt: 0.25 }}
            />
            <Typography variant="body2" sx={{ lineHeight: 1.5 }}>
              {/* A damaged stored sign-in gets its own sentence. "This device is
                  not activated" is true but misleading here: the student did
                  activate, and saying otherwise sends them looking for a code
                  they already own — while the failure that produced it used to
                  be silently swallowed. */}
              {status.storedCodeDamaged
                ? t('home.components.connection.account.storedCodeDamaged')
                : t('home.components.connection.account.notActivated')}
            </Typography>
          </Box>
        </Paper>
      )}
    </Box>
  )
}

const SectionTitle = ({
  icon,
  title,
}: {
  icon: React.ReactNode
  title: string
}) => (
  <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
    <Box sx={{ color: 'text.secondary', display: 'flex' }}>{icon}</Box>
    <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
      {title}
    </Typography>
  </Box>
)

const Row = ({
  label,
  value,
  valueColor,
  tooltip,
  icon,
  mono,
  /** Rendered instead of `value` when the caller has rich content (a badge). */
  valueNode,
}: {
  label: string
  value?: string
  valueColor?: string
  tooltip?: string
  icon?: React.ReactNode
  mono?: boolean
  valueNode?: React.ReactNode
}) => {
  const content = valueNode ?? (
    <Typography
      variant="body2"
      sx={{
        fontWeight: 'medium',
        color: valueColor,
        fontFamily: mono
          ? 'ui-monospace, SFMono-Regular, Menlo, monospace'
          : undefined,
      }}
    >
      {value}
    </Typography>
  )

  return (
    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5, py: 0.75 }}>
      <Typography variant="body2" color="text.secondary" sx={{ flexGrow: 1 }}>
        {label}
      </Typography>
      {icon}
      {/* `Tooltip` requires a single element child, and `content` is now
          sometimes a bare string (via `valueNode`), so it is wrapped rather
          than passed through. */}
      {tooltip ? (
        <Tooltip title={tooltip} arrow>
          <Box component="span">{content}</Box>
        </Tooltip>
      ) : (
        content
      )}
    </Box>
  )
}

export default AccountPage
