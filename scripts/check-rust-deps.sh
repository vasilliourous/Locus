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
# WHERE IT MUST RUN: a job that has run `pnpm prebuild`. The scan compiles the
# chrysalis build script, `tauri_build` reads the gitignored `sidecar/` binaries, and
# without them it panics — so this is not a tree condition any job can skip, it is
# a precondition of the check. See the precondition block below the cargo lookup.
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
#
# ⚠️ THIS NEEDS THE SIDECAR, SO IT NEEDS `prebuild` FIRST.
#
# `tauri_build` copies `bundle.externalBin` (`sidecar/verge-mihomo`, …) into the
# target dir during the chrysalis build script, and those files are gitignored in
# full — `prebuild.mjs` is what fetches them. In a tree where `prebuild` has not
# run, `tauri_build` panics and this scan fails with a message about a proc macro
# rather than about a missing engine.
#
# That is exactly what happened in CI: this script was placed in `verify`, which
# never runs `prebuild`, so the gate reported "cargo check itself failed" on
# EVERY run from the moment it was added (431abd5) — and `release` needs
# `[verify, build]`, so no release could publish. `Build (linux)` runs
# `prebuild`, so the check belongs there.
#
# Said here rather than only in the workflow, because the next person to wire this
# into a job needs to know the precondition, not just that it once broke.
sidecar="$root/client/src-tauri/sidecar"
if [ ! -d "$sidecar" ] || ! ls "$sidecar" >/dev/null 2>&1 || [ -z "$(ls -A "$sidecar" 2>/dev/null)" ]; then
  bad "client/src-tauri/sidecar/ is empty or missing — run \`pnpm prebuild\` first"
  printf '          %s\n' \
    "tauri_build reads bundle.externalBin from it and panics when it cannot," \
    "so the dependency scan would report a proc-macro panic instead of a dead dep." \
    "This check belongs in a job that runs prebuild (e.g. Build (linux))."
  printf '  %s\n' "$pass passed, $fail failed"
  exit 1
fi

out=$(cd "$root/client" && cargo check --all-targets 2>&1)
rc=$?

if [ "$rc" -ne 0 ]; then
  bad "cargo check itself failed — the dependency scan did not complete"
  # Print WHY, not just the verdict. This used to grep '^error' and take five
  # lines, which dropped the panic's own message and the file it named, so the
  # log said "cargo check itself failed" and nothing about `sidecar/`. A failure
  # report that cannot name its cause is the same class as a guard that cannot
  # fail — it reads as a diagnosis and is only a symptom.
  printf '%s\n' "$out" | grep -E 'error|panicked|does not exist|could not compile' \
    | head -15 | sed 's/^/          /'
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
