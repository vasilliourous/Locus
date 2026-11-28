<script setup lang="ts">
// Codes — the table.
//
// THE SHAPE, AND WHY IT CHANGED AGAIN
//
// Two earlier versions fought over this page. The first had a permanent mint form
// on top of an 11-column table with seven same-weight buttons per row. The second
// compressed it: six columns, one visible action, everything else behind a ⋯ menu.
// Both were organised around "how much can I fit", and the second one's cost was
// never counted — Stop access, the second-most-used operator verb, moved from one
// click to two, and every action gained a menu round-trip. That is why the page
// got easier to READ and harder to USE.
//
// This version is organised around what the operator does all day, and puts the
// state in the URL:
//
//   * The query lives in `router.ts`. A filtered list is a link; F5 restores it;
//     Back works. Room to Run, Room to Explore, Phase 1 of the redesign plan.
//   * The selected code is a path segment (/codes/RQ-ABCD-…), not hidden panel
//     state. The inspector beside the table reads it straight from the route.
//   * Three row verbs, chosen by FREQUENCY rather than by count: Renew (daily),
//     Stop/Reactivate (daily), Open (always). Delete and expiry moved into the
//     inspector, which is where their consequences can be shown.
//   * j/k/⏎ keyboard selection, so clearing a queue is a rhythm rather than a
//     mouse journey.
//   * Name, plan and middleman are first-class filters AND editable in the
//     inspector — the label is how you find a student, so it cannot be a
//     read-only column you have to open a panel to search.
//   * Column visibility is the operator's, stored locally. The previous version
//     removed last-seen and term from the table on the operator's behalf.

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { navigate, patchQuery, route } from '../router'
import { call } from '../api'
import StateBlock from '../components/StateBlock.vue'
import ConfirmSheet from '../components/ConfirmSheet.vue'
import { useCodes, act } from './codes/data'
import { meta } from './codes/meta'
import type { CodeRow } from './codes/model'
import {
  isUrgent,
  livenessOf,
  livenessText,
  conflictOf,
  conflictText,
  overrideNote,
  remainingText,
  shortDate,
  stateClass,
  stateLabel,
} from './codes/model'
import Inspector from './codes/Inspector.vue'
import CreateDrawer from './codes/CreateDrawer.vue'
import RenewDrawer from './codes/RenewDrawer.vue'
import StopDrawer from './codes/StopDrawer.vue'
import ReleaseDrawer from './codes/ReleaseDrawer.vue'

const { rows, facets, loading, failed, denied, total, load } = useCodes()

// A holder for messages that must persist: a partial bulk delete reports what it
// left behind, and a toast that vanished after 5 seconds would leave the operator
// believing the cleanup finished.
const loaded = ref({ err: '' })

// ── The query, read from the URL ──
// No local copies. `route` is the state; changing a control calls `setQuery`,
// which writes the URL, which re-renders this computed. One direction only.
const query = computed(() => ({
  query: route.q.trim(),
  tier: route.tier,
  status: route.status,
  middleman: route.middleman,
  expiring_within_days: route.due > 0 ? route.due : 0,
}))

/** The renewal window, or 0 when the list is unfiltered by date. */
const windowDays = computed(() => route.due)

const activeFilterCount = computed(
  () => [route.q, route.tier, route.status, route.middleman].filter((v) => String(v).trim()).length + (route.due > 0 ? 1 : 0),
)

/** Write one filter change to the URL. `replace` so filtering does not fill Back. */
function setQuery(patch: Partial<{ q: string; tier: string; status: string; middleman: string; due: number }>) {
  patchQuery('codes', patch)
}

// Search is typed, so it is debounced; selects apply at once. The debounce is
// for the KEYSTROKE, not for the URL — the input is bound to a local ref and only
// the committed value goes into the route, so typing does not rewrite history.
const searchText = ref(route.q)
watch(
  () => route.q,
  (v) => {
    if (v !== searchText.value) searchText.value = v
  },
)
let debounce: number | undefined
function onSearchInput() {
  window.clearTimeout(debounce)
  debounce = window.setTimeout(() => setQuery({ q: searchText.value }), 220)
}

function clearFilters() {
  searchText.value = ''
  navigate('codes', {})
}

