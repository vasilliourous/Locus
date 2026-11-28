// Shared metadata: the tier ("plan") list and the middleman names.
//
// WHY A MODULE-LEVEL STORE. These two lists feed three surfaces at once — the
// Codes filter bar, the inspector's labelled pickers, and the create/renew
// drawers — and the previous code refetched BOTH on every single mutating action
// (`refreshAll()` awaited `loadMeta()` before reloading the rows), which meant two
// extra round-trips per renewal on a box that does not need them. A renewal does
// not change the set of tiers or the set of middleman names, so the lists have no
// business being in that path.
//
// The rule: fetch once per session, fetch again only when something could have
// changed them — a code created with a new middleman name, or a tier edited in
// Fleet. Both callers are explicit below; nothing reloads them "just in case".
//
// THEY COME FROM THE HUB, NEVER FROM A LITERAL. A literal tier list can drift
// from the seed and offer a plan that mints an unresolvable code — the exact
// failure check-consistency.sh §27(f) guards.

import { computed, ref } from 'vue'
import { call } from '../../api'

interface TierRecord {
  tier: string
  active: boolean
}

const tiers = ref<string[]>([])
const middlemen = ref<string[]>([])
const loaded = ref(false)
const busy = ref(false)

/** Load both lists. Safe to call repeatedly; concurrent calls collapse. */
async function load(force = false): Promise<void> {
  if (busy.value) return
  if (loaded.value && !force) return
  busy.value = true
  try {
    // Parallel: they are independent reads and either alone is useless.
    const [t, m] = await Promise.all([
      call<{ tiers: TierRecord[] }>('tiers.list'),
      call<{ middlemen: { name: string }[] }>('middlemen.list'),
    ])
    if (t.ok && t.data) {
      // Sorted, de-duplicated. An inactive tier is still listed: a code may
      // legitimately carry one, and hiding it would make that code's plan look
      // like it has vanished.
      tiers.value = Array.from(new Set(t.data.tiers.map((x) => x.tier).filter(Boolean))).sort()
    }
    if (m.ok && m.data) {
      middlemen.value = Array.from(new Set(m.data.middlemen.map((x) => x.name).filter(Boolean))).sort()
    }
    loaded.value = true
  } finally {
    busy.value = false
  }
}

export const meta = {
  tiers: computed(() => tiers.value),
  middlemen: computed(() => middlemen.value),
  loaded: computed(() => loaded.value),
  /** Fetch on first use, or again when a caller knows the set changed. */
  refresh: () => load(true),
  ensure: () => load(false),
}
