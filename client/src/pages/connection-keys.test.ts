import { describe, expect, test } from 'vitest'

import { shouldToggleFromKey, type ToggleKeyEvent } from './connection-keys'

/** A key press nothing is focused for — the common case on the Home screen. */
const bare = (overrides: Partial<ToggleKeyEvent> = {}): ToggleKeyEvent => ({
  key: 'Enter',
  repeat: false,
  targetTagName: 'BODY',
  targetIsContentEditable: false,
  ...overrides,
})

describe('keyboard toggle for the tunnel', () => {
  test('Enter and Space both toggle', () => {
    expect(shouldToggleFromKey(bare({ key: 'Enter' }))).toBe(true)
    expect(shouldToggleFromKey(bare({ key: ' ' }))).toBe(true)
  })

  test('no other key toggles', () => {
    for (const key of ['a', 'Escape', 'Tab', 'F5', 'ArrowDown']) {
      expect(shouldToggleFromKey(bare({ key })), key).toBe(false)
    }
  })

  /**
   * The failure this rule exists for.
   *
   * A student typing a code on the activation screen presses Space between
   * groups, and without this the app would try to bring a tunnel up behind the
   * screen they are typing on. Every text-entry surface must swallow the key.
   */
  test('a text field keeps its own Space and Enter', () => {
    for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) {
      expect(
        shouldToggleFromKey(bare({ key: ' ', targetTagName: tag })),
        tag,
      ).toBe(false)
      expect(shouldToggleFromKey(bare({ targetTagName: tag })), tag).toBe(false)
    }
  })

  /**
   * The Connect button itself is a BUTTON. If it were allowed through, one
   * press could toggle twice — once from the button's own click and once from
   * the global handler.
   */
  test('a focused button is left to handle its own press', () => {
    expect(shouldToggleFromKey(bare({ targetTagName: 'BUTTON' }))).toBe(false)
    expect(
      shouldToggleFromKey(bare({ key: ' ', targetTagName: 'BUTTON' })),
    ).toBe(false)
  })

  /**
   * Auto-repeat. Holding Enter must not fire a connect per repeat frame — the
   * operation is expensive and the cancel path would race itself.
   */
  test('a held key does not repeat the action', () => {
    expect(shouldToggleFromKey(bare({ repeat: true }))).toBe(false)
    expect(shouldToggleFromKey(bare({ key: ' ', repeat: true }))).toBe(false)
  })

  /**
   * Content-editable is not reachable today, but it is the same class of surface
   * as a text field and would otherwise start a VPN behind the cursor the first
   * time one is added.
   */
  test('a content-editable surface keeps its own keys', () => {
    expect(shouldToggleFromKey(bare({ targetIsContentEditable: true }))).toBe(
      false,
    )
    expect(
      shouldToggleFromKey(bare({ key: ' ', targetIsContentEditable: true })),
    ).toBe(false)
  })

  /**
   * An anchor behaves like a button: Enter activates the link, which navigates.
   * Toggling as well would both navigate and change the tunnel from one press.
   */
  test('a focused link is left alone', () => {
    expect(shouldToggleFromKey(bare({ targetTagName: 'A' }))).toBe(false)
  })

  /** No focused element at all is still the common case, and must work. */
  test('a press with nothing focused still toggles', () => {
    expect(shouldToggleFromKey(bare({ targetTagName: '' }))).toBe(true)
  })
})
