#!/usr/bin/env bash
# secrets-encrypt.sh — encrypt a plaintext env file into server/secrets.env.age
#
# WHY THIS EXISTS
#
# The secrets file used to be encrypted to an age *keypair*, whose private half
# lived in `age-key.txt` — a gitignored file. That key was lost with the rest of
# the gitignored working tree, and although it was later recovered from the hub,
# the recovery route depended on a copy still sitting on the deployed host. A
# secret that can only be recovered from a box you might delete is not a backup.
#
# This script encrypts to a PASSPHRASE instead (`age -p`). The ciphertext is
# committed; the passphrase is not, and never appears in the repo. So a clone
# plus the passphrase is enough to deploy.
#
# ⚠️  WHY THERE IS A PTY IN HERE
#
# age has NO non-interactive passphrase input. Verified against age 1.3.2 on
# 2026-10-16, by reading the binary and by test:
#
#   * there is no AGE_PASSPHRASE (or similar) environment variable;
#   * it does not read the passphrase from stdin or from an arbitrary fd
#     (`3< file` still hangs waiting for a terminal);
#   * it opens /dev/tty directly, and fails with
#     "standard input is not a terminal, and /dev/tty is not available: ..."
#   * `-i/--identity` is explicitly rejected for passphrase files:
#     "remove all -i/--identity/-j flags to decrypt passphrase-encrypted files".
#
# So the passphrase must be delivered through a terminal. `script -qec` allocates
# a pty, which satisfies age without putting the passphrase in argv. That is the
# mechanism below, and the reason this script is more than a one-line wrapper.
#
# Usage:
#   ./secrets-encrypt.sh                       # encrypt ../.secrets.env
#   ./secrets-encrypt.sh path/to/plaintext.env
#   LOCUS_SECRETS_PASSPHRASE_FILE=/tmp/pw ./secrets-encrypt.sh
#   LOCUS_SECRETS_PASSPHRASE='...'        ./secrets-encrypt.sh
#
# Passphrase sources, in order of precedence (same contract as setup.sh):
#   1. LOCUS_SECRETS_PASSPHRASE       — the passphrase itself
#   2. LOCUS_SECRETS_PASSPHRASE_FILE  — a file containing it
#   3. an interactive prompt (age's own), if neither is set and a tty is present
#
# The passphrase is never echoed, never logged, and never passed as an argument.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT_FILE="${SERVER_DIR}/secrets.env.age"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[ok]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }
fail() { echo -e "${RED}[fail]${NC} $*" >&2; exit 1; }

command -v age >/dev/null 2>&1 || fail "age is not installed. Install it first (apt-get install age, or https://age-encryption.org)."

# ── Which plaintext are we encrypting? ──
IN_FILE="${1:-}"
if [ -z "$IN_FILE" ]; then
    if [ -f "${SERVER_DIR}/../.secrets.env" ]; then
        IN_FILE="${SERVER_DIR}/../.secrets.env"
    else
        fail "no plaintext to encrypt.
  Pass one explicitly:    $0 path/to/plaintext.env
  Or produce it first:    ./secrets-decrypt.sh > ../.secrets.env
  See docs/operate/SECRETS-MANAGEMENT.md for the required variables."
    fi
fi
[ -f "$IN_FILE" ] || fail "plaintext not found: $IN_FILE"

# ── Refuse to encrypt something that is not a secrets file ──
# A missing variable here is invisible until a deploy half-fails on a blank box.
# This is a form check, not a security one: it cannot verify the VALUES are right.
missing=""
for var in DOMAIN ADMIN_API_TOKEN; do
    grep -qE "^${var}=" "$IN_FILE" || missing="${missing} ${var}"
done
if [ -n "$missing" ]; then
    fail "plaintext is missing required variable(s):${missing}
  Refusing to encrypt an incomplete secrets file — see
  docs/operate/SECRETS-MANAGEMENT.md for the full variable list."
fi

# ── Resolve the passphrase source, but do NOT read the value yet ──
# Only the *source* is chosen here; the value is streamed into the pty below.
if [ -n "${LOCUS_SECRETS_PASSPHRASE:-}" ]; then
    PASS_MODE=env;  PASS_SRC="LOCUS_SECRETS_PASSPHRASE (environment)"
