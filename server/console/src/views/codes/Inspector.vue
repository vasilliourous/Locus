<script setup lang="ts">
// The inspector: one code, in full, in a persistent right-hand rail.
//
// WHAT THIS REPLACES, AND WHY
//
// The previous detail was a 431-line slide-over with four tabs — Detail,
// Actions, History, Danger. It covered the list behind a scrim, so "renew this
// one, then the next" meant close → find the row again → open. History was a tab,
// which meant the audit trail (the thing you need while a student is on the
// phone) was two clicks away. And release/delete were on a tab called "Danger",
// which is an honest name for an organisational failure rather than a design.
//
// Now: no tabs. One scrolling column, ordered by how often it is needed:
//
//   identity   — code, name, plan, middleman, state. Read at a glance.
//   verdict    — one sentence: is this healthy, and if not what kind of not.
//   the facts  — term, expiry, last seen, who sold it. Expiry and Term are
//                EDITABLE tiles, not values behind a disclosure: they are
//                always-manual inputs and belong at the top level.
//   actions    — renew/stop, then the direct STATE actions (release, mark in
//                term) with no confirmation step, then the session probe and its
//                visible result.
//   who        — the name/plan/middleman EDITORS (the label is a name here).
//   danger     — delete, last and confirmed in the panel rather than by
//                window.confirm.
//   history    — a closed fold at the very bottom: support-call reading, not
//   record     — the work of managing a code. Both are demoted, not deleted.
//
// The rail stays put while the list scrolls beside it, so selecting another row
// swaps the contents and nothing else moves. That is the difference between a
// view you navigate and a place you work.
//
// ONE FETCH PER SELECTION. The panel asks the hub for the code it was told to
// show (`fetchCode`) plus its history, in parallel. It does not re-find the row
// in the list, because the list may be filtered to rows that exclude it — which
// is exactly the case a URL like /admin/codes/RQ-ABCD opens.

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { CodeRow, HistoryEvent, ProbeSummary } from './model'
import {
  livenessOf,
  livenessText,
  remainingText,
  shortDate,
  shortDateTime,
  stateClass,
  stateLabel,
  overrideNote,
} from './model'
import { act, fetchCode, fetchHistory } from './data'
import { runSessionProbe, type ProbeResponse } from '../../api'
import { meta } from './meta'
import { codeKey } from '../../router'

const props = defineProps<{
  /** The code to show, from the URL. '' means nothing selected. */
  code: string
  /** The list's already-loaded rows, used for an instant paint. */
  rows: CodeRow[]
  /** The tier list as the LIST sees it (facets), so the plan filter and the
   *  inspector offer the same vocabulary even if meta has not loaded. */
  tiers: string[]
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'changed'): void
  (e: 'renew', row: CodeRow): void
  (e: 'stop', row: CodeRow): void
  /**
   * One line for the LIST's header: the probe ran, when, and what it concluded.
   *
   * D1 asks for the last run to be legible from the main view without a log
   * viewer. The rail shows the detail for the code on screen, but switching
   * codes would otherwise erase the only evidence that a run happened — so the
   * summary is lifted to the page that owns the selection.
   */
  (e: 'probed', summary: ProbeSummary): void
}>()

export interface ProbeSummary {
  code: string
  ranAt: string
  /** Fields the probe evaluated (machine gate, entitlement, liveness, override). */
  checks: number
  /** Flags the hub raised for this code. */
  findings: number
  /** Always 0: the probe is documented as write-nothing. */
  changed: number
  /** '' on success; the hub's or the transport's own message otherwise. */
  error: string
}

// ── The row ──
//
// Seeded synchronously from the list when the code is already there, so opening
// a row paints instantly instead of flashing a skeleton over data we hold. The
// authoritative fetch follows and replaces it.
const row = ref<CodeRow | null>(null)
const events = ref<HistoryEvent[]>([])
const historyLoading = ref(false)
const loading = ref(false)
const notFound = ref(false)

const listRow = computed(() => {
  if (!props.code) return null
  const want = codeKey(props.code)
  return props.rows.find((r) => codeKey(r.code) === want) || null
})

