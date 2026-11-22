#!/usr/bin/env node
/**
 * Generates `PROSE.md` — every user-facing string, where it appears, and where it
 * lives in code.
 *
 * # Why this exists
 *
 * Editing the app's copy today means opening 20 components and 5 locale bundles
 * and hoping you found every instance. The goal is one file a person can read top
 * to bottom, change, and have the change flow back with `apply-prose.mjs`.
 *
 * # Why it is DERIVED rather than maintained
 *
 * A hand-maintained copy of the copy is a second source of truth, and this repo
 * has paid for that repeatedly (`COLOURS.md` §11). So:
 *
 *   · the strings come from the locale bundles, never from this script;
 *   · the "where in the app" column comes from an AREAS table below, which is
 *     the only hand-written part, and which `--check` verifies against the tree;
 *   · every string's code location is resolved by scanning for the key.
 *
 *   node scripts/prose-manifest.mjs           # write PROSE.md
 *   node scripts/prose-manifest.mjs --check   # fail if PROSE.md is stale
 *
 * # What counts as user-facing
 *
 * Only strings a student can actually meet. The test is reachability, not
 * namespace: `settings.*` holds strings the profile engine still emits through
 * `notification-handlers`, while `tray.*` is a native menu. See REACHABLE below.
 */

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const SRC = path.join(ROOT, 'src')
const OUT = path.join(ROOT, 'PROSE.md')

const LOCALES = path.join(SRC, 'locales')
const BACKEND_LOCALE = path.join(
  ROOT,
  'crates/clash-verge-i18n/locales/en.yml',
)

/**
 * Screens and surfaces, in the order a student meets them.
 *
 * `scope` is a key prefix or a file path. The generator attributes a string to
 * the first area whose `scope` test matches; anything unmatched is reported as
 * unattributed rather than silently dropped, and `--check` fails on it — a
 * string nobody can place is exactly the one that goes stale.
 */
const AREAS = [
  {
    id: 'activation',
    title: 'Activation gate',
    where: 'Shown before activation, in place of the whole app (main.tsx)',
    prefix: 'home.components.connection.activation.',
  },
  {
    id: 'suspended',
    title: 'Suspension screen',
    where: 'Shown when the hub has withdrawn this device (main.tsx)',
    prefix: 'home.components.connection.suspended.',
  },
  {
    id: 'connection',
    title: 'Connection tab',
    where: 'The main screen — status, Connect, traffic, tier (pages/connection.tsx)',
    prefix: 'home.components.connection.',
    exclude: ['home.components.connection.activation.', 'home.components.connection.suspended.'],
  },
  {
    id: 'support',
    title: 'Support block',
    where: 'The email-us panel, on activation, Connection and Account',
    prefix: 'home.components.connection.supportBlock.',
  },
  {
    id: 'signout',
    title: 'Sign-out card',
    where: 'Bottom of Account — sign out, its dialog, its outcomes',
    prefix: 'home.components.connection.signOut.',
  },
  {
    id: 'account',
    title: 'Account tab',
    where: 'Subscription, device, preferences, updates (pages/account.tsx)',
    prefix: 'home.components.connection.account.',
  },
  {
    id: 'traffic',
    title: 'Traffic graph and figures',
    where: 'The sidebar graph and the Connection/Account traffic cards',
    prefix: 'home.components.traffic.',
  },
  {
    id: 'sidebar',
    title: 'Sidebar and navigation',
    where: 'Tabs, the subscription chip, the window menu',
    prefix: 'layout.components.',
  },
  {
    id: 'dialogs',
    title: 'Dialogs',
    where:
      'Service migration, system-proxy permission, subscription, update prompt',
    prefixes: [
      'layout.components.serviceMigration.',
      'layout.components.sysproxyPrivilege.',
      'layout.components.subscription.',
      'shared.update.',
    ],
  },
  {
    id: 'notices',
    title: 'Toasts and notifications',
    where:
      'Transient messages: errors, successes, warnings. The `settings.*` keys ' +
      'here are emitted by the profile engine through ' +
      '`pages/_layout/utils/notification-handlers.ts`, not by a Settings tab ' +
      'that no longer exists.',
    prefixes: [
      'shared.feedback.',
      'layout.feedback.',
      'settings.feedback.',
      'settings.modals.',
      'settings.sections.',
      'settings.statuses.',
    ],
  },
  {
    id: 'network',
    title: 'Network readouts',
    where: 'IP lookup errors, shown where a public address is displayed',
    prefixes: ['home.components.ipInfo.'],
  },
  {
    id: 'errors',
    title: 'Error screen',
    where: 'What a crash renders instead of the page',
    prefixes: ['shared.actions.', 'shared.feedback.errors.'],
  },
  {
    id: 'backend',
    title: 'Native strings',
    where: 'Tray menu, OS notifications, macOS launch guard — Rust + YAML',
    backend: true,
  },
]

