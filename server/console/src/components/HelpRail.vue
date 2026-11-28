<script setup lang="ts">
// The help rail: the institutional memory, one keystroke away.
//
// WHY THIS EXISTS. This codebase's best asset is its explanations — every
// non-obvious rule has a paragraph saying why. The problem was never the prose,
// it was the PLACEMENT: a permanent "A code is single-use and is not tied to a
// machine…" under the Codes table, a `msg warn` about tier passwords above the
// Tiers form, a `<details>` in Releases. Read once; in the way forever. A museum
// with a management UI attached.
//
// So the prose moves here: a rail you open with `?` when you need it, closed the
// rest of the time. It is still written in ONE place per screen (the HELP map
// below), and the labels quote the vocabulary the code uses, because a help pane
// that renames a concept is worse than none — the operator would look for
// "suspended" in the UI and find "stopped".
//
// NOT FED FROM docs/ AT BUILD TIME. An earlier idea was to render the markdown
// from docs/business/ directly. It was rejected for a concrete reason: the docs
// are the DESIGN RECORD (they contain baseline sections, superseded decisions and
// the word "unbuilt"), so shipping them verbatim would explain things that are no
// longer true. This pane holds the operator-facing subset, and the links point at
// the doc when the reader wants the history.

import { computed } from 'vue'
import type { RouteName } from '../router'

const props = defineProps<{ open: boolean; route: RouteName }>()
const emit = defineEmits<{ (e: 'close'): void }>()

interface HelpSection {
  h: string
  /** Plain sentences. No markdown: this is rendered as text, not parsed. */
  p: string[]
}

