#!/usr/bin/env bash
# Section 39b -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_39b() {
    echo "39b. Every vendored build input is COMMITTED, not merely present"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `vendor/build-inputs/` holds the binaries the client cannot be built without.
    # On 2026-10-29 the first commit of this vendoring staged 9 of the 10 binaries and
    # looked complete: the Windows `.exe` files were silently excluded by the root
    # `.gitignore`'s blanket `*.exe`, a rule written to keep build output out. A fresh
    # clone of that commit would have rebuilt on Windows by downloading its engine,
    # and nothing in the repository would have said so — the vendoring would have
    # "worked" everywhere the developer happened to have a dirty tree.
    #
    # That is the failure the vendoring exists to stop, so it gets the check the
    # eye cannot do: for every group the lock declares, every file the lock pins must
    # be TRACKED BY GIT. Present-but-ignored is the specific state that fooled it.
    #
    # ── WHAT THIS CHECKS ──
    #
    #   * a `vendored` group whose directory exists must have every pinned file
    #     tracked — and a group named `resources` may be absent only while its
    #     directory is absent too (it is documented as declared-but-empty);
    #   * no file in `vendor/build-inputs/` may be untracked or ignored, because a
    #     file there that git does not carry is one a clone does not get.
    # ─────────────────────────────────────────────────────────────
    #
    # ANCHORED TO $REPO, NOT TO THE CALLER'S CWD. This block used to say
    # `VENDOR_ROOT="vendor/build-inputs"` and read `server/data/sidecar.lock.json`
    # relative to the working directory. The script is documented as runnable "from
    # anywhere", and `pnpm check:release` in `client/` is one of the callers — so
    # from `client/` the `-d` test failed on a path that does exist, and the guard
    # reported BAD against a correct tree. A guard that accuses the code because it
    # looked in the wrong place is the same class of defect as one that cannot fail:
    # both report a verdict the evidence does not support. Measured 2026-11-01 by
    # running from `client/` (BAD) and from the repo root (pass) on one commit.
    VENDOR_ROOT="$REPO/vendor/build-inputs"
    VENDOR_LOCK="$REPO/server/data/sidecar.lock.json"
    if [ ! -d "$VENDOR_ROOT" ]; then
        if [ -f "$VENDOR_LOCK" ] && python3 -c "import json,sys; sys.exit(0 if json.load(open('$VENDOR_LOCK')).get('vendored') else 1)" 2>/dev/null; then
            bad "the lock declares vendored inputs but $VENDOR_ROOT does not exist"
        else
            ok "no vendored build inputs declared"
        fi
    else
        vendor_report=$(python3 - "$VENDOR_ROOT" "$VENDOR_LOCK" <<'PYEOF'
import json, os, re, subprocess, sys
from pathlib import Path

root = Path(sys.argv[1])
lock = json.loads(Path(sys.argv[2]).read_text())
vendored = lock.get("vendored", {})
sorted_resources = " ".join(sorted(lock.get("resources", {})))

# `-z` gives NUL-separated paths. Splitting on a literal NUL is a mistake worth
# naming: the escape here previously became a REAL NUL byte in this file, which
# made the script binary to grep AND raised ValueError at runtime — and the
# caller saw empty output, which read as "no problems". A guard whose crash is
# indistinguishable from its pass is the defect §39b exists to catch, so the
# character is built explicitly and a failure to run is reported as a failure.
#
# `git ls-files` prints paths RELATIVE TO THE REPOSITORY ROOT, so the roots it is
# given and the paths it returns have to be normalised to the same space before
# they are compared. `root` is absolute (the caller anchors it to $REPO), so the
# repo root is derived from git itself rather than assumed to be the cwd, and
# every path below is compared as repo-relative. Comparing an absolute path with
# a relative one would mark EVERY vendored file untracked — a false BAD, which is
# the failure this normalisation exists to prevent.
top = subprocess.run(
    ["git", "-C", str(root), "rev-parse", "--show-toplevel"],
    capture_output=True, text=True, check=False,
)
if top.returncode != 0:
    print("ERROR\tgit rev-parse failed (%d): %s" % (top.returncode, top.stderr.strip()[:200]))
    raise SystemExit(0)
repo_root = Path(top.stdout.strip())

def repo_rel(path):
    """The path as git names it, or None when it lies outside the repository."""
    try:
        return path.resolve().relative_to(repo_root).as_posix()
    except ValueError:
        return None

ls = subprocess.run(
    ["git", "-C", str(repo_root), "ls-files", "-z", "--", repo_rel(root) or str(root)],
    capture_output=True, text=True, check=False,
)
if ls.returncode != 0:
    print("ERROR\tgit ls-files failed (%d): %s" % (ls.returncode, ls.stderr.strip()[:200]))
    raise SystemExit(0)
tracked = {p for p in ls.stdout.split(chr(0)) if p}

problems = []
for group in vendored:
    gdir = root / group
    for base, by_triple in lock.get("sidecars", {}).items():
        for triple, entry in by_triple.items():
            path = gdir / entry["file"]
            if not path.exists() and not gdir.exists():
                continue
            if path.exists() and repo_rel(path) not in tracked:
                problems.append("UNTRACKED\t%s (group %s) is on disk but git does not carry it — a fresh clone would not have it" % (path, group))

for p in sorted(root.rglob("*")):
    if not p.is_file():
        continue
    if repo_rel(p) not in tracked:
        problems.append("UNTRACKED\t%s is in the vendor tree but untracked — either add it deliberately or remove it" % p)

# ── The doc this guard is cited from has to agree with the lock ──
#
# docs/operate/VENDORED-BUILD-INPUTS.md declares this script as its guard, and
# for two weeks after the geo databases were committed its §2 layout block said
# `resources/  NOT PRESENT` while §4 said they were committed and the bytes were
# there. Nothing noticed, because §39b only ever looked at `vendor/`. A document
# is one of the things a guard can hold to the truth when the truth is a list, so
# this compares the doc's layout block with the lock's own `resources` keys.
#
# It checks AGREEMENT, not prose: the four filenames in the block must be exactly
# the four keys in the lock. A repin that adds a fifth database fails here until
# the block is updated, which is the intended amount of friction.
doc = repo_root / "docs/operate/VENDORED-BUILD-INPUTS.md"
if sorted_resources and doc.is_file():
    text = doc.read_text()
    # ANCHORED TO THE FENCE, NOT TO THE FIRST MENTION. `text.find("vendor/build-inputs/")`
    # lands on the intro paragraph, which names the directory in prose — the first
    # version of this check did exactly that, scanned the wrong 1000 characters,
    # and reported "lists nothing" on a CORRECT tree. A false BAD is how a guard
    # stops being read, so the anchor is the fenced block that contains the line.
    fence_start = text.find("```text\nvendor/build-inputs/")
    if fence_start != -1:
        fence_end = text.find("```", fence_start + len("```text\n"))
        if fence_end != -1:
            block = text[fence_start:fence_end]
    if not block:
        problems.append("DOC\t%s has no vendor/build-inputs/ layout block to check" % doc.name)
    else:
        declared = set(re.findall(r"[A-Za-z0-9_]+\.(?:mmdb|dat)", block))
        expected = set(sorted_resources.split())
        if declared != expected:
            problems.append(
                "DOC\t%s's layout block lists %s but the lock pins %s — the block is what a reader trusts instead of checking"
                % (doc.name, sorted(declared) or "nothing", sorted(expected))
            )

print("\n".join(dict.fromkeys(problems)))
PYEOF
    )
        if [ -z "$vendor_report" ]; then
            ok "every vendored build input is committed to the repository"
        else
            while IFS= read -r line; do
                case "$line" in
                    UNTRACKED*) bad "vendored input: ${line#UNTRACKED	}" ;;
                    DOC*) bad "vendored input: ${line#DOC	}" ;;
                    *) [ -n "$line" ] && bad "vendored input: $line" ;;
                esac
            done <<EOF
$vendor_report
EOF
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
