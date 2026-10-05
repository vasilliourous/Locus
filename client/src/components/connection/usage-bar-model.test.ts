import { describe, expect, it } from 'vitest'

import type { AllowanceStatus } from '@/services/locus'

import {
  formatBytes,
  shouldAnnounceWarning,
  usageDisplay,
  usageSummary,
  type UsageCopy
} from './usage-bar-model'

/** The English templates, as the component supplies them. */
const COPY: UsageCopy = {
  used: '{{used}} of {{total}} used',
  nearlyOut: '{{used}} of {{total}} used — nearly out of free data',
  throttled: '{{used}} of {{total}} used — slowed until your allowance resets'
}

const MIB = 1024 * 1024

const metered = (over: Partial<Extract<AllowanceStatus, { state: 'metered' }>> = {}) =>
  ({
    state: 'metered',
    usedBytes: 0,
    allowanceBytes: 5120 * MIB,
    percent: 0,
    warning: false,
    throttled: false,
    ...over
  }) as AllowanceStatus

describe('formatBytes', () => {
  it('renders a gigabyte-scale figure with one decimal below ten', () => {
    expect(formatBytes(1.4 * MIB * 1024)).toBe('1.4 GB')
  })

  it('drops the decimal at ten gigabytes and above', () => {
    expect(formatBytes(12 * MIB * 1024)).toBe('12 GB')
  })

  it('renders a megabyte-scale figure', () => {
    expect(formatBytes(480 * MIB)).toBe('480 MB')
    expect(formatBytes(1.4 * MIB)).toBe('1.4 MB')
  })

  it('never renders a negative or non-finite figure as data', () => {
    // A corrupt counter must not render "NaN MB" to a student.
    expect(formatBytes(0)).toBe('0 MB')
    expect(formatBytes(-5)).toBe('0 MB')
    expect(formatBytes(Number.NaN)).toBe('0 MB')
  })
})

describe('usageDisplay', () => {
  it('shows nothing for a paying tier', () => {
    // The whole reason `unlimited` exists: a paying student must never see a
    // free-tier bar, and a bar reading "0 of 0 MB" is exactly that bug.
    expect(usageDisplay({ state: 'unlimited' })).toEqual({ kind: 'none' })
  })

  it('shows a bar for a metered tier', () => {
    const display = usageDisplay(metered({ usedBytes: 256 * MIB, percent: 5 }))
    expect(display.kind).toBe('bar')
    if (display.kind !== 'bar') throw new Error('expected a bar')
    expect(display.percent).toBe(5)
    expect(display.usedLabel).toBe('256 MB')
    expect(display.allowanceLabel).toBe('5.0 GB')
  })

  it('clamps a percentage that overflows the widget', () => {
    const display = usageDisplay(metered({ percent: 140, throttled: true }))
    if (display.kind !== 'bar') throw new Error('expected a bar')
    expect(display.percent).toBe(100)
  })

  it('takes the throttled verdict from the backend, never from the percent', () => {
    // 100% but the backend says not throttled (e.g. the threshold changed and
    // this build predates the number). The backend wins: re-deriving it here is
    // how the bar and the banner disagree.
    const display = usageDisplay(metered({ percent: 100, throttled: false }))
    if (display.kind !== 'bar') throw new Error('expected a bar')
    expect(display.throttled).toBe(false)
  })
})

describe('usageSummary', () => {
  it('is empty when there is no bar', () => {
    expect(usageSummary({ kind: 'none' }, COPY)).toBe('')
  })

  it('states the throttle plainly and says it resets', () => {
    const text = usageSummary(
      {
        kind: 'bar',
        percent: 100,
        usedLabel: '5.0 GB',
        allowanceLabel: '5.0 GB',
        warning: false,
        throttled: true
      },
      COPY
    )
    expect(text).toContain('slowed')
    expect(text).toContain('resets')
  })

  it('warns before the allowance is spent', () => {
    const text = usageSummary(
      {
        kind: 'bar',
        percent: 85,
        usedLabel: '4.3 GB',
        allowanceLabel: '5.0 GB',
        warning: true,
        throttled: false
      },
      COPY
    )
    expect(text).toContain('nearly out')
  })

  it('is a plain figure when nothing is wrong', () => {
    const text = usageSummary(
      {
        kind: 'bar',
        percent: 10,
        usedLabel: '500 MB',
        allowanceLabel: '5.0 GB',
        warning: false,
        throttled: false
      },
      COPY
    )
    expect(text).toBe('500 MB of 5.0 GB used')
  })

  it('uses the supplied copy, not a baked-in English string', () => {
    // The model takes its templates from the caller so a student's own language
    // reaches the screen. If someone "helpfully" inlines the English back into
    // the model, this fails — which is the whole point of injecting them.
    const german: UsageCopy = {
      used: '{{used}} von {{total}} benutzt',
      nearlyOut: '{{used}} von {{total}} benutzt — fast aufgebraucht',
      throttled: '{{used}} von {{total}} benutzt — gedrosselt'
    }
    const text = usageSummary(
      {
        kind: 'bar',
        percent: 100,
        usedLabel: '5.0 GB',
        allowanceLabel: '5.0 GB',
        warning: false,
        throttled: true
      },
      german
    )
    expect(text).toBe('5.0 GB von 5.0 GB benutzt — gedrosselt')
  })
})

describe('shouldAnnounceWarning', () => {
  it('announces only for a metered warning', () => {
    expect(shouldAnnounceWarning(metered({ warning: true }))).toBe(true)
    expect(shouldAnnounceWarning(metered({ warning: false }))).toBe(false)
    expect(shouldAnnounceWarning({ state: 'unlimited' })).toBe(false)
  })
})
