import { LogoutOutlined } from '@mui/icons-material'
import {
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Paper,
  Typography,
  useTheme,
} from '@mui/material'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { cardSx } from '@/pages/_surfaces'
import {
  canStillCancel,
  graceState,
  SIGN_OUT_RESULT_MS,
} from '@/pages/sign-out-timer'
import {
  locusSignOut,
  type SignOutResult,
} from '@/services/locus'
import { errorDetail } from '@/services/notice-service'

/**
 * "Sign out on this device".
 *
 * # What the student is promised, and why the copy says it that way
 *
 * The dialog state machine exists to honour two specific promises, both in words
 * a student can act on rather than mechanisms:
 *
 *  * **Nothing happens while you can still change your mind.** Confirming starts
 *    a three-second window (`sign-out-timer.ts`) with a Cancel beside the
 *    countdown. The sign-out runs when the window closes, not when the button is
 *    pressed.
 *  * **You will be able to read what happened.** The result stays on screen for
 *    three seconds before the app returns to the activation screen.
 *
 * The confirmation also states the two facts that decide whether a student wants
 * this at all: the *subscription is not cancelled and not shortened*, and if they
 * are moving to a new computer, this is the thing to do. Without the second, the
 * control reads as "delete my account" and nobody presses it; without the first,
 * whoever does press it expects to lose the time they paid for.
 *
 * # Why the outcome is three sentences, not one
 *
 * The backend reports `released | nothingToRelease | notConfirmed` (see
 * `locus_sign_out`). Only the first two mean the hub knows; `notConfirmed` means
 * the network did not, and the honest sentence names that and reassures them that
 * the key still works on the next device. Collapsing all three into "Signed out"
 * would tell a student on a dead network that their key had been released — the
 * exact reassurance that costs them a second email.
 */
