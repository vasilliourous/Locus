#!/usr/bin/env bash
# Section 22 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_22() {
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
    echo "22. hooks-sync.sh defines its helpers before use"

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
}