const HELP: Record<RouteName, HelpSection[]> = {
  today: [
    {
      h: 'What this page answers',
      p: [
        'Three questions, in the order a session usually asks them: how much is there, what is about to run out, and what is broken.',
        'Every number is a link. Each one opens the Codes list filtered to exactly the rows it counts — so a number you do not recognise is one click from the evidence.',
      ],
    },
    {
      h: 'Why “Due this week” is the most important thing here',
      p: [
        'Nothing in the system prompts a renewal. The hub holds no identity — no email, no phone — so it cannot know a payment is due, only that a date is approaching.',
        'That makes this queue the revenue mechanism rather than a convenience. If nobody works it, students lapse silently and nothing anywhere says so.',
      ],
    },
    {
      h: 'The faults, and what each one actually means',
      p: [
        'A plan the hub cannot serve: the code activates “successfully” and hands the device no server config. The student sees a connected app that reaches nothing. Fix the plan, not the code.',
        'Unclaimed: the key is free and the term is still running. Usually a move in progress; after days it means anyone holding the card can take the code.',
        'In term but quiet: usually a closed laptop. It is also what a hub outage or a blocked school network looks like — check the hub before the students.',
        'In use with no name: the name is the only link between a student and their code. Without it a support call cannot be answered.',
      ],
    },
  ],
  codes: [
    {
      h: 'The two facts a badge cannot hold',
      p: [
        'State is about the ENTITLEMENT: may this code be used? The hub decides, and refuses.',
        'Last seen is about LIVENESS: did the device reach us? The client decides, by checking in.',
        'They are different questions with different owners, and merging them is the defect this table exists to avoid. “Unclaimed” next to “checked in 2m ago” is not a contradiction — it is a code that is free while its device is still beating.',
        '“—” in the Last seen column means never checked in. It does not mean gone, and it does not mean signed out.',
      ],
    },
    {
      h: 'Renew, set term, set expiry — three different things',
      p: [
        'Renew delivers a SALE: it adds a span to this code, from whichever is later — today, or the current expiry. Paying early never loses days.',
        'Set term changes the PRODUCT: how long one purchase lasts from now on. It does not move an expiry already running.',
        'Set expiry moves the date without you having sold anything, so it records no price and no renewal. Term and Expiry are edited from the two tiles at the top of the panel — there is no separate “overrides” section.',
      ],
    },
    {
      h: '“Mark unused” is not “move to a new computer”',
      p: [
        'A code is not tied to a machine. A student on a new laptop re-enters their own code and it just works.',
        '“Mark unused” ends the current holder’s access and frees the code for a DIFFERENT student. It is one click: the audit row records the decision, and the device is cut off at its next check-in.',
      ],
    },
    {
      h: 'Keyboard',
      p: [
        'j / k move down and up the list, or the arrow keys.',
        'Enter renews the selected code. Escape closes the inspector.',
        '/ jumps to the search box. Ctrl/⌘-K opens the palette from anywhere.',
      ],
    },
  ],
  renewals: [
    {
      h: 'Why columns and not a list',
      p: [
        'A sorted list answers “which is next”. This page answers “how bad is it overall”, which decides whether you work the queue now or on Monday.',
        'The thresholds match the rest of the system: ≤7 days is the same window the client uses for its own expiry warning, and ≤30 days is the number on the Today card. Nothing here invents a threshold.',
      ],
    },
    {
      h: 'Lapsed is not the urgent column',
      p: [
        'A lapsed code needs a catch-up decision — renew it, or let it go. That is a different act from a reminder, so it gets its own column rather than colouring the urgent one.',
        'Stopped codes are excluded from every renewal column. The operator cut that student off deliberately, and prompting a renewal would be asking to undo their own decision.',
      ],
    },
  ],
  distribution: [
    {
      h: 'What the money column is, and is not',
      p: [
        '“Collected” is what an operator recorded when renewing, using the optional price field. Nothing else in the system knows what was paid.',
        'A “≥” means some renewals in the window recorded no price, so the figure is a FLOOR, not a total. A number that silently under-reports is worse than no number, because it will be believed.',
        'There is no commission calculation. The cut is a business agreement, not a stored value, and inventing a rate would produce a figure nobody can audit.',
        'Free stock is listed so it is not mistaken for inventory: a middleman earns nothing on a free code.',
      ],
    },
  ],
  'fleet/tiers': [
    {
      h: 'These settings reach every client on its next check-in',
      p: [
        'A tier is the connection detail a device receives when it activates. Editing one changes the live fleet, not a copy.',
        'A tier with no server or no port cannot be served. Codes on it activate successfully and receive nothing — a connected app that reaches no internet, with no error shown anywhere.',
        'The tier password is deliberately not editable here. Changing it breaks every client already using that tier, with no way for an existing install to recover on its own.',
      ],
    },
  ],
  'fleet/releases': [
    {
      h: 'Publishing is all-or-nothing',
      p: [
        'The hub fetches the four platform binaries, their signatures and the manifest from the GitHub release, verifies every file, and publishes atomically. A partial release cannot be produced from here.',
        'CI must have finished and the tag must be pushed before this will find anything.',
        'The macOS .zip is the download you hand a person. It is not an update payload — the updater replaces the binary directly and cannot unpack a zip — so it never appears in the platform table.',
        'An active release is offered to every client. There is no percentage rollout: `active` is the single switch.',
      ],
    },
  ],
  'fleet/maintenance': [
    {
      h: 'When to use each operation',
      p: [
        'Import is for when the code set and the hub have drifted apart. A code the hub does not hold is not cosmetic: the client treats the 404 as a refusal and deletes its own entitlement, leaving the student signed in with no tunnel and no error.',
        'Import never overwrites an existing row. A code already on the hub may carry real history — activation, renewals — and that must survive a reconciliation.',
        'The one-shot trigger link lets a fetch be started without the admin token (from a phone or CI). It is single-use and expires, which is what makes leaking one harmless.',
      ],
    },
  ],
}

const sections = computed(() => HELP[props.route] || [])
</script>

<template>
  <aside v-if="open" class="help-rail" aria-label="Help for this screen">
    <div class="help-head">
      <strong>About this screen</strong>
      <button class="row-menu" type="button" aria-label="Close help" @click="emit('close')">✕</button>
    </div>
    <div class="help-body">
      <section v-for="s in sections" :key="s.h">
        <h3>{{ s.h }}</h3>
        <p v-for="(para, i) in s.p" :key="i">{{ para }}</p>
      </section>
      <p class="help-foot">
        The long version, with the history and the reasoning, is in
        <code>docs/business/09-console.md</code>.
      </p>
    </div>
  </aside>
</template>
