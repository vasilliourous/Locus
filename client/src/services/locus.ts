import { invoke } from '@tauri-apps/api/core'

/**
 * The Locus product surface.
 *
 * Every call returns a TYPED result rather than a string, because the situations
 * a student can hit are genuinely different and only some of them are their
 * fault: a code bound to their old laptop is not the same as a hub they cannot
 * reach. Collapsing those into "activation failed" is what made the retired
 * client's support conversations start from nothing.
 */

export interface LocusStatus {
  activated: boolean
  tier: string | null
  /** Truncated fingerprint — enough for support to correlate, not a shareable id. */
  deviceId: string
  platform: string | null
  version: string
  /**
   * Whether the tunnel's core is running, as the backend reports it.
   *
   * The connection screen MUST derive its connected/disconnected state from this
   * rather than from "the connect command returned Ok". The backend owns the
   * answer; a UI that guesses can (and did) show a connected panel and then reset
   * it on the next poll.
   */
  connected: boolean
  /**
   * Whether the core is up AND traffic is actually getting through.
   *
   * Distinct from `connected` on purpose. `connected` is "the engine process
   * exists"; this is "the engine is up AND a real request completed through the
   * tunnel". The screen derives its state from THIS, because announcing a tunnel
   * during the dialling window — or on a machine whose uplink is down while the
   * engine still answers locally — is how a student ends up being told they are
   * connected by a tunnel that is not carrying anything.
   */
  ready: boolean
  /**
   * Whether the core is up, whether or not traffic is flowing.
   *
   * `connected && coreUp && !ready` is the connecting window: the tunnel exists
   * but no packet has been proven through it yet (still dialling, no uplink, a
   * dead server, a captive portal). Lets the screen say "connecting" instead of
   * flashing "disconnected" at a core that is coming up. `false` when the core
   * is not running at all.
   *
   * Note the backend does **not** distinguish the causes. They differ in remedy,
   * not in what the student should do — wait, and tell support if it persists —
   * and the client renders them identically, so reporting one state is the
   * honest answer rather than a finer one the UI would discard.
   */
  coreUp: boolean
  subscription: SubscriptionStatus
  /**
   * Unix seconds of the last heartbeat the hub accepted, or `null` if none ever has.
   *
   * Answers "how current is this status?", which the expiry alone cannot: a
   * subscription confirmed a minute ago and one confirmed a week ago render
   * identically without it. `null` is not "just now" — it is "never confirmed",
   * and the two are shown differently.
   */
  lastConfirmedAt: number | null

  /**
   * Whether the core is observed to be moving bytes right now.
   *
   * The second, independent route to `ready`, reported separately so a support
   * report can say which one answered — the two fail for different reasons, and
   * only one of them can be wrong in the direction that produced the recurring
   * "connecting while traffic flows" report.
   *
   * Note this is the *current rate*, not the core's lifetime total: a tunnel that
   * stops carrying bytes stops reporting `true` on the next poll. See
   * `src-tauri/src/core/manager/traffic_probe.rs`.
   */
  trafficFlowing: boolean

  /**
   * The free tier's allowance state — what the usage bar, the warning and the
   * throttle banner all read.
   *
   * A tagged union rather than loose numbers, mirroring the Rust enum. The
   * distinction that matters is `unlimited` versus a metered zero: a paying tier
   * (or an older hub) has no allowance at all and must render no bar and throttle
   * nothing, where a free student who has used nothing must see a bar at 0%.
   * Collapsing the two is how a paying student gets shown a free-tier banner.
   *
   * The backend has already classified it; the UI renders these values and does
   * no arithmetic of its own, so the warning line and the throttle line cannot
   * drift between the two languages.
   */
  allowance: AllowanceStatus
}

/**
 * The free tier's allowance, as the backend classified it.
 *
 * `unlimited` is the answer for every paying tier and for a hub that predates
 * the free tier. It is deliberately not "metered with an allowance of zero" —
 * a zero allowance would throttle every student, and an absent key must never
 * gate a paying user's traffic.
 */