async function refresh() {
  if (!props.code) return
  const seeded = listRow.value
  if (seeded) row.value = seeded
  notFound.value = false
  loading.value = !seeded
  historyLoading.value = true

  // The selection this request was started for. A response that arrives after
  // the operator has moved to another code must be dropped, or the rail shows
  // one code's facts under another code's heading — the classic stale-panel bug.
  const want = codeKey(props.code)

  // Parallel: the row and its history are independent reads and the panel is
  // unusable with only one.
  const [fresh, hist] = await Promise.all([fetchCode(props.code), fetchHistory(props.code)])

  if (codeKey(props.code) !== want) return

  loading.value = false
  historyLoading.value = false
  if (fresh) {
    row.value = fresh
    notFound.value = false
  } else if (!seeded) {
    // The hub does not hold it. A real answer, displayed as one.
    row.value = null
    notFound.value = true
  }
  events.value = hist
}

watch(
  () => props.code,
  () => {
    row.value = null
    events.value = []
    notFound.value = false
    if (props.code) void refresh()
  },
  { immediate: true },
)

// ── Edit fields ──
// Held locally so a half-typed name is not written on every keystroke, and reset
// whenever the panel shows a different code.
const label = ref('')
const middleman = ref('')
const notes = ref('')
const tier = ref('')
const saving = ref(false)

watch(
  () => row.value?.code,
  () => {
    label.value = row.value?.label || ''
    middleman.value = row.value?.middleman || ''
    notes.value = row.value?.notes || ''
    tier.value = row.value?.tier || ''
  },
  { immediate: true },
)

const dirty = computed(
  () =>
    row.value !== null &&
    (label.value !== (row.value.label || '') ||
      middleman.value !== (row.value.middleman || '') ||
      notes.value !== (row.value.notes || '') ||
      tier.value !== (row.value.tier || '')),
)

/**
 * Warn before a half-typed name is silently thrown away.
 *
 * Selecting another row, closing the rail, or navigating away used to discard the
 * edits with no signal — the panel simply repainted. The operator would type a
 * student's name, click the next code to look something up, and lose it.
 *
 * `beforeunload` covers reload and tab close, and only registers while something
 * is actually dirty so it never nags on a clean panel. In-app navigation is
 * guarded by the parent (Codes.vue) asking `hasUnsavedWork()` before it navigates
 * — a module-level callback rather than a router guard, because this is the only
 * page with editable state.
 */
function onBeforeUnload(e: BeforeUnloadEvent) {
  if (!dirty.value) return
  e.preventDefault()
  // Safari and older browsers require returnValue; the string is ignored by
  // modern ones, which show their own generic message.
  e.returnValue = ''
}

onMounted(() => window.addEventListener('beforeunload', onBeforeUnload))
onBeforeUnmount(() => window.removeEventListener('beforeunload', onBeforeUnload))

/**
 * Exposed to the route owner.
 *
 * `reload()` exists because of a real bug found in a browser: renewing a code
 * from the drawer changed it on the hub and in the TABLE, but the inspector kept
 * showing the old expiry — it only re-fetches when `props.code` changes, and
 * renewing does not change which code is open. An operator would see "Dec 1"
 * before and after a renewal that had actually moved it to February.
 *
 * `hasUnsavedWork()` is the unsaved-changes guard.
 */
defineExpose({
  hasUnsavedWork: () => dirty.value,
  reload: () => refresh(),
})

async function save() {
  const r = row.value
  if (!r || saving.value) return
  saving.value = true
  try {
    const ok = await act(
      'codes.update',
      { code: r.code, label: label.value, middleman: middleman.value, notes: notes.value, tier: tier.value },
      'Saved',
    )
    if (ok) {
      emit('changed')
      await refresh()
    }
  } finally {
    saving.value = false
  }
}

function discard() {
  label.value = row.value?.label || ''
  middleman.value = row.value?.middleman || ''
  notes.value = row.value?.notes || ''
  tier.value = row.value?.tier || ''
}