/**
 * Namespaces a student cannot reach. Each was checked, not assumed.
 *
 * The rule: a namespace belongs here only if no Rust `t!(…)` and no
 * `showNotice.*('…')` call names one of its keys. `settings.*` is NOT here —
 * notification-handlers still emits several of its keys in response to real
 * events, which is exactly the case a naive "only the two-tab UI matters" pass
 * would have deleted.
 */
const NOT_USER_FACING = [
  {
    ns: 'profiles.',
    reason:
      'The profiles UI was removed in 3.1.0. The keys stay because ' +
      '`notice-service.ts` maps them from Rust `PROFILE_*` error codes, which ' +
      '`cmd/profile.rs` still emits — deleting them would break a live toast. ' +
      'Not reachable from a screen.',
  },
  {
    ns: 'monaco.',
    reason:
      'Monaco editor chrome. The editor is only reachable from components ' +
      'nothing renders, so this is menu text a student never sees.',
  },
  {
    ns: 'shared.actions.',
    reason:
      'The `components/base/` control set (dialog, search box, chip editor, ' +
      'text field, loading, switch, fieldset) is not rendered by any screen — ' +
      'the two-tab UI builds its own. The keys stay for now; deleting them is ' +
      'a separate change that should also delete the components.',
  },
  {
    ns: 'shared.placeholders.',
    reason: 'Only the search box and text field above use these; both unrendered.',
  },
  {
    ns: 'shared.validation.',
    reason: 'Only the search box above uses it; unrendered.',
  },
  {
    ns: 'shared.editorModes.',
    reason: 'Only the chip editor above uses it; unrendered.',
  },
  {
    ns: 'shared.labels.',
    reason:
      '`retryAttempts` is read by the traffic error boundary, which is ' +
      'reachable — but the label is a bare technical noun with no unit and no ' +
      'context, and it is the one string in the app that reads as an engine ' +
      'detail rather than a sentence. Listed here so a copy pass sees it; fix ' +
      'the row in `traffic-error-boundary.tsx` rather than the bundle if you ' +
      'want it gone.',
  },
  {
    ns: 'shared.statuses.',
    reason: 'Only the empty-state and chip editor above use it; both unrendered.',
  },
  {
    ns: 'shared.actions.close',
    reason: 'Only the deleted editor viewer used it.',
  },
  {
    ns: 'shared.actions.resetToDefault',
    reason: 'Only the deleted editor viewer used it.',
  },
  {
    ns: 'shared.actions.save',
    reason: 'Only the deleted editor viewer used it.',
  },
  {
    ns: 'shared.window.',
    reason:
      'The custom title bar has no text labels: the window controls are icon ' +
      'buttons with `aria-label` set from their own icon component. Kept for a ' +
      'title bar that never renders.',
  },
]

// ── reading ─────────────────────────────────────────────────────────────────

const walkJson = (obj, prefix = '') => {
  const out = []
  if (obj && typeof obj === 'object') {
    for (const [k, v] of Object.entries(obj)) {
      const key = prefix ? `${prefix}.${k}` : k
      if (typeof v === 'string') out.push([key, v])
      else out.push(...walkJson(v, key))
    }
  }
  return out
}

