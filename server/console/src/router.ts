// URL routing for the console — the address bar as the source of truth.
//
// WHY THIS EXISTS NOW, WHEN nav.ts ARGUED AGAINST IT
//
// The previous version had no router on purpose. Its reasoning was: "no URL
// contract to honour (the token lives in sessionStorage, so every page is a
// fresh hash-less load anyway), and a router would be more moving parts than the
// thing it routes."
//
// The second half of that premise is wrong, and the first half does not support
// the conclusion:
//
//   * Caddy ALREADY serves /index.html for an unknown path under /admin/
//     (`try_files {path} /index.html` in templates/Caddyfile). The server was
//     provisioned for real URLs and they were never claimed, so "every page is a
//     fresh hash-less load anyway" described the CLIENT's choices, not a
//     constraint. A router changes the client; nothing on the server moves.
//   * The cost of not having one was never priced in. Without URLs: a support
//     thread could not contain a link to a code; F5 lost every filter (the Codes
//     page only reapplied `nav.codeFilter` when `codeRequestSeq > 0`, which is
//     process lifetime, so a reload silently dropped the operator's search); the
//     back button was inert; and nothing was shareable or bookmarkable.
//
// DELIBERATELY NOT A DEPENDENCY. This is ~200 lines against the History API and
// the app has seven routes. Pulling a router package in to save those lines
// would put its abstractions (guards, route records, nested views) in front of a
// static internal tool on a 512MB box. The API other files use is exactly three
// things: `route`, `navigate`, `codeHref`.
//
// ONE-WAY DATA. `route` is a readonly view of `location`. A page never writes to
// it — it calls `navigate()`, and the popstate/push handlers are the only
// writers. That keeps "what is on screen" and "what the URL says" from drifting,
// which was the failure this replaces: `nav.codeFilter` existed only in memory,
// so it could (and did) disagree with what the operator thought they saw.

import { reactive, readonly } from 'vue'

/** Every destination in the console. Tiers/Releases/Maintenance are `fleet/*`. */
export type RouteName =
  | 'today'
  | 'codes'
  | 'renewals'
  | 'distribution'
  | 'fleet/tiers'
  | 'fleet/releases'
  | 'fleet/maintenance'

export interface Route {
  /** The destination. `codes` covers both the table and a selected code. */
  name: RouteName
  /**
   * The code whose inspector is open, when one is. Upper-cased and
   * hyphen-stripped, the same normalisation the hub's `canonical()` applies, so
   * a pasted lowercase or hyphenated code still opens.
   */
  code: string
  /** Free-text search. */
  q: string
  /** `codes.list` status, or '' for any. */
  status: string
  /** Tier id, or '' for any. */
  tier: string
  /** Middleman name, or '' for any. */
  middleman: string
  /** Renewal window in days. 0 = no window (every code). */
  due: number
  /** The window on /renewals. */
  days: number
}

/** Path <-> name. The paths are what an operator types or bookmarks. */
const PATHS: Record<RouteName, string> = {
  today: '/',
  codes: '/codes',
  renewals: '/renewals',
  distribution: '/distribution',
  'fleet/tiers': '/fleet/tiers',
  'fleet/releases': '/fleet/releases',
  'fleet/maintenance': '/fleet/maintenance',
}

const NAME_BY_PATH: Record<string, RouteName> = Object.fromEntries(
  (Object.entries(PATHS) as [RouteName, string][]).map(([name, path]) => [path, name]),
)

/** Every route's default query. The one place "empty" is defined. */
const DEFAULTS: Omit<Route, 'name'> = {
  code: '',
  q: '',
  status: '',
  tier: '',
  middleman: '',
  due: 0,
  days: 7,
}

/**
 * Normalise a code the way the hub does (`canonical()` in admin_console.pb.js).
 *
 * Codes are stored and printed in the hyphenated form `RQ-ABCD-EFGH-JKMN-T`, and
 * a URL that carried the raw 15 characters would be unreadable and unquotable —
 * the operator has to read these and paste them into messages, so the address
 * uses the same shape the card and the table do.
 *
 * Accepts any pasted variant back: with hyphens, without, lower case. The
 * character set is deliberately NOT validated — a typo should open the inspector
 * and say "not found", not fail to route.
 */
export function normaliseCode(raw: string): string {
  const s = decodeURIComponent(raw).replace(/-/g, '').toUpperCase()
  // Only reshape when it is exactly a code's length; anything else is passed
  // through so the inspector can report it as not found rather than mangle it.
  if (s.length !== 15) return s
  return `${s.substring(0, 2)}-${s.substring(2, 6)}-${s.substring(6, 10)}-${s.substring(10, 14)}-${s.substring(14, 15)}`
}

/**
 * The comparable form of a code: no hyphens, upper case.
 *
 * The inspector compares codes it fetched against codes it was asked for, and
 * both sides must agree that `RQABCDEFGHJKMNT` and `RQ-ABCD-EFGH-JKMN-T` are the
 * same code. Comparing the DISPLAY form would fail on a pasted variant.
 */
export function codeKey(raw: string): string {
  return decodeURIComponent(raw).replace(/-/g, '').toUpperCase().trim()
}

/**
 * The mount point (/admin/ live, / under `vite dev`).
 *
 * Read from the vite base rather than hard-coded, because a dev server that
 * routes differently from production is how a routing bug reaches the operator.
 */
