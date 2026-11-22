import { describe, expect, test } from 'vitest'

import { gateScreen } from './use-entitlement'

/**
 * The gate's decision, pinned.
 *
 * This is the rule the reported bug got wrong, and it is a pure function for
 * exactly that reason: the trap was reachable only through a real sign-out on a
 * suspended device, so nothing that needed a mounted app could ever have caught
 * it. Here it is three booleans and one call.
 */
describe('gateScreen', () => {
  test('a device that has not been read yet is never told anything', () => {
    // `null` and `false` are different facts. Collapsing them rendered the
    // activation prompt for one frame on a perfectly activated device, which a
    // student saw as the app forgetting their code on every navigation.
    expect(gateScreen(null, false)).toBe('loading')
    expect(gateScreen(null, true)).toBe('loading')
  })

  test('a suspended device that holds its code gets the kick-out screen', () => {
    // The healthy case this feature exists for: a suspension KEEPS the code so
    // that unsuspending needs no re-entry, so `activated` stays true.
    expect(gateScreen(true, true)).toBe('suspended')
  })

  test('a signed-out device gets the activation gate, even with the suspension flag set', () => {
    // THE REPORTED TRAP. Signing out of a suspended device removed its code and
    // left `chrysalis_suspended` set, and the kick-out screen — which offers no
    // controls, by design, because its remedy is a conversation — was rendered for
    // a device that held nothing. The only way out was wiping the app's data.
    //
    // A suspension is a statement about a code. With no code there is nothing for
    // it to mean, so the activation prompt is always the right answer here.
    expect(gateScreen(false, true)).toBe('activation')
  })

  test('an ordinary device reaches the app', () => {
    expect(gateScreen(true, false)).toBe('app')
  })

  test('an unactivated device with no suspension is asked for its code', () => {
    expect(gateScreen(false, false)).toBe('activation')
  })
})
