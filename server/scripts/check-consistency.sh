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

# ─────────────────────────────────────────────────────────────
# 1a. THE HUB MUST FETCH THE INSTALLER FOR WINDOWS, NOT THE RAW BINARY.
#
# This is the check whose absence shipped a broken Windows update. The rule it
# guards is the same one §15 guards for the workflow's manifest, but on the
# OTHER side of the contract — and the two sides drifted apart, which is what
# made the bug possible: CI correctly advertised
# `installer-Locus_<v>_x64-setup.exe` in manifest.json from v3.2.12 onward,
# while fetch-release.py and publish-release.sh still resolved
# `locus-windows-amd64.exe`. The hub therefore staged the raw PE into the
# Windows slot, hashed it, paired it with the RAW BINARY's own minisign
# signature, and served all three. Every Windows client refused it — correctly,
# via `is_installer_payload` — so no Windows client could update at all.
#
# The old assertion here ("fetch-release.py mentions 'windows'") passed before,
# during and after that bug: the string was present the whole time. A check that
# cannot tell a right answer from a wrong one is not a check, so this one pins
# the FILENAME.
if grep -qE 'installer-Locus_%s_x64-setup\.exe|INSTALLER_NAME_TEMPLATE' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    ok "fetch-release.py resolves the Windows installer by name"
else
    bad "fetch-release.py does not resolve 'installer-Locus_<version>_x64-setup.exe'"
    bad "  the hub would serve the raw 'locus-windows-amd64.exe' to Windows clients,"
    bad "  which the client's is_installer_payload refuses — no Windows client could update"
fi

# The stale name must be GONE. Checked separately from the positive above so
# that keeping both (e.g. a fallback) cannot slip through: a fallback to the raw
# binary is the bug, not a safety net.
if grep -qE '"windows"[^,]*locus-windows-amd64\.exe|locus-windows-amd64\.exe"' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    bad "fetch-release.py still names 'locus-windows-amd64.exe' as a fetchable asset"
    bad "  that is the raw updater payload; the Windows slot must be the NSIS installer"
else
    ok "fetch-release.py no longer names the raw Windows binary"
fi

# publish-release.sh writes the same update_config row from the CLI path, so it
# must agree. Two writers with two opinions is how the row and the manifest came
# to disagree in the first place.
if grep -qE 'WINDOWS_INSTALLER="installer-Locus_\$\{VERSION\}_x64-setup\.exe"' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
    ok "publish-release.sh resolves the Windows installer by name"
else
    bad "publish-release.sh does not resolve the Windows installer by name"
    bad "  the CLI publish path would write download_windows pointing at the raw binary"
fi

if grep -qE '"windows:locus-windows-amd64\.exe:update_windows"' "$SCRIPTS/publish-release.sh" 2>/dev/null; then
    bad "publish-release.sh still publishes the raw Windows binary as the update"
else
    ok "publish-release.sh no longer publishes the raw Windows binary"
fi

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
# 1c. The macOS HUMAN download — named the same in CI, the hub and the console.
#
# WHY THIS EXISTS
#
# The `.zip` is the one artifact that is deliberately NOT an update payload: it
# is the compressed, ad-hoc-signed `Locus.app` a PERSON downloads. The Windows
# outage of 2026-10-01 was a filename that had to agree across three systems and
# was checked on only one of them, so this name gets the same treatment from the
# start rather than after it drifts.
#
# Three sites must agree:
#
#   1. CI       — `.github/workflows/client.yml`, "Package the macOS app for
#                 download", which BUILDS `Locus_<v>_<arch>.zip` and the release
#                 job which prefixes it to `installer-...`
#   2. the hub  — `fetch-release.py`'s `MACOS_ZIP_TEMPLATE`, which RESOLVES it
#                 from the GitHub Release
#   3. console  — `Releases.vue`'s `MACOS_ARCHES`, which LINKS to it
#
# And one NEGATIVE, which is the property that actually protects the fleet: the
# name must NOT appear in the update path (`PLATFORMS`,
# `resolve_platform_names`, CI's manifest platform map). A compressed bundle in
# an update slot is the raw-Windows-PE mistake in a new costume.
# ─────────────────────────────────────────────────────────────
echo
echo "1c. macOS human download — one name, three sites, and never an update payload"

# `WORKFLOW` is assigned again further down for the sections that also use it.
# It is set here as well because this section runs FIRST and would otherwise
# abort on an unbound variable under `set -u` — which is how this check failed
# the first time it ran, before it had checked anything at all.
WORKFLOW="$REPO/.github/workflows/client.yml"

# The template must exist, in exactly one place per side.
if grep -qE '^MACOS_ZIP_TEMPLATE *= *"installer-Locus_%s_%s\.zip"' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    ok "fetch-release.py defines the macOS zip template once"
else
    bad "fetch-release.py has no single MACOS_ZIP_TEMPLATE definition"
    bad '  expected: MACOS_ZIP_TEMPLATE = "installer-Locus_%s_%s.zip"'
fi

# The arch suffixes are a contract too: CI writes `_amd64`/`_arm64`, and the hub
# resolves the same two. A third arch added on one side only would be fetched
# forever by nobody.
for arch in amd64 arm64; do
    if grep -q "\"$arch\"" "$SCRIPTS/fetch-release.py" 2>/dev/null; then
        ok "the hub knows the '$arch' macOS zip"
    else
        bad "fetch-release.py does not list '$arch' in MACOS_ZIP_ARCHES"
    fi
done

# CI must produce both, and the release job must assert them present. The
# assertion is what stops a build job that silently stopped packaging from
# publishing a release with no macOS download at all.
if grep -qE 'Locus_\$\{ver\}_\$\{arch\}\.zip|Locus_\$\{ver\}_' "$WORKFLOW" 2>/dev/null; then
    ok "CI builds the macOS zip under the versioned name"
else
    bad "no CI step builds 'Locus_<v>_<arch>.zip'"
    bad "  the macOS packaging step is what makes the bypassable Gatekeeper path exist"
fi
if grep -q 'expected two macOS download zips' "$WORKFLOW" 2>/dev/null; then
    ok "the release job refuses to publish without both macOS zips"
else
    bad "the release job does not assert the macOS zips are present"
    bad "  a packaging step that stopped running would publish a release with none"
fi

# The console must name the same shape, or the link 404s for the student.
CONSOLE_VUE="$REPO/server/console/src/views/Releases.vue"
if [ -f "$CONSOLE_VUE" ]; then
    if grep -q 'installer-Locus_${v}_${a.arch}.zip' "$CONSOLE_VUE" 2>/dev/null; then
        ok "the console builds the same zip filename"
    else
        bad "the console does not construct 'installer-Locus_<v>_<arch>.zip'"
        bad "  its download link would point at a file the hub did not fetch"
    fi
    # The console must NOT enumerate the zip among the update platforms, or the
    # "all four platforms published" gate would start counting a human download.
    if grep -qE "zip_macos_amd64', *filename: *'locus-" "$CONSOLE_VUE" 2>/dev/null; then
        bad "the console treats the macOS zip as an update platform"
    else
        ok "the console keeps the macOS zip out of the platform list"
    fi
else
    warn "console Releases.vue not found; skipping the console half of this check"
fi

# ── The negative, and the reason this section exists ──
#
# Everything above is about a name agreeing. This is about the name being in the
# WRONG PLACE, which is the failure that cost three releases on Windows. Checked
# on both sides of the hub, because either alone would be defeated by the other
# being edited.
if grep -qE '^\s*\("macos_(intel|arm).*\.zip' "$SCRIPTS/fetch-release.py" 2>/dev/null; then
    bad "fetch-release.py lists a .zip inside PLATFORMS — the update path"
    bad "  a client would be handed an archive it cannot install from"
else
    ok "PLATFORMS in fetch-release.py carries no .zip"
fi

if grep -qE '"macos_(intel|arm)"[[:space:]]*:[[:space:]]*"[^"]*\.zip"' "$WORKFLOW" 2>/dev/null; then
    bad "CI's manifest platform map advertises a .zip for a macOS platform"
else
    ok "CI's manifest advertises no .zip as an update payload"
fi
if grep -q 'refusing to publish: the manifest names a .zip' "$WORKFLOW" 2>/dev/null; then
    ok "the manifest step actively refuses a .zip platform entry"
else
    bad "the manifest step does not guard against a .zip platform entry"
fi
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

# (e) RETIRED: the device-binding normalisation check.
#
# This used to assert that any hook writing a fingerprint into `device_bindings`
# or `codes.bound_fingerprint` normalised it first, because a raw/stripped
# mismatch made the "one code per device" uniqueness check silently stop
# matching. Both the binding and the index are gone (codes are single-use and
# not device-bound — see activation.pb.js), so the pattern it grepped for no
# longer exists anywhere in the tree.
#
# It is removed rather than left in place on purpose: a guard whose pattern can
# never match reads exactly like a guard that passed, which is the failure mode
# this project has paid for more than once (§(a) of DEBUGGING-METHOD.md). The
# replacement below checks the invariant that exists now.

