#!/usr/bin/env bash
# release-gate.sh — refuse to cut a release that is not ready, BEFORE the tag.
#
# WHY THIS EXISTS
# ---------------
# docs/operate/RELEASING.md states the order, and every step in it has been
# skipped by someone at least once. The failures it records:
#
#   * a tag pushed on a commit whose version files still said the old version —
#     CI caught it, but only after the tag existed and a run had started;
#   * the landing page left advertising the previous release, because step 5
#     ("re-render the page") is a manual step that nothing enforces and whose
#     omission is silent: every button still downloads a real, older file;
#   * a release published with the previous release's legacy update_url, so a
#     client falling back to those fields downloaded the wrong binary.
#
# A procedure written as prose is a procedure with no enforcement. This is the
# enforcement: it runs BEFORE the tag, on the working tree, and exits non-zero
# if anything about the release is out of order. It is deliberately the cheap
# gate — seconds, not a four-platform build.
#
# WHAT IT REFUSES
#   1. version files that disagree with each other (or with docs/state.toml)
#   2. a dirty working tree, or unpushed commits, when about to tag
#   3. a tag that already exists, or a version that is not ahead of the last one
#   4. the build contract not passing (check-consistency.sh, including §37–§39)
#   5. the signing key missing or not matching the public key in tauri.conf.json
#   6. a release whose artifacts are not reproducible from this tree (see §5)
#
# WHAT IT CANNOT CHECK — stated because a green run is not a guarantee:
#   * whether the hub will accept the publish (needs the live host)
#   * whether the landing page is current (needs the served page; that is
#     verify-release.sh's job, AFTER publishing)
#   * whether CI has the signing secret (needs GitHub; see RELEASING.md)
#
# Usage:
#   server/scripts/release-gate.sh              # check the tree is releaseable
#   server/scripts/release-gate.sh --tag v3.4.0 # also check the tag is sane
#   server/scripts/release-gate.sh --help
#
# Exit: 0 ready, 1 not ready (every reason printed), 2 usage.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
FAIL=0
pass() { echo -e "  ${GREEN}OK${NC}   $*"; }
fail() { echo -e "  ${RED}NO${NC}   $*"; FAIL=1; }
warn() { echo -e "  ${YELLOW}!!${NC}   $*"; }

TAG=""
while [ $# -gt 0 ]; do
    case "$1" in
        --tag) TAG="${2:-}"; shift 2 ;;
        -h|--help) sed -n '2,45p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "release-gate: unknown argument '$1'" >&2; exit 2 ;;
    esac
done

# The version the release will be built from. The version files are the
# authority; everything else is compared against them.
VERSION=$(grep -m1 '"version"' client/package.json 2>/dev/null \
    | sed 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')

echo "Release gate — checking the tree is ready to tag"
echo "════════════════════════════════════════════════════════════"

if [ -z "$VERSION" ]; then
    fail "cannot read the version from client/package.json"
    echo -e "${RED}Not ready.${NC}"
    exit 1
fi
echo "  version under test: $VERSION"
echo

echo "1. Every version site agrees"
# Same five sites `release-version.mjs` writes and `state.toml` documents.
for f in client/package.json client/src-tauri/Cargo.toml client/src-tauri/tauri.conf.json; do
    got=""
    case "$f" in
        *.json) got=$(grep -m1 '"version"' "$f" | sed 's/.*:[[:space:]]*"\([^"]*\)".*/\1/') ;;
        *.toml) got=$(grep -m1 '^version' "$f" | sed 's/.*=[[:space:]]*"\([^"]*\)".*/\1/') ;;
    esac
    if [ "$got" = "$VERSION" ]; then
        pass "$f at $got"
    else
        fail "$f says '${got:-<unreadable>}', package.json says '$VERSION'"
    fi
done
state_ver=$(grep -A1 '^\[client\.version\]' docs/state.toml 2>/dev/null \
    | grep -m1 'value' | sed 's/.*=[[:space:]]*"\([^"]*\)".*/\1/')
if [ "$state_ver" = "$VERSION" ]; then
    pass "docs/state.toml at $state_ver"