// ── Load ──
// The URL is the trigger: any change to the derived query reloads. This is what
// makes F5 and Back work without special cases.
async function reload() {
  await load(query.value)
}

watch(query, () => void reload(), { deep: true })

onMounted(async () => {
  // The tier/middleman pickers need names to offer. Fetch once per session; the
  // inspector's save path refreshes them only because a save can introduce a new
  // middleman name.
  void meta.ensure()
  await reload()
})

// ── Sorting ──
// Server order is meaningful (most-recent activity, or most-urgent when a window
// is on), so the default keeps it. Clicking a header re-sorts locally over the
// rows already loaded.
type SortKey = 'code' | 'days' | 'middleman' | 'label' | 'seen'
const sortKey = ref<SortKey>('days')
const sortDir = ref<1 | -1>(1)

function toggleSort(k: SortKey) {
  if (sortKey.value === k) sortDir.value = sortDir.value === 1 ? -1 : 1
  else {
    sortKey.value = k
    // Text sorts ascending first; a "days left" sort is most useful ascending
    // (soonest first), which is also what the server's worklist order means.
    sortDir.value = 1
  }
}

const shown = computed(() => {
  const out = rows.value.slice()
  const dir = sortDir.value
  const cmpDays = (a: CodeRow, b: CodeRow) => {
    const da = a.days_remaining === null || a.days_remaining === undefined ? 1e9 : a.days_remaining
    const db = b.days_remaining === null || b.days_remaining === undefined ? 1e9 : b.days_remaining
    return (da - db) * dir
  }
  switch (sortKey.value) {
    case 'code':
      out.sort((a, b) => a.code.localeCompare(b.code) * dir)
      break
    case 'label':
      out.sort((a, b) => (a.label || '~').localeCompare(b.label || '~') * dir)
      break
    case 'middleman':
      out.sort((a, b) => (a.middleman || '~').localeCompare(b.middleman || '~') * dir)
      break
    case 'seen':
      out.sort((a, b) => (a.last_seen_at || '').localeCompare(b.last_seen_at || '') * dir)
      break
    default:
      out.sort(cmpDays)
  }
  return out
})

// ── Column visibility ──
// The operator's choice, remembered across sessions. The previous redesign
// decided for them (dropping last-seen and term); this hands the decision back —
// which is the whole point of a table.
type Col = 'name' | 'plan' | 'state' | 'seen' | 'who' | 'days'
const COLS: { id: Col; label: string; hint: string }[] = [
  { id: 'name', label: 'Name', hint: 'the label: your name for the student' },
  { id: 'plan', label: 'Plan', hint: 'the tier' },
  { id: 'state', label: 'State', hint: 'entitlement: may it be used' },
  { id: 'seen', label: 'Last seen', hint: 'liveness: did the device reach us' },
  { id: 'who', label: 'Middleman', hint: 'who sold it' },
  { id: 'days', label: 'Days left', hint: 'until expiry' },
]

const COL_KEY = 'chrysalis_cols'
function readCols(): Set<Col> {
  try {
    const raw = localStorage.getItem(COL_KEY)
    if (!raw) return new Set(COLS.map((c) => c.id))
    const arr = JSON.parse(raw) as Col[]
    return new Set(arr.filter((c) => COLS.some((x) => x.id === c)))
  } catch {
    return new Set(COLS.map((c) => c.id))
  }
}
const visible = ref<Set<Col>>(readCols())
function toggleCol(c: Col) {
  const next = new Set(visible.value)
  if (next.has(c)) next.delete(c)
  else next.add(c)
  visible.value = next
  try {
    localStorage.setItem(COL_KEY, JSON.stringify(Array.from(next)))
  } catch {
    /* private mode: the choice lasts for this page only, which is fine */
  }
}
const colsOpen = ref(false)

// ── Selection: the inspector ──
// The selection IS the URL. `openCode` pushes a path; the inspector reads it.
const selected = computed(() => route.code)
const inspector = ref<InstanceType<typeof Inspector> | null>(null)

/**
 * The navigation this sheet was opened for, held until the operator answers.
 *
 * A pending callback rather than a boolean, because the answer resumes a
 * specific intent ("open THAT row", "close the rail", "go to Renewals") — and
 * remembering only "there was a pending navigation" would lose which one.
 */
