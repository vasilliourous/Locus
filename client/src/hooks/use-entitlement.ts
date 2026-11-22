import { useCallback, useEffect, useRef, useState } from 'react'

import { SUBSCRIPTION_POLL_MS } from '@/hooks/use-subscription'
import { chrysalisStatus, type ChrysalisStatus } from '@/services/chrysalis'
import { subscribeEntitlementRefresh } from '@/services/entitlement-refresh'

/**
 * The one place that decides what the gate is allowed to branch on.
 *
 * # Why this exists
 *
 * `main.tsx` used to own this: a poll, an event listener added by hand inside an
 * effect, and a `catch` that guessed. Three surfaces then had to agree about it —
 * the gate, the kick-out screen (which issued its own `chrysalisStatus()`) and the
 * activation screen (which issued a third) — and they read the same facts at
 * different times with different fallbacks.
 *
 * The reliability problem was not any single wrong branch. It was that a fact a
 * human had just changed (an operator suspended the code; the student signed out;
 * the app came back from sleep) reached the UI on the next four- or five-minute
 * tick, or on a Tauri event that existed for only some of the transitions. So the
 * rule is stated once, here:
 *
 *   **the backend is the only authority, and everything that can change the answer
 *   asks it again immediately.**
 *
 * # What re-reads, and why each one
 *
 * * the entitlement change event — the hub's answer landed on a background beat
 *   (`"suspended"`, `"refused"`, `"reinstated"`), or the student signed out
 *   (`"signed-out"`), or activation completed;
 * * the local refresh signal — a React-level notification from the sign-out card,
 *   which fires before the backend's event reaches the window;
 * * **window focus** — a student who left the app open while an operator acted
 *   sees the truth the moment they look at it. This trigger was missing entirely;
 * * **resume** (`pageshow`) and **network change** (`online`) — a laptop closed
 *   overnight missed every beat by definition, and its first look at the world
 *   should not wait for a timer scheduled before it slept;
 * * the poll below, as a floor. It is now the *last* resort rather than the only
 *   mechanism.
 *
 * # Why the poll survives at all
 *
 * Every trigger above is an event from something that might not fire: a window
 * that never loses focus, a machine that never sleeps, a beat that failed
 * quietly. A finite poll is what keeps "event-driven" from meaning
 * "event-dependent". It is five minutes because that matches the heartbeat's
 * floor — polling faster than the hub can answer only re-reads a value that
 * provably has not changed.
 */
export interface Entitlement {
  /**
   * The whole status, or `null` before the first read settles.
   *
   * Surfaces that need more than the gate's two booleans — the kick-out page shows
   * the hub's own sentence — read this rather than issuing their own
   * `chrysalisStatus()`, which is how two screens came to disagree about one device.
   */
  status: ChrysalisStatus | null
  /**
   * Whether a code is stored and accepted.
   *
   * `null` until the first read settles, which is NOT the same as `false`: a
   * device that has not been asked about must not be told it is unactivated.
   */
  activated: boolean | null
  /**
   * Whether an operator has suspended this device.
   *
   * Always `false` when {@link activated} is false or unknown. The backend
   * guarantees that pair (`store::is_suspended` refuses it), and the read below
   * enforces it again, because the cost of the pair reaching the gate is a screen
   * with no controls.
   */
  suspended: boolean
  /** Re-read now. Safe to call from anywhere and at any time. */
  refresh: () => void
}

/**
 * Which screen the gate shows, given the state it has.
 *
 * Split out as a pure function because it is the decision the reported bug got
 * wrong, and a decision that can only be exercised by mounting React against a
 * Tauri mock is a decision that regresses unnoticed (`phase.ts` makes the same
 * trade for the same reason).
 *
 * The order is the fix:
 *
 * 1. **Not read yet** → the spinner. Never a claim.
 * 2. **Not activated** → the activation gate, EVEN IF the suspension flag is set.
 *    A suspension is a statement about a code; with no code there is nothing it
 *    can mean. This is the exact state a signed-out device reached: the flag
 *    survived the sign-out, the kick-out screen rendered, and it has no controls
 *    by design — so the student's only exit was wiping the app's data. The backend
 *    now keeps the pair consistent; this branch means an older build, or a config
 *    written by one, still cannot trap them.
 * 3. **Suspended** → the kick-out screen.
 * 4. Otherwise → the app.
 *
 * `suspended` is checked AFTER `activated` on purpose, and that ordering is the
 * whole of the guarantee: the kick-out screen may only ever be shown to a device
 * that holds a code.
 */
