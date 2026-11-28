<script setup lang="ts">
// Codes — the workbench.
//
// THE SHAPE, AND WHY. This page is where the console spends its life, and the
// previous version made two jobs fight each other: minting (a permanent
// two-row form at the top) and finding/renewing (an 11-column table with seven
// buttons per row and an Apply button). The form pushed the data below the fold;
// the action column wrapped; the primary job — renew — was a browser prompt.
//
// Now:
//   * one sticky bar of filters that apply on change (no Apply button);
//   * a compact table: code, state, who, days left, and ONE primary action;
//   * everything else in the row menu;
//   * create / renew / stop as drawers, so the list is never covered by a form;
//   * detail as a slide-over, so the list stays visible while reading one code.
//
// DENSITY IS THE POINT. Six columns fit on a laptop, the header sticks, and the
// numbers align — three things the old table could not do at 11 columns.

import { computed, onMounted, ref, watch } from 'vue'
import { nav, type CodeFilter } from '../nav'
import { call } from '../api'
import StateBlock from '../components/StateBlock.vue'
import RowMenu from '../components/RowMenu.vue'
import type { MenuItem } from '../components/menu'
import { useCodes, act } from './codes/data'
import type { CodeRow } from './codes/model'
import {
  isUrgent,
  livenessOf,
  livenessText,
  remainingText,
  shortDate,
  stateClass,
  stateLabel,
} from './codes/model'
import CodeDetail from './codes/CodeDetail.vue'
import CreateDrawer from './codes/CreateDrawer.vue'
import RenewDrawer from './codes/RenewDrawer.vue'
import StopDrawer from './codes/StopDrawer.vue'

const { rows, facets, loading, failed, denied, total, load, history } = useCodes()

// A holder for messages that must persist: a partial bulk delete reports what it
// left behind, and a toast that vanished after 5 seconds would leave the operator
// believing the cleanup finished. Declared here, above every handler that writes
// it — it used to be declared 80 lines below its first use, which is a
// temporal-dead-zone crash the first time a bulk delete actually ran.
const loaded = ref({ err: '' })

// ── Filter state ──
// `view` is the segmented control (all / worklist); the rest are the query.
const view = ref<'all' | 'worklist'>('all')
const query = ref('')
const tier = ref('')
const status = ref('')
const middleman = ref('')
const worklistDays = ref(7)
const sortKey = ref<'code' | 'days' | 'middleman'>('days')

const currentQuery = computed(() => ({
  query: query.value.trim(),
  tier: tier.value,
  status: status.value,
  middleman: middleman.value.trim(),
  // The worklist IS `expiring_within_days`; 0 means every code.
  expiring_within_days: view.value === 'worklist' ? worklistDays.value : 0,
}))

let debounce: number | undefined
function reload() {
  void load(currentQuery.value)
}
/** Typing reloads after a pause; selects and toggles reload at once. */
function reloadSoon() {
  window.clearTimeout(debounce)
  debounce = window.setTimeout(reload, 220)
}

watch(currentQuery, reloadSoon, { deep: true })

// ── Deep links from the Overview / palette ──
// `nav.codeRequestSeq` bumps on every request, so asking for the same filter
// twice still re-applies it (a watcher on an unchanged object would fire zero
// times and the click would silently do nothing).
// NOT `immediate`: the mount handler below is the one place that loads the page
// for the first time, and it applies the pending request as part of that single
// load. With `immediate` the page fetched twice on every arrival — once for the
// deep link, once for `onMounted` — and the second response could land first and
// discard the filter the operator asked for.
watch(
  () => nav.codeRequestSeq,
  () => {
    if (nav.page !== 'codes') return
    applyFilter(nav.codeFilter)
  },
)

