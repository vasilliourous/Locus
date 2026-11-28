#!/usr/bin/env bash
# Section 1c -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_1c() {
    # ─────────────────────────────────────────────────────────────
    # 2. Frozen wire names.
    #
    # Both directions matter: the hub must keep EMITTING the old name (deployed
    # clients read it) and the client must keep READING it (the hub passes the
    # stored key through verbatim).
    # ─────────────────────────────────────────────────────────────
    # 1c. The macOS HUMAN download — named the same in CI, the hub and the console.
    #
    # WHY THIS EXISTS
    #
    # The `.zip` is the one artifact that is deliberately NOT an update payload: it
    # is the compressed, ad-hoc-signed `Locus.app` a PERSON downloads. The Windows
    # outage of 2026-10-01 was a filename that had to agree across three systems and
    # was checked on only one of them, so this name gets the same treatment from the
    # start rather than after it drifts.
    #
    # Three sites must agree:
    #
    #   1. CI       — `.github/workflows/client.yml`, "Package the macOS app for
    #                 download", which BUILDS `Locus_<v>_<arch>.zip` and the release
    #                 job which prefixes it to `installer-...`
    #   2. the hub  — `fetch-release.py`'s `MACOS_ZIP_TEMPLATE`, which RESOLVES it
    #                 from the GitHub Release
    #   3. console  — `Releases.vue`'s `MACOS_ARCHES`, which LINKS to it
    #
    # And one NEGATIVE, which is the property that actually protects the fleet: the
    # name must NOT appear in the update path (`PLATFORMS`,
    # `resolve_platform_names`, CI's manifest platform map). A compressed bundle in
    # an update slot is the raw-Windows-PE mistake in a new costume.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "1c. macOS human download — one name, three sites, and never an update payload"

    # `WORKFLOW` is assigned again further down for the sections that also use it.
    # It is set here as well because this section runs FIRST and would otherwise
    # abort on an unbound variable under `set -u` — which is how this check failed
    # the first time it ran, before it had checked anything at all.
    WORKFLOW="$REPO/.github/workflows/client.yml"

    # The template must exist, in exactly one place per side, and there must be TWO:
    # the current `productName` and the pre-move one. Both are asserted, so neither
    # can be dropped alone — see the §1e pair check and NAME-MIGRATION.md §3.1.
    if grep -qE '^MACOS_ZIP_TEMPLATE *= *"installer-Chrysalis_%s_%s\.zip"' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        ok "fetch-release.py defines the current macOS zip template once"
    else
        bad "fetch-release.py has no single MACOS_ZIP_TEMPLATE definition"
        bad '  expected: MACOS_ZIP_TEMPLATE = "installer-Chrysalis_%s_%s.zip"'
    fi
    if grep -qE '^LEGACY_MACOS_ZIP_TEMPLATE *= *"installer-Locus_%s_%s\.zip"' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        ok "fetch-release.py still resolves the pre-move macOS zip name"
    else
        bad "fetch-release.py has no LEGACY_MACOS_ZIP_TEMPLATE — a release built before"
        bad "  the move would read as missing its macOS download instead of resolving it"
    fi

    # The arch suffixes are a contract too: CI writes `_amd64`/`_arm64`, and the hub
    # resolves the same two. A third arch added on one side only would be fetched
    # forever by nobody.
    for arch in amd64 arm64; do
        if grep -q "\"$arch\"" "$SCRIPTS/fetch-release.py" 2>/dev/null; then
            ok "the hub knows the '$arch' macOS zip"
        else
            bad "fetch-release.py does not list '$arch' in MACOS_ZIP_ARCHES"
        fi
    done

    # CI must produce both, and the release job must assert them present. The
    # assertion is what stops a build job that silently stopped packaging from
    # publishing a release with no macOS download at all.
    # AS OF 2026-11-25 THIS ASSERTS THE DERIVATION, NOT A LITERAL. The old form
    # required the string `Locus_${ver}_${arch}.zip`, which meant the guard's
    # correctness depended on the product never being renamed — and when the zip
    # literal was updated to the new name it went red on a CORRECT tree while
    # passing on the tree that shipped the v3.5.0 defect. What must be true is that
    # the build job writes a versioned, arch-bearing zip whose product infix is
    # READ FROM THE CONFIG; §1f is what refuses a stale literal.
    if grep -qE 'out="installer/\$\{APP_NAME%?\.?a?p?p?\}_\$\{ver\}_\$\{arch\}\.zip"|out=.*\$\{ver\}_\$\{arch\}\.zip' "$WORKFLOW" 2>/dev/null; then
        ok "CI builds the macOS zip under the versioned name, product infix from config"
    else
        bad "no CI step builds a '<productName>_<v>_<arch>.zip' macOS download"
        bad "  the macOS packaging step is what makes the bypassable Gatekeeper path exist"
        bad "  the infix is productName, which is 'Locus' — see NAME-MIGRATION.md §3.1"
    fi
    if grep -q 'expected two macOS download zips' "$WORKFLOW" 2>/dev/null; then
        ok "the release job refuses to publish without both macOS zips"
    else
        bad "the release job does not assert the macOS zips are present"
        bad "  a packaging step that stopped running would publish a release with none"
    fi

    # The console must name the same shape, or the link 404s for the student.
    # The release read-out moved under Settings in the console redesign; this name
    # follows the file rather than the old path, so the check keeps checking the code
    # that exists instead of warning that it cannot find it.
    CONSOLE_VUE="$REPO/server/console/src/views/settings/ReleasesPanel.vue"
    # The console's source root and the two files §27 reads out of it. The redesign
    # moved the mint form into views/codes/CreateDrawer.vue and the release read-out
    # into views/settings/ReleasesPanel.vue; these names follow the code so a reader
    # is not sent to a file that no longer exists.
    CONSOLE_SRC="$REPO/server/console/src"
    CONSOLE_CREATE="$CONSOLE_SRC/views/codes/CreateDrawer.vue"
    CONSOLE_CODES="$CONSOLE_SRC/views/Codes.vue"
    if [ -f "$CONSOLE_VUE" ]; then
        if grep -q 'installer-Locus_${v}_${a.arch}.zip' "$CONSOLE_VUE" 2>/dev/null; then
            ok "the console builds the same zip filename"
        else
            bad "the console does not construct 'installer-Locus_<v>_<arch>.zip'"
            bad "  its download link would point at a file the hub did not fetch"
        fi
        # The console must NOT enumerate the zip among the update platforms, or the
        # "all four platforms published" gate would start counting a human download.
        if grep -qE "zip_macos_amd64', *filename: *'chrysalis-" "$CONSOLE_VUE" 2>/dev/null; then
            bad "the console treats the macOS zip as an update platform"
        else
            ok "the console keeps the macOS zip out of the platform list"
        fi
    else
        warn "console Releases.vue not found; skipping the console half of this check"
    fi

    # ── The negative, and the reason this section exists ──
    #
    # Everything above is about a name agreeing. This is about the name being in the
    # WRONG PLACE, which is the failure that cost three releases on Windows. Checked
    # on both sides of the hub, because either alone would be defeated by the other
    # being edited.
    if grep -qE '^\s*\("macos_(intel|arm).*\.zip' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        bad "fetch-release.py lists a .zip inside PLATFORMS — the update path"
        bad "  a client would be handed an archive it cannot install from"
    else
        ok "PLATFORMS in fetch-release.py carries no .zip"
    fi

    if grep -qE '"macos_(intel|arm)"[[:space:]]*:[[:space:]]*"[^"]*\.zip"' "$WORKFLOW" 2>/dev/null; then
        bad "CI's manifest platform map advertises a .zip for a macOS platform"
    else
        ok "CI's manifest advertises no .zip as an update payload"
    fi
    if grep -q 'refusing to publish: the manifest names a .zip' "$WORKFLOW" 2>/dev/null; then
        ok "the manifest step actively refuses a .zip platform entry"
    else
        bad "the manifest step does not guard against a .zip platform entry"
    fi
}
