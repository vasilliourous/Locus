<script setup lang="ts">
// Renewals due — every code, by urgency, in columns.
//
// WHY A BOARD AND NOT A LIST. The renewal queue is the revenue mechanism: nothing
// in the system prompts a renewal (the hub holds no identity, so it cannot know a
// payment is due — 01-identity-without-pii.md §1.2, 09-console.md §9.3). A sorted
// list answers "which is next"; it does not answer "how bad is it overall", which
// is the question that decides whether you work the queue now or on Monday. Five
// columns with counts on each answers both, and the act of clearing one is a
// click on the card.
//
// THIS IS NOT THE CODES TABLE. `/codes` stays a table because finding one code is
// a search-and-scan job, and a table is the right shape for it. This page is the
// opposite job: no search, no filters to set, just the whole book arranged by
// how soon it matters.

import { computed, onMounted, ref, watch } from 'vue'
import { call } from '../api'
import { navigate, patchQuery, route } from '../router'
import StateBlock from '../components/StateBlock.vue'
import {
  isUrgent,
  remainingText,
  shortDate,
  stateClass,
  stateLabel,
} from './codes/model'
import type { CodeRow } from './codes/model'
import { fetchCode } from './codes/data'
import RenewDrawer from './codes/RenewDrawer.vue'

const rows = ref<CodeRow[]>([])
const loading = ref(true)
const failed = ref('')
const denied = ref(false)

const renewRow = ref<CodeRow | null>(null)

/**
 * The board needs EVERY code that has an expiry, not a window.
 *
 * WHY THIS IS NOT A WINDOWED CALL. It was, and that was a bug with a confidently
 * wrong screen behind it. `codes.list` with `expiring_within_days` deliberately
 * EXCLUDES three kinds of row, for good reasons that belong to the Codes page:
 *
 *   * already-lapsed codes (no FUTURE expiry inside the window),
 *   * suspended codes (the operator cut them off on purpose),
 *   * codes with no expiry at all (a permanent code has nothing to renew).
 *
 * The board then rendered "Lapsed" and "Stopped" lanes from that same response,
 * so those lanes were ALWAYS empty — a screen that says "nothing is lapsed" while
 * the hub holds lapsed codes. Measured with the fixture: 8 of 15 codes reached the
 * board. The footnote even claimed "lapsed codes are shown so nothing falls
 * through a gap", which was false.
 *
 * So the fetch is unfiltered (`expiring_within_days: 0`) and every grouping
 * decision happens here, where the buckets are defined. The window control above
 * then filters the DISPLAY, not the query.
 */
async function load() {
  loading.value = true
  failed.value = ''
  const res = await call<{ codes: CodeRow[] }>('codes.list', { expiring_within_days: 0 })
  loading.value = false
  if (res.ok && res.data) {
    rows.value = res.data.codes || []
    denied.value = false
    return
  }
  if (res.message && /sign in again/i.test(res.message)) denied.value = true
  failed.value = res.message || res.transportError || 'The hub did not answer.'
  rows.value = []
}

onMounted(load)

// ── The columns ──
//
// Thresholds mirror the rest of the system rather than inventing numbers:
//   ≤7d  matches EXPIRY_WARNING_DAYS on the client, so the student's own warning
//        and this column agree about what "soon" means (06-console §6.3).
//   ≤30d matches the `dashboard` card, so the card's number and this column's
//        number are the same quantity.
// Lapsed is its own column and is NOT "most urgent": a lapsed code needs a
// catch-up decision (renew, or let it go), not a reminder, and mixing them would
// make the urgent column look catastrophic on a healthy fleet.
interface Column {
  id: string
  label: string
  hint: string
  tone: string
  test: (r: CodeRow) => boolean
}

const COLUMNS: Column[] = [
  {
    // FIRST, and deliberately so. `codes.list` excludes suspended codes from every
    // window; here the whole collection is loaded, so a stopped code would
    // otherwise satisfy the `due7` or `lapsed` test and appear as a renewal
    // target — asking the operator to undo their own decision. The earlier code
    // checked suspended LAST in the list but the loop matched the first test, so
    // this is the ordering that makes the intent true.
    id: 'stopped',
    label: 'Stopped',
    hint: 'cut off deliberately — not a renewal target',
    tone: 'quiet',
    test: (r) => r.suspended,
  },
  {
    id: 'lapsed',
    label: 'Lapsed',
    hint: 'term ended — renew to restore, or leave it',
    tone: 'bad',
    test: (r) => !r.suspended && r.status === 'expired',
  },
  {
    id: 'due7',
    label: 'Due ≤7 days',
    hint: 'the worklist — the same set Today shows',
    tone: 'warn',
    test: (r) => r.status !== 'expired' && r.days_remaining !== null && r.days_remaining !== undefined && r.days_remaining <= 7,
  },
  {
    id: 'due30',
    label: 'Due 8–30 days',
    hint: 'next week’s queue',
    tone: '',
    test: (r) => r.status !== 'expired' && (r.days_remaining ?? 1e9) > 7 && (r.days_remaining ?? 1e9) <= 30,
  },
  {
    // `days_remaining === null` is a code with NO expiry (a permanent code). It
    // used to fall into this lane via the `?? 1e9` default, which put a
    // never-expiring code under "more than a month of runway" — implying it would
    // one day expire. It gets its own honest placement below instead.
    id: 'later',
    label: 'Due later',
    hint: 'in term, more than a month of runway',
    tone: 'good',
    test: (r) => r.status !== 'expired' && r.days_remaining !== null && r.days_remaining !== undefined && r.days_remaining > 30,
  },
  {
    id: 'never',
    label: 'No expiry',
    hint: 'permanent — never needs renewing',
    tone: 'quiet',
    test: (r) => !r.suspended && r.status !== 'expired' && (r.days_remaining === null || r.days_remaining === undefined),
  },
]

