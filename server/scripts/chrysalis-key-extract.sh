#!/usr/bin/env bash
# chrysalis-key-extract.sh — rehydrate .chrysalis-keys/ from the encrypted bundle
#
# WHY THIS IS A SCRIPT AND NOT AN INSTRUCTION
#
# `.chrysalis-keys/chrysalis_update.key` is the minisign PRIVATE key that signs every
# client update. Every installed client trusts exactly one public key — the one
# compiled into it — and the Tauri updater has no bypass. Lose the private key
# and no already-installed client can ever be updated again.
#
# It USED to live only in `.chrysalis-keys/`, a gitignored directory. On 2026-10-16
# the working tree was lost and the key went with it; it survived only because an
# unencrypted copy happened to still be on the deployed host, and the fleet still
# paid a manual reinstall. A secret recoverable only from a box you might delete
# is not a backup.
#
# So the directory is now a CACHE, never a home. The durable copy is
# `LOCUS_UPDATE_KEY_B64` inside `server/secrets.env.age`, which is committed. This
# script is the only supported way to fill the cache, which means:
#
#   * a fresh clone needs a checkout plus the passphrase, and nothing else;
#   * deleting `.chrysalis-keys/*` is always safe — that is the normal state;
#   * there is exactly one route to the key, so it cannot silently fork.
#
# USAGE
#   ./chrysalis-key-extract.sh              # write .chrysalis-keys/, verifying by default
#   ./chrysalis-key-extract.sh --check      # verify the bundle's key, write nothing
#   ./chrysalis-key-extract.sh --remove     # delete the cache after a signing session
#
# Passphrase sources are inherited from secrets-decrypt.sh and are the same three,
# in the same precedence — do not invent a fourth:
#   1. CHRYSALIS_SECRETS_PASSPHRASE       — the passphrase itself
#   2. CHRYSALIS_SECRETS_PASSPHRASE_FILE  — a file containing it
#   3. an interactive prompt
#
# DEPRECATED ALIAS: the pre-rename `LOCUS_SECRETS_PASSPHRASE` and
# `LOCUS_SECRETS_PASSPHRASE_FILE` are still accepted by the scripts this inherits
# from, and `LOCUS_UPDATE_KEY_B64` in the bundle is still spelled with the old
# product word. Both are deliberate; see
# docs/reference/NAME-MIGRATION.md.
#
# WHAT IS VERIFIED, AND WHY EACH HALF MATTERS
#
#   1. The extracted key is a well-formed minisign secret key (its first line is
#      the `untrusted comment:` header), so a truncated or wrong decrypt is caught
#      here rather than at signing time in CI, where the message is about a
#      missing secret and says nothing about corruption.
#
#   2. The PUBLIC half derived from it matches `plugins.updater.pubkey` in
#      `tauri.conf.json`. This is the check that actually protects the fleet: a
#      key that signs correctly but is the WRONG key produces updates every
#      installed client refuses, and nothing fails until a student tries to update.
#      Comparing key IDs is what release-gate.sh §5 does for the on-disk file;
#      this does it for the bundle, which nothing checked before.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_DIR="$(cd "${SERVER_DIR}/.." && pwd)"

KEY_DIR="${REPO_DIR}/.chrysalis-keys"
KEY_PRIV="${KEY_DIR}/chrysalis_update.key"
KEY_PUB="${KEY_DIR}/chrysalis_update.pub"
TAURI_CONF="${REPO_DIR}/client/src-tauri/tauri.conf.json"
DECRYPT="${SCRIPT_DIR}/secrets-decrypt.sh"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[ok]${NC} $*" >&2; }
warn() { echo -e "${YELLOW}[warn]${NC} $*" >&2; }
fail() { echo -e "${RED}[fail]${NC} $*" >&2; exit 1; }

MODE=extract
case "${1:-}" in
    --check)  MODE=check ;;
    --remove) MODE=remove ;;
    "")       ;;
    *)        fail "unknown argument: $1 (accepts --check or --remove)" ;;
esac

# ── --remove: the session is over, so the cache goes ──
#
# `shred` where available, because the whole point is that the key does not sit
# on this disk; `rm -f` on filesystems without shred(1) is still better than the
# alternative of leaving it. The public half is not secret and is removed with it
# only because a lone `.pub` with no `.key` invites someone to think the key is
# set up when it is not.
if [ "$MODE" = remove ]; then
    removed=0
    for f in "$KEY_PRIV" "$KEY_PUB"; do
        [ -e "$f" ] || continue
        if command -v shred >/dev/null 2>&1; then
            shred -u "$f"
        else
            rm -f "$f"
        fi
        removed=$((removed + 1))
    done
    if [ "$removed" -eq 0 ]; then
        log "nothing to remove — the key cache is already empty (the normal state)"
    else
        log "removed ${removed} file(s) from .chrysalis-keys/ — re-run this script to sign again"
    fi
    exit 0
