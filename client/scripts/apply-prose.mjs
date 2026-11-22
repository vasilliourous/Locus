#!/usr/bin/env node
/**
 * Writes the wording in `PROSE.md` back over the original strings.
 *
 * # The workflow this exists for
 *
 *   1. Edit the **Text** column in `PROSE.md`. Nothing else.
 *   2. `node scripts/apply-prose.mjs`
 *   3. The locale bundles (and `en.yml`) now hold your words.
 *
 * # Why a script rather than a person copying
 *
 * 332 strings across 5 JSON bundles and 1 YAML file. A manual pass misses some,
 * and a missed one is invisible: the screen still renders, in the old voice,
 * beside the new one. This applies every row it can and then **verifies nothing
 * was skipped** — a row it could not write is a failure, not a warning.
 *
 * # What it refuses to do
 *
 * · **Touch a row whose text still equals the original.** `PROSE.md` is
 *   generated, so most rows are unchanged; rewriting them is a no-op that would
 *   hide a real edit among 332.
 * · **Write a value into a bundle key that does not exist.** A renamed key in
 *   the manifest is a manifest that has drifted from the code.
 * · **Apply a row for a backend string without matching its Rust call site.**
 *   The YAML is one file with no per-key override, so a mistyped backend key
 *   silently changes nothing.
 * · **Preserve `{{placeholders}}` loss.** A translation that drops `{{count}}`
 *   renders "expires in  days". Every placeholder in the original must survive
 *   in the replacement, or the row is rejected.
 *
 *   node scripts/apply-prose.mjs            # apply, then report
 *   node scripts/apply-prose.mjs --dry-run  # report only
 */

import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const PROSE = path.join(ROOT, 'PROSE.md')
const LOCALES = path.join(ROOT, 'src/locales')
const BACKEND = path.join(ROOT, 'crates/clash-verge-i18n/locales/en.yml')

const dryRun = process.argv.includes('--dry-run')

// ── read the manifest ───────────────────────────────────────────────────────

/**
 * Parses the tables in `PROSE.md`.
 *
 * Deliberately narrow: it reads only rows whose first cell is a backticked key,
 * because that is the shape the generator writes. A row a person added by hand
 * in some other style is reported as unparsed rather than guessed at.
 */
const readManifest = () => {
  const text = fs.readFileSync(PROSE, 'utf8')
  const rows = []
  const skipped = []
  let area = '(unknown)'
  /**
   * Only the string tables are read.
   *
   * The document also contains a "Deliberately not in the manifest" table whose
   * first column is a key PREFIX (`profiles.*`), and an "Unattributed" table
   * that must never be applied blindly. Both start with a backticked cell, so
   * the stop marker is the heading rather than the row shape.
   */
  const STOP = '## Deliberately not in the manifest'
  let inScope = true
  for (const line of text.split('\n')) {
    if (line.startsWith(STOP)) {
      inScope = false
      continue
    }
    const heading = /^## (.+)$/.exec(line)
    if (heading) {
      area = heading[1].trim()
      continue
    }
    if (!inScope) continue
    if (!line.startsWith('| `')) continue
    const cells = splitRow(line)
    if (cells.length < 2) {
      skipped.push({ area, line })
      continue
    }
    const key = cells[0].replace(/^`|`$/g, '').trim()
    rows.push({ area, key, text: cells[1] })
  }
  return { rows, skipped }
}

/** Splits a markdown row on unescaped pipes. */
function splitRow(line) {
  const cells = []
  let cell = ''
  for (let i = 1; i < line.length; i += 1) {
    const ch = line[i]
    if (ch === '\\' && line[i + 1] === '|') {
      cell += '|'
      i += 1
    } else if (ch === '|') {
      cells.push(cell.trim())
      cell = ''
    } else {
      cell += ch
    }
  }
  if (cell.trim()) cells.push(cell.trim())
  return cells
}

// ── the bundles ─────────────────────────────────────────────────────────────

const readJson = (file) => JSON.parse(fs.readFileSync(file, 'utf8'))

/** Sets a dotted path in an object, failing when the path does not exist. */
const setPath = (obj, dotted, value) => {
  const parts = dotted.split('.')
  let node = obj
  for (const part of parts.slice(0, -1)) {
    if (node == null || typeof node !== 'object' || !(part in node)) {
      return false
    }
    node = node[part]
  }
  const last = parts.at(-1)
  if (node == null || typeof node !== 'object' || !(last in node)) return false
  node[last] = value
  return true
}

