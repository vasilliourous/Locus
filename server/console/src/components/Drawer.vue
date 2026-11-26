<script setup lang="ts">
// The flow drawer: create codes, import, renew.
//
// Distinct from the slide-over ON PURPOSE. A slide-over is a VIEW of something
// that already exists (a code, its history). A drawer is a FLOW — it has a
// beginning, a set of fields, and a commit button, and it must not look like
// the panel the operator reads. The difference in the DOM is small (a footer
// with the commit action right-aligned) and in the interface it is the
// difference between "here is this code" and "here is a form that will do
// something".
//
// Escape closes, except while a commit is in flight — closing mid-request would
// leave the operator with no way to see what happened.

import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

const props = defineProps<{ open: boolean; title: string; subtitle?: string; busy?: boolean }>()
const emit = defineEmits<{ (e: 'close'): void }>()

const panel = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null

function onKey(e: KeyboardEvent) {
  if (!props.open) return
  if (e.key === 'Escape' && !props.busy) {
    e.preventDefault()
    emit('close')
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      returnFocus = document.activeElement as HTMLElement | null
      await nextTick()
      const first = panel.value?.querySelector<HTMLElement>(
        'input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled])',
      )
      first?.focus()
    } else {
      returnFocus?.focus?.()
      returnFocus = null
    }
  },
)

onMounted(() => document.addEventListener('keydown', onKey))
onBeforeUnmount(() => document.removeEventListener('keydown', onKey))
</script>

<template>
  <Teleport to="body">
    <template v-if="open">
      <div class="scrim" @click="!busy && emit('close')" />
      <aside ref="panel" class="drawer" role="dialog" aria-modal="true" :aria-label="title" tabindex="-1">
        <div class="dw-head">
          <h2>{{ title }}</h2>
          <p v-if="subtitle" class="page-sub" style="margin-bottom: 0">{{ subtitle }}</p>
        </div>
        <div class="dw-body" :style="busy ? { pointerEvents: 'none', opacity: '.7' } : undefined">
          <slot />
        </div>
        <div class="dw-foot">
          <slot name="foot" />
        </div>
      </aside>
    </template>
  </Teleport>
</template>
