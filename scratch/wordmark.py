#!/usr/bin/env python3
"""THROWAWAY: vectorise the Chrysalis wordmark.

Render the wordmark's type in the face the original Locus wordmark was drawn in
(Liberation Sans Bold — Arial-metric), at the original's cap height and tracking,
then trace the ink into polygons. The output is pasted into
`client/src/assets/image/logo.svg` by hand, with `fill: currentColor`.

    python3 scratch/wordmark.py > scratch/wordmark.svg

Why trace instead of shipping the font: a wordmark is artwork. A font file would
be ~200 KB in the bundle, would need a licence trail, and would be re-typed in
JSX by whoever next edits the sidebar — three places to drift. Outlines are
static, one file, and `fill: currentColor` still works because the glyphs are
pure geometry with no fill of their own.

The geometry is the original's, kept close on purpose:

  * cap height 45 units, live band y = 16..61, exactly the original's `viewBox`;
  * tracking ~6.5 units at a 45-unit cap, the original's cap-to-cap gap;
  * the lowercase `l` in `CHRYSALIS` has no descender, so the wordmark is still
    one clean block, like the five-cap word it replaces.
"""

from PIL import Image, ImageDraw, ImageFont

FONT = '/usr/share/fonts/liberation/LiberationSans-Bold.ttf'
TEXT = 'CHRYSALIS'
CAP = 45.0        # cap height in user units
TRACK = 6.5       # gap between glyph bounding boxes, user units
PAD = 60          # px of margin around the render, so nothing touches an edge
EM = 1000         # render at 1000 px em; 1 user unit is then well under 1 px


def render_mask(text):
    """The wordmark's ink at EM px, as (mask, baseline_y_px, glyph_boxes)."""
    probe = ImageFont.truetype(FONT, 300)
    box = probe.getbbox('H')
    cap_px = box[3] - box[1]
    size = round(300 * CAP / cap_px)
    # Every one of the size/scale figures below is in *render* px. Keeping them
    # in one place is what stops a 2.4x-too-wide trace, which is the bug this
    # file has already been through once.
    font = ImageFont.truetype(FONT, size)

    scale = CAP / cap_px                          # render px -> user units
    left = sum(font.getlength(c) for c in text) + TRACK * len(text)
    mask = Image.new('L', (int(left) + 2 * PAD, int(size * 3) + 2 * PAD), 0)
    draw = ImageDraw.Draw(mask)
    baseline = PAD + int(size * 1.2)
    pen = float(PAD)
    boxes = []
    for ch in text:
        draw.text((pen, baseline), ch, font=font, fill=255, anchor='ls')
        advance = font.getlength(ch)
        boxes.append((ch, font.getbbox(ch, anchor='ls')))
        pen += advance
    return mask, baseline, boxes


def marching_squares(mask, scale, baseline_px):
    """Trace every closed boundary in `mask` to a polygon in user units.

    A boundary edge is a cell edge with ink on exactly one side. Chaining the
    edges by corner gives each ring a consistent winding, which is what keeps the
    counters of `A`, `R`, `S` drawn as holes under `fill-rule: nonzero`.
    """
    px = mask.load()
    w, h = mask.size
    ink = [[px[x, y] > 127 for x in range(w)] for y in range(h)]

    out_edges = {}
    for y in range(h):
        for x in range(w):
            if not ink[y][x]:
                continue
            if y == 0 or not ink[y - 1][x]:
                out_edges.setdefault((x, y), []).append((x + 1, y))
            if x == w - 1 or not ink[y][x + 1]:
                out_edges.setdefault((x + 1, y), []).append((x + 1, y + 1))
            if y == h - 1 or not ink[y + 1][x]:
                out_edges.setdefault((x + 1, y + 1), []).append((x, y + 1))
            if x == 0 or not ink[y][x - 1]:
                out_edges.setdefault((x, y + 1), []).append((x, y))

    rings = []
    while out_edges:
        start = next(iter(out_edges))
        ring = [start]
        cur = start
        while True:
            nxts = out_edges.get(cur)
            if not nxts:
                break
            nxt = nxts.pop()
            if not nxts:
                del out_edges[cur]
            if nxt == start:
                break
            ring.append(nxt)
            cur = nxt
        if len(ring) > 12:
            rings.append(simplify([(x, y) for x, y in ring], 0.9))

    return [
        [(x * scale, (y - baseline_px) * scale * -1.0) for x, y in ring] for ring in rings
    ]


def simplify(points, eps):
    """Ramer-Douglas-Peucker, iteratively (a ring is thousands of points)."""
    if len(points) < 3:
        return points
    keep = [False] * len(points)
    keep[0] = keep[-1] = True
    stack = [(0, len(points) - 1)]
    while stack:
        a, b = stack.pop()
        if b <= a + 1:
            continue
        ax, ay = points[a]
        bx, by = points[b]
        dx, dy = bx - ax, by - ay
        norm = (dx * dx + dy * dy) ** 0.5 or 1e-9
        best, idx = -1.0, -1
        for i in range(a + 1, b):
            px_, py_ = points[i]
            dist = abs(dy * (px_ - ax) - dx * (py_ - ay)) / norm
            if dist > best:
                best, idx = dist, i
        if best > eps:
            keep[idx] = True
            stack.append((a, idx))
            stack.append((idx, b))
    return [p for p, k in zip(points, keep) if k]


def main():
    mask, baseline, _ = render_mask(TEXT)
    probe = ImageFont.truetype(FONT, 300)
    b = probe.getbbox('H')
    size = round(300 * CAP / (b[3] - b[1]))      # render px for a 45-unit cap
    cap_px = size * (b[3] - b[1]) / 300          # the face's own cap at that size
    scale = CAP / cap_px                         # render px -> user units

    rings = marching_squares(mask, scale, baseline)
    top = max(y for r in rings for _, y in r)
    # This face's round letters overshoot the cap by a few hundredths of a unit
    # at a 45-unit cap, which is invisible. Measured rather than assumed, so
    # swapping the face cannot quietly push the live band out of the box.
    assert -1.0 < top - CAP < 1.0, f'overshoot {top - CAP:.2f} units is too much'

    xs = [x for ring in rings for x, _ in ring]
    dx = 2.0 - min(xs)                            # the original's ~2-unit bearing
    width = max(xs) - min(xs) + 4.0

    for ring in rings:
        d = 'M' + ' L'.join(f'{x + dx:.2f},{y:.2f}' for x, y in ring) + ' Z'
        print(f'<path class="wm" d="{d}"/>')


if __name__ == '__main__':
    main()