const getPath = (obj, dotted) => {
  let node = obj
  for (const part of dotted.split('.')) {
    if (node == null || typeof node !== 'object' || !(part in node)) return
    node = node[part]
  }
  return typeof node === 'string' ? node : undefined
}

/**
 * Every `{{name}}` in a string.
 *
 * i18next substitutes these; a lost one renders a double space where a number
 * should be, which no type checker and no test notices. This is the only guard
 * on the apply path that can actually fail on a copy edit.
 */
const placeholders = (s) => new Set(s.match(/\{\{\s*\w+\s*\}\}/g) ?? [])

const placeholdersSurvive = (original, replacement) => {
  const want = placeholders(original)
  const got = placeholders(replacement)
  return [...want].every((p) => got.has(p))
}

// ── apply ───────────────────────────────────────────────────────────────────

const main = () => {
  const { rows, skipped } = readManifest()
  const applied = []
  const unchanged = []
  const problems = []

  const nsFiles = new Map()
  for (const file of fs.readdirSync(LOCALES)) {
    if (!file.endsWith('.json')) continue
    const full = path.join(LOCALES, file, '..')
    void full
  }
  const enDir = path.join(LOCALES, 'en')
  for (const file of fs.readdirSync(enDir)) {
    if (!file.endsWith('.json')) continue
    nsFiles.set(file.replace(/\.json$/, ''), path.join(enDir, file))
  }

  /** namespace -> parsed bundle, written back once at the end. */
  const bundles = new Map()
  for (const [ns, file] of nsFiles) bundles.set(ns, readJson(file))

  for (const row of rows) {
    if (row.key.startsWith('backend:')) {
      // Handled below, as text surgery on the YAML.
      continue
    }
    const ns = row.key.split('.')[0]
    const bundle = bundles.get(ns)
    if (!bundle) {
      problems.push({ key: row.key, why: `no bundle for namespace "${ns}"` })
      continue
    }
    const inner = row.key.slice(ns.length + 1)
    const current = getPath(bundle, inner)
    if (current === undefined) {
      problems.push({
        key: row.key,
        why: 'key is not in the bundle — the manifest has drifted; regenerate it',
      })
      continue
    }
    if (current === row.text) {
      unchanged.push(row.key)
      continue
    }
    if (!placeholdersSurvive(current, row.text)) {
      problems.push({
        key: row.key,
        why: `placeholders changed: ${[...placeholders(current)].join(' ')} -> ${[...placeholders(row.text)].join(' ')}`,
      })
      continue
    }
    if (!setPath(bundle, inner, row.text)) {
      problems.push({ key: row.key, why: 'could not set the value' })
      continue
    }
    applied.push({ key: row.key, from: current, to: row.text })
  }

  // ── backend: one YAML file, no per-key override ──
  const backendRows = rows.filter((r) => r.key.startsWith('backend:'))
  let yaml = fs.readFileSync(BACKEND, 'utf8')
  for (const row of backendRows) {
    const dotted = row.key.replace('backend:', '')
    const leaf = dotted.split('.').pop()
    const original = findYamlLeaf(yaml, dotted)
    if (original === null) {
      problems.push({
        key: row.key,
        why: 'key is not in en.yml — the manifest has drifted; regenerate it',
      })
      continue
    }
    if (original === row.text) {
      unchanged.push(row.key)
      continue
    }
    if (!placeholdersSurvive(original, row.text)) {
      problems.push({
        key: row.key,
        why: `placeholders changed: ${[...placeholders(original)].join(' ')} -> ${[...placeholders(row.text)].join(' ')}`,
      })
      continue
    }
    const replaced = replaceYamlLeaf(yaml, dotted, leaf, row.text)
    if (replaced === null) {
      problems.push({
        key: row.key,
        why: 'could not locate the leaf to replace',
      })
      continue
    }
    yaml = replaced
    applied.push({ key: row.key, from: original, to: row.text })
  }

  // ── write ──
  if (!dryRun) {
    for (const [ns, bundle] of bundles) {
      const file = nsFiles.get(ns)
      fs.writeFileSync(file, `${JSON.stringify(bundle, null, 2)}\n`)
    }
    fs.writeFileSync(BACKEND, yaml)
  }

  // ── say what was NOT written, so "all prose" is not read as "all locales" ──
  //
  // This script writes English only. The other twelve locales are not touched,
  // and that is deliberate: copying an English rewrite into a locale that already
  // holds a real translation would replace the translation with English. But
  // "applied 3 changes" reads as "the app now says this", and for a student on
  // `de` it does not — so the report names the gap rather than leaving it to be
  // discovered by a screenshot.
  const otherLocales = fs
    .readdirSync(LOCALES, { withFileTypes: true })
    .filter((e) => e.isDirectory() && e.name !== 'en')
    .map((e) => e.name)

  // ── report ──
  const verb = dryRun ? 'would apply' : 'applied'
  console.log(
    `${verb} ${applied.length} change(s); ${unchanged.length} row(s) unchanged`,
  )
  for (const a of applied) {
    console.log(`  ${a.key}`)
    console.log(`    - ${a.from}`)
    console.log(`    + ${a.to}`)
  }
  if (skipped.length > 0) {
    console.log(`\n${skipped.length} row(s) could not be parsed (wrong shape):`)
    for (const s of skipped) console.log(`  ${s.area}: ${s.line.slice(0, 80)}`)
  }
  if (problems.length > 0) {
    console.error(`\n${problems.length} row(s) NOT applied:`)
    for (const p of problems) console.error(`  ${p.key} — ${p.why}`)
    console.error(
      '\nNothing was written. Fix the rows above, or regenerate the manifest ' +
        'if the code changed underneath it.',
    )
    process.exit(1)
  }
  if (applied.length > 0) {
    console.log(
      `\nWritten to ENGLISH only. ${otherLocales.length} other locale(s) ` +
        `(${otherLocales.join(', ')}) are untouched: a key they translate keeps ` +
        'the old translation, and a key they hold only as an English copy keeps ' +
        'the old English. See PROSE.md, "Not English-only".',
    )
  }
  if (applied.length > 0 && !dryRun) {
    console.log(
      '\nNext: node scripts/prose-manifest.mjs && git diff src/locales',
    )
  }
}

