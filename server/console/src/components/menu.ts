// Types shared between the row and its overflow menu.
//
// In its own module rather than exported from `RowMenu.vue`: a `<script setup>`
// SFC cannot export a named binding for runtime import, so a type living there
// is only safe while every importer writes `import { type MenuItem }` — which is
// one keystroke away from a build that resolves nothing. A plain .ts file has no
// such cliff.

export interface MenuItem {
  label: string
  /** Right-aligned hint, e.g. why an item is unavailable. */
  hint?: string
  danger?: boolean
  disabled?: boolean
  /** Draw a separator ABOVE this item, to group destructive actions last. */
  sep?: boolean
  run: () => void
}
