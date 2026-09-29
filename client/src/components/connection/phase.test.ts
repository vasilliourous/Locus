import { describe, expect, test } from 'vitest'

import type { LocusStatus } from '@/services/locus'

import { phaseFromStatus } from './phase'

/**
 * Build a status with the fields this rule reads, defaulting the rest.
 *
 * `activated`, `connected` and `ready` are the three consulted by
 * `phaseFromStatus`; the rest is filled so the object satisfies the type without
 * pulling in a factory.
 *
 * `connected` and `ready` are passed separately rather than derived from one
 * another, because their disagreement is exactly what the rule now reads. An
 * earlier version of this helper set `connected: ready`, which quietly made the
 * connecting case unconstructible — the test could not express the bug it was
 * meant to catch.
 */
const status = (
  activated: boolean,
  connected: boolean,
  ready: boolean,
): LocusStatus => ({
  activated,
  connected,
  ready,
  // The core is "up" precisely when it is running (`connected`) — the backend
  // sets `coreUp` from the readiness probe, and a running core that cannot pass
  // traffic still counts as up. `phaseFromStatus` does not read this field, but
  // the type requires it.
  coreUp: connected,
  tier: null,
  deviceId: 'abc123',
  platform: 'windows-x86_64',
  version: '3.2.0',
  subscription: { state: 'unknown' },
  lastConfirmedAt: null,
})

describe('connection phase from backend status', () => {
  /**
   * The regression that produced "the traffic panel flashes then disappears".
   *
   * A running, ready core must render as connected — under an old rule it
   * rendered as disconnected, so the panel appeared for one frame after a
   * successful connect and was then wiped by the very next status poll.
   */
  test('a READY core reads as connected, not disconnected', () => {
    expect(phaseFromStatus(status(true, true, true))).toBe('connected')
  })

  test('an activated device with the core down reads as disconnected', () => {
    expect(phaseFromStatus(status(true, false, false))).toBe('disconnected')
  })

  /**
   * The bug this whole rule exists for: a core that is RUNNING but NOT READY
   * must read as `connecting`, never as `connected` and never as `disconnected`.
   *
   * `connected` is the engine process existing, which happens seconds before the
   * tunnel can carry a packet. Latching onto it announced a working tunnel during
   * the dialling window — the "jumps straight to connected even though nothing is
   * routed" report. Collapsing the same window to `disconnected` (the previous
   * fix) was also wrong: it left no state that said work was in progress, so the
   * button jumped from "not connected" to "connected" with nothing in between.
   */
  test('a running-but-not-ready core reads as connecting', () => {
    const starting: LocusStatus = status(true, true, false)
    expect(phaseFromStatus(starting)).toBe('connecting')
    expect(phaseFromStatus(starting)).not.toBe('connected')
    expect(phaseFromStatus(starting)).not.toBe('disconnected')
  })

  /**
   * The app-open case, stated as its own test because it is the one a student
   * actually hits: the status is read once on launch, so a core left dialling by
   * a previous session must render as `connecting` — not as a settled state with
   * no transition.
   */
  test('a half-dialled core found on app open reads as connecting', () => {
    expect(phaseFromStatus(status(true, true, false))).toBe('connecting')
  })

  /**
   * An unactivated device must never show "connected" even if a stale core is
   * somehow running — the activation gate owns the UI, and the connect control
   * is not reachable from there.
   */
  test('an unactivated device is unknown regardless of core state', () => {
    expect(phaseFromStatus(status(false, false, false))).toBe('unknown')
    expect(phaseFromStatus(status(false, true, false))).toBe('unknown')
    expect(phaseFromStatus(status(false, true, true))).toBe('unknown')
  })

  /**
   * Readiness, not mere activation, is what earns the green light: a device whose
   * code is bound but whose core is idle must not read as connected.
   */
  test('activation alone does not read as connected', () => {
    expect(phaseFromStatus(status(true, false, false))).not.toBe('connected')
  })

  /**
   * The reported bug, as a named case: the app said **connected** on a machine
   * with no usable network.
   *
   * The Core process was up, so the old backend reported `ready` from a latch set
   * when the launch call returned, and this rule — reading `ready` honestly —
   * rendered "connected" for a tunnel that could not carry a packet. Readiness is
   * now probed against the Core's control API, so a Core that cannot answer
   * arrives here as `connected && !ready`.
   *
   * This test pins the SCREEN's half of that contract: given the corrected
   * backend, a running-but-unanswering core must never render as connected.
   */
  test('a core that cannot answer never reads as connected', () => {
    const noNetwork: LocusStatus = status(true, true, false)
    expect(phaseFromStatus(noNetwork)).not.toBe('connected')
    expect(phaseFromStatus(noNetwork)).toBe('connecting')
  })
})