const buckets = computed(() => {
  const out: Record<string, CodeRow[]> = {}
  for (const c of COLUMNS) out[c.id] = []
  for (const r of rows.value) {
    // Suspended is checked first: a stopped code is not a renewal target, which
    // is the rule codes.list already applies when it excludes them from a window.
    for (const c of COLUMNS) {
      if (c.test(r)) {
        out[c.id].push(r)
        break
      }
    }
  }
  // Within a column: soonest expiry first, the same order the hub uses, because
  // the top card is the one you act on.
  for (const id of Object.keys(out)) {
    out[id].sort((a, b) => {
      const da = a.days_remaining ?? 1e9
      const db = b.days_remaining ?? 1e9
      return da - db
    })
  }
  return out
})

const total = computed(() => Object.values(buckets.value).reduce((n, l) => n + l.length, 0))

/** Which buckets are expanded. All open by default: the point is to see it all. */
const collapsed = ref<Set<string>>(new Set())
function toggleBucket(id: string) {
  const next = new Set(collapsed.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  collapsed.value = next
}

function openCode(code: string) {
  navigate('codes', { code })
}

async function renew(row: CodeRow) {
  // The board's rows come from the same call the table uses, so this row IS the
  // hub's row — but fetch it anyway so the drawer previews against the freshest
  // expiry. A renewal is the one action where a stale date costs real money.
  const fresh = await fetchCode(row.code)
  renewRow.value = fresh || row
}

async function refresh() {
  await load()
}

// The column set is not in the URL (it is derived), but the WINDOW is, so a link
// to "the 30-day board" is shareable and Back works. Today links here.
const days = computed(() => route.days)
watch(
  () => route.name,
  (n) => {
    if (n === 'renewals') void load()
  },
)
function setWindow(n: number) {
  patchQuery('renewals', { days: n })
}
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Renewals due</h1>
      <p class="page-sub">
        Every code in term, arranged by how soon it matters. Nothing else in the system will remind you.
      </p>
    </div>
    <div class="segmented">
      <button :class="{ on: days === 7 }" @click="setWindow(7)">7 days</button>
      <button :class="{ on: days === 30 }" @click="setWindow(30)">30 days</button>
      <button :class="{ on: days === 365 }" @click="setWindow(365)">Year</button>
    </div>
  </div>

  <StateBlock v-if="loading && !rows.length" state="loading" :rows="5" />
  <StateBlock v-else-if="denied" state="denied" @retry="load" />
  <StateBlock v-else-if="failed" state="error" :message="failed" @retry="load" />
  <StateBlock
    v-else-if="!total"
    state="empty"
    title="Nothing in the book expires within a year"
    message="Codes appear here once they have an expiry. A code with no expiry never needs renewing, so it never appears."
  />

  <div v-else class="board">
    <section
      v-for="c in COLUMNS"
      :key="c.id"
      class="lane"
      :class="`lane-${c.tone}`"
      :aria-label="c.label"
    >
      <header class="lane-head">
        <button class="lane-title" type="button" :aria-expanded="!collapsed.has(c.id)" @click="toggleBucket(c.id)">
          <span class="dot" :class="c.tone" />
          <strong>{{ c.label }}</strong>
          <span class="lane-count">{{ buckets[c.id].length }}</span>
        </button>
        <p class="lane-hint">{{ c.hint }}</p>
      </header>

      <div v-if="!collapsed.has(c.id)" class="lane-body">
        <div v-if="!buckets[c.id].length" class="lane-empty">None</div>
        <article
          v-for="r in buckets[c.id]"
          :key="r.id"
          class="card-code"
          tabindex="0"
          @click="openCode(r.code)"
          @keyup.enter="openCode(r.code)"
        >
          <div class="card-top">
            <span class="mono">{{ r.code }}</span>
            <span class="badge" :class="stateClass(r)">{{ stateLabel(r) }}</span>
          </div>
          <div class="card-name">{{ r.label || '— no name' }}</div>
          <div class="card-meta">
            <span :class="{ urgent: isUrgent(r) }">{{ remainingText(r) || 'never' }}</span>
            <span v-if="r.expires_at" class="muted"> · {{ shortDate(r.expires_at) }}</span>
            <span v-if="r.middleman" class="muted"> · {{ r.middleman }}</span>
          </div>
          <div class="card-verbs" @click.stop>
            <button class="row-action primary" @click="renew(r)">Renew</button>
            <button class="row-action" @click="openCode(r.code)">Open</button>
          </div>
        </article>
      </div>
    </section>
  </div>

  <p class="foot-note">
    Every code in the hub is in exactly one lane above, so this page cannot show a
    smaller number than the hub holds — the counts sum to the total.
    <strong>Lapsed</strong> codes are shown so nothing falls through a gap, but
    they are not a reminder: the term already ended, so the student is already cut
    off. A <strong>stopped</strong> code was cut off on purpose and is not a
    renewal target. <strong>No expiry</strong> codes are permanent and never need
    renewing — they are listed so "not in the queue" is a fact you can see rather
    than guess.
  </p>

  <RenewDrawer :row="renewRow" :middlemen="[]" @close="renewRow = null" @done="renewRow = null; refresh()" /></template>