elif [ -n "${LOCUS_SECRETS_PASSPHRASE_FILE:-}" ]; then
    [ -f "$LOCUS_SECRETS_PASSPHRASE_FILE" ] || fail "LOCUS_SECRETS_PASSPHRASE_FILE is not a file: $LOCUS_SECRETS_PASSPHRASE_FILE"
    PASS_MODE=file; PASS_SRC="LOCUS_SECRETS_PASSPHRASE_FILE ($LOCUS_SECRETS_PASSPHRASE_FILE)"
else
    PASS_MODE=prompt; PASS_SRC="interactive prompt"
fi

# ── Refuse to overwrite real ciphertext without a backup ──
if [ -f "$OUT_FILE" ]; then
    backup="${OUT_FILE}.bak.$(date +%Y%m%d-%H%M%S)"
    cp -p "$OUT_FILE" "$backup"
    warn "existing ciphertext backed up to ${backup}"
fi

log "encrypting ${IN_FILE} -> ${OUT_FILE}"
log "passphrase source: ${PASS_SRC}"

# ── Encrypt ──
# `age -p` prompts twice when interactive ("Enter passphrase" / "Confirm
# passphrase"), so a scripted source must supply the value TWICE. Feeding it once
# leaves age waiting on the confirmation prompt forever — which is exactly how
# the first version of this script hung.
#
# Cleanup: the pty transcript is written to /dev/null so the passphrase never
# reaches a file on disk, and the stdout of `script` is discarded for the same
# reason (a pty echoes what it reads).
if [ "$PASS_MODE" = prompt ]; then
    age -p -o "$OUT_FILE" "$IN_FILE"
else
    # A temporary copy is NOT used; the value is piped straight in.
    if [ "$PASS_MODE" = env ]; then
        _feed() { printf '%s\n%s\n' "$LOCUS_SECRETS_PASSPHRASE" "$LOCUS_SECRETS_PASSPHRASE"; }
    else
        _pw="$(cat "$LOCUS_SECRETS_PASSPHRASE_FILE")"
        _feed() { printf '%s\n%s\n' "$_pw" "$_pw"; }
    fi

    if ! _feed | script -qec "age -p -o '$OUT_FILE' '$IN_FILE'" /dev/null >/dev/null 2>&1; then
        fail "age failed to encrypt.
  The usual cause is an empty passphrase source. Nothing was written."
    fi
fi

[ -s "$OUT_FILE" ] || fail "encryption produced an empty file — not overwriting anything further."

# ── Prove the ciphertext decrypts before telling the operator it worked ──
# "age exited 0" is not evidence the file is usable. Decrypt it back and compare
# with the input, byte for byte. Same discipline as a guard that must be shown to
# fail: an encrypt step that cannot be shown to round-trip is not verified.
# Only possible when a scripted passphrase source exists; an interactive
# passphrase is not re-read here, so say so rather than implying verification.
if [ "$PASS_MODE" = prompt ]; then
    warn "round-trip NOT verified (interactive passphrase is not re-read)."
    warn "verify manually:  ./secrets-decrypt.sh --check"
else
    _RT="$(mktemp)"
    trap 'rm -f "$_RT"' EXIT
    if [ "$PASS_MODE" = env ]; then
        _feed1() { printf '%s\n' "$LOCUS_SECRETS_PASSPHRASE"; }
    else
        _feed1() { printf '%s\n' "$_pw"; }
    fi
    # `age -d` prompts ONCE, unlike encryption's twice.
    if ! _feed1 | script -qec "age -d -o '$_RT' '$OUT_FILE'" /dev/null >/dev/null 2>&1; then
        fail "round-trip FAILED: the new ciphertext does not decrypt with that passphrase.
  ${OUT_FILE} was written but is suspect. Restore from the .bak if one was made."
    fi
    if ! cmp -s "$IN_FILE" "$_RT"; then
        fail "round-trip MISMATCH: decrypted bytes differ from the input.
  Do not commit. Investigate before retrying."
    fi
    log "round-trip verified (decrypts to ${IN_FILE##*/}, byte-identical)"
fi

size_out=$(wc -c < "$OUT_FILE")
log "wrote ${OUT_FILE} (${size_out} bytes)"
echo
echo "Next:"
echo "  1. shred -u ${IN_FILE}      # once you are satisfied; plaintext must not linger"
echo "  2. git add server/secrets.env.age"
echo "  3. git status               # confirm the passphrase is NOT among the changes"
echo
warn "Treat the passphrase as the only thing protecting this ciphertext, and make"
warn "it long. See docs/operate/SECRETS-MANAGEMENT.md for the threat model."
