#!/usr/bin/env bash
# Prove that the contract guard (check-consistency.sh §37) can fail.
#
# WHY THIS EXISTS
#
# docs/reference/FIXES.md, 2026-10-01: "A GUARD THAT WAS CITED IN SIX
# DOCUMENTS DID NOT EXIST." Two weeks earlier, §8 of this same script had
# stopped working and nobody noticed because the output still looked green.
# A guard that cannot fail reads exactly like one that passed, and there is no
# observation that distinguishes them — so the only way to know is to break the
# code on purpose and check that the guard says so.
#
# This script does that, on a temporary copy, for each contract boundary. It
# restores every file it touches, and it re-verifies the tree is clean at the
# end so a failed restore cannot pass silently.
#
# Usage:  bash server/scripts/test-contract-guard.sh
# Exit:   0 = every injected defect was caught AND the tree was restored.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO"

GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
FAIL=0
pass() { echo -e "  ${GREEN}CAUGHT${NC} $*"; }
miss() { echo -e "  ${RED}MISSED${NC} $*"; FAIL=1; }

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Fingerprint every file the tests will touch, BEFORE any of them run, so the
# restore check at the bottom compares content rather than git status.
sha256sum \
    server/pb_hooks/code_lookup.pb.js \
    client/src-tauri/src/locus/contract.rs \
    client/src-tauri/src/locus/tier.rs \
    client/src/services/locus.ts \
    client/src/services/locus.generated.ts \
    client/src-tauri/sidecar/verge-mihomo-x86_64-unknown-linux-gnu \
    client/src-tauri/src/core/manager/lifecycle.rs \
    > "$TMP/before.sha256" 2>/dev/null

# expect_caught <label> <file> <python-patch-expr>
# Copies the file, applies a patch, runs the checker, restores, and asserts the
# checker failed with a BAD line mentioning the expected text.
expect_caught() {
    label="$1"; file="$2"; patcher="$3"; needle="$4"
    cp "$file" "$TMP/orig"
    python3 - "$file" "$patcher" <<'PY'
import sys
path, patcher = sys.argv[1], sys.argv[2]
# Build the callable as a whole expression. `eval("lambda s: " + patcher)`
# applies the lambda to the string, not the patch to the source — a precedence
# bug that made every test below a no-op while the harness still reported on
# them. This is the failure mode the harness exists to catch, in the harness.
patch = eval(patcher)
src = open(path, encoding="utf-8").read()
out = patch(src)
if out == src:
    raise SystemExit("patch was a no-op — the test proves nothing")
open(path, "w", encoding="utf-8").write(out)
PY
    if [ $? -ne 0 ]; then
        cp "$TMP/orig" "$file"
        miss "$label (the injected defect did not apply — test invalid)"
        return
    fi
    output=$(python3 "$REPO/server/scripts/contract-snapshot.py" --check 2>&1)
    status=$?
    cp "$TMP/orig" "$file"
    if [ "$status" -eq 0 ]; then
        miss "$label — the checker passed with the defect in place"
    elif printf '%s' "$output" | grep -qF "$needle"; then
        pass "$label"
    else
        miss "$label — failed, but not for the expected reason (wanted: $needle)"
        printf '%s\n' "$output" | sed 's/^/      /'
    fi
}

echo "Proving the contract guard can fail"
echo "════════════════════════════════════════════════════════════"

# 1. A wire field removed — the `uot_port` / `server_port_uot` class.
expect_caught \
    "a wire struct field removed (TierConfig.uot_port)" \
    "client/src-tauri/src/locus/contract.rs" \
    'lambda s: s.replace("""    #[serde(rename = "uot_port", alias = "server_port_uot", default)]\n    pub uot_port: u16,\n""", "")' \
    "no longer has field uot_port"

# 2. A wire field renamed — the silent-no-op class, where both sides still
#    compile and the value simply never arrives.
expect_caught \
    "a wire field respelled (ActivateResponse.device_fingerprint)" \
    "client/src-tauri/src/locus/contract.rs" \
    'lambda s: s.replace("    pub device_fingerprint: Option<String>,", """    #[serde(rename = "deviceFingerprint")]\n    pub device_fingerprint: Option<String>,""")' \
    "changed wire name"

# 3. An endpoint the hub no longer serves but the client still calls.
expect_caught \
    "a hub route removed under a client that calls it (/api/code-lookup)" \
    "server/pb_hooks/code_lookup.pb.js" \
    'lambda s: s.replace("routerAdd(\"POST\", \"/api/code-lookup\"", "routerAdd(\"POST\", \"/api/lookup-v2\"")' \
    "no such route"

