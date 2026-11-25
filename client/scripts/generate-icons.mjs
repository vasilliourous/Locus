#!/usr/bin/env node
/**
 * Regenerate the application and tray marks from the palette.
 *
 * # Why this exists
 *
 * `client/src-tauri/icons/*` was the **Clash Verge Rev cat** — a grey-to-black
 * gradient with a white cat on it — in every release up to 3.4.4. The fork
 * re-identified its product, its app id and its wordmark, and left the mark a
 * student actually sees in the taskbar, the window frame, the dock and the tray
 * still belonging to a different program. `COLOURS.md` §8 recorded that as an
 * untaken brand decision; this file is that decision taken: every mark is the
 * FlClash glyph (`src/assets/image/glyph.svg`) on the palette's ground, and the
 * tray is FlClash's own four state assets.
 *
 * # The one rule this file obeys
 *
 * **It does not carry a colour of its own.** The ground is read out of
 * `src/pages/_theme.tsx`, which is `COLOURS.md` §1's single source, and the mark's
 * gradients live in `glyph.svg` because they are the artwork. A generator that
 * carried its own hex would be a second home for colour and would drift from
 * the app the moment the palette changed — and `check-consistency.sh` §49 would
 * fail on it, correctly, for the same reason.
 *
 * # Why a script and not `tauri icon`
 *
 * `tauri icon` takes one PNG and emits `.icns` and `.ico`. It does not produce
 * the Windows Store tile set, it cannot produce per-size art (it downscales a
 * 1024 master, which erodes the margin and fuses detail at 16 px), and it does
 * not touch the tray icons at all. This script renders each size at its own
 * resolution from the vector, and produces the tray status set too.
 *
 * # What it cannot produce, and does not pretend to
 *
 * `icon.icns` needs `iconutil`, which is macOS-only. It is generated from
 * `glyph.svg` by `.github/workflows/generate-macos-icons.yml` on a macOS runner
 * and committed by hand; the committed file is already the new mark.
 *
 * `Assets.car` is deliberately NOT produced and NOT bundled. Only Apple's `actool`
 * can author one, and a compiled catalog takes precedence over `icon.icns` inside
 * a bundle -- so a stale catalog made a correct `.icns` irrelevant. It was removed
 * from `bundle.icon` on 2026-11-23. See `src-tauri/icons/README.md`.
 *
 * # Usage
 *
 *     node client/scripts/generate-icons.mjs          # write the assets
 *     node client/scripts/generate-icons.mjs --check  # fail if they are stale
 *
 * `--check` is what a guard would call: it re-renders every file in memory and
 * compares against what is on disk, so "somebody edited `icon.png` by hand" and
 * "nobody re-ran the generator after a palette change" are both a failure
 * instead of a surprise on a student's taskbar.
 */

import { execFileSync } from 'node:child_process'
import {
  deflateSync as zlibDeflate,
  inflateSync as zlibInflate,
} from 'node:zlib'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const clientRoot = path.resolve(here, '..')
const iconsDir = path.join(clientRoot, 'src-tauri/icons')

/**
 * The favicon, and why it is not in `iconsDir`.
 *
 * `src/index.html` names it as `./assets/image/logo.ico` — a path Vite serves, so
 * the file has to stay where it is. It is generated with the rest rather than
 * left alone because it was the *same* Clash Verge cat in Clash Verge green, and
 * a mark that is regenerated everywhere except the browser tab is the kind of
 * omission that survives a redesign.
 */
const FAVICON = 'src/assets/image/logo.ico'

/**
 * The landing page's favicon, which lives outside the client entirely.
 *
 * `server/site/index.html` names `./favicon.ico`, and `05-caddy.sh` deploys
 * `server/site/` verbatim — so this file is a shipped surface with no build step
 * of its own. It was the Clash Verge cat in Clash Verge green long after the app
 * had moved on, because §54 of `check-consistency.sh` only looked inside
 * `client/`. Generating it here is what puts it under the same `--check`.
 */
const LANDING_FAVICON = '../server/site/favicon.ico'

/*
 * The output table, in one place and by literal name.
 *
 * `OUTPUTS` exists so that "what does this script write" is answerable by reading
 * one array — including from `check-consistency.sh` §54, which scans this file
 * for the names and fails when a config packages an icon that is not here. Before
 * it existed the favicon was written through the `FAVICON` constant, which meant
 * a grep for output names could not see it, and the guard either had to special-
 * case one file or under-report. One table, two readers.
 *
 * `TRAY_*` are loaded by `include_bytes!` in `core/tray/mod.rs` rather than named
 * by a packaging config, which is why they are listed separately: the difference
 * is a fact about how Rust includes them, not a taxonomy worth inventing.
 */
