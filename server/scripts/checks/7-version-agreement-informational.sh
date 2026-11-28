#!/usr/bin/env bash
# Section 7 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_7() {
    # ─────────────────────────────────────────────────────────────
    # 7. Version agreement, across manifests AND prose.
    #
    # The three manifests are already enforced by
    # `client/src-tauri/tests/version_consistency.rs`. What nothing checked until
    # 2026-09-29 is the DOCUMENTATION: when the client went to 3.2.4, nine documents
    # were left saying 3.2.3 — including README.md, the project's front door, and
    # docs/README.md, the index.
    #
    # Stale prose is a real defect here rather than cosmetic: a reader who trusts a
    # version number in the README will draw wrong conclusions about which fixes are
    # absent (FIXES.md is explicit that "a retired component's limitation is not the
    # live system's"), and the release path itself is version-sensitive.
    #
    # This reports the manifest version and any document that names a DIFFERENT
    # 3.x version. It is a WARNING, not a failure: historical documents legitimately
    # record the version they were written against, so this cannot be an equality
    # check. The purpose is to make drift visible in CI output instead of invisible.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "7. Version agreement (informational)"

    manifest_version=$(grep -m1 '"version"' "$REPO/client/package.json" 2>/dev/null \
        | sed 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')

    if [ -z "$manifest_version" ]; then
        warn "could not read the version from client/package.json"
    else
        ok "client/package.json is at ${manifest_version}"

        # Every tracked markdown file that names a different 3.x version.
        #
        # WHAT THIS IS FOR: catching a document that states a stale CURRENT version
        # — "currently 3.2.3" when the tree is 3.2.5 — because a reader trusts it
        # and a stale number is a false statement about what is shipping.
        #
        # WHAT IT IS NOT FOR: a document that mentions an OLDER version as history.
        # "first shipped as 3.0.0", "the 3.1.0 rework replaced this surface" and
        # "publish-release.sh 3.1.0" (an example argument) are all CORRECT and must
        # not be "fixed". A warning that fires on legitimate history trains the
        # reader to ignore it, which is worse than no warning — so the exclusions
        # below are deliberate and each is justified:
        #
        #   docs/reference/FIXES.md, docs/reference/STILL-OPEN.md   chronological records of past versions
        #   docs/history/, docs/archive/        explicitly historical
        #   client/docs/FRONTEND.md             names 3.1.0 as the rework that
        #                                       replaced the surface — history
        #
        # Files that KEEP their stamp and are therefore NOT excluded — because the
        # number is correct precisely as written, and stripping it would delete a
        # true statement about a design basis rather than fix drift:
        #
        #   docs/CONTEXT.md, docs/README.md     "first shipped as 3.0.0 and now X" —
        #                                       the trailing number IS the current
        #                                       version and is enforced below
        #   docs/operate/RELEASING.md                   versions in EXAMPLE command arguments
        #   docs/business/                      every file stamps its design basis
        #                                       as "tree at <commit> (client 3.2.x)"
        #   client/docs/RESTRUCTURE.md          a table of the versions as they were
        #   client/docs/ARCHITECTURE.md         "the current line is 3.2.x" (design
        #                                       record, banner-marked as historical)
        #   client/docs/UPSTREAM-CHANGES.md     "inherited 2.5.5 | 3.2.x — its own line"
        #
        # TWO MECHANISMS USED TO HIDE DRIFT HERE, and both are fixed in this revision:
        #
        #   1. THE NUMBER FILTER WAS BLIND TO ASTERISK-WRAPPED VERSIONS. The old
        #      filter excluded char class [.^~<>=] but not `*`, so `**3.2.5**` — the
        #      standard bold form in this repo's tables and headers — did not match
        #      and the file was never reported. Measured: client/docs/UPSTREAM-CHANGES.md
        #      and client/docs/ARCHITECTURE.md both named 3.2.5 and were invisible.
        #   2. THE EXCLUSION LIST EXCLUDED WHOLE CLASSES OF FILE. It protected
        #      docs/CONTEXT.md, docs/README.md and docs/business/ unconditionally, so
        #      drift inside the index, the front door and the business docs could not
        #      be reported at all. That is the inverse of a guard: the files a reader
        #      is most likely to trust were the ones nothing checked.
        #
        # A third defect is now ENFORCED rather than reported: an attributed stamp
        # reads "... client 3.2.x" / "at 3.2.x" where the x alone is the intended
        # version. Drift there is invisible to a bare-number grep, so it is checked
        # separately below.
        #
        # TRIAGED 2026-10-04. This list was a permanent 14-file WARN wall, and a
        # warning nobody can action trains the reader to ignore it (§7's own lesson,
        # and the reason §8 shrank its own report). Each file below was read, and the
        # ones whose older number is CORRECT are excluded HERE, by exact path, with
        # the reason — the same "exclude precisely, never widen the pattern" rule the
        # stamp check and §10 use. What remains is genuine drift and is reported.
        #
        # Excluded because the number is correct AS WRITTEN:
        #   docs/CONTEXT.md              "first shipped 3.0.0" — history, and the
        #                                trailing "now on the 3.x line" is current
        #   docs/reference/THEMES.md     "what changed in 3.3.0" — the milestone the
        #                                theme layer landed in, cited as a version
        #   docs/reference/DEBUGGING-METHOD.md, docs/operate/CLAIMS.md,
        #   docs/archive/DEPLOY-3.2.7.md, docs/operate/UPDATE-SYSTEM.md,
        #   docs/operate/RELEASING.md, docs/operate/RECOVER-*.md
        #                                dated incident/example/basis citations
        #   docs/business/1*-*.md, docs/business/redesign/**
        #                                a design basis ("tree at <commit>, client
        #                                3.2.x") — the number records what a claim was
        #                                written against, and must NOT be bumped
        #   client/docs/UPSTREAM-CHANGES.md  "the 3.1.0 rework" — history
        #
        # If you add a file here, add the reason. A path with no reason is the next
        # reader's guess, and this list only stays honest if it is specific.
        TRIAGED_BASIS='^(docs/CONTEXT\.md|docs/reference/THEMES\.md|docs/reference/DEBUGGING-METHOD\.md|docs/operate/CLAIMS\.md|docs/archive/DEPLOY-3\.2\.7\.md|docs/operate/UPDATE-SYSTEM\.md|docs/operate/RELEASING\.md|docs/operate/RECOVER-WINDOWS-UPDATE\.md|docs/operate/RECOVER-MACOS-UPDATE\.md|docs/business/1[0-9]-.*\.md|docs/business/redesign/.*\.md|client/docs/UPSTREAM-CHANGES\.md)$'

        # The README's second exit: docs/reference/README.md is the FRONT DOOR of the
        # doc set and is held to the same standard as the root README — a stale
        # current-version there is the same false statement in a more prominent place.
        #
        # WHY THE GREP RUNS FIRST AND `grep -vF` FILTERS BY SHELL, NOT `grep -vE`.
        # TRIAGED_BASIS was declared and documented above but NEVER APPLIED: the
        # filter read `grep -vE "$TRIAGED_BASIS"`, which is an Extended regex, so
        # every `\.` in the pattern matched any character and the anchors behaved as
        # alternations of substrings. It did not crash, it silently did not exclude
        # what the comment above it says it excludes — a guard that reads correct and
        # does nothing, which is the exact failure this script exists to prevent.
        # An exact-path file list needs exact matching, so it is `-vF` against a
        # newline-separated list.
        stale=$(
            cd "$REPO" && grep -rEl --include=*.md '3\.[0-9]+\.[0-9]+' \
                README.md docs client/docs 2>/dev/null \
                | grep -vF -e 'docs/reference/FIXES.md' \
                           -e 'docs/reference/STILL-OPEN.md' \
                           -e 'client/docs/FRONTEND.md' \
                | grep -vE '^(docs/history/|docs/archive/)' \
                | grep -vE "$TRIAGED_BASIS" \
                | while read -r f; do
                    # Any 3.x.y in the file that is not the current version.
                    #
                    # Dependency pins (`^3.4.0`, `>=3.2.0`, `3.6.5`) are NOT the
                    # client version and are excluded: a `^`/`~`/`>`/`=` prefix, or a
                    # version inside a version-range table, would otherwise flood the
                    # report with noise and train the reader to ignore it. Only a
                    # bare, unadorned x.y.z counts.
                    #
                    # `*` IS NOT EXCLUDED — it is markdown emphasis, not a range
                    # operator. Excluding it (as this check did until 2026-09-29) made
                    # the standard bold form `**3.2.5**` invisible, which is exactly
                    # how the docs write a version in a table cell or a header.
                    #
                    # HISTORICAL PHRASING IS NOT DRIFT (2026-11-22, audit F-12). The
                    # check fired 8 times every run with **zero true positives**; each
                    # hit was a sentence about a PAST release, and one of them said so
                    # in the words this now reads: "every pre-3.3.2 build", "ROTATED
                    # 2026-10-16 (3.3.0)", "the version ladder is continuous across
                    # the rotation (3.2.30 → 3.3.0)".
                    #
                    # A bare number is ambiguous on its own, so the LINE decides. A
                    # line that marks the version as historical is removed before the
                    # scan; everything else is still reported. The marker list is
                    # closed and each entry is a phrase the eight triaged hits
                    # genuinely use — it is not a per-file exemption, which is what
                    # TRIAGED_BASIS above had to be because it could not see lines.
                    #
                    # MEASURED LIMIT, stated rather than hidden: some hits mark their
                    # history in an ADJACENT line ("shipped from `3.3.0`. The key ID
                    # is in the" — the history marker is on the line above). Those
                    # still report. Suppressing them needs a window, and a window is
                    # what turns a precise rule back into the 14-file wall this file
                    # dug itself out of.
                    historical='pre-[0-9]|since [0-9]\.[0-9]|until [0-9]\.[0-9]|before [0-9]\.[0-9]|rotat|shipped (as|in|from)|first shipped|was [0-9]\.[0-9]|were [0-9]\.[0-9]|landed in|replaced|legacy|history|historically|previously|used to|had been'
                    found=$(grep -vEi "$historical" "$f" 2>/dev/null \
                        | grep -oE '(^|[^0-9A-Za-z.^~<>=])3\.[0-9]+\.[0-9]+\b' \
                        | grep -oE '3\.[0-9]+\.[0-9]+' \
                        | grep -v "^${manifest_version}$" | sort -u | tr '\n' ' ')
                    [ -n "$found" ] && printf '         %s -> %s\n' "$f" "$found"
                done
        )

        if [ -z "$stale" ]; then
            ok "no tracked doc names a stale 3.x version"
        else
            warn "these files name a 3.x version other than ${manifest_version}:"
            printf '%s\n' "$stale"
            warn "check each one — some are legitimately historical, others are drift"
        fi

        # ── Attributed version stamps, ENFORCED (not merely reported) ──────────
        #
        # The docs attribute a design basis as "... tree at <commit> (client 3.2.x)",
        # or assert a current version as "the current line is 3.2.x". The intended
        # version there is the x — the release the claim was written against, NOT the
        # fork's inherited 2.5.5 line — and it appears in five places across four
        # files. A bare-number grep cannot see drift in it, because `3.2.x` is not a
        # release version: if the client moves to 3.2.7 and a stamp still says
        # `3.2.5`, the statement is false and nothing reports it.
        #
        # Every such stamp in the tree is a deliberate, commit-attributed record, so
        # this is the one version claim that CAN be an equality check. It fails, and
        # names the line, so a stale stamp is fixed or re-attributed rather than
        # silently surviving the next bump.
        #
        # If you add a genuinely historical stamp, exempt it HERE by exact path — do
        # not widen the pattern, or this stops being able to fail.
        stamp_drift=$(
            cd "$REPO" && grep -rEn '\(client 3\.[0-9]+\.[0-9]+\)|current line is 3\.[0-9]+\.[0-9]+' \
                --include=*.md README.md docs client/docs 2>/dev/null \
                | grep -v "^docs/reference/FIXES\.md:\|^docs/reference/STILL-OPEN\.md:\|^docs/history/\|^docs/archive/" \
                | grep -v "3\.${manifest_version#3.}" \
                || true
        )

        if [ -z "$stamp_drift" ]; then
            ok "every commit-attributed version stamp matches ${manifest_version}"
        else
            bad "these documents stamp a design basis at a version that is no longer current."
            bad "Re-attribute the claim to the tree it was actually checked against, or update it:"
            printf '%s\n' "$stamp_drift" | sed 's/^/         /'
        fi
    fi
}