function applyFilter(f: CodeFilter) {
  // A deep link is an explicit instruction, so the fetch is immediate — not the
  // 220ms debounce a keystroke waits for. The window.clearTimeout below stops the
  // `watch(currentQuery)` from ALSO firing a fetch for the same change: the two
  // would race, and the loser would overwrite the winner's rows with identical
  // data or, worse, with a half-applied filter.
  window.clearTimeout(debounce)
  view.value = f.view === 'worklist' ? 'worklist' : 'all'
  query.value = f.query || ''
  tier.value = f.tier || ''
  status.value = f.status || ''
  middleman.value = f.middleman || ''
  if (f.expiring_within_days) worklistDays.value = f.expiring_within_days
  selection.value = new Set()
  reload()
}

// ── Meta: the tier list and the middleman names ──
const tiers = ref<string[]>([])
const middlemen = ref<string[]>([])

async function loadMeta() {
  // Tiers come from the hub, never from a literal here. A literal can drift
  // from the seed and offer a tier that mints an unresolvable code — the exact
  // failure check-consistency.sh §27(f) guards.
  const t = await call<{ tiers: { tier: string }[] }>('tiers.list')
  if (t.ok && t.data) tiers.value = t.data.tiers.map((x) => x.tier)
  const m = await call<{ middlemen: { name: string }[] }>('middlemen.list')
  if (m.ok && m.data) middlemen.value = m.data.middlemen.map((x) => x.name)
}

// ── Sorting ──
// Server order is meaningful (most-recent activity, or most-urgent when the
// worklist is on), so the default keeps it. Clicking a header re-sorts locally
// over the rows already loaded — this is one page of a few hundred, not a
// paged grid, so there is nothing to fetch.
const shown = computed(() => {
  const out = rows.value.slice()
  if (sortKey.value === 'code') out.sort((a, b) => a.code.localeCompare(b.code))
  else if (sortKey.value === 'middleman') out.sort((a, b) => (a.middleman || '~').localeCompare(b.middleman || '~'))
  else
    out.sort((a, b) => {
      const da = a.days_remaining === null || a.days_remaining === undefined ? 1e9 : a.days_remaining
      const db = b.days_remaining === null || b.days_remaining === undefined ? 1e9 : b.days_remaining
      return da - db
    })
  return out
})

/**
 * The tooltip for the liveness cell.
 *
 * It states what was OBSERVED and what was not. "Checked in" means the device
 * reached the hub, not that the student is in front of it, and an unknown says so
 * explicitly rather than leaving a dash to be read as "gone". The wording is the
 * operator's only defence against re-deriving "signed out" from a missing value,
 * which is the defect this column exists to remove.
 */
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

// ── Selection (bulk delete only) ──
// Cleared on every filter change and after every bulk action, so a stale tick
// can never delete a row the operator can no longer see.
const selection = ref<Set<string>>(new Set())
const selectedRows = computed(() => rows.value.filter((r) => selection.value.has(r.code)))
const selectedDeletable = computed(() => selectedRows.value.filter((r) => !r.redeemed))
function toggle(code: string) {
  const next = new Set(selection.value)
  if (next.has(code)) next.delete(code)
  else next.add(code)
  selection.value = next
}
function toggleAll() {
  // Only rows that CAN be deleted are selectable: the batch hook refuses a
  // redeemed code, so offering it a tick would invite a guaranteed skip.
  const deletable = shown.value.filter((r) => !r.redeemed)
  if (deletable.every((r) => selection.value.has(r.code))) selection.value = new Set()
  else selection.value = new Set(deletable.map((r) => r.code))
}
const allSelected = computed(() => {
  const deletable = shown.value.filter((r) => !r.redeemed)
  return deletable.length > 0 && deletable.every((r) => selection.value.has(r.code))
})
watch(currentQuery, () => (selection.value = new Set()), { deep: true })

async function deleteSelected() {
  const codes = selectedDeletable.value.map((r) => r.code)
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
  selection.value = new Set()
}

// ── Panels ──
const detailRow = ref<CodeRow | null>(null)
const detailHistory = ref<Awaited<ReturnType<typeof history>>>([])
const historyLoading = ref(false)

const renewRow = ref<CodeRow | null>(null)
const stopRow = ref<CodeRow | null>(null)
const createOpen = ref(false)