export type AllowanceStatus =
  | { state: 'unlimited' }
  | {
      state: 'metered'
      /** Bytes counted in the current 30-day window. */
      usedBytes: number
      /** The window's allowance, in bytes. */
      allowanceBytes: number
      /** Percentage used, clamped to 0–100. Ready to render. */
      percent: number
      /** Past the 80% line but not yet spent. */
      warning: boolean
      /** The allowance is spent; the connection carries the throttle. */
      throttled: boolean
    }

/**
 * The subscription state, already decided by the backend.
 *
 * A tagged union rather than an optional date, mirroring the Rust enum. The
 * important distinction is `unknown` versus `lapsed`: a device that has never
 * heard from the hub has no expiry, and showing that as "expired" would tell a
 * brand-new student their subscription had run out.
 *
 * `unknown` means render nothing about expiry.
 */
export type SubscriptionStatus =
  | { state: 'unknown' }
  | { state: 'lapsed' }
  | {
      state: 'active'
      /** Whole days remaining, rounded up. */
      daysRemaining: number
      /** Whether this is inside the renewal-warning window. */
      urgent: boolean
      /** The hub's own date string, for a tooltip. */
      expiresAt: string
    }
  | {
      /**
       * The hub refused this device and the entitlement was withdrawn.
       *
       * Its own state, not `unknown`, because after a refusal there genuinely is
       * no subscription to describe — and rendering that as "nothing to show"
       * leaves a student whose access was ended with no idea why. This is the one
       * state that always has something to say.
       */
      state: 'refused'
      /** The hub's own sentence, verbatim. Shown as-is; never paraphrased. */
      reason: string
    }

export interface ValidateCodeResult {
  valid: boolean
  /** The canonical hyphenated form, so the UI can show what will be sent. */
  canonical: string | null
  message: string | null
}

export interface CodeCheck {
  ready: boolean
  tier: string | null
  expiresAt: string | null
  message: string
}

export interface ActivationResult {
  code: string
  tier: string
  udpRelay: boolean
  /** False while the config-apply path is still being wired. */
  configApplied: boolean
  message: string
}

export const locusStatus = () => invoke<LocusStatus>('locus_status')

/** Offline checksum check. No network — safe to call on every keystroke. */
export const locusValidateCode = (code: string) =>
  invoke<ValidateCodeResult>('locus_validate_code', { code })

/** Read-only hub pre-check: is this code real, and is it usable here? */
export const locusCheckCode = (code: string) =>
  invoke<CodeCheck>('locus_check_code', { code })

// `RecognitionResult` and `locusRecognise` — REMOVED.
//
// Device recognition is gone: the client no longer asks the hub whether it
// remembers this device, because nothing identifies a device any more. A student
// who reinstalls keeps their access because the activation code itself is
// persisted to a machine-scoped store (see locus/credential.rs).

export const locusActivate = (code: string) =>
  invoke<ActivationResult>('locus_activate', { code })

export const locusHubUrl = () => invoke<string>('locus_hub_url')

/**
 * Asks the client to confirm the subscription with the hub right now, and waits
 * for that confirmation to complete.
 *
 * Resolves `true` when a beat ran, `false` when nothing is beating (no
 * activation, or the loop is stopped) — so a caller can distinguish "we asked and
 * it finished" from "there was nobody to ask", and say so rather than reporting a
 * check that never happened.
 *
 * It deliberately does NOT return the subscription state: the hub's answer is
 * interpreted in exactly one place (the heartbeat's own outcome handler, which
 * also stops the tunnel on a refusal), and returning a verdict here would be a
 * second interpretation that could disagree with it. Callers re-read status after
 * this settles.
 *
 * **The wait is the point.** This used to resolve as soon as the beat was
 * requested, which made a UI control's pending state last one IPC round trip
 * (milliseconds) while the work it claimed to be reporting had not started. A
 * progress indicator must bracket the operation, and this resolve point is now
 * that boundary.
 */