fi

[ -x "$DECRYPT" ] || fail "cannot find ${DECRYPT} — the bundle route depends on it"
[ -f "$TAURI_CONF" ] || fail "cannot find ${TAURI_CONF}"

# ── Decrypt once, into a private temp file ──
#
# The full bundle is decrypted because secrets-decrypt.sh's sanity check is what
# distinguishes a wrong passphrase from a corrupt ciphertext; asking for only one
# variable would mean re-implementing age's error handling here.
TMP_ENV="$(mktemp)"
# shellcheck disable=SC2064  # expand TMP_ENV now, deliberately
trap 'rm -f "$TMP_ENV"' EXIT
chmod 600 "$TMP_ENV"

"$DECRYPT" > "$TMP_ENV" 2>/dev/null || fail "could not decrypt server/secrets.env.age.
  Check the passphrase. Nothing was written."

# ── Pull the key out, without ever echoing it ──
key_b64=$(grep -m1 '^CHRYSALIS_UPDATE_KEY_B64=' "$TMP_ENV" | cut -d= -f2- || true)
if [ -z "$key_b64" ]; then
    fail "server/secrets.env.age has no CHRYSALIS_UPDATE_KEY_B64.
  The bundle is the key's only durable copy — restore it before signing.
  See docs/operate/RELEASE-RECOVERY.md §2."
fi

TMP_KEY="$(mktemp)"
chmod 600 "$TMP_KEY"
# shellcheck disable=SC2064
trap 'rm -f "$TMP_ENV" "$TMP_KEY"' EXIT
printf '%s' "$key_b64" | base64 -d > "$TMP_KEY" 2>/dev/null \
    || fail "CHRYSALIS_UPDATE_KEY_B64 is not valid base64 — the bundle entry is corrupt"

# ── Verification 1: is this a minisign secret key at all? ──
#
# A minisign secret key file opens with `untrusted comment:` and carries a second
# comment line minisign writes itself. Checking the marker catches the realistic
# failures — a truncated decrypt, or a bundle entry that was pasted with a
# newline folded into it — which would otherwise surface only at signing time.
head -c 18 "$TMP_KEY" | grep -q 'untrusted comment:' \
    || fail "the extracted bytes are not a minisign secret key (missing 'untrusted comment:').
  CHRYSALIS_UPDATE_KEY_B64 in the bundle may be truncated or wrapped."

# ── Verification 2: does the PUBLIC half match what the client trusts? ──
#
# Read the pubkey out of tauri.conf.json, decode it, and compare the 16-hex key
# ID both sides carry. The ID — not the whole key — is what the updater uses to
# decide "was this signed by the key I trust", so it is the meaningful equality.
#
# The public half is derived with `minisign -G`, which REWRITES the key in place
# when given -s. That is why it runs against a COPY in a temp directory and never
# against the real cache: an earlier form of this check in release-gate.sh had to
# learn the same lesson. If minisign needs a passphrase to do this (a protected
# key), the check reports that it could not verify rather than pretending.
# Hoisted so every branch below can see them: whether the public half could be
# derived, and what the published key ID is. Both stay empty on failure, and the
# checks below distinguish "could not determine" from "determined a mismatch" —
# those are different answers and must not collapse into one message.
WORK=""
derived_id=""
pub_id=""

WORK="$(mktemp -d)"
# shellcheck disable=SC2064  # expand these now, deliberately
trap 'rm -f "$TMP_ENV" "$TMP_KEY"; [ -n "$WORK" ] && rm -rf "$WORK"' EXIT

