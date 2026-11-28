#!/usr/bin/env bash
# Section 18 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_18() {
    echo
    echo "18. The Windows installer is signed, under the name the release job prefixes"
    # The `installer-` prefix is added by the RELEASE job's "Prefix and stage the
    # installers" step. The SIGN step runs in the BUILD job, where the file still
    # carries Tauri's raw name. A glob written against the prefixed name in the build
    # job can never match — and because the step exits 1 in that case, the cost is a
    # ~30 minute four-platform build that ends in "no installer to sign".
    #
    # Two separate defects lived here and both shipped:
    #   1. the step was gated on `matrix.os == 'windows-latest'` while the matrix
    #      pins `windows-2022`, so it never ran at all (see §18's sibling check);
    #   2. once it did run, it globbed `installer-Locus_*` in a directory that only
    #      ever holds `Locus_*`.
    # Both are the same failure shape: a name asserted in one half and not the other.
    # This check asserts the AGREEMENT between the two jobs rather than either half
    # (AGENTS.md, "assert the agreement").
    wf_installer="$WORKFLOW"
    if [ -f "$wf_installer" ]; then
        # Scoped to a glob that names an `installer/` entry deliberately. Several
        # steps iterate `installer/*` now (the macOS .dmg repack does), and a bare
        # `head -1` would grab whichever comes first in the file — so this check would
        # silently stop testing the step it exists for, which is the failure mode it
        # is about.
        #
        # THE PRODUCT WORD IS NO LONGER PART OF THE PATTERN, and that is the fix for a
        # real defect. This used to require `installer/Locus_*`, which meant it
        # passed on the tree where the step globbed a product name that no build
        # produced (so the step matched nothing and exited 1) and would have gone red
        # on the corrected tree. What the step must do is glob for the STAGED
        # INSTALLER, whichever spelling productName currently produces — §1f is what
        # refuses a stale literal.
        sign_glob="$(grep -oE 'for f in "?installer/[^;]+_x64-setup\.exe' "$wf_installer" | head -1 || true)"
        prefix_src="$(grep -oE 'cp "\$f" "release/installer-\$base"' "$wf_installer" | head -1 || true)"
        # The `if:` line follows the step name, but a comment block may sit between
        # them (the file explains WHY at the point of contact). Scan forward from the
        # name to the next `if:` or the next `- ` step boundary, whichever comes first.
        sign_cond="$(python3 - "$wf_installer" <<'PY'
import re, sys

lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
cond = ""
for i, line in enumerate(lines):
    if line.strip().startswith("- name:") and "Sign the Windows installer" in line:
        for follow in lines[i + 1:]:
            s = follow.strip()
            if s.startswith("- name:"):
                break            # next step: no if: on this one
            if s.startswith("if:"):
                cond = s
                break
        break
