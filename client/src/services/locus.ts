import { invoke } from '@tauri-apps/api/core'

import type {
  LocusStatus,
  ValidateCodeResult,
  ActivationResult,
  ConnectionResult,
  UpdateStatus,
} from './locus.generated'

/**
 * The Locus product surface.
 *
 * Every call returns a TYPED result rather than a string, because the situations
 * a student can hit are genuinely different and only some of them are their
 * fault: a code bound to their old laptop is not the same as a hub they cannot
 * reach. Collapsing those into "activation failed" is what made the retired
 * client's support conversations start from nothing.
 */

// ── The wire types, generated from the Rust ──────────────────────────────────
//
// These nine types cross the Tauri IPC boundary and are MIRRORS of the Rust
// types in `src-tauri/src/cmd/locus.rs`. They are generated, not hand-written:
// `server/scripts/gen-ts-contract.py` reads the Rust and emits
// `./locus.generated.ts`, and CI fails if the committed output disagrees with
// the Rust.
//
// WHY. Before this, each interface was hand-maintained beside its `invoke()`
// call, with `check-consistency.sh` §33 asserting agreement for ONE of the nine.
// The other eight drifted silently by construction: a field added to Rust and
// not to TypeScript serialises and is ignored, so a feature does nothing; a
// field read by TypeScript that Rust never sends renders as `undefined`. That is
// the `uot_port` defect one layer in, and it is the layer that changes most
// often — `FIXES.md` records exactly this for `udp`.
//
// The fix is not a tenth assertion. It is that there is now one side to write.
// Change the Rust type, run `python3 server/scripts/gen-ts-contract.py --write`,
// and commit the diff — which shows the wire shape changing, reviewably.
//
// The prose that used to sit above each of these types now lives in
// `docs/reference/CONTRACT.md` (the contract as data) and in the Rust doc
// comments, which the generator carries across verbatim.
export type {
  LocusStatus,
  UdpStatus,
  AllowanceStatus,
  SubscriptionStatus,
  ValidateCodeResult,
  ActivationResult,
  ConnectionResult,
  UpdateStatus,
  UpdateProgress,
} from './locus.generated'

export interface CodeCheck {
  ready: boolean
  tier: string | null
  expiresAt: string | null
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

