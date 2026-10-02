<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { call } from '../api'
import { toast } from '../toast'

/**
 * Used codes — the single-use record.
 *
 * # Why this page exists
 *
 * A code is single-use and is **not** tied to a device. The whole record of
 * "this code is in use" is `codes.activated_at`, so this page is that record:
 * which codes have been redeemed, when, and by whom (the middleman/label).
 *
 * It replaced a "Devices" page that listed device bindings. Device binding was
 * removed — a code is not tied to a machine — so there is no per-device view to
 * show. What an operator still needs is the answer to two questions:
 *
 *   * "is this code in use?" — status `redeemed` below;
 *   * "has this student's code been handed to someone else?" — the Release
 *     history, per code, on the Codes page.
 *
 * It is deliberately read-only: every mutation lives on the Codes page, where
 * it already has an audit trail. Adding write actions here would give an
 * operator two routes to the same change and only one of them familiar.
 */

interface CodeRow {
  code: string
  tier: string
  status: 'available' | 'redeemed' | 'suspended' | 'expired'
  redeemed: boolean
  activated_at: string
  expires_at: string
  label: string
  middleman: string
  days_remaining?: number | null
}

interface CodesResponse {
  codes: CodeRow[]
}

const rows = ref<CodeRow[]>([])
const loading = ref(true)
/**
 * Show only codes that have been used. The default hides the ones that have
 * not, because "who is using their code" is the question this page answers —
 * an available code is not yet anyone's.
 */
const usedOnly = ref(true)

async function load() {
  loading.value = true
  // No `status` filter is sent: the page needs the totals for both states, and
  // filtering client-side keeps the two counts honest against one snapshot.
  const res = await call<CodesResponse>('codes.list', {})
  if (res.ok && res.data) {
    rows.value = res.data.codes || []
  } else {
    toast.err(res.message || res.transportError || 'Could not load codes')
  }
  loading.value = false
}

const used = computed(() => rows.value.filter((r) => r.redeemed))
const available = computed(() => rows.value.filter((r) => !r.redeemed))
/** Suspended codes that were also used — worth surfacing, they are not "in use". */
const suspended = computed(() => rows.value.filter((r) => r.suspended))

const shown = computed(() => (usedOnly.value ? used.value : used.value.concat(available.value)))

function when(iso: string): string {
  if (!iso) return '—'
  const d = new Date(iso)
  return isNaN(d.getTime()) ? iso : d.toLocaleString()
}

function expiry(row: CodeRow): string {
  if (!row.expires_at) return '—'
  const days = row.days_remaining
  const date = row.expires_at.slice(0, 10)
  return days === null || days === undefined ? date : `${date} (${days}d)`
}

onMounted(load)
</script>

<template>
  <h1 class="page-title">Used codes</h1>
  <p class="page-sub">
    Codes that a student has activated. A code is single-use, and a student can
    enter it again on any machine to restore their access — it is not tied to a
    device, so there is nothing per-device to show.
  </p>

  <div v-if="loading" class="muted">Loading…</div>

  <template v-else>
    <div class="cards">
      <div class="card">
        <div class="n">{{ used.length }}</div>
        <div class="l">Used</div>
      </div>
      <div class="card">
        <div class="n">{{ available.length }}</div>
        <div class="l">Available</div>
      </div>
      <div class="card" :class="suspended.length ? 'bad' : 'good'">
        <div class="n">{{ suspended.length }}</div>
        <div class="l">Suspended</div>
      </div>
    </div>

    <div class="panel">
      <div class="actions" style="margin-bottom: 10px">
        <button class="tiny" :class="{ primary: usedOnly }" @click="usedOnly = true">
          Used only
        </button>
        <button class="tiny" :class="{ primary: !usedOnly }" @click="usedOnly = false">
          Include available
        </button>
        <button class="tiny" @click="load">Refresh</button>
      </div>

      <div v-if="shown.length === 0" class="muted">
        <template v-if="usedOnly">
          No code has been used yet. A code becomes "used" the first time a
          student activates it.
        </template>
        <template v-else> No codes yet. Create some on the Codes page. </template>
      </div>

      <table v-else>
        <thead>
          <tr>
            <th>Code</th>
            <th>Name</th>
            <th>Tier</th>
            <th>First used</th>
            <th>Expires</th>
            <th>Middleman</th>
            <th>State</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in shown" :key="row.code" :class="{ dim: !row.redeemed }">
            <td class="mono">{{ row.code }}</td>
            <td class="muted">{{ row.label || '—' }}</td>
            <td>{{ row.tier || '—' }}</td>
            <td class="muted">{{ when(row.activated_at) }}</td>
            <td class="muted">{{ expiry(row) }}</td>
            <td class="muted">{{ row.middleman || '—' }}</td>
            <td>
              <span v-if="row.suspended" class="badge suspended" title="An operator suspended this code; it is refused on activation and on heartbeat.">
                suspended
              </span>
              <span v-else-if="row.redeemed" class="badge bound" title="Used. The student can re-enter it on any machine to restore access.">
                used
              </span>
              <span v-else class="badge" title="Not used yet — available for a student to activate.">
                available
              </span>
            </td>
          </tr>
        </tbody>
      </table>

      <p class="muted" style="font-size: 12px; margin-top: 10px">
        To hand a code to a different student, use <strong>Release</strong> on
        the Codes page — it clears the used stamp and the release is recorded in
        that code's history. You do <strong>not</strong> need it to move a
        student to a new laptop: they simply enter the same code again.
      </p>
    </div>
  </template>
</template>
