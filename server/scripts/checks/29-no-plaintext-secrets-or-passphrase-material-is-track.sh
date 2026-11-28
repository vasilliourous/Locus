#!/usr/bin/env bash
# Section 29 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_29() {
    echo "29. No plaintext secrets or passphrase material is tracked"

    # ── WHY THIS GUARD EXISTS ──
    #
    # On 2026-10-16 the project moved from an age KEYPAIR to a PASSPHRASE for
    # `server/secrets.env.age`. That makes the ciphertext committable (it is), and it
    # makes ONE file catastrophic: the passphrase. A leaked ciphertext is an
    # offline-attack problem with a long passphrase; a leaked ciphertext PLUS its
    # passphrase is total compromise of every credential in the project, with no
    # brute force needed.
    #
    # During that migration `encryption-password.txt` sat at the repo root, untracked
    # and unignored. That is the THIRD time this repository has held a live credential
    # one `git add -A` from publication, after `vpspassword.txt` (2026-10-02) and
    # `temp-ghp-token` (2026-10-15). Three incidents of one shape is a missing check,
    # not three moments of carelessness — which is what this section is.
    #
    # ── WHAT THIS CAN AND CANNOT DO ──
    #
    # `.gitignore` is a convention, not a control: it stops `git add -A` and nothing
    # else, and `git add -f` walks straight past it. This section is the second,
    # independent layer: it asks git what is ACTUALLY TRACKED — the index, not the
    # ignore rules — so a file that got in by `-f`, by an old commit, or by an ignore
    # pattern that a later edit removed, is still caught.
    #
    # It is deliberately a NAME/EXTENSION check and not a content scan. A content
    # scan for "looks like a secret" is a heuristic that fires on fixtures and misses
    # real keys; it would be turned off within a week. This checks the file classes we
    # have actually lost material to.
    #
    # ── VERIFIED TO FAIL ──
    #
    # Per docs/operate/CLAIMS.md and the rule "every new guard must be shown to fail":
    # this section was run against a planted `encryption-password.txt` staged with
    # `git add -f` and it reported BAD and exited non-zero. A guard that cannot fail
    # reads exactly like one that passed.
    # The tracked-file list is passed as an ARGUMENT, not on stdin.
    #
    # The first draft was `git ls-files | python3 - <<'PY'`, which reads as obviously
    # correct and is not: `python3 -` takes its SCRIPT from the heredoc, so stdin is
    # already consumed by the program text and the piped file list is discarded
    # entirely. Python then iterates an empty stdin and reports zero findings — a
    # guard that always passes, in a section whose whole purpose is to fail loudly.
    # It was caught only by planting a passphrase file and confirming the check went
    # red; the harness reported OK while `git add -f`ed secrets sat in the index.
    #
    # `sys.argv` cannot be clobbered by the heredoc, so the list travels there.
    TRACKED_BAD=$(
        cd "$REPO" && python3 - "$(git ls-files)" <<'PY'
import re, sys

tracked = sys.argv[1].split('\n') if len(sys.argv) > 1 else []

# Patterns are anchored to a basename or a path segment so they cannot fire on a
# legitimate file that merely mentions the word. Each entry records the incident
# it comes from; none of these are hypothetical.
RULES = [
    # (regex on the tracked path, what it is, when it burned us)
    (r'(^|/)\.secrets\.env$',
     'plaintext secrets file',
     'the pre-encryption form of secrets.env.age'),
    (r'(^|/)encryption-password[^/]*$',
     'secrets passphrase',
     '2026-10-16 — sat at the repo root during the passphrase migration'),
    (r'(^|/)secrets-passphrase[^/]*$',
     'secrets passphrase',
     'documented name for the CHRYSALIS_SECRETS_PASSPHRASE_FILE convention'),
    (r'(^|/)secrets-pass[^/]*$',
     'secrets passphrase',
     'short form of the above'),
    (r'(^|/)passphrase\.txt$',
     'secrets passphrase',
     'ad-hoc name an operator would plausibly choose'),
    # The two below are the .gitignore:250-258 incident, caught on the ignore
    # side and never followed here. `passphrase.txt` above requires the exact
    # name; an operator who adds a suffix, or who writes the passphrase under
    # any `passphrase*` name, matched nothing. `decryptionkey*` is the file that
    # prompted the `.gitignore` rule: it sat at the repo ROOT, untracked and
    # UNIGNORED, through the whole school-network fix.
    (r'(^|/)decryption-?key[^/]*$',
     'secrets passphrase',
     '.gitignore:258-266 — sat at the repo root, untracked and unignored'),
    (r'(^|/)passphrase[^/]*$',
     'secrets passphrase',
     '.gitignore:255 — any `passphrase*` name, not only `passphrase.txt`'),
    (r'(^|/)vpspassword\.txt$',
     'VPS root credential',
     '2026-10-02 — one command short of committing a live credential'),
    (r'(^|/)temp-ghp-token[^/]*$',
     'GitHub personal access token',
     '2026-10-15 — admin:org + delete_repo scopes, untracked and unignored'),
    # The age keypair is retired, but an OLD checkout may still have one tracked,
    # and it is a private key either way.
    (r'(^|/)age-key\.txt$',
     'age private key',
     'the retired keypair; superseded 2026-10-16 but still a secret if present'),
    # Backups written by secrets-encrypt.sh. Not secret — but a stale ciphertext
    # that no longer matches the committed one is a trap for the next reader.
    (r'(^|/)secrets\.env\.age\.bak\.',
     'stale ciphertext backup',
     'written by secrets-encrypt.sh before it overwrites the live ciphertext'),
]

found = False
for path in tracked:
    path = path.strip()
    if not path:
        continue
    for pattern, what, why in RULES:
        if re.search(pattern, path):
            print(f"BAD\t{path}\t{what}\t{why}")
            found = True
            break
PY
    )

    if [ -z "$TRACKED_BAD" ]; then
        ok "no plaintext secrets, passphrases, tokens or stale ciphertext are tracked"
    else
        while IFS="$(printf '\t')" read -r _kind path what why; do
            bad "TRACKED SECRET: $path ($what)"
            bad "  Origin: $why"
            bad "  Remove it from the index:  git rm --cached '$path'"
        done <<EOF
$TRACKED_BAD
EOF
        bad "  A tracked secret is in the HISTORY, not just the working tree. If this"
        bad "  ever contained a live credential, rotate that credential — removing the"
        bad "  file does not un-publish it, and rewriting history does not either."
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