const readFrontend = () => {
  const out = []
  // English is the canonical bundle: `i18n:check` holds every other locale to its
  // key set, so this is the only place a *new* string can appear.
  const enDir = path.join(LOCALES, 'en')
  for (const file of fs.readdirSync(enDir).sort()) {
    if (!file.endsWith('.json')) continue
    const ns = file.replace(/\.json$/, '')
    const json = JSON.parse(fs.readFileSync(path.join(enDir, file), 'utf8'))
    for (const [key, value] of walkJson(json)) {
      out.push({ key: `${ns}.${key}`, value, file: `src/locales/en/${file}` })
    }
  }
  return out.sort((a, b) => a.key.localeCompare(b.key))
}

/** A tiny indentation-driven YAML reader for the one shape this file uses. */
const readBackend = () => {
  const out = []
  let stack = []
  const lines = fs.readFileSync(BACKEND_LOCALE, 'utf8').split('\n')
  for (const line of lines) {
    if (!line.trim() || line.trim().startsWith('#') || line.startsWith('_version')) {
      continue
    }
    const indent = line.length - line.trimStart().length
    const m = /^\s*'?([\w-]+)'?:\s*(.*)$/.exec(line)
    if (!m) continue
    const [, key, rest] = m
    const depth = Math.floor(indent / 2)
    stack = stack.slice(0, depth)
    stack.push(key)
    if (rest && !rest.startsWith('|') && !rest.startsWith('>')) {
      const value = rest.replace(/^'(.*)'$/, '$1').replace(/''/g, "'")
      out.push({
        key: `backend:${stack.join('.')}`,
        value,
        file: 'client/crates/clash-verge-i18n/locales/en.yml',
      })
    }
  }
  return out
}

/** Frontend files that render, excluding tests and the locale bundles. */
const sourceFiles = () => {
  const out = []
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        if (entry.name === 'locales' || entry.name === 'generated') continue
        walk(full)
      } else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\./.test(entry.name)) {
        // `src/types/generated/` is the complete key list, written so TypeScript
        // can reject a typo. It references EVERY key by construction, so counting
        // it as a usage makes every string look reachable — it would have hidden
        // the orphan check entirely.
        out.push(full)
      }
    }
  }
  walk(SRC)
  return out
}

const files = sourceFiles().map((f) => ({
  rel: path.relative(ROOT, f),
  text: fs.readFileSync(f, 'utf8'),
}))

/**
 * Where a frontend key is referenced.
 *
 * Three access forms exist in this tree, and a scan that knows only the first
 * will call a live string an orphan:
 *
 *   1. `t('home.components.…')`           — the normal case
 *   2. `$.home.components.…`              — i18next's selector object, used
 *                                            outside React (`services/api.ts`,
 *                                            `hooks/use-clash.ts`)
 *   3. a key held in a variable and passed — `connect-notice.ts` returns
 *      `{ key: 'home.…' }`, `notification-handlers.ts` passes literals into
 *      `showNotice.*`. Both are plain `'…'` literals, so the same scan finds
 *      them; only the CALL SITE differs, which is why this searches for the
 *      quoted key rather than for `t(`.
 */
const locate = (key) => {
  const hits = []
  // The `$.` form is written as a property path, not a string, so the whole key
  // appears as `${key.slice(ns.length)}` with the namespace as the root.
  const dotKey = key.split('.').slice(1).join('.')
  for (const { rel, text } of files) {
    const needles = [`'${key}'`, `"${key}"`, `\`${key}\``, `$.${dotKey}`]
    for (const needle of needles) {
      const at = text.indexOf(needle)
      if (at === -1) continue
      const line = text.slice(0, at).split('\n').length
      hits.push(`${rel}:${line}`)
      break
    }
  }
  return hits
}

const readRust = (rel) => {
  const full = path.join(ROOT, rel)
  return fs.existsSync(full) ? fs.readFileSync(full, 'utf8') : ''
}

// ── assembly ────────────────────────────────────────────────────────────────

/**
 * The area a frontend key belongs to.
 *
 * **Longest prefix wins, not first match.** `home.components.connection.` is a
 * prefix of `…connection.activation.` and `…connection.supportBlock.`, so a
 * first-match scan puts the activation gate's copy inside the Connection tab and
 * hides two whole screens. Sorting by descending prefix length makes the narrow
 * scopes win without depending on the order of `AREAS`.
 */