export type GateScreen = 'loading' | 'activation' | 'suspended' | 'app'

export const gateScreen = (
  activated: boolean | null,
  suspended: boolean,
): GateScreen => {
  if (activated === null) return 'loading'
  if (!activated) return 'activation'
  return suspended ? 'suspended' : 'app'
}

export const useEntitlement = (): Entitlement => {
  const [status, setStatus] = useState<ChrysalisStatus | null>(null)
  const [activated, setActivated] = useState<boolean | null>(null)
  const [suspended, setSuspended] = useState(false)
  /**
   * Whether the first read has settled, success or failure.
   *
   * A ref rather than state: it must be readable from the `catch` without causing
   * a render, and it is only ever consulted to decide whether a failed read may
   * fall back to "not activated".
   */
  const settledRef = useRef(false)

  /**
   * Reads the authoritative status once and applies it.
   *
   * The only writer of the three state values, so "every trigger re-reads" is a
   * fact about one function rather than a property each caller must remember.
   * Stable across renders, so the effect below can depend on it honestly.
   */
  const read = useCallback(() => {
    chrysalisStatus()
      .then((next) => {
        settledRef.current = true
        setStatus(next)
        setActivated(next.activated)
        // Mirrors the backend's invariant rather than trusting it alone: a
        // suspension only ever describes a device that holds a code.
        setSuspended(next.suspended && next.activated)
      })
      .catch(() => {
        // A failed read is not a state change. Only the FIRST read may fall back
        // to "not activated"; after that, keep what we have. Two distinct wrongs
        // are avoided:
        //
        //   * tearing a working session away because storage hiccuped, which is a
        //     surprise logout;
        //   * letting a failed read clear a suspension, which would silently
        //     re-admit a device an operator had just kicked out — the worse error,
        //     and the one this branch must never make.
        if (!settledRef.current) {
          settledRef.current = true
          setActivated(false)
        }
      })
  }, [])

  useEffect(() => {
    let cancelled = false
    let unlisten: (() => void) | undefined

    const tick = () => {
      if (!cancelled) read()
    }

    tick()

    const timer = window.setInterval(tick, SUBSCRIPTION_POLL_MS)
    // The React-level signal and the backend's event both land here. The former is
    // synchronous with the student's own action (a sign-out completes and flips
    // the gate without waiting for the event round trip); the latter is the only
    // one that exists for a change made elsewhere.
    const unsubscribeLocal = subscribeEntitlementRefresh(tick)

    // `listen` is imported lazily: the module is only reachable inside the Tauri
    // webview, so importing it at module scope would break any render (or test)
    // that loads this hook outside one.
    void (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event')
        const stop = await listen('chrysalis://entitlement-changed', tick)
        if (cancelled) stop()
        else unlisten = stop
      } catch (error) {
        // Without the event the focus/resume/online triggers and the poll still
        // work; a webview that refuses the event API must not lose the gate.
        console.warn(
          '[use-entitlement] could not subscribe to entitlement events',
          error,
        )
      }
    })()

    const onResume = () => tick()
    const onVisibility = () => {
      // A window that was hidden rather than blurred — minimised on Windows — is
      // how a background VPN is usually left, and it never fires `focus`.
      if (document.visibilityState === 'visible') tick()
    }
    window.addEventListener('focus', onResume)
    window.addEventListener('pageshow', onResume)
    window.addEventListener('online', onResume)
    document.addEventListener('visibilitychange', onVisibility)

    return () => {
      cancelled = true
      window.clearInterval(timer)
      unsubscribeLocal()
      unlisten?.()
      window.removeEventListener('focus', onResume)
      window.removeEventListener('pageshow', onResume)
      window.removeEventListener('online', onResume)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [read])

  return { status, activated, suspended, refresh: read }
}