# 4. An enum wire value dropped — the `bound_this_device` / `already_used` class.
expect_caught \
    "an enum wire value removed (LookupStatus::Unbound)" \
    "client/src-tauri/src/locus/contract.rs" \
    'lambda s: s.replace("    /// The code exists and has not been redeemed yet.\n    Unbound,\n", "")' \
    "no longer accepts wire value"

# 5. The mihomo reference-loop trap: the group name made equal to the proxy
#    name. YAML stays valid; only the engine refuses it.
expect_caught \
    "the proxy group name made equal to the proxy name" \
    "client/src-tauri/src/locus/tier.rs" \
    'lambda s: s.replace("""pub const GROUP_NAME: &str = "Locus Auto";""", """pub const GROUP_NAME: &str = "Locus";""")' \
    "reference loop"

echo "════════════════════════════════════════════════════════════"

# ── §38: the TypeScript mirror is generated, and §33 that it stays generated ──
#
# These two are tested differently from the five above, because the defect they
# guard against is not in the contract source — it is in the GENERATED PRODUCT
# or in the hand-written file that is supposed to consume it. So the wrong text
# is injected into the downstream file instead.

# check_full <label> <file> <patcher> <needle>
# Same shape as expect_caught, but runs the WHOLE check-consistency.sh rather
# than only the snapshot checker, because §33 and §38 are sections of that
# script. Restores the file afterwards.
check_full() {
    label="$1"; file="$2"; patcher="$3"; needle="$4"
    cp "$file" "$TMP/orig_full"
    python3 - "$file" "$patcher" <<'PY'
import sys
path, patcher = sys.argv[1], sys.argv[2]
patch = eval(patcher)
src = open(path, encoding="utf-8").read()
out = patch(src)
if out == src:
    raise SystemExit("patch was a no-op")
open(path, "w", encoding="utf-8").write(out)
PY
    if [ $? -ne 0 ]; then
        cp "$TMP/orig_full" "$file"
        miss "$label (the injected defect did not apply — test invalid)"
        return
    fi
    output=$(bash "$REPO/server/scripts/check-consistency.sh" 2>&1)
    status=$?
    cp "$TMP/orig_full" "$file"
    if [ "$status" -eq 0 ]; then
        miss "$label — the checker passed with the defect in place"
    elif printf '%s' "$output" | grep -qF "$needle"; then
        pass "$label"
    else
        miss "$label — failed, but not for the expected reason (wanted: $needle)"
    fi
}

# 6. §38: the committed generated file drifts from the Rust. This is what a
#    developer who edits the Rust type and forgets to regenerate produces.
check_full \
    "the generated TS file left stale after a Rust type change (§38)" \
    "client/src/services/locus.generated.ts" \
    'lambda s: s.replace("  ready: boolean", "  ready: boolean\n  staleDriftField: boolean", 1)' \
    "no longer match the Rust types"

# 7. §33: someone hand-writes the interface back into services/locus.ts,
#    shadowing the generated type. §38 would stay green — the generated file is
#    untouched — so §33 is the only thing that catches this.
check_full \
    "a hand-written interface redeclared over the generated type (§33)" \
    "client/src/services/locus.ts" \
    'lambda s: s + "\nexport interface LocusStatus {\n  activated: boolean\n}\n"' \
    "redeclares LocusStatus"

echo "════════════════════════════════════════════════════════════"

# ── §39: the downloaded build inputs ──
#
# Tested by content again, but against a file in `src-tauri/sidecar/` rather
# than a source file: the defect is a download that does not match its pin.
# The substitute is a VALID gzip — the same shape a corrupted-but-plausible
# archive has, which the old magic-byte check would have accepted.

MIHOMO="client/src-tauri/sidecar/verge-mihomo-x86_64-unknown-linux-gnu"
if [ -f "$MIHOMO" ]; then
    cp "$MIHOMO" "$TMP/mihomo.bak"
    # Prepend a gzip header so the result is still a valid gzip stream.
    { printf '\037\213\010\000\000\000\000\000\000\003'; cat "$TMP/mihomo.bak"; } > "$TMP/mihomo.bad"
    mv "$TMP/mihomo.bad" "$MIHOMO"
    out=$(python3 "$REPO/server/scripts/sidecar-lock.py" --check 2>&1)
    st=$?
    cp "$TMP/mihomo.bak" "$MIHOMO"
    if [ "$st" -eq 0 ]; then
        miss "a substituted engine with a valid gzip header (§39)"
    elif printf '%s' "$out" | grep -qF "but the lock pins"; then
        pass "a substituted engine with a valid gzip header (§39)"
    else
        miss "a substituted engine — failed, but not for the expected reason"
    fi
else
    miss "a substituted engine (§39) — the sidecar is not present, cannot test"
fi

echo "════════════════════════════════════════════════════════════"

