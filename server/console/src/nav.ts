// Navigation state for the console.
//
// WHY THIS EXISTS. The console has no router and should not grow one: five
// destinations, no URL contract to honour (the token lives in sessionStorage, so
// every page is a fresh hash-less load anyway), and a router would be more
// moving parts than the thing it routes. The first version of cross-page
// navigation was a `window` CustomEvent — "chrysalis:open-worklist" — dispatched
// by the Dashboard and caught in App.vue, because the Dashboard is not the
// parent of Codes and threading a callback would have coupled two pages that
// should not know about each other.
//
// That event worked and was the wrong shape: it could only express ONE
// navigation (open the worklist with N days), it needed a matching removeEventListener
// to avoid leaking a handler per mount, and once a second page wanted to open a
// third with a filter, the event would have needed a union type of every
// destination's parameters. A `reactive` object is the same amount of machinery
// and has none of those limits — so the event is gone.
//
// The filter travels WITH the destination rather than as a separate ref the
// target page has to watch, because "open Codes filtered to the renewals due in
// 7 days" is one intent, and splitting it into two pieces of state is what made
// the old version need a watcher plus a mount path.

import { reactive } from 'vue'

export type Page = 'overview' | 'codes' | 'distribution' | 'settings'

/** Which Settings section is showing. Settings is one nav item, three surfaces. */
export type SettingsSection = 'tiers' | 'releases' | 'maintenance'

/**
 * A request to look at codes. Every field is a `codes.list` filter, so the
 * Codes page applies it directly rather than interpreting a bespoke mode.
 *
 * `expiring_within_days` is the renewal worklist (0 = every code); `status` is
 * one of codes.list's four; `query` is free text matching code, label, notes or
 * middleman.
 */
export interface CodeFilter {
  query?: string
  status?: '' | 'available' | 'redeemed' | 'expired' | 'suspended'
  tier?: string
  middleman?: string
  expiring_within_days?: number
  /** Which tab of the Codes page: the workbench or the renewal queue. */
  view?: 'all' | 'worklist'
}

interface NavState {
  page: Page
  section: SettingsSection
  /**
   * Bumped on every request. The Codes page watches this rather than the filter
   * object, so asking for the SAME view twice (e.g. clicking the same Overview
   * card again) still re-applies it — a deep watcher on an unchanged object
   * would fire zero times and the click would silently do nothing.
   */
  codeRequestSeq: number
  codeFilter: CodeFilter
}

export const nav = reactive<NavState>({
  page: 'overview',
  section: 'tiers',
  codeRequestSeq: 0,
  codeFilter: { view: 'all' },
})

/** Go to the Codes page, applying `filter`. Every call re-applies, by design. */
export function openCodes(filter: CodeFilter = {}) {
  nav.codeFilter = { view: 'all', ...filter }
  nav.codeRequestSeq++
  nav.page = 'codes'
}

/** Go to a Settings section. Re-selecting the current section is a no-op. */
export function openSettings(section: SettingsSection) {
  nav.section = section
  nav.page = 'settings'
}

export function go(page: Page) {
  // Re-clicking the active destination should not reset its state: the operator
  // may have typed a search and mis-clicked. Only a genuine destination CHANGE
  // clears the Codes filter, so returning to Codes from elsewhere starts fresh
  // rather than resurrecting a filter from five minutes ago.
  if (page === 'codes' && nav.page !== 'codes') {
    nav.codeFilter = { view: 'all' }
    nav.codeRequestSeq++
  }
  nav.page = page
}