export const SignOutCard = ({ onSignedOut }: { onSignedOut: () => void }) => {
  const { t } = useTranslation()
  const theme = useTheme()

  // `idle` → `counting` → running → done. `elapsed` is not a state here: it is
  // computed from `startedAt` and drives the effect below, so the moment the
  // sign-out actually runs is decided by one rule rather than by a second timer.
  const [startedAt, setStartedAt] = useState<number | null>(null)
  const [now, setNow] = useState(() => Date.now())
  const [result, setResult] = useState<SignOutResult | null>(null)
  const [failure, setFailure] = useState<string | null>(null)

  // A ref, not state: whether the sign-out has already been fired is not
  // something the UI renders, and putting it in state would re-render on the
  // transition that matters least. Named `firedRef` because the lint's ref
  // naming rule is worth obeying rather than suppressing: `useRef` values are
  // not state, and the name is the only thing that says so at the call site.
  const firedRef = useRef(false)

  const windowState = graceState(startedAt, now)

  // Derived rather than stored: "a sign-out is running" is true exactly when the
  // window has closed and neither outcome has arrived yet. Storing it meant two
  // renders per keystroke of state that is a pure function of the others.
  const busy = startedAt !== null && windowState.kind === 'elapsed' && !result && !failure

  // Tick only while the window is open. A `setInterval` that runs for the life of
  // the page is a timer nobody stopped; this one exists exactly as long as the
  // countdown does.
  useEffect(() => {
    if (windowState.kind !== 'counting') return
    const id = window.setInterval(() => setNow(Date.now()), 100)
    return () => window.clearInterval(id)
  }, [windowState.kind])

  // The sign-out itself, fired once when the window closes.
  //
  // Guarded by a ref rather than by the state alone: this effect can run twice
  // for the same `startedAt` (a re-render, or React's strict-mode double invoke),
  // and a second call would sign out twice — harmless server-side, but it would
  // overwrite the first result with a second one and re-run the teardown.
  useEffect(() => {
    if (windowState.kind !== 'elapsed' || firedRef.current) return
    firedRef.current = true

    // `busy` is DERIVED, not set here. The lint rule that flagged the original
    // `setBusy(true)` is pointing at a real problem: setting state synchronously
    // in an effect renders once with the old value and once with the new, and
    // this effect already re-runs on `windowState.kind`. The state that matters
    // is "a request is in flight", which only the async continuation can know —
    // so `busy` becomes `startedAt !== null && !result && !failure` below, and
    // the effect touches state only from inside the promise where it belongs.
    void (async () => {
      try {
        const outcome = await locusSignOut()
        setResult(outcome)
      } catch (error) {
        // `errorDetail` because a Rust failure arrives as
        // `CommandFailure { code, detail }` — `String(error)` renders
        // "[object Object]" and throws away the only sentence, which is the one
        // that says nothing changed on this device.
        setFailure(
          errorDetail(error) ||
            t('home.components.connection.signOut.signedOutFailed'),
        )
      } finally {
        // `startedAt` is deliberately NOT cleared here. `busy` is derived from
        // it, so clearing it in the finally would say "nothing is happening"
        // for the render between this promise settling and `result`/`failure`
        // arriving — and the dialog would show a live button over a sign-out
        // that had just completed. `setStartedAt(null)` happens in `cancel`, and
        // a `result` or `failure` closes the dialog on its own (see `open`).
      }
    })()
  }, [windowState.kind, t])

  // The result, held long enough to read, then the gate takes over.
  //
  // The wait is this component's promise, so it is owned here: `onSignedOut`
  // flips `main.tsx` back to the activation gate, which is a full screen change.
  // Firing it immediately would replace the sentence with a code prompt before the
  // student could read it.
  useEffect(() => {
    if (!result) return
    const id = window.setTimeout(() => onSignedOut(), SIGN_OUT_RESULT_MS)
    return () => window.clearTimeout(id)
  }, [result, onSignedOut])

  const cancel = useCallback(() => {
    // Cancelling before the window closes means the sign-out never runs: the
    // effect above only fires on `elapsed`, so clearing the start is the whole
    // of the implementation.
    //
    // `firedRef` is reset too, so reopening the dialog after a cancel fires the
    // sign-out again rather than being swallowed by a ref left true.
    setStartedAt(null)
    firedRef.current = false
  }, [])

  const outcomeText = (() => {
    if (!result) return null
    switch (result.hub.kind) {
      case 'released':
        return t('home.components.connection.signOut.signedOutReleased')
      case 'nothingToRelease':
        return t('home.components.connection.signOut.signedOut')
      case 'notConfirmed':
        return t('home.components.connection.signOut.signedOutOffline')
    }
  })()

  return (
    <Paper elevation={0} sx={{ p: 2, ...cardSx(theme) }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
        <Box sx={{ color: 'text.secondary', display: 'flex' }}>
          <LogoutOutlined fontSize="small" />
        </Box>
        <Typography variant="subtitle2" sx={{ fontWeight: 600 }}>
          {t('home.components.connection.signOut.title')}
        </Typography>
      </Box>

      {/* What signing out means, stated before the control rather than after the
          click. The subscription sentence is the one that decides whether a
          student dares press this at all. */}
      <Typography
        variant="body2"
        color="text.secondary"
        sx={{ lineHeight: 1.6, mb: 1.5 }}
      >
        {t('home.components.connection.signOut.body')}
      </Typography>

      {/* A settlement line. Once the result is in, the control is replaced by the
          sentence rather than sitting beside it — a button that still looks live
          after the thing it does has been done invites a second press. */}
      {result ? (
        <Typography variant="body2" sx={{ lineHeight: 1.6 }}>
          {outcomeText}
        </Typography>
      ) : failure ? (
        <Typography variant="body2" color="error.main" sx={{ lineHeight: 1.6 }}>
          {failure}
        </Typography>
      ) : (
        <Button
          size="small"
          variant="outlined"
          disabled={busy}
          startIcon={
            busy ? (
              <CircularProgress size={14} color="inherit" />
            ) : (
              <LogoutOutlined fontSize="small" />
            )
          }
          onClick={() => {
            setNow(Date.now())
            setStartedAt(Date.now())
          }}
        >
          {t('home.components.connection.signOut.action')}
        </Button>
      )}

      {/* The confirmation.
          `Dialog` rather than an inline panel: this is a destructive-feeling
          action and it must be the only thing on screen while it is being
          decided. It cannot be dismissed by clicking away (`onClose` is not
          wired to the backdrop) — closing by accident should be impossible, and
          Cancel is right there. */}
      <Dialog
        // Open from the moment the student confirms until the outcome is in.
        //
        // The `elapsed` case is included deliberately: once the countdown ends
        // the dialog must stay up while the request runs, or the student watches
        // it vanish and then reappear as a result. A `result`/`failure` closes it,
        // because by then the sentence lives on the card itself.
        //
        // `cancel` clears `startedAt`, which returns this to `idle` — so "Cancel"
        // and "Escape" close it and nothing was signed out.
        open={
          windowState.kind !== 'idle' &&
          !result &&
          !failure
        }
        onClose={(_event, reason) => {
          // Escape and a backdrop click both mean "I did not mean this". Honouring
          // them as Cancel is the behaviour a student expects; refusing to close
          // would trap them in a dialog they opened by mistake.
          if (reason === 'escapeKeyDown' || reason === 'backdropClick') cancel()
        }}
        maxWidth="xs"
        fullWidth
      >
        <DialogTitle sx={{ fontWeight: 600 }}>
          {t('home.components.connection.signOut.confirmTitle')}
        </DialogTitle>
        <DialogContent>
          <Typography variant="body2" sx={{ lineHeight: 1.6 }}>
            {t('home.components.connection.signOut.confirmBody')}
          </Typography>
          <Typography variant="body2" sx={{ lineHeight: 1.6, mt: 1 }}>
            {t('home.components.connection.signOut.confirmKeeps')}
          </Typography>
          <Typography
            variant="body2"
            color="text.secondary"
            sx={{ lineHeight: 1.6, mt: 1 }}
          >
            {t('home.components.connection.signOut.confirmMoving')}
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          {/* Cancel is available for the whole window — asserted in
              `sign-out-timer.test.ts`. A control that disappears while the
              countdown still runs is a trap. */}
          <Button onClick={cancel} disabled={!canStillCancel(windowState)}>
            {t('home.components.connection.signOut.undo')}
          </Button>
          <Button
            variant="contained"
            color="error"
            disabled
            // Deliberately disabled and non-interactive: the student has already
            // confirmed by opening this dialog, and the countdown is the window in
            // which they can stop it. A second "confirm" button would suggest the
            // countdown is not what does the work.
            sx={{ minWidth: 130 }}
          >
            {windowState.kind === 'counting'
              ? t('home.components.connection.signOut.countingDown', {
                  count: windowState.remaining,
                })
              : // The window has closed and the call is in flight. Saying
                // "Signing out in 1…" while it happens would be a countdown that
                // never reaches zero.
                t('home.components.connection.signOut.action')}
          </Button>
        </DialogActions>
      </Dialog>
    </Paper>
  )
}