async function openDetail(row: CodeRow) {
  detailRow.value = row
  detailHistory.value = []
  historyLoading.value = true
  detailHistory.value = await history(row.code)
  historyLoading.value = false
}

/**
 * Reload everything, keeping the open detail panel in sync.
 *
 * The detail panel holds a COPY of the row it was opened with, so after any
 * action it would otherwise keep showing the pre-action state — "renewed" in the
 * list while the panel still says the old expiry. Re-reading it is what makes
 * the panel trustworthy after an action taken from inside it.
 *
 * `loadMeta` is awaited FIRST because the tier and middleman lists feed the
 * drawers' datalists; refreshing them after the rows would let a newly minted
 * middleman name be missing from the very list that offered it.
 */
async function refreshAll() {
  await loadMeta()
  await load(currentQuery.value)
  const open = detailRow.value
  if (!open) return
  const fresh = rows.value.find((r) => r.code === open.code)
  if (!fresh) {
    // The row is gone (deleted): close rather than leave a stale panel behind.
    detailRow.value = null
    return
  }
  detailRow.value = fresh
  historyLoading.value = true
  detailHistory.value = await history(fresh.code)
  historyLoading.value = false
}

function rowMenu(row: CodeRow): MenuItem[] {
  return [
    { label: 'Open detail', run: () => openDetail(row) },
    { label: 'Renew…', run: () => (renewRow.value = row) },
    { sep: true, label: row.suspended ? 'Reactivate' : 'Stop access…', run: () => {
      if (row.suspended) {
        void act('codes.unsuspend', { code: row.code }, 'Reactivated').then(refreshAll)
      } else {
        stopRow.value = row
      }
    } },
    { label: 'Set expiry (manual)…', run: () => manualExpiry(row) },
    { sep: true, label: 'Delete…', danger: true, disabled: row.redeemed, hint: row.redeemed ? 'release first' : undefined, run: () => deleteOne(row) },
  ]
}

function manualExpiry(row: CodeRow) {
  const current = row.expires_at ? row.expires_at.slice(0, 10) : ''
  const input = window.prompt(
    `Set an expiry for ${row.code}\n\nYYYY-MM-DD, or blank for none.\n\n` +
      `This is the manual override. To add paid time, use Renew instead — it ` +
      `previews the new date and keeps what is left of the term.`,
    current,
  )
  if (input === null) return
  const v = input.trim()
  if (v && !/^\d{4}-\d{2}-\d{2}/.test(v)) {
    loaded.value.err = 'Enter a date like 2027-03-01, or leave it blank.'
    return
  }
  void act('codes.expire', { code: row.code, expires_at: v ? `${v}T00:00:00.000Z` : '' }, v ? `Expires ${v}` : 'Expiry cleared').then(refreshAll)
}

async function deleteOne(row: CodeRow) {
  if (row.redeemed) return
  const sure = window.confirm(
    `Permanently delete ${row.code}?${row.label ? ` (${row.label})` : ''}\n\n` +
      `The code is removed from the hub and cannot be re-activated. An audit ` +
      `entry is kept. This cannot be undone.`,
  )
  if (!sure) return
  await act('codes.delete', { code: row.code }, 'Deleted')
  if (detailRow.value?.code === row.code) detailRow.value = null
  await refreshAll()
}

const activeFilterCount = computed(
  () => [query.value, tier.value, status.value, middleman.value].filter((v) => String(v).trim()).length,
)

function clearFilters() {
  query.value = ''
  tier.value = ''
  status.value = ''
  middleman.value = ''
  view.value = 'all'
}

const tierOptions = computed(() => {
  const fromRows = Object.keys(facets.value.tier || {})
  return (fromRows.length ? fromRows : tiers.value).sort()
})