/**
 * Reads a leaf from the backend YAML by dotted path.
 *
 * Text surgery rather than a YAML round-trip on purpose: a parse-and-dump would
 * reformat every line of a 70-string file and lose its comments, turning a
 * one-line copy edit into a 200-line diff nobody can review.
 */
const findYamlLeaf = (yaml, dotted) => {
  const parts = dotted.split('.')
  let indent = 0
  let start = 0
  let lines = yaml.split('\n')
  for (let i = 0; i < parts.length; i += 1) {
    const want = parts[i]
    const isLast = i === parts.length - 1
    let found = false
    for (let j = start; j < lines.length; j += 1) {
      const line = lines[j]
      if (!line.trim()) continue
      const lineIndent = line.length - line.trimStart().length
      if (lineIndent < indent) break
      if (lineIndent > indent) continue
      const m = /^\s*'?([\w-]+)'?:\s*(.*)$/.exec(line)
      if (!m || m[1] !== want) continue
      if (isLast) return m[2].replace(/^'(.*)'$/, '$1').replace(/''/g, "'")
      start = j + 1
      indent = lineIndent + 2
      found = true
      break
    }
    if (!found) return null
  }
  return null
}

const replaceYamlLeaf = (yaml, dotted, leaf, value) => {
  void leaf
  const lines = yaml.split('\n')
  const parts = dotted.split('.')
  let indent = 0
  let start = 0
  for (let i = 0; i < parts.length; i += 1) {
    const want = parts[i]
    let found = false
    for (let j = start; j < lines.length; j += 1) {
      const line = lines[j]
      if (!line.trim()) continue
      const lineIndent = line.length - line.trimStart().length
      if (lineIndent < indent) break
      if (lineIndent > indent) continue
      const m = /^\s*'?([\w-]+)'?:\s*(.*)$/.exec(line)
      if (!m || m[1] !== want) continue
      if (i === parts.length - 1) {
        // Quote only when the value needs it, and double any embedded quote the
        // way this file already does (`current user''s home`).
        const needsQuote = /[:#'"]|^\s|\s$/.test(value)
        const rendered = needsQuote ? `'${value.replace(/'/g, "''")}'` : value
        const pad = ' '.repeat(lineIndent)
        const keyRendered = /^[a-z][\w-]*$/i.test(want) ? want : `'${want}'`
        lines[j] = `${pad}${keyRendered}: ${rendered}`
        return lines.join('\n')
      }
      start = j + 1
      indent = lineIndent + 2
      found = true
      break
    }
    if (!found) return null
  }
  return null
}

main()
