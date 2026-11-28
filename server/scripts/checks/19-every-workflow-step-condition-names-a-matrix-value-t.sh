#!/usr/bin/env bash
# Section 19 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_19() {
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
}