# ── §21: the extractor must not report a defect it never looked for ──
#
# Observed once on 2026-10-26: §21 printed
#   BAD prepare_startup does not CALL should_request_install_for_absent_service
# on a tree where the call site was demonstrably present at line 512, and went
# green on the identical tree minutes later. A single read had caught the file
# mid-write (cargo was running), so the extractor returned a truncated body and
# the guard reported a defect it never looked for. That is exactly the
# "check cannot distinguish 'wrong' from 'unreadable'" failure this repo has
# paid for before — and it can now block a release, because release-gate.sh runs
# this script.
#
# The extractor now re-reads until two reads agree. This test proves the two
# behaviours that matter: a truncated file must NOT produce a code-side BAD, and
# a genuinely missing call site must still produce one.

LIFECYCLE="client/src-tauri/src/core/manager/lifecycle.rs"
if [ -f "$LIFECYCLE" ]; then
    cp "$LIFECYCLE" "$TMP/lifecycle.bak"

    # (a) A file that cannot be read whole. `head -c` produces a valid prefix
    #     that ends mid-file; the stability check cannot accept it, so the guard
    #     must report "cannot read", never a BAD about the code.
    head -c 4000 "$TMP/lifecycle.bak" > "$LIFECYCLE"
    out=$(bash "$REPO/server/scripts/check-consistency.sh" 2>&1)
    cp "$TMP/lifecycle.bak" "$LIFECYCLE"
    if printf '%s' "$out" | grep -q 'BAD.*does not CALL should_request_install'; then
        miss "a truncated lifecycle.rs reported a code defect (§21)"
    else
        pass "a truncated lifecycle.rs does not report a code defect (§21)"
    fi

    # (b) The real defect must still fail. Without this, (a) could be satisfied
    #     by a guard that never fails at all.
    python3 - "$LIFECYCLE" <<'PY'
import sys
path = sys.argv[1]
s = open(path, encoding="utf-8").read()
old = "        if should_request_install_for_absent_service("
assert old in s
open(path, "w", encoding="utf-8").write(
    s.replace(old, "        if false && should_request_install_for_absent_service(", 1))
PY
    out=$(bash "$REPO/server/scripts/check-consistency.sh" 2>&1)
    cp "$TMP/lifecycle.bak" "$LIFECYCLE"
    if printf '%s' "$out" | grep -q 'BAD.*does not CALL should_request_install'; then
        pass "a genuinely missing call site is still reported (§21)"
    else
        miss "the §21 guard no longer catches a removed call site"
    fi
else
    miss "§21 truncation test — lifecycle.rs not found"
fi

echo "════════════════════════════════════════════════════════════"

# The restore must be proved, not assumed: a test that leaves a file patched is
# worse than no test, because the next person debugs a phantom.
#
# HOW THIS IS CHECKED, AND WHY NOT WITH `git status`. The first version compared
# `git status --porcelain` against an empty expectation and failed on a clean
# checkout — because the files under test were themselves uncommitted work, and
# git reports those as modified whether or not the test restored them. That
# check could not distinguish "the test broke the file" from "the file was
# already being edited", so it failed for the wrong reason while the restore was
# in fact correct.
#
# The honest check is content: every file the test touches must be byte-identical
# to how it was found. `git diff` against HEAD cannot say that, and neither can
# `git status`; a hash of the pre-test state can.
restore_ok=1
for f in server/pb_hooks/code_lookup.pb.js \
         client/src-tauri/src/locus/contract.rs \
         client/src-tauri/src/locus/tier.rs \
         client/src/services/locus.ts \
         client/src/services/locus.generated.ts \
         client/src-tauri/sidecar/verge-mihomo-x86_64-unknown-linux-gnu \
         client/src-tauri/src/core/manager/lifecycle.rs; do
    want=$(sed -n 's/^\([a-f0-9]\{64\}\)  .*/\1/p' "$TMP/before.sha256" 2>/dev/null | head -1)
    if [ -f "$TMP/before.sha256" ]; then
        expected=$(grep "  $f\$" "$TMP/before.sha256" | cut -d' ' -f1)
        actual=$(sha256sum "$f" | cut -d' ' -f1)
        if [ -n "$expected" ] && [ "$expected" != "$actual" ]; then
            echo -e "  ${RED}BAD${NC}  the test left $f modified (content differs from before the test)"
            restore_ok=0
            FAIL=1
        fi
    fi
done
if [ "$restore_ok" -eq 1 ]; then
    echo -e "  ${GREEN}OK${NC}   every patched file was restored byte-for-byte"
fi

if [ "$FAIL" -eq 0 ]; then
    echo -e "${GREEN}The contract guard fails on every defect it claims to catch.${NC}"
else
    echo -e "${RED}At least one injected defect was not caught. The guard is incomplete.${NC}"
fi
exit "$FAIL"
