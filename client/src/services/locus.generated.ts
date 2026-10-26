// AUTO-GENERATED — do not edit by hand.
//
// Source: client/src-tauri/src/cmd/locus.rs
// Tool:   server/scripts/gen-ts-contract.py
//
// Every type in this file is a mechanical mirror of a Rust type that crosses the
// Tauri IPC boundary. There is exactly one side to write — the Rust — so the two
// cannot disagree. Editing this file directly is a bug: the change is erased by
// the next regeneration and fails `--check` in CI before it can ship.
//
// To change a wire shape, change the Rust type and run:
//   python3 server/scripts/gen-ts-contract.py --write
//
// Prose documentation for these types lives in services/locus.ts beside the
// functions that return them. This file carries the Rust doc comments as
// context, but the decision to document a field belongs with the hand-written
// file, which is not generated.


/**
 * Whether the app has an activation to work with, and what it is.
 *
 * The single call the UI polls. Deliberately small and cheap: the activation
 * gate renders on every launch, and a slow first paint is the first thing a
 * student notices.
 */
export interface LocusStatus {
  /**
   * Whether this device has a code bound and accepted.
   */
  activated: boolean
  /**
   * The tier name as the hub sent it (`free`/`eco`, or `strike`), when known.
   * This is the WIRE name, not the label the student reads — the frontend
   * maps it in `tier-badge.tsx`, which is the single place that mapping
   * lives, so the two can never disagree.
   */
  tier: string | null
  /**
   * The device fingerprint, **truncated** — enough for support to correlate,
   * not enough to be a useful identifier if someone screenshots it.
   */
  deviceId: string
  /**
   * Which platform build this is, so a support report says so without asking.
   */
  platform: string | null
  /**
   * The running app version.
   */
  version: string
  /**
   * Whether the tunnel's core is actually running right now.
   *
   * This is the authoritative answer to "is the VPN up?", read from the same
   * run state the core manager publishes — **not** inferred by the UI from the
   * fact that a connect command once returned `Ok`.
   *
   * It exists because the UI could not answer that question and guessed
   * instead. `locus_status` carried only entitlement (`activated`), so after a
   * successful connect the connection screen's periodic status re-read found
   * `activated == true` and concluded "not connected" — the traffic panel the
   * student had just earned flashed on for one frame and vanished, and every
   * poll reset it again. A field the backend owns is the fix: the screen can
   * now render what is true rather than what it last remembers deciding.
   *
   * `NotRunning` is the only "down" answer.
   */
  connected: boolean
  /**
   * Whether the core is up *and* ready to carry traffic.
   *
   * Deliberately separate from [`Self::connected`]. `connected` means the
   * engine process exists; this means it has bound its ports and can route a
   * packet. The gap between them is seconds long on a real connection, and
   * reporting "connected" across it is what made the button lie: a student who
   * pressed Connect and immediately re-checked their address was told they
   * were not connected by a tunnel that was merely still dialling.
   *
   * The screen renders its state from THIS field. `connected` remains what it
   * always was — an honest statement that a process is running — and the two
   * disagreeing is meaningful rather than a bug.
   *
   * Since the egress fix, `ready` means the core is up **and a real request
   * completed through the tunnel**. A core that is running on a machine with
   * no usable uplink — or pointed at an unreachable server, or behind a
   * captive portal — reports `ready: false` while `connected: true`. That is
   * the honest answer, and the one the screen must not round up to
   * "connected".
   */
  ready: boolean
  /**
   * Whether the core is up, regardless of whether traffic is flowing.
   *
   * This is the middle state between "nothing running" and "connected":
   * `connected && coreUp && !ready` is precisely *the tunnel exists but the
   * internet is not reachable through it*. It is what lets the screen say
   * "connected, but no traffic" instead of either claiming success or
   * claiming the app is off. `false` whenever the core is not serving.
   */
  coreUp: boolean
  /**
   * The subscription state, already parsed — see [`SubscriptionStatus`].
   *
   * Pre-parsed rather than handed to the UI as a raw date string, because the
   * "never invent a date" rule needs exactly one decision point. If the
   * frontend received the raw string it would have to decide what an empty
   * value means, and each screen could decide differently.
   */
  subscription: SubscriptionStatus
  /**
   * Unix seconds of the last heartbeat the hub accepted, when there has been.
   *
   * Exposed so the Account screen can answer "how current is this?" — a
   * student who has just renewed needs to know whether the app has actually
   * confirmed it, and "expires in 30 days" reads identically whether it was
   * confirmed a minute ago or a week ago. Without this the only honest answer
   * to "did my renewal land?" is a guess.
   *
   * `None` means no beat has ever succeeded — which is a different statement
   * from "confirmed zero seconds ago", and rendering one as the other would
   * tell a fresh install its status was just verified.
   */
  lastConfirmedAt: number | null
  /**
   * Whether the Core is observed to be moving bytes right now.
   *
   * The second route to `ready`, reported alongside it rather than folded into
   * it, for the same reason `ready` is separate from `connected`: the two
   * routes fail for different reasons and a support report has to say which
   * one answered. `ready: true, traffic: false` is a tunnel proven by a delay
   * test on an idle machine; `ready: true, traffic: true` is the student's own
   * bytes moving. Both are connected, and only one of them can be wrong in the
   * direction that produced the recurring report — so this field is what lets
   * the next investigation tell them apart without a debug build.
   *
   * It is a *rate*, not the Core's lifetime total: see
   * `core::manager::traffic_probe`.
   */
  trafficFlowing: boolean
  /**
   * The free tier's allowance state — what the usage bar, the warning and the
   * throttle banner all read.
   *
   * A closed set rather than loose numbers, for the same reason
   * [`SubscriptionStatus`] is: "no allowance applies" (a paying tier, or a hub
   * that predates the field) and "zero bytes used of 5 GiB" must not be
   * confusable. The first must never show a bar or throttle anything; the
   * second must show a bar at 0%.
   *
   * The UI renders from this and never recomputes: the classification is the
   * behaviour, and a second implementation in TypeScript is how the warning
   * line and the throttle line drift apart.
   */
  allowance: AllowanceStatus
  /**
   * How this tier's UDP-over-TCP endpoint is configured, or why there is not
   * one.
   *
   * # Why this is reported rather than left to be inferred
   *
   * The free plan now advertises a UoT endpoint of its own (8447, its own
   * sing-box listener, its own 1 Mbps class). When any of the three parts —
   * the `udp_relay` flag, the `uot_port`, or the listener behind it — is
   * missing or stale, the symptom a student reports is **not** "UDP is not
   * tunnelled". It is "the free plan does not connect", or a crash, because the
   * generated profile then carries a profile whose first rule points at an
   * outbound that cannot carry anything. The proxy group's default member is
   * the TCP proxy, so TCP keeps working while the very first rule in the
   * document sends every datagram at a dead endpoint.
   *
   * Nothing in the client could say which of those was true: the tier payload
   * is stored as one opaque blob, and the connect path reduces it to a single
   * boolean. This field is the missing sentence, so the next report of "free
   * does not work" starts from a fact instead of from a rebuild.
   */
  udp: UdpStatus
}