const OUTPUTS = {
  app: ['icon.png', '128x128.png', '128x128@2x.png', '32x32.png', 'icon.ico'],
  tiles: [
    'Square30x30Logo.png',
    'Square44x44Logo.png',
    'Square71x71Logo.png',
    'Square89x89Logo.png',
    'Square107x107Logo.png',
    'Square142x142Logo.png',
    'Square150x150Logo.png',
    'Square284x284Logo.png',
    'Square310x310Logo.png',
    'StoreLogo.png',
  ],
  /**
   * The tray, as FlClash ships it: a state × platform matrix, not one file per
   * state.
   *
   * FlClash keeps three directories (`windows/`, `unix/`, `macos/`) because the
   * three tray hosts want different formats and different sizes:
   *
   *   Windows   a multi-frame `.ico`; no per-scale PNGs exist upstream
   *   Linux     `.png` at 1x/2x/3x/4x — a HiDPI panel picks by scale factor and
   *             cannot use an `.ico`
   *   macOS     `.png` at 1x/2x/3x/4x, black-on-alpha for the template
   *
   * Collapsing that to one `.ico` per state (which this file used to do) ships
   * something that WORKS on every platform but is wrong on two: Linux and macOS
   * get an `.ico` decoded at one size and upscaled by the host. That is soft on a
   * 2x display and visibly so at the 18 pt a macOS menu bar uses.
   *
   * FlClash also ships macOS `status_4` but NOT `status_2`/`status_3` template
   * variants — a template discards colour, and its launcher picks `status_1` for
   * every non-safe-mode case on macOS (`lib/common/tray.dart`: the second
   * `switch` arm is `_isTemplate(monochrome) || !isStart => 1`). So a macOS
   * template set of `1` and `4` is complete, not truncated. This file matches
   * that rather than inventing two templated states upstream does not have.
   */
  tray: [
    // Windows: one ICO per state, frames rendered per size.
    'tray-icon.ico',
    'tray-icon-sys.ico',
    'tray-icon-tun.ico',
    'tray-icon-safe.ico',
    // Linux: PNG per state per scale factor.
    'tray-icon.png',
    'tray-icon@2x.png',
    'tray-icon@3x.png',
    'tray-icon@4x.png',
    'tray-icon-sys.png',
    'tray-icon-sys@2x.png',
    'tray-icon-sys@3x.png',
    'tray-icon-sys@4x.png',
    'tray-icon-tun.png',
    'tray-icon-tun@2x.png',
    'tray-icon-tun@3x.png',
    'tray-icon-tun@4x.png',
    'tray-icon-safe.png',
    'tray-icon-safe@2x.png',
    'tray-icon-safe@3x.png',
    'tray-icon-safe@4x.png',
    // macOS template: black-on-alpha, states 1 and 4 only (see the comment).
    'tray-icon-mono.png',
    'tray-icon-mono@2x.png',
    'tray-icon-mono@3x.png',
    'tray-icon-mono@4x.png',
    'tray-icon-safe-mono.png',
    'tray-icon-safe-mono@2x.png',
    'tray-icon-safe-mono@3x.png',
    'tray-icon-safe-mono@4x.png',
  ],
  /** Written to `FAVICON` and `LANDING_FAVICON`, both outside `iconsDir`. */
  favicon: ['logo.ico', 'favicon.ico'],
}
const themeFile = path.join(clientRoot, 'src/pages/_theme.tsx')

/**
 * The application mark, and the reason every raster is generated from it.
 *
 * `glyph.svg` is FlClash's glyph: two bars and a dot on a 30° lattice. It is the
 * only vector source for the app icon, the Windows tile set, the tray and the
 * favicon. It carries its own three gradients (top bar, middle bar, dot), which
 * are the artwork rather than a palette — the app draws them unchanged on every
 * surface, so this file does not recolour them.
 *
 * The previous mark was the Clash Verge Rev cat, then a recoloured Locus square.
 * Both are gone: `icons/README.md` records what replaced what and when.
 */
const glyphFile = path.join(clientRoot, 'src/assets/image/glyph.svg')

