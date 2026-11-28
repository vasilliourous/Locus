<script setup lang="ts">
// A small yes/no sheet, for the ONE decision that has to interrupt.
//
// WHY NOT window.confirm. Everywhere else in this console, confirmation happens
// IN PLACE (the inspector's delete row, the release row) so the operator can see
// what they are confirming. This one is different: it guards a NAVIGATION, so the
// thing being confirmed is "you are about to leave with unsaved edits" and the
// answer must stop a route change that is already in flight. There is no panel to
// put it in.
//
// It is still not window.confirm. A native confirm is unstylable, renders in the
// browser's chrome (so it can look like a phishing prompt), ignores the app's
// keyboard model, and blocks the whole tab. This is a focus-trapped sheet with an
// explicit destructive label, and it composes with Escape.

import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps<{
  open: boolean
  title: string
  body?: string
  /** The label of the action that DISCARDS the work. */
  confirmLabel: string
  cancelLabel?: string
}>()

const emit = defineEmits<{ (e: 'confirm'): void; (e: 'cancel'): void }>()

const confirmBtn = ref<HTMLButtonElement | null>(null)

function onKey(e: KeyboardEvent) {
  if (!props.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('cancel')
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (!isOpen) return
    await nextTick()
    // Focus the SAFE action. A destructive confirm that grabs focus is a
    // confirm someone presses with the Enter they were already typing.
    confirmBtn.value?.blur()
    document.getElementById('leave-cancel')?.focus()
  },
)

onMounted(() => document.addEventListener('keydown', onKey))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <template v-if="open">
      <div class="scrim" @click="emit('cancel')" />
      <div class="confirm-sheet" role="alertdialog" aria-modal="true" :aria-label="title">
        <h2>{{ title }}</h2>
        <p v-if="body" class="muted">{{ body }}</p>
        <div class="actions">
          <button id="leave-cancel" @click="emit('cancel')">{{ cancelLabel || 'Keep editing' }}</button>
          <button ref="confirmBtn" class="danger" @click="emit('confirm')">{{ confirmLabel }}</button>
        </div>
      </div>
    </template>
  </Teleport>
</template>
