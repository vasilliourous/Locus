#!/usr/bin/env bash
# smoke-signature-encoding.sh — prove the hub stores `signature_<platform>` in the
# exact wire format the client's updater decodes, with no network and no changes
# to the live hub.
#
# WHY THIS EXISTS
#
# The client's `tauri-plugin-updater` runs `base64_to_string()` over the
# `signature_<platform>` field BEFORE parsing the result as minisign text:
#
#     Signature::decode(base64_to_string(signature_<platform>))
#
# So the stored value must be base64 of the WHOLE four-line .sig file, encoded
# exactly once. Storing the .sig text verbatim passes every presence check, is
# advertised to every client, and then fails at install time with
#
#     Invalid byte …, offset N.   (or:  Only base64 data is allowed)
#
# because the .sig file contains spaces and newlines. That is exactly the defect
# that shipped release 3.2.6 uninstallable by every client on all four platforms,
# while every automated check called it healthy.
#
# `check-consistency.sh` §1b greps for the right SHAPE of code. This script goes
# further: it EXTRACTS the encode step and the validation predicate from the
# shipped sources and exercises them, so it cannot pass against a reimplementation
# that has drifted from what actually runs — the same approach as
# `smoke-update-endpoint.sh` takes for the version gate.
#
# Usage:
#   server/scripts/smoke-signature-encoding.sh            # offline, always
#   server/scripts/smoke-signature-encoding.sh --live URL # also checks a live hub
#
# Exit codes: 0 all assertions held, 1 an assertion failed, 2 setup problem.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FETCH="${REPO_ROOT}/server/scripts/fetch-release.py"
HOOK="${REPO_ROOT}/server/pb_hooks/admin_console.pb.js"

[ -f "$FETCH" ] || { echo "smoke-sig: no such file: $FETCH" >&2; exit 2; }
[ -f "$HOOK" ]  || { echo "smoke-sig: no such file: $HOOK" >&2; exit 2; }

LIVE_URL=""
while [ $# -gt 0 ]; do
    case "$1" in
        --live) LIVE_URL="${2:-}"; shift 2 ;;
        *) echo "smoke-sig: unknown argument: $1" >&2; exit 2 ;;
    esac
done

FAILED=0
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
ok()   { printf "  ${GREEN}OK${NC}   %s\n" "$*"; }
bad()  { printf "  ${RED}BAD${NC}  %s\n" "$*"; FAILED=1; }

echo "smoke-sig: signature wire-format check"
echo "════════════════════════════════════════════════════════════"

# ── 1. The encode step in fetch-release.py must be base64(), applied once. ──
# A real four-line minisign signature (ASCII), the shape minisign produces.
SIG_TEXT=$'untrusted comment: signature from minisign secret key\nRUT9eeTiJVDWZMJ1eXBiMS4qlMaGGVF6hE8xEXlBcUdHu0YLdfXUAhs6l6ZH6GgA6b8K6hWD30nVZOrF4zO9MffNdy/LGbvIbwI=\ntrusted comment: Locus linux\nnkBxj2PmjuWWrQy+uZhSupAZZZ35WQ9Np2d+hRWLjVwAT7R24JTo0zRePetJDzCgDJcIwxUaxRYKhbtdoVbiBg=='

CORRECT_WIRE=$(printf '%s' "$SIG_TEXT" | python3 -c 'import base64,sys; print(base64.b64encode(sys.stdin.buffer.read().decode().encode()).decode())')

# Does fetch-release.py, as shipped, produce exactly that from the raw text?
python3 - "$FETCH" > /dev/null 2>&1 <<'PY' && enc_ok=1 || enc_ok=0
import ast, sys, re
src = open(sys.argv[1]).read()
# Find the fold-in assignment: entry["signature"] = ...
m = re.search(r'entry\["signature"\]\s*=\s*(.+)', src)
if not m:
    sys.exit(1)
