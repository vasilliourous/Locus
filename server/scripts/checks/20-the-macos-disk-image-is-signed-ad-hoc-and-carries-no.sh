#!/usr/bin/env bash
# Section 20 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_20() {
    echo
    echo "20. The macOS disk image is signed ad-hoc, and carries no README"
    # ─────────────────────────────────────────────────────────────
    # WHY THIS EXISTS
    #
    # This section used to assert the OPPOSITE: that the .dmg carried a
    # "READ ME FIRST.txt" telling the student to run `xattr -cr` in Terminal.
    # That mitigation is gone, and the reason it is gone is worth keeping, because
    # the file looked like the fix for a year and was not.
    #
    # WHAT WAS WRONG WITH IT
    #
    # The file existed because an unsigned, quarantined .app produced
    #
    #   "Locus" is damaged and can't be opened. You should move it to the Trash.
    #
    # — and the README told the student that message was expected and what to type.
    # That is true, and it is still true that the wording cannot be fixed without an
    # Apple account. What the README could not fix is that the remedy required a
    # TERMINAL: a student who does not know what a command line is had no path
    # forward at all short of following instructions to type a command they cannot
    # evaluate. A text file next to the app is documentation for a failure the
    # student has to escape, not a fix for it.
    #
    # WHAT REPLACED IT
    #
    # Ad-hoc signing. `codesign --sign -` does not make the app trusted — there is
    # still no Developer ID and no notarization — but it changes WHICH Gatekeeper
    # outcome the student meets, and that difference is the whole point:
    #
    #   unsigned  -> "Chrysalis is damaged and can't be opened"   -> NO "Open Anyway"
    #   ad-hoc    -> "Apple cannot check it for malicious
    #                software" (unidentified developer)        -> "Open Anyway" in
    #                                                             Privacy & Security
    #
    # Gatekeeper treats a signature that exists but is not a Developer ID as an
    # identity/notarization problem, not as corruption, so the app moves from the
    # unbypassable class to the bypassable one. The remedy becomes a control in
    # System Settings instead of a command in Terminal.
    #
    # WHY "SIGNED, NOT VERIFIED" IS NOT A GUARD
    #
    # A step that runs `codesign` and does not read the result back is indistinguish-
    # able from one that silently did nothing — the same shape as `hdiutil create`
    # exiting 0 on an image that lost the file (which is why the old section mounted
    # the .dmg back). So this check does not assert that a `codesign` line exists.
    # It asserts that the workflow ALSO verifies: `codesign --verify`, a
    # `spctl`/assessment read, or an explicit artifact read-back. Without that half,
    # a runner where signing failed would publish an unsigned installer and nothing
    # in the tree would say so.
    #
    # WHAT THIS CANNOT CHECK
    #
    # Like §14 this is a grep over the tree, not a mount or a launch. It cannot prove
    # the signature reaches a student, and it cannot prove the "Open Anyway" button
    # appears on a given macOS version — that needs real hardware, and it is recorded
    # as unverified in `docs/reference/STILL-OPEN.md`. What it catches is the change
    # that would make the mechanism dead: deleting the signing step, dropping its
    # verification half, or reintroducing the README it replaced.
    # ─────────────────────────────────────────────────────────────

    # ── The README is GONE, and must stay gone ──
    #
    # Stated as an assertion so that reintroducing the file is a deliberate act that
    # turns a check red, rather than a change that quietly restores a Terminal-only
    # path alongside the signing that made it unnecessary. The `xattr -cr` advice is
    # not wrong, it is just no longer the answer, and two remedies that disagree
    # about which is primary is worse than one.
    MACOS_README="$REPO/client/src-tauri/packages/macos/READ ME FIRST.txt"
    if [ -f "$MACOS_README" ]; then
        bad "client/src-tauri/packages/macos/READ ME FIRST.txt is back"
        bad "  ad-hoc signing replaced it: it moves the Gatekeeper failure from the"
        bad "  unbypassable 'damaged' dialog into Privacy & Security -> Open Anyway."
        bad "  If the README is genuinely needed again, say so in docs/operate/OPS.md"
        bad "  and update this section — do not leave both remedies in the tree."
    else
        ok "the macOS in-image README is gone (replaced by ad-hoc signing)"
    fi

    # No workflow step may copy it back in. This catches the case where the file is
    # deleted but the repack step that references it survives — which would fail at
    # build time on a macOS runner and pass everywhere else, including here.
    #
    # MATCHES THE COPY, NOT THE NAME. A bare `grep READ ME FIRST` also matches the
    # comment that explains WHY the file was removed — so this check failed on its
    # own documentation the first time it ran, and the tempting fix (delete the
    # explanation) would have left the guard passing for the wrong reason while the
    # reason for the removal was lost. That is the "check that cannot distinguish a
    # right answer from a wrong one" trap §14 and the old §20 both describe. What is
    # asserted is the MECHANISM: a `cp` (or an `install`) whose source is that path,
    # or a step that reads it back out of the image. Prose cannot satisfy either.
    if [ -f "$WORKFLOW" ]; then
        readme_copy=$(grep -nE '(cp|install)[^|]*READ ME FIRST\.txt|\[ -f [^]]*READ ME FIRST\.txt' "$WORKFLOW" || true)
        if [ -n "$readme_copy" ]; then
            bad "the workflow still copies/reads READ ME FIRST.txt"
            printf '%s\n' "$readme_copy" | head -5 | sed 's/^/         /'
        else
            ok "no workflow step copies READ ME FIRST.txt into the image"
        fi

        # ── The signing step, and the half that makes it real ──
        #
        # `codesign ... --sign -` is the ad-hoc form: the identity is a literal
        # hyphen, not a name. A Developer-ID identity would look different, and this
        # check would then need rewriting — which is correct, because that is a
        # different mechanism with different claims.
        if grep -qE 'codesign[^|]*--sign[[:space:]]+-' "$WORKFLOW"; then
            ok "a workflow step ad-hoc signs the macOS bundle"
        else
            bad "no ad-hoc codesign step found in the workflow"
            bad "  without it a quarantined .app shows 'damaged and can't be opened',"
            bad "  which offers no 'Open Anyway' and can only be escaped in Terminal"
        fi

        # The read-back. Signing that is not verified is the "check that cannot fail"
        # trap in its most literal form: `codesign --sign` on a bundle it cannot
        # process exits non-zero, but a step whose failure is swallowed by `||true`,
        # a `continue-on-error`, or a missing `set -e` publishes the unsigned image
        # while the log shows the signing line having run.
        if grep -qE 'codesign[^|]*(--verify|-v[[:space:]])' "$WORKFLOW"; then
            ok "the signing step is read back with a verification of its own"
        else
            bad "the ad-hoc signing step is never verified"
            bad "  add a codesign --verify (or an spctl/artifact read-back) after signing,"
            bad "  so a runner where signing failed cannot publish an unsigned image"
        fi

        # macOS-only, for the same reason the old repack step had to be: `codesign`
        # and `hdiutil` do not exist on the Linux and Windows runners, so an
        # ungated step is a hard failure on two of four platforms rather than a
        # no-op. Gating on `matrix.label` rather than `matrix.os` is the lesson §18
        # and §19 encode — `os` gets bumped (macos-14 -> macos-15-intel) and detaches
        # every condition written against the old value.
        if grep -qE "if: *startsWith\(matrix\.label, *'macos'\)" "$WORKFLOW"; then
            ok "the macOS signing/repack steps are gated on matrix.label"
        else
            bad "the macOS-only steps are not gated on matrix.label starting with 'macos'"
        fi
    else
        warn "workflow not found at ${WORKFLOW}; skipping the signing-step checks"
    fi

    # ── The live docs must not still promise the README as PRESENT ──
    #
    # This is the drift that made the original section necessary in reverse: the
    # Tauri rewrite dropped the file while prose kept claiming it was there. The
    # same failure is available in the other direction — a doc left asserting the
    # image carries instructions that no longer exist, so a student (or an operator
    # answering a support ticket) is told to look for a file that is not there.
    #
    # WHAT THIS CANNOT DO, stated because the first version of this check got it
    # wrong: a grep cannot tell "the image carries a README" from "the README was
    # removed, and here is why". Banning the name outright made the check fail on
    # its own explanation, and the cheapest way to make it green would have been to
    # delete the explanation — leaving the guard passing for the wrong reason and
    # the reasoning lost. So the assertion is on the OPERATIONAL verb: a sentence
    # that says the artifact *carries*, *has*, or *includes* the file, or tells the
    # reader to *check/look for/find* it in the image. A doc that narrates the
    # removal does not match any of them.
    #
    # The negations are filtered out deliberately, and this is the third shape this
    # check took. `no longer carries`, `does not carry` and `no README` are sentences
    # that agree with the removal; a rule that flags them forces the writer to stop
    # explaining, which is the opposite of what this file is for. The filter is
    # narrow — it matches only an explicit negation immediately before the verb —
    # so "the image carries a README" still fails while "the image no longer carries
    # a README" passes.
    OPS_DOC="$REPO/docs/operate/OPS.md"
    if [ -f "$OPS_DOC" ]; then
        stale_claim=$(grep -nEi '(carries|has|includes|ships)[^.]*READ ME FIRST|(look|check|find) for[^.]*READ ME FIRST|hdiutil attach[^.]*READ ME FIRST' "$OPS_DOC" \
            | grep -viE 'no longer|does not|doesn.t|no README|not carry|never|removed' || true)
        if [ -n "$stale_claim" ]; then
            bad "docs/operate/OPS.md still says the macOS image carries a README"
            printf '%s\n' "$stale_claim" | head -3 | sed 's/^/         /'
            bad "  fix the prose in the same change that removes the file (README rule 3)"
        else
            ok "OPS.md does not promise a README in the macOS image"
        fi
    fi
}