// ── The term editor (codes.set-term) ──
//
// Kept separate from renew because the two answer different questions, and
// conflating them is how a student gets charged for the wrong span:
//
//   set-term  changes the PRODUCT (what one purchase buys, from now on)
//   renew     delivers a SALE  (adds a span to this code's expiry)
//
// Inline rather than window.prompt: a prompt cannot show the current expiry, the
// current term, or what the change will not touch, and it is unstyleable,
// unreadable and un-keyboard-navigable. The old panel used one for a field that
// writes paid time.
const editingTerm = ref(false)
const termInput = ref('')
const termBusy = ref(false)

function startTermEdit() {
  termInput.value = String(row.value?.term_days || 0)
  editingTerm.value = true
}

async function saveTerm() {
  const r = row.value
  if (!r || termBusy.value) return
  const days = Number(termInput.value.trim())
  if (!Number.isFinite(days) || days < 0) return
  termBusy.value = true
  try {
    const ok = await act(
      'codes.set-term',
      { code: r.code, term_days: days },
      `Term set to ${days > 0 ? days + 'd' : 'never expires'} — expiry unchanged`,
    )
    if (ok) {
      editingTerm.value = false
      emit('changed')
      await refresh()
    }
  } finally {
    termBusy.value = false
  }
}

// ── Manual expiry (codes.expire) ──
// The labelled manual OVERRIDE, next to Renew, so it is not mistaken for the
// normal path. Inline for the same reason as the term editor.
const editingExpiry = ref(false)
const expiryInput = ref('')
const expiryBusy = ref(false)

function startExpiryEdit() {
  expiryInput.value = row.value?.expires_at ? row.value.expires_at.slice(0, 10) : ''
  editingExpiry.value = true
}

async function saveExpiry() {
  const r = row.value
  if (!r || expiryBusy.value) return
  const v = expiryInput.value.trim()
  if (v && !/^\d{4}-\d{2}-\d{2}/.test(v)) return
  expiryBusy.value = true
  try {
    const ok = await act(
      'codes.expire',
      { code: r.code, expires_at: v ? `${v}T00:00:00.000Z` : '' },
      v ? `Expires ${v}` : 'Expiry cleared',
    )
    if (ok) {
      editingExpiry.value = false
      emit('changed')
      await refresh()
    }
  } finally {
    expiryBusy.value = false
  }
}

// ── Stop / reactivate ──
const toggling = ref(false)
async function toggleSuspend() {
  const r = row.value
  if (!r) return
  if (!r.suspended) {
    // A stop with a reason is shown to the student; the drawer owns that flow.
    emit('stop', r)
    return
  }
  toggling.value = true
  try {
    const ok = await act('codes.unsuspend', { code: r.code }, 'Reactivated')
    if (ok) {
      emit('changed')
      await refresh()
    }
  } finally {
    toggling.value = false
  }
}

// ── State: direct actions, no confirmation step ──
//
// R4/D3. These are corrections an operator makes while a code is being managed
// by hand — and there is no fixed workflow to protect them from. The previous
// release flow asked for a reason and, when a live device was about to be cut
// off, made the operator type the last four characters of the code. That typed
// step changed nothing on the server (the commit was guarded by a claim token,
// not by the typing), so it is gone; the release is one click.
//
// THE PAIR IS NOW A TRUE UNDO. `Release` clears the claim (and cuts off a live
// device); `Mark in term` puts the claim back and clears the eviction. So the
// operator can go in either direction without leaving the panel, which is what
// D3's "reversible by overriding back" means here. The old `Undo release`
// button is gone: it cleared only the eviction and left the code `released`,
// which made its label a promise it did not keep.
const stateBusy = ref(false)

async function releaseCode() {
  const r = row.value
  if (!r || stateBusy.value) return
  stateBusy.value = true
  try {
    const ok = await act('codes.release', { code: r.code }, 'Released — free for a new holder')
    if (ok) {
      emit('changed')
      await refresh()
    }
  } finally {
    stateBusy.value = false
  }
}

async function markInTerm() {
  const r = row.value
  if (!r || stateBusy.value) return
  stateBusy.value = true
  try {
    // One id per attempt so a retried click is a replay, not a second audit row.
    const id = `in-term-${r.code}-${Date.now().toString(36)}`
    const ok = await act(
      'codes.override',
      { code: r.code, to_state: 'in_term', override_id: id },
      'In term',
    )
    if (ok) {
      emit('changed')
      await refresh()
    }
  } finally {
    stateBusy.value = false
  }
}

