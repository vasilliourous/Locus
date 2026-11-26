<script setup lang="ts">
// Distribution — who sold what, and what they owe.
//
// WHY THIS IS A PAGE NOW. `middlemen.list` existed, was correct, and was fetched
// on every Codes page load — to populate a `<datalist>` of names. The data was
// never shown. Meanwhile two documents name the missing view:
//
//   09-console.md §9.3   "No middleman ledger. `middlemen.list` gives counts, not
//                         money owed; remittance is reconciled manually."
//   07-billing.md §7.4   "The `middleman` field on each code is the only record of
//                         who owes what."
//
// Renewal made that worse rather than better: one code can now be paid for five
// times, so "how many codes did Sarah sell" stopped implying "how much did Sarah
// collect". `middlemen.summary` (new, in admin_console.pb.js) answers both, from
// data that already exists — the money comes out of the `price=` token the
// renewal event carries, which the console previously never sent.
//
// THE MONEY COLUMN IS HONEST OR IT IS ABSENT. A total is only a total when every
// renewal in the window carried a price. Otherwise the figure is a floor, and it
// says so — a money column that silently under-reports is worse than none,
// because it will be believed.
//
// FREE vs PAID is a SEGMENT, not a second page. 08-distribution §8.5: a middleman
// earns nothing on a free code, and free stock is the operator's to hand out. That
// makes it a different commercial thing, but the same entity operationally — one
// row per person, with their two stock types inside. Splitting it into its own tab
// would be the "Tiers" mistake: a surface that exists structurally without a job.

import { computed, onMounted, ref } from 'vue'
import { call } from '../api'
import { openCodes } from '../nav'
import StateBlock from '../components/StateBlock.vue'

interface Middleman {
  name: string
  codes: number
  available: number
  in_term: number
  due_7d: number
  suspended: number
  expired: number
  released_once: number
  free_codes: number
  paid_codes: number
  renewals: number
  collected: number
  priced_renewals: number
  collected_complete: boolean
}

const rows = ref<Middleman[]>([])
const loading = ref(true)
const failed = ref('')
const denied = ref(false)
const windowDays = ref(30)
const since = ref('')
const segment = ref<'all' | 'paid' | 'free'>('all')

async function load() {
  loading.value = true
  failed.value = ''
  const res = await call<{ middlemen: Middleman[]; window_days: number; since: string }>(
    'middlemen.summary',
    { window_days: windowDays.value },
  )
  loading.value = false
  if (res.message && /sign in again/i.test(res.message)) denied.value = true
  if (!res.ok || !res.data) {
    failed.value = res.message || res.transportError || 'Could not load the distribution summary.'
    return
  }
  rows.value = res.data.middlemen || []
  since.value = res.data.since || ''
}

onMounted(load)

const shown = computed(() => {
  if (segment.value === 'paid') return rows.value.filter((r) => r.paid_codes > 0)
  if (segment.value === 'free') return rows.value.filter((r) => r.free_codes > 0)
  return rows.value
})

/**
 * Column totals.
 *
 * Only the STOCK columns are summed. Money is deliberately not totalled here: a
 * grand total would need every middleman to have a complete figure, and folding
 * an incomplete one into a total is exactly the plausible-looking lie the
 * per-row flag exists to avoid. The per-row dash says "not recorded"; a total
 * would have no way to say that.
 */
const totals = computed(() => {
  const t = { codes: 0, in_term: 0, due_7d: 0, available: 0, suspended: 0, expired: 0, renewals: 0 }
  for (const r of shown.value) {
    t.codes += r.codes
    t.in_term += r.in_term
    t.due_7d += r.due_7d
    t.available += r.available
    t.suspended += r.suspended
    t.expired += r.expired
    t.renewals += r.renewals
  }
  return t
})

const anyMoney = computed(() => shown.value.some((r) => r.priced_renewals > 0))

function money(r: Middleman): string {
  if (!r.renewals) return '—'
  if (!r.priced_renewals) return '—'
  const n = r.collected.toFixed(2).replace(/\.00$/, '')
  return r.collected_complete ? n : `≥ ${n}`
}

function moneyTitle(r: Middleman): string {
  if (!r.renewals) return 'No renewals in this window.'
  if (!r.priced_renewals) return `${r.renewals} renewal(s), none recorded a price.`
  if (r.collected_complete) return `${r.renewals} renewal(s), all priced.`
  return `${r.priced_renewals} of ${r.renewals} renewals recorded a price, so this is a floor, not a total.`
}

function openFor(r: Middleman, status?: string) {
  openCodes({ middleman: r.name, status: status || '', view: 'all' })
}
</script>