rhs = m.group(1).strip()
# Must be base64.b64encode(<something>).… and must NOT be a bare .read().strip()
if "base64.b64encode(" not in rhs:
    sys.exit(1)
if re.search(r'\.read\(\)\s*\.strip\(\)\s*$', rhs):
    sys.exit(1)
PY
if [ "$enc_ok" = "1" ]; then
    ok "fetch-release.py base64-encodes the .sig text (not raw text)"
else
    bad "fetch-release.py does NOT base64-encode the stored signature — every client would fail on an offset error"
fi

# ── 2. The rejections must actually be rejected by a base64 decoder. ──
# A raw-.sig field (the shipped bug) must FAIL strict base64 decoding.
if printf '%s' "$SIG_TEXT" | python3 -c 'import base64,sys; base64.b64decode(sys.stdin.read(), validate=True)' 2>/dev/null; then
    bad "raw .sig text unexpectedly decoded as base64 — the premise of this test is wrong"
else
    ok "raw .sig text is rejected by a strict base64 decode (the offset error)"
fi

# And the correct wire value must round-trip to the original text.
if printf '%s' "$CORRECT_WIRE" | python3 -c '
import base64, sys
t = base64.b64decode(sys.stdin.read(), validate=True).decode()
assert t.startswith("untrusted comment: "), t
assert len([l for l in t.splitlines() if l.strip()]) >= 4, t
' 2>/dev/null; then
    ok "base64(whole .sig) round-trips to minisign text (the plugin's decode)"
else
    bad "base64(whole .sig) did not round-trip — the encode step is wrong"
fi

# ── 3. The hook must validate shape in BOTH the publish and set paths, with
#       the same predicate. Extract and exercise it directly. ──
python3 - "$HOOK" > /dev/null 2>&1 <<'PY' && hook_ok=1 || hook_ok=0
import sys, re
src = open(sys.argv[1]).read()
if "function isPlausibleMinisignSignature" not in src:
    sys.exit(1)
# Must be USED in the publish path (on `a.signature`) and the set path (`gsig`).
if not re.search(r'isPlausibleMinisignSignature\(a\.signature\)', src):
    sys.exit(1)
if not re.search(r'isPlausibleMinisignSignature\(gsig\)', src):
    sys.exit(1)
PY
if [ "$hook_ok" = "1" ]; then
    ok "admin_console.pb.js validates the shape in both publish and set paths"
else
    bad "admin_console.pb.js does not validate the signature shape in both paths — publish and activate can disagree"
fi

# ── 4. Optional: check the LIVE hub actually serves the right format. ──
if [ -n "$LIVE_URL" ]; then
    echo
    echo "smoke-sig: checking the live hub at $LIVE_URL"
    for plat in linux windows macos_intel macos_arm; do
        body=$(curl -fsS -m 20 "${LIVE_URL%/}/api/update?version=0.0.1&platform=${plat}" 2>/dev/null || true)
        # A 204 (no body) is a legitimate "nothing to install". Skip those.
        if [ -z "$body" ]; then
            printf "  ${YELLOW}SKIP${NC} %-12s (no update offered)\n" "$plat"
            continue
        fi
        if printf '%s' "$body" | python3 -c '
import base64, json, sys
d = json.load(sys.stdin)
sig = d.get("signature", "")
t = base64.b64decode(sig, validate=True).decode()
assert t.startswith("untrusted comment: "), t
assert len([l for l in t.splitlines() if l.strip()]) >= 4, t
' 2>/dev/null; then
            ok "$plat: live signature field is base64 of the .sig file"
        else
            bad "$plat: live signature field is NOT the correct wire format (clients cannot install this)"
        fi
    done
fi

echo
if [ "$FAILED" -eq 0 ]; then
    printf '\033[0;32mThe signature wire format is correct everywhere it is checked.\033[0m\n'
else
    printf '\033[0;31mSignature wire-format check FAILED — clients may be unable to install the update.\033[0m\n' >&2
fi
exit "$FAILED"