/**
 * Whether "Mark in term" would change anything.
 *
 * Derived from the SAME label the badge renders, so the button can never appear
 * on a code that already reads "in term" (an inert control — I5) and can never
 * be missing from one that does not (a dead end — I2). A code that has never been
 * issued is excluded for a different reason: the student's own activation claims
 * it, and claiming it here would make that activation fail.
 */
const canMarkInTerm = computed(() => {
  const r = row.value
  if (!r) return false
  // The student's activation claims an unissued code; claiming it here would make
  // that activation fail ("already in use on another device").
  if (!r.first_seen_at && !r.activated_at) return false
  const label = stateLabel(r)
  if (label === 'in term') return false
  // A PLAIN suspension is undone by `Reactivate`, which is one control away in the
  // row above. Offering both would be two controls for one outcome, and a stopped
  // code's expiry is hidden behind its badge (`status` is "suspended"), so the
  // lapsed case has to be read from the date itself.
  const lapsed = !!r.expires_at && new Date(r.expires_at).getTime() < Date.now()
  if (label === 'stopped' && r.redeemed && !r.released_pending && !lapsed && r.force_state !== 'released') {
    return false
  }
  return true
})

// ── The session probe, run by hand ──
//
// R2. The probe endpoint existed and the client's Account card could run it, but
// the CONSOLE — the screen an operator is actually on when a student's state
// disagrees with theirs — had no way to invoke it. It quoted the device id in a
// collapsed "Record" fold and told the operator to use it elsewhere.
//
// The result is rendered in full here rather than logged: the two axes, the named
// verdict, the divergences, and any override the hub is holding. A failed run is
// shown as a failure with the hub's own reason, never as an empty success.
const probeBusy = ref(false)
const probeResult = ref<ProbeResponse | null>(null)
const probeError = ref('')
const probeRanAt = ref('')

// The checks the probe performs, as four named ones rather than a number chosen
// to look thorough: the machine gate, then the three things it reports.
const PROBE_CHECKS = 4

/** Flags the hub raised: its divergence list. `0` is a real, common answer. */
function findingsOf(result: ProbeResponse): number {
  return (result.divergence || []).length
}

function reportProbe(code: string, ranAt: string, result: ProbeResponse | null, error: string) {
  emit('probed', {
    code,
    ranAt,
    checks: PROBE_CHECKS,
    findings: result ? findingsOf(result) : 0,
    // The endpoint's contract (`session_probe.pb.js`): it writes NOTHING, not
    // even `last_seen_at`. So "changed" is not a guess — it is the design, and
    // showing 0 is the honest report of it.
    changed: 0,
    error,
  })
}

async function runProbe() {
  const r = row.value
  if (!r || probeBusy.value) return
  const device = (r.first_seen_fingerprint || '').trim()
  probeError.value = ''
  probeResult.value = null
  if (!device) {
    probeError.value =
      'The hub holds no device id for this code — it was never activated, or it predates the field. The probe needs the machine that first redeemed it.'
    probeRanAt.value = new Date().toISOString()
    reportProbe(r.code, probeRanAt.value, null, probeError.value)
    return
  }
  probeBusy.value = true
  try {
    const res = await runSessionProbe(r.code, device)
    probeRanAt.value = new Date().toISOString()
    if (res.ok && res.data) {
      probeResult.value = res.data
    } else {
      probeError.value = res.message || res.transportError || 'The probe did not answer'
    }
    reportProbe(r.code, probeRanAt.value, probeResult.value, probeError.value)
  } finally {
    probeBusy.value = false
  }
}

// ── Delete (codes.delete) ──
// In-panel confirmation rather than window.confirm: the confirm has to be able
// to show what the panel knows (a redeemed code cannot be deleted; mark it
// unused first) and to be readable.
const confirmDelete = ref(false)
const deleting = ref(false)

async function del() {
  const r = row.value
  if (!r || deleting.value || r.redeemed) return
  deleting.value = true
  try {
    const ok = await act('codes.delete', { code: r.code }, 'Deleted')
    if (ok) {
      emit('changed')
      emit('close')
    }
  } finally {
    deleting.value = false
  }
}

