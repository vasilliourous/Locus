<script setup lang="ts">
// Stop access to a code — with or without telling the student why.
//
// WHAT THIS MERGES. The console had two buttons for one effect:
//
//   codes.kick    — requires a reason, shown verbatim on the student's sign-out
//                   screen.
//   codes.suspend — the same cut-off, no message.
//
// An operator had to know which hook action mapped to which button, and the two
// were labelled "Kick out" and "Suspend" side by side in the same row — names for
// the same act, differing only in whether a sentence is displayed. That is a
// choice about TONE, not about effect, so it is a checkbox here and the hook
// actions stay exactly as they are.
//
// DEFAULTS TO TELLING THEM WHY. An unexplained suspension is a support call to
// whoever sold the code, asking about a decision nobody wrote down
// (admin_console.pb.js, codes.kick). The quiet path exists for a bounce that
// needs no explanation, and it is one click away — but it is not the default.

import { computed, ref, watch } from 'vue'
import Drawer from '../../components/Drawer.vue'
import type { CodeRow } from './model'
import { act } from './data'

const props = defineProps<{ row: CodeRow | null }>()
const emit = defineEmits<{ (e: 'close'): void; (e: 'done'): void }>()

const withReason = ref(true)
const reason = ref('')
const busy = ref(false)

watch(
  () => props.row?.code,
  () => {
    withReason.value = true
    reason.value = ''
  },
)

const tooLong = computed(() => reason.value.trim().length > 200)
const canStop = computed(() => !busy.value && (!withReason.value || (!!reason.value.trim() && !tooLong.value)))

// Examples written FOR the student, because the text is displayed to them.
// These are the three reasons this has actually been used for.
const EXAMPLES = ['Payment not received', 'This key is in use on several machines', 'Chargeback / refund issued']

async function stop() {
  if (!canStop.value) return
  busy.value = true
  try {
    const ok = withReason.value
      ? await act('codes.kick', { code: props.row!.code, reason: reason.value.trim() }, 'Stopped — cut off at the next check-in')
      : await act('codes.suspend', { code: props.row!.code }, 'Stopped')
    if (ok) emit('done')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <Drawer :open="!!row" :title="`Stop access to ${row?.code || ''}`" :busy="busy" @close="emit('close')">
    <template v-if="row">
      <p class="foot-note" style="margin-top: 0">
        The tunnel stops at the student's next check-in — within about five minutes
        while they are connected. Do not press it twice.
        <strong>The code stays on their computer</strong>: reactivating restores
        access with no re-entry and no restart.
      </p>

      <label class="field" style="display: flex; align-items: center; gap: 8px; margin-top: 16px">
        <input v-model="withReason" type="checkbox" style="width: auto" />
        <span style="margin: 0">Tell them why (shown on their sign-out screen)</span>
      </label>

      <template v-if="withReason">
        <label class="field">
          <span>Reason they will read</span>
          <textarea v-model="reason" placeholder="Write it for the student, not for us." />
        </label>
        <p class="foot-note" style="margin-top: -6px" :style="tooLong ? 'color: var(--warn)' : ''">
          {{ reason.trim().length }}/200 characters. Keep it short and factual — it is
          displayed in full.
        </p>
        <div class="actions" style="margin-top: 0">
          <button v-for="e in EXAMPLES" :key="e" class="tiny" @click="reason = e">{{ e }}</button>
        </div>
      </template>
      <p v-else class="foot-note">
        No message is shown. Use this for a bounce that needs no explanation; use
        the reason above when the student should understand what happened.
      </p>
    </template>

    <template #foot>
      <button :disabled="busy" @click="emit('close')">Cancel</button>
      <button class="danger" :disabled="!canStop" @click="stop">
        {{ busy ? 'Stopping…' : 'Stop access' }}
      </button>
    </template>
  </Drawer>
</template>
