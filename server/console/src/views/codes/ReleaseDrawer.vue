<script setup lang="ts">
// Release a code — the "this code is free, let it go" action.
//
// WHY THIS IS A SEPARATE DRAWER FROM StopDrawer. Stopping access keeps the
// student's claim: the code is still theirs, they are just cut off, and undoing
// it needs no card. Releasing clears the claim so the code can be sold again —
// a different and much larger consequence. Two different blast radii, so two
// different confirmations.
//
// ── WHAT MAKES THIS CONFIRMATION SPECIFIC ──
//
// The consequences list is NOT prose written here. It comes from
// `codes.release { dry_run: true }`, which computes it from the live row — so
// what the operator reads is what the handler will actually do, including
// whether a device is about to be evicted. When it will evict a live client, the
// operator must type the last four characters of the code: it names the code, the
// transition and the side effects rather than asking "are you sure?".
//
// ── WHY IT WARNS INSTEAD OF REFUSING ──
//
// Releasing a code a device is still using is a legitimate operator decision (the
// card was double-sold, the student was refunded, the machine was replaced). The
// hub cannot know "in use" — `last_seen_at` proves the device REACHED the hub, not
// that a student is at it — so refusing would be refusing on a guess. The override
// wins, loudly, and the audit row records the observation it was made on.

import { computed, ref, watch } from 'vue'
import Drawer from '../../components/Drawer.vue'
import type { CodeRow } from './model'
import { call } from '../../api'
import { toast } from '../../toast'
import { act } from './data'

const props = defineProps<{ row: CodeRow | null }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'done'): void }>()

const reason = ref('')
const typed = ref('')
const busy = ref(false)
const previewing = ref(false)
const consequences = ref<string[]>([])
const severity = ref<'evicts-live-client' | 'release-only' | ''>('')
// The claim the preview was computed against. Sent on commit so the hub can refuse
// to act on a decision made about a state that has since changed.
const claimToken = ref<string>('')

// One id per attempt, so a retry after a timeout is a replay rather than a
// second release. Minted when the drawer opens, reused until it succeeds.
let overrideId = ''

function mintId(): string {
  // Not cryptographic: it only has to be unique against this code's history.
  return `rel-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`
}

async function preview() {
  if (!props.row) return
  previewing.value = true
  consequences.value = []
  severity.value = ''
  try {
    const res = await call<{ consequences: string[]; severity: string; claim_token: string }>('codes.release', {
      code: props.row.code,
      reason: reason.value.trim() || 'Released from console',
      override_id: overrideId,
      dry_run: true,
    })
    if (res.ok && res.data) {
      consequences.value = res.data.consequences || []
      severity.value = (res.data.severity as typeof severity.value) || ''
      claimToken.value = res.data.claim_token || ''
    } else {
      consequences.value = [res.message || res.transportError || 'Could not preview this change.']
    }
  } finally {
    previewing.value = false
  }
}

watch(
  () => props.row?.code,
  (code) => {
    reason.value = ''
    typed.value = ''
    consequences.value = []
    severity.value = ''
    overrideId = mintId()
    if (code) void preview()
  },
)

const tooLong = computed(() => reason.value.trim().length > 200)
// The typed confirmation is required ONLY when a live client is about to be cut
// off. Requiring it for every release would put friction on the common case and
// teach the operator to type without looking, which is worse than not asking.
const typeNeeded = computed(() => severity.value === 'evicts-live-client')
const tail = computed(() => (props.row?.code || '').slice(-4))
const typedOk = computed(() => !typeNeeded.value || typed.value.trim().toUpperCase() === tail.value.toUpperCase())
const canGo = computed(() => !busy.value && !tooLong.value && !previewing.value && typedOk.value)