<template>
  <div class="page-head">
    <div class="titles">
      <h1>Distribution</h1>
      <p class="page-sub">
        Who holds which stock, and what was collected through them.
      </p>
    </div>
    <div class="segmented">
      <button :class="{ on: windowDays === 30 }" @click="windowDays = 30; load()">30 days</button>
      <button :class="{ on: windowDays === 90 }" @click="windowDays = 90; load()">90 days</button>
      <button :class="{ on: windowDays === 365 }" @click="windowDays = 365; load()">Year</button>
    </div>
  </div>

  <StateBlock v-if="loading && !rows.length" state="loading" :rows="5" />
  <StateBlock v-else-if="denied" state="denied" @retry="load" />
  <StateBlock v-else-if="failed" state="error" :message="failed" @retry="load" />

  <template v-else>
    <div class="filters" style="margin-bottom: 12px">
      <div class="segmented">
        <button :class="{ on: segment === 'all' }" @click="segment = 'all'">All stock</button>
        <button :class="{ on: segment === 'paid' }" @click="segment = 'paid'">Paid</button>
        <button :class="{ on: segment === 'free' }" @click="segment = 'free'">Free</button>
      </div>
      <span class="muted" style="font-size: 12px">
        A middleman earns nothing on a free code — free stock is the operator's to
        hand out, and it is listed here so it is not mistaken for inventory.
      </span>
    </div>

    <div class="table-wrap">
      <StateBlock
        v-if="!shown.length"
        state="empty"
        title="Nobody has been attributed stock yet"
        message="A code is attributed when it is minted with a middleman name, or amended later in its detail panel."
      >
        <template #actions>
          <button @click="openCodes({ view: 'all' })">Open Codes</button>
        </template>
      </StateBlock>

      <table v-else class="data">
        <thead>
          <tr>
            <th>Middleman</th>
            <th class="num">Codes</th>
            <th class="num">In term</th>
            <th class="num">Due ≤7d</th>
            <th class="num">Unused</th>
            <th class="num">Stopped</th>
            <th class="num">Lapsed</th>
            <th class="num">Renewals</th>
            <th class="num">Collected</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="r in shown" :key="r.name">
            <td>
              <button
                style="border: none; background: none; padding: 0; color: var(--text); font: inherit; font-weight: 600"
                @click="openFor(r)"
              >
                {{ r.name }}
              </button>
              <div class="muted" style="font-size: 11.5px">
                <template v-if="r.paid_codes && r.free_codes">{{ r.paid_codes }} paid · {{ r.free_codes }} free</template>
                <template v-else-if="r.paid_codes">paid stock</template>
                <template v-else>free stock only</template>
                <template v-if="r.released_once"> · {{ r.released_once }} released</template>
              </div>
            </td>
            <td class="num">{{ r.codes }}</td>
            <td class="num">
              <button v-if="r.in_term" class="linkish num" @click="openFor(r, 'redeemed')">{{ r.in_term }}</button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num">
              <button v-if="r.due_7d" class="linkish num urgent" @click="openCodes({ middleman: r.name, view: 'worklist', expiring_within_days: 7 })">
                {{ r.due_7d }}
              </button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num">
              <button v-if="r.available" class="linkish num" @click="openFor(r, 'available')">{{ r.available }}</button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num">
              <button v-if="r.suspended" class="linkish num" @click="openFor(r, 'suspended')">{{ r.suspended }}</button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num">
              <button v-if="r.expired" class="linkish num" @click="openFor(r, 'expired')">{{ r.expired }}</button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num">
              <button v-if="r.renewals" class="linkish num" @click="openCodes({ middleman: r.name, view: 'all' })">{{ r.renewals }}</button>
              <span v-else class="muted">0</span>
            </td>
            <td class="num" :title="moneyTitle(r)">
              <span :class="{ muted: !r.priced_renewals }">{{ money(r) }}</span>
            </td>
          </tr>
        </tbody>
        <tfoot>
          <tr>
            <td class="muted" style="padding: 8px 12px; font-size: 12px">Totals</td>
            <td class="num muted">{{ totals.codes }}</td>
            <td class="num muted">{{ totals.in_term }}</td>
            <td class="num muted">{{ totals.due_7d }}</td>
            <td class="num muted">{{ totals.available }}</td>
            <td class="num muted">{{ totals.suspended }}</td>
            <td class="num muted">{{ totals.expired }}</td>
            <td class="num muted">{{ totals.renewals }}</td>
            <td class="num muted" title="Not totalled: a sum would require every row to have a complete figure">
              —
            </td>
          </tr>
        </tfoot>
      </table>
    </div>

    <p class="foot-note">
      <template v-if="anyMoney">
        <strong>Collected</strong> is what operators recorded at renewal, over the
        last {{ windowDays }} days<template v-if="since"> (since {{ since.slice(0, 10) }})</template>.
        A “≥” means some renewals in the window recorded no price, so the figure is
        a floor. <strong>Renewals</strong> counts every extension, priced or not.
      </template>
      <template v-else>
        <strong>No money has been recorded yet.</strong>
        Prices are captured when an operator renews a code and fills in the
        optional Price field. Cheap to do, and it cannot be reconstructed later.
      </template>
      There is no commission calculation here: the cut is a business agreement, not
      a stored value, and inventing a rate would produce a figure nobody can audit.
    </p>
  </template>
</template>

<style scoped>
/* A number that is also a link. Kept small and unadorned so a row of them still
   reads as a row of columns rather than as a toolbar. */
.linkish {
  border: none;
  background: none;
  padding: 0;
  color: var(--accent);
  font: inherit;
  cursor: pointer;
}
.linkish:hover { text-decoration: underline; }
tfoot td { border-bottom: none; border-top: 1px solid var(--border); }
</style>
