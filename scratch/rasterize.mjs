#!/usr/bin/env node
// THROWAWAY preview tool: rasterise glyph outlines so a human can LOOK at them.
// Not part of the product. Produces PPM (no deps) which we convert to PNG if
// ImageMagick is around.
//
// Input: a JSON file mapping name -> { viewBox:[w,h], shapes:[{d, fill?, stroke?, strokeWidth?}] }
// Curves are flattened by sampling, then filled with the even-odd rule at 4x4
// supersampling. Enough for eyeballing a wordmark.

import { readFileSync, writeFileSync } from 'node:fs'

const [, , specPath, outPath] = process.argv
const spec = JSON.parse(readFileSync(specPath, 'utf8'))
const SS = 4
const PX = Number(process.env.RASTER_PX || 4) // pixels per user unit

function parsePath(d) {
  const toks = d.match(/[MmLlHhVvCcSsQqTtAaZz]|-?\d*\.?\d+(?:e[-+]?\d+)?/gi) || []
  const subs = []
  let cur = null
  let x = 0
  let y = 0
  let sx = 0
  let sy = 0
  let prevCtrl = null
  let cmd = null
  let i = 0
  const num = () => Number(toks[i++])
  const isCmd = (t) => /^[A-Za-z]$/.test(t)
  const push = (px, py) => cur.pts.push([px, py])
  while (i < toks.length) {
    if (isCmd(toks[i])) cmd = toks[i++]
    const rel = cmd === cmd.toLowerCase()
    const C = cmd.toUpperCase()
    if (C === 'M') {
      const nx = num()
      const ny = num()
      x = rel ? x + nx : nx
      y = rel ? y + ny : ny
      cur = { pts: [] }
      subs.push(cur)
      push(x, y)
      sx = x
      sy = y
      cmd = rel ? 'l' : 'L'
    } else if (C === 'L') {
      const nx = num()
      const ny = num()
      x = rel ? x + nx : nx
      y = rel ? y + ny : ny
      push(x, y)
    } else if (C === 'H') {
      const nx = num()
      x = rel ? x + nx : nx
      push(x, y)
    } else if (C === 'V') {
      const ny = num()
      y = rel ? y + ny : ny
      push(x, y)
    } else if (C === 'C' || C === 'S') {
      let x1, y1
      if (C === 'C') {
        x1 = rel ? x + num() : lastNum()
      }
      // handled below explicitly
    } else {
      throw new Error('path command not supported: ' + cmd)
    }
  }
  return subs
}