# (e2) A code is single-use, and `activated_at` is the whole of that rule.
#
# WHY THIS IS A CHECK. The rule is easy to break by accident in a hook that
# "helpfully" clears the stamp — an operator release legitimately does, but an
# expiry or a suspension must NOT, because clearing `activated_at` would make a
# code the student is using look unused and available for a second sale.
#
# So: only the release paths may clear `activated_at`. Every other hook is
# refused. `admin_unbind.pb.js` and the console's codes.unbind are the two
# places an operator deliberately releases a code.
stray_clear=$(grep -rnE 'set\("activated_at",\s*(null|"")\s*\)' "$HOOKS_DIR"/*.js 2>/dev/null \
    | grep -v 'admin_unbind\.pb\.js' \
    | grep -v 'admin_console\.pb\.js' || true)
if [ -z "$stray_clear" ]; then
    ok "only the release paths clear the single-use stamp"
else
    bad "a hook outside the release paths clears codes.activated_at — that makes a"
    bad "code a student is using look unused, and it can then be sold twice:"
    printf '%s\n' "$stray_clear" | sed 's/^/         /'
fi

# (e3) No hook may read or write the retired device-binding fields on a code.
#
# `bound_fingerprint` is a frozen column on an existing hub (kept so an operator
# can still inspect history) but nothing may go on using it, or the retired
# device model would quietly creep back into the activation path.
used_binding=$(grep -rn 'bound_fingerprint' "$HOOKS_DIR"/*.js 2>/dev/null \
    | grep -v '^\s*//' \
    | grep -vE ':\s*(//|\*)' || true)
if [ -z "$used_binding" ]; then
    ok "no hook uses the retired bound_fingerprint field"
else
    bad "a hook still reads or writes codes.bound_fingerprint:"
    printf '%s\n' "$used_binding" | sed 's/^/         /'
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
    #
    # TRIAGED 2026-10-04. This list was a permanent 14-file WARN wall, and a
    # warning nobody can action trains the reader to ignore it (§7's own lesson,
    # and the reason §8 shrank its own report). Each file below was read, and the
    # ones whose older number is CORRECT are excluded HERE, by exact path, with
    # the reason — the same "exclude precisely, never widen the pattern" rule the
    # stamp check and §10 use. What remains is genuine drift and is reported.
    #
    # Excluded because the number is correct AS WRITTEN:
    #   docs/CONTEXT.md              "first shipped 3.0.0" — history, and the
    #                                trailing "now on the 3.x line" is current
    #   docs/reference/THEMES.md     "what changed in 3.3.0" — the milestone the
    #                                theme layer landed in, cited as a version
    #   docs/reference/DEBUGGING-METHOD.md, docs/operate/CLAIMS.md,
    #   docs/operate/DEPLOY-3.2.7.md, docs/operate/UPDATE-SYSTEM.md,
    #   docs/operate/RELEASING.md, docs/operate/RECOVER-*.md
    #                                dated incident/example/basis citations
    #   docs/business/1*-*.md, docs/business/redesign/**
    #                                a design basis ("tree at <commit>, client
    #                                3.2.x") — the number records what a claim was
    #                                written against, and must NOT be bumped
    #   client/docs/UPSTREAM-CHANGES.md  "the 3.1.0 rework" — history
    #
    # If you add a file here, add the reason. A path with no reason is the next
    # reader's guess, and this list only stays honest if it is specific.
    TRIAGED_BASIS='^(docs/CONTEXT\.md|docs/reference/THEMES\.md|docs/reference/DEBUGGING-METHOD\.md|docs/operate/CLAIMS\.md|docs/operate/DEPLOY-3\.2\.7\.md|docs/operate/UPDATE-SYSTEM\.md|docs/operate/RELEASING\.md|docs/operate/RECOVER-WINDOWS-UPDATE\.md|docs/operate/RECOVER-MACOS-UPDATE\.md|docs/business/1[0-9]-.*\.md|docs/business/redesign/.*\.md|client/docs/UPSTREAM-CHANGES\.md)$'
    stale=$(
        cd "$REPO" && grep -rEl --include=*.md '3\.[0-9]+\.[0-9]+' \
            README.md docs client/docs 2>/dev/null \
            | grep -vE '^(docs/reference/FIXES\.md|docs/reference/STILL-OPEN\.md|docs/history/.*|docs/archive/.*|client/docs/FRONTEND\.md)$' \
            | grep -vE "$TRIAGED_BASIS" \
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

def landing_domain():
    """The landing page hostname, from the Caddyfile generator.

    modules/05-caddy.sh is what actually writes the site block, so its
    LANDING_DOMAIN default is the source. The template and the page's canonical
    link are checked for AGREEMENT with this in section 26 — this function only
    answers "what does the deployed config say".
    """
    m = re.search(r'LANDING_DOMAIN="\$\{LANDING_DOMAIN:-([^}]+)\}"',
                  read("server/modules/05-caddy.sh"))
    return m.group(1) if m else None

def landing_docroot():
    """The landing page document root, from deploy_site()'s own target.

    Scoped to the deploy_site() function deliberately. A bare search for
    `local target="/var/www/..."` matches deploy_console()'s `/var/www/admin`
    FIRST (it appears earlier in the file), so the recorded root would silently
    have been the console's — a checker that returns a plausible wrong answer is
    worse than one that returns None, because it reports OK on a wrong fact.
    """
    src = read("server/modules/05-caddy.sh")
    i = src.find("deploy_site() {")
    if i < 0:
        return None
    end = src.find("\n# ", i)          # the next section comment ends the function
    body = src[i:end if end > 0 else len(src)]
    m = re.search(r'^\s*local target="(/var/www/[a-z]+)"', body, re.M)
    return m.group(1) + "/" if m else None

def landing_download_path():
    """The shape of a download URL, from the renderer's own format string.

    Reconstructed rather than stored, so the recorded fact cannot survive a
    change to how deploy-site.sh builds the URL — which is the only thing that
    can make the documented shape wrong.
    """
    src = read("server/scripts/deploy-site.sh")
    if '"%s/updates/%s/%s"' not in src:
        return None
    m = re.search(r'HUB_BASE:-(\S+?)\}', src)
    if not m:
        return None
    return m.group(1) + "/updates/<version>/<file>"
def free_tier():
    """The free tier's port, cap and tier-row names, from the tree.

    Recomputed from the two files that DEFINE it rather than from either alone:
    the cap and port come from `04-tc.sh`'s applied class, and the tier-row names
    from `seed-pb.py`'s seed loop. §23 asserts the same agreement from the other
    direction (and adds the reboot unit and the heartbeat key); this is what
    `state.toml` is checked against, so the data file cannot go stale about a
    value the docs point at.

    Returns a dict matching the TOML shape, or None if either file has moved.
    """
    tc = read("server/modules/04-tc.sh")
    m = re.search(r'apply_tc_now\s+(\d+)\s+"[0-9:]+"\s+"([0-9a-z]+)"', tc)
    if not m:
        return None
    port, cap = m.group(1), m.group(2)

    seed = read("server/scripts/seed-pb.py")
    rows = re.findall(r'\("([a-z_]+)",\s*"ECO_PASS",\s*' + re.escape(port) + r'\)', seed)
    if not rows:
        return None
    # Sorted: the two rows describe one endpoint and their order in the seed loop
    # carries no meaning, so comparing by position would fail on a harmless
    # reorder. Compare as a set.
    return {"port": int(port), "cap": cap, "tier_rows": sorted(rows)}
def paid_tier():
    """The paid tier's port, cap and row names, from the tree.

    Same shape and same reasoning as `free_tier()`: the cap and port come from
    `04-tc.sh`'s applied class, the seeded row name from `seed-pb.py`. §27
    asserts the same agreement from the other direction (reboot unit, udp_relay
    gating, the console placeholder, and the negative that the retired tier is
    not seeded).

    NOTE the cap regex differs from `free_tier`'s: `free_tier` takes the FIRST
    `apply_tc_now` (8443) and this takes the one for the paid port. Both are
    anchored on the apply line so a comment cannot satisfy them.

    Returns a dict matching the TOML shape, or None if either file has moved.
    """
    tc = read("server/modules/04-tc.sh")
    m = re.search(r'apply_tc_now\s+8445\s+"[0-9:]+"\s+"([0-9a-z]+)"', tc)
    if not m:
        return None
    cap = m.group(1)

    seed = read("server/scripts/seed-pb.py")
    rows = re.findall(r'\("(strike)",\s*"STRIKE_PASS",\s*8445\)', seed)
    if not rows:
        return None
    # The retirement is part of the fact: `state.toml` says Stealth is not seeded,
    # and §9 must be able to FAIL when it is seeded again.
    retired = [t for t in ("stealth",) if not re.search(r'\(' + t + r'",\s*"STEALTH_PASS"', seed)]
    return {"port": 8445, "cap": cap, "tier_rows": sorted(rows), "retired_rows": sorted(retired)}

def licence_client():
    """The client/ licence, as asserted by the root LICENSE scope table.
    Derived from the LICENSE table rather than from either manifest, because the
    manifests are what drift. §11 then checks the manifests against the same row,
    so all three are bound to one authority.
    """
    m = re.search(r'^\|\s*`client/`\s*\|\s*\**`?([A-Za-z0-9.\-]+)`?\**\s*\|',
                  read("LICENSE"), re.MULTILINE)
    return m.group(1) if m else None

def theme_ids():
    """The theme registry's ids, in dropdown order.

    Parsed from `THEMES` in the registry rather than from `THEME_IDS`, because
    `THEME_IDS` is *derived from* `THEMES` (`Object.keys`) — comparing the two
    would be checking the file against itself and could never fail. The keys of
    the object literal are the authored artefact; that is what state.toml must
    agree with.

    The parse is deliberately narrow: it matches the top-level entries of the
    `THEMES` literal only, by taking quoted keys at exactly one indent level.
    A regex over nested TOML-shaped source is a crude tool, which is why it is
    paired with the vitest suite that asserts the real invariants (id == key,
    unique, resolvable) — see `client/tests/theme-colors.test.ts`.
    """
    src = read("client/src/pages/_themes.ts")
    m = re.search(r'export const THEMES: Record<ThemeId, ThemeSpec> = \{(.*?)\n\}',
                  src, re.DOTALL)
    if not m:
        return None
    return re.findall(r"^  '?([a-z0-9-]+)'?:", m.group(1), re.MULTILINE) or None

def version_sites():
    """The files `pnpm release-version` rewrites, from the bump script itself.

    Derived by scanning `release-version.mjs` for the path each `update*Version()`
    call targets, so the list cannot be restated into state.toml and drift. §17
    asserts the same set from the other direction (that every site has a live
    call); this is what the data file is checked against, so prose that links
    here ("ALL FIVE version sites") can never disagree with the code.

    Sorted, because the order the script happens to call the updaters in carries
    no meaning and comparing by position would fail on a harmless reorder.
    """
    src = read("client/scripts/release-version.mjs")
    # Each updater call names its target as a repo-relative path literal. Match
    # the call sites, not the definitions, so a renamed-but-uncalled helper is
    # not counted (the same distinction §17 makes and documents).
    found = set()
    for fn, rel in (
        ("updatePackageVersion",      "client/package.json"),
        ("updateCargoVersion",        "client/src-tauri/Cargo.toml"),
        ("updateTauriConfigVersion",  "client/src-tauri/tauri.conf.json"),
        ("updateCargoLockVersion",    "client/Cargo.lock"),
        ("updateStateTomlVersion",    "docs/state.toml"),
    ):
        if re.search(re.escape(fn) + r'\s*\(', src):
            found.add(rel)
    return sorted(found) or None

# Map fact -> callable returning the recomputed value, or None if not checkable.
CHECKERS = {
    "client.version":       manifest_version,
    "client.version_sites": version_sites,
    "hub.domain":           setup_domain,
    "hub.api_base":         publish_api_base,
    "hub.console_path":     lambda: setup_log_path("Admin console"),
    "hub.pocketbase_ui":    lambda: setup_log_path("PocketBase UI"),
    "platforms.keys":       platform_keys,
    "licence.client":       licence_client,
    "client.theme_ids":     theme_ids,
    "tiers.free":           free_tier,
    "tiers.paid":           paid_tier,
    "site.domain":           landing_domain,
    "site.document_root":    landing_docroot,
    "site.download_path":    landing_download_path,
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
        elif actual != value:
            # Structural compare, not `str() == str()`: a nested table (like
            # `tiers.free`) has no guaranteed key order on either side, so a
            # string comparison would fail on a harmless reordering — a check
            # that is wrong about the wrong thing is one people learn to ignore.
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

# ─────────────────────────────────────────────────────────────
# 15. No platform's manifest entry may name the raw updater binary.
#
# WHY THIS EXISTS
#
# `v3.2.9` and `v3.2.10` both published `manifest.json` with
#
#     "windows": { "file": "locus-windows-amd64.exe", ... }
#
# and that is the RAW PE EXECUTABLE, not an installer. The client downloads the
# entry named for its platform and hands the bytes to `tauri_plugin_updater`,
# which accepts *any* PE as an NSIS installer (`extract_exe` asks only
# `infer::app::is_exe(bytes)`) and ShellExecutes it.
#
# So the update did not install anything: it relaunched a copy of the client's
# own binary, standalone, outside the install directory, and exited. With no
# bundled web assets beside it, Tauri resolved the start page to a `file://`
# path that does not exist and Chromium displayed, inside a Locus window:
#
#     File not found
#     It may have been moved, edited, or deleted.
#     ERR_FILE_NOT_FOUND
#
# Nothing named Locus. The install directory was never touched, so reinstalling
# and restarting both changed nothing — and it recurred on every release, because
# the client accepts whatever the manifest names as newest.
#
# WHY A GUARD AND NOT JUST A FIX
#
# The fix is a line or two in the workflow. What made this bug expensive is that
# `locus-windows-amd64.exe` is a legitimate asset with a legitimate consumer —
# the retired portable client — so "stage the binary" and "advertise it for
# self-install" look identical in review, and this manifest was read several
# times without anyone noticing. This check names the distinction.
#
# The two halves are deliberately independent:
#   - the manifest must not advertise a raw updater payload for self-install;
#   - the client must refuse one even if a manifest does (see
#     `is_installer_payload` in `client/src-tauri/src/locus/update/install.rs`).
# Either alone would have prevented the bug; both together mean a future
# regression has to defeat two mechanisms in two different languages.
# ─────────────────────────────────────────────────────────────
echo
echo "15. No manifest platform entry advertises a raw updater binary for self-install"

# The name CI stages as the Windows updater payload. It is what the hub serves
# to a client that is going to EXECUTE the bytes, so it must be an installer.
if grep -qE '"windows"[[:space:]]*:[[:space:]]*"locus-windows-amd64\.exe"' "$WORKFLOW" 2>/dev/null; then
    bad "the workflow's manifest platform map advertises 'locus-windows-amd64.exe' for windows"
    bad "  that is the raw updater executable, which a client cannot install from itself:"
    bad "  tauri_plugin_updater treats any PE as an NSIS installer and would ShellExecute it"
    bad "  advertise the NSIS setup executable (installer-Locus_*_x64-setup.exe) instead"
else
    ok "the manifest does not advertise the raw Windows binary for self-install"
fi

# The positive half: the Windows entry must name a setup executable. Without
# this, deleting the platform map entirely would pass the check above.
if grep -qE 'installer-Locus_\*_x64-setup\.exe|windows_installer\(\)' "$WORKFLOW" 2>/dev/null; then
    ok "the Windows platform entry resolves to an installer"
else
    bad "the workflow does not appear to advertise an installer for windows"
    bad "  the windows platform key must resolve to installer-Locus_*_x64-setup.exe"
fi

# The client-side half must still exist, or the guard above is decorative.
#
# This checks the CALL SITE, not the symbol. `grep -q 'is_installer_payload'`
# over the whole file is a check that cannot fail: the function's definition and
# its fifteen unit tests all contain the name, so deleting the call from
# `ReadyInstall::install` — the one line that actually protects a student's
# machine — leaves eighteen matches behind and the check still prints OK.
# Demonstrated 2026-10-02: with `if true /* was: if !is_installer_payload(&bytes) */`
# in install(), this section passed. The pattern below is anchored to the
# negated call over the downloaded bytes, so it fails when the call goes away.
INSTALL_GUARD="$REPO/client/src-tauri/src/locus/update/install.rs"
if [ ! -f "$INSTALL_GUARD" ]; then
    bad "client/src-tauri/src/locus/update/install.rs is missing — the payload guard cannot be checked"
elif grep -qE 'if[[:space:]]*![[:space:]]*is_installer_payload\(&bytes\)' "$INSTALL_GUARD"; then
    ok "the client refuses to execute a payload that is not an installer"
else
    bad "the client no longer calls is_installer_payload on the downloaded bytes"
    bad "  without it, any manifest edit can hand a raw executable to ShellExecute"
fi

# The constant itself is the other half of the two-sided contract: the hub's
# fetch-release.py and the client must agree on the same bytes, or a payload is
# publishable and not installable (or worse, installable and not publishable).
# This is the 2026-10-02 defect — both sides looked for `NullsoftInstaller`,
# which no real installer contains.
# Single-quoted, so the backslashes are literal: the Rust/Python source carries
# the escape sequence `\xef\xbe\xad\xde` as TEXT in the file, and that text is
# what must agree. Reading the actual bytes here instead would check the compiled
# meaning, not the source, and this script only ever greps source.
NSIS_SIGNATURE='\xef\xbe\xad\xdeNullsoftInst'
if grep -qF "b\"$NSIS_SIGNATURE\"" "$INSTALL_GUARD"; then
    ok "the client looks for the NSIS firstheader (0xDEADBEEF + NullsoftInst)"
else
    bad "the client's NSIS signature is not the firstheader magic + NullsoftInst"
    bad "  a bare 'NullsoftInstaller' string is not in any real installer: every"
    bad "  genuine Windows update is refused before it is executed"
fi

FETCH_SERVICE="$REPO/server/scripts/fetch-release.py"
if [ ! -f "$FETCH_SERVICE" ]; then
    bad "server/scripts/fetch-release.py is missing — the hub's payload check cannot be verified"
elif grep -qF "b\"$NSIS_SIGNATURE\"" "$FETCH_SERVICE"; then
    ok "the hub looks for the same NSIS firstheader as the client"
else
    bad "the hub and the client disagree on the NSIS signature"
    bad "  the hub would publish a payload the client refuses (or the reverse)"
fi

echo
echo "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
echo "16. No absolute frontendDist is compiled into the client"

# WHY THIS EXISTS
#
# `frontendDist` is not only a build-time input. `tauri::generate_context!()`
# resolves it at COMPILE time and BAKES the resolved path into the binary, where
# it becomes the runtime asset root for `WebviewUrl::App(..)`. An absolute path
# taken from the CI runner therefore ships inside the artifact.
#
# The shipped symptom, reported 2026-09-30 against an installed Windows client
# with a clean config (`start_page: /`) and a fresh install:
#
#     File not found
#     It may have been moved, edited, or deleted.
#     ERR_FILE_NOT_FOUND
#
# with an Edge logo, in a window titled Locus, and this in `latest.log`:
#
#     [Window] page load started:  url=file:///D:/
#
# The `D:/` is the Windows CI runner's own workspace (`D:\a\Locus\Locus`),
# compiled into the binary by an absolute `frontendDist` and reintroduced at run
# time against a directory that does not exist. The window never becomes visible
# (it is shown on page-load Finished, which never arrives), so the app looks
# broken while every install is healthy — and no config, restart or reinstall
# can affect it, because the bad value is in the executable.
#
# The path arrived here with a commit ("better workflow") that made CI faster by
# building the frontend once and overriding `frontendDist` to an absolute path so
# `tauri build` would reuse it. A follow-up normalized its separators to fix a
# JSON-escape panic ("invalid escape at line 1 column 54") — which made the build
# GREEN while leaving the defect intact, because the wrong thing (an absolute
# runner path in a shipped binary) was never the escaping.
#
# The committed `tauri.conf.json` carries `"frontendDist": "../dist"`, relative
# to the config file, and the build job downloads the bundle to `client/dist` —
# exactly what `../dist` resolves to. No absolute path is ever needed. This check
# fails if one is set again, in the workflow OR in the committed config.
#
# `frontendDist` may legitimately be a build-machine path in a form that never
# ships (none today). The rule this enforces is narrow and mechanical: no
# `frontendDist` value may be, or be built from, an absolute path.
WORKFLOW="$REPO/.github/workflows/client.yml"
TAURI_CONF="$REPO/client/src-tauri/tauri.conf.json"

# (a) The workflow must not set an absolute `frontendDist` anywhere — not as a
#     literal, and not by interpolating a workspace/root variable. The positive
#     form is the risky one: a `TAURI_CONFIG` that looks assembled is exactly
#     what shipped this bug, so both shapes are refused.
# The rule is deliberately the SIMPLEST one that catches every shape: no LIVE
# line of the workflow may mention `frontendDist`. The correct state sets none —
# `tauri.conf.json`'s relative value is used as-is — so any live occurrence is an
# override, and an override is the defect. Matching "any live mention" also means
# the guard cannot be evaded by a shape I did not anticipate: the literal,
# `${{ github.workspace }}`, `$GITHUB_WORKSPACE`, a node/jq-assembled value, and
# anything future all fail the same way.
#
# COMMENT lines are excluded, and that is not a loophole: a `#` line cannot set
# an environment variable, so it cannot reach the build. The comments above the
# build step legitimately discuss this defect and name the key; the guard's job
# is the config, not the prose. (Only full-line comments are stripped — a
# trailing `# ...` on a live line is NOT, because `key: value # comment` is live
# config and must be checked.)
LIVE_WORKFLOW="$(grep -vE '^[[:space:]]*#' "$WORKFLOW" 2>/dev/null || true)"

if printf '%s' "$LIVE_WORKFLOW" | grep -qF 'frontendDist' 2>/dev/null; then
    bad "the workflow sets 'frontendDist' — it must not"
    bad "  an absolute value is compiled into the binary as its runtime asset root"
    bad "  and ships: a Windows build then resolves its page against"
    bad "  D:/a/Locus/Locus/... and WebView2 shows ERR_FILE_NOT_FOUND on a healthy"
    bad "  install. Normalizing separators does NOT help."
    bad "  use the committed relative \"../dist\" and download the bundle to client/dist"
else
    ok "the workflow sets no frontendDist override"
fi

# (b) The committed config must keep `frontendDist` relative. An absolute value
#     here is the same defect with a longer fuse: it only breaks on machines that
#     lack the author's path, and it is what the workflow override was copying.
if [ ! -f "$TAURI_CONF" ]; then
    bad "client/src-tauri/tauri.conf.json is missing — frontendDist cannot be checked"
elif grep -qE '"frontendDist"[[:space:]]*:[[:space:]]*"/' "$TAURI_CONF" 2>/dev/null; then
    bad "client/src-tauri/tauri.conf.json sets an absolute 'frontendDist'"
    bad "  it must stay relative (\"../dist\"): an absolute path is compiled into"
    bad "  the binary and breaks every machine that lacks that directory."
elif grep -qE '"frontendDist"[[:space:]]*:[[:space:]]*"\.\./dist"' "$TAURI_CONF" 2>/dev/null; then
    ok "tauri.conf.json keeps a relative frontendDist (\"../dist\")"
else
    warn "tauri.conf.json 'frontendDist' is not the expected \"../dist\" — confirm it is relative"
fi

# (c) The build job must download the bundle to the directory `../dist` resolves
#     to. If these drift apart, the relative path silently points at nothing and
#     `tauri::generate_context!()` panics (or, worse, bundles the wrong tree).
if grep -qE 'path:[[:space:]]*client/dist' "$WORKFLOW" 2>/dev/null; then
    ok "the build job downloads the frontend bundle to client/dist"
else
    bad "the build job does not download the frontend bundle to client/dist"
    bad "  the committed \"../dist\" (relative to client/src-tauri/) resolves there;"
    bad "  a different path means the bundled assets are not where tauri looks."
fi

echo "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
echo "17. Every version site is written by the bump script"

# WHY THIS EXISTS
#
# A release is cut by `cd client && pnpm release-version X`, which rewrites every
# file that carries the version. On 2026-09-30 a `v3.2.13` tag was pushed on a
# commit whose files all still said `3.2.12`; CI (which labels the release from
# the tag name and never bumps) published 3.2.12 binaries under a 3.2.13 release
# and the hub offered clients an "update" that reinstalled the same version
# forever. The recovery attempt then bumped only FOUR files, because
# `docs/state.toml`'s `[client.version]` is not a manifest and was easy to miss —
# and this very script's §7 caught it with `BAD client.version`.
#
# §7 catches the value being WRONG. This catches a site being MISSING from the
# bump, which §7 cannot see: if a fifth manifest appears and nobody adds it to
# the script, the script keeps succeeding and publishes a bug in the field.
#
# It does not guess. It asserts the version in each known manifest is a string
# the script can and does rewrite — i.e. each site is named in release-version.mjs
# — so a new site fails here, at the cheap gate, naming itself.
BUMP_SCRIPT="$REPO/client/scripts/release-version.mjs"
if [ ! -f "$BUMP_SCRIPT" ]; then
    bad "client/scripts/release-version.mjs is missing — no supported way to bump"
else
    # Each entry: a manifest path, and the exact CALL the bump script must make
    # to rewrite that file. The marker is the invocation, not the function name:
    # a name also appears in the definition and the docstring, so grepping for it
    # would pass even with the call deleted — which is precisely the failure this
    # guard exists to catch. (Verified: with the call removed and the function
    # left defined, a name-based marker reported OK.)
    #
    # The misses are accumulated in a FILE, not a variable: the loop reads its
    # list from a heredoc, and a construct that runs the loop in a subshell (as
    # some shells do) would silently discard a variable set inside it — a guard
    # that reports OK no matter what. A file survives either way.
    bump_gaps=$(mktemp)
    bump_checked=0
    while IFS='|' read -r site_path script_marker label; do
        [ -n "$site_path" ] || continue
        bump_checked=$((bump_checked + 1))
        if [ ! -f "$REPO/$site_path" ]; then
            # Record it as a gap too, so the success branch below cannot also
            # fire. Without this the check printed BAD *and* "all N ... OK" for
            # the same run — a self-contradicting result.
            echo "         $label — the file $site_path does not exist" >> "$bump_gaps"
            continue
        fi
        if ! grep -qF "$script_marker" "$BUMP_SCRIPT"; then
            echo "         $label ($site_path)" >> "$bump_gaps"
        fi
    done <<'SITES'
client/package.json|await updatePackageVersion(|package.json
client/src-tauri/Cargo.toml|await updateCargoVersion(|Cargo.toml
client/src-tauri/tauri.conf.json|await updateTauriConfigVersion(|tauri.conf.json
client/Cargo.lock|await updateCargoLockVersion(|Cargo.lock
docs/state.toml|await updateStateTomlVersion(|docs/state.toml
SITES

    if [ -s "$bump_gaps" ]; then
        bad "these version sites are not covered by the bump script:"
        while IFS= read -r line; do bad "$line"; done < "$bump_gaps"
        bad "A release would leave them stale, publishing a mislabelled build."
        bad "Add the file to client/scripts/release-version.mjs (its"
        bad "updateStateTomlVersion shows the pattern) and to §17's SITES list."
    else
        ok "all $bump_checked version site(s) are written by client/scripts/release-version.mjs"
    fi
    rm -f "$bump_gaps"
fi

echo
echo "18. The Windows installer is signed, under the name the release job prefixes"
# The `installer-` prefix is added by the RELEASE job's "Prefix and stage the
# installers" step. The SIGN step runs in the BUILD job, where the file still
# carries Tauri's raw name. A glob written against the prefixed name in the build
# job can never match — and because the step exits 1 in that case, the cost is a
# ~30 minute four-platform build that ends in "no installer to sign".
#
# Two separate defects lived here and both shipped:
#   1. the step was gated on `matrix.os == 'windows-latest'` while the matrix
#      pins `windows-2022`, so it never ran at all (see §18's sibling check);
#   2. once it did run, it globbed `installer-Locus_*` in a directory that only
#      ever holds `Locus_*`.
# Both are the same failure shape: a name asserted in one half and not the other.
# This check asserts the AGREEMENT between the two jobs rather than either half
# (AGENTS.md, "assert the agreement").
wf_installer="$WORKFLOW"
if [ -f "$wf_installer" ]; then
    # Scoped to `installer/Locus_*` deliberately. Several steps iterate
    # `installer/*` now (the macOS .dmg repack does), and a bare `head -1` would
    # grab whichever comes first in the file — so this check would silently stop
    # testing the step it exists for, which is the failure mode it is about.
    sign_glob="$(grep -oE 'for f in installer/Locus_[^;]+' "$wf_installer" | head -1 || true)"
    prefix_src="$(grep -oE 'cp "\$f" "release/installer-\$base"' "$wf_installer" | head -1 || true)"
    # The `if:` line follows the step name, but a comment block may sit between
    # them (the file explains WHY at the point of contact). Scan forward from the
    # name to the next `if:` or the next `- ` step boundary, whichever comes first.
    sign_cond="$(python3 - "$wf_installer" <<'PY'
import re, sys

lines = open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
cond = ""
for i, line in enumerate(lines):
    if line.strip().startswith("- name:") and "Sign the Windows installer" in line:
        for follow in lines[i + 1:]:
            s = follow.strip()
            if s.startswith("- name:"):
                break            # next step: no if: on this one
            if s.startswith("if:"):
                cond = s
                break
        break
print(cond)
PY
)"

    if [ -z "$sign_glob" ]; then
        bad "the \"Sign the Windows installer\" step no longer globs a staged installer"
        bad "  if that step was removed, the Windows installer ships unsigned and no"
        bad "  Windows client can install the update (RECOVER-WINDOWS-UPDATE.md)"
    else
        # The glob must NOT carry the prefix: it runs before the rename.
        case "$sign_glob" in
            *installer/Locus_*)
                ok "the sign step globs the pre-prefix name (installer/Locus_*)"
                ;;
            *)
                bad "the sign step globs '${sign_glob#for f in }' in the BUILD job"
                bad "  the 'installer-' prefix is added later by the release job, so this"
                bad "  can never match and the step exits 1 after a full four-platform build"
                ;;
        esac
    fi

    if [ -n "$prefix_src" ]; then
        ok "the release job prefixes the installer (release/installer-\$base)"
    else
        bad "the release job's installer-prefixing step is missing or renamed"
        bad "  the hub, the manifest and fetch-release.py all resolve"
        bad "  installer-Locus_<v>_x64-setup.exe, so without the prefix no"
        bad "  Windows client is offered a file it can install"
    fi

    if [ -z "$sign_cond" ]; then
        warn "could not read the sign step's if: condition; skipping its check"
    elif printf '%s' "$sign_cond" | grep -q "matrix.label"; then
        ok "the sign step is gated on matrix.label (${sign_cond#*if: })"
    else
        bad "the sign step is gated on ${sign_cond#*if: }, not matrix.label"
        bad "  gate platform steps on matrix.label: matrix.os is a runner image that"
        bad "  gets bumped (macos-13 -> macos-14) and silently detaches the condition"
    fi
fi

echo
echo "19. Every workflow step condition names a matrix value that exists"
# A step gated on a matrix value that the matrix never produces is a step that
# CANNOT RUN — and it skips silently, so the job still goes green.
#
# This is not hypothetical: "Sign the Windows installer" was gated on
# `if: matrix.os == 'windows-latest'` while the matrix pins `os: windows-2022`
# (component actions aside, images are pinned deliberately — `*-latest` is a
# world claim nobody re-checks). The condition was false on every run, so the
# NSIS installer shipped unsigned and every Windows client refused its update.
#
# The failure mode is the one DEBUGGING-METHOD.md §3 names: a check that cannot
# fire reads exactly like a check that passed. Nothing in CI could see it,
# because a skipped step is a green step.
if [ -f "$WORKFLOW" ]; then
    matrix_gaps="$(python3 - "$WORKFLOW" <<'PY'
import re, sys

path = sys.argv[1]
text = open(path, encoding="utf-8", errors="replace").read()

# Values the matrix can actually produce, per key. `include:` entries are
# `- key: value` lines indented under the matrix block.
def matrix_values(key):
    return set(re.findall(r'^\s+%s:\s*(\S+)\s*$' % re.escape(key), text, re.M))

# Conditions in this workflow only ever test matrix.<key>.
tested = set(re.findall(r'matrix\.([A-Za-z_][A-Za-z0-9_]*)\s*[!=]', text))

if not tested:
    sys.exit(0)

lines = text.splitlines()
gaps = []
for i, line in enumerate(lines, 1):
    m = re.search(r'^\s+if:\s*(.+)$', line)
    if not m:
        continue
    cond = m.group(1)
    for key in re.findall(r'matrix\.([A-Za-z_][A-Za-z0-9_]*)\s*[!=]=', cond):
        values = matrix_values(key)
        if not values:
            gaps.append(f"{path}:{i}: condition tests matrix.{key}, which the matrix does not define")
            continue
        # Every literal compared against, e.g. == 'windows' / == "windows"
        for literal in re.findall(r'matrix\.%s\s*[!=]=\s*[\'"]([^\'"]*)[\'"]' % re.escape(key), cond):
            # `!=` means "everything but", so only `==` can be vacuous.
            if re.search(r'matrix\.%s\s*==\s*[\'"]%s[\'"]' % (re.escape(key), re.escape(literal)), cond) \
               and literal not in values:
                gaps.append(
                    f"{path}:{i}: if: matrix.{key} == '{literal}' can never match; "
                    f"the matrix defines {sorted(values)}"
                )
print("\n".join(gaps))
PY
)"
    if [ -n "$matrix_gaps" ]; then
        bad "a workflow condition tests a matrix value that does not exist (the step would silently skip):"
        printf '%s\n' "$matrix_gaps" | while IFS= read -r line; do bad "  $line"; done
        bad "A skipped step is a green step, so nothing else in CI can catch this."
    else
        ok "every matrix-dependent step condition matches a value the matrix produces"
    fi
else
    warn "workflow not found at ${WORKFLOW}; skipping the matrix-condition check"
fi

echo
echo "20. The macOS disk image is signed ad-hoc, and carries no README"
# ─────────────────────────────────────────────────────────────
# WHY THIS EXISTS
#
# This section used to assert the OPPOSITE: that the .dmg carried a
# "READ ME FIRST.txt" telling the student to run `xattr -cr` in Terminal.
# That mitigation is gone, and the reason it is gone is worth keeping, because
# the file looked like the fix for a year and was not.
#
# WHAT WAS WRONG WITH IT
#
# The file existed because an unsigned, quarantined .app produced
#
#   "Locus" is damaged and can't be opened. You should move it to the Trash.
#
# — and the README told the student that message was expected and what to type.
# That is true, and it is still true that the wording cannot be fixed without an
# Apple account. What the README could not fix is that the remedy required a
# TERMINAL: a student who does not know what a command line is had no path
# forward at all short of following instructions to type a command they cannot
# evaluate. A text file next to the app is documentation for a failure the
# student has to escape, not a fix for it.
#
# WHAT REPLACED IT
#
# Ad-hoc signing. `codesign --sign -` does not make the app trusted — there is
# still no Developer ID and no notarization — but it changes WHICH Gatekeeper
# outcome the student meets, and that difference is the whole point:
#
#   unsigned  -> "Locus is damaged and can't be opened"   -> NO "Open Anyway"
#   ad-hoc    -> "Apple cannot check it for malicious
#                software" (unidentified developer)        -> "Open Anyway" in
#                                                             Privacy & Security
#
# Gatekeeper treats a signature that exists but is not a Developer ID as an
# identity/notarization problem, not as corruption, so the app moves from the
# unbypassable class to the bypassable one. The remedy becomes a control in
# System Settings instead of a command in Terminal.
#
# WHY "SIGNED, NOT VERIFIED" IS NOT A GUARD
#
# A step that runs `codesign` and does not read the result back is indistinguish-
# able from one that silently did nothing — the same shape as `hdiutil create`
# exiting 0 on an image that lost the file (which is why the old section mounted
# the .dmg back). So this check does not assert that a `codesign` line exists.
# It asserts that the workflow ALSO verifies: `codesign --verify`, a
# `spctl`/assessment read, or an explicit artifact read-back. Without that half,
# a runner where signing failed would publish an unsigned installer and nothing
# in the tree would say so.
#
# WHAT THIS CANNOT CHECK
#
# Like §14 this is a grep over the tree, not a mount or a launch. It cannot prove
# the signature reaches a student, and it cannot prove the "Open Anyway" button
# appears on a given macOS version — that needs real hardware, and it is recorded
# as unverified in `docs/reference/STILL-OPEN.md`. What it catches is the change
# that would make the mechanism dead: deleting the signing step, dropping its
# verification half, or reintroducing the README it replaced.
# ─────────────────────────────────────────────────────────────

# ── The README is GONE, and must stay gone ──
#
# Stated as an assertion so that reintroducing the file is a deliberate act that
# turns a check red, rather than a change that quietly restores a Terminal-only
# path alongside the signing that made it unnecessary. The `xattr -cr` advice is
# not wrong, it is just no longer the answer, and two remedies that disagree
# about which is primary is worse than one.
MACOS_README="$REPO/client/src-tauri/packages/macos/READ ME FIRST.txt"
if [ -f "$MACOS_README" ]; then
    bad "client/src-tauri/packages/macos/READ ME FIRST.txt is back"
    bad "  ad-hoc signing replaced it: it moves the Gatekeeper failure from the"
    bad "  unbypassable 'damaged' dialog into Privacy & Security -> Open Anyway."
    bad "  If the README is genuinely needed again, say so in docs/operate/OPS.md"
    bad "  and update this section — do not leave both remedies in the tree."
else
    ok "the macOS in-image README is gone (replaced by ad-hoc signing)"
fi

# No workflow step may copy it back in. This catches the case where the file is
# deleted but the repack step that references it survives — which would fail at
# build time on a macOS runner and pass everywhere else, including here.
#
# MATCHES THE COPY, NOT THE NAME. A bare `grep READ ME FIRST` also matches the
# comment that explains WHY the file was removed — so this check failed on its
# own documentation the first time it ran, and the tempting fix (delete the
# explanation) would have left the guard passing for the wrong reason while the
# reason for the removal was lost. That is the "check that cannot distinguish a
# right answer from a wrong one" trap §14 and the old §20 both describe. What is
# asserted is the MECHANISM: a `cp` (or an `install`) whose source is that path,
# or a step that reads it back out of the image. Prose cannot satisfy either.
if [ -f "$WORKFLOW" ]; then
    readme_copy=$(grep -nE '(cp|install)[^|]*READ ME FIRST\.txt|\[ -f [^]]*READ ME FIRST\.txt' "$WORKFLOW" || true)
    if [ -n "$readme_copy" ]; then
        bad "the workflow still copies/reads READ ME FIRST.txt"
        printf '%s\n' "$readme_copy" | head -5 | sed 's/^/         /'
    else
        ok "no workflow step copies READ ME FIRST.txt into the image"
    fi

    # ── The signing step, and the half that makes it real ──
    #
    # `codesign ... --sign -` is the ad-hoc form: the identity is a literal
    # hyphen, not a name. A Developer-ID identity would look different, and this
    # check would then need rewriting — which is correct, because that is a
    # different mechanism with different claims.
    if grep -qE 'codesign[^|]*--sign[[:space:]]+-' "$WORKFLOW"; then
        ok "a workflow step ad-hoc signs the macOS bundle"
    else
        bad "no ad-hoc codesign step found in the workflow"
        bad "  without it a quarantined .app shows 'damaged and can't be opened',"
        bad "  which offers no 'Open Anyway' and can only be escaped in Terminal"
    fi

    # The read-back. Signing that is not verified is the "check that cannot fail"
    # trap in its most literal form: `codesign --sign` on a bundle it cannot
    # process exits non-zero, but a step whose failure is swallowed by `||true`,
    # a `continue-on-error`, or a missing `set -e` publishes the unsigned image
    # while the log shows the signing line having run.
    if grep -qE 'codesign[^|]*(--verify|-v[[:space:]])' "$WORKFLOW"; then
        ok "the signing step is read back with a verification of its own"
    else
        bad "the ad-hoc signing step is never verified"
        bad "  add a codesign --verify (or an spctl/artifact read-back) after signing,"
        bad "  so a runner where signing failed cannot publish an unsigned image"
    fi

    # macOS-only, for the same reason the old repack step had to be: `codesign`
    # and `hdiutil` do not exist on the Linux and Windows runners, so an
    # ungated step is a hard failure on two of four platforms rather than a
    # no-op. Gating on `matrix.label` rather than `matrix.os` is the lesson §18
    # and §19 encode — `os` gets bumped (macos-13 -> macos-14) and detaches every
    # condition written against the old value.
    if grep -qE "if: *startsWith\(matrix\.label, *'macos'\)" "$WORKFLOW"; then
        ok "the macOS signing/repack steps are gated on matrix.label"
    else
        bad "the macOS-only steps are not gated on matrix.label starting with 'macos'"
    fi
else
    warn "workflow not found at ${WORKFLOW}; skipping the signing-step checks"
fi

# ── The live docs must not still promise the README as PRESENT ──
#
# This is the drift that made the original section necessary in reverse: the
# Tauri rewrite dropped the file while prose kept claiming it was there. The
# same failure is available in the other direction — a doc left asserting the
# image carries instructions that no longer exist, so a student (or an operator
# answering a support ticket) is told to look for a file that is not there.
#
# WHAT THIS CANNOT DO, stated because the first version of this check got it
# wrong: a grep cannot tell "the image carries a README" from "the README was
# removed, and here is why". Banning the name outright made the check fail on
# its own explanation, and the cheapest way to make it green would have been to
# delete the explanation — leaving the guard passing for the wrong reason and
# the reasoning lost. So the assertion is on the OPERATIONAL verb: a sentence
# that says the artifact *carries*, *has*, or *includes* the file, or tells the
# reader to *check/look for/find* it in the image. A doc that narrates the
# removal does not match any of them.
#
# The negations are filtered out deliberately, and this is the third shape this
# check took. `no longer carries`, `does not carry` and `no README` are sentences
# that agree with the removal; a rule that flags them forces the writer to stop
# explaining, which is the opposite of what this file is for. The filter is
# narrow — it matches only an explicit negation immediately before the verb —
# so "the image carries a README" still fails while "the image no longer carries
# a README" passes.
OPS_DOC="$REPO/docs/operate/OPS.md"
if [ -f "$OPS_DOC" ]; then
    stale_claim=$(grep -nEi '(carries|has|includes|ships)[^.]*READ ME FIRST|(look|check|find) for[^.]*READ ME FIRST|hdiutil attach[^.]*READ ME FIRST' "$OPS_DOC" \
        | grep -viE 'no longer|does not|doesn.t|no README|not carry|never|removed' || true)
    if [ -n "$stale_claim" ]; then
        bad "docs/operate/OPS.md still says the macOS image carries a README"
        printf '%s\n' "$stale_claim" | head -3 | sed 's/^/         /'
        bad "  fix the prose in the same change that removes the file (README rule 3)"
    else
        ok "OPS.md does not promise a README in the macOS image"
    fi
fi

echo "21. macOS asks for the Service install when it is absent"
# ─────────────────────────────────────────────────────────────
# WHY THIS EXISTS
#
# macOS has no installer that registers the Locus Service. On Windows the NSIS
# setup does it during install, so a fresh install is never `NotInstalled` — but
# a `.app` dragged out of the `.dmg` is, on first launch, with `enable_tun_mode`
# defaulting to false. The only writer of that flag was the Connect path, which
# sits behind the `tun_capable()` refusal an absent Service causes:
#
#     no Service -> Connect refused -> TUN never enabled
#     TUN disabled -> install never requested -> no Service
#
# Nothing broke the loop, so a student who installed from the `.dmg` could never
# connect, and the install affordance never appeared either (the dialog is driven
# by `serviceNeedsAttention`, and an absent Service was deliberately excluded
# from it). A real student hit this and there was no in-app route out at all.
#
# The fix is that `prepare_startup` requests the install for an absent Service on
# macOS. This guard exists because that behaviour is INVISIBLE from the tree: it
# cannot be exercised without macOS, the deadlock is silent, and reverting it
# restores a state where the app looks healthy and simply never connects. Every
# separate piece below was independently necessary, so each is asserted:
#   - the predicate that decides it,
#   - its macOS gate,
#   - that it is actually called from the startup path,
#   - and that the refusal no longer points at a Settings control macOS lacks.
# ─────────────────────────────────────────────────────────────
LIFECYCLE="$REPO/client/src-tauri/src/core/manager/lifecycle.rs"
if [ ! -f "$LIFECYCLE" ]; then
    warn "no lifecycle.rs — this guard cannot run"
else
    if grep -q 'fn should_request_install_for_absent_service' "$LIFECYCLE"; then
        ok "the absent-Service install predicate exists"
    else
        bad "should_request_install_for_absent_service is gone from lifecycle.rs"
        bad "  without it nothing requests the Service install on macOS, and a DMG"
        bad "  install can never connect — see the comment above it for the loop"
    fi

    # The call site, not just the definition. A predicate nothing calls is inert,
    # which is the exact shape of the original bug (the pieces were all present).
    #
    # Read through a python extractor that strips comments, because this file
    # explains all three pieces in prose at the point of contact — a bare grep
    # matches the explanation, so deleting the call while keeping its comment
    # would read as a pass. That blind spot was found by falsifying this guard
    # rather than by writing it.
    lifecycle_code="$(python3 - "$LIFECYCLE" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
# Drop `//`-style comments (including doc comments) line by line.
kept = []
for line in text.splitlines():
    stripped = line.strip()
    if stripped.startswith("//"):
        continue
    kept.append(line)
print("\n".join(kept))
PY
)"

    if printf '%s' "$lifecycle_code" | grep -q 'if should_request_install_for_absent_service('; then
        ok "the startup path calls the absent-Service predicate"
    else
        bad "prepare_startup does not CALL should_request_install_for_absent_service"
        bad "  the predicate existing is not enough: nothing raises the install request,"
        bad "  so a macOS DMG install still cannot connect"
    fi

    if printf '%s' "$lifecycle_code" | grep -q 'require_install_for_session()'; then
        ok "the startup path still raises the install request"
    else
        bad "prepare_startup no longer calls require_install_for_session()"
    fi

    # The gate must be macOS. On Windows the NSIS installer registers the Service,
    # so an absent one means removal and the pre-existing handling applies;
    # widening this platform-blind would change Windows behaviour unasked.
    #
    # Checked as the cfg attribute IMMEDIATELY preceding the call, since the file
    # also carries a `#[cfg(target_os = "macos")]` elsewhere (the dev chmod path).
    if python3 - "$LIFECYCLE" <<'PY'
import re, sys
code = "\n".join(
    l for l in open(sys.argv[1], encoding="utf-8", errors="replace").read().splitlines()
    if not l.strip().startswith("//")
)
# An `if should_request_install_for_absent_service(` guarded by a macOS cfg on the
# line directly above it.
ok = re.search(
    r'#\[cfg\(target_os = "macos"\)\]\s*\n\s*if should_request_install_for_absent_service\(',
    code,
)
sys.exit(0 if ok else 1)
PY
    then
        ok "the absent-Service request is cfg-gated to macOS at the call site"
    else
        bad "the absent-Service install request is not immediately cfg-gated to macOS"
        bad "  it must not change Windows behaviour, where absence means removal"
    fi
fi

# The refusal wording. It used to say "Install the service from Settings", which
# macOS does not render — a student followed it, found no such control, and had
# nowhere left to go. A message naming UI that does not exist is worse than no
# message, because it sends someone looking.
REFUSAL="$REPO/client/src-tauri/src/cmd/locus.rs"
if [ ! -f "$REFUSAL" ]; then
    warn "no cmd/locus.rs — this guard cannot run"
else
    # Scoped to the CONSTANT, not the file. The explanatory comments above it
    # quote the old wording to say why it was wrong, and a bare file-wide grep
    # matches those — so removing the string while keeping its rationale would
    # read as a failure, which is backwards. This extracts the value only.
    refusal_value="$(python3 - "$REFUSAL" <<'PY'
import re, sys
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
m = re.search(r'TUN_UNAVAILABLE_MESSAGE\s*:\s*&str\s*=\s*"(.*?)";', text, re.S)
print(m.group(1) if m else "")
PY
)"

    if [ -z "$refusal_value" ]; then
        bad "TUN_UNAVAILABLE_MESSAGE was not found in cmd/locus.rs"
        bad "  if it was renamed, update this guard so the wording stays checked"
    elif printf '%s' "$refusal_value" | grep -q 'from Settings'; then
        bad "the TUN refusal still tells the student to install the service 'from Settings'"
        bad "  macOS renders no such control; the message must name the permission"
        bad "  prompt the app raises instead"
    else
        ok "the refusal does not point at a Settings control that may not exist"
    fi
fi

# 22. A shell helper is defined before its first use.
#
# WHY THIS EXISTS
#
# `hooks-sync.sh` called `remote()` from `assert_reachable()` 132 lines before
# `remote()` was defined. Every invocation therefore died with `remote: command
# not found`, the reachability sentinel came back empty, and the script reported
#
#   cannot reach <host> over SSH (no usable key, or the host is down)
#
# for every host — including a reachable one. The tool was never runnable, and
# nobody could tell, because a hardcoded failure is indistinguishable from a real
# one by exit status.
#
# The function that broke is the one whose entire purpose is to stop "we could
# not ask" being read as "the answer is no" (see its comment, and
# docs/reference/DEBUGGING-METHOD.md). A guard that always answers "no" is the
# exact defect it existed to prevent, so it gets its own check.
#
# The rule asserted: within a script that defines helper functions, no call to a
# `name()` function may appear textually before that function's definition. This
# is a textual approximation of a runtime property — it is conservative (it does
# not model branches or subshells) but it catches the real failure, which is a
# definition placed below the code that uses it.
echo
echo "22. Shell helpers are defined before their first use"

HOOKSYNC="$REPO/server/scripts/hooks-sync.sh"
if [ ! -f "$HOOKSYNC" ]; then
    warn "no hooks-sync.sh — this guard cannot run"
else
    ordering_problem="$(python3 - "$HOOKSYNC" <<'PY'
import re, sys

path = sys.argv[1]
lines = open(path, encoding="utf-8", errors="replace").read().splitlines()

# Definitions: a line starting with `name() {` at any indent.
defs = {}
for i, line in enumerate(lines):
    m = re.match(r'^\s*([A-Za-z_][A-Za-z0-9_]*)\(\)\s*\{', line)
    if m:
        defs.setdefault(m.group(1), i)

problems = []
for name, def_line in defs.items():
    # Find the first CALL of this helper: `name ` or `$(name ` etc., skipping the
    # definition line itself and any line that is a comment.
    for i, line in enumerate(lines):
        if i == def_line:
            continue
        stripped = line.lstrip()
        if stripped.startswith('#'):
            continue
        if re.search(r'(?:^|[^\w$])' + re.escape(name) + r'(?![\w-])', line):
            if i < def_line:
                problems.append(
                    '%s called at line %d, defined at line %d'
                    % (name, i + 1, def_line + 1)
                )
            break

print('\n'.join(problems))
PY
)"

    if [ -n "$ordering_problem" ]; then
        bad "a helper is used before it is defined in hooks-sync.sh:"
        while IFS= read -r line; do
            [ -n "$line" ] && bad "  $line"
        done <<EOF
$ordering_problem
EOF
    else
        ok "every helper in hooks-sync.sh is defined before its first use"
    fi
fi

echo
echo "23. The free tier's endpoint, cap and allowance agree across the tree"
#
# WHY THIS EXISTS
#
# The free tier reuses the legacy Eco slot: port 8443, the Eco password, the
# `tc-eco-cap.service` unit — renamed customerside only. That means FOUR files
# describe one endpoint, in three different vocabularies:
#
#   server/modules/04-tc.sh      the tc cap ("1mbit") and the port (8443)
#   server/scripts/seed-pb.py    the tier_configs rows ("free" and "eco" -> 8443)
#   server/scripts/fix-tier-configs.py / seed-live.py   the repair/seed tools
#   server/pb_hooks/heartbeat.pb.js   the allowance (free_allowance_mb)
#
# Nothing tied them together, so the free tier could be half-deployed in a way
# that looks complete: the cap dropped here, the row forgotten there, and the
# student gets an activation that names a tier with no server_config — a
# "connected" app that cannot reach the internet (the exact failure class in
# FIXES.md). This asserts the AGREEMENT, not each file separately: the point is
# that they match, and a check on one file alone cannot tell you that.
#
# Every assertion below was observed FAILING against the pre-fix tree before it
# was kept: the 5mbit cap, the missing `free` seed row, and a heartbeat with no
# allowance field each tripped it.

TC_MOD="$REPO/server/modules/04-tc.sh"
SEED="$REPO/server/scripts/seed-pb.py"
HB="$HOOKS/heartbeat.pb.js"

if [ ! -f "$TC_MOD" ] || [ ! -f "$SEED" ] || [ ! -f "$HB" ]; then
    warn "free-tier guard: a file it needs is missing — skipping"
else
    # (a) The 8443 tc class is capped at 1mbit, not the legacy 5mbit.
    #     Match the apply line specifically: `apply_tc_now 8443 "1:10" "1mbit"`.
    if grep -Eq 'apply_tc_now[[:space:]]+8443[[:space:]]+"1:10"[[:space:]]+"1mbit"' "$TC_MOD"; then
        ok "04-tc.sh caps the free/Eco port (8443) at 1mbit"
    else
        bad "04-tc.sh does NOT cap port 8443 at 1mbit (expected: apply_tc_now 8443 \"1:10\" \"1mbit\")"
    fi

    # (b) The reboot-persistence unit uses the same rate, or a reboot silently
    #     restores the old cap while the running class says otherwise.
    if grep -Eq 'create_tc_service[[:space:]]+"eco"[[:space:]]+"1:10"[[:space:]]+"1mbit"[[:space:]]+8443' "$TC_MOD"; then
        ok "04-tc.sh's tc-eco-cap.service uses the same 1mbit rate"
    else
        bad "04-tc.sh's tc-eco-cap.service rate disagrees with the applied cap"
    fi

    # (c) The seed carries BOTH tier names against 8443. `free` is minted
    #     against; `eco` must survive so codes already in the field resolve.
    if grep -Eq '\("free",[[:space:]]*"ECO_PASS",[[:space:]]*8443\)' "$SEED"; then
        ok "seed-pb.py seeds the \`free\` tier against port 8443"
    else
        bad "seed-pb.py has no \`free\` tier row on port 8443 — the console cannot mint a working free code"
    fi
    # INVERTED 2026-10-06. This assertion used to REQUIRE the legacy `eco` row,
    # on the reasoning that a code minted in the Eco era carries that string and
    # must keep resolving. That reasoning was sound and is now spent: the live
    # hub holds NO code with `tier = "eco"` (verified 2026-10-06), so the row was
    # a duplicate of `free` on the same port — which the console renders as a
    # second plan sharing one port, i.e. a bug every operator sees daily, buying
    # protection for a code that does not exist.
    #
    # Kept as a tombstone rather than deleted, so a reader who remembers the old
    # rule finds out why it is gone instead of re-adding the row.
    if grep -Eq '\("eco",[[:space:]]*"ECO_PASS",[[:space:]]*8443\)' "$SEED"; then
        bad "seed-pb.py still seeds the retired \`eco\` row — it duplicates \`free\` on 8443 and shows as a second plan in the console"
    else
        ok "seed-pb.py seeds one row per plan (no retired \`eco\` duplicate)"
    fi

    # (d) The heartbeat carries the allowance, and only for the free tiers.
    #     Assert the KEY, because that is the wire contract the client reads.
    # Anchor on the ASSIGNMENT — see the note on the throttle check below.
    if grep -Eq 'response\.free_allowance_mb[[:space:]]*=' "$HB"; then
        ok "heartbeat.pb.js sends free_allowance_mb"
    else
        bad "heartbeat.pb.js does not send the free-tier allowance — the client has nothing to count against"
    fi
    # Anchor on the ASSIGNMENT (`response.free_throttle_mbps =`), not a bare
    # mention: the key also appears in the file's own comments and in the
    # enforcement-version note, so a plain grep stays green with the assignment
    # deleted — a check that cannot fail, which is the defect this section
    # exists to catch. (It did exactly that on the first draft.)
    if grep -Eq 'response\.free_throttle_mbps[[:space:]]*=' "$HB"; then
        ok "heartbeat.pb.js sends free_throttle_mbps"
    else
        bad "heartbeat.pb.js does not send the free-tier throttle speed — the client has nothing to slow to"
    fi
    if grep -Eq 'tierVal === "free"' "$HB" && ! grep -Eq 'tierVal === "eco"' "$HB"; then
        ok "the allowance is gated to the free plan only"
    elif ! grep -Eq 'tierVal === "free"' "$HB"; then
        bad "the free allowance is not gated to the free plan at all — a paying tier would be metered"
    else
        bad "the allowance still accepts the retired \`eco\` tier — the row is gone, so the branch is dead weight"
    fi

    # (e) TWO-SIDED: the client must read the same key the hub writes. A hub key
    #     with no client reader is a silent no-op, which is precisely how the
    #     UoT endpoint stayed dead fleet-wide (FIXES.md 29). The heartbeat
    #     response struct is where the wire keys are declared.
    CLIENT_HB="$REPO/client/src-tauri/src/locus/heartbeat.rs"
    if [ -f "$CLIENT_HB" ]; then
        # Anchor on the FIELD DECLARATION, not a bare mention: the doc comments
        # and tests also name the key, so a plain grep passes even when the
        # field itself has been renamed — a check that cannot fail is the exact
        # defect this whole section exists to avoid.
        if grep -Eq '^[[:space:]]*pub free_allowance_mb:' "$CLIENT_HB"; then
            ok "the client declares free_allowance_mb (hub key has a reader)"
        else
            bad "the hub sends free_allowance_mb but the client does not declare it — the allowance would be a silent no-op"
        fi
        if grep -Eq '^[[:space:]]*pub free_throttle_mbps:' "$CLIENT_HB"; then
            ok "the client declares free_throttle_mbps (hub key has a reader)"
        else
            bad "the hub sends free_throttle_mbps but the client does not declare it — the throttle speed would be a silent no-op"
        fi
    else
        warn "no client heartbeat.rs found — cannot check the client side of the allowance"
    fi

    # (f) The cached allowance/throttle must be READ back, not only written.
    #     A `store_*` with no reader is the same class of silent no-op as a wire
    #     key with no reader: the value arrives, is persisted, and nothing ever
    #     uses it. `throttle_mbps` is the live example of this being worth
    #     checking — it is deliberately stored-but-not-yet-applied, and the guard
    #     makes that an explicit, greppable fact rather than a missing call.
    CLIENT_STORE="$REPO/client/src-tauri/src/locus/store.rs"
    if [ -f "$CLIENT_STORE" ]; then
        if grep -Eq 'pub async fn throttle_mbps\(' "$CLIENT_STORE"; then
            ok "the client can read back the stored throttle speed"
        else
            bad "the client stores the throttle speed with no reader — the value would be persisted and never used"
        fi
    fi

    # (g) THE PROSE ABOUT THE CAPS, not just the caps.
    #
    # WHY THIS EXISTS. The (a)–(f) checks all passed while two DOCUMENTS
    # restated the caps wrongly: the root `README.md` said "Eco 5Mbit ... tc caps
    # (5/100/200 Mbps)" — the pre-rename value the cap was CHANGED FROM — and
    # `04-tc.sh`'s own header comment said "Eco (port 8443): 5 Mbps" above code
    # that applies 1mbit. The guard checked the code and the docs drifted in
    # silence, which is the §7 lesson one level out: verifying the value is not
    # verifying what the documents SAY about it.
    #
    # The value is in `state.toml` ([tiers.free].cap). These two files are the
    # ones a reader meets first (the front door, and the module that owns the
    # cap), so they are the ones pinned here. A prose copy that reintroduces the
    # old number now fails the build rather than shipping.
    README_MD="$REPO/README.md"
    if [ -f "$README_MD" ]; then
        # The merged ladder: free 1 Mbps, paid 100 Mbps. NOTE this literal moved
        # from (1/100/200) to (1/100) in the same commit as every other file that
        # restated it — that is this guard doing its job, not a guard breaking.
        # The paid-tier half of the same agreement is asserted by §27(g).
        if grep -Eq 'tc caps \(1/100 Mbps\)' "$README_MD"; then
            ok "README.md's tier-cap summary agrees with state.toml (1/100 Mbps)"
        else
            bad "README.md restates the tier caps and disagrees with state.toml — expected 'tc caps (1/100 Mbps)'"
        fi
        if grep -Eq 'Eco 5Mbit' "$README_MD"; then
            bad "README.md still calls the free/Eco tier '5Mbit' — the cap was lowered to 1mbit"
        else
            ok "README.md does not restate the retired 5Mbit free-tier cap"
        fi
    fi
    # The module header comment must not contradict the code below it. Match the
    # retired "Eco (port 8443): 5 Mbps" line specifically; the applied caps are
    # checked by (a)/(b) above and by §27(a)/(b).
    if grep -Eq 'Eco \(port 8443\): 5 Mbps' "$TC_MOD"; then
        bad "04-tc.sh's header comment says 'Eco (port 8443): 5 Mbps' but the code below applies 1mbit"
    else
        ok "04-tc.sh's header comment does not contradict its own applied cap"
    fi
fi

echo
echo "27. The paid tier is the only paid tier, and every surface agrees"
#
# WHY THIS EXISTS
#
# §23 does this for the FREE tier. The paid tier had no such treatment, and that
# is how Stealth and Strike drifted into being the same plan at two rates for
# months without anything noticing: four files described the paid endpoint
# (`04-tc.sh` cap, `02-shadowsocks.sh` service, `seed-pb.py` row + udp_relay,
# `Codes.vue` dropdown) in three vocabularies, and no check asserted they AGREED.
#
# The merge (docs/business/04-tiers.md §4.2.4) collapses those to one tier, and
# this holds the collapse in place. It is written as a set of AGREEMENTS plus a
# NEGATIVE, because the failure modes here are:
#
#   1. Someone re-adds a 200mbit class or a `stealth` service because a stale
#      comment said the ladder had three tiers.
#   2. The cap in the applied class and the cap in the reboot unit disagree, so
#      a reboot silently restores the old rate.
#   3. The console offers a tier the hub does not seed, so the operator can mint
#      a code that can never resolve.
#
# Every assertion below was observed FAILING against the pre-merge tree before it
# was kept, and against each half-merged state (the 200mbit cap, the surviving
# `stealth` seed row, the console's stale dropdown literal).
PAID_PORT=8445
PAID_CAP=100mbit
PAID_ROW=strike
CONSOLE_CODES="$REPO/server/console/src/views/Codes.vue"

if [ ! -f "$TC_MOD" ] || [ ! -f "$SEED" ]; then
    warn "paid-tier guard: a file it needs is missing — skipping"
else
    # (a) The applied cap. Anchored on the APPLY LINE, not a bare mention: the
    #     number also appears in comments, so a plain grep stays green with the
    #     assignment deleted — a check that cannot fail.
    if grep -Eq "apply_tc_now[[:space:]]+${PAID_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${PAID_CAP}\"" "$TC_MOD"; then
        ok "04-tc.sh caps the paid port (${PAID_PORT}) at ${PAID_CAP}"
    else
        bad "04-tc.sh does NOT cap port ${PAID_PORT} at ${PAID_CAP} (expected: apply_tc_now ${PAID_PORT} \"1:30\" \"${PAID_CAP}\")"
    fi

    # (b) The reboot unit uses the same rate, or a reboot silently restores the
    #     old cap while the running class says otherwise.
    if grep -Eq "create_tc_service[[:space:]]+\"${PAID_ROW}\"[[:space:]]+\"[0-9:]+\"[[:space:]]+\"${PAID_CAP}\"[[:space:]]+${PAID_PORT}" "$TC_MOD"; then
        ok "04-tc.sh's tc-${PAID_ROW}-cap.service uses the same ${PAID_CAP} rate"
    else
        bad "04-tc.sh's tc-${PAID_ROW}-cap.service rate disagrees with the applied cap"
    fi

    # (c) THE NEGATIVE — the retired rate must not come back. This is the
    #     property that actually protects the fleet: `200mbit` anywhere in the
    #     module means someone has restored the tier the merge deleted.
    if grep -Eq '200mbit' "$TC_MOD"; then
        bad "04-tc.sh still names a 200mbit cap — the retired second paid tier has come back"
    else
        ok "04-tc.sh names no retired 200mbit cap"
    fi

    # (d) The seed carries the paid row, with UDP advertised. Assert the ROW and
    #     the udp flag together: a row without udp_relay sells a gaming plan that
    #     does not do gaming, which is the FIXES.md-29 class of silent no-op.
    if grep -Eq "\(\"${PAID_ROW}\",[[:space:]]*\"STRIKE_PASS\",[[:space:]]*${PAID_PORT}\)" "$SEED"; then
        ok "seed-pb.py seeds the \`${PAID_ROW}\` row on port ${PAID_PORT}"
    else
        bad "seed-pb.py has no \`${PAID_ROW}\` row on port ${PAID_PORT} — a paid code could not resolve"
    fi
    # INVERTED 2026-10. This assertion used to REQUIRE the paid-only gate
    # (`udp_enabled and t == "strike"`), because UDP was the paid plan's
    # differentiator. UDP is now a given on both plans and the differentiator is
    # RATE, so the gate it demanded is precisely the defect. The stronger
    # two-sided form now lives at (h): every row advertises UDP, and each row
    # names its own listener. Kept here as a TOMBSTONE rather than deleted, so a
    # reader who remembers the old gate finds out why it is gone instead of
    # re-adding it.
    if grep -Eq 'uot_enabled and t == "strike"' "$SEED"; then
        bad "seed-pb.py gates UDP to the paid row — UDP is a given on BOTH plans now (see §27(h)); the differentiator is rate"
    else
        ok "seed-pb.py does not gate UDP to the paid row (both plans carry it)"
    fi

    # (e) The retired tier must not be SEEDED. Its row may survive in a live DB
    #     (field codes may carry the string), but a fresh seed must not recreate
    #     it — that is how a retired tier silently becomes sellable again.
    if grep -Eq '\("stealth\",[[:space:]]*"STEALTH_PASS"' "$SEED"; then
        bad "seed-pb.py still seeds a \`stealth\` row — the retired tier is sellable again on a fresh deploy"
    else
        ok "seed-pb.py seeds no retired \`stealth\` row"
    fi

    # (f) TWO-SIDED, the console. The dropdown literal is a PRE-LOAD placeholder
    #     (Codes.vue hydrates it from tiers.list), so it is not an allow-list in
    #     the way a deployed client's enum is — but a placeholder naming a
    #     retired tier is a dropdown entry that can mint an unresolvable code on
    #     first paint. Assert it names only seeded rows.
    if [ -f "$CONSOLE_CODES" ]; then
        if grep -Eq "ref<string\[\]>\(\[[^]]*'${PAID_ROW}'" "$CONSOLE_CODES"; then
            ok "the console's tier placeholder names the paid row"
        else
            bad "the console's tier placeholder does not name '${PAID_ROW}' — the mint form could offer no valid paid tier on first paint"
        fi
        if grep -Eq "ref<string\[\]>\(\[[^]]*'stealth'" "$CONSOLE_CODES"; then
            bad "the console's tier placeholder still names the retired 'stealth' tier"
        else
            ok "the console's tier placeholder names no retired tier"
        fi
    else
        warn "no console Codes.vue found — cannot check the tier placeholder"
    fi

    # (h) UDP IS A GIVEN — BOTH PLANS ADVERTISE IT, ON SEPARATE PORTS.
    #
    # WHY THIS EXISTS. UDP stopped being the paid plan's differentiator: the two
    # plans are separated by RATE (100 Mbps vs 1 Mbps), not by whether a UDP path
    # exists. That makes three things load-bearing at once, and each has a silent
    # failure mode:
    #
    #   1. `seed-pb.py` must set udp_relay for EVERY row, not just `strike`. The
    #      old gate (`t == "strike"`) is a one-word edit away from coming back,
    #      and the symptom would be "free users can't play Roblox" — noticed by
    #      students, not by CI.
    #   2. Each row must name its OWN uot_port. A sing-box listener is bound to
    #      one password, so pointing the free row at the paid port sends ECO_PASS
    #      to a listener expecting STRIKE_PASS: the handshake fails and UDP dies
    #      for exactly one plan.
    #   3. The free UDP port must be SHAPED. Without a tc class on it, the free
    #      plan's UDP is bounded only by the client's own cap — the hub trusting
    #      the thing it is supposed to be shaping, and a modified client gets
    #      uncapped UDP. That is the security half of this change, and the reason
    #      the user asked for a server-side cap.
    #
    # Every assertion below was observed FAILING against the pre-change tree.
    UOT_FREE_PORT=8447
    FW="$REPO/server/modules/08-firewall.sh"

    if grep -Eq 'udp = uot_enabled$' "$SEED"; then
        ok "seed-pb.py advertises UDP for every plan (no paid-only gate)"
    else
        bad "seed-pb.py does NOT advertise UDP for every plan — the paid-only gate is back, and free users lose UDP silently"
    fi

    if grep -Eq 'uot_port if t == "strike" else uot_port_free' "$SEED"; then
        ok "seed-pb.py gives each plan its own uot_port (free -> ${UOT_FREE_PORT})"
    else
        bad "seed-pb.py does not select a per-plan uot_port — one plan would be told to use the other's listener"
    fi

    if grep -Eq "apply_tc_now[[:space:]]+${UOT_FREE_PORT}[[:space:]]+\"[0-9:]+\"[[:space:]]+\"1mbit\"" "$TC_MOD"; then
        ok "04-tc.sh caps the free UDP port (${UOT_FREE_PORT}) at 1mbit, server-side"
    else
        bad "04-tc.sh does NOT cap the free UDP port (${UOT_FREE_PORT}) — free UDP would be bounded only by the client, which a modified client controls"
    fi

    if grep -Eq "create_tc_service[[:space:]]+\"eco-udp\"[[:space:]]+\"[0-9:]+\"[[:space:]]+\"1mbit\"[[:space:]]+${UOT_FREE_PORT}" "$TC_MOD"; then
        ok "04-tc.sh's tc-eco-udp-cap.service uses the same 1mbit rate"
    else
        bad "04-tc.sh's free-UDP reboot unit disagrees with the applied cap"
    fi

    # The paid UDP port must NOT be shaped: capping a latency-sensitive flow to
    # protect bandwidth is the wrong trade for a plan sold on 100 Mbps.
    if grep -Eq "apply_tc_now[[:space:]]+8446" "$TC_MOD"; then
        bad "04-tc.sh shapes the PAID UDP port (8446) — that can add latency to exactly the traffic the paid plan exists for"
    else
        ok "04-tc.sh does not shape the paid UDP port"
    fi

    # Two-sided: the firewall must admit both listeners, or clients sit on an
    # advertised port that nothing answers.
    # ANCHORED ON THE COMMAND, not on a bare mention of the port variable.
    #
    # The first draft of this check grepped for `${UOT_PORT_FREE:-8447}` and
    # stayed GREEN when both `ufw allow` lines were deleted — because the port
    # also appears in the surrounding `log` lines. That is the same
    # "a check on a value is not a check on what the code does with it" defect
    # §23(f) and the macOS section both record, so it is anchored on `ufw allow`
    # now and was re-observed failing against the deleted-rule tree.
    if [ -f "$FW" ]; then
        if grep -Eq '^[[:space:]]*ufw allow[[:space:]]+"\$\{UOT_PORT_FREE:-8447\}"/(tcp|udp)' "$FW"; then
            ok "08-firewall.sh actually opens the free UDP port (8447) with ufw"
        else
            bad "08-firewall.sh does not open the free UDP port with ufw allow — the hub would advertise it and refuse it"
        fi
    fi

    # (i) THE UNIT NAMES `setup.sh` STARTS MUST BE CREATED BY A MODULE.
    #
    # WHY THIS EXISTS. `setup.sh` keeps its own SERVICES list, and after the
    # per-plan UoT split that list still named the retired `sing-box-uot`. The
    # modules were correct, the listeners worked, and the only symptom was
    # "sing-box-uot failed to start" in the deploy log and a smoke test failing
    # on a service that does not exist. That is noise which teaches an operator
    # to skim deploy output — how a real failure gets missed.
    #
    # HOW IT IS CHECKED. The unit names are TEMPLATED (`tc-${name}-cap.service`,
    # `sing-box-uot-${name}.service`, `shadowsocks-${tier}.service`), so grepping
    # for the final filename cannot work — the first draft of this check was a
    # regex that matched everything and therefore could never fail, which is the
    # defect it exists to catch. Instead each name in SERVICES is decomposed into
    # the TEMPLATE it must come from plus the ARGUMENT that must have been passed,
    # and the argument is looked up in the module that owns that template.
    SETUP="$REPO/server/setup.sh"
    if [ -f "$SETUP" ]; then
        # BOTH assignment forms, because the first draft read only
        # `SERVICES="..."` and silently missed `SERVICES="${SERVICES} ..."` — the
        # line the UoT units are added on. That is why the check passed while the
        # retired `sing-box-uot` was still in the list: it was never extracted, so
        # there was nothing to fail on. A verifier that cannot see the value it
        # checks is the §23(f) defect again.
        _svc_list="$(
            sed -n 's/^[[:space:]]*SERVICES=\(.*\)$/\1/p' "$SETUP" \
                | sed 's/^"//; s/"$//; s/\${SERVICES}//; s/[{}]//g' \
                | tr ' ' '\n' | grep -v '^$'
        )"
        _unbacked=""
        for _svc in $_svc_list; do
            case "$_svc" in
                tc-*-cap)
                    _arg="${_svc#tc-}"; _arg="${_arg%-cap}"
                    grep -qE "create_tc_service[[:space:]]+\"${_arg}\"" "$TC_MOD" \
                        || _unbacked="$_unbacked $_svc" ;;
                sing-box-uot-*)
                    _arg="${_svc#sing-box-uot-}"
                    grep -qE "write_uot_service[[:space:]]+\"${_arg}\"" \
                        "$REPO/server/modules/02-shadowsocks.sh" \
                        || _unbacked="$_unbacked $_svc" ;;
                shadowsocks-*)
                    _arg="${_svc#shadowsocks-}"
                    grep -qE "write_service[[:space:]]+\"${_arg}\"" \
                        "$REPO/server/modules/02-shadowsocks.sh" \
                        || _unbacked="$_unbacked $_svc" ;;
                # ANYTHING ELSE IS A FAILURE, not a skip.
                #
                # The first draft used `*) : ;;` here — so a name that did not
                # match one of the three templates (an old `sing-box-uot`, a
                # typo, a unit some future module invents) fell through and was
                # never checked. That is allow-list blindness: the check
                # confirmed only the names it already understood, which is
                # precisely the set that cannot be wrong. Observed passing on a
                # deliberately stale `sing-box-uot`.
                *)
                    _unbacked="$_unbacked $_svc" ;;
            esac
        done
        if [ -n "$_unbacked" ]; then
            bad "setup.sh starts service(s) no module creates:${_unbacked}"
            bad "  each name must match a write_service / write_uot_service / create_tc_service argument"
        else
            ok "every service setup.sh starts is created by a module"
        fi
    fi

    # (g) THE PROSE ABOUT THE PAID CAP, not just the cap (§23(g)'s lesson one
    #     tier over). Two documents restate it and both are ones a reader meets
    #     first. A prose copy that reintroduces the retired 200 Mbps now fails.
    if [ -f "$REPO/README.md" ]; then
        if grep -Eq 'tc caps \(1/100 Mbps\)' "$REPO/README.md"; then
            ok "README.md's tier-cap summary agrees with the merged ladder (1/100 Mbps)"
        else
            bad "README.md restates the tier caps and disagrees with the tree — expected 'tc caps (1/100 Mbps)'"
        fi
        if grep -Eq 'Stealth [0-9]+ ?Mbps|5M tc' "$REPO/README.md"; then
            bad "README.md still restates a retired tier's cap"
        else
            ok "README.md does not restate a retired tier's cap"
        fi
    fi
