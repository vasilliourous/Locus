#!/usr/bin/env bash
# Section 5 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_5() {
    # ─────────────────────────────────────────────────────────────
    # 5. Paths that cannot be checked out on Windows.
    #
    # A tracked filename containing ':' makes `actions/checkout` refuse the WHOLE
    # checkout, and only on the Windows runner — so the branch looks green
    # everywhere else. This has happened once (62 Zone.Identifier files).
    # ─────────────────────────────────────────────────────────────
    echo
    echo "5. Windows-illegal tracked paths"

    colon_paths=$(cd "$REPO" && git ls-files 2>/dev/null | grep ':' || true)
    if [ -z "$colon_paths" ]; then
        ok "no tracked path contains ':'"
    else
        bad "tracked path(s) contain ':' — Windows checkout will fail entirely:"
        printf '%s\n' "$colon_paths" | head -10 | sed 's/^/         /'
    fi

    # ── 5b. The signing-key directory carries a README and nothing else ──
    #
    # WHY. `.chrysalis-keys/` holds the minisign private key that every installed client
    # verifies updates against, so the directory is gitignored — which also means a
    # clone gets an EMPTY directory, with nothing in it to say what it is for or why
    # deleting its contents is unrecoverable. The README is carved back out of that
    # ignore so a clone can read the warning.
    #
    # An earlier comment in `.gitignore` CITED a check here that did not exist. That
    # is the defect FIXES.md 2026-10-01 records ("a guard cited in six documents did
    # not exist"), so it is written rather than re-described: this asserts the carve
    # is effective (the README is trackable) and that nothing else in the directory
    # can ever be staged (no other file is trackable). Both directions, because a
    # carve that silently stopped working would look exactly like one that works.
    chrysalis_key_tracked=$(cd "$REPO" && git ls-files .chrysalis-keys/ 2>/dev/null)
    if [ "$chrysalis_key_tracked" = ".chrysalis-keys/README.md" ]; then
        ok ".chrysalis-keys/ carries exactly its README, and no key is trackable"
    else
        bad ".chrysalis-keys/ tracked contents are '${chrysalis_key_tracked:-nothing}'; expected exactly '.chrysalis-keys/README.md'"
    fi
    # The negative half: a key file placed there must still be ignored.
    if [ -f "$REPO/.chrysalis-keys/README.md" ] && (cd "$REPO" && git check-ignore -q .chrysalis-keys/chrysalis_update.key 2>/dev/null); then
        ok "a private key in .chrysalis-keys/ remains ignored"
    else
        bad "a private key in .chrysalis-keys/ is NOT ignored — check the .gitignore carve"
    fi

    # ── 5c. The extractable key never becomes a dependency, and never a backup ──
    #
    # WHY. `.chrysalis-keys/` is DELIBERATELY EMPTY after a clone: the signing key is a
    # build input extracted from `server/secrets.env.age` per session (see
    # .chrysalis-keys/README.md and OPERATE/SECRETS-MANAGEMENT.md). The 2026-10-16 loss
    # happened because that directory was treated as the key's home rather than as a
    # cache, so when the working tree went the key went with it. Two things keep that
    # from recurring, and neither is a sentence in a document:
    #
    #   1. No tracked file may READ the key as though it were always present. A build
    #      that assumes `.chrysalis-keys/chrysalis_update.key` exists fails on a fresh clone,
    #      which is precisely the state that must be the normal one. The CI signing
    #      step writes the key to a runner-temp path from the Actions secret instead,
    #      so the working copy is never on the critical path.
    #   2. The rebuild recipe must exist and say where the key comes from, because a
    #      cache that cannot be refilled is a backup that has already failed.
    #
    # This does NOT decrypt the bundle — CI has no passphrase, and a guard that
    # needed one would be skipped on the only machine that runs it. What it asserts
    # is recomputable from the checkout: the README is present and names the bundle,
    # and the key directory carries no tracked file other than it (5b, above).
    if [ -f "$REPO/.chrysalis-keys/README.md" ]; then
        # BOTH facts are required, and they are checked as whole tokens rather than
        # as one substring. A first version of this check only looked for
        # `secrets.env.age` anywhere in the file — so mutating the prose but leaving
        # the markdown link's href intact PASSED, which is a guard reporting on a
        # document that no longer said the thing. Verified by breaking it.
        readme_ok=1
        grep -qF '`server/secrets.env.age`' "$REPO/.chrysalis-keys/README.md" 2>/dev/null || readme_ok=0
        grep -qF 'chrysalis-key-extract.sh' "$REPO/.chrysalis-keys/README.md" 2>/dev/null || readme_ok=0
        if [ "$readme_ok" = 1 ]; then
            ok ".chrysalis-keys/README.md names the bundle AND the extraction script by exact path"
        else
            bad ".chrysalis-keys/README.md must name BOTH 'server/secrets.env.age' (as code) and 'chrysalis-key-extract.sh' — it is the only text a clone sees before the directory fills"
        fi
    else
        bad ".chrysalis-keys/README.md is missing — a clone gets an empty directory that explains nothing"
    fi
    # The extraction script must exist and be executable: the README's recipe is a
    # citation, and a cited command that is not there is the defect 5b records.
    if [ -x "$REPO/server/scripts/chrysalis-key-extract.sh" ]; then
        ok "server/scripts/chrysalis-key-extract.sh exists and is executable"
    else
        bad "server/scripts/chrysalis-key-extract.sh is missing or not executable — the README cites it"
    fi
    # The plaintext bundle must NOT be tracked: it is the pre-encryption copy, and
    # committing it would put every credential in git in the clear.
    if (cd "$REPO" && git ls-files --error-unmatch .secrets.env >/dev/null 2>&1); then
        bad ".secrets.env is TRACKED — the plaintext bundle must never be in git; only server/secrets.env.age"
    else
        ok ".secrets.env (plaintext bundle) is not tracked"
    fi

    # ── 5d. A fresh clone can build, and the page that says how is accurate ──
    #
    # WHY. The goal is that deleting this directory and re-cloning loses nothing: the
    # passphrase is the only thing a clone cannot reproduce. Two build steps break
    # that silently, because `cargo` does not run them and its failure message names
    # neither:
    #
    #   * `pnpm run prebuild` stages `src-tauri/sidecar/` from vendor/. Without it the
    #     Rust build script dies on "resource path 'sidecar/clash-verge-service-…'
    #     doesn't exist".
    #   * `pnpm web:build` produces `../dist`, which `frontendDist` names. Without it
    #     the tauri macro panics on a missing path — even for `cargo test`.
    #
    # Both were hit on a real clone while writing docs/operate/SESSION.md. Documenting
    # them is not enough on its own: a reader who skips the page gets the raw cargo
    # error, and a reader whose page has drifted gets advice that no longer applies.
    # So this asserts the two prerequisites are NAMED in SESSION.md, and re-derives
    # the frontendDist claim from tauri.conf.json rather than restating it.
    SESSION_DOC="$REPO/docs/operate/SESSION.md"
    if [ -f "$SESSION_DOC" ]; then
        # The commands, as a reader would type them. Both are required.
        missing=""
        grep -q 'prebuild' "$SESSION_DOC" || missing="$missing prebuild"
        grep -q 'web:build' "$SESSION_DOC" || missing="$missing web:build"
        if [ -z "$missing" ]; then
            ok "docs/operate/SESSION.md names both build prerequisites (prebuild, web:build)"
        else
            bad "docs/operate/SESSION.md does not name:${missing} — a fresh clone fails at cargo with an error naming neither"
        fi
        # And that the prerequisite ORDER is stated, because running cargo first is
        # the mistake: the page must come before the gate list, not after it.
        prebuild_line=$(grep -n 'prebuild' "$SESSION_DOC" | head -1 | cut -d: -f1)
        cargo_line=$(grep -n 'cargo test --manifest-path' "$SESSION_DOC" | head -1 | cut -d: -f1)
        if [ -n "$prebuild_line" ] && [ -n "$cargo_line" ] && [ "$prebuild_line" -lt "$cargo_line" ]; then
            ok "SESSION.md introduces prebuild before the cargo gate it unblocks"
        else
            bad "SESSION.md lists 'cargo test' before 'prebuild' — the order a reader follows is the order it fails in"
        fi
    else
        bad "docs/operate/SESSION.md is missing — nothing tells a fresh clone what to run"
    fi
    # The frontendDist path the doc describes must be the one the config declares.
    # A hardcoded "../dist" in prose would drift the moment the build moves; this
    # reads the config and fails if the two ever disagree.
    declared_dist=$(python3 - "$REPO/client/src-tauri/tauri.conf.json" <<'PYEOF'
import json, sys
try:
    conf = json.load(open(sys.argv[1]))
    print(conf["build"]["frontendDist"].lstrip("./"))
except Exception:
    print("")
PYEOF
    )
    if [ -n "$declared_dist" ] && grep -qF "$declared_dist" "$SESSION_DOC" 2>/dev/null; then
        ok "SESSION.md's frontendDist claim ($declared_dist) matches tauri.conf.json"
    elif [ -z "$declared_dist" ]; then
        bad "could not read build.frontendDist from tauri.conf.json"
    else
        bad "SESSION.md does not mention '$declared_dist', the frontendDist tauri.conf.json declares — the page is describing a build that no longer exists"
    fi

    # ── 5e. The extraction route works from a clone, not just from this machine ──
    #
    # WHY. The whole point of the bundle model is that a fresh clone plus the
    # passphrase is sufficient. That claim dies quietly if the extraction path depends
    # on a file OUTSIDE the repository — a `~/.chrysalis-secrets/` copy, a hardcoded home
    # directory, an absolute path from the machine it was written on. Such a script
    # works perfectly for its author and fails for everyone else, which is the worst
    # kind of working.
    #
    # So: the script must resolve everything relative to its own location, and must
    # name nothing outside the checkout. HOME-relative paths are the specific trap —
    # they are how "it works on my machine" gets committed.
    EXTRACT="$REPO/server/scripts/chrysalis-key-extract.sh"
    if [ -f "$EXTRACT" ]; then
        # Absolute home paths, `~/`, and `$HOME` would all mean the route depends on
        # this machine. `$HOME` was MISSED by the first version of this check, which
        # only looked for `~/` — so appending `KEY_BAK="$HOME/..."` passed. A guard
        # that cannot see the most common form of the thing it looks for is worse than
        # none, because it reports OK.
        machine_paths=$(grep -nE '(\$HOME|\$\{HOME\}|~\/|/home/[a-z]|/Users/[A-Za-z])' "$EXTRACT" 2>/dev/null \
                        | grep -vE '^[0-9]+:\s*#' || true)
        if [ -z "$machine_paths" ]; then
            ok "chrysalis-key-extract.sh names no machine-local path (portable to a clone)"
        else
            bad "chrysalis-key-extract.sh references a machine-local path — it will work for its author and fail for a clone:"
            printf '%s\n' "$machine_paths" | head -5 | sed 's/^/         /'
        fi
        # It must resolve the repo from its own location, not from $PWD: a reader may
        # run it as `./server/scripts/...` from the root or from anywhere else.
        if grep -q 'BASH_SOURCE' "$EXTRACT"; then
            ok "chrysalis-key-extract.sh resolves the repo from its own path, not the caller's cwd"
        else
            bad "chrysalis-key-extract.sh does not use BASH_SOURCE — it will resolve paths against the caller's cwd"
        fi
        # And it must not print the key. A script that extracts a secret and echoes it
        # puts the key in shell history, scrollback and CI logs.
        if grep -qE 'echo[^|]*"\$(key_b64|TMP_KEY)"' "$EXTRACT" 2>/dev/null; then
            bad "chrysalis-key-extract.sh echoes the key material"
        else
            ok "chrysalis-key-extract.sh never echoes the key material"
        fi
    else
        bad "server/scripts/chrysalis-key-extract.sh is missing"
    fi
}
