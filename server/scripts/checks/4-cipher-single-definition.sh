#!/usr/bin/env bash
# Section 4 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_4() {
    # ─────────────────────────────────────────────────────────────
    # 4. The cipher is defined exactly once.
    #
    # The SS2022 migration collapsed ten copies into one. This keeps it collapsed:
    # a bare quoted SS2022 cipher string anywhere except modules/00-env.sh and the
    # templates (which are declarative references, not config writers) is drift.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "4. Cipher single-definition"

    cipher_defs=$(grep -rn '2022-blake3' "$REPO/server" \
        --include=*.sh --include=*.py 2>/dev/null \
        | grep -v node_modules \
        | grep -v 'check-consistency.sh' \
        | grep -vE 'SS_METHOD=|\$\{SS_METHOD\}|SS_METHOD\b|04-tc' \
        | grep -cE '"2022-blake3|= *"2022-blake3')

    if [ "$cipher_defs" -eq 0 ]; then
        ok "no hardcoded SS2022 cipher outside the SS_METHOD definition"
    else
        bad "${cipher_defs} hardcoded SS2022 cipher literal(s) outside SS_METHOD:"
        grep -rn '2022-blake3' "$REPO/server" --include=*.sh --include=*.py 2>/dev/null \
            | grep -v node_modules \
            | grep -v 'check-consistency.sh' \
            | grep -vE 'SS_METHOD=|\$\{SS_METHOD\}|SS_METHOD\b' \
            | grep -E '"2022-blake3' | sed 's/^/         /'
    fi

    if grep -q 'SS_METHOD=' "$REPO/server/modules/00-env.sh" 2>/dev/null; then
        ok "SS_METHOD is defined in modules/00-env.sh"
    else
        bad "SS_METHOD is not defined in modules/00-env.sh"
    fi
}