function basePath(): string {
  const base = import.meta.env.BASE_URL || '/'
  return base.replace(/\/+$/, '')
}

/** The address of one code's inspector. The thing you paste into a message. */
export function codeHref(code: string): string {
  return `${basePath()}/codes/${normaliseCode(code)}`
}

function toNum(v: string | null, fallback: number): number {
  const n = Number(v)
  return Number.isFinite(n) && v !== null && v.trim() !== '' ? n : fallback
}

/** Build a Route from a path+query. The only parser; used on load and on popstate. */
function parse(href: string): Route {
  const url = new URL(href, window.location.origin)
  let path = url.pathname
  const base = basePath()
  if (base && path.startsWith(base + '/')) path = path.slice(base.length)
  else if (base && path === base) path = '/'
  if (!path.startsWith('/')) path = '/' + path
  path = path.replace(/\/+$/, '') || '/'

  const params = url.searchParams

  // /codes/<CODE> — the inspector address. The route name stays `codes`, because
  // the table is still behind it; `code` is what makes the inspector open. The
  // other filters are dropped: a single code has no list to filter.
  const codeMatch = path.match(/^\/codes\/([^/]+)\/?$/)
  if (codeMatch) return { ...DEFAULTS, name: 'codes', code: normaliseCode(codeMatch[1]) }

  const name: RouteName = NAME_BY_PATH[path] || 'today'
  return {
    name,
    code: '',
    q: params.get('q') || '',
    status: params.get('status') || '',
    tier: params.get('tier') || '',
    middleman: params.get('m') || '',
    due: toNum(params.get('due'), 0),
    days: toNum(params.get('days'), 7),
  }
}

const state = reactive<Route>(parse(window.location.href))

/**
 * The current location. `readonly()` is not decoration: a page that mutated
 * `route` would change what renders without changing the URL, which is exactly
 * the drift this module removes.
 */
export const route: Route = readonly(state) as Route

function apply(r: Route) {
  Object.assign(state, r)
}

/**
 * Serialise a route to a URL. THE canonical builder — `navigate` and
 * `patchQuery` both go through it, so the two cannot disagree about how a
 * filter is spelled.
 *
 * Omitted rather than written when empty/default: a URL that says `due=0` or
 * `status=` invites a reader to think a zero-day window or an empty filter was
 * applied. Absent means "nothing set", and says it honestly.
 */
function toUrl(name: RouteName, r: Omit<Route, 'name'>): string {
  // A code's address is its path segment, not a query param.
  if (name === 'codes' && r.code) {
    return `${codeHref(r.code)}`
  }

  const qs = new URLSearchParams()
  if (r.q) qs.set('q', r.q)
  if (r.status) qs.set('status', r.status)
  if (r.tier) qs.set('tier', r.tier)
  if (r.middleman) qs.set('m', r.middleman)
  if (r.due > 0) qs.set('due', String(r.due))
  if (name === 'renewals' && r.days !== DEFAULTS.days) qs.set('days', String(r.days))

  const search = qs.toString()
  return `${basePath()}${PATHS[name]}${search ? `?${search}` : ''}`
}

function commit(url: string, mode: 'push' | 'replace') {
  const here = window.location.pathname + window.location.search
  if (url === here) {
    // Same URL: still re-apply, because a caller may be asking to re-read a
    // location the state object has not caught up with (e.g. after sign-in).
    apply(parse(url))
    return
  }
  if (mode === 'replace') window.history.replaceState({}, '', url)
  else window.history.pushState({}, '', url)
  apply(parse(url))
}

/**
 * Navigate to a destination.
 *
 * `opts` is a PARTIAL of the query and is merged onto that destination's
 * DEFAULTS, never onto the current route — so going from a filtered `/codes?
 * status=redeemed` to `/renewals` does not carry the status filter across, and
 * going to `/codes` from `/renewals` starts from a clean list unless the caller
 * asks for a filter. That is the bug the old `nav.ts` had to add a whole
 * "only a genuine destination CHANGE clears the filter" rule to work around.
 */
export function navigate(
  name: RouteName,
  opts: Partial<Omit<Route, 'name'>> = {},
  mode: 'push' | 'replace' = 'push',
): void {
  commit(toUrl(name, { ...DEFAULTS, ...opts }), mode)
}

/**
 * Change the query of the destination currently showing, keeping the rest.
 *
 * The one composition a page needs: "the operator changed the tier filter, keep
 * the search". Defaults to `replace`, so typing in a search box does not fill
 * the Back button with one entry per keystroke.
 */
export function patchQuery(
  name: RouteName,
  opts: Partial<Omit<Route, 'name'>>,
  mode: 'push' | 'replace' = 'replace',
): void {
  const current: Omit<Route, 'name'> = {
    code: state.code,
    q: state.q,
    status: state.status,
    tier: state.tier,
    middleman: state.middleman,
    due: state.due,
    days: state.days,
  }
  commit(toUrl(name, { ...current, ...opts }), mode)
}

/** Start listening for Back/Forward. Called once, from App.vue. */
export function startRouter(): void {
  window.addEventListener('popstate', () => apply(parse(window.location.href)))
}

export { PATHS, DEFAULTS }
