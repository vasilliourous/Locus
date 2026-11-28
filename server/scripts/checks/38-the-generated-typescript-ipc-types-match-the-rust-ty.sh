#!/usr/bin/env bash
# Section 38 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_38() {
    echo "38. The generated TypeScript IPC types match the Rust types"

    # ── WHY THIS GUARD EXISTS ──
    #
    # Section 33 asserts that `ChrysalisStatus`'s fields agree across Rust and
    # TypeScript. It asserted that for ONE type out of nine, by regex, against two
    # hand-maintained files. The other eight — ValidateCodeResult, ActivationResult,
    # ConnectionResult, UpdateStatus, UpdateProgress, and the three tagged unions
    # UdpStatus / AllowanceStatus / SubscriptionStatus — had no check at all.
    #
    # That is not a missing assertion, it is a missing mechanism. Nine hand-written
    # mirrors of nine Rust types will disagree eventually, and `FIXES.md` records the
    # identical failure for `udp`: added to the Rust struct, forgotten in the
    # interface, serialised and ignored.
    #
    # `client/src/services/chrysalis.generated.ts` is now derived from the Rust by
    # `gen-ts-contract.py`. There is one side to write. This section fails when the
    # committed output disagrees with what the Rust currently says — which is what a
    # developer who edits the Rust without regenerating produces.
    #
    # ── WHAT THIS CHECKS ──
    #
    # Regenerates into memory (without touching the file) and compares to the
    # committed copy. Any difference is reported as a diff. It also fails when the
    # extractor finds zero types, because a generator that silently extracts nothing
    # would make every comparison below pass against an empty file.
    # ─────────────────────────────────────────────────────────────
    tsgen_report=$(python3 "$SCRIPTS/gen-ts-contract.py" --check 2>&1)
    tsgen_status=$?
    if [ "$tsgen_status" -eq 0 ]; then
        ok "the TypeScript IPC types are a faithful mirror of the Rust"
    else
        while IFS= read -r line; do
            case "$line" in
                BAD*) bad "ts-contract: ${line#BAD	}" ;;
                *) [ -n "$line" ] && bad "ts-contract: $line" ;;
            esac
        done <<EOF
$tsgen_report
EOF
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
