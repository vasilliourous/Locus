<script setup lang="ts">
// One code, in full.
//
// WHAT THIS REPLACES. The old detail was a centred modal containing: four
// inputs, seven buttons of equal weight, a paragraph of explanation, and a
// history TABLE nested inside it. Everything the operator could do to a code was
// present at once, so nothing was discoverable as the next step and the panel
// had to scroll to show its own history.
//
// FOUR TABS, BY QUESTION:
//   Detail  — who is this and is it working? (read, plus the two identity fields
//             an operator actually edits)
//   Actions — what do I want to do to it? (renew first, because that is the
//             routine; everything destructive last and separated)
//   History — what happened? (the audit trail, now long enough to deserve a tab)
//   Danger  — what must I be careful about? (expiry override, release, delete)

import { computed, ref, watch } from 'vue'
import SlideOver from '../../components/SlideOver.vue'
import type { CodeRow, HistoryEvent } from './model'
import { remainingText, shortDate, shortDateTime, stateClass, stateLabel } from './model'
import { act } from './data'

const props = defineProps<{ row: CodeRow | null; history: HistoryEvent[]; historyLoading: boolean }>()
const emit = defineEmits<{
  (e: 'close'): void
  (e: 'changed'): void
  (e: 'renew'): void
  (e: 'stop'): void
}>()

const tab = ref<'detail' | 'actions' | 'history' | 'danger'>('detail')

// Identity edits. Held locally so a half-typed label is not written on every
// keystroke, and reset whenever the panel shows a different code.
const label = ref('')
const middleman = ref('')
const notes = ref('')
const savingDetails = ref(false)

watch(
  () => props.row?.code,
  () => {
    tab.value = 'detail'
    label.value = props.row?.label || ''
    middleman.value = props.row?.middleman || ''
    notes.value = props.row?.notes || ''
    releaseReason.value = ''
    releasing.value = false
    settingTerm.value = false
  },
  { immediate: true },
)

// Release destroys a real relationship, so it asks WHY in the same breath.
// The hook has always accepted `reason` and written it into the audit trail;
// the redesigned panel initially called the action without it, which would have
// left every future release unexplained — the one thing the trail exists for.
const releaseReason = ref('')
const releasing = ref(false)

async function release() {
  const row = props.row
  if (!row || releasing.value) return
  // Guard the double-click: the hook refuses a second release ("That code has
  // not been used yet"), and an operator who sees that error has no way to tell
  // it from a real failure.
  releasing.value = true
  try {
    const why = releaseReason.value.trim()
    const ok = await act(
      'codes.unbind',
      { code: row.code, reason: why || 'released from the console' },
      'Released — the code is available again',
    )
    if (ok) emit('changed')
  } finally {
    releasing.value = false
  }
}

// codes.set-term: how long ONE purchase lasts, from activation.
//
// Kept separate from renew because the two are answers to different questions
// and conflating them is how a student gets charged for the wrong span:
//
//   set-term  changes the PRODUCT (what a purchase buys, from now on)
//   renew     delivers a SALE  (adds a span to this code's expiry)
//
// docs/business/09-console.md lists it among "the three that run the business",
// and the redesign dropped its UI — so it is restored here rather than left
// reachable only by curl.
const settingTerm = ref(false)

async function setTerm() {
  const row = props.row
  if (!row) return
  const current = row.term_days ? String(row.term_days) : '0'
  const input = window.prompt(
    `Term for ${row.code}\n\n` +
      `How many days does ONE purchase last, measured from activation?\n` +
      `70 is about a 10-week school term; 0 means the code never expires.\n\n` +
      `This does NOT move the current expiry (${row.expires_at ? row.expires_at.slice(0, 10) : 'none'}).\n` +
      `To add paid time, use Renew instead.`,
    current,
  )
  if (input === null) return
  const days = Number(input.trim())
  if (!Number.isFinite(days) || days < 0) {
    window.alert('Enter a number of days (0 for never expires).')
    return
  }
  settingTerm.value = true
  try {
    const ok = await act('codes.set-term', { code: row.code, term_days: days }, `Term set to ${days > 0 ? days + 'd' : 'never expires'} — expiry unchanged`)
    if (ok) emit('changed')
  } finally {
    settingTerm.value = false
  }
}

const dirty = computed(
  () =>
    props.row !== null &&
    (label.value !== (props.row.label || '') ||
      middleman.value !== (props.row.middleman || '') ||
      notes.value !== (props.row.notes || '')),
)