/**
 * The tray state glyphs, `status_1`…`status_4`, copied verbatim from FlClash.
 *
 * FlClash draws its tray from four assets rather than recolouring one, and the
 * mapping is FlClash's own (`lib/common/tray.dart`):
 *
 *   1 — not started, or macOS templated (the idle mark)
 *   2 — running, TUN off
 *   3 — running, TUN on
 *   4 — safe mode
 *
 * States 1–3 share one silhouette and differ only by colour; state 4 is a
 * different shape entirely. That asymmetry is the upstream design and is kept
 * rather than tidied, for the same reason the frozen wire names are kept: the
 * point of using this icon set is that it is this icon set.
 */
const trayStatusDir = path.join(clientRoot, 'src/assets/image/mark')

/* ────────────────────────────────────────────────────────────────────────────
 * The palette, read from its one home.
 * ──────────────────────────────────────────────────────────────────────────── */

/**
 * One value out of `CHRYSALIS_COLORS` in `_theme.tsx`.
 *
 * A regex rather than importing the module: `_theme.tsx` imports
 * `@/utils/get-system`, which resolves through a Vite alias that plain Node does
 * not have. Parsing the object literal is honest about what is being read — a
 * `key: '#RRGGBB'` pair, which is the exact shape `tests/theme-colors.test.ts`
 * pins — and a missing key throws rather than defaulting, because a silent
 * fallback colour is the defect this whole file exists to avoid.
 */
const paletteValue = (source, key) => {
  const match = source.match(new RegExp(`\\b${key}:\\s*'(#[0-9A-Fa-f]{6})'`))
  if (!match) {
    throw new Error(
      `generate-icons: ${path.basename(themeFile)} has no string colour for \`${key}\`. ` +
        'The palette is the only source for a colour (/COLOURS.md §1); restore it there.',
    )
  }
  return match[1]
}

const themeSource = readFileSync(themeFile, 'utf8')

const PALETTE = {
  // The app icon's rounded-square ground. The mark itself is the artwork in
  // `glyph.svg` and is never recoloured — see that file's provenance comment.
  background: paletteValue(themeSource, 'background'),
  // Black, for the macOS template set only.
  //
  // A template image is tinted by the OS from its ALPHA channel, so this RGB is
  // discarded and cannot affect what a student sees. It is BLACK rather than
  // white because upstream's own macOS assets are black-on-alpha
  // (`assets/images/tray/macos/status_1.png`), and matching the source is what
  // makes a future diff against it meaningful. Declared here rather than inlined
  // at the draw call so this file has exactly one colour it did not read from the
  // palette, and a reader can see why.
  templateInk: '#000000',
}

/* ────────────────────────────────────────────────────────────────────────────
 * A minimal PNG codec.
 *
 * Deliberately in-file rather than a dependency (`pngjs`, `sharp`, `canvas`):
 * this is a build script that runs on every platform in three CI jobs, and the
 * only operations it needs are "write RGBA rows" and "read the alpha channel of
 * a PNG rsvg-convert just produced". A dependency for that is a supply-chain and
 * build-time cost for forty lines of format that has not changed since 1996.
 *
 * Scope, stated so nobody extends it by surprise: 8-bit, non-interlaced, colour
 * type 6 (RGBA) encode; decode accepts 6 (RGBA) and 2 (RGB) so it can read
 * rsvg-convert's output whichever it chooses.
 * ────────── */

const PNG_SIGNATURE = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
])

const CRC_TABLE = (() => {
  const table = new Int32Array(256)
  for (let n = 0; n < 256; n += 1) {
    let c = n
    for (let k = 0; k < 8; k += 1) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    }
    table[n] = c
  }
  return table
})()

const crc32 = (buffer) => {
  let crc = -1
  for (const byte of buffer) {
    crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8)
  }
  return (crc ^ -1) >>> 0
}

/** One PNG chunk: length, type, payload, CRC over type+payload. */
const chunk = (type, payload) => {
  const length = Buffer.alloc(4)
  length.writeUInt32BE(payload.length, 0)
  const typeAndPayload = Buffer.concat([Buffer.from(type, 'ascii'), payload])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(typeAndPayload), 0)
  return Buffer.concat([length, typeAndPayload, crc])
}