const pendingLeave = ref<null | (() => void)>(null)
const askLeave = ref(false)

/**
 * Run `next`, but first refuse to discard a half-typed name silently.
 *
 * The row click is the easiest mistake to make while editing, because the whole
 * row is a link. Without this, typing a student's name and then clicking the next
 * code to look something up lost the name with no signal at all.
 */
function guarded(next: () => void) {
  if (inspector.value?.hasUnsavedWork?.()) {
    pendingLeave.value = next
    askLeave.value = true
    return
  }
  next()
}

function confirmLeave() {
  askLeave.value = false
  const next = pendingLeave.value
  pendingLeave.value = null
  next?.()
}

function cancelLeave() {
  askLeave.value = false
  pendingLeave.value = null
}

function openCode(code: string) {
  guarded(() => navigate('codes', { code }))
}
function closeInspector() {
  guarded(() => {
    // Back to the list, keeping the filters that were on screen. `push`, so the
    // close is a Back target — the operator closed a panel, not a page.
    navigate('codes', { q: route.q, tier: route.tier, status: route.status, middleman: route.middleman, due: route.due })
  })
}

// ── Keyboard: j/k/⏎/esc ──
// Only when the operator is not typing and no drawer is open. Checked on the
// event target rather than via a focus trap, because the table is not a widget —
// it is the page.
function isTyping(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement | null
  if (!t) return false
  return (
    t.tagName === 'INPUT' ||
    t.tagName === 'TEXTAREA' ||
    t.tagName === 'SELECT' ||
    t.isContentEditable
  )
}

function onKey(e: KeyboardEvent) {
  if (isTyping(e) || e.metaKey || e.ctrlKey || e.altKey) return
  if (renewRow.value || stopRow.value || createOpen.value) return
  // The inspector's inline editors also live in inputs; isTyping covers them.

  if (e.key === 'j' || e.key === 'ArrowDown') {
    e.preventDefault()
    step(1)
  } else if (e.key === 'k' || e.key === 'ArrowUp') {
    e.preventDefault()
    step(-1)
  } else if (e.key === 'Enter' && selected.value) {
    e.preventDefault()
    const r = shown.value.find((x) => x.code === selected.value)
    if (r) renewRow.value = r
  } else if (e.key === 'Escape' && selected.value) {
    e.preventDefault()
    closeInspector()
  } else if (e.key === '/') {
    e.preventDefault()
    document.querySelector<HTMLInputElement>('input[type="search"]')?.focus()
  }
}
function step(delta: number) {
  if (!shown.value.length) return
  const i = shown.value.findIndex((r) => r.code === selected.value)
  const next = i < 0 ? (delta > 0 ? 0 : shown.value.length - 1) : Math.min(shown.value.length - 1, Math.max(0, i + delta))
  openCode(shown.value[next].code)
}
onMounted(() => document.addEventListener('keydown', onKey))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))

// ── Selection for bulk delete ──
// Separate from the inspector selection ON PURPOSE: inspecting a row is a read,
// ticking a box is an intent to destroy. Conflating them is how a click meant to
// look at a row would arm a delete.
const checked = ref<Set<string>>(new Set())
const checkedRows = computed(() => rows.value.filter((r) => checked.value.has(r.code)))
const checkedDeletable = computed(() => checkedRows.value.filter((r) => !r.redeemed))

function toggleCheck(code: string) {
  const next = new Set(checked.value)
  if (next.has(code)) next.delete(code)
  else next.add(code)
  checked.value = next
}
function toggleAll() {
  const deletable = shown.value.filter((r) => !r.redeemed)
  if (deletable.every((r) => checked.value.has(r.code))) checked.value = new Set()
  else checked.value = new Set(deletable.map((r) => r.code))
}
const allChecked = computed(() => {
  const deletable = shown.value.filter((r) => !r.redeemed)
  return deletable.length > 0 && deletable.every((r) => checked.value.has(r.code))
})
// A stale tick must never delete a row the operator can no longer see.
watch(query, () => (checked.value = new Set()), { deep: true })