print(cond)
PY
    )"

        if [ -z "$sign_glob" ]; then
            bad "the \"Sign the Windows installer\" step no longer globs a staged installer"
            bad "  if that step was removed, the Windows installer ships unsigned and no"
            bad "  Windows client can install the update (RECOVER-WINDOWS-UPDATE.md)"
        else
            # The glob must NOT carry the `installer-` prefix: it runs before the
            # rename, so a prefixed pattern can never match.
            #
            # THE INFIX IS CHECKED AS "NOT A STALE PRODUCT WORD" rather than as a
            # literal. It is `productName`, and §1e already pins that string against
            # the installer the hub resolves; requiring `Locus_` here made this check
            # pass on the tree where the glob matched nothing and fail on the fix.
            case "$sign_glob" in
                *"installer/"*Locus_*|*"installer/"*Chrysalis_*)
                    # A literal product word: correct only if it is the CURRENT one.
                    _sg_word="$(printf '%s' "$sign_glob" | grep -oE 'Locus_|Chrysalis_' | head -1)"
                    if [ "$_sg_word" = "${_wn_product:-Locus}_" ]; then
                        ok "the sign step globs the pre-prefix name (installer/${_sg_word}*)"
                    else
                        bad "the sign step globs '${sign_glob#for f in }' in the BUILD job"
                        bad "  the 'installer-' prefix is added later by the release job, so this"
                        bad "  can never match and the step exits 1 after a full four-platform build"
                        bad "  the infix is productName, which is '${_wn_product}' — see NAME-MIGRATION.md §3.1"
                    fi
                    ;;
                *installer/*)
                    # Derived from the config (`installer/${PRODUCT}_…`): cannot go stale.
                    ok "the sign step globs the pre-prefix name from productName (${sign_glob#for f in })"
                    ;;
                *)
                    bad "the sign step globs '${sign_glob#for f in }' in the BUILD job"
                    bad "  the 'installer-' prefix is added later by the release job, so this"
                    bad "  can never match and the step exits 1 after a full four-platform build"
                    bad "  the infix is productName, which is 'Locus' — see NAME-MIGRATION.md §3.1"
                    ;;
            esac
        fi

        if [ -n "$prefix_src" ]; then
            ok "the release job prefixes the installer (release/installer-\$base)"
        else
            bad "the release job's installer-prefixing step is missing or renamed"
            bad "  the hub, the manifest and fetch-release.py all resolve"
            bad "  installer-Locus_<v>_x64-setup.exe, so without the prefix no"
            bad "  Windows client is offered a file it can install"
        fi

        if [ -z "$sign_cond" ]; then
            warn "could not read the sign step's if: condition; skipping its check"
        elif printf '%s' "$sign_cond" | grep -q "matrix.label"; then
            ok "the sign step is gated on matrix.label (${sign_cond#*if: })"
        else
            bad "the sign step is gated on ${sign_cond#*if: }, not matrix.label"
            bad "  gate platform steps on matrix.label: matrix.os is a runner image that"
            bad "  gets bumped (macos-14 -> macos-15-intel) and silently detaches the condition"
        fi
    fi

    # The "Verify every artifact is signed" step's glob must name the REAL updater
    # prefix.
    #
    # This guard exists because that glob was renamed and the artifacts were not.
    # `refactor!: rename the product from Locus to Chrysalis` (2026-11-22) changed
    # `for f in release/locus-*` to `for f in release/chrysalis-*` in that step, but
    # the payloads stayed `locus-*` — `fetch-release.py` resolves those names from
    # an allowlist, so they are a frozen contract. The glob then matched nothing,
    # verified nothing, and the v3.5.0 tag failed at `Release` with the bare literal
    # `chrysalis-*` as its only diagnostic, after a full four-platform build.
    #
    # A glob that matches nothing is invisible: it does not trip `set -u`, the loop
    # body never runs, and the step exits 0. So the prefix is asserted here rather
    # than trusted.
    # SINCE 2026-11-25 THE STEP MUST GLOB BOTH SETS. The original lesson stands —
    # a glob naming the wrong prefix matches nothing and the step passes having
    # verified nothing — but the fix is no longer "the prefix is locus-*": CI now
    # publishes `chrysalis-*` as primary and `locus-*` as the alias, so a glob for
    # either alone leaves the other unverified. The one that would go unchecked is
    # whichever the glob omits, which today is the set the hub PREFERS.
    #
    # So this asserts the union, not a literal: both prefixes must appear in the same
    # loop, and the current one must be the one whose emptiness is FATAL (the aliases
    # are a migration artifact, so their absence is a warning rather than a failure).
    if grep -qE 'for f in release/chrysalis-\* release/locus-\*; do' "$WORKFLOW" 2>/dev/null; then
        ok "the signature check globs both name sets (chrysalis-* and locus-*)"
    elif grep -qE 'for f in release/(locus|chrysalis)-\*; do' "$WORKFLOW" 2>/dev/null; then
        bad "the \"Verify every artifact is signed\" step globs only ONE name set"
        bad "  CI publishes chrysalis-* as primary and locus-* as the alias; a glob for"
        bad "  one prefix leaves the other's signatures unchecked while the step passes"
    else
        bad "the \"Verify every artifact is signed\" step does not glob the updater payloads"
        bad "  the payloads are chrysalis-* (current) and locus-* (deprecated alias); a"
        bad "  glob naming neither matches nothing, so the step verifies nothing"
    fi

    if grep -q 'no updater payload matched release/chrysalis-\*' "$WORKFLOW" 2>/dev/null; then
        ok "the signature check asserts the CURRENT set's glob matched something"
    else
        bad "the signature check does not assert that its current-name glob matched anything"
        bad "  an empty glob exits 0; without this, 'nothing to check' is"
        bad "  indistinguishable from 'everything checked and passed'"
    fi
}