/** Encodes RGBA pixels as a PNG. `data` is already width*height*4 bytes. */
const encodePng = (width, height, data) => {
  const raw = Buffer.alloc(height * (width * 4 + 1))
  for (let y = 0; y < height; y += 1) {
    const rowStart = y * (width * 4 + 1)
    raw[rowStart] = 0 // filter type 0 (None): these images are flat colour
    data.copy(raw, rowStart + 1, y * width * 4, (y + 1) * width * 4)
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0)
  header.writeUInt32BE(height, 4)
  header.writeUInt8(8, 8) // bit depth
  header.writeUInt8(6, 9) // colour type: RGBA
  header.writeUInt8(0, 10) // compression: deflate
  header.writeUInt8(0, 11) // filter: adaptive
  header.writeUInt8(0, 12) // interlace: none
  return Buffer.concat([
    PNG_SIGNATURE,
    chunk('IHDR', header),
    chunk('IDAT', zlibDeflate(raw)),
    chunk('IEND', Buffer.alloc(0)),
  ])
}

/**
 * "Reads" a PNG, which here means: pulls its pixels into RGBA.
 *
 * Throws on anything outside the documented scope rather than guessing — a
 * silent misparse would ship a corrupted icon, and the whole point of rendering
 * from the vector is that the result is checkable.
 */
const decodePng = (buffer) => {
  if (!buffer.subarray(0, 8).equals(PNG_SIGNATURE)) {
    throw new Error('generate-icons: rsvg-convert did not produce a PNG')
  }
  let offset = 8
  let width = 0
  let height = 0
  let colourType = 0
  const idat = []
  while (offset < buffer.length) {
    const length = buffer.readUInt32BE(offset)
    const type = buffer.toString('ascii', offset + 4, offset + 8)
    const payload = buffer.subarray(offset + 8, offset + 8 + length)
    if (type === 'IHDR') {
      width = payload.readUInt32BE(0)
      height = payload.readUInt32BE(4)
      const bitDepth = payload.readUInt8(8)
      colourType = payload.readUInt8(9)
      const interlace = payload.readUInt8(12)
      if (bitDepth !== 8 || interlace !== 0) {
        throw new Error(
          `generate-icons: unsupported PNG (depth ${bitDepth}, interlace ${interlace}); ` +
            'this codec handles 8-bit non-interlaced only',
        )
      }
    } else if (type === 'IDAT') {
      idat.push(payload)
    } else if (type === 'IEND') {
      break
    }
    offset += 12 + length
  }
  if (width === 0 || height === 0)
    throw new Error('generate-icons: PNG had no IHDR')

  const channels = colourType === 6 ? 4 : colourType === 2 ? 3 : 0
  if (channels === 0) {
    throw new Error(`generate-icons: unsupported PNG colour type ${colourType}`)
  }

  const raw = zlibInflate(Buffer.concat(idat))
  const stride = width * channels
  const out = Buffer.alloc(width * height * 4)
  const previous = Buffer.alloc(stride)
  const current = Buffer.alloc(stride)
  let cursor = 0
  for (let y = 0; y < height; y += 1) {
    const filter = raw[cursor]
    cursor += 1
    raw.copy(current, 0, cursor, cursor + stride)
    cursor += stride
    // Undo the row filter. rsvg-convert emits at least `Paeth` rows, so all five
    // have to be handled even though this file only ever *writes* filter 0.
    for (let i = 0; i < stride; i += 1) {
      const left = i >= channels ? current[i - channels] : 0
      const up = previous[i]
      const upLeft = i >= channels ? previous[i - channels] : 0
      let value = current[i]
      if (filter === 1) value += left
      else if (filter === 2) value += up
      else if (filter === 3) value += Math.floor((left + up) / 2)
      else if (filter === 4) {
        const p = left + up - upLeft
        const pa = Math.abs(p - left)
        const pb = Math.abs(p - up)
        const pc = Math.abs(p - upLeft)
        value += pa <= pb && pa <= pc ? left : pb <= pc ? up : upLeft
      }
      current[i] = value & 0xff
    }
    for (let x = 0; x < width; x += 1) {
      const from = x * channels
      const to = (y * width + x) * 4
      out[to] = current[from]
      out[to + 1] = current[from + 1]
      out[to + 2] = current[from + 2]
      out[to + 3] = channels === 4 ? current[from + 3] : 255
    }
    current.copy(previous)
  }
  return { width, height, data: out }
}

/* ────────────────────────────────────────────────────────────────────────────
 * Rendering.
 * ──────────────────────────────────────────────────────────────────────────── */

const tmpRoot = mkdtempSync(path.join(tmpdir(), 'chrysalis-icons-'))