fi


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
INSTALL_RS="$REPO/client/src-tauri/src/locus/update/install.rs"
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
    if printf '%s\n' "$release_files" | grep -q 'locus-darwin-.*\.app\.tar\.gz$'; then
        ok "the release publishes the macOS .app.tar.gz assets"
    else
        bad "the release's file list omits the macOS .app.tar.gz — the manifest would advertise files the release does not carry"
    fi
    #      And its signature, for the reason in (f2).
    if printf '%s\n' "$release_files" | grep -q 'locus-darwin-.*\.app\.tar\.gz\.sig$'; then
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
    #     locus-darwin-*.app.tar.gz". The manifest guard caught it correctly, but
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

# ─────────────────────────────────────────────────────────────
# 25. Front-matter conformance — status is structure, not a banner.
#
# WHY THIS EXISTS
#
# `docs/README.md` rule 1 and `docs/STATE.md` rule 3 both say a document's status
# is a front-matter FIELD (`audience:` / `status:`), not a prose banner, and that
# a retired document lives in `archive/`/`history/`, where the FOLDER carries the
# status. Nothing enforced it, so the rule held only where someone remembered it.
#
# It came apart in the redesign corpus: 17 files under `docs/business/redesign/`
# and its `implementation/` subdirectory had a banner but no `status:` field, so
# a reader — and any guard — had to parse prose to learn whether a file described
# live code or a removed design. Three of them described the *removed* device
# binding as if live. That is exactly the `dev-vs-code-drift` class this repo
# keeps re-learning: the doc was right when written and the world moved.
#
# WHAT IT ENFORCES, and why each is safe to require:
#   * Every `.md` under `docs/` and `client/docs/` EXCEPT `archive/` and
#     `history/` opens with a fenced front-matter block carrying `audience:` and
#     `status:`.
#   * A file under `archive/` or `history/` need NOT (the folder is the status) —
#     but if it DOES, that is fine (LAYOUT.md keeps its own).
#   * `status:` is one of the three values the rule names.
#   * Also: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` at the repo root are
#     exempt by path — they are the entry points and predate the convention.
#
# ─────────────────────────────────────────────────────────────
# 26. The landing page: one hostname, four download slots, two sides agreeing.
#
# WHY THIS EXISTS
#
# The landing page is a second hostname on the hub box, and it introduces exactly
# the class of drift this script is for. Two things must agree and NOTHING in
# the tree made them:
#
#   (a) THE HOSTNAME. It is written in four places with four different jobs:
#       the Caddyfile generator (which block to serve), the manual-reference
#       template (so a hand-built hub matches), docs/state.toml (the recorded
#       fact), and the page itself (the canonical link, which is what a search
#       engine treats as the page's identity). Three of those can be edited
#       without the fourth, and the failure is quiet: a canonical link naming a
#       host that serves nothing tells Google the real page is elsewhere.
#
#   (b) THE DOWNLOAD FILENAMES. The page's four buttons are rendered from the
#       release manifest's platforms map — the same map publish-release.sh's
#       PLATFORMS list and fetch-release.py resolve against. The macOS entries
#       name .app.tar.gz tarballs, and the Windows entry is the NSIS installer
#       rather than the raw .exe. Getting one wrong is not a visible break: the
#       button renders, the page loads, and the download 404s.
#
# THIS SECTION CHECKS THE AGREEMENT, NOT EACH SIDE. That is section 24's lesson,
# applied here: a check that each half is individually valid passes happily while
# the two halves disagree — which is how the v3.2.12 Windows outage shipped.
# ─────────────────────────────────────────────────────────────
echo
echo "26. The landing page — hostname and download slots agree everywhere"