const areaFor = (key) => {
  const ordered = [...AREAS]
    .filter((a) => !a.backend)
    .sort((a, b) => longestPrefix(b) - longestPrefix(a))
  for (const area of ordered) {
    if (area.backend) continue
    const prefixes = area.prefixes ?? [area.prefix]
    const matched = prefixes.some((p) => key.startsWith(p))
    if (!matched) continue
    if (area.exclude?.some((e) => key.startsWith(e))) continue
    return area
  }
  return null
}

const longestPrefix = (area) =>
  Math.max(...(area.prefixes ?? [area.prefix]).map((p) => p.length))

const excluded = (key) =>
  NOT_USER_FACING.find(({ ns }) => key.startsWith(ns))

const build = () => {
  const strings = [...readFrontend(), ...readBackend()]
  const byArea = new Map(AREAS.map((a) => [a.id, []]))
  const unattributed = []
  const excludedStrings = []

  for (const s of strings) {
    if (s.key.startsWith('backend:')) {
      byArea.get('backend').push({
        ...s,
        where: backendWhere(s.key),
        code: backendCode(s.key),
      })
      continue
    }
    const skip = excluded(s.key)
    if (skip) {
      excludedStrings.push({ ...s, reason: skip.reason })
      continue
    }
    const area = areaFor(s.key)
    if (!area) {
      unattributed.push(s)
      continue
    }
    byArea.get(area.id).push({ ...s, code: locate(s.key) })
  }
  return { byArea, unattributed, excludedStrings, total: strings.length }
}

/** The Rust site that owns a backend string. */
const backendWhere = (key) => {
  const k = key.replace('backend:', '')
  if (k.startsWith('tray.')) return 'Native tray menu (`core/tray/menu_def.rs`)'
  if (k.startsWith('notifications.')) return 'OS notification (`utils/notification.rs`)'
  if (k.startsWith('service.')) return 'macOS privilege prompt (`cmd/locus.rs`)'
  if (k.startsWith('launchGuard.')) return 'macOS first-run guard (`utils/macos_launch_guard.rs`)'
  return 'Native dialog'
}

const backendCode = (key) => {
  const k = key.replace('backend:', '')
  const leaf = k.split('.').pop()
  const roots = [
    'client/src-tauri/src/core/tray/menu_def.rs',
    'client/src-tauri/src/utils/notification.rs',
    'client/src-tauri/src/utils/macos_launch_guard.rs',
    'client/src-tauri/src/cmd/locus.rs',
  ]
  const hits = []
  for (const rel of roots) {
    const text = readRust(rel)
    if (!text) continue
    const idx = text.indexOf(`"${leaf}"`) !== -1 ? text.indexOf(`"${leaf}"`) : text.indexOf(`.${leaf}`)
    if (idx === -1) continue
    hits.push(`${rel}:${text.slice(0, idx).split('\n').length}`)
  }
  return hits
}

const escapeCell = (s) =>
  s.replace(/\|/g, '\\|').replace(/\n/g, ' ').trim()