async function saveDetails() {
  const row = props.row
  if (!row) return
  savingDetails.value = true
  try {
    const ok = await act(
      'codes.update',
      { code: row.code, label: label.value, middleman: middleman.value, notes: notes.value },
      'Saved',
    )
    if (ok) emit('changed')
  } finally {
    savingDetails.value = false
  }
}

const remaining = computed(() => (props.row ? remainingText(props.row) : null))

/**
 * The one-line "is this healthy?" verdict.
 *
 * Written as a sentence rather than a colour because the failure modes differ in
 * kind: a suspended code is a decision, a lapsed code is a missed renewal, and an
 * unservable tier is a configuration fault that will break the student's tunnel
 * silently. A badge cannot say which.
 */
const verdict = computed(() => {
  const r = props.row
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
    return { tone: 'warn', text: 'Was issued once and then released. Free to hand out, but handing it over ends the previous holder’s access.' }
  }
  return { tone: '', text: 'Minted, never used.' }
})
</script>

<template>
  <SlideOver :open="!!row" wide :title="row?.code" @close="emit('close')">
    <template #head>
      <div v-if="row" style="display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap">
        <h2 class="so-title">{{ row.code }}</h2>
        <span class="badge" :class="stateClass(row)">{{ stateLabel(row) }}</span>
        <span v-if="row.tier" class="chip">{{ row.tier }}</span>
      </div>
      <p v-if="row" class="page-sub" style="margin-bottom: 0">
        {{ row.label || 'No name recorded' }}
        <template v-if="row.middleman"> · sold via {{ row.middleman }}</template>
      </p>
    </template>

    <template v-if="row">
      <div class="msg" :class="verdict.tone === 'bad' ? 'err' : verdict.tone === 'warn' ? 'warn' : 'ok'">
        {{ verdict.text }}
      </div>

      <div class="subtabs">
        <button :class="{ on: tab === 'detail' }" @click="tab = 'detail'">Detail</button>
        <button :class="{ on: tab === 'actions' }" @click="tab = 'actions'">Actions</button>
        <button :class="{ on: tab === 'history' }" @click="tab = 'history'">
          History<span v-if="history.length" class="muted"> · {{ history.length }}</span>
        </button>
        <button :class="{ on: tab === 'danger' }" @click="tab = 'danger'">Danger</button>
      </div>

      <!-- ── Detail ── -->
      <div v-if="tab === 'detail'">
        <dl class="facts">
          <dt>State</dt>
          <dd>{{ stateLabel(row) }}</dd>
          <dt>Tier</dt>
          <dd>{{ row.tier || '—' }}</dd>
          <dt>Term</dt>
          <dd>
            <template v-if="row.term_days">{{ row.term_days }} days from activation</template>
            <template v-else>Never expires</template>
          </dd>
          <dt>Expires</dt>
          <dd>
            {{ row.expires_at ? shortDate(row.expires_at) : 'Never' }}
            <span v-if="remaining" class="muted"> · {{ remaining }}</span>
          </dd>
          <dt>First used</dt>
          <dd>{{ row.activated_at ? shortDateTime(row.activated_at) : 'Not used yet' }}</dd>
          <template v-if="row.first_seen_at">
            <dt>First issued</dt>
            <dd>
              {{ shortDate(row.first_seen_at) }}
              <span v-if="!row.redeemed" class="muted"> · released since</span>
            </dd>
          </template>
          <template v-if="row.release_count">
            <dt>Sign-outs</dt>
            <dd>
              {{ row.release_count }} time(s)
              <span v-if="row.released_at" class="muted"> · last {{ shortDate(row.released_at) }}</span>
            </dd>
          </template>
        </dl>

        <div class="section-title">Who owns this</div>
        <p class="foot-note" style="margin-top: 0; margin-bottom: 12px">
          The hub stores no personal data, so the name here is the only link between
          a student and their code. Without it, a support call cannot be answered.
        </p>
        <label class="field">
          <span>Name (label)</span>
          <input v-model="label" placeholder="e.g. Macleans Y13 — Tom R" />
        </label>
        <label class="field">
          <span>Middleman</span>
          <input v-model="middleman" placeholder="who sold it" />
        </label>
        <label class="field">
          <span>Notes</span>
          <textarea v-model="notes" placeholder="free-form" />
        </label>
        <button class="primary" :disabled="!dirty || savingDetails" @click="saveDetails">
          {{ savingDetails ? 'Saving…' : dirty ? 'Save details' : 'Saved' }}
        </button>
      </div>

      <!-- ── Actions ── -->
      <div v-else-if="tab === 'actions'">
        <div class="section-title">The routine</div>
        <p class="foot-note" style="margin-top: 0">
          Renewing adds the purchased term to whichever is later: today, or the
          current expiry. Paying early never loses days.
        </p>
        <div class="actions" style="margin-top: 12px">
          <button class="primary" @click="emit('renew')">Renew this code…</button>
        </div>

        <div class="section-title">The term itself</div>
        <p class="foot-note" style="margin-top: 0">
          The term is how long ONE purchase lasts, counted from activation. Changing
          it does not move the expiry already running — renewals from now on add the
          new number. Use <strong>Renew</strong> to add paid time.
        </p>
        <div class="actions" style="margin-top: 12px">
          <button :disabled="settingTerm" @click="setTerm">
            {{ settingTerm ? 'Saving…' : `Change term (currently ${row.term_days ? row.term_days + 'd' : 'never expires'})` }}
          </button>
        </div>

        <div class="section-title">Stop access</div>
        <p class="foot-note" style="margin-top: 0">
          Both stop the tunnel at the student's next check-in (within about five
          minutes). A stop with a reason is shown to them on their sign-out screen;
          a quiet stop is not. Neither deletes anything — reactivating restores access
          with no re-entry.
        </p>
        <div class="actions" style="margin-top: 12px">
          <button v-if="!row.suspended" @click="emit('stop')">Stop access…</button>
          <button v-else class="primary" @click="act('codes.unsuspend', { code: row.code }, 'Reactivated').then(() => emit('changed'))">
            Reactivate
          </button>
        </div>
      </div>

      <!-- ── History ── -->
      <div v-else-if="tab === 'history'">
        <p class="foot-note" style="margin-top: 0">
          Everything an operator or a student has done to this code, newest first.
          Entries survive the code itself.
        </p>
        <div v-if="historyLoading" class="state">Loading…</div>
        <div v-else-if="!history.length" class="state">
          <strong>No events recorded</strong>
          <p>A code that has only been minted, never used or changed, has nothing to show.</p>
        </div>
        <table v-else class="data" style="margin-top: 12px">
          <tbody>
            <tr v-for="(h, i) in history" :key="i">
              <td class="muted nowrap" style="width: 1px">{{ shortDateTime(h.created) }}</td>
              <td>
                <strong>{{ h.event }}</strong>
                <div class="muted" style="font-size: 12px">{{ h.detail }}</div>
              </td>
              <td class="muted nowrap" style="width: 1px">{{ h.actor || '' }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- ── Danger ── -->
      <div v-else>
        <div class="section-title">Release to a different student</div>
        <p class="foot-note" style="margin-top: 0">
          Clears the “in use” stamp so the code can be activated fresh by someone
          else. This ends the current holder's access and cannot be undone. You do
          <strong>not</strong> need it to move a student to a new computer — they
          re-enter their own code and it just works.
        </p>
        <template v-if="row.redeemed">
          <label class="field">
            <span>Reason <span class="muted">(recorded in this code's history)</span></span>
            <input v-model="releaseReason" placeholder="e.g. Re-issued to a different student" />
          </label>
          <button
            :disabled="releasing"
            @click="release"
          >
            {{ releasing ? 'Releasing…' : 'Release this code' }}
          </button>
          <p class="foot-note">
            The reason is what a later reader has to explain why a code changed
            hands. Without it the history says <em>that</em> it happened and
            nothing about why.
          </p>
        </template>
        <span v-else class="muted" style="font-size: 12.5px">Not in use, so there is nothing to release.</span>

        <div class="section-title">Delete</div>
        <p class="foot-note" style="margin-top: 0">
          Removes the code from the hub permanently. An audit entry is kept, but the
          code itself is gone and cannot be re-activated. Prefer stopping or
          releasing — deletion is for typos, duplicate batches and test rows.
        </p>
        <div class="actions" style="margin-top: 12px">
          <button
            class="danger"
            :disabled="row.redeemed"
            :title="row.redeemed ? 'Release it first' : ''"
            @click="act('codes.delete', { code: row.code }, 'Deleted').then(() => { emit('changed'); emit('close') })"
          >
            Delete permanently
          </button>
        </div>
      </div>
    </template>
  </SlideOver>
</template>
