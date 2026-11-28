#!/usr/bin/env bash
# Section 41 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_41() {
    echo "41. The support address exists once, and every stuck screen renders it"

    # ── WHY THIS GUARD EXISTS ──
    #
    # The support address is the product's last resort: it is what a student uses
    # after the midleman could not help, or when the code was never reachable in the
    # first place. Before this, it existed in exactly one place on one screen —
    # a one-line footer under the Connect button — and the activation gate, which is
    # where a stuck student actually IS, showed nothing at all.
    #
    # Two failures this pins:
    #
    #   1. A SECOND COPY of the address. Two literals drift, and the day one of them
    #      is typo'd or the address changes, the app is telling half its students to
    #      email somewhere nobody reads. The address must appear exactly once in the
    #      English bundle and nowhere else in the client's source.
    #   2. A SCREEN THAT STOPS RENDERING IT. Deleting the `SupportBlock` from a page
    #      is an invisible regression: nothing fails, the page just quietly stops
    #      offering a route to a human, and the next report is "there is no way to
    #      contact anyone", which is the complaint this whole change answers.
    #
    # The reply promise is checked here too, because it is a promise the UI MAKES on
    # the operator's behalf: if the wording is removed, the screen still tells people
    # to email but no longer says when they will hear back, and silence starts
    # reading as being ignored.
    #
    # ── WHAT THIS CHECKS, AND WHAT IT CANNOT ──
    #
    # It checks the TEXT, not the rendered DOM: that there is one definition, that
    # the four screens reference the component, and that the promise is present. A
    # screen could reference the component and still hide it behind a condition that
    # is never true — that is caught by the vitest check in `client/tests/`, which
    # renders the gate and asserts the address is in the document.
    #
    # Observed failing before being kept: the address was duplicated into
    # `connection.tsx` as a literal (reported), each `SupportBlock` reference was
    # deleted from `activation.tsx`, `connection.tsx` and `account.tsx` in turn
    # (reported for each), and `replyPromise` was renamed in the bundle while the
    # component still read the old key (reported).
    # ─────────────────────────────────────────────────────────────
    LOCALE_HOME="$REPO/client/src/locales/en/home.json"
    SUPPORT_COMPONENT="$REPO/client/src/components/shared/support-block.tsx"
    SUPPORT_ADDRESS="maillcontainer@proton.me"

    if [ ! -f "$LOCALE_HOME" ]; then
        bad "the English locale bundle is missing — the support copy cannot be checked"
    else
        # 1. Exactly one definition of the address, anywhere in the client's source.
        #    `$SUPPORT_ADDRESS` is spelled here as the EXPECTED value, so a deliberate
        #    change of address has to be made in two places on purpose — which is the
        #    point, since a change of address is exactly when both halves must agree.
        #
        #    THE TWELVE TRANSLATIONS ARE EXCLUDED, AND THAT IS A FEATURE (amended
        #    2026-11-23). A locale bundle is a copy of the KEY STRUCTURE, not of the
        #    decision: an address, a port or a price is the same string in every
        #    language, and leaving `supportEmail` out of `de/home.json` is not a
        #    translation choice — it is the key falling through to the fallback
        #    language, i.e. a hole that was invisible until `tests/locale-parity.test.ts`
        #    existed. Each locale legitimately carries the address INSIDE ITS OWN
        #    BUNDLE, and `support-block.tsx` resolves it through ONE key at runtime,
        #    so the runtime property this guard exists to protect — one reference, no
        #    drifting literal in a component — is untouched by the copies here.
        #
        #    `en` is deliberately NOT excluded: it is the single definition this check
        #    is about, and excluding the whole `locales/` directory would also delete
        #    the thing being counted (observed: it reported "gone from the client
        #    entirely" on a correct tree).
        #
        #    This was found the honest way: filling the 56 missing keys per locale
        #    (the parity fix above) made this line count 13 and report BAD on a
        #    correct tree. The check was wrong, not the locale files.
        locale_excludes=""
        for _loc in ar de es fa id jp ko ru tr tt zh zhtw; do
            locale_excludes="$locale_excludes --exclude-dir=$_loc"
        done
        # shellcheck disable=SC2086
        address_count=$(grep -rF --include='*.ts' --include='*.tsx' --include='*.json' \
            $locale_excludes \
            -- "$SUPPORT_ADDRESS" "$REPO/client/src" \
            | wc -l | tr -d ' ')
        address_count=$(printf '%s' "$address_count" | tr -dc '0-9')
        if [ "${address_count:-0}" -eq 0 ]; then
            bad "the support address ${SUPPORT_ADDRESS} is gone from the client entirely — students are told to email someone but given no address"
        elif [ "${address_count:-0}" -gt 1 ]; then
            bad "the support address ${SUPPORT_ADDRESS} appears ${address_count} times in client/src — it must be defined once (locales/en/home.json) and referenced by key, or the copies will drift"
        else
            ok "the support address is defined exactly once"
        fi

        # 2. The reply promise and the middleman-first rule must both be present and
        #    non-empty. A renamed key renders as the raw key string in a shipped app,
        #    which is worse than no line at all.
        for key in replyPromise middleman emailLead emailNote; do
            if ! grep -q "\"${key}\"[[:space:]]*:[[:space:]]*\"[^\"]" "$LOCALE_HOME"; then
                bad "home.json is missing a non-empty connection.supportBlock.${key} — the support block would render incomplete"
            fi
        done
        if grep -q '"replyPromise"[[:space:]]*:[[:space:]]*".*12 hours' "$LOCALE_HOME"; then
            ok "the reply promise still names the 12-hour window"
        else
            bad "the support reply promise no longer states the 12-hour window — the promise the operator made to students is gone from the screen that makes it"
        fi

        # 3. Every screen a stuck student can reach must render the block. The
        #    activation gate is the one that matters most: it is the screen BEFORE a
        #    student has any product to look things up in.
        if [ ! -f "$SUPPORT_COMPONENT" ]; then
            bad "components/shared/support-block.tsx is missing — every screen it was added to now renders nothing"
        else
            for page in activation connection account; do
                # A RENDER, not a mention. The first version of this check was
                # `grep -q SupportBlock` — which the IMPORT line satisfies — so it
                # passed with the component deleted from the page body entirely.
                # Observed failing, then rewritten to require the JSX tag.
                if ! grep -qE '<SupportBlock[ /]' "$REPO/client/src/pages/${page}.tsx"; then
                    bad "client/src/pages/${page}.tsx no longer renders SupportBlock — that screen has no route to a human again"
                else
                    ok "pages/${page}.tsx renders the support block"
                fi
            done
        fi
    fi
}