/**
 * The glyph's ink bounds, in its own 240-unit viewBox.
 *
 * Measured from a raster of the vector rather than trusted from the file's
 * nominal `width`/`height`: the bars are strokes with round caps, so the ink
 * overflows the path coordinates and a naive `viewBox` fit clips the dot. Reading
 * the measured box keeps the mark centred instead of guessing from `240`.
 */
const GLYPH_INK = { x0: 36.09, y0: 7.97, x1: 203.44, y1: 231.09 }

/** Rasterises any SVG string to a PNG through rsvg-convert. */
const rasterise = (svg, box, tag) => {
  const svgPath = path.join(tmpRoot, `${tag}.svg`)
  const pngPath = path.join(tmpRoot, `${tag}.png`)
  writeFileSync(svgPath, svg)
  execFileSync('rsvg-convert', [
    '-w',
    String(box),
    '-h',
    String(box),
    svgPath,
    '-o',
    pngPath,
  ])
  return decodePng(readFileSync(pngPath))
}

/**
 * The app icon: the mark on a rounded-square ground, at one size.
 *
 * This is FlClash's own app-icon framing, measured from its 1024 master rather
 * than invented: the ground occupies 84.8 % of the canvas, leaving a symmetric
 * page margin, and the ink occupies 51 % of the ground's width by 68 % of its
 * height. `inset` grows the mark slightly as the canvas shrinks, because a
 * glyph that keeps its full margin at 16 px becomes a smudge.
 *
 * The corner radius is 22.4 % of the ground, which is what makes it read as an
 * app tile rather than a square photograph.
 */
const renderAppIcon = (size, { ground, inset }) => {
  const square = size * (inset ?? 0.848)
  const radius = square * 0.224
  const margin = (size - square) / 2
  const ink = Math.round(square * 0.72)
  const inkOffset = (size - ink) / 2
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">`,
    `<rect x="${margin.toFixed(3)}" y="${margin.toFixed(3)}" width="${square.toFixed(3)}" height="${square.toFixed(3)}" rx="${radius.toFixed(3)}" ry="${radius.toFixed(3)}" fill="${ground}"/>`,
  ]
  // The mark is composed on top of its own ground, so it is drawn from the
  // same `glyph.svg` body the tray and favicon use — one artwork, four callers.
  const glyphBody = readFileSync(glyphFile, 'utf8')
    .replace(/^[\s\S]*?<svg[^>]*>/, '')
    .replace(/<\/svg>\s*$/, '')
    .trim()
  const inkW = GLYPH_INK.x1 - GLYPH_INK.x0
  const inkH = GLYPH_INK.y1 - GLYPH_INK.y0
  const scale = Math.min(ink / inkW, ink / inkH)
  const tx = inkOffset + (ink - inkW * scale) / 2 - GLYPH_INK.x0 * scale
  const ty = inkOffset + (ink - inkH * scale) / 2 - GLYPH_INK.y0 * scale
  svg.push(
    `<g transform="translate(${tx.toFixed(4)} ${ty.toFixed(4)}) scale(${scale.toFixed(6)})">`,
    glyphBody,
    '</g>',
    '</svg>',
  )
  return encodePng(
    size,
    size,
    rasterise(svg.join(''), size, `app-${size}`).data,
  )
}

const hexToRgb = (hex) => [
  parseInt(hex.slice(1, 3), 16),
  parseInt(hex.slice(3, 5), 16),
  parseInt(hex.slice(5, 7), 16),
]

/**
 * A tray state glyph, rasterised at one size.
 *
 * FlClash ships four states as four SVGs rather than one recoloured shape, and
 * its tray picks between them by run state (`lib/common/tray.dart`). Copying the
 * assets verbatim is the whole point of adopting this set, so this renders the
 * named asset and adds nothing of its own.
 *
 * Rendered at the full canvas rather than inset: a menu-bar or taskbar glyph is
 * already small, and the upstream assets are drawn to the edge of their box.
 */
const renderTrayStatus = (size, status) => {
  const source = readFileSync(
    path.join(trayStatusDir, `status_${status}.svg`),
    'utf8',
  )
  return encodePng(
    size,
    size,
    rasterise(source, size, `tray-${status}-${size}`).data,
  )
}