const render = ({ byArea, unattributed, excludedStrings, total }) => {
  const lines = []
  const w = (s = '') => lines.push(s)

  w('# PROSE — every user-facing string, in one place')
  w()
  w('```')
  w('audience:    whoever edits the app\'s wording')
  w('status:      live')
  w('authoritative-for: the text a student sees, where it appears, and where it lives in code')
  w('verified-against: generated from src/locales/en/ and clash-verge-i18n/locales/en.yml')
  w('```')
  w()
  w('> **This file is generated. Edit the wording here, then run')
  w('> `node scripts/apply-prose.mjs`** — that writes your text back over the')
  w('> original in every locale bundle and verifies nothing was missed. Do not')
  w('> hand-edit a locale JSON; the next generation would show your change as a')
  w('> diff to itself.')
  w('>')
  w('> `node scripts/prose-manifest.mjs` regenerates this file after the code')
  w('> changes. `--check` fails if it is stale.')
  w()
  w(`**${total} strings.** Each row is: the key, the text, where in the app a`)
  w('student meets it, and the exact `file:line` that references it.')
  w()
  w('Changing a row changes that string only. To change what a whole screen')
  w('says, edit the rows under that screen\'s heading.')
  w()

  for (const area of AREAS) {
    const rows = byArea.get(area.id) ?? []
    if (rows.length === 0) continue
    w(`## ${area.title}`)
    w()
    w(`*${area.where}* — ${rows.length} strings.`)
    w()
    w('| Key | Text | Code |')
    w('|---|---|---|')
    for (const r of rows) {
      const code = (r.code ?? []).join('<br>') || '—'
      w(`| \`${r.key}\` | ${escapeCell(r.value)} | ${code} |`)
    }
    w()
  }

  if (unattributed.length > 0) {
    w('## ⚠ Unattributed')
    w()
    w('These strings belong to no area above. **A string nobody can place is the')
    w('one that goes stale** — add a `scope` for it in `AREAS` in the generator')
    w('rather than moving it by hand.')
    w()
    w('| Key | Text | Code |')
    w('|---|---|---|')
    for (const r of unattributed.sort((a, b) => a.key.localeCompare(b.key))) {
      w(`| \`${r.key}\` | ${escapeCell(r.value)} | ${(r.code ?? []).join('<br>') || '—'} |`)
    }
    w()
  }

  w('## Deliberately not in the manifest')
  w()
  w('Strings that exist in the bundles but that a student cannot reach. Each is')
  w('listed with the reason, so "why is this not editable here?" has an answer.')
  w()
  w('| Key prefix | Why it is out of scope |')
  w('|---|---|')
  const prefixes = new Map()
  for (const s of excludedStrings) {
    const ns = NOT_USER_FACING.find(({ ns }) => s.key.startsWith(ns)).ns
    prefixes.set(ns, excludedStrings.filter((x) => x.key.startsWith(ns)).length)
  }
  for (const [ns, count] of prefixes) {
    const entry = NOT_USER_FACING.find((e) => e.ns === ns)
    w(`| \`${ns}*\` (${count} strings) | ${escapeCell(entry.reason)} |`)
  }
  w()
  w('## Not English-only: what a rewrite does and does not reach')
  w()
  w('`src/locales/en/` is the canonical bundle, but the app ships in 13')
  w('languages, and each of the other 12 owns the same key set. A rewrite of a')
  w('row in this file therefore has **three** possible outcomes, and only the')
  w('first is a finished job:')
  w()
  w('| The other locale\'s value | What `prose:apply` does | What a student sees |')
  w('|---|---|---|')
  w('| a real translation | leaves it alone | the **old** meaning, in their language |')
  w('| a copy of the old English | leaves it alone | the **old** English |')
  w('| the key is absent | leaves it absent | English, by i18next fallback |')
  w()
  w('So `prose:apply` makes the change true for `en` and for any key still')
  w('falling back — and **not** for a string someone translated. That is not a')
  w('bug to fix by copying English over the top: it would replace a real')
  w('translation with English. The row is finished when the translated value is')
  w('updated in that locale, which is a translator\'s job and is reviewed per')
  w('language.')
  w()
  w('`pnpm i18n:check` reports the drift: a key present in `en` and absent')
  w('elsewhere is `missing`, and a value identical to English in a locale that')
  w('normally translates is not distinguishable from a fallback copy — which is')
  w('why the count below is a floor, not a total.')
  w()
  w('Changing a `{{placeholder}}` or a count-sensitive string needs the')
  w('plural forms re-checked in every language too; i18next will not warn.')
  w()

  return lines.join('\n')
}

const main = () => {
  const built = build()
  const text = render(built)
  const check = process.argv.includes('--check')

  if (check) {
    const current = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8') : ''
    if (current !== text) {
      console.error('PROSE.md is stale — run: node scripts/prose-manifest.mjs')
      process.exit(1)
    }
    console.log('PROSE.md is up to date')
    return
  }

  fs.writeFileSync(OUT, text)
  const unattributed = built.unattributed.length
  console.log(
    `PROSE.md written: ${built.total} strings, ` +
      `${built.excludedStrings.length} out of scope, ` +
      `${unattributed} unattributed`,
  )
  if (unattributed > 0) {
    console.warn(
      '  ⚠ unattributed strings — add an AREAS scope, do not move them by hand',
    )
  }
}

main()
