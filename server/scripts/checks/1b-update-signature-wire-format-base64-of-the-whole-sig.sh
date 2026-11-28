#!/usr/bin/env bash
# Section 1b -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_1b() {
    # ─────────────────────────────────────────────────────────────
    # 1b. The update-signature WIRE FORMAT.
    #
    # The client's `tauri-plugin-updater` runs `base64_to_string()` over the
    # `signature_<platform>` field BEFORE parsing the result as minisign text:
    #
    #     Signature::decode(base64_to_string(signature_<platform>))
    #
    # So the stored value MUST be base64 of the ENTIRE four-line .sig file — not the
    # file text itself. This is the exact fault that shipped a live `update_config`
    # row no client could install: the raw text (which contains spaces and newlines)
    # fails base64 decoding on the client with `Invalid byte …, offset N`. Every
    # automated check called the release healthy.
    #
    # Three places must agree, and a comment saying "keep these in sync" is not a
    # mechanism — this is:
    #
    #   1. fetch-release.py must base64-ENCODE the .sig text when folding it in.
    #   2. admin_console.pb.js must VALIDATE the shape on BOTH publish and activate,
    #      using the SAME predicate, or one path accepts what the other rejects.
    #   3. the client contract test must pin base64 as the wire format.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "1b. Update-signature wire format (base64 of the whole .sig)"

    if grep -q 'base64.b64encode(sig_text' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        ok "fetch-release.py base64-encodes the .sig text before storing it"
    else
        bad "fetch-release.py does not base64-encode the stored signature — the
             raw .sig text fails the client's base64 decode (\"Invalid byte, offset\")"
    fi

    # The value must NOT be the raw file text. `.strip()` alone (no encode) is the
    # old bug; catch a regression that reintroduces it.
    if grep -qE 'entry\["signature"\] = f\.read\(\)\.strip\(\)' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        bad "fetch-release.py stores the raw .sig text again (the offset bug)"
    else
        ok "fetch-release.py no longer stores the raw .sig text"
    fi

    # The console must validate the shape in BOTH the publish and set paths.
    sig_shape_uses=$(grep -c 'isPlausibleMinisignSignature(' "$HOOKS/admin_console.pb.js" 2>/dev/null)
    if grep -q 'function isPlausibleMinisignSignature' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
        ok "admin_console.pb.js defines the shared signature-shape predicate"
    else
        bad "admin_console.pb.js has no isPlausibleMinisignSignature predicate"
    fi
    if grep -qE 'isPlausibleMinisignSignature\(a\.signature\)' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
        ok "releases.publish validates the signature shape before writing it"
    else
        bad "releases.publish writes signature_<platform> without a shape check —
             a raw-text row can be published and reach every client"
    fi

    # ── The Overview's cards and the lists they open must count the SAME rows ──
    #
    # WHY THIS IS A CHECK. On the live tree the "Activated on a device" card read
    # `data.codes.bound` after the hook had stopped returning `bound`, so it rendered
    # blank; and `expiringSoon` counted every future expiry inside 30 days — including
    # UNUSED stock — while the worklist it links to excludes unsold and suspended
    # codes. Both look like a working page. The operator clicks a card that says 7 and
    # gets 11 rows, and concludes the console is stale.
    #
    # It runs the REAL handler rather than grepping it: a text match cannot tell
    # whether two expressions agree about a boundary. Each assertion below was
    # observed FAILING against the pre-fix expression (`isUsed` removed from the
    # expiringSoon condition) before it was kept.
    CARD_CHECK="$REPO/server/scripts/console-card-consistency.js"
    if [ -f "$CARD_CHECK" ] && command -v node >/dev/null 2>&1; then
        if card_out=$(node "$CARD_CHECK" 2>&1); then
            ok "the Overview's cards agree with the lists they open ($(printf '%s' "$card_out" | grep -c '^OK') assertions)"
        else
            while IFS= read -r line; do
                [ -n "$line" ] && bad "  $line"
            done <<< "$(printf '%s' "$card_out" | grep -v '^OK')"
            bad "the Overview's cards disagree with the lists they open — an operator clicks a count and gets a different set of rows"
        fi
    else
        warn "console-card-consistency.js or node is unavailable — cannot check the Overview's cards"
    fi
    if grep -qE 'isPlausibleMinisignSignature\(gsig\)' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
        ok "releases.set validates the signature shape before activating"
    else
        bad "releases.set no longer validates the stored signature shape"
    fi
    [ "$sig_shape_uses" -ge 3 ] || bad "the signature-shape predicate is defined but not used in both paths (uses=$sig_shape_uses)"

    # The client test must still pin base64(the whole .sig text) as the wire format.
    if grep -q 'the whole .sig file, base64' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null \
       || grep -q 'base64 of the ENTIRE' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null \
       || grep -q 'base64(whole' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null; then
        ok "the client contract test pins base64-of-the-whole-.sig as the wire format"
    else
        warn "could not confirm the client test pins the base64 wire format — check update_signature_contract.rs"
    fi
}
