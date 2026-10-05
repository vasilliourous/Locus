<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue'
import { call } from '../api'
import { toast } from '../toast'

interface CodeRow {
  id: string
  code: string
  tier: string
  status: 'available' | 'redeemed' | 'suspended' | 'expired'
  /**
   * Whether this code has been redeemed. A code is single-use and is not tied
   * to a device, so this replaces the old `bound` flag.
   */
  redeemed: boolean
  suspended: boolean
  expires_at: string
  activated_at: string
  middleman: string
  label: string
  notes: string
  /** How long one purchase lasts, in days. 0/absent = never expires. */
  term_days?: number | null
  term_kind?: string
  /** Whole days until expiry (rounded up), or null when there is no expiry. */
  days_remaining?: number | null
}

const codes = ref<CodeRow[]>([])
const loading = ref(false)

// Row selection, for bulk delete. A Set keyed by code string; cleared whenever
// the filter changes or a bulk action completes, so a stale tick cannot delete
// a row the operator can no longer see.
const selected = ref<Set<string>>(new Set())
const selectedCount = computed(() => selected.value.size)
function toggleSelected(code: string) {
  const next = new Set(selected.value)
  if (next.has(code)) next.delete(code)
  else next.add(code)
  selected.value = next
}
function toggleSelectAll() {
  if (selected.value.size === codes.value.length) selected.value = new Set()
  else selected.value = new Set(codes.value.map((c) => c.code))
}

// Filters
const query = ref('')
const tierFilter = ref('')
const statusFilter = ref('')
const middlemanFilter = ref('')
/** 0 = all codes; >0 = only codes expiring within this many days. */
const worklistDays = ref(0)

// Set by another page (the Dashboard's expiring card) to open this view already
// filtered to the renewal worklist. Applied once on mount, then owned by the
// local toggle buttons — the parent must not keep re-imposing a mode the
// operator has since changed.
const props = defineProps<{ worklistDays?: number }>()

// Generation form
// Defaults to `free`: the retired `eco` row was removed from tier_configs, so
// minting against it would now fail with a tier that does not resolve. The
// form is re-hydrated from `tiers.list` on load; this is the pre-load value.
const genTier = ref('free')
const genCount = ref(10)
const genExpires = ref('')
// The term is the PREFERRED way to mint: the clock starts at activation, so a
// card does not decay on the shelf while it waits to be sold. Defaults to 70
// days (~10 weeks), matching the term-pass product the business plan leads
// with; 0 means the code never expires.
const genTermDays = ref(70)
const genMiddleman = ref('')
const genLabel = ref('')
const genNotes = ref('')
const generating = ref(false)
const generated = ref<string[]>([])

// Per-code detail
const detail = ref<CodeRow | null>(null)
const history = ref<{ event: string; detail: string; created: string }[]>([])
const historyLoading = ref(false)

// A filter change invalidates any ticked rows: the operator can no longer see
// what they selected, and "delete N selected" where N counts rows that scrolled
// out of view is how a bulk delete goes wrong.
watch([query, tierFilter, statusFilter, middlemanFilter, worklistDays], () => {
  selected.value = new Set()
})

// Pre-load placeholder only: `loadMeta()` replaces this with the hub's real
// `tier_configs` rows via `tiers.list`. It exists so the dropdown is not empty
// on first paint. It must still name only tiers the hub actually seeds — a
// placeholder offering a retired tier is a dropdown entry that mints a code
// which can never resolve, and `check-consistency.sh` §27 holds it to
// `seed-pb.py` for exactly that reason.
const tiers = ref<string[]>(['free', 'strike'])
const middlemen = ref<{ name: string; codes: number }[]>([])

async function load() {
  loading.value = true
  const res = await call<{ codes: CodeRow[]; total: number }>('codes.list', {
    query: query.value,
    tier: tierFilter.value,
    status: statusFilter.value,
    middleman: middlemanFilter.value,
    // 0 = no window (every code). The hub treats anything <= 0 as "no window",
    // so sending 0 is the explicit "all codes" case rather than an omission.
    expiring_within_days: worklistDays.value,
  })
  if (res.ok && res.data) {
    codes.value = res.data.codes
  } else {
    toast.err(res.message || res.transportError || 'Could not load codes')
  }
  loading.value = false
}

