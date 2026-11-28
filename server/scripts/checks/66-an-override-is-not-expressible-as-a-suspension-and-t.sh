#!/usr/bin/env bash
# Section 66 -- an override must not be expressible as a suspension
#
# The override/eviction feature has two structural invariants that no unit test
# and no typecheck can see, and both were broken in the first draft of its design:
#
#   1. An eviction written as `codes.suspended = true`. That column is read by
#      activation.pb.js, so "evict" expressed as a suspension REFUSES THE NEXT
#      HOLDER and breaks the resale releasing exists for. The bug presents as
#      "released the code and it cannot be sold" -- after the release succeeded.
#   2. An eviction predicate the heartbeat does not read, or reads after the
#      suspension check. Then a live client keeps serving a released code (the
#      original silent bug) or reports the wrong reason for its refusal.
#
# Both are SHAPE facts about goja source spread across three files and one
# TypeScript module. So this runs `override-consistency.js`, which reads the rules
# out of the source, the same way section 58 runs `liveness-consistency.js`.
#
# OBSERVED FAILING, both, before being kept:
#   - injecting `recU.set("suspended", true)` into codes.release  -> assertion 1
#   - moving the heartbeat's eviction block below the suspension check -> assertion 3
# A guard that cannot fail reads exactly like one that passed.
section_66() {
    echo "66. An override is not expressible as a suspension, and the heartbeat enforces it"

    local OV_CHECK="$REPO/server/scripts/override-consistency.js"
    if [ ! -f "$OV_CHECK" ]; then
        bad "override-consistency.js is missing -- an eviction could silently become a suspension that refuses the next holder"
    elif ! command -v node >/dev/null 2>&1; then
        warn "node is unavailable -- cannot check the override/eviction invariants"
    else
        local ov_out
        if ov_out=$(node "$OV_CHECK" 2>&1); then
            ok "override invariants hold ($(printf '%s' "$ov_out" | grep -c '^  OK') assertions)"
        else
            while IFS= read -r line; do
                [ -z "$line" ] && continue
                # The script already prefixes its failures with "BAD"; strip it so
                # the runner's own marker is the only one on the line.
                bad "$(printf '%s' "$line" | sed 's/^ *BAD *//')"
            done <<< "$(printf '%s' "$ov_out" | grep 'BAD')"
            bad "an override invariant is broken -- see docs/reference/OVERRIDES.md"
        fi
    fi
}
