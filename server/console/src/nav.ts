// The console's navigation model.
//
// WHAT THIS FILE IS NOW, AND WHAT IT WAS. It used to be a command bus: a
// `reactive` object holding the current page, the settings section, a filter
// object and a request counter, plus three functions (`go`, `openCodes`,
// `openSettings`) that pages called to move each other around. The comments in it
// explained, at length, why a CustomEvent had been replaced by that object, and
// then why the object needed a `codeRequestSeq` so that asking for the same filter
// twice still applied it.
//
// Every one of those mechanisms existed to compensate for the fact that the URL
// did not hold the state. Once it does (`router.ts`), they are all dead weight —
// and worse than dead: `nav.codeFilter` was a second copy of the query that could
// disagree with the page's own refs, which is how F5 lost a filter. So the bus is
// gone, and this file is what a nav model should be: the list of destinations and
// how they group.
//
// ONE ITEM PER JOB. The old sidebar kept four top-level items by hiding Tiers,
// Releases and Maintenance behind a Settings item — with a sub-menu that appeared
// only while Settings was open, PLUS a segmented control inside Settings
// re-expressing the same three. Two mechanisms, one job, and neither said where
// you were. Tiers/Releases/Maintenance are three different jobs on three different
// objects; they are now three destinations under one heading. Six items in two
// groups is cheaper to scan than four items plus a conditional sub-tree.

import type { RouteName } from './router'
import { route, navigate } from './router'

export interface NavItem {
  name: RouteName
  label: string
  /** Shown under the label in the palette and as a title, never in the sidebar. */
  hint: string
}

export interface NavGroup {
  label: string
  items: NavItem[]
}

/**
 * The destinations, in reading order.
 *
 * `today` is first because it is the entry point of a session; `fleet` is last
 * because it holds the changes you make rarely and deliberately. The order is
 * the only thing grouping here — there is no nesting, so there is no hidden
 * sub-menu to discover.
 */
export const NAV: NavGroup[] = [
  {
    label: 'Work',
    items: [
      { name: 'today', label: 'Today', hint: 'counts, the renewal queue and what needs attention' },
      { name: 'codes', label: 'Codes', hint: 'find, renew and fix a code' },
      { name: 'renewals', label: 'Renewals due', hint: 'every renewal code by urgency' },
      { name: 'distribution', label: 'Distribution', hint: 'per-middleman stock and collections' },
    ],
  },
  {
    label: 'Fleet',
    items: [
      { name: 'fleet/tiers', label: 'Tiers', hint: 'connection settings every client receives' },
      { name: 'fleet/releases', label: 'Releases', hint: 'which build the fleet is offered' },
      { name: 'fleet/maintenance', label: 'Maintenance', hint: 'import codes, trigger links, export' },
    ],
  },
]

/** Flat list, for the palette. */
export const NAV_FLAT: NavItem[] = NAV.flatMap((g) => g.items)

/**
 * Which sidebar item is active for a location.
 *
 * `/codes/<code>` is the Codes destination plus a selection, so it highlights
 * Codes. Kept as a function rather than comparing `item.name === route.name`
 * everywhere, because that comparison is where "the selected code un-highlights
 * its own page" bugs come from.
 */
export function activeName(): RouteName {
  return route.name
}

/** Navigate, as a plain function so templates can call it without a helper. */
export function go(name: RouteName): void {
  // Re-selecting the destination you are on must not reset its filters: the
  // operator may have typed a search and mis-clicked. `navigate` with no opts
  // would CLEAR the query, so an active item is a no-op here.
  if (name === route.name) return
  navigate(name)
}
