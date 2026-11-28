#!/usr/bin/env bash
# Section 55 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_55() {
    echo
    echo "55. No two inline SVGs share an element id"

    # ── WHY THIS EXISTS ──
    #
    # `vite-plugin-svgr` renders an SVG INLINE into the document, so every `id="..."`
    # inside it joins the page-wide id namespace. Two inline SVGs that both declare
    # `id="top"` do not each get their own: the first one defined wins for both, and
    # the second mark silently renders in the first's gradients. The symptom is a
    # correctly-shaped icon in the wrong colours, which reads as a design choice.
    #
    # This became reachable when the FlClash assets landed: `glyph.svg` and
    # `status_1..3.svg` each declare `id="top"`, `id="mid"` and `id="dot"`, because
    # they are the same mark in different colours. Only `glyph.svg` is imported into
    # the UI today (the `status_*` files are generator inputs, never bundled), so
    # there is no collision NOW — and that is exactly the luck that ends the moment
    # somebody imports one more icon. This makes the second import fail here instead.
    #
    # Scope is deliberate: only SVGs reached through `?react` can be inlined, so only
    # those are scanned. A static `<img src>` or a build-time raster cannot collide.
    inline_svg_report=$(cd "$REPO" && python3 - <<'PY'
import os, re

# Every file the frontend imports as a React component. `?react` is the marker
# svgr keys on; without it Vite hands back a URL and nothing is inlined.
IMPORT = re.compile(r"""from\s+['"]([^'"]+\.svg)\?react['"]""")
ID = re.compile(r'\bid="([^"]+)"')

imported = []

for dirpath, dirs, files in os.walk('client/src'):
    dirs[:] = [d for d in dirs if d not in {'node_modules', 'target', 'dist', '.git'}]
    for fn in files:
        if not fn.endswith(('.tsx', '.ts')):
            continue
        source = os.path.join(dirpath, fn)
        try:
            text = open(source, encoding='utf-8', errors='replace').read()
        except OSError:
            continue
        for match in IMPORT.finditer(text):
            rel = match.group(1)
            if rel.startswith('@/'):
                resolved = os.path.join('client/src', rel[2:])
            elif rel.startswith('.'):
                resolved = os.path.normpath(os.path.join(dirpath, rel))
            else:
                continue
            imported.append((os.path.normpath(resolved), source))

if not imported:
    print('BAD\tno ?react SVG import was found - either the scan is broken or the')
    print('BAD\t  marks are no longer inlined, and this check is proving nothing')
    raise SystemExit

# id -> {svg file: the file that imports it}
declared = {}
for svg, importer in imported:
    try:
        text = open(svg, encoding='utf-8', errors='replace').read()
    except OSError:
        print('BAD\t%s imports %s, which does not exist' % (importer, svg))
        continue
    for match in ID.finditer(text):
        declared.setdefault(match.group(1), {})[svg] = importer

collisions = {k: v for k, v in declared.items() if len(v) > 1}
if collisions:
    for ident, where in sorted(collisions.items()):
        detail = '; '.join('%s (imported from %s)' % (svg, imp) for svg, imp in sorted(where.items()))
        print('BAD\tid="%s" is declared in more than one inlined SVG: %s' % (ident, detail))
        print('BAD\t  The first definition wins for every use, so the second mark')
        print("BAD\t  draws in the first one's gradients. Give one a unique id.")
else:
    print('OK\tevery id in an inlined SVG is unique across the inlined set (%d svg, %d ids)' % (
        len({svg for svg, _ in imported}), len(declared)))
PY
    )
    if [ -z "$inline_svg_report" ]; then
        bad "the inline-SVG id check produced no output - it did not run"
    else
        while IFS= read -r line; do
            case "$line" in
                OK*)   ok "${line#OK	}" ;;
                BAD*)  bad "${line#BAD	}" ;;
                *)     bad "inline-svg: $line" ;;
            esac
        done <<EOF
$inline_svg_report
EOF
    fi
}
