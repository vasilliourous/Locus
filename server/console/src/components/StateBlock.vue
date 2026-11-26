<script setup lang="ts">
// Empty / loading / error, in one component.
//
// WHY ONE COMPONENT. The console previously had three unrelated treatments for
// the same three conditions: a bare `<p class="muted">Loading…</p>`, a
// `v-if="shown === 0"` paragraph that differed per page, and — for errors — a
// toast that vanished before it could be read, leaving a silently empty table.
// An empty table and a failed request looked identical, which is the one state
// pair an operator must never have to guess between.
//
// Every state here is designed, not defaulted:
//   loading — keeps the geometry of the content it replaces
//   empty   — says what would put something here
//   error   — says what failed and offers the retry
//   denied  — a session that expired mid-action is not an empty result

withDefaults(
  defineProps<{
    state: 'loading' | 'empty' | 'error' | 'denied'
    title?: string
    message?: string
    hint?: string
    /** Skeleton row count for `loading`; matches a table body. */
    rows?: number
  }>(),
  { rows: 6 },
)

const emit = defineEmits<{ (e: 'retry'): void }>()
</script>

<template>
  <!-- Loading renders as skeleton rows, so a refresh of a populated table does
       not collapse to a spinner and snap back. -->
  <div v-if="state === 'loading'" aria-busy="true" aria-live="polite">
    <div v-for="i in rows" :key="i" class="skel-line">
      <div class="skel" :style="{ flexBasis: 18 + ((i * 7) % 30) + '%' }" />
      <div class="skel" style="flex: 2" />
      <div class="skel" style="flex: 0 0 64px" />
    </div>
    <span class="sr-only">Loading…</span>
  </div>

  <div v-else-if="state === 'denied'" class="state err" role="alert">
    <strong>{{ title || 'Your session has ended' }}</strong>
    <p>{{ message || 'The admin token was not accepted. Sign in again to continue.' }}</p>
    <div class="actions">
      <button class="primary" @click="emit('retry')">Sign in again</button>
    </div>
  </div>

  <div v-else-if="state === 'error'" class="state err" role="alert">
    <strong>{{ title || 'Could not load this' }}</strong>
    <p>{{ message || 'The hub did not answer.' }}</p>
    <div class="actions">
      <button @click="emit('retry')">Try again</button>
    </div>
    <p v-if="hint" class="hint">{{ hint }}</p>
  </div>

  <div v-else class="state">
    <strong>{{ title || 'Nothing here yet' }}</strong>
    <p>{{ message }}</p>
    <p v-if="hint" class="hint">{{ hint }}</p>
    <div v-if="$slots.actions" class="actions"><slot name="actions" /></div>
  </div>
</template>

<style scoped>
.skel-line {
  display: flex;
  gap: 12px;
  align-items: center;
  height: var(--row-h);
  padding: 0 12px;
  border-bottom: 1px solid var(--border);
}
.skel-line:last-child { border-bottom: none; }
</style>