/**
 * The result of the offline code check.
 */
export interface ValidateCodeResult {
  valid: boolean
  /**
   * The canonical hyphenated form, so the UI can show what will be sent.
   */
  canonical: string | null
  message: string | null
}

/**
 * What a successful activation produced.
 */
export interface ActivationResult {
  code: string
  tier: string
  udpRelay: boolean
  /**
   * Whether the tunnel configuration was written and applied. False while the
   * store and apply path are still being wired, so the UI can say so rather
   * than implying the VPN is ready.
   */
  configApplied: boolean
  message: string
}

/**
 * The result of a connect or disconnect.
 */
export interface ConnectionResult {
  connected: boolean
  message: string
}

/**
 * What the UI should show about updates right now.
 */
export interface UpdateStatus {
  /**
   * The running version.
   */
  currentVersion: string
  /**
   * A version the hub has offered and that is worth prompting about, if any.
   *
   * Read from storage rather than fetched, so the prompt can appear on launch
   * without waiting for a heartbeat — the offer is recorded when a beat
   * delivers it, and this is what the popup reads.
   */
  offeredVersion: string | null
  /**
   * Why no update is being offered, when the last heartbeat did not offer one.
   *
   * The answer to "why is this device not updating?" — a question that had no
   * answer before, because every reason was logged at `debug` and the default
   * level is `Info`. A student on macOS 3.2.24 reported exactly that ("the
   * update wasn't offered") and nothing on the machine said why.
   *
   * `None` means nothing to report: either an offer exists, or no beat has run.
   * Deliberately **not** set in the ordinary "up to date" case, so a healthy
   * install shows no notice.
   */
  noOfferReason: string | null
  /**
   * Whether automatic update checking is enabled.
   *
   * Surfaced alongside the reason because it is the one cause a student can
   * fix themselves, and the one most likely to be stuck: `auto_check_update`
   * is inherited from upstream Clash Verge Rev, so it can be `false` from an
   * era when the Account-page row was mis-wired to auto-launch. The UI uses
   * this to point at the exact control rather than leaving them to hunt.
   */
  automaticChecksEnabled: boolean
}