/**
 * Switch between the full list and a renewal worklist.
 *
 * Reloads immediately: this is a mode change the operator performs in order to
 * act, so leaving the old rows on screen until they press something else would
 * mean acting on the wrong set.
 */
async function setWorklist(days: number) {
  worklistDays.value = days
  selected.value = new Set()
  await load()
}

async function loadMeta() {
  const t = await call<{ tiers: { tier: string }[] }>('tiers.list')
  if (t.ok && t.data) tiers.value = t.data.tiers.map((x) => x.tier)

  const m = await call<{ middlemen: { name: string; codes: number }[] }>('middlemen.list')
  if (m.ok && m.data) middlemen.value = m.data.middlemen
}

function defaultExpiry(): string {
  // One year out — the previous codes all expired ~12 months after issue.
  const d = new Date()
  d.setFullYear(d.getFullYear() + 1)
  return d.toISOString().slice(0, 10)
}

async function generate() {
  if (!genTier.value) {
    toast.err('Choose a tier first')
    return
  }
  const count = Number(genCount.value)
  if (!Number.isFinite(count) || count < 1 || count > 500) {
    toast.err('Number of codes must be between 1 and 500')
    return
  }
  generating.value = true
  generated.value = []
  try {
    const res = await call<{ created: string[]; skipped: number }>('codes.generate', {
      tier: genTier.value,
      count,
      // Both are sent when both are set. The hub treats the term as the
      // authority at ACTIVATION (it recomputes the expiry then), and the
      // absolute date as the mint-time value a code carries until it is
      // activated — so the two do not fight.
      term_days: genTermDays.value || 0,
      expires_at: genExpires.value,
      middleman: genMiddleman.value,
      label: genLabel.value,
      notes: genNotes.value,
    })
    if (res.ok && res.data) {
      generated.value = res.data.created
      toast.ok(`Created ${res.data.created.length} code(s)` + (res.data.skipped ? `, ${res.data.skipped} skipped` : ''))
      await load()
      await loadMeta()
    } else {
      toast.err(res.message || res.transportError || 'Could not create codes')
    }
  } finally {
    generating.value = false
  }
}

async function setSuspended(row: CodeRow, suspend: boolean) {
  const res = await call('codes.' + (suspend ? 'suspend' : 'unsuspend'), { code: row.code })
  if (res.ok) {
    toast.ok(`${suspend ? 'Suspended' : 'Reactivated'} ${row.code}`)
    await load()
  } else {
    toast.err(res.message || res.transportError || 'Action failed')
  }
}

async function unbind(row: CodeRow) {
  const reason = window.prompt(
    `Release ${row.code}?\n\n` +
      `A code is single-use and is NOT tied to a device. Releasing it clears the ` +
      `used stamp so a DIFFERENT student can activate it.\n\n` +
      `You do not need this to move a student to a new laptop — they simply enter ` +
      `the same code again and it restores their access.`,
    'Re-issued to a different student',
  )
  if (reason === null) return
  const res = await call('codes.unbind', { code: row.code, reason })
  if (res.ok) {
    toast.ok(`Released ${row.code} — it is available again`)
    await load()
    if (detail.value?.code === row.code) await openDetail(row)
  } else {
    toast.err(res.message || res.transportError || 'Release failed')
  }
}

/**
 * Renew a code after a middleman reports payment.
 *
 * The whole point of this action is that the operator never does date
 * arithmetic in their head. The confirm prompt therefore shows the BEFORE and
 * AFTER expiry, computed by the hub, and the operator approves a concrete
 * change rather than a number of days.
 *
 * A renewal EXTENDS FROM THE EXISTING EXPIRY — renewing a week early keeps the
 * week and adds the new term on top. That is stated in the prompt because the
 * alternative (measuring from today) would silently delete paid time, and an
 * operator quoting a date to a middleman needs to know which it is.
 */