// The generic parser above got fiddly; do a clean single pass with explicit cases.
function parse(d) {
  const toks = d.match(/[MmLlHhVvCcSsQqTtAaZz]|-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/gi) || []
  const subs = []
  let cur = { pts: [] }
  subs.push(cur)
  let x = 0, y = 0, sx = 0, sy = 0, pcx = null, pcy = null
  let cmd = null, i = 0
  const num = () => Number(toks[i++])
  const bez = (p0, p1, p2, p3) => {
    const N = 12
    for (let k = 1; k <= N; k++) {
      const t = k / N, u = 1 - t
      cur.pts.push([
        u * u * u * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t * t * t * p3[0],
        u * u * u * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t * t * t * p3[1],
      ])
    }
  }
  const quad = (p0, p1, p2) => {
    const N = 10
    for (let k = 1; k <= N; k++) {
      const t = k / N, u = 1 - t
      cur.pts.push([
        u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0],
        u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1],
      ])
    }
  }
  while (i < toks.length) {
    if (/^[A-Za-z]$/.test(toks[i])) cmd = toks[i++]
    const rel = cmd === cmd.toLowerCase()
    const C = cmd.toUpperCase()
    if (C === 'M') {
      const nx = num(), ny = num()
      x = rel ? x + nx : nx
      y = rel ? y + ny : ny
      cur = { pts: [[x, y]] }
      subs.push(cur)
      sx = x; sy = y
      cmd = rel ? 'l' : 'L'
      pcx = pcy = null
    } else if (C === 'L') {
      const nx = num(), ny = num()
      x = rel ? x + nx : nx
      y = rel ? y + ny : ny
      cur.pts.push([x, y])
      pcx = pcy = null
    } else if (C === 'H') {
      const nx = num()
      x = rel ? x + nx : nx
      cur.pts.push([x, y])
      pcx = pcy = null
    } else if (C === 'V') {
      const ny = num()
      y = rel ? y + ny : ny
      cur.pts.push([x, y])
      pcx = pcy = null
    } else if (C === 'C') {
      const a = Number(toks[i++]), b = Number(toks[i++])
      const c2 = Number(toks[i++]), d2 = Number(toks[i++])
      const e = Number(toks[i++]), f = Number(toks[i++])
      const p1 = rel ? [x + a, y + b] : [a, b]
      const p2 = rel ? [x + c2, y + d2] : [c2, d2]
      const p3 = rel ? [x + e, y + f] : [e, f]
      bez([x, y], p1, p2, p3)
      pcx = p2[0]; pcy = p2[1]
      x = p3[0]; y = p3[1]
    } else if (C === 'S') {
      const a = Number(toks[i++]), b = Number(toks[i++])
      const c2 = Number(toks[i++]), d2 = Number(toks[i++])
      const p1 = pcx === null ? [x, y] : [2 * x - pcx, 2 * y - pcy]
      const p2 = rel ? [x + a, y + b] : [a, b]
      const p3 = rel ? [x + c2, y + d2] : [c2, d2]
      bez([x, y], p1, p2, p3)
      pcx = p2[0]; pcy = p2[1]
      x = p3[0]; y = p3[1]
    } else if (C === 'Q') {
      const a = Number(toks[i++]), b = Number(toks[i++])
      const c2 = Number(toks[i++]), d2 = Number(toks[i++])
      const p1 = rel ? [x + a, y + b] : [a, b]
      const p2 = rel ? [x + c2, y + d2] : [c2, d2]
      quad([x, y], p1, p2)
      pcx = p1[0]; pcy = p1[1]
      x = p2[0]; y = p2[1]
    } else if (C === 'Z') {
      x = sx; y = sy
      cur.closed = true
      pcx = pcy = null
    } else {
      throw new Error('unsupported path command ' + cmd)
    }
  }
  return subs.filter((s) => s.pts.length > 1)
}

function strokePolygon(pts, w) {
  // A crude round-joined outline: offset each segment into a quad + a disc at
  // each vertex. Overlapping pieces are fine: coverage is computed per pixel and
  // the result is a binary-ish mark, and this is a preview, not a renderer.
  const out = []
  for (let k = 0; k < pts.length - 1; k++) {
    const [x0, y0] = pts[k]
    const [x1, y1] = pts[k + 1]
    const dx = x1 - x0, dy = y1 - y0
    const len = Math.hypot(dx, dy) || 1e-9
    const nx = (-dy / len) * (w / 2), ny = (dx / len) * (w / 2)
    out.push({ pts: [[x0 + nx, y0 + ny], [x1 + nx, y1 + ny], [x1 - nx, y1 - ny], [x0 - nx, y0 - ny]] })
  }
  for (const [cx, cy] of pts) {
    const pts2 = []
    for (let a = 0; a < 16; a++) {
      const t = (a / 16) * Math.PI * 2
      pts2.push([cx + Math.cos(t) * (w / 2), cy + Math.sin(t) * (w / 2)])
    }
    out.push({ pts: pts2 })
  }
  return out
}

