#!/usr/bin/env bash
# Section 49 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_49() {
    echo "49. Colour is written down in exactly one place"

    # ── WHY THIS GUARD EXISTS ──
    #
    # Colour is the fact class this project has rewritten most often and recorded
    # worst. Three documents were carrying palettes as though they were current:
    #
    #   * `THEMES.md` held the pre-3.3.0 palette, and a light-vs-dark accent argument
    #     that stopped being true when the values changed under it;
    #   * `client/docs/FRONTEND.md` listed ten themes and named eight that had never
    #     existed in the registry;
    #   * `docs/archive/UI-AESTHETICS.md` was cited as "the spec" by the two above,
    #     while describing a client that had been retired and a palette that had been
    #     replaced twice.
    #
    # None of those could fail. A hex value in a sentence is not recomputable, so it
    # drifts in silence — the defect `CLAIMS.md` §2 exists to prevent, in the one
    # domain where it kept being granted an exception because "it's design".
    #
    # The rule: `/COLOURS.md` is the only file allowed to state a colour. Everything
    # else links to it. This check makes that a property rather than a habit.
    #
    # Deliberately NOT flagged, because a false positive trains people to ignore the
    # check: a `#`-prefixed value with no colour meaning (a git short hash, a GitHub
    # issue number such as #13518, a shell `${line#BAD}`); the words "colour" and
    # "palette" in prose, which is how a document LINKS here; `docs/history/`, which
    # is cryptography and hashes; and `legacy/`, a retired client never built.
    # ─────────────────────────────────────────────────────────────
    COLOUR_HOME="COLOURS.md"

    # THE ALLOWLIST, and why it is not only the doc.
    #
    # A palette has to be expressible in code, so the modules `/COLOURS.md` §2-§8
    # name hold the literal and the doc describes it. That pair is the whole
    # allowance. Everything else - every document, every comment, every component -
    # links instead of stating.
    #
    # This list is asserted, not trusted: each path must exist, so a renamed palette
    # module fails here rather than silently widening the allowlist to nothing.
    COLOUR_SOURCES="
    client/src/pages/_theme.tsx
    client/src/index.html
    client/src/assets/styles/index.scss
    client/src-tauri/src/utils/resolve/window.rs
    client/scripts/generate-icons.mjs
    server/site/style.css
    server/console/src/styles.css
    "
    if [ -f "$REPO/$COLOUR_HOME" ]; then
        ok "$COLOUR_HOME exists at the workspace root"
    else
        bad "$COLOUR_HOME is missing - colour has no home, and this guard has nothing to allow"
    fi
    for src in $COLOUR_SOURCES; do
        if [ -f "$REPO/$src" ]; then
            ok "palette source present: $src"
        else
            bad "palette source named by $COLOUR_HOME is missing: $src"
            bad "  Either the file moved (update the allowlist and the doc), or it was"
            bad "  deleted (then the palette it held is gone and the doc is describing air)."
        fi
    done

    colour_offenders=$(cd "$REPO" && LC_ALL=C python3 - "$COLOUR_SOURCES" <<'PYCOLOUR'
import os, re, sys

HOME = 'COLOURS.md'
ALLOWED = set(sys.argv[1].split())

# A colour literal: a `#`-prefixed hex value, or a functional notation with a
# numeric argument. The lookbehind keeps `${line#BAD}` and `&#x...` out.
HEX = re.compile(r'(?<![\w&$#])#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3})(?![0-9a-fA-F])')
FN = re.compile(r'\b(?:rgba?|hsla?)\s*\(\s*[0-9]')

ROOTS = ['docs', 'client/docs', 'client/src', 'client/src-tauri/src',
         'client/scripts',
         'server/site', 'server/console/src', 'server/scripts']
SKIP_DIRS = {'node_modules', 'target', 'dist', '.git', 'legacy'}
TEXT_EXT = {'.md', '.ts', '.tsx', '.rs', '.sh', '.css', '.scss', '.html', '.json', '.py', '.vue',
            '.mjs', '.js', '.svg'}

# SVG is scanned because the brand mark is a vector and its gradients ARE colour.
# Before it was, `client/src/assets/image/glyph.svg` carried six hex values that
# this guard could not see -- so "colour is written down in exactly one place" was
# true of the scan and false of the tree. The marks are the one family of colour
# the palette does not own (the artwork's own gradients), and they are named here
# rather than left invisible.
COLOUR_BEARING_SVG = {
    'client/src/assets/image/glyph.svg',
    'client/src/assets/image/mark/status_1.svg',
    'client/src/assets/image/mark/status_2.svg',
    'client/src/assets/image/mark/status_3.svg',
    'client/src/assets/image/mark/status_4.svg',
    'server/site/icon.svg',
}

# A `#` followed by six hex characters is ALSO how a commit sha and an issue
# number are written. The word test separates a palette entry from a reference:
# a colour is written near a colour word, or after a declaration separator.
# `#13518` in a sentence about CI runners has neither.
NEAR = re.compile(r'\b(colou?r|palette|accent|background|foreground|fill|stroke|border|tint|shade|hex)\b', re.I)
DECL = re.compile(r'^\s*(?:--?[\w-]+|[\w-]+)\s*[:=]\s*')

offenders = []

for root in ROOTS:
    if not os.path.isdir(root):
        continue
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for fn in files:
            if os.path.splitext(fn)[1] not in TEXT_EXT:
                continue
            path = os.path.normpath(os.path.join(dirpath, fn)).replace(os.sep, '/')
            if path == HOME or path in ALLOWED or path.startswith('docs/history/'):
                continue
            # An SVG that carries brand artwork is colour by definition; it is
            # allowed by name so the set is enumerated, not inferred. Any OTHER
            # svg in the tree is scanned and must not carry a literal.
            if path in COLOUR_BEARING_SVG:
                continue
            if path.endswith('.svg') and '/itemicon/' in path:
                continue
            # Third-party brand marks, used as latency-test targets. Their colours
            # are Google's and YouTube's trademarks, not this project's palette --
            # a guard that demanded they be moved to COLOURS.md would be asking us
            # to repaint somebody else's logo.
            if path.endswith('.svg') and '/assets/image/test/' in path:
                continue
            try:
                text = open(path, encoding='utf-8', errors='replace').read()
            except OSError:
                continue
            for n, line in enumerate(text.splitlines(), 1):
                m = HEX.search(line)
                if m and (NEAR.search(line) or DECL.search(line)):
                    offenders.append('%s:%d\t%s' % (path, n, m.group(0)))
                    continue
                f = FN.search(line)
                if f:
                    offenders.append('%s:%d\t%s' % (path, n, f.group(0).strip()))

print('\n'.join(offenders))
PYCOLOUR
    )

    if [ -z "$colour_offenders" ]; then
        ok "no colour literal outside ${COLOUR_HOME} across docs and the client/server source trees"
    else
        bad "a colour value is written outside ${COLOUR_HOME}:"
        printf '%s\n' "$colour_offenders" | head -25 | sed 's/^/        /' >&2
        bad "  ${COLOUR_HOME} is the single home for every colour. State the value there, and"
        bad "  put a link to it where the value was."
    fi

    # The purge must stay done. The archived spec is the one document that has
    # previously been cited as an authority on colour, so assert it carries none.
    if grep -qE '#[0-9a-fA-F]{6}|rgba?\(' "$REPO/docs/archive/UI-AESTHETICS.md" 2>/dev/null; then
        bad "docs/archive/UI-AESTHETICS.md has grown a colour value again"
        bad "  That document is cited as a design authority; a value in it is a second home."
    else
        ok "the archived design spec carries no colour value"
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
