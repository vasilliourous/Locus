/**
 * Where a string a student can meet is *consumed*.
 *
 * Split out of `prose-manifest.mjs` so a test can call the **same** functions the
 * generator does. A test that re-implemented this scan would pass while the
 * generator stayed broken — which is the failure this file exists to prevent,
 * twice over (see `tests/prose-locate.test.ts`).
 *
 * It lives in `scripts/`, not `scripts/lib/`, because `client/eslint.config.ts`
 * lists `scripts/*.mjs` in `allowDefaultProject` — a file one directory deeper is
 * not covered by any tsconfig project and cannot be linted at all. A module no
 * linter can see is the same class of problem this file is fixing.
 *
 *   locate(key)      → where a frontend key is referenced
 *   backendCode(key) → where a native (Rust/YAML) key is referenced
 *   nativeLeafKeys() → every backend key in the YAML, for the guard test
 */

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
/** `client/` — the directory this repo's app lives in. */
export const ROOT = path.resolve(__dirname, '..')
const SRC = path.join(ROOT, 'src')
const BACKEND_LOCALE = path.join(ROOT, 'crates/clash-verge-i18n/locales/en.yml')

/** Frontend files that render, excluding tests, the locale bundles and the
 * generated key list. `src/types/generated/` references EVERY key by
 * construction; counting it would make every string look reachable and hide the
 * orphan check entirely. */
const sourceFiles = () => {
  const out = []
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name)
      if (entry.isDirectory()) {
        if (entry.name === 'locales' || entry.name === 'generated') continue
        walk(full)
      } else if (
        /\.(ts|tsx)$/.test(entry.name) &&
        !/\.test\./.test(entry.name)
      ) {
        out.push(full)
      }
    }
  }
  walk(SRC)
  return out
}

export const files = sourceFiles().map((f) => ({
  rel: path.relative(ROOT, f),
  text: fs.readFileSync(f, 'utf8'),
}))

/**
 * A property path must END where the key ends.
 *
 * `…dashboardToggled` is a prefix of `…dashboardToggled.title`, so a plain
 * `indexOf` on an unquoted path can report a call site belonging to a different
 * string. The next character after the match must not continue the path.
 */
const PATH_END = /[^\w$]/

const pathIndex = (text, needle) => {
  let from = text.indexOf(needle)
  while (from !== -1) {
    const after = text[from + needle.length]
    if (after === undefined || PATH_END.test(after)) return from
    from = text.indexOf(needle, from + 1)
  }
  return -1
}

/**
 * Where a frontend key is referenced.
 *
 * Four access forms exist in this tree, and a scan that knows only the first will
 * call a live string an orphan:
 *
 *   1. `t('home.components.…')`  — the normal case. Exact, because `'…connect'`
 *      cannot be a prefix of `'…connected'`.
 *   2. `i18n.t(($) => $.a.b.c)`  — i18next's selector form, used outside React
 *      (`hooks/use-clash.ts`, `services/api.ts`). THE KEY IS A BARE PROPERTY
 *      PATH: no quotes, no `$.` immediately before it. The first version of this
 *      function missed it entirely, and four live strings read as unreachable.
 *   3. `$.a.b.c` — the same selector written against the namespace root.
 *   4. a key held in a variable and passed — `connect-notice.ts` returns
 *      `{ key: 'home.…' }`, `notification-handlers.ts` passes literals into
 *      `showNotice.*`. Those are plain `'…'` literals, so form 1 finds them.
 */
export const locate = (key) => {
  const hits = []
  for (const { rel, text } of files) {
    let found = -1
    for (const needle of [`'${key}'`, `"${key}"`, `\`${key}\``]) {
      found = text.indexOf(needle)
      if (found !== -1) break
    }
    if (found === -1) found = pathIndex(text, key)
    if (found === -1) {
      const dotKey = key.split('.').slice(1).join('.')
      if (dotKey) found = pathIndex(text, `$.${dotKey}`)
    }
    if (found === -1) continue
    hits.push(`${rel}:${text.slice(0, found).split('\n').length}`)
  }
  return hits
}

const readRust = (rel) => {
  const full = path.join(ROOT, rel)
  return fs.existsSync(full) ? fs.readFileSync(full, 'utf8') : ''
}

/**
 * Every Rust file that passes a native key to the i18n macro.
 *
 * These are `client/`-relative, because `ROOT` is `client/`. They used to be
 * written with a `client/` prefix on top of that, so `readRust` returned `''`
 * for every one of them and all 68 native rows silently resolved to nothing.
 */
const RUST_SITES = [
  'src-tauri/src/core/tray/menu_def.rs',
  'src-tauri/src/core/tray/mod.rs',
  'src-tauri/src/utils/notification.rs',
  'src-tauri/src/utils/macos_launch_guard.rs',
  'src-tauri/src/core/service.rs',
]

/**
 * Where a native string is consumed.
 *
 * Rust names the whole dotted path, so that is what is searched for — never the
 * leaf. `title`, `body`, `on` and `off` repeat in every notification, so a leaf
 * search can never be right.
 *
 * The emitted hit is `client/`-prefixed because that is the path an editor opens
 * from the repo root.
 */
export const backendCode = (key) => {
  const k = key.replace('backend:', '')
  const hits = []
  for (const rel of RUST_SITES) {
    const text = readRust(rel)
    if (!text) continue
    const idx = text.indexOf(`"${k}"`)
    if (idx === -1) continue
    hits.push(`client/${rel}:${text.slice(0, idx).split('\n').length}`)
  }
  return hits
}

/** Every leaf key in the native YAML bundle, as `backend:`-prefixed keys. */
export const nativeLeafKeys = () => {
  const out = []
  let stack = []
  for (const line of fs.readFileSync(BACKEND_LOCALE, 'utf8').split('\n')) {
    if (
      !line.trim() ||
      line.trim().startsWith('#') ||
      line.startsWith('_version')
    ) {
      continue
    }
    const indent = line.length - line.trimStart().length
    const m = /^\s*'?([\w-]+)'?:\s*(.*)$/.exec(line)
    if (!m) continue
    const [, key, rest] = m
    stack = stack.slice(0, Math.floor(indent / 2))
    stack.push(key)
    if (rest && !rest.startsWith('|') && !rest.startsWith('>')) {
      out.push({ key: `backend:${stack.join('.')}` })
    }
  }
  return out
}