# Read the entry by scanning from its header to the next header, NOT a fixed
# number of lines: this entry carries a long comment block, and a fixed offset
# silently returned nothing (the guard fired with "<missing>" on its first run,
# which is how the bug was found — worth keeping the shape that exposed it).
#
# The value is taken with an explicit quote split rather than a greedy gsub: the
# first attempt used `gsub(/.*"|".*/,"")`, which strips the whole line because
# the second alternative matches a quote followed by anything, including the
# closing quote and the value between them.
LANDING_TOML=$(awk '/^\[site\.domain\]/{f=1;next} /^\[/{f=0} f && /^value/{n=split($0,a,"\"");print a[2];exit}' \
    "$REPO/docs/state.toml" 2>/dev/null)
LANDING_MODULE=$(grep -oE 'LANDING_DOMAIN="\$\{LANDING_DOMAIN:-[^}]*\}"' "$REPO/server/modules/05-caddy.sh" 2>/dev/null \
    | sed 's/.*:-\(.*\)}"/\1/' | head -1)
LANDING_TEMPLATE=$(grep -oE '^[a-z0-9.-]+ \{' "$REPO/server/templates/Caddyfile" 2>/dev/null \
    | grep -vi 'domain' | sed 's/ {//' | head -1)
LANDING_PAGE=$(grep -oE '<link rel="canonical" href="https://[^/"]+' "$REPO/server/site/index.html" 2>/dev/null \
    | sed 's|.*https://||' | head -1)