async function renew(row: CodeRow) {
  const current = row.expires_at ? row.expires_at.slice(0, 10) : 'no expiry'
  const stated = window.prompt(
    `Renew ${row.code} (${row.tier})?\n\n` +
      `Current expiry: ${current}\n\n` +
      `Enter the number of DAYS being purchased (e.g. 70 for a ~10-week term, ` +
      `30 for a month).\n\n` +
      `The new term is ADDED to the current expiry, so paying early never loses days.`,
    '70',
  )
  if (stated === null) return
  const termDays = Number(stated)
  if (!Number.isFinite(termDays) || termDays <= 0) {
    toast.err('Enter a positive number of days')
    return
  }

  // The middleman reference is optional but recorded in the audit detail, so
  // "how much did X collect this month" is answerable later. Cash is collected
  // by middlemen, and once a code can be renewed repeatedly that stops being
  // derivable from a code count.
  const reference = window.prompt(
    `Optional: middleman name and price, recorded in the audit trail.\n\n` +
      `Format: name, price   (e.g. "Wanzhen, 10")\n` +
      `Leave blank to skip.`,
    row.middleman || '',
  )
  if (reference === null) return
  const [mmName, mmPrice] = reference.split(',').map((s) => s.trim())

  const res = await call<{ expires_at: string; previous_expires_at: string; extended: boolean }>(
    'codes.renew',
    {
      code: row.code,
      term_days: termDays,
      middleman: mmName || '',
      price: mmPrice || '',
    },
  )
  if (res.ok && res.data) {
    const before = res.data.previous_expires_at ? res.data.previous_expires_at.slice(0, 10) : 'none'
    const after = res.data.expires_at.slice(0, 10)
    toast.ok(
      `Renewed ${row.code}: ${before} → ${after}` +
        (res.data.extended ? ' (extended from the existing expiry)' : ''),
    )
    await load()
    if (detail.value?.code === row.code) {
      const fresh = codes.value.find((c) => c.code === row.code)
      if (fresh) await openDetail(fresh)
    }
  } else {
    toast.err(res.message || res.transportError || 'Renew failed')
  }
}

/**
 * Set a code's TERM (how long one purchase lasts), without touching its expiry.
 *
 * Distinct from Renew, which extends an expiry. This changes what future
 * activations and renewals produce; it must never rewrite a date a student is
 * already running on, which is why it is a separate button.
 */
async function setTerm(row: CodeRow) {
  const stated = window.prompt(
    `Set the term for ${row.code}.\n\n` +
      `The term is how long ONE purchase lasts, measured from ACTIVATION.\n` +
      `Enter days (70 ≈ a 10-week term, 30 ≈ a month, 365 ≈ a year).\n` +
      `Enter 0 for a code that never expires.\n\n` +
      `This does NOT change the current expiry (${row.expires_at ? row.expires_at.slice(0, 10) : 'none'}).`,
    '70',
  )
  if (stated === null) return
  const days = Number(stated)
  if (!Number.isFinite(days) || days < 0) {
    toast.err('Enter a number of days (0 = never expires)')
    return
  }
  const res = await call('codes.set-term', { code: row.code, term_days: days })
  if (res.ok) {
    toast.ok(`Term for ${row.code} set to ${days > 0 ? `${days}d` : 'never expires'} (expiry unchanged)`)
    await load()
    if (detail.value?.code === row.code) await openDetail(row)
  } else {
    toast.err(res.message || res.transportError || 'Could not set the term')
  }
}

/**
 * Permanently delete ONE code.
 *
 * Deliberately the only destructive action in the console. The standing rule is
 * "suspend or unbind, never delete" (see docs/CONTEXT.md), so this exists only
 * for the cases that rule does not cover — a typo, a duplicate, a test row —
 * and it refuses a BOUND code outright, because that is a student mid-service.
 * The hook writes an audit event before the row goes, so the code_events trail
 * outlives the code.
 *
 * The confirmation names the code and its tier/middleman so a mis-click on the
 * wrong row is caught by reading, not by luck.
 */
async function deleteCode(row: CodeRow) {
  if (row.redeemed) {
    toast.err(`${row.code} has been used by a student — release it first`)
    return
  }
  const what = [row.tier, row.middleman].filter(Boolean).join(' / ')
  const sure = window.confirm(
    `Permanently DELETE ${row.code}${what ? ` (${what})` : ''}?\n\n` +
      `This removes the code from the hub. It cannot be undone — an audit ` +
      `entry is kept, but the code itself is gone and cannot be re-activated.`,
  )
  if (!sure) return
  const res = await call('codes.delete', { code: row.code })
  if (res.ok) {
    toast.ok(`Deleted ${row.code}`)
    if (detail.value?.code === row.code) detail.value = null
    await load()
  } else {
    toast.err(res.message || res.transportError || 'Delete failed')
  }
}

