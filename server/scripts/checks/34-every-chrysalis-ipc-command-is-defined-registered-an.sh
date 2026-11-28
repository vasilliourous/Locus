#!/usr/bin/env bash
# Section 34 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_34() {
    echo "34. Every Chrysalis IPC command is defined, registered, and called"

    # ── WHY THIS GUARD EXISTS ──
    #
    # A Tauri command has THREE sites that must agree, and none of them is checked by
    # the compiler:
    #
    #   1. the `#[tauri::command]` definition in Rust,
    #   2. its entry in the `generate_handler!` list in lib.rs,
    #   3. the `invoke('name')` call in the frontend.
    #
    # Miss (2) and the function compiles, the frontend calls it, and the call fails at
    # runtime with "command not found" — the exact class of failure this project keeps
    # finding in production because a green build proves nothing about a name crossing
    # a boundary. Miss (3) and the code is dead surface that looks maintained.
    #
    # ── WHAT THIS CHECKS ──
    #
    # The set of command names is identical across all three sites. Names are compared
    # AFTER camelCasing the Rust identifiers, because Tauri's default command name is
    # the function name verbatim — these are all already snake_case and the frontend
    # uses snake_case too, so the comparison is exact and any future rename that
    # forgets one site fails here.
    # ─────────────────────────────────────────────────────────────
    command_report=$(python3 - "$REPO" <<'PY'
import glob, re, sys

repo = sys.argv[1]
rust_sources = glob.glob(f"{repo}/client/src-tauri/src/**/*.rs", recursive=True)
lib = open(f"{repo}/client/src-tauri/src/lib.rs", encoding="utf-8").read()
ts_dir = f"{repo}/client/src"

defined = set()
for path in rust_sources:
    src = open(path, encoding="utf-8").read()
    for name in re.findall(r'#\[tauri::command\]\s*\npub (?:async )?fn (\w+)', src):
        # Scope to the product's own surface; upstream Verge commands have their
        # own callers and are not this project's contract to police.
        if name.startswith("chrysalis_"):
            defined.add(name)

registered = set(n for n in re.findall(r'\bcmd::(\w+)', lib) if n.startswith("chrysalis_"))

called = set()
for path in glob.glob(f"{ts_dir}/**/*.ts*", recursive=True):
    src = open(path, encoding="utf-8").read()
    for name in re.findall(r"invoke(?:<[^>]*>)?\(\s*['\"](chrysalis_\w+)['\"]", src):
        called.add(name)

# Some commands are called from JavaScript this repository INJECTS into a page
# rather than from a .ts file — `chrysalis_js_log` is the console bridge, called from
# the injected script in utils/resolve/window_script.rs. A guard that only read
# .ts/.tsx reported it as dead, which is a false positive of exactly the kind this
# project documents: the check was scoped narrower than the thing it claims to
# cover (§15's lesson, one layer down).
for path in rust_sources:
    src = open(path, encoding="utf-8").read()
    for name in re.findall(r"invoke\(\s*['\"](chrysalis_\w+)['\"]", src):
        called.add(name)

for name in sorted(defined - registered):
    print("BAD\t%s is defined but not registered in lib.rs — calling it fails at runtime" % name)
for name in sorted(registered - defined):
    print("BAD\t%s is registered but no command defines it" % name)
for name in sorted(defined - called):
    print("BAD\t%s is defined and registered but the frontend never calls it" % name)
for name in sorted(called - defined):
    print("BAD\tthe frontend invokes %s, which no command defines" % name)
PY
    )

    if [ -z "$command_report" ]; then
        ok "every Chrysalis IPC command is defined, registered and called"
    else
        while IFS="$(printf '\t')" read -r _kind message; do
            bad "IPC: $message"
        done <<EOF
$command_report
EOF
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