/**
 * Progress of an in-flight update download, emitted as `locus://update-progress`.
 */
export interface UpdateProgress {
  /**
   * Bytes downloaded so far, **cumulative** — not the size of one chunk.
   *
   * Cumulative rather than per-chunk on purpose: the UI computes a percentage
   * from it directly, and a per-chunk value would need the frontend to keep a
   * running total that could disagree with the backend's.
   */
  chunkLength: number
  /**
   * Total bytes, when the server advertised a length.
   *
   * `None` means the hub used a chunked response, and the UI should show an
   * indeterminate bar rather than a stalled 0%.
   */
  contentLength: number | null
}

/**
 * Whether this tier's UDP path is configured, and what it is configured to do.
 *
 * Deliberately a closed set: the three failure modes are distinct, and collapsing
 * them (as a single `bool` does) is what made them indistinguishable in the field.
 */
export type UdpStatus =
  /**
   * The hub advertises no UDP for this tier. UDP travels raw inside the TCP
   * proxy's `udp: true`, which is correct for a TCP-only tier and is what a
   * paying tier looked like before UoT existed.
   */
  | { kind: 'none' }
  /**
   * Configured and handed to the core: `udp_relay` is on and the port is
   * non-zero, so the profile carries `Locus-UoT` and pins UDP to it.
   *
   * **Configured is not the same as working.** It says the document is
   * coherent, not that the listener is up or that the credentials match. The
   * only observation that settles that is traffic through it, which is what
   * `traffic_flowing` and `ready` report.
   */
  | {
      kind: 'configured'
      port: number
    }
  /**
   * Advertised but unusable, with the reason. This is the state that used to
   * be invisible and present as "the plan is broken".
   */
  | {
      kind: 'unusable'
      reason: string
    }

/**
 * What the Connection screen should show about the free tier's allowance.
 *
 * Mirrors [`crate::locus::usage::AllowanceState`], plus the presentation fields
 * the bar needs, so the frontend does no arithmetic at all.
 */
export type AllowanceStatus =
  /**
   * No allowance applies. A paying tier, or an older hub. Render nothing and
   * never throttle.
   */
  | { state: 'unlimited' }
  /**
   * An allowance applies. Carries everything the UI needs.
   */
  | {
      state: 'metered'
      usedBytes: number
      allowanceBytes: number
      percent: number
      warning: boolean
      throttled: boolean
    }

/**
 * What the Account screen should say about the subscription.
 *
 * A closed set of cases rather than an optional date, because "we do not know"
 * and "it has lapsed" are different answers with different copy, and collapsing
 * them into one nullable field is how a blank or wrong date reaches a paying
 * student.
 *
 * `rename_all` on an enum renames the *variants*, not the fields inside them, so
 * the struct-variant fields need `rename_all_fields` to reach the frontend as
 * `daysRemaining`/`expiresAt`. Without it serde emits `days_remaining` and
 * `expires_at`, the TypeScript type reads `undefined` for both, and the UI
 * renders "{{count}} days" with an empty count — or nothing at all. A test
 * below pins the wire shape and fails if this attribute is dropped.
 */
export type SubscriptionStatus =
  /**
   * The hub has not told us. Render nothing about expiry.
   */
  | { state: 'unknown' }
  /**
   * The date has passed.
   */
  | { state: 'lapsed' }
  /**
   * Valid, with whole days remaining and the date for a tooltip.
   */
  | {
      state: 'active'
      daysRemaining: number
      urgent: boolean
      expiresAt: string
    }
  /**
   * Whole days remaining, rounded up. `0` is possible only mid-day.
   * Whether this is inside the renewal-warning window.
   * The hub's own string, so the UI can show the exact date on hover
   * without re-deriving it.
   * The hub refused this device and the entitlement was withdrawn.
   *
   * Its own state, ahead of [`Unknown`](Self::Unknown), because after a
   * refusal the client legitimately knows *nothing* about a subscription — the
   * code is gone — and reporting that as `Unknown` would be technically true
   * and completely useless. "We have nothing to show you" and "your access was
   * ended, and here is why" call for different things on screen, and the
   * second is the one a student can act on.
   *
   * Carries the hub's own sentence, so a suspension and an expiry are not
   * flattened into one message when the student's next step differs.
   */
  | {
      state: 'refused'
      reason: string
    }