export const locusCheckSubscription = () =>
  invoke<boolean>('locus_check_subscription')

export const locusUpdateStagingDir = () =>
  invoke<string>('locus_update_staging_dir')

export interface ConnectionResult {
  connected: boolean
  message: string
}

/**
 * Brings the tunnel up, and waits for the core to actually be ready.
 *
 * Resolves when the tunnel can carry traffic, NOT when the engine process was
 * spawned — the gap between those two is what made the button say "Connected"
 * while nothing was routed yet. A timeout returns `connected: false` with the
 * backend's own explanation rather than hanging, so the button can always be
 * pressed again.
 */
export const locusConnect = () => invoke<ConnectionResult>('locus_connect')

/**
 * Abandons an in-flight connect.
 *
 * Exists so the connecting button stays pressable: a connect that cannot be
 * cancelled is the failure mode where a student is left with a spinner and no
 * move but to kill the app.
 */
export const locusCancelConnect = () => invoke<void>('locus_cancel_connect')

/**
 * Brings the tunnel down.
 *
 * Safe to call when already disconnected — the backend stops the core
 * unconditionally, which is what prevents the "engine still running but the app
 * thinks it is not" state the retired client could not escape from.
 */
export const locusDisconnect = () =>
  invoke<ConnectionResult>('locus_disconnect')

/** The update state the prompt renders from. */
export interface UpdateStatus {
  /** The version currently running. */
  currentVersion: string
  /**
   * A version the hub has offered, if any.
   *
   * Recorded when a heartbeat carries the signal, so the prompt survives a
   * restart instead of existing only in the instant after a beat. `null` means
   * nothing has been offered — which is NOT the same as "up to date".
   */
  offeredVersion: string | null

  /**
   * Why no update is being offered, when the last heartbeat did not offer one.
   *
   * The answer to "why is this device not updating?", which had no answer before
   * — every reason was logged at `debug` and the default level is `Info`, so a
   * suppressed offer left no trace anywhere. A macOS student on 3.2.24 reported
   * exactly that and there was nothing on the machine to diagnose it with.
   *
   * `null` means nothing to report: either an offer exists, or no beat has run.
   * Deliberately not set for the ordinary "up to date" case.
   */
  noOfferReason: string | null

  /**
   * Whether automatic update checking is enabled.
   *
   * Surfaced because it is the one cause a student can fix themselves, and the
   * one most likely to be stuck: the field is inherited from upstream Clash
   * Verge Rev, so it can be `false` from an era when the Account row was
   * mis-wired to auto-launch. The UI points at that control directly.
   */
  automaticChecksEnabled: boolean
}

/** Reports the update state. Never touches the network. */
export const locusUpdateStatus = () =>
  invoke<UpdateStatus>('locus_update_status')

/**
 * Downloads and installs the offered update.
 *
 * The plugin owns the download and the mandatory signature verification; this
 * does not re-implement the swap. On Windows the installer launches and the app
 * exits, so this may never resolve — callers must treat completion as "expect a
 * restart" rather than assuming the promise settles.
 */
export const locusInstallUpdate = () => invoke<void>('locus_install_update')

/**
 * Forgets an offered update without installing it.
 *
 * Called when the student dismisses the prompt, so they are not asked again for
 * the same version on every launch. A newer offer still prompts.
 */
export const locusDismissUpdate = () => invoke<void>('locus_dismiss_update')

/** Progress of an in-flight update download, from `locus://update-progress`. */
export interface UpdateProgress {
  /** Bytes downloaded so far, cumulative — not the size of one chunk. */
  chunkLength: number
  /** Total bytes, when the server advertised a length. `null` means unknown. */
  contentLength: number | null
}