async function release() {
  if (!canGo.value || !props.row) return
  busy.value = true
  try {
    const res = await call<{ evicted: boolean; replay?: boolean; already_released?: boolean }>('codes.release', {
      code: props.row.code,
      reason: reason.value.trim() || 'Released from console',
      override_id: overrideId,
      claim_token: claimToken.value,
      dry_run: false,
    })
    if (!res.ok || !res.data) {
      // 409 means the row moved between preview and confirm: the consequences the
      // operator read no longer describe this code. Re-preview rather than pretend
      // it failed, so they see what they are now confirming.
      if (/changed while you were confirming/i.test(res.message || '')) {
        toast.err(res.message || 'This code changed — review the new consequences.')
        await preview()
        return
      }
      toast.err(res.message || res.transportError || 'Could not release the code')
      return
    }
    if (res.data.already_released || res.data.replay) {
      // Truthful no-op, not a failure: the code is already as asked.
      toast.ok('Already released — nothing changed')
      emit('done')
      return
    }
    toast.ok(res.data.evicted ? 'Released — the device is cut off at its next check-in' : 'Released')
    overrideId = mintId()
    emit('done')
  } finally {
    busy.value = false
  }
}

async function undo() {
  if (!props.row) return
  busy.value = true
  try {
    const ok = await act(
      'codes.reactivate',
      { code: props.row.code, reason: 'Released in error' },
      'Override undone',
    )
    if (ok) emit('done')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Drawer :open="!!row" :title="`Release ${row?.code || ''}`" :busy="busy || previewing" @close="emit('close')">
    <template v-if="row">
      <p class="foot-note" style="margin-top: 0">
        This clears the single-use claim so the code can be activated by a
        <strong>different student</strong>. Its tier and expiry are unchanged.
        To cut one student off while keeping their claim, use
        <strong>Stop access</strong> instead.
      </p>

      <div class="override-preview" :class="severity === 'evicts-live-client' ? 'is-danger' : ''">
        <div class="preview-head">
          <strong>What will happen</strong>
          <span v-if="previewing" class="muted">Checking…</span>
        </div>
        <ul>
          <li v-for="(c, i) in consequences" :key="i">{{ c }}</li>
        </ul>
      </div>

      <label class="field">
        <span>Reason (shown to the student if they are cut off)</span>
        <textarea v-model="reason" placeholder="Write it for the student, not for us." @blur="preview" />
      </label>
      <p class="foot-note" style="margin-top: -6px" :style="tooLong ? 'color: var(--warn)' : ''">
        {{ reason.trim().length }}/200 characters.
      </p>

      <label v-if="typeNeeded" class="field">
        <span>Type the last four characters of the code (<code>{{ tail }}</code>) to confirm</span>
        <input v-model="typed" placeholder="Last four characters" autocomplete="off" spellcheck="false" />
      </label>

      <button v-if="row.force_state === 'released'" class="tiny" :disabled="busy" @click="undo">
        Undo: this code is released by override
      </button>
    </template>

    <template #foot>
      <button :disabled="busy" @click="emit('close')">Cancel</button>
      <button class="danger" :disabled="!canGo" @click="release">
        {{ busy ? 'Releasing…' : typeNeeded ? 'Release and cut off the device' : 'Release' }}
      </button>
    </template>
  </Drawer>
</template>

<style scoped>
/* Small and local on purpose: the console has no shared panel component for
   this, and inventing one for a single drawer would be a framework.
   Every colour comes from a --variable (COLOURS.md owns the values); a literal
   hex here is a §49 failure and a second place to change a brand colour. */
.override-preview {
  margin: 14px 0;
  padding: 10px 12px;
  border-left: 3px solid var(--muted);
  background: var(--panel-2);
}
.override-preview.is-danger {
  border-left-color: var(--warn);
}
.preview-head {
  display: flex;
  gap: 8px;
  justify-content: space-between;
  margin-bottom: 6px;
}
.override-preview ul {
  margin: 0;
  padding-left: 18px;
}
.override-preview li {
  margin: 3px 0;
  font-size: 0.92em;
}
</style>
