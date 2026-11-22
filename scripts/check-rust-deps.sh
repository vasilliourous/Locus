#!/usr/bin/env bash
#
# 53. No `[dependency]` in a manifest is dead
#
# WHY THIS EXISTS: `cargo clippy ... -D warnings` CANNOT SEE THIS CLASS. The
# warnings below are emitted by *cargo*, not by rustc, and `-D warnings` is a
# rustc flag. A manifest could accumulate a dependency nothing ever used and the
# Linux gate would report **exit 0** while printing the warning to a log nobody
# reads. That is how `tauri-build` sat reported-but-passing long enough for the
# handover to record it as "a fourth gate nobody named" (audit F-21).
#
# The lints, each of which is `warn` by default and therefore never fatal:
#
#   unused_dependencies             a `[dependencies]` entry no code in the crate names
#   unused_workspace_dependencies   a `[workspace.dependencies]` entry no member uses
#
# `--all-targets` is NOT optional here. It is what makes cargo look at tests
# before deciding a dependency is unused, so a crate whose only reference is a
# dev-dependency helper is not reported (and we do not have to add the lint to
# `[dev-dependencies]` on every crate to silence a false positive).
#
# WHAT IT DOES NOT COVER: `missing [lints] to inherit [workspace.lints]`, which
# cargo also reports as a manifest warning. That one is a style instruction about
# inheritance, not a dead dependency, and turning it on would fail on three crates
# for a reason unrelated to this check. Stated so the gap is visible.
#
# The count printed on success is the point of the guard: "0 dead" from a search
# that ran, not an empty result from a search that never ran. If cargo did not
# scan, this reports BAD rather than OK.
#
# See docs/reference/STILL-OPEN.md, audit F-21.

set -uo pipefail

RED=$'\033[0;31m'; GREEN=$'\033[0;32m'; YELLOW=$'\033[0;33m'; RESET=$'\033[0m'
pass=0 fail=0
ok()   { pass=$((pass+1)); printf '  %sOK%s    %s\n' "$GREEN" "$RESET" "$1"; }
bad()  { fail=$((fail+1)); printf '  %sBAD%s   %s\n' "$RED" "$RESET" "$1"; }
warn() { printf '  %sWARN%s  %s\n' "$YELLOW" "$RESET" "$1"; }

root=${1:-$(cd "$(dirname "$0")/.." && pwd)}

if ! command -v cargo >/dev/null 2>&1; then
  warn "cargo not on PATH — dead-dependency lint NOT run (this is a gap, not a pass)"
  exit 0
fi

# The scan is a type-check, not a build: `cargo check` is the cheapest thing that
# still runs the manifest lints. `--all-targets` keeps dev-dependency helpers from
# reading as unused. Output is captured because the lints are warnings — a bare
# exit code proves nothing.
out=$(cd "$root/client" && cargo check --all-targets 2>&1)
rc=$?

if [ "$rc" -ne 0 ]; then
  bad "cargo check itself failed — the dependency scan did not complete"
  printf '%s\n' "$out" | grep -E '^error' | head -5 | sed 's/^/          /'
  printf '  %s\n' "$pass passed, $fail failed"
  exit 1
fi

dead=$(printf '%s\n' "$out" | grep -E '^warning: unused (workspace )?dependency' || true)
n=$(printf '%s' "$dead" | grep -c . || true)

if [ "${n:-0}" -eq 0 ]; then
  ok "0 dead dependencies across the client workspace (cargo scanned every target)"
else
  bad "$n dead dependenc$([ "$n" -eq 1 ] && echo y || echo ies) — cargo reports them as warnings, so no compiler gate can fail on them:"
  printf '%s\n' "$dead" | sed 's/^warning: /          /' | head -20
fi

printf '  %s\n' "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
