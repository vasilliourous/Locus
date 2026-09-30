#!/usr/bin/env bash
# Locus cross-language consistency check.
#
# WHY THIS EXISTS
#
# This project's worst bugs have not been logic errors. They have been the same
# shape, repeatedly:
#
#   * `uot_port` vs `server_port_uot` — the hub emitted one name, the client
#     declared another, and the mismatch was a SILENT no-op. UDP-over-TCP was
#     dead fleet-wide while both sides were individually "correct".
#   * `download_<platform>` vs `update_<platform>` — two publish scripts
#     disagreed, and a Windows client was handed a Linux binary.
#   * `unbound_at` / `unbind_reason` — written by a hook for months against
#     columns that did not exist. PocketBase silently discarded them, so an
#     audit trail looked implemented and recorded nothing.
#   * `aes-256-gcm` duplicated in ten places with nothing binding them.
#
# Every one is a value that must agree across languages (Rust / goja JS /
# Python / shell) with NO mechanism enforcing agreement. A comment saying
# "keep these in sync" is not a mechanism.
#
# This script is the mechanism. It is deliberately dumb and greppy: it does not
# try to understand the code, it checks that the constants agree and fails loudly
# when they do not. Run it before any change that touches a wire name, a
# platform key, the schema, or the cipher.
#
# Usage:  server/scripts/check-consistency.sh        (from anywhere)
# Exit:   0 = consistent, 1 = drift found.

set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOKS="$REPO/server/pb_hooks"
SCRIPTS="$REPO/server/scripts"
RUST="$REPO/client/src-tauri/src/locus"

FAIL=0
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

ok()   { echo -e "  ${GREEN}OK${NC}   $*"; }
bad()  { echo -e "  ${RED}BAD${NC}  $*"; FAIL=1; }
warn() { echo -e "  ${YELLOW}WARN${NC} $*"; }

echo "Locus cross-language consistency check"
echo "════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────
# 1. The platform key list must be identical everywhere it appears.
#
# It is duplicated in six hooks (PocketBase loads each *.pb.js separately, so
# they cannot share a module) plus Rust and Python. A platform missing from any
# ONE of these means that platform silently stops being offered an update.
# ─────────────────────────────────────────────────────────────
echo
echo "1. Platform keys"

EXPECTED='["linux","windows","macos_intel","macos_arm"]'
EXPECTED_SPACED='["linux", "windows", "macos_intel", "macos_arm"]'

