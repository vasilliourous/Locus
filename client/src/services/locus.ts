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
   * The middle state: `connected && coreUp && !ready` means the tunnel exists
   * but the internet is not reachable through it (no uplink, dead server,
   * captive portal). Lets the screen say "connected, but no traffic" instead of
   * claiming success or claiming the app is off. `false` when the core is not up.
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

/**
 * What a first-launch device-recognition check concluded.
 *
 * Three cases, and the third is why this is not a boolean: a hub we could not
 * reach is not the same as a hub that does not know this device. Both lead to
 * the code prompt, but only the second is a statement about the student's
 * account, and telling them "your device is not recognised" during an outage
 * would be a false claim about their entitlement.
 */
export type RecognitionResult =
  | {
      kind: 'recognised'
      tier: string
      expiresAt: string | null
      /**
       * Whether this device's identity survives a reinstall.
       *
       * `false` means the identity lives only in the app config, so a future
       * reinstall will lose it and need the card again. The UI must say so
       * rather than claim the device is remembered — over-promising here is the
       * support call this whole feature exists to remove.
       */
      durable: boolean
    }
  | { kind: 'unknownDevice' }
  | { kind: 'unavailable' }

/**
 * Asks the hub whether this device already holds an entitlement.
 *
 * Called on first launch with nothing stored, so a reinstalling student can skip
 * the code prompt. Never binds, never activates, and the hub's answer never
 * contains the activation code — it authenticates on a value the device proves
 * it holds, and reports only whether this device has a live tier.
 *
 * Every failure path resolves to something the caller can fall through to the
 * code prompt for, so a student holding their card is never worse off.
 */
export const locusRecognise = () =>
  invoke<RecognitionResult>('locus_recognise')

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
