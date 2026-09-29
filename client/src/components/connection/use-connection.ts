import { useCallback, useEffect, useRef, useState } from 'react'

import {
  type ConnectionPhase,
  phaseFromStatus,
} from '@/components/connection/phase'
import { useSystemState } from '@/hooks/use-system-state'
import {
  locusCancelConnect,
  locusConnect,
  locusDisconnect,
  locusStatus,
  type LocusStatus,
} from '@/services/locus'
import { errorDetail } from '@/services/notice-service'

/** Re-exported so existing importers of the hook keep working. */
export type { ConnectionPhase }

/**
 * The tunnel state machine, lifted from the home card that used to own it.
 *
 * Kept as one hook rather than inlined in the screen because the screen will
 * grow and the state rules are the part that must not drift. Every rule below
 * was paid for by a real failure mode in the retired client; the comments say
 * which, so a future edit can tell a rule from a preference.
 */
export interface ConnectionState {
  phase: ConnectionPhase
  /** The backend's own sentence, shown verbatim. `null` when there is none. */
  error: string | null
  /** The activation/subscription state, once read. */
  status: LocusStatus | null
  toggle: () => Promise<void>
  /** Force a status re-read, e.g. after an action elsewhere changes it. */
  refresh: () => Promise<void>
  /**
   * Whether connecting is currently possible on this machine.
   *
   * Exposed so the screen can say so BEFORE the student presses Connect. The
   * backend checked this on every connect and refused correctly, but a refusal
   * only arrives after a click, so a machine that could never connect looked
   * identical to one that simply had not tried yet. That is the shape of the
   * "it just doesn't connect" report: the button worked, the answer was no, and
   * nothing said so until you asked.
   */
  canConnect: boolean
  /**
   * Whether the in-flight connect can be abandoned.
   *
   * True only while connecting. The screen uses it to keep that press live rather
   * than disabling the control: a connect you cannot call off is how a student
   * ends up watching a spinner with nothing to do but kill the app.
   */
  canCancel: boolean
}

/** How often to re-read status while the screen is open. */
const POLL_INTERVAL_MS = 15000

/**
 * How often to re-read status while a connect is settling.
 *
 * Much faster than the idle poll, and only while `connecting`. The backend
 * resolves connect on readiness, but the *screen* can only learn readiness from a
 * status read — so the interval here is what decides how long the button keeps
 * saying "connecting" after the tunnel is actually up. A slow poll made a
 * working tunnel look stuck.
 */
const CONNECTING_POLL_INTERVAL_MS = 750