# Every literal list in the hooks, normalised (strip spaces) for comparison.
hook_lists=$(grep -rhoE '\["linux"[^]]*\]' "$HOOKS"/*.js 2>/dev/null | tr -d ' ' | sort -u)

if [ -z "$hook_lists" ]; then
    bad "no platform-key list found in $HOOKS — has the layout changed?"
elif [ "$(printf '%s\n' "$hook_lists" | wc -l)" -gt 1 ]; then
    bad "the hooks disagree with each other about the platform list:"
    printf '%s\n' "$hook_lists" | sed 's/^/         /'
else
    found=$(printf '%s' "$hook_lists")
    if [ "$found" = "$EXPECTED" ]; then
        n=$(grep -rlE '\["linux"[^]]*\]' "$HOOKS"/*.js 2>/dev/null | wc -l)
        ok "all hook platform lists identical across ${n} file(s)"
    else
        bad "hook platform list is '$found', expected '$EXPECTED'"
    fi
fi

# Python: the signature columns and the release fetch must cover the same set.
for p in linux windows macos_intel macos_arm; do
    grep -q "\"$p\"" "$SCRIPTS/fetch-release.py" 2>/dev/null \
        || bad "fetch-release.py does not mention platform '$p'"
done

# Rust: the constants, and the artifact mapping for each.
for pair in "PLATFORM_LINUX:linux" "PLATFORM_WINDOWS:windows" \
            "PLATFORM_MACOS_INTEL:macos_intel" "PLATFORM_MACOS_ARM:macos_arm"; do
    const="${pair%%:*}"; value="${pair#*:}"
    if grep -qE "^pub const ${const}: &str = \"${value}\";" "$RUST/contract.rs" 2>/dev/null; then
        ok "contract.rs ${const} = \"${value}\""
    else
        bad "contract.rs ${const} is not \"${value}\""
    fi
done

# ─────────────────────────────────────────────────────────────
# 1b. The update-signature WIRE FORMAT.
#
# The client's `tauri-plugin-updater` runs `base64_to_string()` over the
# `signature_<platform>` field BEFORE parsing the result as minisign text:
#
#     Signature::decode(base64_to_string(signature_<platform>))
#
# So the stored value MUST be base64 of the ENTIRE four-line .sig file — not the
# file text itself. This is the exact fault that shipped a live `update_config`
# row no client could install: the raw text (which contains spaces and newlines)
# fails base64 decoding on the client with `Invalid byte …, offset N`. Every
# automated check called the release healthy.
#
# Three places must agree, and a comment saying "keep these in sync" is not a
# mechanism — this is:
#
#   1. fetch-release.py must base64-ENCODE the .sig text when folding it in.
#   2. admin_console.pb.js must VALIDATE the shape on BOTH publish and activate,
#      using the SAME predicate, or one path accepts what the other rejects.
#   3. the client contract test must pin base64 as the wire format.
# ─────────────────────────────────────────────────────────────
echo
echo "1b. Update-signature wire format (base64 of the whole .sig)"

if grep -q 'base64.b64encode(sig_text' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    ok "fetch-release.py base64-encodes the .sig text before storing it"
else
    bad "fetch-release.py does not base64-encode the stored signature — the
         raw .sig text fails the client's base64 decode (\"Invalid byte, offset\")"
fi

# The value must NOT be the raw file text. `.strip()` alone (no encode) is the
# old bug; catch a regression that reintroduces it.
if grep -qE 'entry\["signature"\] = f\.read\(\)\.strip\(\)' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    bad "fetch-release.py stores the raw .sig text again (the offset bug)"
else
    ok "fetch-release.py no longer stores the raw .sig text"
fi

# The console must validate the shape in BOTH the publish and set paths.
sig_shape_uses=$(grep -c 'isPlausibleMinisignSignature(' "$HOOKS/admin_console.pb.js" 2>/dev/null)
if grep -q 'function isPlausibleMinisignSignature' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
    ok "admin_console.pb.js defines the shared signature-shape predicate"
else
    bad "admin_console.pb.js has no isPlausibleMinisignSignature predicate"
fi
if grep -qE 'isPlausibleMinisignSignature\(a\.signature\)' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
    ok "releases.publish validates the signature shape before writing it"
else
    bad "releases.publish writes signature_<platform> without a shape check —
         a raw-text row can be published and reach every client"
fi
if grep -qE 'isPlausibleMinisignSignature\(gsig\)' "$HOOKS/admin_console.pb.js" 2>/dev/null; then
    ok "releases.set validates the signature shape before activating"
else
    bad "releases.set no longer validates the stored signature shape"
fi
[ "$sig_shape_uses" -ge 3 ] || bad "the signature-shape predicate is defined but not used in both paths (uses=$sig_shape_uses)"

# The client test must still pin base64(the whole .sig text) as the wire format.
if grep -q 'the whole .sig file, base64' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null \
   || grep -q 'base64 of the ENTIRE' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null \
   || grep -q 'base64(whole' "$REPO/client/src-tauri/tests/update_signature_contract.rs" 2>/dev/null; then
    ok "the client contract test pins base64-of-the-whole-.sig as the wire format"
else
    warn "could not confirm the client test pins the base64 wire format — check update_signature_contract.rs"
fi

# ─────────────────────────────────────────────────────────────
# 2. Frozen wire names.
#
# Both directions matter: the hub must keep EMITTING the old name (deployed
# clients read it) and the client must keep READING it (the hub passes the
# stored key through verbatim).
# ─────────────────────────────────────────────────────────────
echo
echo "2. Frozen wire names"

# The hub passes tier_configs.config through verbatim, so the stored key is the
# wire key. `uot_port` must appear in the hook, the seeder, and the client.
if grep -q 'uot_port' "$HOOKS/heartbeat.pb.js" 2>/dev/null; then
    ok "heartbeat hook references uot_port"
else
    bad "heartbeat hook no longer references uot_port (clients read it)"
fi

if grep -q 'uot_port' "$SCRIPTS/seed-pb.py" 2>/dev/null; then
    ok "seed-pb.py writes uot_port"
else
    bad "seed-pb.py no longer writes uot_port"
fi

if grep -qE 'rename = "uot_port"' "$RUST/contract.rs" 2>/dev/null; then
    ok "client deserialises uot_port"
else
    bad "client no longer deserialises uot_port (UDP-over-TCP would go silent)"
fi

# The record->heartbeat rename is load-bearing: deployed clients read
# `update_<platform>`, the record stores `download_<platform>`.
if grep -q 'update_' "$HOOKS/heartbeat.pb.js" 2>/dev/null; then
    ok "heartbeat hook still emits update_<platform>"
else
    bad "heartbeat hook no longer emits update_<platform> (deployed clients read it)"
fi

if grep -q 'download_' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
    ok "publish-release.sh still writes download_<platform>"
else
    bad "publish-release.sh no longer writes download_<platform>"
fi

# ─────────────────────────────────────────────────────────────
# 3. Fields the hooks WRITE must exist in the collection schema.
#
# This is the `unbound_at` class: PocketBase silently discards an unknown
# field, so a hook can write faithfully for months into nothing. `seed-pb.py`
# is the schema authority; this checks the two agree.
# ─────────────────────────────────────────────────────────────
echo
echo "3. Hook-written fields vs the schema"

schema_fields=$(grep -oE '"name"[[:space:]]*:[[:space:]]*"[a-z0-9_]+"' "$SCRIPTS/seed-pb.py" 2>/dev/null \
    | sed 's/.*"\([a-z0-9_]*\)"$/\1/' | sort -u)

if [ -z "$schema_fields" ]; then
    bad "could not extract any field names from seed-pb.py — has its shape changed?"
else
    # Fields written via `.set("name", ...)` in the hooks. Deliberately excludes
    # PocketBase's own auto fields (created/updated/id) and the code_events
    # collection, which is written with the same helper.
    # Fields written via `.set("name", ...)` in the hooks. Two shapes occur:
    # a literal name, and a concatenated prefix like `"download_" + platform`
    # which expands to the schema's `download_linux`, `download_windows`, ...
    # The second is expanded here rather than skipped — it is exactly the kind
    # of dynamic write that hides a missing column.
    hook_fields=$(
        {
            # Literal field names. A trailing '_' means the name is a
            # concatenation prefix, which the block below expands instead.
            grep -rhoE '\.set\("[a-z_]+"' "$HOOKS"/*.js 2>/dev/null \
                | sed 's/\.set("//;s/"//' \
                | grep -v '_$'
            # Concatenated prefixes: emit one field per known platform suffix.
            for prefix in $(grep -rhoE '\.set\("[a-z0-9_]+_"' "$HOOKS"/*.js 2>/dev/null \
                    | sed 's/\.set("//;s/"//' | sort -u); do
                for plat in linux windows macos_intel macos_arm; do
                    printf '%s%s\n' "$prefix" "$plat"
                done
            done
        } | sort -u
    )

    missing=""
    for f in $hook_fields; do
        case "$f" in
            id|created|updated|collectionId|collectionName|expand) continue ;;
        esac
        printf '%s\n' "$schema_fields" | grep -qx "$f" || missing="$missing $f"
    done

    if [ -n "$missing" ]; then
        bad "hook(s) write field(s) that are NOT in the schema — PocketBase will"
        bad "silently discard them, so the write looks like it worked:"
        echo "        $missing"
        echo "        Add them to seed-pb.py, or stop writing them."
    else
        n=$(printf '%s\n' "$hook_fields" | wc -l)
        ok "all ${n} hook-written field(s) exist in the schema"
    fi
fi

# ─────────────────────────────────────────────────────────────
# 4. The cipher is defined exactly once.
#
# The SS2022 migration collapsed ten copies into one. This keeps it collapsed:
# a bare quoted SS2022 cipher string anywhere except modules/00-env.sh and the
# templates (which are declarative references, not config writers) is drift.
# ─────────────────────────────────────────────────────────────
echo
echo "4. Cipher single-definition"

cipher_defs=$(grep -rn '2022-blake3' "$REPO/server" \
    --include=*.sh --include=*.py 2>/dev/null \
    | grep -v node_modules \
    | grep -v 'check-consistency.sh' \
    | grep -vE 'SS_METHOD=|\$\{SS_METHOD\}|SS_METHOD\b|04-tc' \
    | grep -cE '"2022-blake3|= *"2022-blake3')

if [ "$cipher_defs" -eq 0 ]; then
    ok "no hardcoded SS2022 cipher outside the SS_METHOD definition"
else
    bad "${cipher_defs} hardcoded SS2022 cipher literal(s) outside SS_METHOD:"
    grep -rn '2022-blake3' "$REPO/server" --include=*.sh --include=*.py 2>/dev/null \
        | grep -v node_modules \
        | grep -v 'check-consistency.sh' \
        | grep -vE 'SS_METHOD=|\$\{SS_METHOD\}|SS_METHOD\b' \
        | grep -E '"2022-blake3' | sed 's/^/         /'
fi

if grep -q 'SS_METHOD=' "$REPO/server/modules/00-env.sh" 2>/dev/null; then
    ok "SS_METHOD is defined in modules/00-env.sh"
else
    bad "SS_METHOD is not defined in modules/00-env.sh"
fi

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

# ─────────────────────────────────────────────────────────────
# 6. PocketBase hook traps.
#
# This project has lost more time to goja/PocketBase specifics than to logic
# errors, and each one failed SILENTLY — a rate limit that never ran, an audit
# column that recorded nothing, a helper that 500'd a whole endpoint for its
# entire life. Each is mechanical enough to check:
#
#   a. File-scope helper functions are invisible inside routerAdd callbacks
#      ("helperFn is not defined"). Every hook here declares helpers INSIDE the
#      callback for this reason; a file-level `function` is a latent 500.
#   b. `findRecordsByFilter` returns zero rows with no error on PB 0.22.21. Any
#      use of it is a silent no-op — the working calls are findFirstRecordByFilter
#      and findRecordsByExpr.
#   c. Admin token comparison must not use `!==`/`==` on the secret, which exits
#      at the first differing byte.
#   d. A hook that claims to read `X-Admin-Token` must read it from the REQUEST
#      headers. `$apis.requestInfo(e).data` carries the body only, and
#      `e.request().header.get(...)` reads the key-value store, which never holds
#      headers — so such a fallback is dead code that looks like a feature.
# ─────────────────────────────────────────────────────────────
echo
echo "6. PocketBase hook traps"

HOOKS_DIR="$REPO/server/pb_hooks"

# (a) A file-scope `function name()` — i.e. one at column 0 — is not visible to
# routerAdd callbacks. Inline helpers are indented inside the callback.
file_scope_fn=$(grep -rn '^function ' "$HOOKS_DIR"/*.js 2>/dev/null || true)
if [ -z "$file_scope_fn" ]; then
    ok "no file-scope function declarations in any hook"
else
    bad "file-scope function declaration(s) — invisible inside routerAdd callbacks:"
    printf '%s\n' "$file_scope_fn" | sed 's/^/         /'
fi

# (b) findRecordsByFilter is broken in this PB build.
#
# Match only an ACTUAL call, not a comment describing the trap. Every hook here
# carries a warning comment naming this function, so a naive grep reports the
# documentation as the defect. Stripping to code before the comment marker, and
# requiring the call syntax `findRecordsByFilter(`, is what makes this usable.
broken_filter=$(
    for f in "$HOOKS_DIR"/*.js; do
        [ -f "$f" ] || continue
        # `sed 's://.*::'` drops `//` line comments, so prose cannot match.
        code=$(sed 's://.*::' "$f")
        hit=$(printf '%s\n' "$code" | grep -n 'findRecordsByFilter(' || true)
        [ -n "$hit" ] && printf '%s: %s\n' "$f" "$hit"
    done
)
if [ -z "$broken_filter" ]; then
    ok "no use of findRecordsByFilter (silently returns zero rows on this build)"
else
    bad "findRecordsByFilter called — returns zero rows with no error:"
    printf '%s\n' "$broken_filter" | sed 's/^/         /'
fi

# (c) Token comparisons must be constant-time, not `!==`.
plain_compare=$(grep -rnE '(adminToken|supplied|admin_token)[[:space:]]*!==[[:space:]]*validToken|validToken[[:space:]]*!==[[:space:]]*(adminToken|supplied)' "$HOOKS_DIR"/*.js 2>/dev/null || true)
if [ -z "$plain_compare" ]; then
    ok "admin token compared without an early-exit (constantTimeEquals)"
else
    bad "admin token compared with !== — leaks match length by timing:"
    printf '%s\n' "$plain_compare" | sed 's/^/         /'
fi

# (d) A hook must not claim a header fallback it cannot honour. Look for the
# dead pattern specifically: reading a hyphenated header name out of the
# requestInfo key-value store.
dead_header=$(grep -rn 'requestInfo(e)\.data\|\.header\.get("X-\|\.header\.get(.X-' "$HOOKS_DIR"/*.js 2>/dev/null | grep -E 'header\.get\("X-' || true)
if [ -z "$dead_header" ]; then
    ok "no header read through the requestInfo key-value store"
else
    bad "header read through e.request().header.get() — that store holds the body, not headers:"
    printf '%s\n' "$dead_header" | sed 's/^/         /'
fi

# (e) The binding index must be read and written under the SAME key.
#
# WHY THIS IS A CHECK AND NOT A COMMENT. Fingerprints are normalised to
# [a-zA-Z0-9] before touching `device_bindings`. An earlier version looked up
# the stripped value but INSERTED the raw one, and compared a stored
# (normalised) value against a raw incoming fingerprint. The failure is silent:
# the write succeeds, the row looks right, and only the uniqueness check quietly
# stops matching — which is precisely the "one code per device" guarantee.
#
# So: any hook that writes a fingerprint into a row must normalise it in the
# same statement. A bare `set("fingerprint", fp)` or
# `set("bound_fingerprint", fp)` is the bug. The normalised form is required.
raw_fp_write=$(grep -rnE 'set\("(fingerprint|bound_fingerprint)",\s*(fp|newFp|fingerprint)\s*\)' "$HOOKS_DIR"/*.js 2>/dev/null || true)
if [ -z "$raw_fp_write" ]; then
    ok "fingerprints are normalised before being written to a binding"
else
    bad "raw fingerprint written to a binding — must use the stripped value, or the"
    bad "index lookup (which normalises) will not find the row:"
    printf '%s\n' "$raw_fp_write" | sed 's/^/         /'
fi

# ─────────────────────────────────────────────────────────────
# 7. Version agreement, across manifests AND prose.
#
# The three manifests are already enforced by
# `client/src-tauri/tests/version_consistency.rs`. What nothing checked until
# 2026-09-29 is the DOCUMENTATION: when the client went to 3.2.4, nine documents
# were left saying 3.2.3 — including README.md, the project's front door, and
# docs/README.md, the index.
#
# Stale prose is a real defect here rather than cosmetic: a reader who trusts a
# version number in the README will draw wrong conclusions about which fixes are
# absent (FIXES.md is explicit that "a retired component's limitation is not the
# live system's"), and the release path itself is version-sensitive.
#
# This reports the manifest version and any document that names a DIFFERENT
# 3.x version. It is a WARNING, not a failure: historical documents legitimately
# record the version they were written against, so this cannot be an equality
# check. The purpose is to make drift visible in CI output instead of invisible.
# ─────────────────────────────────────────────────────────────
echo
echo "7. Version agreement (informational)"

manifest_version=$(grep -m1 '"version"' "$REPO/client/package.json" 2>/dev/null \
    | sed 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')

if [ -z "$manifest_version" ]; then
    warn "could not read the version from client/package.json"
else
    ok "client/package.json is at ${manifest_version}"

    # Every tracked markdown file that names a different 3.x version.
    #
    # WHAT THIS IS FOR: catching a document that states a stale CURRENT version
    # — "currently 3.2.3" when the tree is 3.2.5 — because a reader trusts it
    # and a stale number is a false statement about what is shipping.
    #
    # WHAT IT IS NOT FOR: a document that mentions an OLDER version as history.
    # "first shipped as 3.0.0", "the 3.1.0 rework replaced this surface" and
    # "publish-release.sh 3.1.0" (an example argument) are all CORRECT and must
    # not be "fixed". A warning that fires on legitimate history trains the
    # reader to ignore it, which is worse than no warning — so the exclusions
    # below are deliberate and each is justified:
    #
    #   docs/reference/FIXES.md, docs/reference/STILL-OPEN.md   chronological records of past versions
    #   docs/history/, docs/archive/        explicitly historical
    #   client/docs/FRONTEND.md             names 3.1.0 as the rework that
    #                                       replaced the surface — history
    #
    # Files that KEEP their stamp and are therefore NOT excluded — because the
    # number is correct precisely as written, and stripping it would delete a
    # true statement about a design basis rather than fix drift:
    #
    #   docs/CONTEXT.md, docs/README.md     "first shipped as 3.0.0 and now X" —
    #                                       the trailing number IS the current
    #                                       version and is enforced below
    #   docs/operate/RELEASING.md                   versions in EXAMPLE command arguments
    #   docs/business/                      every file stamps its design basis
    #                                       as "tree at <commit> (client 3.2.x)"
    #   client/docs/RESTRUCTURE.md          a table of the versions as they were
    #   client/docs/ARCHITECTURE.md         "the current line is 3.2.x" (design
    #                                       record, banner-marked as historical)
    #   client/docs/UPSTREAM-CHANGES.md     "inherited 2.5.5 | 3.2.x — its own line"
    #
    # TWO MECHANISMS USED TO HIDE DRIFT HERE, and both are fixed in this revision:
    #
    #   1. THE NUMBER FILTER WAS BLIND TO ASTERISK-WRAPPED VERSIONS. The old
    #      filter excluded char class [.^~<>=] but not `*`, so `**3.2.5**` — the
    #      standard bold form in this repo's tables and headers — did not match
    #      and the file was never reported. Measured: client/docs/UPSTREAM-CHANGES.md
    #      and client/docs/ARCHITECTURE.md both named 3.2.5 and were invisible.
    #   2. THE EXCLUSION LIST EXCLUDED WHOLE CLASSES OF FILE. It protected
    #      docs/CONTEXT.md, docs/README.md and docs/business/ unconditionally, so
    #      drift inside the index, the front door and the business docs could not
    #      be reported at all. That is the inverse of a guard: the files a reader
    #      is most likely to trust were the ones nothing checked.
    #
    # A third defect is now ENFORCED rather than reported: an attributed stamp
    # reads "... client 3.2.x" / "at 3.2.x" where the x alone is the intended
    # version. Drift there is invisible to a bare-number grep, so it is checked
    # separately below.
    stale=$(
        cd "$REPO" && grep -rEl --include=*.md '3\.[0-9]+\.[0-9]+' \
            README.md docs client/docs 2>/dev/null \
            | grep -vE '^(docs/reference/FIXES\.md|docs/reference/STILL-OPEN\.md|docs/history/.*|docs/archive/.*|client/docs/FRONTEND\.md)$' \
            | while read -r f; do
                # Any 3.x.y in the file that is not the current version.
                #
                # Dependency pins (`^3.4.0`, `>=3.2.0`, `3.6.5`) are NOT the
                # client version and are excluded: a `^`/`~`/`>`/`=` prefix, or a
                # version inside a version-range table, would otherwise flood the
                # report with noise and train the reader to ignore it. Only a
                # bare, unadorned x.y.z counts.
                #
                # `*` IS NOT EXCLUDED — it is markdown emphasis, not a range
                # operator. Excluding it (as this check did until 2026-09-29) made
                # the standard bold form `**3.2.5**` invisible, which is exactly
                # how the docs write a version in a table cell or a header.
                found=$(grep -oE '(^|[^0-9A-Za-z.^~<>=])3\.[0-9]+\.[0-9]+\b' "$f" 2>/dev/null \
                    | grep -oE '3\.[0-9]+\.[0-9]+' \
                    | grep -v "^${manifest_version}$" | sort -u | tr '\n' ' ')
                [ -n "$found" ] && printf '         %s -> %s\n' "$f" "$found"
            done
    )

    if [ -z "$stale" ]; then
        ok "no tracked doc names a stale 3.x version"
    else
        warn "these files name a 3.x version other than ${manifest_version}:"
        printf '%s\n' "$stale"
        warn "check each one — some are legitimately historical, others are drift"
    fi

    # ── Attributed version stamps, ENFORCED (not merely reported) ──────────
    #
    # The docs attribute a design basis as "... tree at <commit> (client 3.2.x)",
    # or assert a current version as "the current line is 3.2.x". The intended
    # version there is the x — the release the claim was written against, NOT the
    # fork's inherited 2.5.5 line — and it appears in five places across four
    # files. A bare-number grep cannot see drift in it, because `3.2.x` is not a
    # release version: if the client moves to 3.2.7 and a stamp still says
    # `3.2.5`, the statement is false and nothing reports it.
    #
    # Every such stamp in the tree is a deliberate, commit-attributed record, so
    # this is the one version claim that CAN be an equality check. It fails, and
    # names the line, so a stale stamp is fixed or re-attributed rather than
    # silently surviving the next bump.
    #
    # If you add a genuinely historical stamp, exempt it HERE by exact path — do
    # not widen the pattern, or this stops being able to fail.
    stamp_drift=$(
        cd "$REPO" && grep -rEn '\(client 3\.[0-9]+\.[0-9]+\)|current line is 3\.[0-9]+\.[0-9]+' \
            --include=*.md README.md docs client/docs 2>/dev/null \
            | grep -v "^docs/reference/FIXES\.md:\|^docs/reference/STILL-OPEN\.md:\|^docs/history/\|^docs/archive/" \
            | grep -v "3\.${manifest_version#3.}" \
            || true
    )

    if [ -z "$stamp_drift" ]; then
        ok "every commit-attributed version stamp matches ${manifest_version}"
    else
        bad "these documents stamp a design basis at a version that is no longer current."
        bad "Re-attribute the claim to the tree it was actually checked against, or update it:"
        printf '%s\n' "$stamp_drift" | sed 's/^/         /'
    fi
fi

# ─────────────────────────────────────────────────────────────
# 8. Documentation cross-references resolve.
#
# WHY THIS EXISTS: the business docs navigate by cross-reference —
# `19-cross-reference.md` is nothing but links — so a dead anchor silently
# removes a reader's route to the detail. The docs use `file.md#NN` to mean
# "§N.N" (`04-tiers.md#44` = section 4.4), and Markdown has no idea what that
# is: the real anchor for "## 4.4 The free tier, in full" is
# `#44-the-free-tier-in-full`. 71 such links were dead before this check.
#
# The rule: a link's target file must exist, and if it carries a fragment the
# fragment must match a real heading's slug.
# ─────────────────────────────────────────────────────────────
echo
echo "8. Documentation cross-references"

doc_links=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys

def slug(h):
    h = h.strip().lower()
    h = re.sub(r'[^a-z0-9 \-_]', '', h)
    return h.replace(' ', '-')

def md_files(root):
    for dirpath, _dirs, files in os.walk(root):
        # Never walk generated or dependency trees.
        if any(part in dirpath for part in ('node_modules', 'target', '.git', 'dist')):
            continue
        for fn in files:
            if fn.endswith('.md'):
                yield os.path.join(dirpath, fn)

docs = [p for p in md_files('docs')] + \
       [p for p in md_files('client/docs')] + ['README.md']
docs = [p for p in docs if os.path.exists(p)]

anchors = {}
for p in docs:
    s = set()
    with open(p, encoding='utf-8', errors='replace') as fh:
        for line in fh:
            m = re.match(r'^#{1,6}\s+(.*)$', line.strip())
            if m:
                s.add(slug(m.group(1)))
    anchors[os.path.normpath(p)] = s

bad = []
for p in docs:
    base = os.path.dirname(p)
    with open(p, encoding='utf-8', errors='replace') as fh:
        text = fh.read()
    for m in re.finditer(r'\]\(([^)\s]+?\.md)(#([^)\s]+))?\)', text):
        target = os.path.normpath(os.path.join(base, m.group(1)))
        if not os.path.exists(target):
            bad.append(f"{p}: missing target {m.group(1)}")
            continue
        frag = m.group(3)
        if frag and target in anchors and frag not in anchors[target]:
            bad.append(f"{p}: {m.group(1)}#{frag} matches no heading")

# Plain-text references: a doc or code comment that names a doc path the way a
# reader would navigate to it (e.g. "see docs/DEPLOY.md"), but by a path that
# no longer exists. These are NOT markdown links, so the check above cannot see
# them; the 2026-09 reorg moved ten docs into operate/ and reference/ and this
# is the class of reference that silently rots when a path changes.
#
# We only flag a reference that carries an explicit `docs/` prefix AND names a
# `.md` file that does not exist there — that is precise enough not to fire on
# prose ("FIXES.md is a dated log") or on historical citations.
#
# Scoped to the basenames the 2026-09 reorg actually moved, so it flags exactly
# the reorg-rot class and does not cry wolf on incidental mentions or files that
# never existed in docs/ (private research notes, the client's own docs).
MOVED_BASENAMES = (
    'DEPLOY.md', 'OPS.md', 'POCKETBASE-SETUP.md', 'SECRETS-MANAGEMENT.md',
    'UPDATE-SYSTEM.md', 'RELEASING.md', 'CI-CD.md', 'API.md', 'FIXES.md',
    'STILL-OPEN.md', 'ARCHITECTURE.md', 'ENGINE-SWAP-ANALYSIS.md',
    'GAMING-UDP.md', 'CONTEXT.md',
)
STALE_PATHS = re.compile(r'(?<![A-Za-z0-9/._-])docs/([A-Za-z0-9._/-]+\.md)')
for p in docs:
    # Historical logs cite old paths on purpose — they record what happened.
    if p.startswith('docs/reference/FIXES.md') or p.startswith('docs/history/') \
            or p.startswith('docs/archive/'):
        continue
    with open(p, encoding='utf-8', errors='replace') as fh:
        for lineno, line in enumerate(fh, 1):
            for m in STALE_PATHS.finditer(line):
                rel = m.group(1)
                if os.path.basename(rel) not in MOVED_BASENAMES:
                    continue
                if os.path.exists(os.path.join('docs', rel)):
                    continue
                # A reference into another tree (client/docs/…) is written with
                # its own prefix and will not match `docs/…` here.
                bad.append(f"{p}:{lineno}: stale path docs/{rel} (no such file)")

for b in sorted(set(bad)):
    print(b)
PY
)

if [ -z "$doc_links" ]; then
    ok "every documentation link and anchor resolves"
else
    bad "dead documentation link(s) — a reader following these finds nothing:"
    printf '%s\n' "$doc_links" | sed 's/^/         /'
fi

# ─────────────────────────────────────────────────────────────
# 9. docs/state.toml — the derived facts still agree with the tree.
#
# WHY THIS EXISTS
#
# docs/STATE.md used to hold a table of "volatile facts" — a version, an IP, host
# specs, a HEAD commit — and call itself the single source of truth for facts
# that change. It was wrong in a way worth stating precisely, because it is the
# second-most-common defect in this repo after cross-language drift:
#
#   The IP in that table went stale THREE times and finished by naming an address
#   that did not exist. Nothing failed. Nothing noticed. A document asserting a
#   travelling value in the present tense is indistinguishable, to a reader, from
#   a document asserting a stable one.
#
# The fix is structural, not editorial: the facts moved into docs/state.toml
# where each one declares a `source` and a `provenance`, and this check recomputes
# every `provenance = "derived"` fact against the tree. A fact that cannot be
# recomputed cannot be `derived`, and anything marked `unverified` is reported as
# a world claim rather than silently trusted.
#
# WHAT IT ENFORCES: the value of each derived fact.
# WHAT IT REPORTS: asserted facts (true by decision) and unverified ones (world
# claims we could not check) — loudly, but without failing the build, because a
# world claim being unverified is the honest state, not a defect. Failing on it
# would make the honest choice the expensive one, and the next person would just
# delete the marker.
# ─────────────────────────────────────────────────────────────
echo
echo "9. docs/state.toml — derived facts agree with the tree"

if [ ! -f "$REPO/docs/state.toml" ]; then
    bad "docs/state.toml is missing — the derived facts have no home"
else
    # Parsed with tomllib (Python 3.11+) rather than grepped: the file is nested
    # TOML, and a regex reader would silently mis-read it the first time someone
    # reformats. If Python cannot parse it, that is itself the finding.
    state_report=$(cd "$REPO" && python3 - <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    print("PARSE_FAIL\tpython3 has no tomllib (needs 3.11+)")
    sys.exit(0)

try:
    with open("docs/state.toml", "rb") as fh:
        data = tomllib.load(fh)
except Exception as exc:
    print(f"PARSE_FAIL\t{exc}")
    sys.exit(0)

# Flatten to dotted paths, keeping only fact subtables (those with a `value`).
facts = {}
for section, body in data.items():
    if not isinstance(body, dict):
        continue
    for name, entry in body.items():
        if isinstance(entry, dict) and "value" in entry:
            facts[f"{section}.{name}"] = entry

# ── Recompute every derived fact from the tree. ──
def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return ""

import json, re

def manifest_version():
    try:
        with open("client/package.json", encoding="utf-8") as fh:
            return json.load(fh).get("version")
    except Exception:
        return None

def setup_domain():
    # setup.sh requires DOMAIN from the environment; the deployed value lives in
    # setup.sh's own invocation. We look for the literal the repo actually uses.
    m = re.search(r'DOMAIN="?([a-z0-9.-]+\.[a-z]{2,})"?', read("server/setup.sh"))
    if m:
        return m.group(1)
    # Fall back to any duckdns hostname in the file (the required-env error text).
    m = re.search(r'([a-z0-9-]+\.duckdns\.org)', read("server/setup.sh"))
    return m.group(1) if m else None

def publish_api_base():
    m = re.search(r'PB_API:-(\S+?)\}', read("server/scripts/publish-release.sh"))
    return m.group(1) if m else None

def setup_log_path(marker):
    """Pull the console/UI path out of setup.sh's own success log lines.

    setup.sh prints `Admin console: https://${DOMAIN}/admin/` and
    `PocketBase UI: https://${DOMAIN}/_/`. Those lines are the deploy's own
    statement of where it put the files, so they are the right source — not a
    constant we would have to keep in two places.
    """
    m = re.search(re.escape(marker) + r':\s*https://\$\{DOMAIN\}(/[^\s"\']*)', read("server/setup.sh"))
    return m.group(1) if m else None

def platform_keys():
    """The four platform keys, from the Rust contract — the frozen definition."""
    found = re.findall(r'pub const PLATFORM_[A-Z_]+: &str = "([a-z_]+)";',
                       read("client/src-tauri/src/locus/contract.rs"))
    return found or None

def licence_client():
    """The client/ licence, as asserted by the root LICENSE scope table.

    Derived from the LICENSE table rather than from either manifest, because the
    manifests are what drift. §11 then checks the manifests against the same row,
    so all three are bound to one authority.
    """
    m = re.search(r'^\|\s*`client/`\s*\|\s*\**`?([A-Za-z0-9.\-]+)`?\**\s*\|',
                  read("LICENSE"), re.MULTILINE)
    return m.group(1) if m else None

# Map fact -> callable returning the recomputed value, or None if not checkable.
CHECKERS = {
    "client.version":       manifest_version,
    "hub.domain":           setup_domain,
    "hub.api_base":         publish_api_base,
    "hub.console_path":     lambda: setup_log_path("Admin console"),
    "hub.pocketbase_ui":    lambda: setup_log_path("PocketBase UI"),
    "platforms.keys":       platform_keys,
    "licence.client":       licence_client,
}

for path, entry in sorted(facts.items()):
    prov = entry.get("provenance", "")
    value = entry.get("value")
    source = entry.get("source", "")

    if not source:
        print(f"BAD\t{path}\tdeclares no `source`, so it cannot be checked")
        continue

    if prov == "derived":
        checker = CHECKERS.get(path)
        if checker is None:
            # A derived fact with no checker is worse than an unverified one: it
            # looks verified and is not. This is how `STATE.md` felt to a reader.
            print(f"BAD\t{path}\tmarked `derived` with no checker in this script — "
                  f"add one, or mark it `asserted`/`unverified`")
            continue
        actual = checker()
        if actual is None:
            print(f"BAD\t{path}\tcould not recompute (source: {source})")
        elif str(actual) != str(value):
            print(f"BAD\t{path}\tstate says {value!r}, tree says {actual!r} (source: {source})")
        else:
            print(f"OK\t{path}\t{value!r} agrees")
    elif prov == "asserted":
        print(f"NOTE\t{path}\tasserted by decision: {value!r} ({source})")
    elif prov == "unverified":
        print(f"WARN\t{path}\tUNVERIFIED world claim: {value!r} — see docs/operate/CLAIMS.md")
    else:
        print(f"BAD\t{path}\thas no valid `provenance` (got {prov!r})")
PY
)

    if [ -z "$state_report" ]; then
        bad "could not read docs/state.toml"
    else
        printf '%s\n' "$state_report" | while IFS="$(printf '\t')" read -r level detail rest; do
            case "$level" in
                OK)   ok   "$detail — $rest" ;;
                NOTE) warn "  $detail — $rest" ;;
                WARN) warn "$detail" ;;
                BAD)  bad  "$detail" ;;
            esac
        done
        # The while-above runs in a subshell, so FAIL set inside it would be lost.
        # Re-derive the failure condition here rather than rely on it.
        if printf '%s\n' "$state_report" | grep -q '^BAD'; then
            FAIL=1
        fi
        derived=$(printf '%s\n' "$state_report" | grep -c '^OK' || true)
        unver=$(printf '%s\n' "$state_report" | grep -c '^WARN' || true)
        ok "state.toml: ${derived} derived fact(s) agree with the tree${unver:+, ${unver} unverified world claim(s) reported}"
    fi
fi

# ─────────────────────────────────────────────────────────────
# 10. No volatile fact is written into live prose.
#
# WHY THIS EXISTS
#
# Section 7 above already enforces this for the VERSION: a live document may not
# inline a version number that disagrees with the manifests. This is the same
# rule generalised, because the same error was made for other values and nothing
# caught it:
#
#   * An IP address, written as "Live hub host" in the file others were told to
#     trust. It went stale three times and finished by naming a server that no
#     longer existed.
#   * A count of live activation codes with a distributor's name attached. Wrong
#     the next time a code was sold, and customer-identifying in a product whose
#     central design decision is to store no identity.
#
# Both are CLASS A world claims (docs/operate/CLAIMS.md). The rule this enforces
# is the one that already worked for versions and generalises cleanly:
#
#   YOU CANNOT HAVE DRIFTED FROM A VALUE YOU NEVER WROTE DOWN.
#
# So a live document must not contain one at all — it links to state.toml
# (derived) or CLAIMS.md §5 (world). This is a grep, deliberately: it does not try
# to understand the sentence, only to notice an unowned value.
#
# WHAT IT DOES NOT DO: it cannot tell that "the hub is online" in prose is a claim
# about the world. That class is covered by CLAIMS.md's rules and the §9 report,
# not by a pattern — a regex broad enough to catch it would fire on correct
# sentences, and a guard that cries wolf is worse than no guard (§7 learned this).
#
# EXCLUSIONS ARE EXACT PATHS, and each is justified. If you need to add one, add
# the path here with a reason — do not widen the pattern, or this stops being able
# to fail.
# ─────────────────────────────────────────────────────────────
echo
echo "10. No volatile fact (IP address / hub URL) inlined in live documentation"

# Live documentation = everything except the three archives-of-record.
#   docs/reference/FIXES.md        chronological log: cites the addresses that
#                                  were live when an entry was written, on purpose
#   docs/history/, docs/archive/   explicitly retired material
#   docs/operate/CLAIMS.md         the ONE place a world claim may be described
#                                  (its §5 register and §1 worked example)
#   docs/state.toml                the derived-facts data file itself
DOCS_TO_SCAN=$(cd "$REPO" && find docs client/docs -name '*.md' 2>/dev/null \
    | grep -vE '^docs/reference/FIXES\.md$|^docs/history/|^docs/archive/|^docs/operate/CLAIMS\.md$' \
    | sort)

# An IPv4 literal. Deliberately does NOT flag addresses that are provably NOT the
# hub, because those are legitimate and flagging them would train the reader to
# ignore this check (§7's lesson). The exclusions are closed sets, each with a
# reason — an address that does not match one of them is a finding:
#
#   127.0.0.0/8      loopback
#   0.0.0.0          "unspecified", used in bind examples
#   255.255.255.255  broadcast
#   8.8.8.8 / 8.8.4.4 / 1.1.1.1 / 1.0.0.1   public DNS resolvers, cited in the UoT
#                    and DoH verification steps as the thing being resolved. They
#                    are somebody else's stable addresses, not our travelling one.
#   203.0.113.0/24   RFC 5737 documentation range — reserved for examples,
#                    guaranteed never routable. Its presence is proof the author
#                    meant an example.
#   192.0.2.0/24     RFC 5737 documentation range (same reason)
#   198.51.100.0/24  RFC 5737 documentation range (same reason)
#   77.88.8.8        Yandex DNS, used as a second resolver in one probe
#
# If you need one more, ADD IT HERE WITH A REASON, in this list. Do not widen the
# pattern to a class — the next hub address to go stale would hide inside it.
ALLOWED_IPS='127\.|0\.0\.0\.0$|255\.255\.255\.255$|^8\.8\.8\.8$|^8\.8\.4\.4$|^1\.1\.1\.1$|^1\.0\.0\.1$|^203\.0\.113\.|^192\.0\.2\.|^198\.51\.100\.|^77\.88\.8\.8$'
IP_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
    [ -n "$f" ] || continue
    grep -nEo '\b([0-9]{1,3}\.){3}[0-9]{1,3}\b' "$f" 2>/dev/null \
        | while IFS=: read -r lineno ip; do
            printf '%s\n' "$ip" | grep -qE "($ALLOWED_IPS)" && continue
            echo "$f:$lineno:$ip"
        done
done)

if [ -z "$IP_HITS" ]; then
    ok "no live document inlines an IP address"
else
    bad "these live documents inline an IP address — it is a world claim, not a fact:"
    printf '%s\n' "$IP_HITS" | sed 's/^/         /'
    bad "Remove it. Address the hub by its domain (docs/state.toml) or record the"
    bad "claim, dated, in docs/operate/CLAIMS.md §5. See CLAIMS.md §1 and §6."
fi

# A hub URL inlined in prose. The canonical value is in state.toml and CLAIMS.md;
# a live doc should reference it rather than restate it. Command examples in the
# operate guides are the grey area, and they are EXCLUDED by path below because a
# runbook the operator copies must contain a real URL — that is its function.
URL_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
    [ -n "$f" ] || continue
    # Exclude the operator runbooks, whose whole job is copy-pasteable commands.
    case "$f" in
        docs/operate/OPS.md|docs/operate/POCKETBASE-SETUP.md|docs/operate/RELEASING.md|docs/operate/DEPLOY.md|docs/operate/SECRETS-MANAGEMENT.md|docs/operate/DEPLOY-3.2.7.md|docs/operate/UPDATE-SYSTEM.md|docs/reference/API.md)
            continue ;;
    esac
    grep -nE 'https?://[a-z0-9.-]*duckdns\.org' "$f" 2>/dev/null | sed "s|^|$f:|"
done)

if [ -z "$URL_HITS" ]; then
    ok "no live document inlines the hub URL outside the operator runbooks"
else
    warn "these live documents inline the hub URL (allowed, but link state.toml where possible):"
    printf '%s\n' "$URL_HITS" | sed 's/^/         /'
fi

# A count of live codes / customers. There is no legitimate reason for this to
# appear in the tree at all — see docs/operate/CLAIMS.md §4.
COUNT_HITS=$(cd "$REPO" && printf '%s\n' "$DOCS_TO_SCAN" | while read -r f; do
    [ -n "$f" ] || continue
    grep -nE '\b[0-9]+ (real|live|active|paying) (strike |activation )?(codes?|customers?|users?|subscribers?)\b' "$f" 2>/dev/null \
        | sed "s|^|$f:|"
done)

if [ -z "$COUNT_HITS" ]; then
    ok "no live document states a count of live codes or customers"
else
    bad "these live documents state a live headcount — it decays and it identifies a customer:"
    printf '%s\n' "$COUNT_HITS" | sed 's/^/         /'
    bad "Delete the number, keep the rule. See docs/operate/CLAIMS.md §4."
fi

# ─────────────────────────────────────────────────────────────
# 11. The licence identifier agrees across the three places it is written.
#
# WHY THIS EXISTS
#
# The root LICENSE is CC BY-NC-ND 4.0 with an explicit scope carve-out: `client/`
# is NOT covered by it, because client/ is a fork of Clash Verge Rev and a GPL
# fork cannot be relicensed. That carve-out is a legal statement, and it is
# duplicated as a machine-readable SPDX id in two manifests. Nothing bound them.
#
# It drifted immediately. The commit that ADDED the carve-out also stamped
# `"license": "CC-BY-NC-ND-4.0"` onto client/package.json in the same change —
# asserting the exact thing the carve-out excludes — and left
# client/src-tauri/Cargo.toml at the earlier `UNLICENSED`. Three files, three
# answers, no mechanism. This is the same defect class as §1 and §4, on the one
# field where being wrong is a licensing violation rather than a silent no-op.
#
# WHAT IT ENFORCES: the `client/` row of the root LICENSE scope table is the
# authority. Both client manifests must carry that exact SPDX id.
# ─────────────────────────────────────────────────────────────
echo
echo "11. Licence identifier — root carve-out vs client manifests"

LICENSE_FILE="$REPO/LICENSE"
PKG_JSON="$REPO/client/package.json"
CARGO_TOML="$REPO/client/src-tauri/Cargo.toml"

if [ ! -f "$LICENSE_FILE" ]; then
    bad "LICENSE is missing — there is no authority for the carve-out"
else
    # The scope table row is:  | `client/` | **GPL-3.0-only** | ... |
    # Read it rather than hardcoding, so changing the licence is a one-file edit
    # and this check follows. A strikethrough/italic marker is tolerated.
    EXPECTED_LIC=$(grep -E '^\|[[:space:]]*`client/`' "$LICENSE_FILE" \
        | head -1 \
        | awk -F'|' '{print $3}' \
        | sed -e 's/\*//g' -e 's/`//g' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')

    if [ -z "$EXPECTED_LIC" ]; then
        bad "LICENSE has no \`client/\` row in its scope table — the carve-out is gone"
        bad "expected a line like: | \`client/\` | **GPL-3.0-only** | ... |"
    else
        # package.json — read the JSON rather than grepping, so a reformat does
        # not defeat it. Parse failure is itself the finding.
        PKG_LIC=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("license",""))' "$PKG_JSON" 2>/dev/null)
        if [ -z "$PKG_LIC" ]; then
            bad "could not read a license field from client/package.json"
        elif [ "$PKG_LIC" = "$EXPECTED_LIC" ]; then
            ok "client/package.json — '${PKG_LIC}' agrees with the root carve-out"
        else
            bad "client/package.json says '${PKG_LIC}' but the root LICENSE carves client/ out as '${EXPECTED_LIC}'"
        fi

        # Cargo.toml — `license = "..."` in [package]. Only the first such line.
        CARGO_LIC=$(grep -m1 -E '^license[[:space:]]*=' "$CARGO_TOML" \
            | sed -e 's/^license[[:space:]]*=[[:space:]]*//' -e 's/^"//' -e 's/"$//')
        if [ -z "$CARGO_LIC" ]; then
            bad "could not read a license field from client/src-tauri/Cargo.toml"
        elif [ "$CARGO_LIC" = "$EXPECTED_LIC" ]; then
            ok "client/src-tauri/Cargo.toml — '${CARGO_LIC}' agrees with the root carve-out"
        else
            bad "client/src-tauri/Cargo.toml says '${CARGO_LIC}' but the root LICENSE carves client/ out as '${EXPECTED_LIC}'"
        fi

        # client/LICENSE must actually BE that licence — a fork whose bundled
        # text disagrees with its declared SPDX id is the same defect one level
        # down, and that file is the one a downstream reader opens.
        if [ -f "$REPO/client/LICENSE" ]; then
            case "$EXPECTED_LIC" in
                GPL-3.0-only|GPL-3.0)
                    if grep -q 'GNU GENERAL PUBLIC LICENSE' "$REPO/client/LICENSE" \
                       && grep -q 'Version 3' "$REPO/client/LICENSE"; then
                        ok "client/LICENSE contains the GPL-3 text it is declared to be"
                    else
                        bad "client/LICENSE is not the GPL-3 text, but is declared '${EXPECTED_LIC}'"
                    fi ;;
                *)
                    # Other licences are not text-matched here; only the two
                    # manifests are enforced. Reported so the gap is visible.
                    warn "client/LICENSE not text-checked against '${EXPECTED_LIC}' (only GPL-3 is)" ;;
            esac
        else
            bad "client/LICENSE is missing — the carve-out points at a file that is not there"
        fi
    fi
fi

# 12. The update install path must not reintroduce the buffering downloader.
#
# WHY THIS EXISTS
#
# The client's updater crashed on "Install Now": the progress spinner stayed at
# 0% and the app died with nothing in the log. The cause was that the install
# path used the Tauri plugin's `download_and_install`, which reads the ENTIRE
# artifact into a `Vec<u8>` in memory before verifying it — enough to abort the
# allocation on a school laptop, killing the process.
#
# The streaming, disk-hashing downloader that avoids this (`locus/update/apply.rs`)
# already existed and was DEAD CODE. The two modules were written from
# incompatible plans and only the unsafe one got wired up. Nothing noticed,
# because nothing checked.
#
# This guard is what notices. It fails the build if the live install path calls
# the plugin's buffering downloader again, or if the streaming downloader loses
# its only production caller.
echo
echo "12. Update install path uses the streaming downloader"

INSTALL_RS="$REPO/client/src-tauri/src/locus/update/install.rs"
APPLY_RS="$REPO/client/src-tauri/src/locus/update/apply.rs"
CMD_RS="$REPO/client/src-tauri/src/cmd/locus.rs"

if [ ! -f "$INSTALL_RS" ] || [ ! -f "$APPLY_RS" ] || [ ! -f "$CMD_RS" ]; then
    bad "the update path files are missing — this guard cannot run"
else
    # (a) The buffering call must not be present in the install module at all.
    #     Matched on the call, not the name, so a doc comment explaining why it
    #     is avoided does not trip it.
    if grep -qE '\.download_and_install[[:space:]]*\(' "$INSTALL_RS" "$CMD_RS"; then
        bad "the install path calls the plugin's download_and_install, which buffers the whole artifact in memory"
        bad "  use PendingInstall::download_to_file + ReadyInstall::install instead (see client/src-tauri/src/locus/update/install.rs)"
    else
        ok "no call to the memory-buffering download_and_install in the install path"
    fi

    # (b) The streaming downloader must have a production caller. `pub mod apply`
    #     plus a re-export is not a caller — that is exactly the dead-code state
    #     that caused this bug.
    if grep -qE 'apply::download[[:space:]]*\(' "$INSTALL_RS"; then
        ok "the streaming downloader apply::download is called by the install path"
    else
        bad "apply::download is not called by the install path — the streaming downloader is dead code again"
        bad "  this is the state that produced the 0%-then-crash updater bug"
    fi

    # (c) The install command must log BEFORE it can block. The original bug was
    #     undiagnosable precisely because the path was silent until it failed.
    if grep -q 'update install requested' "$CMD_RS"; then
        ok "the install command logs at the start of the attempt"
    else
        bad "locus_install_update has no start-of-attempt log line — a crash mid-download would again leave nothing behind"
    fi

    # (d) The hub's checksum must be read and enforced. Installing without a
    #     hash is installing unverified bytes.
    if grep -qE 'sha256_from_manifest|\.sha256\(\)' "$INSTALL_RS" \
       && grep -q 'LOCUS_UPDATE_NO_CHECKSUM' "$CMD_RS"; then
        ok "the install path reads the hub checksum and refuses when it is absent"
    else
        bad "the install path does not enforce the hub's sha256 — an unverified install would be possible"
    fi
fi

# ─────────────────────────────────────────────────────────────
# 13. No Windows path is interpolated raw into JSON that a build parses.
#
# WHY THIS EXISTS
#
# The Windows build died in the `tauri` build script with:
#
#     called `Result::unwrap()` on an `Err` value: invalid escape at line 1 column 54
#
# The cause was `TAURI_CONFIG` in `.github/workflows/client.yml`:
#
#     {"...","frontendDist":"${{ github.workspace }}/client/dist"}
#
# `TAURI_CONFIG` is JSON, parsed by `serde_json::from_str` in tauri-utils. On
# Windows `github.workspace` is `D:\a\Locus\Locus`, so the raw interpolation
# produced `"D:\a\Locus\..."` — where `\a` is an INVALID JSON ESCAPE (`\a` sits
# at exactly column 54 for this path). Linux and macOS have forward slashes and
# stayed green, so the failure existed only on the one runner the author could
# not see.
#
# This is the same shape as §5's ':' path bug: a Windows-only break that looks
# green on every other runner. The guard is a grep for the pattern, because the
# value interpolated is a runner path that does not exist in this checkout.
# ─────────────────────────────────────────────────────────────
echo
echo "13. No raw Windows-path interpolation into build JSON"

WORKFLOW="$REPO/.github/workflows/client.yml"
if [ ! -f "$WORKFLOW" ]; then
    warn "no .github/workflows/client.yml — this guard cannot run"
else
    # A JSON-looking line (has a "key":"value" shape) that interpolates
    # github.workspace directly. That is always wrong: the result carries
    # backslashes on Windows, which are not valid JSON escapes.
    json_path_hits=$(grep -nE '\$?\{\{?[[:space:]]*github\.workspace' "$WORKFLOW" \
        | grep -E '"' \
        || true)

    if [ -z "$json_path_hits" ]; then
        ok "no JSON line interpolates github.workspace directly"
    else
        bad "github.workspace is interpolated into a JSON-looking line — on Windows this is an invalid JSON escape"
        bad "  normalize to forward slashes and encode with a real JSON encoder (see the build job's TAURI_CONFIG)"
        printf '%s\n' "$json_path_hits" | head -5 | sed 's/^/         /'
    fi

    # The positive half: TAURI_CONFIG must still be exported somewhere, or the
    # build silently reverts to the interactive `beforeBuildCommand` rebuild.
    if grep -q 'TAURI_CONFIG' "$WORKFLOW"; then
        ok "TAURI_CONFIG is still constructed by the workflow"
    else
        warn "TAURI_CONFIG no longer appears in the workflow — if the frontend rebuild was reintroduced deliberately, ignore this"
    fi
fi

# ─────────────────────────────────────────────────────────────
# 14. Every file the Windows bundler config NAMES must be produced by CI.
#
# WHY THIS EXISTS
#
# `v3.2.9` was fully green and shipped a Windows installer no student could get
# past: the bundle BUILT (so all four build jobs passed) but the setup `.exe`
# aborted at install time with an NSIS "File not found" dialog. The cause was a
# bundler config naming a file nothing produced:
#
#   tauri.windows.conf.json
#     "installerHooks": "./packages/windows/core-hashes.nsh"
#     "template":       "./packages/windows/installer.nsi"
#
# `core-hashes.nsh` was generated by `scripts/prebuild.mjs`, and that ran inside
# `beforeBuildCommand` — which every CI build job BLANKS via `TAURI_CONFIG` (the
# frontend bundle is built once in `verify` and downloaded). So the file did not
# exist where the Windows bundle was built.
#
# This is §13's shape one level up: not a bad VALUE but a missing FILE, on the
# one platform whose output no step here executes. Both halves of the check
# matter — a config naming a file that is wrong, and CI disarming the step that
# creates it — and either half alone is silent.
#
# The guard is deliberately a grep, matching this script's style: it cannot
# prove an NSIS bundle compiles, but it fails loudly at the exact moment someone
# adds `installerHooks`/`template` back without a producer, which is the change
# that broke a release.
# ─────────────────────────────────────────────────────────────
echo
echo "14. Windows bundler config references only files CI produces"

WIN_CONF="$REPO/client/src-tauri/tauri.windows.conf.json"
if [ ! -f "$WIN_CONF" ]; then
    warn "no client/src-tauri/tauri.windows.conf.json — this guard cannot run"
else
    # The two keys that make the bundler read a path OUT of this tree. Anything
    # else in the file is data, not a file reference.
    refs=$(grep -oE '"(installerHooks|template)"[[:space:]]*:[[:space:]]*"[^"]+"' "$WIN_CONF" \
        | sed -E 's/.*"[^"]+"[[:space:]]*:[[:space:]]*"([^"]+)"/\1/' \
        || true)

    if [ -z "$refs" ]; then
        ok "tauri.windows.conf.json references no external bundler file"
    else
        for ref in $refs; do
            # Relative to client/src-tauri/, which is where tauri resolves it.
            path="$REPO/client/src-tauri/${ref#./}"
            if [ ! -f "$path" ]; then
                bad "tauri.windows.conf.json references '$ref', which is not in the tree"
                continue
            fi
            # Committed is not the same as produced: a generated file must not be
            # named unless something regenerates it on the runner that needs it.
            if git -C "$REPO" ls-files --error-unmatch "client/src-tauri/${ref#./}" >/dev/null 2>&1; then
                ok "'$ref' is committed and present"
            else
                bad "'$ref' is named by the bundler config but is GENERATED and NOT committed"
                bad "  a file generated inside beforeBuildCommand never exists in CI: the build jobs blank"
                bad "  that command via TAURI_CONFIG, so makensis aborts with '!include: could not open file'"
                bad "  produce it in an explicit prebuild step on the Windows runner, or stop naming it"
            fi
        done
    fi

    # The positive half. `beforeBuildCommand` is what CI blanks, so a bundler
    # reference to a generated file is only survivable if prebuild also runs as
    # its own workflow step. Without THIS step, the failure above returns the
    # moment anyone adds installerHooks back.
    if grep -q 'prebuild\.mjs' "$WORKFLOW" 2>/dev/null; then
        ok "a workflow step runs prebuild.mjs explicitly (not only via beforeBuildCommand)"
    else
        bad "no workflow step runs prebuild.mjs directly — the build jobs blank beforeBuildCommand,"
        bad "  so every winOnly generated file would be missing on the Windows runner"
    fi
fi

echo
echo "15. Every route the app will navigate to is a route the frontend defines"

WIN_DEFAULT="$REPO/client/src-tauri/src/utils/resolve/window.rs"
NAV_META="$REPO/client/src/pages/_navigation-meta.ts"
if [ ! -f "$WIN_DEFAULT" ] || [ ! -f "$NAV_META" ]; then
    warn "start-page guard cannot run (missing $WIN_DEFAULT or $NAV_META)"
else
    # The Rust-side list of routes the window will accept.
    served=$(sed -n 's/^const SERVED: &\[&str\] = &\[\(.*\)\];$/\1/p' "$WIN_DEFAULT" \
        | tr ',' '\n' | tr -d ' "' | grep -v '^$' | sort)
    # The routes the frontend actually defines.
    routes=$(sed -n "s/^[[:space:]]*path:[[:space:]]*'\(.*\)',*$/\1/p" "$NAV_META" | sort)

    if [ -z "$served" ]; then
        bad "no 'const SERVED' list found in window.rs — the start-page guard is gone"
    elif [ -z "$routes" ]; then
        bad "no routes found in _navigation-meta.ts — the extraction pattern has drifted"
    elif [ "$served" = "$routes" ]; then
        ok "window.rs accepts exactly the routes _navigation-meta.ts defines"
    else
        bad "window.rs SERVED and _navigation-meta.ts disagree — a start_page the window accepts"
        bad "  but the frontend does not define loads no bundled asset, and the webview reports"
        bad "  Chromium's ERR_FILE_NOT_FOUND in a window that never becomes visible"
        printf '         window.rs:    %s\n' "$(echo "$served" | tr '\n' ' ')"
        printf '         frontend:     %s\n' "$(echo "$routes" | tr '\n' ' ')"
    fi
fi

echo
echo "════════════════════════════════════════════════════════════"
if [ "$FAIL" -eq 0 ]; then
    echo -e "${GREEN}All consistency checks passed.${NC}"
else
    echo -e "${RED}Consistency FAILED — see BAD lines above.${NC}"
fi
echo "════════════════════════════════════════════════════════════"
exit "$FAIL"
