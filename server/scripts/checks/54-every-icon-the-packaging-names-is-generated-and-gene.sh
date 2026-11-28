#!/usr/bin/env bash
# Section 54 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_54() {
    echo "54. Every icon the packaging names is generated, and generated from the palette"

    # ── WHY THIS EXISTS ──
    #
    # An icon is the one artefact whose correctness nothing can compile-check. A stale
    # or misspelled icon path produces a build that compiles, signs, publishes and
    # installs, and the only symptom is a student looking at another program's logo —
    # which is exactly what shipped for every release up to 3.4.4, where the window,
    # taskbar, dock, tray and favicon were all still Clash Verge Rev's cat.
    #
    # Two failure modes, and both are silent:
    #
    #   1. **A config names a file nothing produces.** `bundle.icon`,
    #      `nsis.installerIcon` and the `webview2.*.json` installer icon each name a
    #      path; a rename leaves the config pointing at a file that no longer exists,
    #      and on Windows that surfaces as an installer that cannot draw its own icon.
    #   2. **A generator forgets a file.** `scripts/generate-icons.mjs` rewrites a
    #      fixed set; a new config entry naming a file outside that set means the icon
    #      cannot be regenerated, so it drifts from the palette forever.
    #
    # This checks BOTH directions against one list — the files the generator claims —
    # so neither side can widen without the other.
    #
    # It deliberately does NOT run the generator or decode the images: that needs
    # `rsvg-convert`, and a check that depends on an optional binary is a check that
    # is skipped where it matters. `node client/scripts/generate-icons.mjs --check`
    # compares bytes and is what CI should run; this asserts the wiring.
    ICON_GEN="$REPO/client/scripts/generate-icons.mjs"
    if [ ! -f "$ICON_GEN" ]; then
        bad "client/scripts/generate-icons.mjs is missing - the marks have no source"
        bad "  Without it every icon is a hand-edited binary, and 'the taskbar still"
        bad "  shows the old logo' becomes unfixable by inspection."
    else
        icon_report=$(cd "$REPO" && python3 - <<'PY'
import json, os, re

gen = 'client/scripts/generate-icons.mjs'
body = open(gen, encoding='utf-8').read()

# The names the generator writes, read from its own `OUTPUTS` table rather than
# from a list here: two lists is the drift this check exists to prevent. The
# table is split into `app`/`tiles`/`tray`/`favicon` blocks, each a literal
# array of quoted names, so this scans the quoted strings inside them.
outputs = re.search(r'const OUTPUTS = \{(.*?)^\}', body, re.S | re.M)
if outputs is None:
    print('BAD\tcould not find the OUTPUTS table in %s - the scan, not the tree,' % gen)
    print('BAD\t  is wrong, and this guard would pass while checking nothing')
    raise SystemExit
generated = set(re.findall(r"'([^']+)'", outputs.group(1)))
# `OUTPUTS.favicon` holds a path (`src/assets/image/logo.ico`) rather than a bare
# name, because it is written outside `iconsDir`. Compare on basenames, which is
# what every config entry resolves to.
generated = {os.path.basename(name) for name in generated}
if not generated:
    print('BAD\tcould not read any output name from %s - the scan, not the tree,' % gen)
    print('BAD\t  is broken, and a guard that cannot see its subject always passes')
    raise SystemExit

# Every icon a config names, with the config that names it.
named = {}

def add(path, where):
    if not path:
        return
    named.setdefault(os.path.normpath(path), set()).add(where)

conf = json.load(open('client/src-tauri/tauri.conf.json', encoding='utf-8'))
for entry in conf.get('bundle', {}).get('icon', []):
    add(os.path.join('client/src-tauri', entry), 'tauri.conf.json bundle.icon')

# The set of packaging configs that may name an installer icon. It is asserted
# rather than probed, because a `continue` on a missing file is how this guard
# stops covering something without saying so: the four `webview2.*.json` variants
# were deleted (they named NSIS files retired in 9f95aff), and a silent `continue`
# would have left the scan claiming the same coverage over one fewer config.
#
# If a future build reintroduces a variant, add it HERE and to the bundler; the
# check that it still names a real file is the `add`/existence pass below.
PACKAGING_CONFIGS = ('tauri.windows.conf.json',)
missing_configs = [
    v for v in PACKAGING_CONFIGS
    if not os.path.exists(os.path.join('client/src-tauri', v))
]
if missing_configs:
    print('BAD\tthe packaging-config set names %s, which is not in the tree' % ', '.join(missing_configs))
    print('BAD\t  Update PACKAGING_CONFIGS in this guard in the same change.')

for variant in PACKAGING_CONFIGS:
    path = os.path.join('client/src-tauri', variant)
    data = json.load(open(path, encoding='utf-8'))
    nsis = data.get('bundle', {}).get('windows', {}).get('nsis', {})
    if isinstance(nsis, dict) and nsis.get('installerIcon'):
        add(os.path.join('client/src-tauri', nsis['installerIcon']), variant + ' nsis.installerIcon')

if not named:
    print('BAD\tno icon path could be read from the packaging configs - the scan,')
    print('BAD\t  not the tree, is wrong; this guard is currently proving nothing')

# The favicon is named by index.html rather than a config.
html = open('client/src/index.html', encoding='utf-8').read()
m = re.search(r'href="\.?/?([^"]+\.ico)"', html)
if m:
    # `index.html` lives in `client/src/`, so a relative href resolves against
    # that directory — not the client root. Getting this wrong is how the first
    # draft of this check reported a favicon that was present as missing.
    add(os.path.join('client/src', m.group(1)), 'index.html favicon')
else:
    print('BAD\tclient/src/index.html names no favicon - the mark a browser tab')
    print('BAD\t  shows has gone, or the scan cannot see it')

# The landing page is a SEPARATE shipped surface with no build step: `05-caddy.sh`
# deploys `server/site/` verbatim, and `index.html` names `./favicon.ico` and
# `./icon.svg`. Before this was checked, the landing favicon was still the Clash
# Verge cat in Clash Verge green a full migration after the app had moved on --
# because every path above lives under `client/` and this scan never looked
# outside it. A mark the product has replaced, still served at the marketing URL,
# is not a smaller defect than one in the taskbar.
landing_html = os.path.join('server/site/index.html')
if not os.path.exists(landing_html):
    print('BAD\tserver/site/index.html is missing - the landing page ships from')
    print('BAD\t  there, or the scan is looking in the wrong place')
else:
    page = open(landing_html, encoding='utf-8').read()
    for match in re.finditer(r'(?:href|src)="\./([^"]+\.(?:ico|svg))"', page):
        add(os.path.join('server/site', match.group(1)), 'server/site/index.html')
    if not any(p.startswith('server/site') for p in named):
        print('BAD\tserver/site/index.html names no icon - either the mark is gone')
        print('BAD\t  from the landing page, or the scan cannot see it')

for path, where in sorted(named.items()):
    if not os.path.exists(path):
        print('BAD\t%s names %s, which does not exist' % (', '.join(sorted(where)), path))
        print('BAD\t  A build still succeeds; the platform falls back to a default icon,')
        print('BAD\t  and nothing reports that the mark is missing.')
        continue
    base = os.path.basename(path)
    if path.endswith('.icns') or path.endswith('.car'):
        # Documented as macOS-toolchain-only in icons/README.md. Presence is the
        # whole claim that can be made from here.
        print('OK\t%s is present (macOS-generated; see %s)' % (base, 'client/src-tauri/icons/README.md'))
        continue
    if path.endswith('.svg'):
        # A vector master, not a raster the generator can compare bytes against.
        # `server/site/icon.svg` is a copy of `client/src/assets/image/glyph.svg`,
        # and the check that they agree is a diff, not a byte compare -- stated
        # below rather than folded in here.
        print('OK\t%s is present (vector master)' % base)
        continue
    if base not in generated:
        print('BAD\t%s is named by %s but %s does not write it' % (
            base, ', '.join(sorted(where)), gen))
        print('BAD\t  An icon outside the generator cannot be regenerated, so it drifts')
        print('BAD\t  from the palette silently - the exact way the old mark survived.')
        continue
    print('OK\t%s is generated and named by %s' % (base, ', '.join(sorted(where))))

# The other direction: a generated file nothing packages is dead weight, and it
# is how a set ends up half-migrated.
named_bases = {os.path.basename(p) for p in named}
# Generated deliberately, and referenced by nothing. `icon.png` is the 512 master
# every icon convention expects to find (and what any future `iconutil`/`actool`
# run in the macOS gap would consume); `active_models` is not a thing — the list
# is kept short and each entry has to be justified here.
UNPACKAGED_BY_DESIGN = {'icon.png'}
for name in sorted(generated):
    if name in named_bases or name in UNPACKAGED_BY_DESIGN:
        continue
    # Tray icons are loaded by `include_bytes!`, not by a config, and the tile
    # set is inside the MSIX bundle Tauri builds from `bundle.icon`'s directory.
    if name.startswith('tray-icon'):
        tray_rs = open('client/src-tauri/src/core/tray/mod.rs', encoding='utf-8').read()
        if name in tray_rs:
            continue
        print('BAD\t%s is generated but core/tray/mod.rs does not include it' % name)
        print('BAD\t  A tray icon nothing loads is a file that will be forgotten.')
        continue
    if name.startswith('Square') or name == 'StoreLogo.png':
        continue
    print('BAD\t%s is generated but no config names it' % name)
    print('BAD\t  Either a reference was dropped, or the generator writes a file')
    print('BAD\t  nothing ships.')

# The third direction, and the one that actually let the old mark linger: a file
# sitting in `icons/` that the generator does NOT write. The scans above only ever
# look at names the generator declares or a config names, so a leftover binary --
# the cat's `tray-icon-sys-mono.ico`, a hand-copied PNG -- was invisible to every
# one of them. `git rm` is the fix when it happens; this is what says it happened.
#
# `icon.icns` is exempt because it is the one macOS binary the generator genuinely
# cannot produce in Node (`iconutil` is macOS-only) -- it is built by
# `.github/workflows/generate-macos-icons.yml` from the same vector. `README.md`
# because it is prose.
#
# `Assets.car` is NOT exempt and must not be re-added: it was a 1.9 MB compiled
# catalog holding the old mark, it takes precedence over `icon.icns` on macOS, and
# nothing but Apple's `actool` can author it. It was removed from `bundle.icon` on
# 2026-11-23 so the regenerated `.icns` is what macOS reads. If a future change
# reintroduces one, this list is where it would silently slip through -- a stray
# `.car` now fails here instead.
GENERATED_OR_EXEMPT = set(generated) | {'icon.icns', 'README.md'}
icons_dir = 'client/src-tauri/icons'
if os.path.isdir(icons_dir):
    for entry in sorted(os.listdir(icons_dir)):
        full = os.path.join(icons_dir, entry)
        if not os.path.isfile(full):
            continue
        if entry not in GENERATED_OR_EXEMPT:
            print('BAD\t%s is in icons/ but %s does not write it' % (entry, gen))
            print('BAD\t  A file the generator cannot reproduce drifts from the mark')
            print('BAD\t  forever, and nothing else in this repo reads it.')
            continue
    print('OK\tevery file in icons/ is one the generator writes, or a macOS artifact')

# ── The sentence about the ink, not just the ink ──
#
# The scans above prove the BYTES are right. None of them can see a DOCUMENT that
# describes those bytes wrongly, and on 2026-11-23 exactly that shipped: COLOURS.md
# §8 said the macOS template ink was "pure white, `#FFFFFF` ... white is the
# format's convention" in the same paragraph that correctly explained a template
# discards its RGB -- while `generate-icons.mjs` declares `templateInk: '#000000'`
# and draws it black, matching upstream. The `--check` above passed throughout, and
# §49 passed too: §49 asserts WHERE a colour literal lives, never whether a
# sentence about it is TRUE. So the prose drifted with the whole gate green, which
# is the `04-tc.sh` header case at the top of this file one level out.
#
# WHAT IT ASSERTS: the ink value stated in COLOURS.md §8 equals the `templateInk`
# the generator uses, both compared as normalised lowercase hex. The value has
# exactly one home in code and one home in prose, and this fails when they disagree.
#
# VERIFIED TO FAIL: restoring the old sentence ("pure white, `#FFFFFF`") produces
# the BAD line below; so does changing `templateInk` without the document.
colours_doc = 'COLOURS.md'
if not os.path.exists(colours_doc):
    print('BAD\tCOLOURS.md is missing - the template ink has no prose home')
else:
    doc = open(colours_doc, encoding='utf-8').read()
    # The paragraph names the template set and states one hex value.
    para = re.search(r'macOS template ink.{0,400}?`(#[0-9A-Fa-f]{6})`', doc, re.S)
    ink = re.search(r"templateInk:\s*'(#[0-9A-Fa-f]{6})'", body)
    if ink is None:
        print('BAD\t%s declares no templateInk - the macOS template ink has no' % gen)
        print('BAD\t  single home in code, so the document cannot be held to it')
    elif para is None:
        print('BAD\tCOLOURS.md states no hex for the macOS template ink - the one')
        print('BAD\t  colour outside the palette has lost its explanation')
    elif para.group(1).lower() != ink.group(1).lower():
        print('BAD\tCOLOURS.md says the template ink is %s but %s draws %s' % (
            para.group(1), gen, ink.group(1)))
        print('BAD\t  §49 checks where a colour literal LIVES, not whether a')
        print('BAD\t  sentence about it is true; that is how this drifted.')
    else:
        print('OK\tCOLOURS.md states the template ink the generator draws (%s)' % ink.group(1))
PY
    )
        if [ -z "$icon_report" ]; then
            bad "the icon-provenance check produced no output - it did not run"
        elif printf '%s' "$icon_report" | grep -q 'Traceback (most recent call last)'; then
            bad "the icon-provenance check CRASHED - a crashing guard reads like a passing one:"
            printf '%s\n' "$icon_report" | sed 's/^/        /' >&2
        else
            icon_bad=0
            while IFS="$(printf '\t')" read -r _kind message; do
                case "$_kind" in
                    OK)  ok "$message" ;;
                    BAD) bad "$message" ; icon_bad=$((icon_bad + 1)) ;;
                    *)   bad "icon-provenance: $message" ; icon_bad=$((icon_bad + 1)) ;;
                esac
            done <<EOF