export const useConnection = (): ConnectionState => {
  const [phase, setPhase] = useState<ConnectionPhase>('unknown')
  const [error, setError] = useState<string | null>(null)
  const [status, setStatus] = useState<LocusStatus | null>(null)

  // Whether TUN can start here, as Rust decided it. Read from the same run-state
  // snapshot the manager uses, so the warning on screen cannot disagree with the
  // gate that will refuse the connect.
  const { isTunModeAvailable } = useSystemState()

  // The phase as the async paths below need to read it. `useState`'s value is
  // captured by the closure at render time, so a `toggle` that started before a
  // poll landed would act on a stale phase — the exact double-fire the poll
  // guard below exists to prevent. A ref always reads the current value.
  const phaseRef = useRef<ConnectionPhase>('unknown')
  phaseRef.current = phase

  // Whether THIS hook has a connect command in flight.
  //
  // Needed because `phase === 'connecting'` is now reachable two ways: a connect
  // the student started (a command is running, and a press means "cancel" — the
  // backend sets its own flag and unwinds), and a core a *previous* session left
  // half-dialled, which the first status read reports as `connecting` with no
  // command running. In the second case there is nothing for `locusCancelConnect`
  // to unwind — it only raises a flag — so the press would do nothing and the
  // button would look dead. A press there means "stop this half-formed tunnel",
  // which is a disconnect. This ref is what tells the two apart.
  const connectInFlightRef = useRef(false)

  const refresh = useCallback(async () => {
    try {
      const next = await locusStatus()
      setStatus(next)
      setPhase((current) => {
        // Never stomp an in-flight transition with a *stale* reading. A poll that
        // lands mid-connect may report the pre-connect state, and accepting that
        // would snap the button back and let the student double-fire it.
        //
        // But a reading that MATCHES the transition must be allowed to advance or
        // end it, or the button strands:
        //
        //   - `connected && ready`  -> the tunnel is up; promote to connected.
        //     The connect command may still be waiting on the readiness it has
        //     already reached, and refusing this reading would hold "connecting"
        //     until the command returned.
        //   - `connected && !ready` -> still dialling; hold. Same fact as now, and
        //     the fast poll keeps watching.
        //   - `!connected`          -> the core stopped or never started. This is
        //     the change that used to be swallowed: a connect that FAILED (or a
        //     core that died mid-connect) left `current` at "connecting" forever,
        //     because the guard returned `current` for every non-ready reading.
        //     Settle to the honest state instead.
        if (current === 'connecting') {
          if (next.ready) return 'connected'
          return next.connected ? current : 'disconnected'
        }
        if (current === 'disconnecting') return current
        return phaseFromStatus(next)
      })
    } catch {
      // Leave the phase alone. A status read failing is not evidence about the
      // tunnel, and flapping the UI on a transient error is worse than a stale
      // label a moment longer.
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  useEffect(() => {
    // Two intervals rather than one: the idle poll is cheap and rarely matters,
    // while a settling connect needs to be observed closely enough that the
    // button reflects readiness almost immediately.
    const interval =
      phase === 'connecting' ? CONNECTING_POLL_INTERVAL_MS : POLL_INTERVAL_MS
    const timer = window.setInterval(() => void refresh(), interval)
    return () => window.clearInterval(timer)
  }, [phase, refresh])

  const toggle = useCallback(async () => {
    const current = phaseRef.current

    // A press while connecting is a CANCEL, not a second connect. This is
    // the deliberate shape of the loading button: it is not disabled, because a
    // transition you cannot abandon is the retired client's forever-spinner.
    //
    // But `connecting` can also mean a half-dialled core found on app open, with
    // no connect of ours running. `locusCancelConnect` only raises a flag for an
    // in-flight command to notice, so with none running it would do nothing and
    // the button would look dead. In that case the useful action is to take the
    // half-formed tunnel down — a disconnect — which is what the student means.
    if (current === 'connecting') {
      if (!connectInFlightRef.current) {
        setError(null)
        setPhase('disconnecting')
        try {
          const result = await locusDisconnect()
          setError(result.connected ? null : result.message)
        } catch (err) {
          setError(errorDetail(err))
        }
        void refresh()
        return
      }
      try {
        await locusCancelConnect()
      } catch (err) {
        // Reporting a failed cancel would be noise: the connect is still running
        // and the next poll will settle the screen either way.
        console.error('[locus] cancel failed:', err)
      }
      return
    }

    if (current === 'disconnecting' || current === 'unknown') {
      return
    }

    setError(null)
    const connecting = current !== 'connected'
    setPhase(connecting ? 'connecting' : 'disconnecting')
    if (connecting) connectInFlightRef.current = true

    try {
      const result = connecting ? await locusConnect() : await locusDisconnect()
      // The backend's sentence is shown when it carries an explanation rather
      // than a bare confirmation. A timed-out connect returns `connected: false`
      // WITH the reason, and swallowing it would leave a student watching a
      // button reset with no idea why.
      if (!result.connected && connecting) {
        setError(result.message)
      }
      // Settle from the command's own verdict. `locusConnect` resolves only once
      // the core reports ready, so `connected: true` here does imply ready; a false
      // is "the tunnel did not come up". The status re-read below re-settles the
      // phase from the backend either way, which is what promotes a success that
      // somehow outran readiness to `connecting` rather than a premature green.
      setPhase(result.connected ? 'connected' : 'disconnected')
      // Re-read so the tier, subscription and core state shown are the ones the
      // backend now holds — and so a connect that returned `connected: true` but is
      // not yet ready settles to `connecting` rather than to a settled state.
      void refresh()
    } catch (err) {
      const message = errorDetail(err)
      setError(message)
      setPhase(connecting ? 'disconnected' : 'connected')
    } finally {
      connectInFlightRef.current = false
    }
  }, [refresh])

  // Whether the tunnel can be brought up on this machine.
  //
  // This is `tun_capable` — the backend's own predicate (`is_admin ||
  // service_usable()`), read from the same run-state snapshot the connect gate
  // consumes. Deliberately NOT re-derived from `isAdmin` and service flags here:
  // a second implementation is a second thing that can disagree with the gate,
  // and the whole point is that the warning on screen matches the answer the
  // student will get when they press the button.
  const canConnect = isTunModeAvailable

  return {
    phase,
    error,
    status,
    toggle,
    refresh,
    canConnect,
    canCancel: phase === 'connecting',
  }
}
