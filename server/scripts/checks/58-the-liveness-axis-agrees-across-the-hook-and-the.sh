#!/usr/bin/env bash
# Section 58 -- the liveness axis: one rule, two languages, or one bug
#
# The `last_seen_at` column is the only trace a heartbeat leaves on the hub, and
# the desync this section guards against is what happens when the two things that
# classify it disagree.
#
#   * `server/pb_hooks/session_probe.pb.js` (goja) emits live/stale/absent/unknown
#   * `server/console/src/views/codes/model.ts` (TypeScript) renders the badge
#
# Neither can import the other, so the lease bound, the stale bound and the four
# vocabulary words are duplicated across a language boundary with nothing
# enforcing agreement -- the exact incident class docs/reference/CONTRACT.md
# catalogues four times (`uot_port`, `download_*`, `bound_this_device`,
# `macos_intel`). A grep cannot catch it, because what has to agree is the
# BOUNDARY BEHAVIOUR of two expressions, not their text.
#
# So this runs `liveness-consistency.js`, which lifts the real `livenessOf` out of
# model.ts, overrides its bounds with the constants parsed out of the hook, and
# drives both across every boundary. It also asserts the two things the fix
# depends on and that a refactor could quietly remove:
#
#   1. the heartbeat actually WRITES `last_seen_at` (without it, liveness is
#      permanently unknown and the reported bug is unfixable);
#   2. it writes it BEFORE the 403/410 refusal returns, so a suspended or expired
#      device that is still beating reads as alive rather than absent;
#   3. `unknown` is never rendered as a state ("an unknown state must never be
#      silently rendered as signed out").
#
# Each of the three was OBSERVED FAILING against the pre-fix code before it was
# kept -- see server/scripts/liveness-consistency.js and the guard-failure
# evidence in docs/reference/AUTH-STATE.md §6.
section_58() {
    echo "58. The liveness axis agrees across the hook and the console, and unknown is not a state"

    local LIVE_CHECK="$REPO/server/scripts/liveness-consistency.js"
    if [ ! -f "$LIVE_CHECK" ]; then
        bad "liveness-consistency.js is missing -- the console could render any liveness the hook never emits"
    elif ! command -v node >/dev/null 2>&1; then
        warn "node is unavailable -- cannot check the liveness vocabulary across the hook and the console"
    else
        local live_out
        if live_out=$(node "$LIVE_CHECK" 2>&1); then
            ok "the liveness rule, its lease bound and its vocabulary agree ($(printf '%s' "$live_out" | grep -c '^OK') assertions)"
        else
            while IFS= read -r line; do
                [ -n "$line" ] && bad "  $line"
            done <<< "$(printf '%s' "$live_out" | grep -v '^OK' | grep -v '^$')"
            bad "the liveness axis disagrees -- a device the probe calls stale could be shown as checked in, or an unknown rendered as signed out"
        fi
    fi

    # ── The probe hook's BEHAVIOUR, not its text ──────────────────────────
    #
    # `liveness-consistency.js` above compares two implementations of one rule.
    # It cannot tell whether `session_probe.pb.js` does what its own comments say,
    # and that matters more than usual because the probe is the endpoint the
    # runbook tells an operator to trust. So this runs the REAL handler under a
    # stub dao, the same technique `console-card-consistency.js` uses.
    #
    # It found two real defects on first run: a comment asserting a uniform 404
    # the code did not implement, and an `in_sync` fallthrough that reported
    # "both sides agree" for a device the hub had never seen. Mutation-tested —
    # see the header of the script.
    PROBE_CHECK="$REPO/server/scripts/probe-hook-consistency.js"
    if [ ! -f "$PROBE_CHECK" ]; then
        bad "probe-hook-consistency.js is missing -- the probe's behaviour is unchecked"
    elif command -v node >/dev/null 2>&1; then
        probe_out=
        if probe_out=$(node "$PROBE_CHECK" 2>&1); then
            ok "the session probe behaves as documented ($(printf '%s' "$probe_out" | grep -c '^OK') assertions)"
        else
            while IFS= read -r line; do
                [ -n "$line" ] && bad "  $line"
            done <<< "$(printf '%s' "$probe_out" | grep -v '^OK' | grep -v '^$')"
            bad "the probe answers something other than what the runbook promises -- an operator would act on it"
        fi
    else
        warn "node is unavailable -- the probe's behaviour was not executed"
    fi

    # The schema must carry the column the write targets, or every beat throws
    # into the guard's catch and liveness is silently always unknown. That is the
    # "repo fix inert until the piece that runs it is redeployed" trap one layer
    # down: the hook would be right and the collection would not have the field.
    if grep -qE '\{"name":"last_seen_at","type":"date"\}' "$REPO/server/scripts/seed-pb.py" 2>/dev/null; then
        ok "the codes collection declares last_seen_at, so the heartbeat's write has a column"
    else
        bad "seed-pb.py no longer declares codes.last_seen_at -- the heartbeat's write would throw on every beat"
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
