#!/usr/bin/env python3
"""Where a guard is allowed to say the thing it bans.

WHY THIS EXISTS

Three guards scan the tree for a string they must also NAME in order to explain
themselves — §4 (the SS2022 cipher literal), §8 (`docs/<moved>.md`) and §53
(the repository path). Each carried a hand-written exemption:

    if fn == "check-consistency.sh": continue

Every one of those is DEAD. `check-consistency.sh` was split by
`extract-sections.py` into one file per section under `server/scripts/checks/`,
so the guards' own explanations no longer live in the file the exemption named.
The result, observed on 2026-11-28: §4 reported 2 hits, §8 reported 4, and both
were the guards' own pattern lines. More seriously, §53 found its own paragraph
and went red — but only on the SECOND run, because the first run's transcript was
not yet in the tree. A guard that reds on the second run is worse than either
state: `check-consistency.sh` is run twice in a row by a human all the time.

WHAT THE RULE IS

A guard may name a banned string inside a COMMENT in the guard's own directory,
and nowhere else. That is precisely the licence the exemption was trying to
express, and it is the minimum that lets a guard carry its evidence.

A comment is `#`-to-end-of-line for shell and Python, `//` for JS/TS/Rust. The
markers are per-language rather than "any line that contains #", so a banned
string inside a shell single-quoted pattern is not treated as a comment. That
distinction is load-bearing: §4's own PATTERN lines are what it was reporting,
and they are code, not comments.

WHY NOT A WIDER EXEMPTION

Widening to "skip any file under server/scripts/checks/" would stop the guards
checking each other, which is a real thing they do (§53 checks every `.sh` in the
tree including its neighbours). Widening to "skip any line containing the word
`comment`" would pass §4's self-report while still hiding a genuine literal. The
licence is scoped to (own directory) AND (inside a comment) — both, not either.

USAGE

    sys.path.insert(0, os.path.join(REPO, 'server/scripts/lib'))
    from guard_self import is_explanation
    if is_explanation(path, line): continue

`path` is repo-relative and POSIX-separated. `line` is the whole line, with its
newline already stripped.
"""
import os
import sys

# The directory every extracted section lives in. A guard's explanation may cite
# a banned string only from here.
CHECKS_DIR = "server/scripts/checks/"

# Shebang lines are not comments, but no guard bans `#!/usr/bin/env`.
_COMMENT_MARKERS = {
    ".sh": ("#",),
    ".py": ("#",),
    ".pyw": ("#",),
    ".js": ("//",),
    ".mjs": ("//",),
    ".cjs": ("//",),
    ".ts": ("//",),
    ".tsx": ("//",),
    ".vue": ("//",),
    ".rs": ("//",),
    ".yml": ("#",),
    ".yaml": ("#",),
    ".toml": ("#",),
}

# Files in the checks directory that are NOT guards: the split tool and this
# module both have to name the strings they talk about, and neither is a section.
_LIB_DIR = "server/scripts/lib/"
_LIB_FILES_MAY_QUOTE = ("extract-sections.py", "verify-split.py", "guard_self.py")


def _strip_comment(line, markers):
    """The line if it carries a comment marker outside any quotes, else ''.

    Shell/Python/JS all agree that `#` or `//` inside single or double quotes is
    literal text and not a comment, and the guards lean on that: §4's banned
    literal text and not a comment, and the guards lean on that: §4's PATTERN line
    and §53's assignment line both carry the banned string inside quotes. Only an
    unquoted marker starts a comment.
    """
    quote = None
    i = 0
    while i < len(line):
        ch = line[i]
        if quote:
            if ch == "\\" and quote == '"':
                i += 2
                continue
            if ch == quote:
                quote = None
            i += 1
            continue
        if ch in ("'", '"'):
            quote = ch
            i += 1
            continue
        for m in markers:
            if line.startswith(m, i):
                return line[i:]
        i += 1
    return ""


def is_explanation(path, line):
    """True iff `line` may legitimately name a banned string.

    Both conditions must hold: the file is the guard's own, AND the string is
    inside a comment there. See the module docstring for why neither alone is
    enough.
    """
    p = path.replace(os.sep, "/")
    if p.startswith("./"):
        p = p[2:]

    if not p.startswith(CHECKS_DIR):
        return False

    markers = _COMMENT_MARKERS.get(os.path.splitext(p)[1], ("#",))
    return bool(_strip_comment(line, markers))


def selftest():
    """Prove the two conditions, on cases this file only DESCRIBES.

    The banned strings arrive through `argv`, not as literals in this file, and
    that is not fastidiousness. This module sits in `server/scripts/lib/`, which
    §4 and §53 both scan; a selftest that spelled the literals out would be a
    guard whose OWN TEST makes it red — the exact defect that put those two
    guards on the floor in the first place. So the caller passes one banned
    word and the cases are built around it.
    """
    if len(sys.argv) < 2:
        print("usage: guard_self.py <a-banned-literal>")
        return 2
    banned = sys.argv[1]

    cases = [
        # (path, line, expected, why)
        ("server/scripts/checks/4-cipher-single-definition.sh",
         "        | grep -cE '\"%s|= *\"%s')" % (banned, banned), False,
         "the guard's own PATTERN line is code, not explanation"),
        ("server/scripts/checks/4-cipher-single-definition.sh",
         '    # the literal "%s" is banned' % banned, True,
         "a comment in the guard's own directory"),
        ("server/scripts/checks/53-retained-locus-names-match-the-register.sh",
         'BAD_REPO = "%s"' % banned, False,
         "the guard's own assignment is code, not explanation"),
        ("server/scripts/modules/00-env.sh",
         '    # SS_METHOD="%s"' % banned, False,
         "a comment OUTSIDE the checks directory is still a finding"),
        ("docs/reference/NAME-MIGRATION.md",
         "# %s" % banned, False,
         "a comment in a document is not the guard explaining itself"),
    ]
    failures = 0
    for path, line, expected, why in cases:
        got = is_explanation(path, line)
        if got != expected:
            print("SELFTEST FAILED: %s -> %s (expected %s): %s"
                  % (path, got, expected, why))
            failures += 1
    if failures:
        raise SystemExit(1)
    print("OK guard_self: %d cases" % len(cases))
    return 0


if __name__ == "__main__":
    raise SystemExit(selftest())