/**
 * Bulk-delete the codes ticked in the table.
 *
 * Redeemed codes are always skipped by the hook (there is no force flag on the
 * batch path), and the result reports what was skipped and why — a bulk delete
 * that silently left three rows behind would be worse than one that failed.
 */
async function deleteSelected() {
  const chosen = codes.value.filter((r) => selected.value.has(r.code)).map((r) => r.code)
  if (!chosen.length) return
  const used = codes.value.filter((r) => selected.value.has(r.code) && r.redeemed).length
  const sure = window.confirm(
    `Permanently DELETE ${chosen.length} code(s)?\n\n` +
      (used ? `${used} of them have been used by a student and will be SKIPPED.\n\n` : '') +
      `Cannot be undone. An audit entry is kept for each.`,
  )
  if (!sure) return
  const res = await call('codes.deleteBatch', { codes: chosen })
  if (res.ok) {
    const d = Number(res.deleted_count ?? 0)
    const s = Number(res.skipped_count ?? 0)
    if (s) {
      const why = (res.skipped || []).map((x: any) => `${x.code}: ${x.why}`).join('; ')
      toast.err(`Deleted ${d}; skipped ${s} (${why})`)
    } else {
      toast.ok(`Deleted ${d} code(s)`)
    }
    selected.value = new Set()
    await load()
  } else {
    toast.err(res.message || res.transportError || 'Bulk delete failed')
  }
}

/**
 * Set or clear a code's expiry.
 *
 * `codes.expire` existed in the hook but had no UI at all, so an operator could
 * only change an expiry by hand-editing PocketBase. That was tolerable while
 * expiry was advisory; it is not any more, because an expired code is now
 * refused on re-activation as well as on first use (see FIXES.md 33) — a lapsed
 * code needs a remedy the operator can reach.
 *
 * Empty clears the expiry (the code then never expires).
 */
async function setExpiry(row: CodeRow, when: string) {
  const res = await call('codes.expire', { code: row.code, expires_at: when })
  if (res.ok) {
    toast.ok(when ? `Expires ${when.slice(0, 10)}` : 'Expiry cleared')
    await load()
    if (detail.value?.code === row.code) {
      detail.value.expires_at = when
      await openDetail(detail.value)
    }
  } else {
    toast.err(res.message || res.transportError || 'Could not set expiry')
  }
}

/** Prompt for a new expiry date, pre-filled with the current one. */
async function editExpiry(row: CodeRow) {
  const current = row.expires_at ? row.expires_at.slice(0, 10) : ''
  const input = window.prompt(
    `Expiry for ${row.code}\n\nYYYY-MM-DD, or leave blank for no expiry.\n` +
      `A code past its expiry is refused on activation AND on re-activation.`,
    current,
  )
  if (input === null) return
  const trimmed = input.trim()
  if (trimmed && !/^\d{4}-\d{2}-\d{2}/.test(trimmed)) {
    toast.err('Enter a date like 2027-03-01, or leave blank')
    return
  }
  // Send a full timestamp so the server stores a well-formed date; PocketBase's
  // date field accepts ISO 8601.
  await setExpiry(row, trimmed ? `${trimmed}T00:00:00.000Z` : '')
}

async function openDetail(row: CodeRow) {
  detail.value = row
  historyLoading.value = true
  history.value = []

  const res = await call<{ events: { event: string; detail: string; created: string }[] }>(
    'codes.history',
    { code: row.code },
  )
  if (res.ok && res.data) history.value = res.data.events
  historyLoading.value = false
}

async function saveDetail() {
  if (!detail.value) return
  const res = await call('codes.update', {
    code: detail.value.code,
    middleman: detail.value.middleman,
    label: detail.value.label,
    notes: detail.value.notes,
  })
  if (res.ok) {
    toast.ok('Saved')
    await load()
    await loadMeta()
  } else {
    toast.err(res.message || res.transportError || 'Save failed')
  }
}

