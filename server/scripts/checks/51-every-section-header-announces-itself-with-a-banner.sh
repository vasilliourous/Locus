#!/usr/bin/env bash
# Section 51 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_51() {
    echo "51. Every section header announces itself with a banner"

    # ── WHY THIS GUARD EXISTS ──
    #
    # The §-numbering is an API: live prose cites §N by number 649 times across the
    # doc set, and a reader who follows one runs this script and looks for the line.
    # A section whose `# NN.` header exists but whose `echo "NN. …"` does not is a
    # section that executes and is invisible — §42 did exactly that from the day it
    # was written until 2026-11-22, and four runbooks cited §1a for a section that
    # had a header and no banner either.
    #
    # A citation to a section nobody can see is not a weaker guard, it is a false
    # claim that a guard exists. This is the cheapest possible check on the
    # numbering: pure sed, no semantics.
    #
    # ── WHAT THIS CHECKS ──
    #
    # ONE DIRECTION: every `# NN[a-z]?. ` header must have a matching banner. That is
    # the defect class — a section that executes and is invisible.
    #
    # NOT the reverse direction, and the measurements say why: the file has 26 header
    # lines and 54 banners. A banner with no header is the NORMAL shape here: a
    # section may carry a one-line `echo "NN. …"` and put its long reasoning behind a
    # `# WHY THIS EXISTS` comment instead of a number. Requiring a numbered header for
    # every banner failed 29 times on the first run, every one of them a section that
    # was working, cited and correct. That is §7's lesson applied to a new check: a
    # check that fires 29 times on correct input is worse than no check.
    #
    # Duplicates are tolerated on the header side, because §25 carries the header
    # twice (`:3994` "—" and `:4122` "(parentheses)"), which is a pre-existing
    # inconsistency this check is deliberately not the place to fix.
    # ─────────────────────────────────────────────────────────────
    # THE SCAN SET IS THE WHOLE CHECKER, NOT ONE FILE.
    #
    # Sections live in `checks/NN-*.sh` and are indented into function bodies, so
    # the anchors are "optional leading whitespace, then `# N.`" and the banner is
    # matched with or without the indent. Scanning only `check-consistency.sh`
    # would find zero headers and zero banners and pass vacuously -- the failure
    # mode this check exists to catch, applied to itself.
    #
    # The orchestrator is scanned too: it still carries prose between sections.
    _check_files=$(ls "$CHECKS_DIR"/*.sh 2>/dev/null; echo "$REPO/server/scripts/check-consistency.sh")
    banner_report=$(grep -rh -oE '^[[:space:]]*# [0-9]+[a-z]?\. ' $_check_files \
          | sed 's/^[[:space:]]*# //;s/\. $//' | sort -u > /tmp/chrysalis_hdr.$$ ; \
        grep -rh -oE '^[[:space:]]*echo "[0-9]+[a-z]?\. ' $_check_files \
          | sed 's/^[[:space:]]*echo "//;s/\. $//' | sort -u > /tmp/chrysalis_echo.$$ ; \
        comm -23 /tmp/chrysalis_hdr.$$ /tmp/chrysalis_echo.$$ \
          | awk '{printf "%s\t%s\n", "NOBANNER", $0}' )
    rm -f /tmp/chrysalis_hdr.$$ /tmp/chrysalis_echo.$$
    if [ -z "$banner_report" ]; then
        ok "every numbered '#' section header has a matching echo banner"
    else
        while IFS="$(printf '\t')" read -r _kind number; do
            bad "§${number} has a '# ${number}.' header but no 'echo \"${number}. …\"' — it executes and is invisible, so a citation to it sends the reader to a section they cannot find"
        done <<EOF
$banner_report
EOF
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