echo "  state.toml      : ${LANDING_TOML:-<missing>}"
echo "  05-caddy.sh     : ${LANDING_MODULE:-<missing>}"
echo "  templates/      : ${LANDING_TEMPLATE:-<missing>}"
echo "  site/index.html : ${LANDING_PAGE:-<missing>}"

if [ -z "$LANDING_TOML" ] || [ -z "$LANDING_MODULE" ] || [ -z "$LANDING_TEMPLATE" ] || [ -z "$LANDING_PAGE" ]; then
    bad "landing hostname could not be read from all four sites — one is missing"
else
    LAND_OK=1
    [ "$LANDING_MODULE" = "$LANDING_TOML" ]   || { bad "05-caddy.sh LANDING_DOMAIN='${LANDING_MODULE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
    [ "$LANDING_TEMPLATE" = "$LANDING_TOML" ] || { bad "templates/Caddyfile block '${LANDING_TEMPLATE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
    [ "$LANDING_PAGE" = "$LANDING_TOML" ]     || { bad "site/index.html canonical '${LANDING_PAGE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
    [ "$LAND_OK" = "1" ] && ok "landing hostname agrees across all four sites"
fi

# The four slots must exist in the page, one per FROZEN platform key. A missing
# slot is three buttons instead of four, which no other check would notice.
MISSING_SLOTS=""
for k in linux windows macos_intel macos_arm; do
    grep -q "data-download=\"${k}\"" "$REPO/server/site/index.html" 2>/dev/null \
        || MISSING_SLOTS="$MISSING_SLOTS $k"
done
if [ -n "$MISSING_SLOTS" ]; then
    bad "server/site/index.html is missing download slot(s):${MISSING_SLOTS}"
    bad "  Each platform key needs one — they are the keys section 1 pins."
else
    ok "index.html carries all four download slots"
fi

# And the renderer must cover the same set. A slot the renderer does not know
# about ships as href="#" — a dead button on a page that otherwise looks right.
RENDER_SLOTS=$(grep -oE '^SLOTS = \[.*\]' "$REPO/server/scripts/deploy-site.sh" 2>/dev/null \
    | grep -oE '"[a-z_]+"' | tr -d '"' | sort | tr '\n' ' ')
EXPECT_SLOTS="linux macos_arm macos_intel windows "
if [ "$RENDER_SLOTS" != "$EXPECT_SLOTS" ]; then
    bad "deploy-site.sh SLOTS='${RENDER_SLOTS:-<missing>}' != expected '${EXPECT_SLOTS}'"
    bad "  The renderer and the page must cover the same four platform keys."
else
    ok "deploy-site.sh renders the same four slots the page carries"
fi

# The landing block must never serve the hub's surfaces. This is the property a
# future "just add a handle" edit removes with no symptom on the page itself.
if awk '/^locusvpn\.jadedns\.uk \{/,/^\}/' "$REPO/server/templates/Caddyfile" 2>/dev/null \
    | grep -qE '^[[:space:]]*(handle|handle_path|reverse_proxy|try_files)'; then
    bad "the landing block in templates/Caddyfile contains a handle/proxy directive"
    bad "  It must be a bare file_server: the hub's /updates/, /api/* and /admin/"
    bad "  must not be reachable from the indexable hostname."
else
    ok "landing block serves static files only (no handle, no proxy)"
fi

# ─────────────────────────────────────────────────────────────
# 25. Front-matter conformance (status is structure, not a banner)
#
# WHAT IT DOES NOT ENFORCE: that the declared status is TRUE. A guard cannot read
# a design record and know it is superseded. This catches the missing field and
# the invented value; the honest value is still a human's job.
# ─────────────────────────────────────────────────────────────
echo
echo "25. Front-matter conformance (status is structure, not a banner)"
fm_report=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys

ALLOWED_STATUS = {"live", "reference", "design-record"}
missing, badvalue = [], []

def md_files(root):
    for dirpath, dirs, files in os.walk(root):
        if any(part in dirpath for part in ('node_modules', 'target', '.git', 'dist')):
            continue
        for fn in files:
            if fn.endswith('.md'):
                yield os.path.join(dirpath, fn)

def front_matter(path):
    """The keys in the leading fenced block, or None if there is no block."""
    with open(path, encoding="utf-8", errors="replace") as fh:
        lines = fh.read().splitlines()
    # Skip a leading H1 (the docs put the front-matter block AFTER the title).
    i = 0
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i < len(lines) and lines[i].startswith("# "):
        i += 1
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i >= len(lines) or not lines[i].startswith("```"):
        return None
    i += 1
    keys = {}
    while i < len(lines) and not lines[i].startswith("```"):
        m = re.match(r'^([a-z-]+):\s*(.*)$', lines[i].strip())
        if m:
            keys[m.group(1)] = m.group(2).strip()
        i += 1
    return keys

for root in ("docs", "client/docs"):
    for path in md_files(root):
        rel = path.replace(os.sep, "/")
        # Archived/historical material: the folder carries the status.
        if rel.startswith("docs/archive/") or rel.startswith("docs/history/"):
            continue
        keys = front_matter(path)
        if keys is None:
            missing.append(rel)
            continue
        if "audience" not in keys:
            missing.append(rel + "  (no `audience:`)")
        if "status" not in keys:
            missing.append(rel + "  (no `status:`)")
        elif keys["status"] not in ALLOWED_STATUS:
            badvalue.append(f"{rel}  status = {keys['status']!r} (allowed: {sorted(ALLOWED_STATUS)})")

for f in sorted(set(missing)):
    print("MISSING\t" + f)
for f in sorted(set(badvalue)):
    print("BADVALUE\t" + f)
PY
)

if [ -z "$fm_report" ]; then
    ok "every live document declares audience: and status: in its front-matter"
else
    while IFS="$(printf '\t')" read -r kind detail; do
        case "$kind" in
            MISSING)  bad "no front-matter (or a missing key): $detail" ;;
            BADVALUE) bad "invalid status: $detail" ;;
        esac
    done <<EOF
$fm_report
EOF
    bad "Fix: give each file a fenced block after its H1 with \`audience:\` and"
    bad "\`status: live|reference|design-record\` — or move it to archive/history."
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