async function copyCodes(list: string[]) {
  const text = list.join('\n')
  try {
    await navigator.clipboard.writeText(text)
    toast.ok(`Copied ${list.length} code(s) to the clipboard`)
  } catch {
    toast.warn('Could not copy automatically — select the text below and copy manually')
  }
}

function downloadCsv() {
  const rows = [['code', 'tier', 'status', 'middleman', 'label', 'expires_at', 'activated_at']]
  for (const c of codes.value) {
    rows.push([c.code, c.tier, c.status, c.middleman, c.label, c.expires_at, c.activated_at])
  }
  const csv = rows.map((r) => r.map((v) => `"${String(v).replace(/"/g, '""')}"`).join(',')).join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `locus-codes-${new Date().toISOString().slice(0, 10)}.csv`
  a.click()
  URL.revokeObjectURL(a.href)
}

const shown = computed(() => codes.value.length)

onMounted(async () => {
  genExpires.value = defaultExpiry()
  await loadMeta()
  await load()
})

// The Dashboard's "expiring" card can ask for the worklist while this view is
// ALREADY mounted (the operator went Codes -> Dashboard -> card), in which case
// `onMounted` will not run again. Watching the prop covers both paths: a first
// mount and a return visit.
watch(
  () => props.worklistDays,
  (days) => {
    if (typeof days === 'number' && days >= 0) {
      worklistDays.value = days
      selected.value = new Set()
      void load()
    }
  },
)
</script>

