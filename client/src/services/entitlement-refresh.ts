/**
 * A tiny "the entitlement changed — re-read it now" signal.
 *
 * # Why this exists
 *
 * `main.tsx` decides whether a student sees the activation gate or the app, and
 * it re-reads that fact on a five-minute poll. That is the right cadence for a
 * fact the HUB changes (the only other writer): a suspension arrives on a
 * heartbeat, so polling faster would only ever re-read a value that cannot have
 * changed.
 *
 * It is the wrong cadence for a fact THIS DEVICE just changed. After a student
 * signs out, the gate would keep rendering the app for up to five minutes —
 * showing a subscription for a device that no longer holds a key, with the
 * Connect button live. The tab would look signed in until a poll caught up, which
 * is the "close and reopen the app" workaround this project keeps paying for.
 *
 * So the actor that changed the state says so, and the gate re-reads at once.
 *
 * # Why a module-level set rather than React context
 *
 * The only subscriber is the gate in `main.tsx`, which renders ABOVE every
 * provider (that is the point of it). Passing a callback down through the tree
 * would mean every page carrying a prop for a signal none of them emit, and the
 * one emitter (the Account page's sign-out card) is several layers down. A module
 * with three functions is smaller than the plumbing, and it cannot be rendered
 * into the wrong place.
 *
 * Subscribers are a `Set`, so the same component subscribing twice (React strict
 * mode) cannot fire the re-read twice.
 */
type Subscriber = () => void

const subscribers = new Set<Subscriber>()

/**
 * Subscribe to "re-read the entitlement now". Returns the unsubscribe function.
 *
 * Always call the returned function on unmount: a leaked subscriber keeps a
 * closure over a dead component's state setter alive for the life of the window.
 */
export const subscribeEntitlementRefresh = (subscriber: Subscriber) => {
  subscribers.add(subscriber)
  return () => {
    subscribers.delete(subscriber)
  }
}

/**
 * Tell every listener that the local entitlement changed.
 *
 * Called after a sign-out completes. Safe with no subscribers (nothing is mounted
 * yet, or the app is on the activation screen) and safe to call twice — it is a
 * notification, not a counter.
 */
export const requestEntitlementRefresh = () => {
  for (const subscriber of subscribers) {
    // One throwing subscriber must not prevent the others from running: the gate
    // is the only one today, and a future listener that fails should not silence
    // it.
    try {
      subscriber()
    } catch (error) {
      console.error('[entitlement-refresh] a subscriber threw:', error)
    }
  }
}