onMounted(async () => {
  // One load, filter already applied. `codeRequestSeq` is 1 or more whenever the
  // operator arrived here from a card or the palette; 0 means a plain click on
  // the nav item, which wants the unfiltered list.
  if (nav.codeRequestSeq > 0) applyFilter(nav.codeFilter)
  await refreshAll()
})
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Codes</h1>
      <p class="page-sub">
        Find a code, renew it, or fix an access problem.
      </p>
    </div>
    <button class="primary" @click="createOpen = true">Create codes</button>
  </div>

  <!-- ── Filters ── -->
  <div class="sticky-bar">
    <div class="filters">
      <div class="search-input grow">
        <span class="ico">⌕</span>
        <input
          v-model="query"
          type="search"
          placeholder="Code, student's name, notes or middleman"
          @input="reloadSoon"
        />
      </div>
      <select v-model="tier" aria-label="Tier">
        <option value="">All tiers</option>
        <option v-for="t in tierOptions" :key="t" :value="t">
          {{ t }}<template v-if="facets.tier[t]"> · {{ facets.tier[t] }}</template>
        </option>
      </select>
      <select v-model="status" aria-label="State">
        <option value="">Any state</option>
        <option value="available">Unused · {{ facets.status.available || 0 }}</option>
        <option value="redeemed">In term · {{ facets.status.redeemed || 0 }}</option>
        <option value="expired">Lapsed · {{ facets.status.expired || 0 }}</option>
        <option value="suspended">Stopped · {{ facets.status.suspended || 0 }}</option>
      </select>
      <select v-model="middleman" aria-label="Middleman">
        <option value="">Any middleman</option>
        <option v-for="m in middlemen" :key="m" :value="m">{{ m }}</option>
      </select>
      <button v-if="activeFilterCount" class="tiny" @click="clearFilters">Clear</button>
    </div>

    <div class="filters" style="margin-top: 10px">
      <div class="segmented" role="tablist">
        <button :class="{ on: view === 'all' }" @click="view = 'all'">All codes</button>
        <button :class="{ on: view === 'worklist' }" @click="view = 'worklist'">
          Renewals due
        </button>
      </div>
      <template v-if="view === 'worklist'">
        <div class="segmented">
          <button :class="{ on: worklistDays === 7 }" @click="worklistDays = 7">7 days</button>
          <button :class="{ on: worklistDays === 30 }" @click="worklistDays = 30">30 days</button>
        </div>
        <span class="muted" style="font-size: 12px">
          Most urgent first. Stopped codes are excluded — renewing one would ask you
          to undo your own decision.
        </span>
      </template>
      <div class="spacer" style="flex: 1" />
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
  <div v-if="selection.size" class="bulkbar">
    <strong>{{ selection.size }} selected</strong>
    <span class="muted" style="font-size: 12.5px">
      Used codes are never selectable — the batch refuses them.
    </span>
    <div class="spacer" />
    <button class="tiny" @click="selection = new Set()">Clear</button>
    <button class="tiny danger" :disabled="!selectedDeletable.length" @click="deleteSelected">
      Delete {{ selectedDeletable.length }}
    </button>
  </div>

  <!-- ── Table ── -->
  <div class="table-wrap">
    <StateBlock
      v-if="loading && !rows.length"
      state="loading"
      :rows="8"
    />
    <StateBlock
      v-else-if="denied"
      state="denied"
      @retry="() => load(currentQuery)"
    />
    <StateBlock
      v-else-if="failed"
      state="error"
      :message="failed"
      @retry="() => load(currentQuery)"
    />
    <StateBlock
      v-else-if="!shown.length"
      state="empty"
      :title="view === 'worklist' ? 'Nothing due in this window' : 'No codes match'"
      :message="
        view === 'worklist'
          ? 'No code is ending in this window. Widen it to 30 days, or check All codes.'
          : 'No code matches these filters.'
      "
      :hint="
        view === 'worklist'
          ? 'Codes already lapsed are not here — they are on the Overview under “Lapsed”.'
          : 'Try a shorter search, or clear the filters.'
      "
    >
      <template #actions>
        <button v-if="view === 'worklist'" @click="view = 'all'">Show all codes</button>
        <button v-else @click="clearFilters">Clear filters</button>
      </template>
    </StateBlock>

    <div v-else class="table-scroll">
      <table class="data">
        <thead>
          <tr>
            <th style="width: 30px">
              <input
                type="checkbox"
                :checked="allSelected"
                :disabled="!shown.filter((r) => !r.redeemed).length"
                aria-label="Select all unused codes"
                @change="toggleAll"
              />
            </th>
            <th class="sortable" :class="{ sorted: sortKey === 'code' }" @click="sortKey = 'code'">
              Code <span class="arrow">↑</span>
            </th>
            <th>State</th>
            <th title="The last time this code reached the hub — a different fact from State, which is about the entitlement">
              Last seen
            </th>
            <th class="sortable" :class="{ sorted: sortKey === 'middleman' }" @click="sortKey = 'middleman'">
              Who <span class="arrow">↑</span>
            </th>
            <th class="sortable num" :class="{ sorted: sortKey === 'days' }" @click="sortKey = 'days'">
              Days left <span class="arrow">↑</span>
            </th>
            <th style="width: 150px"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in shown" :key="row.id" :class="{ selected: selection.has(row.code) }">
            <td>
              <input
                type="checkbox"
                :checked="selection.has(row.code)"
                :disabled="row.redeemed"
                :aria-label="`Select ${row.code}`"
                @change="toggle(row.code)"
              />
            </td>
            <td>
              <button
                class="mono"
                style="border: none; background: none; padding: 0; color: var(--accent); font-size: 12.5px"
                @click="openDetail(row)"
              >
                {{ row.code }}
              </button>
              <div v-if="row.label" class="muted truncate" style="font-size: 11.5px">{{ row.label }}</div>
            </td>
            <td>
              <span class="badge" :class="stateClass(row)">{{ stateLabel(row) }}</span>
            </td>
            <!--
              The LIVENESS cell, deliberately its own column rather than folded
              into the State badge. The two answer different questions with
              different owners, and merging them is the defect this change fixes:
              "signed out" is derived from `released_at` (an entitlement fact) and
              says nothing about whether the device is beating.

              An unknown liveness renders as "—" with a tooltip, never as "away"
              and never as "signed out". See `livenessOf` in ./codes/model.
            -->
            <td class="muted nowrap" :title="livenessTitle(row)">
              {{ livenessText(row) }}
            </td>
            <td class="muted truncate">
              {{ row.middleman || '—' }}
            </td>
            <td class="num nowrap">
              <template v-if="row.expires_at">
                <strong v-if="isUrgent(row)" class="urgent">{{ remainingText(row) }}</strong>
                <span v-else>{{ remainingText(row) }}</span>
                <div class="muted" style="font-size: 11px">{{ shortDate(row.expires_at) }}</div>
              </template>
              <span v-else class="muted">never</span>
            </td>
            <td>
              <div style="display: flex; gap: 4px; align-items: center; justify-content: flex-end">
                <button
                  class="row-action primary"
                  :title="`Add paid time to ${row.code}`"
                  @click="renewRow = row"
                >
                  Renew
                </button>
                <RowMenu :items="rowMenu(row)" :label="`More actions for ${row.code}`" />
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>

  <p class="foot-note">
    A code is single-use and is not tied to a machine: a student on a new laptop
    re-enters their own code and it just works. <strong>Release</strong> is only
    for handing a code to a <em>different</em> student.
  </p>

  <!-- ── Panels ── -->
  <CodeDetail
    :row="detailRow"
    :history="detailHistory"
    :history-loading="historyLoading"
    @close="detailRow = null"
    @changed="refreshAll"
    @renew="renewRow = detailRow"
    @stop="stopRow = detailRow"
  />
  <RenewDrawer
    :row="renewRow"
    :middlemen="middlemen"
    @close="renewRow = null"
    @done="refreshAll"
  />
  <StopDrawer :row="stopRow" @close="stopRow = null" @done="stopRow = null; refreshAll()" />
  <CreateDrawer
    :open="createOpen"
    :tiers="tiers"
    :middlemen="middlemen"
    @close="createOpen = false"
    @created="refreshAll"
  />
</template>