async function deleteChecked() {
  const codes = checkedDeletable.value.map((r) => r.code)
  if (!codes.length) return
  const res = await call<{
    deleted_count: number
    skipped_count: number
    skipped?: { code: string; why: string }[]
  }>('codes.deleteBatch', { codes })
  if (!res.ok) {
    loaded.value.err = res.message || res.transportError || 'Bulk delete failed'
  } else if (res.data?.skipped_count) {
    // A partial batch reports what it left behind. A bare "deleted" toast over a
    // silent 3-of-50 would be worse than an error, because the operator would
    // believe the cleanup is done.
    const why = (res.data.skipped || []).map((s) => `${s.code}: ${s.why}`).join('; ')
    loaded.value.err = `Deleted ${res.data.deleted_count}, skipped ${res.data.skipped_count} (${why})`
  }
  await refreshAll()
  checked.value = new Set()
}

// ── Drawers ──
const renewRow = ref<CodeRow | null>(null)
const stopRow = ref<CodeRow | null>(null)
// Release has its own drawer: clearing the claim is a larger consequence than
// cutting one device off, and its confirmation shows what the handler will do.
const releaseRow = ref<CodeRow | null>(null)
const createOpen = ref(false)

// ── Row verbs ──
// Three, by frequency. Renew and Stop are daily; Open is how you read a row.
async function toggleSuspend(row: CodeRow) {
  if (!row.suspended) {
    stopRow.value = row
    return
  }
  const ok = await act('codes.unsuspend', { code: row.code }, 'Reactivated')
  if (ok) await refreshAll()
}

/**
 * Reload the list (and the inspector's copy of its code) after any mutation.
 *
 * The inspector is not reloaded from here: it re-fetches its own code when the
 * URL changes, and it reloads itself after its own actions. Passing a `refresh`
 * signal would duplicate that. The list reload is what actually needs doing.
 */
async function refreshAll() {
  await load(query.value)
  // The inspector is a SEPARATE reader: it fetches its own code rather than
  // looking it up in `rows`, so a reload of the list does not refresh it. Every
  // mutation that lands here (renew, stop, create) must also tell the open panel
  // to re-read, or it keeps showing the pre-action date — measured in a browser:
  // a renewal moved the expiry to February while the panel still said December.
  await inspector.value?.reload?.()
}

// ── Facts for the header ──
const tierOptions = computed(() => {
  const fromRows = Object.keys(facets.value.tier || {})
  const set = new Set([...fromRows, ...meta.tiers.value])
  return Array.from(set).filter(Boolean).sort()
})

const middlemanOptions = computed(() => {
  const set = new Set(meta.middlemen.value)
  for (const r of rows.value) if (r.middleman) set.add(r.middleman)
  return Array.from(set).sort()
})

function livenessTitle(row: CodeRow): string {
  switch (livenessOf(row)) {
    case 'live':
      return 'This device reached the hub recently. It does not mean the student is using it.'
    case 'stale':
      return 'No contact for longer than the heartbeat lease. The device may be off, offline, or quits — the hub cannot tell which.'
    case 'absent':
      return 'No contact for over 7 days. The record is kept; the device is presumed gone.'
    case 'unknown':
      return 'This code has never checked in, so there is no liveness to report. This is not the same as signed out.'
  }
}
</script>