function render(name, svgSpec) {
  const [vw, vh] = svgSpec.viewBox
  const W = Math.round(vw * PX), H = Math.round(vh * PX)
  const buf = new Uint8Array(W * H)
  // Collect polygons (already flattened).
  const polys = []
  const bg = svgSpec.background || null
  for (const sh of svgSpec.shapes) {
    if (sh.rect) {
      const [rx, ry, rw, rh] = sh.rect
      polys.push({ pts: [[rx, ry], [rx + rw, ry], [rx + rw, ry + rh], [rx, ry + rh]], evenOdd: false })
      continue
    }
    if (!sh.d) continue
    const subs = parse(sh.d)
    if (sh.stroke) {
      for (const s of subs) for (const q of strokePolygon(s.pts, sh.strokeWidth)) polys.push({ pts: q.pts, evenOdd: true })
    } else {
      // All subpaths of one `d` are one fill with the nonzero rule.
      polys.push({ ptsList: subs.map((s) => s.pts), evenOdd: false })
    }
  }
  // Supersample per pixel.
  const inv = 1 / (PX * SS)
  for (let py = 0; py < H; py++) {
    for (let px = 0; px < W; px++) {
      let cov = 0
      for (let sy = 0; sy < SS; sy++) {
        for (let sx = 0; sx < SS; sx++) {
          const ux = (px + (sx + 0.5) / SS) / PX
          const uy = (py + (sy + 0.5) / SS) / PX
          if (inside(ux, uy)) cov++
        }
      }
      // Store COVERAGE in the RED channel only. The writer below composites
      // `ink` over `ground` using R as alpha. Writing the same coverage into all
      // three channels made R and B disagree and produced a false "ink" mask.
      buf[py * W + px] = Math.round((cov / (SS * SS)) * 255)
      void inv
    }
  }
  function inside(ux, uy) {
    for (const p of polys) {
      const lists = p.ptsList ? [p.ptsList] : [[p.pts]]
      for (const list of lists) {
        let w = 0
        for (const poly of list) {
          let c = false
          for (let k = 0, j = poly.length - 1; k < poly.length; j = k++) {
            const [xi, yi] = poly[k], [xj, yj] = poly[j]
            if (yi > uy !== yj > uy && ux < ((xj - xi) * (uy - yi)) / (yj - yi) + xi) c = !c
          }
          if (c) w++
        }
        if (w > 0) return true
      }
    }
    return false
  }
  // Ink is white-ish so a dark ground shows it; PPM.
  const ink = svgSpec.ink || [230, 235, 245]
  const ground = bg || [26, 27, 38]
  const out = Buffer.alloc(W * H * 3)
  for (let k = 0; k < W * H; k++) {
    const a = buf[k] / 255
    for (let c = 0; c < 3; c++) {
      out[k * 3 + c] = Math.round(ground[c] * (1 - a) + ink[c] * a)
    }
  }
  // Coverage is recoverable from the red channel alone, but only if R moves with
  // alpha on this ground; assert it rather than hope.
  if (Math.abs(ink[0] - ground[0]) < 4) throw new Error('ink and ground share a red level')
  return { name, W, H, data: out }
}

const results = Object.entries(spec).map(([n, s]) => render(n, s))
// Stack vertically with 8px gutter.
const gutter = 12
const maxW = Math.max(...results.map((r) => r.W))
const totalH = results.reduce((s, r) => s + r.H + gutter, gutter)
const out = Buffer.alloc(maxW * totalH * 3, 0) // black ground
for (let k = 0; k < maxW * totalH; k++) { out[k * 3] = 8; out[k * 3 + 1] = 9; out[k * 3 + 2] = 14 }
let y0 = gutter
for (const r of results) {
  for (let y = 0; y < r.H; y++) {
    for (let x = 0; x < r.W; x++) {
      const src = (y * r.W + x) * 3
      const dst = ((y0 + y) * maxW + Math.floor((maxW - r.W) / 2) + x) * 3
      out[dst] = r.data[src]; out[dst + 1] = r.data[src + 1]; out[dst + 2] = r.data[src + 2]
    }
  }
  y0 += r.H + gutter
}
writeFileSync(outPath.replace(/\.png$/, '.ppm'), Buffer.concat([Buffer.from(`P6\n${maxW} ${totalH}\n255\n`), out]))
console.log('wrote', outPath.replace(/\.png$/, '.ppm'), maxW, totalH)
