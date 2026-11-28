// The subscription data source, replaced for the harness ONLY.
//
// `LayoutSubscription` renders whatever `useSubscription()` returns, and that
// hook is backed by the Tauri IPC + a query cache — neither of which exists in a
// browser. The harness therefore swaps the DATA SOURCE and mounts the real
// component, so what is under test is the component's own mapping from a state to
// the chip it draws. That mapping is the thing R5 changes: a hub-confirmed "no
// expiry" must read as "No expiry", not as "Subscription status unavailable".
//
// The real type is imported by relative path, NOT through the `@/` alias, because
// the harness config aliases exactly that specifier to this file — importing it
// here would alias onto itself.
import type { SubscriptionState } from '../src/hooks/use-subscription'

// The alias replaces the whole module, so every name another file imports has to
// exist here. `SUBSCRIPTION_POLL_MS` is the gate's poll interval; it is a constant,
// so it is re-exported from the real module rather than restated.
export { SUBSCRIPTION_POLL_MS } from '../src/hooks/use-subscription'

declare global {
  interface Window {
    __SUBSCRIPTION_FIXTURE__?: Partial<SubscriptionState>
  }
}

const EMPTY: SubscriptionState = {
  subscription: { state: 'unknown' },
  activated: true,
  storedCodeDamaged: false,
  tier: 'strike',
  refresh: async () => undefined,
  checkNow: async () => true,
}

export const useSubscription = (): SubscriptionState => ({
  ...EMPTY,
  ...(typeof window === 'undefined' ? {} : window.__SUBSCRIPTION_FIXTURE__),
})