// ── Derived display ──
const remaining = computed(() => (row.value ? remainingText(row.value) : null))

/**
 * The one-line "is this healthy?" verdict.
 *
 * Written as a sentence rather than a colour because the failure modes differ in
 * KIND: a suspended code is a decision, a lapsed code is a missed renewal, and an
 * unservable plan is a configuration fault that breaks the student's tunnel
 * silently. A badge cannot say which.
 */
const verdict = computed(() => {
  const r = row.value
  if (!r) return { tone: '', text: '' }
  if (r.suspended) {
    return { tone: 'bad', text: 'Stopped by an operator. The student sees a message on their next check-in.' }
  }
  if (r.status === 'expired') {
    return { tone: 'warn', text: 'Term ended. Access is refused until this is renewed.' }
  }
  // BEFORE the `redeemed` arm, because an unclaimed code IS redeemed and would
  // otherwise be described as "In use" while its badge says "unclaimed" and the
  // next activation can take it. The badge and this sentence disagreed.
  if (r.redeemed && r.released_pending) {
    return {
      tone: 'warn',
      text: 'A sign-out is recorded and not claimed: the next activation takes this code. Mark it in term to put the claim back.',
    }
  }
  if (r.redeemed) {
    if (!r.expires_at) return { tone: 'good', text: 'In use, never expires.' }
    return { tone: 'good', text: `In use. ${remaining.value || ''} left in the term.` }
  }
  if (r.first_seen_at) {
    return {
      tone: 'warn',
      text: 'Was issued once and then released. Free to hand out, but handing it over ends the previous holder’s access.',
    }
  }
  return { tone: '', text: 'Minted, never used.' }
})

/** Plans offered in the editor: the hub's list, plus this code's own (it may be
 *  retired, and a `<select>` that cannot show the current value would silently
 *  rewrite it on save). */
const planOptions = computed(() => {
  const set = new Set<string>(props.tiers)
  for (const t of meta.tiers.value) set.add(t)
  if (row.value?.tier) set.add(row.value.tier)
  return Array.from(set).sort()
})
</script>