else
    fail "docs/state.toml says '${state_ver:-<unreadable>}', expected '$VERSION'"
fi
lock_ver=$(grep -m1 -A1 'name = "locus"' client/Cargo.lock 2>/dev/null \
    | grep -m1 'version' | sed 's/.*=[[:space:]]*"\([^"]*\)".*/\1/')
if [ -n "$lock_ver" ] && [ "$lock_ver" != "$VERSION" ]; then
    fail "client/Cargo.lock pins locus $lock_ver, package.json says $VERSION (run: pnpm release-version)"
else
    pass "client/Cargo.lock agrees"
fi
echo

echo "2. The working tree is committed and pushed"
if [ -n "$(git status --porcelain)" ]; then
    fail "the working tree is dirty — a release is built from a commit, not a diff:"
    git status --porcelain | sed 's/^/         /' | head -20
else
    pass "the working tree is clean"
fi
if git rev-parse --abbrev-ref '@{u}' >/dev/null 2>&1; then
    ahead=$(git rev-list --count '@{u}..HEAD' 2>/dev/null || echo 0)
    if [ "${ahead:-0}" -gt 0 ]; then
        fail "$ahead commit(s) are not pushed — CI builds what is on origin, not what is here"
    else
        pass "HEAD is pushed"
    fi
else
    warn "no upstream is set for this branch; push state not checked"
fi
echo

echo "3. The tag and the version"
if [ -n "$TAG" ]; then
    bare="${TAG#v}"
    if [ "$bare" != "$VERSION" ]; then
        fail "tag '$TAG' does not name version '$VERSION' — CI builds binaries that misreport themselves"
    else
        pass "tag '$TAG' names $VERSION"
    fi
    if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null 2>&1; then
        fail "tag '$TAG' already exists"
    else
        pass "tag '$TAG' does not exist yet"
    fi
else
    warn "no --tag given; tag/version agreement not checked (CI enforces it too)"
