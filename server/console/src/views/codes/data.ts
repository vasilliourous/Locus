// Codes: the data layer.
//
// One composable owns every request the codes surfaces make, so the views are
// about layout and the request/error/retry handling exists once rather than in a
// dozen handlers. The previous page called `load()` from ten places with no
// shared notion of "is a load in flight", which is how a stale response could
// land after a filter change and silently show the wrong rows.
//
// WHAT CHANGED WITH THE URL REDESIGN
//
// The query is no longer held here. It lives in the URL (`router.ts`), and this
// module is handed a flat query object by whoever owns the route. That matters
// because it used to be held in TWO places — the page's refs and `nav.codeFilter`
// — and the two could disagree, which is how F5 lost a filter and how "click the
// same Overview card twice" needed a request counter to work around.

import { ref } from 'vue'
import { call } from '../../api'
import { toast } from '../../toast'
import { codeKey } from '../../router'
import type { CodeRow, Facets, HistoryEvent } from './model'

export interface CodesQuery {
  query: string
  tier: string
  status: string
  middleman: string
  expiring_within_days: number
}

/**
 * One code's audit trail, newest first.
 *
 * A module function rather than a method on `useCodes()`: the inspector needs it
 * for ONE code and has no list state, and calling `useCodes()` there would
 * instantiate a second, unused store with its own request counter.
 */
export async function fetchHistory(code: string): Promise<HistoryEvent[]> {
  const res = await call<{ events: HistoryEvent[] }>('codes.history', { code })
  if (!res.ok || !res.data) {
    toast.err(res.message || res.transportError || 'Could not load the history')
    return []
  }
  return res.data.events || []
}

export function useCodes() {
  const rows = ref<CodeRow[]>([])
  const facets = ref<Facets>({ status: {}, tier: {} })
  const loading = ref(false)
  const failed = ref('')
  const denied = ref(false)
  /** Total in the CURRENT filter, which is what "200 of 214" needs. */
  const total = ref(0)
  let seq = 0

  async function load(q: CodesQuery) {
    const mine = ++seq
    loading.value = true
    failed.value = ''
    const res = await call<{
      codes: CodeRow[]
      total: number
      facets?: Facets
      expiring_within_days: number
    }>('codes.list', { ...q })
    // A response from an earlier query must not overwrite a newer one. Without
    // this, two quick filter clicks can show the first filter's rows under the
    // second filter's label.
    if (mine !== seq) return
    loading.value = false
    if (res.ok && res.data) {
      rows.value = res.data.codes || []
      total.value = res.data.total ?? rows.value.length
      if (res.data.facets) facets.value = res.data.facets
      denied.value = false
      return
    }
    if (res.message && /sign in again/i.test(res.message)) denied.value = true
    failed.value = res.message || res.transportError || 'The hub did not answer.'
    rows.value = []
    total.value = 0
  }

  return { rows, facets, loading, failed, denied, total, load }
}

/**
 * One code, by code string, straight from the hub.
 *
 * WHY THIS EXISTS. The inspector used to be handed a row object by the list, and
 * re-found it in `rows.value` after every action. That works only while the row
 * is IN the loaded page — so with filters (or a URL pasted from a message) the
 * inspector had nothing to show even though the code exists. This asks
 * `codes.list` for that one code (the query matches the code string), which is
 * the same authority the list uses, so the two cannot disagree.
 *
 * Returns null when the hub does not hold it — a real, displayable answer.
 */
export async function fetchCode(code: string): Promise<CodeRow | null> {
  const res = await call<{ codes: CodeRow[] }>('codes.list', { query: code })
  if (!res.ok || !res.data) return null
  const want = codeKey(code)
  const hit = (res.data.codes || []).find(
    (r) => r.code.toUpperCase().replace(/-/g, '') === want,
  )
  return hit || null
}

/**
 * Run a mutating action, toast the outcome, and report success.
 *
 * Every action already went through this shape (call → toast → reload) but
 * hand-rolled at each site, so the error text varied and one of them forgot to
 * reload. Centralising it means a new action cannot forget the reload.
 */
export async function act(
  action: string,
  payload: Record<string, unknown>,
  okText: string,
): Promise<boolean> {
  const res = await call(action, payload)
  if (res.ok) {
    if (okText) toast.ok(okText)
    return true
  }
  toast.err(res.message || res.transportError || 'That did not work')
  return false
}