/**
 * A tray state as a macOS template: black on transparent, at one size.
 *
 * A template image is tinted by the OS from its ALPHA channel, so the ink colour
 * is discarded and only the shape matters. It is BLACK rather than white to
 * match upstream's own macOS assets, which are black on transparency
 * (`assets/images/tray/macos/status_1.png`). White is the other convention and
 * looks identical on screen, but matching upstream means the asset in this repo
 * and the asset it came from are the same thing.
 *
 * The per-segment gradients are flattened to one ink: a template cannot express a
 * gradient, and leaving `url(#top)` in would render as black-to-transparent.
 */
const renderTemplateGlyph = (size, status) => {
  const source = readFileSync(
    path.join(trayStatusDir, `status_${status}.svg`),
    'utf8',
  )
  // The source's OWN viewBox, not an assumed one. `status_4` is a Material
  // Symbols glyph (`viewBox="100 -854 760 760"`) while states 1-3 are the mark
  // (`0 0 240 240`); hard-coding either renders the other off-canvas and produces
  // a blank template — which is exactly what happened before this was read.
  const viewBox = source.match(/viewBox="([^"]+)"/)
  if (!viewBox) {
    throw new Error(`generate-icons: status_${status}.svg has no viewBox`)
  }
  const body = source
    .replace(/^[\s\S]*?<svg[^>]*>/, '')
    .replace(/<\/svg>\s*$/, '')
    .replace(/fill="url\(#[^)]*\)"/g, `fill="${PALETTE.templateInk}"`)
    .replace(/stroke="url\(#[^)]*\)"/g, `stroke="${PALETTE.templateInk}"`)
    .trim()
  const svg = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="${viewBox[1]}">`,
    body,
    '</svg>',
  ].join('')
  return encodePng(
    size,
    size,
    rasterise(svg, size, `template-${status}-${size}`).data,
  )
}

/* ────────────────────────────────────────────────────────────────────────────
 * The ICO container.
 * ──────────────────────────────────────────────────────────────────────────── */

/**
 * Writes a multi-size `.ico` whose frames are PNGs.
 *
 * Windows Vista and later accept PNG-compressed frames, and the set these
 * replace already uses them — so this is the same container, not a new format.
 * Every frame is rendered at its own size (see `renderAppIcon`), which is what
 * keeps a 16 px icon a mark rather than a grey square.
 */
const buildIco = (frames) => {
  const count = frames.length
  const header = Buffer.alloc(6)
  header.writeUInt16LE(0, 0) // reserved
  header.writeUInt16LE(1, 2) // type: icon
  header.writeUInt16LE(count, 4)

  const directory = Buffer.alloc(16 * count)
  let offset = 6 + 16 * count
  frames.forEach(({ size, png }, index) => {
    const entry = index * 16
    // 256 is encoded as 0 in a directory entry — the field is one byte.
    directory.writeUInt8(size >= 256 ? 0 : size, entry)
    directory.writeUInt8(size >= 256 ? 0 : size, entry + 1)
    directory.writeUInt8(0, entry + 2) // palette size
    directory.writeUInt8(0, entry + 3) // reserved
    directory.writeUInt16LE(1, entry + 4) // colour planes
    directory.writeUInt16LE(32, entry + 6) // bits per pixel
    directory.writeUInt32LE(png.length, entry + 8)
    directory.writeUInt32LE(offset, entry + 12)
    offset += png.length
  })

  return Buffer.concat([header, directory, ...frames.map((frame) => frame.png)])
}

/* ────────────────────────────────────────────────────────────────────────────
 * The sets.
 * ──────────────────────────────────────────────────────────────────────────── */

/**
 * The frames every app icon carries.
 *
 * `20` and `24` are not in the previous `icon.ico` (which had 16/32/48/64/128/256)
 * and are added because they are the sizes Windows uses for the taskbar at 125 %
 * and 150 % scaling — the two settings where a missing frame is silently
 * replaced by a downscale of the nearest larger one. The failure that looked like
 * "the taskbar icon is still the old one" is much more often "the taskbar icon is
 * a blurry downscale" than an install problem.
 */
const APP_FRAMES = [16, 20, 24, 32, 48, 64, 128, 256]
const TRAY_FRAMES = [16, 20, 24, 32, 48, 64, 256]

/**
 * The 1x tray size, which every scale factor multiplies.
 *
 * `18` is FlClash's macOS menu-bar size (`size: isMacOS ? 18 : 16` in
 * `lib/common/tray.dart`) and it matches its own asset ladder exactly:
 * `macos/status_1.png` is 18×18 and `@2x`/`@3x`/`@4x` are 36/54/72. Using the
 * same base means this repo's `@Nx` files and upstream's are the same pixels, so
 * a future diff against the source is meaningful.
 *
 * Windows does not use this: it reads `TRAY_FRAMES` out of the `.ico`, because a
 * taskbar asks for 16/20/24 px regardless of a 4x display.
 */
const TRAY_BASE = 18

/** The Windows Store / MSIX tile set, in the sizes `tauri.conf.json`'s icons use. */
const TILE_SIZES = {
  'Square30x30Logo.png': 30,
  'Square44x44Logo.png': 44,
  'Square71x71Logo.png': 71,
  'Square89x89Logo.png': 89,
  'Square107x107Logo.png': 107,
  'Square142x142Logo.png': 142,
  'Square150x150Logo.png': 150,
  'Square284x284Logo.png': 284,
  'Square310x310Logo.png': 310,
  'StoreLogo.png': 50,
}

/**
 * How much of the canvas the rounded-square ground may occupy, by size.
 *
 * The 0.848 default is FlClash's own app-icon margin, measured from its 1024
 * master. Smaller sizes grow the tile slightly — a ground that keeps an 85 %
 * margin at 16 px is 13 px of tile, and the rounded corners then eat most of it.
 */
/**
 * How much of the canvas the rounded-square ground may occupy, by size.
 *
 * The 0.848 default is FlClash's own app-icon margin, measured from its 1024
 * master. Smaller sizes grow the tile slightly — a ground that keeps an 85 %
 * margin at 16 px is 13 px of tile, and the rounded corners then eat most of it.
 */
const insetFor = (size) => {
  if (size <= 20) return 0.98
  if (size <= 32) return 0.94
  if (size <= 64) return 0.9
  return 0.848
}

/**
 * Every file this generator owns, as `name → bytes`.
 *
 * Built in memory so `--check` can compare without touching the tree, and so a
 * failure part-way through leaves nothing half-written.
 */
const buildAll = () => {
  const files = new Map()

  // App icon: the mark on its rounded-square ground, at each frame size.
  const appPng = (size) =>
    renderAppIcon(size, { ground: PALETTE.background, inset: insetFor(size) })

  const appIcon = buildIco(
    APP_FRAMES.map((size) => ({ size, png: appPng(size) })),
  )

  // Written from `OUTPUTS` rather than by repeating the names: a name in the
  // table with no builder below throws, and a builder with no table entry
  // produces a file nothing declares. That failure direction is the point —
  // §54 of `check-consistency.sh` reads the table to know what ships.
  const builders = {
    'icon.png': () => appPng(512),
    '128x128.png': () => appPng(128),
    '128x128@2x.png': () => appPng(256),
    '32x32.png': () => appPng(32),
    'icon.ico': () => appIcon,

    // The document's favicon. `src/index.html` points at
    // `assets/image/logo.ico`, which was the same Clash Verge cat in Clash Verge
    // green — so the tab was the one place a redesign would have forgotten.
    // Keyed by its BASE NAME, matching `OUTPUTS.favicon`, so the table and the
    // builder use the same token and `writeTargetFor` decides the directory.
    [path.basename(FAVICON)]: () => appIcon,
  }

  // Tray: FlClash's four state assets, rendered into the per-platform matrix
  // upstream ships. These are NOT recoloured — `status_1` and `status_2` differ
  // by colour because upstream decided they should, and repainting them to the
  // palette would be a different set of icons wearing this one's shapes.
  //
  // Wire name → upstream asset:
  //
  //   tray-icon{,-@Nx}        status_1  idle / nothing engaged
  //   tray-icon-sys{,-@Nx}    status_2  running, TUN off (system proxy)
  //   tray-icon-tun{,-@Nx}    status_3  running, TUN on
  //   tray-icon-safe{,-@Nx}   status_4  safe mode
  //   tray-icon-mono{,-@Nx}   status_1  macOS template
  //   tray-icon-safe-mono*    status_4  macOS template
  const trayIco = (status) =>
    buildIco(
      TRAY_FRAMES.map((size) => ({
        size,
        png: renderTrayStatus(size, status),
      })),
    )

  /** `.png` at a scale factor, for the Linux and macOS tray hosts. */
  const trayPng = (status, scale, template) => {
    const size = TRAY_BASE * scale
    return template
      ? renderTemplateGlyph(size, status)
      : renderTrayStatus(size, status)
  }

  // One entry per output name: which state, and whether it is a template.
  // Written as data so the name↔asset mapping is readable in one place and
  // `OUTPUTS.tray` can be checked against it.
  const traySpecs = (() => {
    const specs = new Map()
    const states = {
      'tray-icon': 1,
      'tray-icon-sys': 2,
      'tray-icon-tun': 3,
      'tray-icon-safe': 4,
    }
    for (const [stem, status] of Object.entries(states)) {
      // Windows: ICO, frames at every size Windows asks for.
      specs.set(`${stem}.ico`, { kind: 'ico', status })
      // Linux: PNG per scale factor.
      for (const scale of [1, 2, 3, 4]) {
        const suffix = scale === 1 ? '' : `@${scale}x`
        specs.set(`${stem}${suffix}.png`, { kind: 'png', status, scale })
      }
    }
    // macOS template: states 1 and 4 only, matching upstream's macos/ directory.
    for (const [stem, status] of [
      ['tray-icon-mono', 1],
      ['tray-icon-safe-mono', 4],
    ]) {
      for (const scale of [1, 2, 3, 4]) {
        const suffix = scale === 1 ? '' : `@${scale}x`
        specs.set(`${stem}${suffix}.png`, {
          kind: 'png',
          status,
          scale,
          template: true,
        })
      }
    }
    return specs
  })()

  // The table and the builder must agree in BOTH directions; a name in one and
  // not the other is the failure that makes a declared output never get written.
  for (const name of OUTPUTS.tray) {
    if (!traySpecs.has(name)) {
      throw new Error(
        `generate-icons: OUTPUTS.tray names \`${name}\` but traySpecs has no entry. ` +
          'Add the state/scale, or drop it from the table.',
      )
    }
  }
  for (const name of traySpecs.keys()) {
    if (!OUTPUTS.tray.includes(name)) {
      throw new Error(
        `generate-icons: traySpecs builds \`${name}\` but OUTPUTS.tray does not declare it. ` +
          'A file written but undeclared is one the provenance check cannot see.',
      )
    }
  }

  const wanted = [
    ...OUTPUTS.app,
    ...OUTPUTS.tiles,
    ...OUTPUTS.tray,
    ...OUTPUTS.favicon,
  ]
  for (const name of wanted) {
    if (TILE_SIZES[name] !== undefined) {
      files.set(name, appPng(TILE_SIZES[name]))
      continue
    }
    const spec = traySpecs.get(name)
    if (spec !== undefined) {
      files.set(
        name,
        spec.kind === 'ico'
          ? trayIco(spec.status)
          : trayPng(spec.status, spec.scale, spec.template === true),
      )
      continue
    }
    if (name === path.basename(LANDING_FAVICON)) {
      files.set(name, appIcon)
      continue
    }
    const build = builders[name]
    if (!build) {
      throw new Error(
        `generate-icons: OUTPUTS names \`${name}\` but no builder produces it. ` +
          'Add one, or drop it from the table — a declared output that is never ' +
          'written makes the icon-provenance check pass on a file that does not exist.',
      )
    }
    files.set(name, build())
  }

  return files
}