# The published key ID, read from the base64 blob in tauri.conf.json. The blob is
# a base64-encoded minisign PUBLIC KEY FILE, and the ID is in its comment line.
pub_id=$(python3 -c '
import base64, json, re, sys
try:
    conf = json.load(open(sys.argv[1]))
    blob = base64.b64decode(conf["plugins"]["updater"]["pubkey"]).decode()
except Exception:
    sys.exit(0)
m = re.search(r"key ([0-9A-F]{16})", blob)
if m:
    print(m.group(1))
' "$TAURI_CONF" 2>/dev/null || true)

if command -v minisign >/dev/null 2>&1; then
    # Derive the public half from the secret key. The flag is `-R` (recreate a
    # public key file FROM a secret key), NOT `-G`. `-G` GENERATES a new keypair
    # and would overwrite the file given to `-s` — so a `-G` here would have
    # "verified" a freshly-minted key that no client trusts, which is the exact
    # failure this check exists to catch. release-gate.sh used `-G` for the same
    # purpose; it is wrong there too, but that path falls back to reading the
    # `.pub` sibling first and so rarely reaches it.
    #
    # `-R` rewrites the secret key file in place, which is why it runs against a
    # COPY in a temp directory and never against the real cache.
    cp "$TMP_KEY" "$WORK/secret.key"
    if minisign -R -p "$WORK/derived.pub" -s "$WORK/secret.key" >/dev/null 2>&1; then
        derived_id=$(grep -m1 -oE 'key [0-9A-F]{16}' "$WORK/derived.pub" 2>/dev/null \
                     | grep -oE '[0-9A-F]{16}$' || true)
    fi
fi

if [ -z "$derived_id" ]; then
    # Three ways to get here: minisign absent, the key passphrase-protected, or a
    # derivation bug. All three mean "unverified", which is why the message says
    # so rather than implying the key is fine.
    warn "could not derive the key ID from the bundle's key (minisign missing, or a"
    warn "  passphrase-protected secret key). The key was extracted but is UNVERIFIED"
    warn "  against tauri.conf.json."
elif [ -z "$pub_id" ]; then
    fail "could not read the published pubkey ID from tauri.conf.json.
  An unreadable pubkey means nothing can check the key against the client."
elif [ "$derived_id" = "$pub_id" ]; then
    log "the bundle's key ($derived_id) matches the pubkey compiled into the client"
else
    fail "THE BUNDLE'S KEY IS NOT THE KEY THE CLIENT TRUSTS.
  bundle key ID:    $derived_id
  tauri.conf.json:  $pub_id

  Signing with this would produce updates every installed client refuses, and
  nothing would fail until a student tried to update. Do not sign with it.
  Either the bundle entry is stale (put the right key into CHRYSALIS_UPDATE_KEY_B64
  and re-encrypt) or the pubkey was rotated without re-encrypting.
  See docs/operate/RELEASE-RECOVERY.md §2."
fi

if [ "$MODE" = check ]; then
    log "verified (nothing written; .chrysalis-keys/ unchanged)"
    exit 0
fi

# ── Write the cache ──
mkdir -p "$KEY_DIR"
chmod 700 "$KEY_DIR"
cp "$TMP_KEY" "$KEY_PRIV"
chmod 600 "$KEY_PRIV"

# The public half: prefer the one derived from the key, since it is then certain
# to correspond. Fall back to the published blob so a signing session is never
# left with a `.key` and no `.pub`.
if [ -f "$WORK/derived.pub" ]; then
    cp "$WORK/derived.pub" "$KEY_PUB"
elif [ -n "$pub_id" ]; then
    python3 -c '
import base64, json, sys
conf = json.load(open(sys.argv[1]))
sys.stdout.write(base64.b64decode(conf["plugins"]["updater"]["pubkey"]).decode())
' "$TAURI_CONF" > "$KEY_PUB" 2>/dev/null || true
fi
[ -s "${KEY_PUB:-}" ] && chmod 600 "$KEY_PUB"

# ── Assert the modes rather than assuming them ──
#
# The README's handling rules say the directory is 0700 and both keys 0600, and
# says nothing enforces it. This is that enforcement, on the one path that
# creates them — a `umask` inherited from somewhere else must not decide this.
priv_mode=$(stat -c '%a' "$KEY_PRIV" 2>/dev/null || stat -f '%Lp' "$KEY_PRIV" 2>/dev/null)
dir_mode=$(stat -c '%a' "$KEY_DIR" 2>/dev/null || stat -f '%Lp' "$KEY_DIR" 2>/dev/null)
if [ "$priv_mode" != "600" ] || [ "$dir_mode" != "700" ]; then
    fail "permissions are wrong: .chrysalis-keys/ is ${dir_mode:-?}, chrysalis_update.key is ${priv_mode:-?}
  Expected 700 / 600. Refusing to leave a signing key group- or world-readable."
fi

log "extracted the signing key into .chrysalis-keys/ (key ID ${derived_id:-${pub_id:-?}}, 0600)"
echo
echo "  Signing works now. When the session is over:"
echo "    $0 --remove"
echo
warn "This directory is a CACHE. The durable copy is CHRYSALIS_UPDATE_KEY_B64 in"
warn "server/secrets.env.age. Never treat the file here as the backup."
