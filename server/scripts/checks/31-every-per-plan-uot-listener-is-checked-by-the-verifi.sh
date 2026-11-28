#!/usr/bin/env bash
# Section 31 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_31() {
    echo "31. Every per-plan UoT listener is checked by the verifier"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `verify-ss2022.sh` is the script an operator runs ON THE HOST to answer "is
    # every listener serving SS2022 with the right key?". After the per-plan UoT
    # split (one sing-box listener per plan, `sing-box-uot-strike` on 8446 and
    # `sing-box-uot-eco` on 8447), it was still checking ONE listener — the retired
    # `sing-box-uot` — against ONE config, `/etc/sing-box/config.json`, which no
    # longer exists.
    #
    # The consequence was not a cosmetic warning. It compared the retired unit's key
    # against Strike and had no concept of the free plan's listener at all, so a free
    # key that did not match its own listener passed the whole script: TCP works, game
    # traffic is silently dead, and the verifier says the deployment is healthy. That
    # is the same class as §24 — a check that cannot fail, guarding the one pairing a
    # student notices.
    #
    # ── WHAT THIS CHECKS ──
    #
    #   (a) The verifier names each per-plan unit the deploy creates.
    #   (b) It does not rely on the retired single-listener unit or config as if they
    #       were current — both may appear, but only in a check that expects them to
    #       be ABSENT.
    #   (c) It compares each plan's UoT key against THAT PLAN's tier key, i.e. the
    #       agreement, not one side.
    # ─────────────────────────────────────────────────────────────
    VERIFY_SS="$SCRIPTS/verify-ss2022.sh"
    if [ ! -f "$VERIFY_SS" ]; then
        bad "verify-ss2022.sh is missing — the host verifier cannot check anything"
    else
        for unit in sing-box-uot-strike sing-box-uot-eco; do
            if grep -q "$unit" "$VERIFY_SS"; then
                ok "the verifier names ${unit}"
            else
                bad "verify-ss2022.sh does not name ${unit} — a listener nothing verifies"
            fi
        done

        # The retired unit must only appear as a NEGATIVE assertion or in prose.
        #
        # A positive requirement and a negative assertion both contain
        # `is-active --quiet sing-box-uot`, so the text alone cannot tell them apart —
        # and the first version of this guard reported the fixed file (matching its own
        # comment), while a naive "grep for the unit" version would also miss the real
        # defect. What distinguishes them is the SUCCESS arm: a requirement reports
        # PASS when the unit is up and FAIL when it is not; a negative assertion has no
        # success arm at all, only a failure.
        #
        # So: the block is a positive requirement iff a `pass` appears inside it.
        if awk '
            /is-active --quiet sing-box-uot( |$)/ { inblock = 1; depth = 0 }
            inblock && /pass / { found = 1 }
            inblock && /^\s*fi\s*$/ { inblock = 0 }
            END { exit found ? 0 : 1 }
        ' "$VERIFY_SS"; then
            bad "the verifier still reports success from the retired sing-box-uot.service"
            bad "  (that unit binds :8446 and races the per-plan listener)"
        else
            ok "the retired single-listener unit is not required by the verifier"
        fi

        # The free plan's inbound must be paired with the FREE tier's key.
        if grep -vE '^\s*#' "$VERIFY_SS" | grep -qE 'sing-box-uot-eco'; then
            ok "the verifier pairs the free UoT listener with its own config"
        else
            bad "verify-ss2022.sh does not pair eco.json with sing-box-uot-eco"
            bad "  A free listener whose key is never compared to the free tier key can"
            bad "  be dead while the verifier reports healthy — see §24's lesson."
        fi

        # Stale path: the pre-split config names nothing that exists on a current box.
        # Again code lines only, so the explanatory header cannot trip it.
        if grep -vE '^\s*#' "$VERIFY_SS" | grep -q '/etc/sing-box/config\.json'; then
            bad "verify-ss2022.sh still reads the retired /etc/sing-box/config.json"
        else
            ok "the verifier reads only the per-plan sing-box configs"
        fi

        # ── EVERY SEEDED PLAN MUST BE VERIFIED, NOT JUST THE TWO OLDEST ──────────
        #
        # WHY THIS EXISTS. Adding Budget to the tree while leaving the verifier's
        # tier list at `eco strike` is a defect that cannot be seen from the tree:
        # the hub serves a working plan, `verify-ss2022.sh` reports healthy, and the
        # one script whose job is "is this plan's key right" never looked at it. That
        # is the same shape as §24's free-listener gap, one tier over.
        #
        # HOW. The seed loop in `seed-pb.py` is the authority for which rows exist,
        # so each seeded row is required to appear in the verifier's tier loop. Both
        # sides are read from code lines, so a comment naming a tier cannot satisfy
        # it — and the verifier's list is literal on purpose (a directory scan would
        # verify a retired `stealth.json` as if it were current).
        _seed_rows="$(
            grep -oE '^\s+\("([a-z_]+)",\s*"[A-Z_]+_PASS"' "$SEED" 2>/dev/null \
                | sed -E 's/.*\("([a-z_]+)".*/\1/' | sort -u
        )"
        _verify_loop="$(grep -vE '^\s*#' "$VERIFY_SS" | grep -oE '^for tier in [a-z ]+; do' | head -1)"
        _missing_verified=""
        for _row in $_seed_rows; do
            # `free`'s on-box config is `eco.json` — the frozen asymmetry §4.5
            # documents. Mapping it here rather than renaming either side.
            _cfg_name="$_row"
            [ "$_row" = "free" ] && _cfg_name="eco"
            case "$_verify_loop" in
                *"$_cfg_name"*) : ;;
                *) _missing_verified="$_missing_verified ${_row}(as ${_cfg_name}.json)" ;;
            esac
        done
        if [ -n "$_missing_verified" ]; then
            bad "verify-ss2022.sh never checks these seeded plans:${_missing_verified}"
            bad "  a plan the hub serves whose key is never verified can be dead on TCP and"
            bad "  still be reported healthy — extend the 'for tier in ...' loop."
        else
            ok "verify-ss2022.sh verifies every seeded plan's config and key"
        fi

        # Budget's UoT listener carries its own class, and that is a SECURITY
        # property, not a cosmetic one: 25 Mbps is what the plan sells.
        if grep -vE '^\s*#' "$VERIFY_SS" | grep -qE 'uot_check "budget".*"50"'; then
            ok "the verifier asserts Budget's UoT listener is shaped (class 1:50)"
        else
            bad "verify-ss2022.sh does not assert Budget's UoT class — its UDP could be"
            bad "  unshaped on the box while every check reports healthy."
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