/* ────────────────────────────────────────────────────────────────────────────
 * Entry point.
 * ──────────────────────────────────────────────────────────────────────────── */
const main = () => {
  const check = process.argv.includes('--check')
  const files = buildAll()

  const stale = []
  for (const [name, bytes] of files) {
    const target =
      name === path.basename(FAVICON)
        ? path.join(clientRoot, FAVICON)
        : name === path.basename(LANDING_FAVICON)
          ? path.resolve(clientRoot, LANDING_FAVICON)
          : path.join(iconsDir, name)
    if (check) {
      let current = null
      try {
        current = readFileSync(target)
      } catch {
        // Missing is stale, not an error: the fix is the same.
      }
      if (!current || !current.equals(bytes)) {
        stale.push(name)
      }
      continue
    }
    writeFileSync(target, bytes)
  }

  if (check) {
    if (stale.length > 0) {
      console.error(
        `generate-icons: ${stale.length} icon(s) do not match the generator:\n` +
          stale.map((name) => `  - ${name}`).join('\n') +
          '\n\nRun `node client/scripts/generate-icons.mjs` and commit the result.',
      )
      process.exitCode = 1
      return
    }
    console.log(`generate-icons: ${files.size} file(s) match the generator`)
    return
  }

  console.log(
    `generate-icons: wrote ${files.size} file(s) to ${path.relative(process.cwd(), iconsDir)}`,
  )
}

try {
  main()
} finally {
  rmSync(tmpRoot, { recursive: true, force: true })
}
