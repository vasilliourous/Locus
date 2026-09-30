import {
  locusCheckSubscription,
  locusStatus,
  type SubscriptionStatus,
} from '@/services/locus'
import { useQuery } from '@/services/query-client'

import { useVisibility } from './use-visibility'

/**
 * The subscription's state, shared app-wide from one query.
 *
 * One cache entry means the sidebar indicator, the connection warning and the
 * Account screen can never disagree about the same device — a second query would
 * be a second opinion, and the whole point is that "expires in 3 days" is one
 * fact, not three renderings of it.
 *
 * Kept off the connect path deliberately: `locus_connect` refuses a lapsed code
 * in Rust, from the stored date, with no IPC and no dependency on this hook
 * having ever run. This is the *display* half; the enforcement half is
 * `subscription_allows_connect`.
 */
export const subscriptionQueryKey = ['locusSubscription'] as const

/**
 * How often to re-read the subscription.
 *
 * Five minutes, matching the heartbeat's floor: the hub is the only thing that
 * can change the date (a renewal or a lapse), and it does so on a heartbeat, so
 * polling faster than the heartbeat would only ever re-read a value that cannot
 * have changed. Slow enough to be free, fast enough that a renewal shows up
 * while the student is still looking at the screen they renewed from.
 *
 * Exported because the activation gate in `main.tsx` polls the same fact. Two
 * independent intervals would let the gate and the sidebar indicator disagree
 * about whether the same device is entitled, which is the one thing the shared
 * query exists to prevent.
 */
export const SUBSCRIPTION_POLL_MS = 5 * 60 * 1000

export interface SubscriptionState {
  /**
   * The state as the backend decided it.
   *
   * `unknown` when there is nothing to say — a fresh install, or a hub that has
   * never reported a date. Callers MUST render nothing for it rather than
   * guessing: "we do not know" and "expired" are different statements about
   * someone's account.
   */
  subscription: SubscriptionStatus
  /** Whether the device has an entitlement at all — gates the whole indicator. */
  activated: boolean
  /** The tier, for context in the indicator. `null` before activation. */
  tier: string | null
  /** Force a re-read, e.g. right after activation or a renewal. */
  refresh: () => Promise<unknown>
  /**
   * Ask the hub to confirm the subscription now, then re-read.
   *
   * Distinct from {@link refresh}, which only re-reads what the client already
   * holds. Only the hub can change an expiry or a suspension, and the client
   * learns about it on a heartbeat whose interval is a floor of five minutes — so
   * a plain re-read cannot make a just-renewed subscription appear, and a student
   * checking "now" would be shown a value that could not have changed yet.
   *
   * Resolves `true` if a beat was started, `false` if nothing is beating (no
   * activation). The UI says which, rather than implying a check happened.
   */
  checkNow: () => Promise<boolean>
}

/** Fail to "unknown" until the first read, so nothing renders as expired. */
const UNKNOWN: SubscriptionStatus = { state: 'unknown' }

export const useSubscription = (): SubscriptionState => {
  const pageVisible = useVisibility()

  const { data, mutate } = useQuery({
    queryKey: subscriptionQueryKey,
    queryFn: locusStatus,
    // A backgrounded window has no one to show it to; re-reading on focus keeps
    // a student who left the app open from seeing a stale date when they return.
    //
    // Note what this poll CAN and CANNOT see. `locus_status` reads the client's
    // own stored state, which a heartbeat writes — so this reflects a change
    // shortly after the beat that caused it, and cannot itself make one happen.
    // Only the hub can move an expiry, and only a beat asks it; that is what
    // `checkNow` is for. Polling faster than the heartbeat would therefore only
    // re-read a value that provably has not changed.
    refetchInterval: pageVisible ? SUBSCRIPTION_POLL_MS : false,
    refetchOnWindowFocus: true,
    refetchOnReconnect: true,
    retry: 1,
  })

  return {
    subscription: data?.subscription ?? UNKNOWN,
    // `undefined` (never read) is treated as not-activated, so the indicator
    // renders nothing rather than flashing an empty badge before the first read.
    activated: data?.activated ?? false,
    tier: data?.tier ?? null,
    refresh: () => mutate(),
    checkNow: async () => {
      // Awaits the beat, not merely its request.
      //
      // `locus_check_subscription` used to return as soon as the loop was woken,
      // so this cleared the caller's pending state before the request had left
      // the machine — and the "Check status now" control flicked to "Checking…"
      // and back in single-digit milliseconds, unrelated to the hub round trip
      // the student was waiting for. The command now resolves once the beat has
      // completed and its outcome has been applied, so awaiting it IS the
      // progress bar.
      const started = await locusCheckSubscription()
      // Re-read regardless: the beat that just completed will have written an
      // expiry, recorded a refusal, or applied a refreshed config on its own
      // path, and the screen must show that rather than the pre-check state until
      // the next poll.
      await mutate()
      return started
    },
  }
}
