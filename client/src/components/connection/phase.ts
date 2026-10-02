import type { LocusStatus } from '@/services/locus'

/**
 * The connection phase a settled (not mid-transition) status implies.
 *
 * Lives in its own module, importing only a type, for one reason: this rule is
 * what the connect bug got wrong, and a rule that can only be exercised by
 * mounting React hooks against a Tauri mock is a rule that regresses unnoticed.
 * As a pure function it is unit-tested directly (`phase.test.ts`).
 *
 * The outcomes, in priority order:
 *
 *   - not activated                     -> `unknown` (the activation gate owns the UI)
 *   - activated, core not running       -> `disconnected`
 *   - activated, core running, not ready -> `connecting`
 *   - activated, core running and ready -> `connected`
 *
 * **Both `connected` and `ready` are consulted, and their disagreement IS the
 * connecting phase.** `connected` means the engine process exists; `ready` means
 * it has bound its ports and can carry a packet. That gap is seconds long on a
 * real connection.
 *
 * `ready` is **observed, not assumed**, and it is observed twice over
 * (`CoreManager::observe_readiness`): once by asking the Core's own control API,
 * and once by making a real request *through* the tunnel. That matters here: the
 * rule below is only honest because the backend refuses to report `ready` for a
 * Core it cannot get an answer from **and** for a Core that answers locally but
 * cannot carry a packet. A Core that is up but unreachable — a machine with no
 * usable network, an engine that has died, a captive portal — reports
 * `connected && !ready`, which this renders as `connecting`, never as
 * `connected`. This is the school-wifi bug: the engine binds its control port
 * and answers whether or not the internet works, so a local-only check called it
 * connected while every packet died.
 *
 * The previous rule read only `ready` and sent everything else to
 * `disconnected`. That was still a lie, just a quieter one: a student who pressed
 * Connect saw the button sit on "not connected" for the whole dialling window and
 * then snap to "connected" — no third state, nothing that said work was in
 * progress. And because the status is also read once on app open, a core left
 * running from a previous session settled straight onto `connected` with no
 * transition at all, before a single packet had been carried.
 *
 * Deriving `connecting` from `connected && !ready` fixes both without the UI
 * having to guess: the transition is a fact the backend reports, so a fresh
 * app-open that finds a half-dialled core renders "connecting" honestly, and a
 * core that starts and then dies settles back to `disconnected` on the next poll
 * instead of leaving a green light on.
 */
export type ConnectionPhase =
  /**
   * No read has settled yet. NOT the same fact as `unactivated`.
   *
   * These were one phase (`unknown`) until it caused a visible lie: the screen
   * rendered "This device needs an activation code before it can connect" for the
   * one render between mounting and the first `locusStatus()` reply, on a device
   * that was perfectly well activated. The reporter saw it on every navigation
   * Home -> Settings, for "milliseconds, sometimes over a second" — the length of
   * the status read, which proves the tunnel with a real request through it and so
   * is slow exactly when a connection is settling.
   *
   * So "we have not asked yet" and "we asked, and this device is not activated"
   * are separate states. Only the second one may tell a student to find their card.
   */
  | 'checking'
  /** Read, and this device holds no entitlement. The activation prompt applies. */
  | 'unactivated'
  | 'disconnected'
  | 'connecting'
  | 'disconnecting'
  | 'connected'

export const phaseFromStatus = (status: LocusStatus): ConnectionPhase => {
  if (!status.activated) return 'unactivated'
  // Running but not yet able to carry a packet: this is the connecting window,
  // and it is a backend fact rather than a UI guess.
  //
  // `ready` is the ONLY thing that means connected, on both sides of the wire —
  // the backend sets it solely from a packet proven through the tunnel, and this
  // is the only place that reads it as "connected". A backend that reported
  // readiness from anything cheaper (the Core answering, the process existing)
  // would make this line lie, and the reverse — deriving more states here than
  // `ready` expresses — is what the old `NoEgress`/`NotReady` split did before it
  // was collapsed. One fact, one place.
  if (status.connected && !status.ready) return 'connecting'
  return status.ready ? 'connected' : 'disconnected'
}
