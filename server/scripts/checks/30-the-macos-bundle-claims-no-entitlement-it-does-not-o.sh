#!/usr/bin/env bash
# Section 30 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_30() {
    echo "30. The macOS bundle claims no entitlement it does not own"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `client/src-tauri/packages/macos/entitlements.plist` is inherited from the
    # upstream Clash Verge Rev fork, and for the whole life of this fork nobody had
    # asked what its keys ASSERT about Chrysalis rather than about Verge. Two of them
    # asserted things that are not true of this bundle:
    #
    #   * `com.apple.security.inherit = true` — meaningful ONLY on a child process of
    #     a sandboxed parent. Chrysalis is `app-sandbox = false` and is not sandboxed, so
    #     the key says "my entitlements come from my parent" about a process that has
    #     no sandboxed parent. It is also the one entitlement whose validity is judged
    #     per SIGNATURE SLICE, which is why an identical .app can behave differently
    #     on two architectures. Found while diagnosing a macOS Intel report
    #     ("configuration is already running" / "lock preferences").
    #
    #   * `com.apple.security.application-groups` — naming
    #     `io.github.clash-verge-rev.clash-verge-rev`, ANOTHER PRODUCT'S identifier.
    #     This app's identifier is `com.locus.client` (tauri.macos.conf.json), nothing
    #     in the tree creates that group container, and an ad-hoc signature cannot
    #     carry a group it does not own.
    #
    # ── WHAT THIS CHECKS ──
    #
    #   (a) `com.apple.security.inherit` is absent, or is `false`. Present-and-true is
    #       a contradiction against `app-sandbox = false`, and this guard reads BOTH
    #       keys so the contradiction cannot be reintroduced by flipping one of them.
    #   (b) Any `application-groups` entry names an identifier this bundle OWNS — i.e.
    #       it must contain the app's own identifier prefix from tauri.macos.conf.json.
    #   (c) The file parses as a plist. A malformed entitlements file makes `codesign`
    #       fail at build time on a macOS runner, which is expensive to discover; the
    #       same file is cheap to parse here.
    #
    # ── HOW IT WAS SHOWN TO FAIL ──
    #
    # Restoring either removed key (inherit=true, the Verge group) produces a BAD line.
    # See docs/reference/FIXES.md for the entry; a guard that cannot fail reads exactly
    # like a guard that passed.
    # ─────────────────────────────────────────────────────────────
    ENTLMTS="$REPO/client/src-tauri/packages/macos/entitlements.plist"
    MACOS_CONF="$REPO/client/src-tauri/tauri.macos.conf.json"
    if [ ! -f "$ENTLMTS" ]; then
        bad "missing $ENTLMTS (the macOS bundle is configured to use it)"
    else
        entitlements_report=$(python3 - "$ENTLMTS" "$MACOS_CONF" <<'PY'
import json, plistlib, sys

ent_path, conf_path = sys.argv[1], sys.argv[2]
problems = []

try:
    with open(ent_path, "rb") as fh:
        ent = plistlib.load(fh)
except Exception as exc:  # noqa: BLE001 - any parse failure is the finding
    print("BAD\tdoes not parse as a plist: %s" % exc)
    raise SystemExit(0)

sandboxed = bool(ent.get("com.apple.security.app-sandbox", False))

# (a) inherit is only meaningful on a sandboxed child process.
inherit = ent.get("com.apple.security.inherit", False)
if inherit and not sandboxed:
    problems.append(
        "com.apple.security.inherit is true on a non-sandboxed bundle "
        "(app-sandbox is false) — it asserts a sandboxed parent this app does not have"
    )
if inherit and sandboxed:
    problems.append(
        "com.apple.security.inherit is true — Chrysalis is a top-level app, not a child "
        "process; remove it unless a sandboxed parent is introduced deliberately"
    )

# (b) an app group must be one this bundle may own.
try:
    with open(conf_path, encoding="utf-8") as fh:
        identifier = json.load(fh).get("identifier", "")
except Exception as exc:  # noqa: BLE001
    problems.append("could not read the app identifier from %s: %s" % (conf_path, exc))
    identifier = ""

for group in ent.get("com.apple.security.application-groups", []) or []:
    if not identifier:
        problems.append("application-groups entry %r cannot be checked: no app identifier" % group)
        continue
    # The bundle may only claim a group built on its own identifier family.
    family = ".".join(identifier.split(".")[:2])
    if identifier not in group and family not in group:
        problems.append(
            "application-groups entry %r is not this bundle's — the app identifier is %r"
            % (group, identifier)
        )

for problem in problems:
    print("BAD\t%s" % problem)
PY
    )
        if [ -z "$entitlements_report" ]; then
            ok "entitlements.plist parses and claims only entitlements this bundle owns"
        else
            while IFS="$(printf '\t')" read -r _kind message; do
                bad "ENTITLEMENTS: $message"
            done <<EOF
$entitlements_report
EOF
            bad "  File: client/src-tauri/packages/macos/entitlements.plist"
            bad "  An entitlement is a claim about what the process may do. Never inherit"
            bad "  one from the upstream fork without asking what it asserts about THIS"
            bad "  bundle — see the comment block in the file itself."
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