$icon_report
EOF
            if [ "$icon_bad" -eq 0 ]; then
                ok "every packaged icon is one the generator writes"
            fi
        fi

        # Presence is not freshness, and this is the half that catches a stale mark.
        #
        # The scan above proves a named file EXISTS and is one the generator claims.
        # It cannot see that the bytes are old, which is exactly how the landing
        # favicon stayed the Clash Verge cat in Clash Verge green: every path existed,
        # every name matched, and the picture was wrong. `--check` re-renders in
        # memory and compares bytes, so "someone restored the old artwork" and "nobody
        # re-ran the generator" both fail here.
        #
        # It needs `rsvg-convert` and `node`, so it is skipped with a named reason
        # rather than silently passing when either is absent -- a check that cannot
        # run must say so (the class this file exists to enforce).
        if ! command -v node >/dev/null 2>&1; then
            warn "icon freshness: node is not installed, so --check could not run"
        elif ! command -v rsvg-convert >/dev/null 2>&1; then
            warn "icon freshness: rsvg-convert is not installed, so --check could not run"
        else
            if icon_drift=$(cd "$REPO" && node client/scripts/generate-icons.mjs --check 2>&1); then
                ok "every generated icon matches the generator byte for byte"
            else
                bad "generated icons do not match the generator:"
                printf '%s\n' "$icon_drift" | sed 's/^/        /' >&2
            fi
        fi
    fi
}
