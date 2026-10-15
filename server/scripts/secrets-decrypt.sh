#!/usr/bin/env bash
# secrets-decrypt.sh — decrypt server/secrets.env.age
#
# The `age -d` invocation used to require `-i age-key.txt`, a gitignored key file
# that was lost and only recovered because a copy happened to still be on the
# deployed host. This reads the PASSPHRASE instead, so a fresh clone plus the
# passphrase is all that is needed.
#
# ⚠️  WHY THERE IS A PTY IN HERE
#
# age has NO non-interactive passphrase input — verified against age 1.3.2 on
# 2026-10-16, both by reading the binary and by test: no AGE_PASSPHRASE variable,
# no stdin/fd input, /dev/tty opened directly, and `-i` explicitly rejected for
# passphrase files. `script -qec` allocates the pty that age demands. See
# secrets-encrypt.sh for the full explanation.
#
# Usage:
#   ./secrets-decrypt.sh                          # to stdout
#   ./secrets-decrypt.sh > ../.secrets.env        # to a file you then shred
#   ./secrets-decrypt.sh --check                  # verify it decrypts; print nothing
#   LOCUS_SECRETS_PASSPHRASE_FILE=/tmp/pw ./secrets-decrypt.sh --check
#
# Passphrase sources, in order of precedence (identical to secrets-encrypt.sh and
# to the deploy path in setup.sh / restore.sh):
#   1. LOCUS_SECRETS_PASSPHRASE       — the passphrase itself
#   2. LOCUS_SECRETS_PASSPHRASE_FILE  — a file containing it
#   3. an interactive prompt (age's own), if neither is set
#
# The passphrase is never echoed and never passed as an argument.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SECRETS_FILE="${SERVER_DIR}/secrets.env.age"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[ok]${NC} $*" >&2; }
fail() { echo -e "${RED}[fail]${NC} $*" >&2; exit 1; }

CHECK_ONLY=""
case "${1:-}" in
    --check) CHECK_ONLY=1 ;;
    "")      ;;
    *)       fail "unknown argument: $1 (only --check is accepted)" ;;
esac

command -v age >/dev/null 2>&1 || fail "age is not installed. Install it first (apt-get install age, or https://age-encryption.org)."
[ -f "$SECRETS_FILE" ] || fail "no ciphertext at ${SECRETS_FILE}"

# ── Resolve passphrase source ──
if [ -n "${LOCUS_SECRETS_PASSPHRASE:-}" ]; then
    PASS_MODE=env
elif [ -n "${LOCUS_SECRETS_PASSPHRASE_FILE:-}" ]; then
    [ -f "$LOCUS_SECRETS_PASSPHRASE_FILE" ] || fail "LOCUS_SECRETS_PASSPHRASE_FILE is not a file: $LOCUS_SECRETS_PASSPHRASE_FILE"
    PASS_MODE=file
else
    PASS_MODE=prompt
fi

# ── Decrypt to a temp file, then emit ──
# Never decrypt straight onto stdout for a caller to redirect: `age` writes
# incrementally, so a failure partway through would leave a truncated
# .secrets.env behind that a later deploy would happily source. Decrypt fully,
# then emit.
TMP_OUT="$(mktemp)"
trap 'rm -f "$TMP_OUT"' EXIT

if [ "$PASS_MODE" = prompt ]; then
    age -d -o "$TMP_OUT" "$SECRETS_FILE" || fail "decryption failed.
  The passphrase is wrong, or ${SECRETS_FILE##*/} is corrupt."
else
    if [ "$PASS_MODE" = env ]; then
        _feed() { printf '%s\n' "$LOCUS_SECRETS_PASSPHRASE"; }
    else
        _pw="$(cat "$LOCUS_SECRETS_PASSPHRASE_FILE")"
        _feed() { printf '%s\n' "$_pw"; }
    fi

    if ! _feed | script -qec "age -d -o '$TMP_OUT' '$SECRETS_FILE'" /dev/null >/dev/null 2>&1; then
        fail "decryption failed.
  Most likely the passphrase is wrong. Nothing was written."
    fi
fi

# ── Sanity: is this actually an env file? ──
# A wrong passphrase is caught above, but an EMPTY or unrelated file decrypts
# fine and then fails much later inside setup.sh with a confusing message.
if ! grep -qE '^[A-Z_]+=' "$TMP_OUT"; then
    fail "decrypted successfully but the result has no KEY=VALUE lines.
  ${SECRETS_FILE##*/} may have been encrypted from the wrong file."
fi

if [ -n "$CHECK_ONLY" ]; then
    n=$(grep -cE '^[A-Z_]+=' "$TMP_OUT")
    log "OK — decrypted ${SECRETS_FILE##*/} (${n} variables, contents not printed)"
    exit 0
fi

cat "$TMP_OUT"