fi
# A version that moved backwards would make the updater loop: the client is
# offered the older build, installs it, and is offered it again forever.
latest_tag=$(git tag --list 'v*' --sort=-v:refname | head -1)
if [ -n "$latest_tag" ]; then
    if [ "$(printf '%s\n%s\n' "${latest_tag#v}" "$VERSION" | sort -V | tail -1)" = "$VERSION" ] \
        && [ "${latest_tag#v}" != "$VERSION" ]; then
        pass "$VERSION is ahead of the last tag ($latest_tag)"
    elif [ "${latest_tag#v}" = "$VERSION" ]; then
        warn "version equals the last tag ($latest_tag) — a re-release of the same version"
    else
        fail "$VERSION is BEHIND the last tag ($latest_tag) — the updater would loop"
    fi
fi
echo

echo "4. The build contract passes"
if cc_out=$(bash server/scripts/check-consistency.sh 2>&1); then
    pass "check-consistency.sh is green (contract §37–§39 included)"
else
    fail "check-consistency.sh failed:"
    printf '%s\n' "$cc_out" | grep -E 'BAD' | head -10 | sed 's/^/         /'
fi
echo

echo "5. The signing key is present and is the key tauri.conf.json publishes"
PUBKEY_FILE="client/src-tauri/tauri.conf.json"
KEY_PRIV=".locus-keys/locus_update.key"
if [ ! -f "$KEY_PRIV" ]; then
    fail "no private signing key at $KEY_PRIV — every signed build will fail, and"
    fail "  a lost key means no installed client can ever be updated again."
    fail "  See docs/operate/RELEASE-RECOVERY.md before generating a replacement."
elif ! command -v minisign >/dev/null 2>&1; then
    warn "minisign is not installed; cannot confirm the private key matches the published public key"
else
    # Compare the key ID from the private key to the one the config publishes.
    #
    # The config stores the PUBKEY as a base64-encoded minisign public-key FILE
    # (`dW50cnVzdGVkIGNvbW1lbnQ6...`), not as a bare key. An earlier version of
    # this check compared the raw base64 to the `RW…` body and always fell
    # through to a warning — a check that could not fail, which is exactly what
    # this file exists to prevent. So: decode the published blob, and compare
    # the 16-hex key ID that both sides carry. The ID is what the updater uses
    # to decide "is this signed by the key I trust", so it is the right thing
    # to compare.
    # The key ID is carried in the comment line of the PUBLIC key file. The
    # private key's own comment is an arbitrary label ("Locus client update
    # signing key") and carries no ID, so reading the ID from the private file
    # finds nothing and the check cannot pass — which is how this failed the
    # first time. The `.pub` beside the private key is the right source.
    KEY_PUB_SIBLING="$(dirname "$KEY_PRIV")/locus_update.pub"
    key_id_priv=""
    if [ -f "$KEY_PUB_SIBLING" ]; then
        key_id_priv=$(grep -m1 -oE 'key ([0-9A-F]{16})' "$KEY_PUB_SIBLING" 2>/dev/null \
                      | grep -oE '[0-9A-F]{16}$' || true)
    fi
    # Last resort: ask minisign to emit the public key from the private one.
    # This needs no passphrase in minisign's default mode; if it does, the
    # check reports that it could not verify rather than pretending it did.
    if [ -z "$key_id_priv" ]; then
        if minisign -G -p /tmp/rg-pub.$$ -s "$KEY_PRIV" >/dev/null 2>&1; then
            key_id_priv=$(grep -m1 -oE 'key ([0-9A-F]{16})' /tmp/rg-pub.$$ 2>/dev/null \
                          | grep -oE '[0-9A-F]{16}$' || true)
        fi
        rm -f /tmp/rg-pub.$$
    fi

    published_b64=$(grep -m1 -oE '"pubkey"[[:space:]]*:[[:space:]]*"[^"]+"' "$PUBKEY_FILE" 2>/dev/null \
                    | sed 's/.*"\([^"]*\)"$/\1/' || true)
    key_id_pub=""
    if [ -n "$published_b64" ]; then
        key_id_pub=$(printf '%s' "$published_b64" | base64 -d 2>/dev/null \
                     | grep -m1 -oE 'key ([0-9A-F]{16})' \
                     | grep -oE '[0-9A-F]{16}$' || true)
    fi

    if [ -z "$key_id_priv" ]; then
        fail "could not read the key ID from $KEY_PRIV — is it a minisign key?"
    elif [ -z "$key_id_pub" ]; then
        fail "could not decode the published pubkey in $PUBKEY_FILE"
    elif [ "$key_id_priv" = "$key_id_pub" ]; then
        pass "the private key ($key_id_priv) matches the published public key"
    else
        fail "the private key does NOT match the pubkey in tauri.conf.json"
        fail "  private key ID:   $key_id_priv"
        fail "  published key ID: $key_id_pub"
        fail "  CI would sign with a key the updater rejects."
    fi
fi
echo

echo "6. The build inputs are pinned and present"
if python3 server/scripts/sidecar-lock.py --check >/tmp/rg-sidecar.$$ 2>&1; then
    pass "every downloaded build input matches its pinned sha256"
else
    if grep -q '^BAD' /tmp/rg-sidecar.$$; then
        while IFS= read -r line; do
            case "$line" in
                BAD*) fail "${line#BAD	}" ;;
                *) ;;
            esac
        done < /tmp/rg-sidecar.$$
    else
        warn "build inputs not present yet — prebuild will fetch and verify them at build time"
    fi
fi
rm -f /tmp/rg-sidecar.$$
command -v node >/dev/null 2>&1 \
    && pass "node is available for prebuild" \
    || fail "node is not installed; the sidecar fetch cannot run"
echo

echo "════════════════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
    echo -e "${GREEN}Ready to tag ${VERSION}.${NC}"
    echo
    echo "   Next, in this order (docs/operate/RELEASING.md):"
    echo "     git tag -a v${VERSION} -m \"Locus client v${VERSION}\""
    echo "     git push origin main && git push origin v${VERSION}"
    echo "   Then publish, then RE-RENDER THE PAGE (deploy.sh --site),"
    echo "   then confirm with verify-release.sh ${VERSION}."
    exit 0
else
    echo -e "${RED}NOT ready to tag. Fix the NO lines above.${NC}"
    exit 1
fi