<template>
  <h1 class="page-title">Codes &amp; Clients</h1>
  <p class="page-sub">Generate activation codes, see who is using them, and fix problems.</p>

  <!-- ── Generate ── -->
  <div class="panel">
    <h2>Create codes</h2>
    <div class="row">
      <label class="field">
        <span>Tier</span>
        <select v-model="genTier">
          <option v-for="t in tiers" :key="t" :value="t">{{ t }}</option>
        </select>
      </label>
      <label class="field">
        <span>How many</span>
        <input v-model.number="genCount" type="number" min="1" max="500" />
      </label>
      <label class="field">
        <span>Term (days from activation)</span>
        <input v-model.number="genTermDays" type="number" min="0" placeholder="70 = a 10-week term" />
      </label>
      <label class="field">
        <span>Expires (absolute, overrides term)</span>
        <input v-model="genExpires" type="date" />
      </label>
      <label class="field">
        <span>For middleman (optional)</span>
        <input v-model="genMiddleman" list="middlemen-list" placeholder="e.g. Sarah" />
        <datalist id="middlemen-list">
          <option v-for="m in middlemen" :key="m.name" :value="m.name" />
        </datalist>
      </label>
    </div>
    <div class="row">
      <label class="field">
        <span>Label (optional)</span>
        <input v-model="genLabel" placeholder="e.g. Macleans Year 13" />
      </label>
      <label class="field">
        <span>Notes (optional)</span>
        <input v-model="genNotes" placeholder="free-form" />
      </label>
      <div class="shrink">
        <button class="primary" :disabled="generating" @click="generate">
          {{ generating ? 'Creating…' : 'Create codes' }}
        </button>
      </div>
    </div>

    <div v-if="generated.length" class="msg ok" style="margin-top: 12px">
      <div><strong>Created {{ generated.length }} code(s):</strong></div>
      <div class="pre-wrap mono" style="margin-top: 8px; max-height: 220px; overflow: auto">{{ generated.join('\n') }}</div>
      <div class="actions">
        <button class="tiny" @click="copyCodes(generated)">Copy all</button>
        <button class="tiny" @click="generated = []">Hide</button>
      </div>
    </div>
  </div>

  <!-- ── Filter / list ── -->
  <div class="panel">
    <h2>Issued codes</h2>

    <!-- The renewal worklist.
         Renewal is an operator action and nothing in the system prompts one:
         the hub holds no identity, so it cannot know a payment is due, only
         that a date is approaching. This toggle is therefore the revenue
         mechanism, not a convenience — if nobody opens it, students lapse
         silently and no money is collected.
         The 7-day window matches the client's own warning threshold, so the
         student's notice and the operator's worklist agree on what "soon"
         means. -->
    <div class="actions" style="margin-bottom: 10px">
      <button
        class="tiny"
        :class="{ primary: worklistDays === 0 }"
        @click="setWorklist(0)"
      >All codes</button>
      <button
        class="tiny"
        :class="{ primary: worklistDays === 7 }"
        @click="setWorklist(7)"
      >Due in 7 days</button>
      <button
        class="tiny"
        :class="{ primary: worklistDays === 30 }"
        @click="setWorklist(30)"
      >Due in 30 days</button>
      <span v-if="worklistDays > 0" class="muted" style="font-size: 12px">
        {{ codes.length }} code(s) ending within {{ worklistDays }} days, most urgent first.
        Renew each as its payment is reported. Suspended codes are excluded.
      </span>
    </div>

    <div class="row">
      <label class="field">
        <span>Search code</span>
        <input v-model="query" placeholder="code, name or notes" @keyup.enter="load" />
      </label>
      <label class="field">
        <span>Tier</span>
        <select v-model="tierFilter" @change="load">
          <option value="">All</option>
          <option v-for="t in tiers" :key="t" :value="t">{{ t }}</option>
        </select>
      </label>
      <label class="field">
        <span>Status</span>
        <select v-model="statusFilter" @change="load">
          <option value="">All</option>
          <option value="available">Available</option>
          <option value="redeemed">Used</option>
          <option value="suspended">Suspended</option>
          <option value="expired">Expired</option>
        </select>
      </label>
      <label class="field">
        <span>Middleman</span>
        <input v-model="middlemanFilter" placeholder="name" @keyup.enter="load" />
      </label>
      <div class="shrink">
        <button @click="load">Apply</button>
      </div>
      <div class="shrink">
        <button @click="downloadCsv">Export CSV</button>
      </div>
      <div v-if="selectedCount" class="shrink">
        <button class="tiny danger" @click="deleteSelected">Delete {{ selectedCount }} selected</button>
      </div>
    </div>

    <div v-if="loading" class="muted" style="margin-top: 10px">Loading…</div>
    <div v-else-if="shown === 0" class="muted" style="margin-top: 10px">
      No codes match. Create some above, or clear the filters.
    </div>
    <table v-else style="margin-top: 10px">
      <thead>
        <tr>
          <th style="width: 26px">
            <input
              type="checkbox"
              :checked="selectedCount > 0 && selectedCount === codes.length"
              @change="toggleSelectAll"
              title="Select all shown"
            />
          </th>
          <th>Code</th><th>Tier</th><th>Status</th><th>Middleman</th>
          <th>Expires</th><th>Activated</th><th></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in codes" :key="row.id">
          <td>
            <input
              type="checkbox"
              :checked="selected.has(row.code)"
              @change="toggleSelected(row.code)"
            />
          </td>
          <td>
            <code>{{ row.code }}</code>
            <div v-if="row.label" class="muted" style="font-size: 11px">{{ row.label }}</div>
          </td>
          <td>{{ row.tier }}</td>
          <td><span class="badge" :class="row.status">{{ row.status }}</span></td>
          <td class="muted">{{ row.middleman || '—' }}</td>
          <!-- Expiry shows the DATE and the days left. A worklist is worked by
               urgency, and "3 days" is the fact the operator acts on; the date
               is kept because it is what a student or middleman will quote. -->
          <td class="muted">
            {{ row.expires_at ? row.expires_at.slice(0, 10) : '—' }}
            <span v-if="row.days_remaining !== null && row.days_remaining !== undefined">
              <strong v-if="row.days_remaining <= 7" class="urgent">
                ({{ row.days_remaining }}d)
              </strong>
              <span v-else>({{ row.days_remaining }}d)</span>
            </span>
          </td>
          <td class="muted">{{ row.activated_at ? row.activated_at.slice(0, 10) : '—' }}</td>
          <td>
            <div class="actions" style="margin: 0">
              <button class="tiny" @click="openDetail(row)">Details</button>
              <!-- Renew is the payment operation: the middleman has reported
                   the money, so extend the term. Placed first among the
                   mutating actions because it is the one performed routinely. -->
              <button class="tiny primary" @click="renew(row)">Renew</button>
              <button v-if="row.redeemed" class="tiny" @click="unbind(row)">Release</button>
              <button class="tiny" @click="editExpiry(row)">Expiry</button>
              <button v-if="!row.suspended" class="tiny danger" @click="setSuspended(row, true)">Suspend</button>
              <button v-else class="tiny" @click="setSuspended(row, false)">Reactivate</button>
              <button
                class="tiny danger"
                :disabled="row.redeemed"
                :title="row.redeemed ? 'Release it first' : 'Permanently delete this code'"
                @click="deleteCode(row)"
              >Delete</button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <!-- ── Detail modal ── -->
  <div v-if="detail" class="overlay" @click.self="detail = null">
    <div class="modal">
      <h2 style="margin-top: 0"><code>{{ detail.code }}</code></h2>
      <p class="muted" style="margin-top: 0">
        Tier <strong>{{ detail.tier }}</strong> ·
        status <strong>{{ detail.status }}</strong>
        <span v-if="detail.activated_at"> · first used {{ detail.activated_at.slice(0, 10) }}</span>
      </p>

      <label class="field">
        <span>Middleman</span>
        <input v-model="detail.middleman" list="middlemen-list" />
      </label>
      <label class="field">
        <span>Label</span>
        <input v-model="detail.label" />
      </label>
      <label class="field">
        <span>Notes</span>
        <textarea v-model="detail.notes"></textarea>
      </label>

      <label class="field">
        <span>Expiry</span>
        <input
          :value="detail.expires_at ? detail.expires_at.slice(0, 10) : ''"
          placeholder="YYYY-MM-DD (blank = never expires)"
          @change="editExpiry(detail)"
        />
      </label>

      <div class="actions">
        <button class="primary" @click="saveDetail">Save details</button>
        <!-- Renew is the routine action a student's payment triggers, so it
             leads. Set term is separate and less frequent: it changes what
             FUTURE renewals produce, and deliberately never moves the expiry
             of the term already running. -->
        <button class="primary" @click="renew(detail)">Renew</button>
        <button @click="setTerm(detail)">
          Set term{{ detail.term_days ? ` (${detail.term_days}d)` : ' (none)' }}
        </button>
        <!-- Release is for handing the code to a DIFFERENT student. A student
             moving to a new laptop just enters the code again. -->
        <button v-if="detail.redeemed" @click="unbind(detail)">Release code</button>
        <button
          v-if="!detail.suspended"
          class="danger"
          @click="setSuspended(detail, true); detail = null"
        >Suspend</button>
        <button v-else @click="setSuspended(detail, false); detail = null">Reactivate</button>
        <button
          class="danger"
          :disabled="detail.redeemed"
          :title="detail.redeemed ? 'Release it first' : 'Permanently delete this code'"
          @click="deleteCode(detail)"
        >Delete code</button>
        <button @click="detail = null">Close</button>
      </div>

      <!-- State the two invariants the operator would otherwise have to
           remember: what this code's term is, and that renewing ADDS to the
           expiry rather than replacing it. -->
      <p class="muted" style="font-size: 12px; margin-top: 8px">
        Term:
        <strong>{{ detail.term_days ? `${detail.term_days} days from activation` : 'none (never expires)' }}</strong>
        · Renewing adds the purchased days to the current expiry, so paying early
        never loses days · Renew does not change the tier or whether the code is in use.
      </p>

      <!-- A code is single-use and is not tied to a device. This states the
           rule where the operator acts on a code, so "Release" is not mistaken
           for "move this student to a new laptop". -->
      <div class="muted" style="font-size: 12px; margin-top: 6px">
        <template v-if="detail.redeemed">
          <strong>Used</strong> — first activated
          {{ detail.activated_at ? detail.activated_at.slice(0, 16).replace('T', ' ') : '' }}.
          The student can enter this code again on any machine and it restores
          their access; use <strong>Release</strong> only to hand it to a
          different student.
        </template>
        <template v-else>
          <strong>Not used yet</strong> — available for a student to activate.
        </template>
      </div>

      <h3 style="font-size: 13px; margin-bottom: 6px">History</h3>
      <div v-if="historyLoading" class="muted">Loading…</div>
      <div v-else-if="history.length === 0" class="muted">No recorded events for this code.</div>
      <table v-else>
        <tbody>
          <tr v-for="(h, i) in history" :key="i">
            <td class="muted">{{ h.created ? h.created.slice(0, 16).replace('T', ' ') : '' }}</td>
            <td>{{ h.event }}</td>
            <td class="muted">{{ h.detail }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