<template>
  <!-- A placeholder rather than nothing: the rail keeps its width so the table
       does not jump when a code is selected or cleared. -->
  <aside v-if="!code" class="inspector inspector-idle" aria-label="Code inspector">
    <div class="ins-empty">
      <p class="ins-empty-title">No code selected</p>
      <p class="ins-empty-hint">
        Pick a row to see it here. <span class="kbd">j</span>/<span class="kbd">k</span> moves,
        <span class="kbd">⏎</span> renews, <span class="kbd">esc</span> closes.
      </p>
    </div>
  </aside>

  <aside v-else class="inspector" aria-label="Code inspector">
    <div class="ins-head">
      <div class="ins-title">
        <span class="mono ins-code">{{ row?.code || code }}</span>
        <span v-if="row" class="badge" :class="stateClass(row)">{{ stateLabel(row) }}</span>
        <!-- Beside the status, never as it: the badge is the hub's derived status;
             this is the operator's decision. See model.ts:stateLabel. -->
        <span v-if="row && overrideNote(row)" class="override-note">{{ overrideNote(row) }}</span>
      </div>
      <div class="ins-sub">
        <span v-if="row?.label" class="ins-name">{{ row.label }}</span>
        <span v-else-if="row" class="muted">No name recorded</span>
        <span v-if="row?.tier" class="chip">{{ row.tier }}</span>
        <button class="row-menu" type="button" aria-label="Close inspector" @click="emit('close')">✕</button>
      </div>
    </div>

    <div class="ins-body">
      <div v-if="loading" class="state">Loading…</div>
      <div v-else-if="notFound" class="state err">
        <strong>The hub does not hold this code</strong>
        <p>{{ code }} is not in the codes collection. It was never minted here, or it was deleted.</p>
      </div>

      <template v-else-if="row">
        <!-- ── Verdict ── -->
        <div class="msg" :class="verdict.tone === 'bad' ? 'err' : verdict.tone === 'warn' ? 'warn' : 'ok'">
          {{ verdict.text }}
        </div>

        <!-- ── The facts that matter, at a glance ──
             EXPIRES and TERM are EDITABLE here, at the top level. They are not
             "overrides" behind a disclosure: they are primary, always-manual
             inputs, and the value an operator manages regularly must be one step
             from the panel they are already looking at. Last seen and Who sold it
             stay read-only facts beside them. -->
        <div class="ins-facts">
          <button type="button" class="ins-fact ins-fact-edit" title="Set the expiry date" @click="startExpiryEdit">
            <span class="k">Expires</span>
            <strong>{{ row.expires_at ? shortDate(row.expires_at) : 'Never' }}</strong>
            <span class="v">{{ remaining ? remaining + ' · set…' : 'set…' }}</span>
          </button>
          <button type="button" class="ins-fact ins-fact-edit" title="Set how long one purchase lasts" @click="startTermEdit">
            <span class="k">Term</span>
            <strong>{{ row.term_days ? row.term_days + 'd' : 'never' }}</strong>
            <span class="v">per purchase · set…</span>
          </button>
          <div class="ins-fact">
            <span class="k">Last seen</span>
            <strong>{{ livenessOf(row) === 'unknown' ? '—' : livenessText(row) }}</strong>
            <span v-if="livenessOf(row) === 'unknown'" class="v">never checked in</span>
          </div>
          <div class="ins-fact">
            <span class="k">Who sold it</span>
            <strong>{{ row.middleman || '—' }}</strong>
            <span v-if="row.release_count" class="v">{{ row.release_count }}× signed out</span>
          </div>
        </div>

        <!-- The editors open IN PLACE under the values they change. There is no
             "Manual overrides" disclosure any more: it hid two values that were
             never automated in the first place. -->
        <div v-if="editingExpiry" class="ins-edit">
          <label class="field">
            <span>Expires on <span class="muted">(blank = never expires)</span></span>
            <input v-model="expiryInput" type="date" @keyup.enter="saveExpiry" @keyup.esc="editingExpiry = false" />
          </label>
          <div class="actions">
            <button class="tiny primary" :disabled="expiryBusy" @click="saveExpiry">
              {{ expiryBusy ? 'Saving…' : 'Set expiry' }}
            </button>
            <button class="tiny" :disabled="expiryBusy" @click="editingExpiry = false">Cancel</button>
          </div>
        </div>
        <div v-if="editingTerm" class="ins-edit">
          <label class="field">
            <span>Days per purchase <span class="muted">(0 = never expires)</span></span>
            <input v-model="termInput" type="number" min="0" @keyup.enter="saveTerm" @keyup.esc="editingTerm = false" />
          </label>
          <p class="foot-note" style="margin-top: -4px">Changes the product, not this code's date. Use Renew to add paid time.</p>
          <div class="actions">
            <button class="tiny primary" :disabled="termBusy" @click="saveTerm">
              {{ termBusy ? 'Saving…' : 'Set term' }}
            </button>
            <button class="tiny" :disabled="termBusy" @click="editingTerm = false">Cancel</button>
          </div>
        </div>

        <!-- ── Actions, in frequency order ── -->
        <div class="ins-actions">
          <button class="primary" @click="emit('renew', row)">Renew…</button>
          <button :disabled="toggling" @click="toggleSuspend">
            {{ row.suspended ? 'Reactivate' : 'Stop access…' }}
          </button>
        </div>

        <!-- ── State, changed directly ──
             One click, no confirmation step. `Release` clears the claim (and cuts
             off a live device at its next check-in); `Mark in term` restores the
             claim, stops the cut-off, lifts a suspension and, on a lapsed code,
             extends the expiry by the code's own term. Between them the operator
             can move a code in either direction from this panel. -->
        <div v-if="(row.redeemed && !row.suspended) || canMarkInTerm" class="ins-actions ins-state-actions">
          <!-- Not while stopped: releasing leaves the suspension in place, so the
               badge would stay "stopped" and the button would be a promise it does
               not keep. `Reactivate` (above) is the first half of that move. -->
          <button v-if="row.redeemed && !row.suspended" class="tiny" :disabled="stateBusy" @click="releaseCode">Release</button>
          <button v-if="canMarkInTerm" class="tiny" :disabled="stateBusy" @click="markInTerm">Mark in term</button>
        </div>
        <p v-if="(row.redeemed && !row.suspended) || canMarkInTerm" class="foot-note" style="margin-top: 2px">
          <template v-if="row.redeemed">
            Release frees it for a different student and cuts off the device using it within about five minutes.
            The badge then reads “released”, not “unused”, because this code has been sold before.
          </template>
          <template v-else>
            Mark in term puts the claim back and stops cutting the device off.
            <template v-if="row.status === 'expired'"> It also extends the expiry by the code’s term.</template>
          </template>
        </p>

        <!-- ── The session probe, run by hand ──
             The result is rendered here, not logged: the two axes, the named
             verdict, the flags and any override, so "did it run, and what did it
             say?" is answerable without opening a log. -->
        <div class="section-title">Session probe</div>
        <div class="ins-probe">
          <button class="tiny" :disabled="probeBusy" @click="runProbe">
            {{ probeBusy ? 'Running…' : 'Run session probe' }}
          </button>
          <span v-if="probeRanAt" class="muted ins-probe-when">ran {{ shortDateTime(probeRanAt) }}</span>
        </div>
        <div v-if="probeError" class="msg err">
          <strong>The probe did not answer.</strong>
          <div>{{ probeError }}</div>
        </div>
        <div v-else-if="probeResult" class="ins-probe-result">
          <div class="msg" :class="probeResult.override ? 'warn' : 'ok'">
            <strong>Hub answered.</strong>
            May be used: {{ probeResult.entitlement }} · device: {{ probeResult.liveness }} · verdict:
            <code>{{ probeResult.reason_code }}</code>
          </div>
          <div v-if="probeResult.override" class="msg warn">
            <strong>Override active</strong> — {{ probeResult.override.force_state || 'evicted' }}<template
              v-if="probeResult.override.evicted"
            >: the device is cut off at its next check-in</template
            >.<span v-if="probeResult.override.evicted_reason"> Reason: {{ probeResult.override.evicted_reason }}</span>
          </div>
          <p class="foot-note" style="margin: 4px 0">
            <strong>{{ PROBE_CHECKS }} checks</strong> · {{ findingsOf(probeResult) }}
            {{ findingsOf(probeResult) === 1 ? 'finding' : 'findings' }} · 0 changes (the probe writes nothing) · no
            errors
          </p>
          <p class="foot-note" style="margin: 4px 0">
            Checked in:
            {{ probeResult.hub_view.last_seen_at ? shortDateTime(probeResult.hub_view.last_seen_at) : 'never' }}
            · lease ends {{ probeResult.lease_expires_at ? shortDateTime(probeResult.lease_expires_at) : '—' }}
            <template v-if="probeResult.divergence.length"> · flags: {{ probeResult.divergence.join(', ') }}</template>
          </p>
        </div>

        <!-- ── Who owns this: the editors ──
             The name field is a NAME. The hub stores no personal data, so this is
             the only link between a student and their code — without it a support
             call cannot be answered, and an Overview check exists precisely
             because it is often missing. -->
        <div class="section-title">Who owns this</div>
        <label class="field">
          <span>Name <span class="muted">(the label: how you find this student)</span></span>
          <input v-model="label" placeholder="e.g. Macleans Y13 — Tom R" @keyup.enter="dirty && save()" />
        </label>
        <div class="ins-pair">
          <label class="field">
            <span>Plan</span>
            <select v-model="tier">
              <option v-for="t in planOptions" :key="t" :value="t">{{ t }}</option>
            </select>
          </label>
          <label class="field">
            <span>Middleman</span>
            <input v-model="middleman" list="inspector-middlemen" placeholder="who sold it" @keyup.enter="dirty && save()" />
            <datalist id="inspector-middlemen">
              <option v-for="m in meta.middlemen.value" :key="m" :value="m" />
            </datalist>
          </label>
        </div>
        <label class="field">
          <span>Notes</span>
          <textarea v-model="notes" rows="2" placeholder="free-form" />
        </label>
        <div class="ins-save">
          <button class="primary" :disabled="!dirty || saving" @click="save">
            {{ saving ? 'Saving…' : dirty ? 'Save changes' : 'Saved' }}
          </button>
          <button v-if="dirty" class="tiny" :disabled="saving" @click="discard">Discard</button>
          <span v-if="dirty" class="muted ins-warn">Unsaved</span>
        </div>

        <!-- ── Irreversible, last of the ACTIONS ──
             Release left this section: it is now a direct state action above
             (`Release`), not a confirmed destructive flow. Delete is the one
             thing here that destroys the row. -->
        <div class="section-title">Irreversible</div>

        <div class="ins-danger">
          <template v-if="!confirmDelete">
            <div class="ins-row">
              <span class="ins-row-label">
                Delete permanently
                <small>Prefer stopping or releasing. An audit entry is kept but the code is gone — for typos, duplicate batches and test rows that were never issued.</small>
              </span>
              <!-- The hub refuses a code that has EVER been issued, so the control
                   is disabled on the same predicate. It used to check only the live
                   claim, which left an enabled button that could never work on a
                   released code (I5). -->
              <button
                class="tiny"
                :disabled="row.redeemed || !!row.first_seen_at"
                :title="row.redeemed ? 'Release it first' : row.first_seen_at ? 'This code has been issued, so its record cannot be deleted' : ''"
                @click="confirmDelete = true"
              >
                Delete…
              </button>
            </div>
          </template>
          <template v-else>
            <p class="ins-confirm">
              Permanently delete <strong class="mono">{{ row.code }}</strong
              ><template v-if="row.label"> ({{ row.label }})</template>? It cannot be re-activated.
            </p>
            <div class="actions">
              <button class="tiny danger" :disabled="deleting" @click="del">
                {{ deleting ? 'Deleting…' : 'Yes, delete it' }}
              </button>
              <button class="tiny" :disabled="deleting" @click="confirmDelete = false">Cancel</button>
            </div>
          </template>
        </div>

        <!-- ── History and Record, DEMOTED ──
             Both are support-call reading, not the work of managing a code, so
             they sit at the bottom of the rail as closed folds. They are not
             deleted: the count in the History summary says at a glance whether
             there is anything to open. -->
        <details class="ins-fold ins-fold-bottom">
          <summary class="section-title">
            History
            <span v-if="events.length" class="n">{{ events.length }}</span>
            <span class="ins-fold-hint">what happened to this code</span>
          </summary>
          <div v-if="historyLoading && !events.length" class="state tiny-state">Loading…</div>
          <div v-else-if="!events.length" class="state tiny-state">
            <p style="margin: 0">Nothing recorded. A code that has only been minted has no events.</p>
          </div>
          <ol v-else class="timeline">
            <li v-for="(h, i) in events" :key="i">
              <span class="when">{{ shortDateTime(h.created) }}</span>
              <span class="what">
                <strong>{{ h.event }}</strong>
                <span v-if="h.detail" class="detail">{{ h.detail }}</span>
              </span>
            </li>
          </ol>
        </details>

        <details class="ins-fold ins-fold-bottom">
          <summary class="section-title">
            Record
            <span class="ins-fold-hint">device id · first used · activation rule</span>
          </summary>
          <dl class="facts">
            <dt>First used</dt>
            <dd>{{ row.activated_at ? shortDateTime(row.activated_at) : 'Not used yet' }}</dd>
            <template v-if="row.first_seen_at">
              <dt>First issued</dt>
              <dd>
                {{ shortDate(row.first_seen_at) }}
                <span v-if="!row.redeemed" class="muted"> · released since</span>
              </dd>
            </template>
            <template v-if="row.first_seen_fingerprint">
              <dt>Device id</dt>
              <dd>
                <code>{{ row.first_seen_fingerprint }}</code>
                <div class="muted" style="font-size: 11.5px">Used by Run session probe above.</div>
              </dd>
            </template>
            <template v-if="row.redeemed">
              <dt>Next activation</dt>
              <dd v-if="row.released_pending">
                Will succeed — a Sign out was recorded {{ shortDateTime(row.released_at) }} and not claimed since
              </dd>
              <dd v-else>Refused on another device until this one signs out</dd>
            </template>
          </dl>
        </details>
      </template>
    </div>
  </aside>
</template>
