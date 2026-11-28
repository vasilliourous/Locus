#!/usr/bin/env bash
# Section 67 -- every state, from every other state
#
# The requirement this guards was once prose: "the UI must tolerate ad-hoc,
# out-of-order, human-driven changes to codes without the operator fighting it".
# Nothing could check that, so it was satisfied by moving two buttons. It is now a
# MATRIX -- for every pair (from, to) of the six states the console can display,
# there is a direct operator action (one, or two with no ordered prerequisite)
# that produces `to`, or the cell is impossible for a documented reason.
#
# A matrix asserted in a report is a matrix nobody ran, and the previous pass
# shipped exactly that: `released` could be reached from `in term` but never the
# other way round, and no test could see it. So this runs `state-matrix.js`, which
# lifts the REAL `admin_console.pb.js` handlers, drives each of the 36 cells
# against a stub dao, and compares the result with the label the console's own
# `stateLabel` would render (transformed out of `model.ts` with esbuild, so the
# rule is not restated).
#
# OBSERVED FAILING before it was kept: narrowing `codes.override` back to "only a
# pending sign-out" made released->in-term, lapsed->in-term and stopped->in-term
# all fail, plus the audit assertions -- 5 BAD lines. A guard that cannot fail
# reads exactly like one that passed.
section_67() {
    echo "67. Every state is reachable from every other state, or named impossible"

    local SM_CHECK="$REPO/server/scripts/state-matrix.js"
    if [ ! -f "$SM_CHECK" ]; then
        bad "state-matrix.js is missing -- the state model could lose a transition with nothing to notice"
    elif ! command -v node >/dev/null 2>&1; then
        warn "node is unavailable -- cannot drive the state matrix"
    else
        local sm_out
        if sm_out=$(node "$SM_CHECK" 2>&1); then
            ok "the state matrix holds ($(printf '%s' "$sm_out" | grep -c '^  OK') cells/assertions)"
        else
            while IFS= read -r line; do
                [ -z "$line" ] && continue
                bad "$(printf '%s' "$line" | sed 's/^ *BAD *//')"
            done <<< "$(printf '%s' "$sm_out" | grep 'BAD')"
            bad "an operator cannot reach a state the console can display -- see the I2 matrix in the spec report"
        fi
    fi
}
