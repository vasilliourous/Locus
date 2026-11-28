#!/usr/bin/env bash
# Section 24 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_24() {
    echo
    echo "24. The macOS updater payload is a .app.tar.gz, and three sites agree on that"
    #
    # WHY THIS EXISTS
    #
    # A macOS student on 3.2.24 reported "the update wasn't offered". The hub half of
    # the cause was that BOTH sides named a bare Mach-O (`locus-darwin-arm64`) in a
    # macOS update slot, while `tauri_plugin_updater` extracts a tar of an `.app`
    # bundle on macOS. The download verified and the install could not work.
    #
    # Nothing checked this, so it shipped. §1 covers the Windows installer name and
    # §1c covers the macOS human-download zip, but no section covered what a macOS
    # CLIENT downloads to replace itself — which is why the mismatch survived.
    #
    # FOUR sites must agree, and this asserts the AGREEMENT rather than each one:
    #
    #   1. CI       — `.github/workflows/client.yml`, the manifest's platform map
    #   2. the hub  — `fetch-release.py` PLATFORMS
    #   3. publish  — `publish-release.sh` PLATFORMS
    #   4. the doc  — docs/operate/UPDATE-SYSTEM.md, which tells an operator what
    #                 each slot carries
    #
    # And one NEGATIVE, the property that actually protects the fleet: a bare Mach-O
    # must NOT appear as a macOS update payload anywhere. That is the defect this
    # section was written for.
    WORKFLOW="$REPO/.github/workflows/client.yml"
    FETCH="$SCRIPTS/fetch-release.py"
    PUBLISH="$SCRIPTS/publish-release.sh"

    for target in "$WORKFLOW" "$FETCH" "$PUBLISH"; do
        [ -f "$target" ] || { bad "macOS updater guard cannot run: $target is missing"; continue; }
        want_tgz='locus-darwin-(amd64|arm64)\.app\.tar\.gz'
        if grep -Eq "$want_tgz" "$target"; then
            ok "$(basename "$target") names the macOS payload as .app.tar.gz"
        else
            bad "$(basename "$target") does not name a .app.tar.gz for a macOS update slot"
        fi
    done

    # The negative: a bare `locus-darwin-<arch>` must not be what a macOS slot
    # RESOLVES TO. Checked on the PLATFORMS declarations specifically, not on the
    # whole file: `fetch-release.py` keeps a `LEGACY_MACOS_NAMES` map holding the old
    # bare names ON PURPOSE, as the narrow fallback for releases published before the
    # packaging step. A whole-file grep cannot tell that deliberate fallback from an
    # accidental re-advertisement, and a guard that fails on intent-correct code is
    # one people learn to bypass.
    #
    # So this reads the lines that BUILD the payload list.
    if [ -f "$FETCH" ]; then
        # The artefact names are a DEPRECATED ALIAS (docs/reference/NAME-MIGRATION.md),
        # so a bare `locus-darwin-<arch>` is what must not reach an update slot.
        if sed -n '/^NEW_PLATFORMS = \[/,/^\]/p' "$FETCH" | grep -E 'locus-darwin-(amd64|arm64)["'"'"',:)]' \
            | grep -v 'app\.tar\.gz' >/dev/null 2>&1; then
            bad "fetch-release.py's NEW_PLATFORMS advertises a bare Mach-O in a macOS slot"
        else
            ok "fetch-release.py's NEW_PLATFORMS names no bare Mach-O"
        fi
    fi
    if [ -f "$PUBLISH" ]; then
        if sed -n '/^PLATFORMS=(/,/^)/p' "$PUBLISH" | grep -E 'locus-darwin-(amd64|arm64)' \
            | grep -v 'app\.tar\.gz' >/dev/null 2>&1; then
            bad "publish-release.sh's PLATFORMS advertises a bare Mach-O in a macOS slot"
        else
            ok "publish-release.sh's PLATFORMS names no bare Mach-O"
        fi
    fi
    if [ -f "$WORKFLOW" ]; then
        # The manifest's platform map, which is the declaration CI publishes.
        if sed -n '/platforms = {/,/}/p' "$WORKFLOW" | grep -E '"macos_(intel|arm)":' \
            | grep -v 'app\.tar\.gz' >/dev/null 2>&1; then
            bad "client.yml's manifest platform map advertises a bare Mach-O in a macOS slot"
        else
            ok "client.yml's manifest platform map names no bare Mach-O"
        fi
    fi

    # The client side of the contract: the updater's own guard must accept a gzip
    # and refuse a Mach-O. A hub that publishes the right artifact is useless if the
    # client refuses it, or — worse — if the client accepts the wrong one.
    INSTALL_RS="$REPO/client/src-tauri/src/chrysalis/update/install.rs"
    if [ -f "$INSTALL_RS" ]; then
        if grep -q 'app\.tar\.gz\|gzip' "$INSTALL_RS"; then
            ok "the client's payload guard is aware of the macOS tarball"
        else
            warn "install.rs does not mention the macOS tarball — check is_installer_payload still accepts it"
        fi
        # The Mach-O refusal is what makes a bare binary fail loudly rather than
        # fall through as an unrecognised "container".
        #
        # Anchored on the actual MAGIC BYTES, not the words "Mach-O": those also
        # appear in the doc comment explaining the rule, so a word-level grep stays
        # green with the matching arms deleted — the same "check that cannot fail"
        # defect §23 hit on its first draft. The escaped byte literals are the code.
        # Scoped to `is_bare_executable`'s body, NOT the whole file: the same magic
        # bytes appear in this file's own TESTS, so a whole-file grep stays green
        # with the implementation arm deleted. (It did — that is why this is scoped.)
        if sed -n '/^fn is_bare_executable/,/^}/p' "$INSTALL_RS" \
            | grep -Fq 'xcf\xfa\xed\xfe'; then
            ok "the client's payload guard matches Mach-O magic (a bare macOS binary is refused, not forwarded)"
        else
            bad "install.rs's is_bare_executable does not match Mach-O magic — a bare macOS binary would fall through as an unknown container"
        fi

        # (f2) The tarball needs its OWN `.sig`, and the upload must carry it.
        #      minisign signs exact bytes and the plugin verifies the bytes it
        #      DOWNLOADED — which on macOS is the tarball, not the Mach-O inside it.
        #      Shipping the raw binary's `.sig` alongside the tarball is a release
        #      that downloads, fails verification, and installs nothing.
        if grep -q 'app\.tar\.gz\.sig' "$WORKFLOW" 2>/dev/null; then
            ok "CI uploads the macOS tarball's own .sig"
        else
            bad "CI does not upload the macOS tarball's .sig — the updater would verify the tarball against the raw binary's signature and refuse it"
        fi
        # And the hub must accept that name.
        if grep -q 'app\.tar\.gz\.sig\|name + "\.sig"' "$FETCH" 2>/dev/null; then
            ok "the hub's allow-list admits the tarball's .sig"
        else
            bad "fetch-release.py would refuse the macOS tarball's .sig as an unknown filename"
        fi

        # (f3) The RELEASE must actually PUBLISH the tarballs. This is the check
        #      whose absence let a green run produce a release whose manifest
        #      advertised files the release did not carry:
        #
        #        manifest.json:  macos_arm -> locus-darwin-arm64.app.tar.gz
        #        release assets: locus-darwin-arm64  (the bare binary)
        #
        #      Both halves were individually valid — only the agreement was broken —
        #      so nothing failed. The `Create the release` step's `files:` list is
        #      what decides the assets, and it must name the tarballs.
        #
        #      Scoped to the `files:` block itself, not the surrounding step: the
        #      step's own comment quotes the tarball name while explaining the bug,
        #      so a step-wide grep stays green with the entry deleted. (It did — this
        #      is the third assertion in this section to make that mistake, which is
        #      why every one of them is now checked against a deliberate break.)
        release_files=$(sed -n '/^ *files: |$/,/^ *draft: false/p' "$WORKFLOW" \
            | sed -n '/^ *release\//p')
        #      EITHER SPELLING, because the tarball cannot be published under the
        #      pre-move one: the alias step renames `.../chrysalis-*` to `.../locus-*`
        #      and so cannot match a filename already ending `.app.tar.gz`, so no
        #      `locus-darwin-*.app.tar.gz` has ever been produced. Requiring the
        #      `locus-` spelling made this guard the cause of a broken release rather
        #      than its detector: v3.5.1's `fail_on_unmatched_files` aborted on it
        #      after every platform had built.
        if printf '%s\n' "$release_files" | grep -qE '(locus|chrysalis)-darwin-.*\.app\.tar\.gz$'; then
            ok "the release publishes the macOS .app.tar.gz assets"
        else
            bad "the release's file list omits the macOS .app.tar.gz — the manifest would advertise files the release does not carry"
        fi
        #      And its signature, for the reason in (f2).
        if printf '%s\n' "$release_files" | grep -qE '(locus|chrysalis)-darwin-.*\.app\.tar\.gz\.sig$'; then
            ok "the release publishes the tarball signatures too"
        else
            bad "the release's file list omits the macOS tarball .sig — the updater verifies mandatorily and would refuse the download"
        fi

        # (g) ORDERING: the `update-*` upload must come AFTER the macOS packaging
        #     step. This is not style — the tarball does not exist until the bundle
        #     has been signed and packed, so an upload above it ships a payload set
        #     with no macOS artifact.
        #
        #     This is exactly what happened on the first 3.2.27 run: all four builds
        #     passed, packaging printed a clean 85 MB tarball, and the release job
        #     then failed with "refusing to publish a partial release — missing:
        #     chrysalis-darwin-*.app.tar.gz". The manifest guard caught it correctly, but
        #     only after a ~30 minute build. This makes the same mistake a two-second
        #     local failure instead.
        pack_line=$(grep -n 'name: Package the macOS updater payload' "$WORKFLOW" | head -1 | cut -d: -f1)
        upload_line=$(grep -n 'name: update-\${{ matrix.label }}' "$WORKFLOW" | head -1 | cut -d: -f1)
        if [ -z "$pack_line" ] || [ -z "$upload_line" ]; then
            warn "could not locate the macOS packaging and update-upload steps — ordering unchecked"
        elif [ "$upload_line" -gt "$pack_line" ]; then
            ok "the update upload is below the macOS packaging step (order is correct)"
        else
            bad "the update-* upload (line $upload_line) is ABOVE the macOS packaging step (line $pack_line) — the .app.tar.gz would not exist yet, and the release would refuse to publish"
        fi

        # (h) THE SILENT FALLBACK MUST NOT EXIST.
        #
        #     `resolve_platform_names(available=…)` used to return the pre-fix bare
        #     Mach-O when a release carried no `.app.tar.gz`, log a WARNING, and
        #     carry on. It therefore PUBLISHED a live `update_config` row pointing a
        #     macOS slot at a payload no macOS client can install — which is exactly
        #     how v3.2.26 reached the live hub, and why a student on 3.2.24 was
        #     offered an update that failed with "not an installer (48150583 bytes)".
        #
        #     Sections §24(a)–(g) all check that the *declarations* agree. Not one
        #     of them could see this, because the declarations were correct — the
        #     defect was a runtime branch that DOWNGRADED a correct declaration into
        #     a broken artifact. So this asserts the branch is a REFUSAL, not a
        #     fallback: inside the macOS arm of `resolve_platform_names`, the legacy
        #     name may only appear in a `raise FetchError(…)`, never as an assignment
        #     to `resolved`.
        #
        #     Scoped to the function body and to the assignment shape on purpose:
        #     `LEGACY_MACOS_NAMES` is a legitimate map and its *name* appears in
        #     comments and the allow-list, so a whole-file grep (or a grep for the
        #     identifier alone) stays green with the fallback restored. This checks
        #     the thing that does the damage — `resolved = legacy`.
        if [ -f "$FETCH" ]; then
            fetch_body=$(sed -n '/^def resolve_platform_names/,/^def /p' "$FETCH")
            if printf '%s\n' "$fetch_body" | grep -Eq '^[[:space:]]*resolved[[:space:]]*=[[:space:]]*legacy'; then
                bad "fetch-release.py silently downgrades a macOS slot to the bare Mach-O (resolved = legacy) — a release with no .app.tar.gz would publish an uninstallable row instead of refusing"
            else
                ok "fetch-release.py refuses a macOS slot with no .app.tar.gz instead of silently serving the bare Mach-O"
            fi
            #     The refusal must be a FetchError that says WHY, or the operator is
            #     back to "no updates, and nothing says why".
            if printf '%s\n' "$fetch_body" | grep -q 'raise FetchError('; then
                ok "fetch-release.py's macOS refusal raises FetchError (visible at publish time)"
            else
                bad "fetch-release.py's macOS slot-refusal path does not raise FetchError — the operator would see a crash, not a reason"
            fi
        fi

        # (i) ONE ANSWER, NOT TWO.
        #
        #     The manifest cross-check must compare against the SAME list the staging
        #     loop fetched — `resolved`, built with `available=set(assets)`. It used
        #     to re-call `resolve_platform_names(version)` with NO `available=`,
        #     which answers "what does the hub *prefer*"; staging asked "what will
        #     the hub *fetch*". The two agreed only while every release carried every
        #     preferred name, so the check could pass while a different file was
        #     served — the "two sources of truth with no reconciliation" shape this
        #     whole script exists to catch.
        #
        #     The invariant is narrow and stated as AGREEMENT, not shape: the loop
        #     that builds `mismatches` iterates the list staging produced. Not a
        #     "zero calls" rule — `verify_and_report` and the signature-fold resolve
        #     the *preferred* names on purpose, to look up platform KEYS (which are
        #     version-independent) and to read the staged filename back out of
        #     `results`. Asserting "no second call" would fail intent-correct code,
        #     which is how a guard gets bypassed (§24's own recurring lesson).
        #
        #     Scoped to the block between the staged-list binding and the signature
        #     fold: that is the cross-check, and `mismatches.append` is inside it.
        if [ -f "$FETCH" ]; then
            if grep -q 'resolved = resolve_platform_names(version, available=set(assets))' "$FETCH"; then
                ok "fetch-release.py binds the staged platform list from the release's own assets"
            else
                bad "fetch-release.py no longer binds \`resolved\` from available=set(assets) — staging and the cross-check can disagree again"
            fi
            # The cross-check iterating anything other than the staged `resolved`
            # list is the bug. Anchored on the loop that feeds `mismatches`.
            xcheck=$(sed -n '/mismatches = \[\]/,/mismatches.append/p' "$FETCH")
            if printf '%s\n' "$xcheck" | grep -Eq 'for key, name, _ in resolved:'; then
                ok "fetch-release.py's manifest cross-check compares against the staged list (what is served is what is checked)"
            else
                bad "fetch-release.py's manifest cross-check does not iterate the staged \`resolved\` list — it re-resolves the names and can pass while a different file is served"
            fi
        fi

        # (j) THE BEHAVIOUR, not the text.
        #
        #     (h) and (i) above are greps — fast, but they can only prove the code
        #     LOOKS right. The property that matters is behavioural: a pre-fix
        #     release is REFUSED, not served. `smoke-macos-payload-resolution.sh`
        #     imports the shipped `resolve_platform_names` and asserts exactly that,
        #     so a rewrite that keeps the identifiers but restores the fallback
        #     fails here even if every grep above is satisfied.
        #
        #     Run from the fast gate so it is enforced, not merely available: the
        #     other smoke scripts are documented manual tools and nothing in CI ran
        #     them, so a rule they asserted could be broken with every check green.
        #     This one is wired in. It is offline and deterministic (it imports the
        #     module and calls one function), so it costs a Python start-up.
        SMOKE_MACOS="$SCRIPTS/smoke-macos-payload-resolution.sh"
        if [ -f "$SMOKE_MACOS" ]; then
            if smoke_out=$(bash "$SMOKE_MACOS" 2>&1); then
                ok "the macOS payload resolution refuses a pre-fix release and serves the tarball otherwise (behavioural)"
            else
                bad "smoke-macos-payload-resolution.sh FAILED — a macOS slot could be served a payload no client can install:"
                printf '%s\n' "$smoke_out" | sed 's/^/        /' >&2
            fi
        else
            warn "no smoke-macos-payload-resolution.sh — the behavioural half of this contract is unenforced"
        fi
    else
        warn "no install.rs — cannot check the client side of the macOS payload contract"
    fi
}
