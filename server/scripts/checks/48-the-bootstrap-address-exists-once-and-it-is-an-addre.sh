#!/usr/bin/env bash
# Section 48 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_48() {
    echo "48. The bootstrap address exists once, and it is an address"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `contract::ACTIVATION_TUNNEL_ADDR` is the ONE place in this repository where an
    # address is allowed to be a constant, and the permission rests entirely on it
    # doing the one job a hostname cannot: removing DNS from the enrolment path on a
    # network that filters it. The client's activation tunnel dials the hub's
    # bootstrap listener at the hub's *name* first (an operator moving the hub can
    # change a name without a release) and at this *address* second.
    #
    # Every `.md` in this repository is forbidden an address (`CLAIMS.md` §1, §10), so
    # this constant is the only home the value may have. Three ways that stops being
    # true, none visible at runtime:
    #
    #   * it becomes a HOSTNAME — the fallback then resolves the same name as the
    #     path it exists to rescue, and a DNS-filtered network fails both. That is
    #     precisely the defect this fixes;
    #   * it is left EMPTY — the fallback silently degrades to name-only, and the
    #     "address candidate" in the docs describes something the binary does not do;
    #   * a DOCUMENT grows a copy — two homes, and the day they disagree one of them
    #     is dialling a box that moved.
    #
    # Each assertion below was observed failing against the defect it catches before
    # being kept (see the commit that added this section).
    # ─────────────────────────────────────────────────────────────
    CLIENT_CONTRACT="$REPO/client/src-tauri/src/chrysalis/contract.rs"
    ADDR_VALUE=$(sed -n 's/^pub const ACTIVATION_TUNNEL_ADDR: &str = "\(.*\)";/\1/p' "$CLIENT_CONTRACT" 2>/dev/null | head -1)

    if [ -z "$ADDR_VALUE" ]; then
        bad "could not read ACTIVATION_TUNNEL_ADDR from client/src-tauri/src/chrysalis/contract.rs"
    elif ! printf '%s' "$ADDR_VALUE" | grep -qE '^([0-9]{1,3}\.){3}[0-9]{1,3}$'; then
        bad "ACTIVATION_TUNNEL_ADDR is '${ADDR_VALUE}', which is not an IPv4 literal"
        bad "  A name here re-introduces the DNS lookup the constant exists to remove."
    else
        ok "the bootstrap address is an IPv4 literal (${ADDR_VALUE})"

        # An address must not also be written into a live document. Archive/history
        # are exempt: they record what was true, not what is dialled.
        ADDR_IN_DOCS=$(grep -rlF "$ADDR_VALUE" "$REPO/docs" --include='*.md' 2>/dev/null \
            | grep -v '/archive/' | grep -v '/history/' | head -5 || true)
        if [ -n "$ADDR_IN_DOCS" ]; then
            bad "the bootstrap address is written into a live document:"
            printf '%s\n' "$ADDR_IN_DOCS" | sed 's/^/        /' >&2
            bad "  An address is a property of the box; the constant in contract.rs is its home."
        else
            ok "the bootstrap address is not restated in any live document"
        fi
    fi

    # The bootstrap path must be BUILT from the candidate list rather than a single
    # hardcoded server — that is what makes the address reachable at all.
    if grep -q 'fn bootstrap_targets' "$REPO/client/src-tauri/src/chrysalis/activation_tunnel.rs" \
       && grep -q 'ACTIVATION_TUNNEL_ADDR' "$REPO/client/src-tauri/src/chrysalis/activation_tunnel.rs"; then
        ok "the client builds its bootstrap candidates from the named and address targets"
    else
        bad "activation_tunnel.rs no longer builds candidates from ACTIVATION_TUNNEL_ADDR — "
        bad "  the tunnel is back to a single hostname and a DNS-filtered network has no route"
    fi

    # And the failure that candidate exists for must actually reach the tunnel.
    if grep -q 'Self::ResetDuringHandshake | Self::Refused | Self::Dns' \
            "$REPO/client/src-tauri/src/chrysalis/hub/client.rs"; then
        ok "a DNS failure is retried over the tunnel (the address needs no resolver)"
    else
        bad "Failure::Dns is not retryable over the tunnel — the address candidate is never dialled"
        bad "  and a DNS-filtered school network still has no path to enrolment."
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
