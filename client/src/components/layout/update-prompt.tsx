import { Box, LinearProgress, Typography } from '@mui/material'
import { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { BaseDialog } from '@/components/base/base-dialog'
import {
  chrysalisDismissUpdate,
  chrysalisInstallUpdate,
  chrysalisUpdateStatus,
  type UpdateProgress,
} from '@/services/chrysalis'
import { errorDetail } from '@/services/notice-service'

/**
 * The "an update is ready" prompt.
 *
 * # Why it exists
 *
 * The updater was wired end to end on the server and entirely dead on the client:
 * the heartbeat received the hub's offer and logged it, and nothing else ever
 * happened. So a student was never told a new build existed, and — because there
 * was no control anywhere — had no way to ask. This is the prompt the original
 * Clash Verge client showed, restored on top of Chrysalis's own hub-mediated updater.
 *
 * # Why a popup and nothing else
 *
 * Decided deliberately: no sidebar badge, no permanent Account-page button. An
 * update is a rare, discrete event rather than a status to monitor, and a
 * standing "update available" chip competes for attention with the subscription
 * warning that genuinely does need to be seen. One prompt per offered version,
 * with a real choice, is the whole surface.
 *
 * # Why installing is never silent
 *
 * The download drops the connection — the installer replaces the running binary
 * — so doing it unasked would cut off a student mid-session, which on a school
 * network is the worst possible moment. The prompt exists so that interruption is
 * the student's decision, not a surprise.
 */
export const UpdatePrompt = () => {
  const { t } = useTranslation()

  /** The offered version, or `null` when there is nothing to ask about. */
  const [offered, setOffered] = useState<string | null>(null)
  /** Whether the prompt is on screen. Separate from `offered`, so dismissing is instant. */
  const [open, setOpen] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [progress, setProgress] = useState<UpdateProgress | null>(null)
  const [error, setError] = useState<string | null>(null)

  // Read the offer on mount, AND subscribe for new ones.
  //
  // Read from STORAGE, not the hub: the offer was recorded when a heartbeat (or
  // the periodic check) delivered it, and re-asking the hub at start-up would
  // delay the first paint behind a network call. `offeredVersion` being null
  // means nothing has been offered yet — not that we are up to date — so it
  // simply shows nothing.
  //
  // # Why the subscription, not just the mount read
  //
  // This used to read exactly once. Chrysalis is a background VPN: the window is
  // started with the app and then left alone for days, so a read on mount meant
  // an offer that arrived ten seconds later was never shown until the student
  // happened to relaunch — which people do not do. The backend now emits
  // `chrysalis://update-offered` when a check records a NEW offer
  // (`chrysalis::update::poll`), and this is the half that turns it into a dialog.
  //
  // The listener re-reads status rather than trusting the payload: the event
  // carries a version, but the prompt's contract is the stored offer, and one
  // source of truth is what stops the two disagreeing.
  useEffect(() => {
    let cancelled = false

    const read = () =>
      chrysalisUpdateStatus()
        .then((status) => {
          if (cancelled) return
          // The backend already refuses to report an offer that is not newer
          // than the running build, so this is a belt-and-braces check on the
          // same rule. It matters because a prompt for the version already
          // running is the exact loop being fixed: pressing Update on it
          // answered "no update available" and the prompt came back.
          const stale =
            !!status.offeredVersion &&
            status.offeredVersion === status.currentVersion
          if (status.offeredVersion && !stale) {
            setOffered(status.offeredVersion)
            setOpen(true)
          } else {
            // An offer that is no longer there is one the student has already
            // installed, or one the backend found to be spent. Drop the prompt
            // rather than leaving a dialog for a version that is not on offer.
            setOffered(null)
            setOpen(false)
          }
        })
        .catch(() => {
          // No prompt if we cannot read the state. Failing to *offer* an update
          // is harmless; a broken dialog on launch is not.
        })

    void read()

    let unlisten: (() => void) | undefined
    void (async () => {
      const { listen } = await import('@tauri-apps/api/event')
      const stop = await listen('chrysalis://update-offered', () => {
        void read()
      })
      // A fast unmount can resolve this after cleanup, in which case nothing
      // would ever remove the listener.
      if (cancelled) stop()
      else unlisten = stop
    })()

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [])

  // Download progress, emitted by the backend while the install runs.
  useEffect(() => {
    if (!installing) return

    let unlisten: (() => void) | undefined
    let cancelled = false

    void (async () => {
      const { listen } = await import('@tauri-apps/api/event')
      const stop = await listen<UpdateProgress>(
        'chrysalis://update-progress',
        (event) => {
          setProgress(event.payload)
        },
      )
      // The listener can resolve after the effect has already been cleaned up
      // (a fast failure unmounts this before the promise settles), in which case
      // nothing would ever remove it.
      if (cancelled) stop()
      else unlisten = stop
    })()

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [installing])

  const install = useCallback(async () => {
    setError(null)
    setInstalling(true)
    try {
      await chrysalisInstallUpdate()
      // Reached only when the install did NOT restart the app — Windows exits
      // into the NSIS installer instead, so this is the Linux/macOS path, and
      // even there the new binary is not running until a restart.
      setOpen(false)
    } catch (err) {
      // Shown in the dialog rather than as a toast: the student is looking at
      // this dialog, and a failure here needs a retry, not a message that
      // disappears while they read it.
      setError(errorDetail(err))
    } finally {
      setInstalling(false)
    }
  }, [])

  const dismiss = useCallback(() => {
    // Optimistic: the prompt goes immediately, and forgetting the offer is
    // bookkeeping the student should not wait for. A failure means they may be
    // asked once more, which is a better outcome than a dialog that will not
    // close.
    setOpen(false)
    void chrysalisDismissUpdate().catch(() => {})
  }, [])

  if (!offered) return null

  const percent =
    progress?.contentLength && progress.contentLength > 0
      ? Math.min(
          100,
          Math.round(
            ((progress.chunkLength ?? 0) / progress.contentLength) * 100,
          ),
        )
      : null

  return (
    <BaseDialog
      open={open}
      // Not dismissible by clicking away: an accidental close would silently
      // ignore the update, and the student would not know they had declined it.
      // "Later" is the explicit way to decline.
      onClose={undefined}
      disableEnforceFocus
      title={t('shared.feedback.notifications.updateAvailable')}
      okBtn={t('shared.update.installNow')}
      cancelBtn={t('shared.update.later')}
      loading={installing}
      onOk={() => void install()}
      onCancel={dismiss}
    >
      <Box
        sx={{
          minWidth: 320,
          display: 'flex',
          flexDirection: 'column',
          gap: 1.5,
        }}
      >
        <Typography variant="body2">
          {t('shared.update.body', { version: offered })}
        </Typography>

        {installing && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 0.75 }}>
            <LinearProgress
              variant={percent === null ? 'indeterminate' : 'determinate'}
              value={percent ?? undefined}
            />
            <Typography variant="caption" color="text.secondary">
              {percent === null
                ? t('shared.update.downloading')
                : t('shared.update.downloadingPercent', { percent })}
            </Typography>
          </Box>
        )}

        {/* On Windows the app exits into the installer, so "waiting" is not a
            hang and saying so prevents a student killing the process. */}
        {installing && (
          <Typography variant="caption" color="text.secondary">
            {t('shared.update.restartNotice')}
          </Typography>
        )}

        {error && (
          <Typography
            variant="body2"
            color="error.main"
            sx={{ lineHeight: 1.5 }}
          >
            {error}
          </Typography>
        )}

        {!installing && !error && (
          <Typography variant="caption" color="text.secondary">
            {t('shared.update.connectionNotice')}
          </Typography>
        )}
      </Box>
    </BaseDialog>
  )
}
