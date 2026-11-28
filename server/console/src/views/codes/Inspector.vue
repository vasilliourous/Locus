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
//   the facts  — term, expiry, last seen, next activation. The four that matter.
//   who        — the name/plan/middleman EDITORS (the label is a name here).
//   actions    — in frequency order, never alphabetical.
//   history    — inline, always loaded with the code, never behind a tab.
//   danger     — release and delete, last, separated, and confirmed in the panel
//                rather than by window.confirm.
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
import type { CodeRow, HistoryEvent } from './model'
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
  (e: 'release', row: CodeRow): void
}>()

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

// ── Release (codes.unbind) ──
// Destroys a real relationship, so it asks WHY in the same breath. The hook has
// always accepted `reason` and written it into the audit trail; a panel that
// called it without one would leave every future release unexplained — the one
// thing the trail exists for.
// ── Release ──
// THE FLOW LIVES IN ReleaseDrawer NOW, and this panel only opens it.
//
// It used to run `codes.unbind` inline with a two-step confirm. That was correct
// while releasing was a quiet metadata change. It is not quiet any more: a release
// of a code a device is still using now evicts that device, and the confirmation
// has to show the consequences and (when evicting) require the code to be typed.
// Keeping the inline version would mean two release paths whose confirmations
// could drift — one of them the weaker.
function release() {
  const r = row.value
  if (!r) return
  emit('release', r)
}

// ── Delete (codes.delete) ──
// In-panel confirmation rather than window.confirm: the confirm has to be able
// to show what the panel knows (a redeemed code cannot be deleted; release
// first) and to be readable.
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

        <!-- ── The four facts that matter, at a glance ── -->
        <div class="ins-facts">
          <div class="ins-fact">
            <span class="k">Expires</span>
            <strong>{{ row.expires_at ? shortDate(row.expires_at) : 'Never' }}</strong>
            <span v-if="remaining" class="v">{{ remaining }}</span>
          </div>
          <div class="ins-fact">
            <span class="k">Term</span>
            <strong>{{ row.term_days ? row.term_days + 'd' : 'never' }}</strong>
            <span class="v">from activation</span>
          </div>
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

        <!-- ── Actions, in frequency order ── -->
        <div class="ins-actions">
          <button class="primary" @click="emit('renew', row)">Renew…</button>
          <button :disabled="toggling" @click="toggleSuspend">
            {{ row.suspended ? 'Reactivate' : 'Stop access…' }}
          </button>
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

        <!-- ── Manual overrides ──
             Collapsed by default. These are the RARE corrections: set-term changes
             the product (not a sale), and set-expiry moves a date without any money
             changing hands. Without a mouse, an operator scrolling this rail to
             reach "Renew" had to pass through both every time — the rail measured
             1.87 screens, and the History it was heading for sat at the bottom.
             Closed, both compress to one line each; open, they are exactly what
             they were. -->
        <details class="ins-fold">
          <summary class="section-title">
            Manual overrides
            <span class="ins-fold-hint">term · expiry — not for a sale</span>
          </summary>

        <div v-if="!editingTerm" class="ins-row">
          <span class="ins-row-label">
            Term
            <small>How long one purchase lasts, from activation. Changing it does not move the current expiry.</small>
          </span>
          <button class="tiny" @click="startTermEdit">Change</button>
        </div>
        <div v-else class="ins-edit">
          <label class="field">
            <span>Days per purchase <span class="muted">(0 = never expires)</span></span>
            <input v-model="termInput" type="number" min="0" @keyup.enter="saveTerm" @keyup.esc="editingTerm = false" />
          </label>
          <div class="actions">
            <button class="tiny primary" :disabled="termBusy" @click="saveTerm">
              {{ termBusy ? 'Saving…' : 'Set term' }}
            </button>
            <button class="tiny" :disabled="termBusy" @click="editingTerm = false">Cancel</button>
          </div>
        </div>

        <div v-if="!editingExpiry" class="ins-row">
          <span class="ins-row-label">
            Expiry
            <small>The manual override. To add paid time, use Renew — it previews the new date and keeps what is left of the term.</small>
          </span>
          <button class="tiny" @click="startExpiryEdit">Set</button>
        </div>
        <div v-else class="ins-edit">
          <label class="field">
            <span>Expires on <span class="muted">(blank = never)</span></span>
            <input v-model="expiryInput" type="date" @keyup.enter="saveExpiry" @keyup.esc="editingExpiry = false" />
          </label>
          <div class="actions">
            <button class="tiny primary" :disabled="expiryBusy" @click="saveExpiry">
              {{ expiryBusy ? 'Saving…' : 'Set expiry' }}
            </button>
            <button class="tiny" :disabled="expiryBusy" @click="editingExpiry = false">Cancel</button>
          </div>
        </div>
        </details>

        <!-- ── The full record ──
             Also folded: the device id, "first issued", the activation rule. Read
             once, on a support call — not on every renewal. -->
        <details class="ins-fold">
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
              <div class="muted" style="font-size: 11.5px">
                Quote this to run the session probe. It is not a sign-in credential.
              </div>
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

        <!-- ── History, inline ──
             Not a tab. This is what you read while a student is on the phone. -->
        <div class="section-title">
          History
          <span v-if="events.length" class="n">{{ events.length }}</span>
        </div>
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

        <!-- ── Danger, last ── -->
        <div class="section-title">Irreversible</div>

        <div v-if="row.redeemed" class="ins-danger">
          <div class="ins-row">
            <span class="ins-row-label">
              Release to a different student
              <small>Clears “in use” so someone else can activate it. Ends the current holder's access. Not needed to move a student to a new computer — they re-enter their own code.</small>
            </span>
            <button class="tiny" @click="release">Release…</button>
          </div>
        </div>

        <div class="ins-danger">
          <template v-if="!confirmDelete">
            <div class="ins-row">
              <span class="ins-row-label">
                Delete permanently
                <small>Prefer stopping or releasing. An audit entry is kept but the code is gone — for typos, duplicate batches and test rows.</small>
              </span>
              <button class="tiny" :disabled="row.redeemed" :title="row.redeemed ? 'Release it first' : ''" @click="confirmDelete = true">
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
      </template>
    </div>
  </aside>
</template>