<template>
  <!-- The page splits into the list and the rail. Both scroll independently. -->
  <div class="codes-layout">
    <div class="codes-main">
      <div class="page-head">
        <div class="titles">
          <h1>Codes</h1>
          <p class="page-sub">Find a code, renew it, or fix an access problem.</p>
        </div>
        <button class="primary" @click="createOpen = true">Create codes</button>
      </div>

      <!-- ── Filters ── -->
      <div class="sticky-bar">
        <div class="filters">
          <div class="search-input grow">
            <span class="ico">⌕</span>
            <input
              v-model="searchText"
              type="search"
              placeholder="Code, name, notes or middleman"
              @input="onSearchInput"
              @keyup.esc="searchText = ''; setQuery({ q: '' })"
            />
          </div>
          <select :value="route.tier" aria-label="Plan" @change="setQuery({ tier: ($event.target as HTMLSelectElement).value })">
            <option value="">All plans</option>
            <option v-for="t in tierOptions" :key="t" :value="t">
              {{ t }}<template v-if="facets.tier[t]"> · {{ facets.tier[t] }}</template>
            </option>
          </select>
          <select :value="route.status" aria-label="State" @change="setQuery({ status: ($event.target as HTMLSelectElement).value })">
            <option value="">Any state</option>
            <option value="available">Unused · {{ facets.status.available || 0 }}</option>
            <option value="redeemed">In term · {{ facets.status.redeemed || 0 }}</option>
            <option value="expired">Lapsed · {{ facets.status.expired || 0 }}</option>
            <option value="suspended">Stopped · {{ facets.status.suspended || 0 }}</option>
          </select>
          <select :value="route.middleman" aria-label="Middleman" @change="setQuery({ middleman: ($event.target as HTMLSelectElement).value })">
            <option value="">Any middleman</option>
            <option v-for="m in middlemanOptions" :key="m" :value="m">{{ m }}</option>
          </select>
          <button v-if="activeFilterCount" class="tiny" @click="clearFilters">Clear</button>
        </div>

        <div class="filters" style="margin-top: 10px">
          <div class="segmented" role="tablist">
            <button :class="{ on: windowDays === 0 }" @click="setQuery({ due: 0 })">All codes</button>
            <button :class="{ on: windowDays === 7 }" @click="setQuery({ due: 7 })">Due ≤7d</button>
            <button :class="{ on: windowDays === 30 }" @click="setQuery({ due: 30 })">Due ≤30d</button>
          </div>
          <template v-if="windowDays > 0">
            <span class="muted" style="font-size: 12px">
              Stopped codes are excluded — renewing one would ask you to undo your own decision.
            </span>
          </template>
          <div class="spacer" style="flex: 1" />

          <!-- Column choice. A dropdown of checkboxes rather than a settings page:
               the decision belongs where the table is. -->
          <details class="cols-menu">
            <summary class="tiny">Columns</summary>
            <div class="cols-body">
              <label v-for="c in COLS" :key="c.id" class="cols-row">
                <input type="checkbox" :checked="visible.has(c.id)" @change="toggleCol(c.id)" />
                <span>
                  {{ c.label }}
                  <small>{{ c.hint }}</small>
                </span>
              </label>
            </div>
          </details>

          <span v-if="!loading" class="muted" style="font-size: 12px; font-variant-numeric: tabular-nums">
            {{ total }} {{ total === 1 ? 'code' : 'codes' }}
          </span>
        </div>
      </div>

      <div v-if="loaded.err" class="msg err">
        {{ loaded.err }}
        <button class="tiny" style="margin-left: 8px" @click="loaded.err = ''">Dismiss</button>
      </div>

      <!-- ── Bulk bar ── -->
      <div v-if="checked.size" class="bulkbar">
        <strong>{{ checked.size }} selected</strong>
        <span class="muted" style="font-size: 12.5px">Used codes are never selectable — the batch refuses them.</span>
        <div class="spacer" />
        <button class="tiny" @click="checked = new Set()">Clear</button>
        <button class="tiny danger" :disabled="!checkedDeletable.length" @click="deleteChecked">
          Delete {{ checkedDeletable.length }}
        </button>
      </div>

      <!-- ── Table ── -->
      <div class="table-wrap">
        <StateBlock v-if="loading && !rows.length" state="loading" :rows="8" />
        <StateBlock v-else-if="denied" state="denied" @retry="reload" />
        <StateBlock v-else-if="failed" state="error" :message="failed" @retry="reload" />
        <StateBlock
          v-else-if="!shown.length"
          state="empty"
          :title="windowDays > 0 ? 'Nothing due in this window' : 'No codes match'"
          :message="
            windowDays > 0
              ? 'No code is ending in this window. Widen it to 30 days, or check All codes.'
              : activeFilterCount
                ? 'No code matches these filters.'
                : 'The hub holds no codes yet.'
          "
          :hint="
            windowDays > 0
              ? 'Codes already lapsed are not here — they are on Today under “Lapsed”.'
              : 'Try a shorter search, or clear the filters.'
          "
        >
          <template #actions>
            <button v-if="windowDays > 0" @click="setQuery({ due: 0 })">Show all codes</button>
            <button v-else-if="activeFilterCount" @click="clearFilters">Clear filters</button>
            <button v-else class="primary" @click="createOpen = true">Create codes</button>
          </template>
        </StateBlock>

        <div v-else class="table-scroll">
          <table class="data codes-table">
            <thead>
              <tr>
                <th class="c-check">
                  <input
                    type="checkbox"
                    :checked="allChecked"
                    :disabled="!shown.filter((r) => !r.redeemed).length"
                    aria-label="Select all unused codes"
                    @change="toggleAll"
                  />
                </th>
                <!-- Widths are declared here and nowhere else. With
                     `table-layout: fixed` these are the share each column takes;
                     the free-text ones (name, middleman) get the remainder and
                     truncate, and the fixed-meaning ones (state, days) are wide
                     enough for their longest real value, not their longest
                     possible one. -->
                <!-- Widths: only the columns whose content has a KNOWN maximum are
                     pinned (a code is 19 chars, a badge is short, the verbs are two
                     buttons). Name and Middleman are declared `auto` so they share
                     whatever is left and truncate — that is what keeps the declared
                     widths under the container instead of forcing the table to
                     950px inside a 754px track. -->
                <!-- Widths are PERCENTAGES, not pixels: with `table-layout: fixed`
                     the browser honours specified widths literally, so px widths
                     that summed to 856 kept the table at 856 even inside a 754px
                     box (measured — that was the sideways scrollbar). Percentages
                     are distributed against the container.
                     The split gives the two FREE-TEXT columns (Code's label and
                     Middleman) enough to read a school name, and keeps the
                     fixed-meaning ones (Plan, State, Days) just wide enough for
                     their longest real value. The checkbox and verbs are sized by
                     their content. -->
                <th class="sortable sticky-code c-code" :class="{ sorted: sortKey === 'code' }" @click="toggleSort('code')">
                  Code <span class="arrow">{{ sortKey === 'code' ? (sortDir === 1 ? '↑' : '↓') : '↑' }}</span>
                </th>
                <th v-if="visible.has('name')" class="sortable c-name" :class="{ sorted: sortKey === 'label' }" @click="toggleSort('label')">
                  Name <span class="arrow">{{ sortKey === 'label' ? (sortDir === 1 ? '↑' : '↓') : '↑' }}</span>
                </th>
                <th v-if="visible.has('plan')" class="c-plan">Plan</th>
                <th v-if="visible.has('state')" class="c-state">State</th>
                <th v-if="visible.has('seen')" class="sortable c-seen" :class="{ sorted: sortKey === 'seen' }" title="The last time this code reached the hub — a different fact from State, which is about the entitlement" @click="toggleSort('seen')">
                  Last seen <span class="arrow">{{ sortKey === 'seen' ? (sortDir === 1 ? '↑' : '↓') : '↑' }}</span>
                </th>
                <th v-if="visible.has('who')" class="sortable c-who" :class="{ sorted: sortKey === 'middleman' }" @click="toggleSort('middleman')">
                  Middleman <span class="arrow">{{ sortKey === 'middleman' ? (sortDir === 1 ? '↑' : '↓') : '↑' }}</span>
                </th>
                <th v-if="visible.has('days')" class="sortable num c-days" :class="{ sorted: sortKey === 'days' }" @click="toggleSort('days')">
                  Days left <span class="arrow">{{ sortKey === 'days' ? (sortDir === 1 ? '↑' : '↓') : '↑' }}</span>
                </th>
                <th class="c-verbs"></th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in shown"
                :key="row.id"
                :class="{ selected: checked.has(row.code), inspecting: selected === row.code }"
                @click="openCode(row.code)"
              >
                <td @click.stop>
                  <input
                    type="checkbox"
                    :checked="checked.has(row.code)"
                    :disabled="row.redeemed"
                    :aria-label="`Select ${row.code}`"
                    @change="toggleCheck(row.code)"
                  />
                </td>
                <td class="sticky-code">
                  <span class="mono code-link" :class="{ active: selected === row.code }">{{ row.code }}</span>
                </td>
                <!-- The name is the operator's own label for a student, so it is
                     the one free-text cell that must be READABLE. It truncates with
                     an ellipsis when it genuinely does not fit, and carries the full
                     text as a title so a truncated name is still one hover away —
                     the alternative (clipping silently) made two students with the
                     same school prefix look identical. -->
                <td v-if="visible.has('name')" class="truncate" :title="row.label || 'No name recorded'">
                  <span v-if="row.label">{{ row.label }}</span>
                  <span v-else class="muted">— no name</span>
                </td>
                <td v-if="visible.has('plan')">
                  <span class="chip">{{ row.tier || '—' }}</span>
                </td>
                <td v-if="visible.has('state')">
                  <span class="badge" :class="stateClass(row)">{{ stateLabel(row) }}</span>
                  <!-- The OVERRIDE, beside the status and never as it: the badge
                       above is the hub's derived status, this is the operator's
                       decision. Two facts, two places — see stateLabel. -->
                  <span v-if="overrideNote(row)" class="override-note">{{ overrideNote(row) }}</span>
                  <!--
                    The CONFLICT marker: the observed state disagreeing with the
                    main status, or an eviction outstanding. A pure render — it
                    never writes anything — so an observation cannot silently
                    become a status. The title says WHY, because "reads free but a
                    device checked in" is a fact an operator acts on, not a verdict
                    against the student.
                  -->
                  <span
                    v-if="conflictOf(row) !== 'none'"
                    class="conflict"
                    :class="conflictOf(row)"
                    :title="conflictText(row)"
                    :aria-label="conflictText(row)"
                    role="img"
                  >!</span>
                </td>
                <!--
                  The LIVENESS cell, deliberately its own column rather than folded
                  into the State badge. The two answer different questions with
                  different owners, and merging them is the defect this column fixes:
                  "signed out" is derived from `released_at` (an entitlement fact) and
                  says nothing about whether the device is beating.
                -->
                <td v-if="visible.has('seen')" class="muted nowrap" :title="livenessTitle(row)">
                  {{ livenessText(row) }}
                </td>
                <td v-if="visible.has('who')" class="muted truncate">
                  {{ row.middleman || '—' }}
                </td>
                <td v-if="visible.has('days')" class="num nowrap">
                  <template v-if="row.expires_at">
                    <strong v-if="isUrgent(row)" class="urgent">{{ remainingText(row) }}</strong>
                    <span v-else>{{ remainingText(row) }}</span>
                    <div class="muted" style="font-size: 11px">{{ shortDate(row.expires_at) }}</div>
                  </template>
                  <span v-else class="muted">never</span>
                </td>
                <td @click.stop>
                  <div class="row-verbs">
                    <button class="row-action primary" :title="`Add paid time to ${row.code}`" @click="renewRow = row">Renew</button>
                    <button class="row-action" :title="row.suspended ? 'Restore access' : 'Cut access off'" @click="toggleSuspend(row)">
                      {{ row.suspended ? 'Reactivate' : 'Stop' }}
                    </button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <p class="foot-note">
        A code is single-use and is not tied to a machine: a student on a new laptop re-enters their own code and it just works.
        <strong>Release</strong> (in the inspector) is only for handing a code to a <em>different</em> student.
      </p>
    </div>

    <!-- ── The rail ── -->
    <Inspector
      ref="inspector"
      :code="selected"
      :rows="rows"
      :tiers="tierOptions"
      @close="closeInspector"
      @changed="refreshAll"
      @renew="(r) => (renewRow = r)"
      @stop="(r) => (stopRow = r)"
      @release="(r) => (releaseRow = r)"
    />
  </div>

  <!-- ── Drawers ── -->
  <RenewDrawer :row="renewRow" :middlemen="middlemanOptions" @close="renewRow = null" @done="renewRow = null; refreshAll()" />
  <StopDrawer :row="stopRow" @close="stopRow = null" @done="stopRow = null; refreshAll()" />
  <ReleaseDrawer :row="releaseRow" @close="releaseRow = null" @done="releaseRow = null; refreshAll()" />
  <CreateDrawer :open="createOpen" :tiers="meta.tiers.value" :middlemen="middlemanOptions" @close="createOpen = false" @created="refreshAll(); meta.refresh()" />

  <ConfirmSheet
    :open="askLeave"
    title="Discard unsaved changes?"
    body="This code has edits that have not been saved. Leaving now throws them away."
    confirm-label="Discard and leave"
    cancel-label="Keep editing"
    @confirm="confirmLeave"
    @cancel="cancelLeave"
  />
</template>
