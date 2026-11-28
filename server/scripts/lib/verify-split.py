#!/usr/bin/env python3
"""Verify that the split check-consistency.sh behaves identically to the original.

WHY THIS EXISTS

The split is only worth doing if it cannot change a verdict. A guard that has been
quietly weakened is worse than one that does not exist, because it still prints
"All consistency checks passed".

WHAT IS COMPARED, AND WHAT IS ALLOWED TO DIFFER

Verdicts are compared exactly: the sequence of section banners, and the counts of
OK / BAD / WARN lines. The raw text is compared after normalising file:line
coordinates, because three guards (§4, §8, §25/§28) print their own location in
the checker -- and splitting a file necessarily moves those numbers. A byte diff
would flag every one of them as a regression and bury the differences that matter.

THE REPO PROBLEM

`check-consistency.sh` derives REPO from its own path and takes no argument. The
original therefore must run from exactly where the orchestrator lives, or every
path-relative guard reports "found nothing" -- 158 BAD lines that say nothing
about the split. So the two are swapped for the duration of the run. Nothing is
copied into the tree: an earlier version staged a copy in `server/scripts/`, and
§4/§8/§53 reported findings against that copy, so the comparison measured the
staging.

USAGE

    verify-split.py <original.sh> <orchestrator.sh>
"""
import os
import re
import shutil
import subprocess
import sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
COORD = re.compile(r"(\.(?:sh|py|js|ts|mjs)\s*:\s*)(\d+)")
BANNER = re.compile(r"^\s*\d+[a-z]?(?:-bis)?\. ", re.MULTILINE)


def run(script):
    proc = subprocess.run(["bash", script], capture_output=True, text=True,
                          timeout=1800)
    return proc.returncode, ANSI.sub("", proc.stdout), ANSI.sub("", proc.stderr)


def normalise(text):
    """Drop file:line coordinates, which necessarily differ after a split."""
    return COORD.sub(r"\1?:", text)


def verdicts(text):
    return (
        [l.strip() for l in text.splitlines() if BANNER.match(l)],
        text.count("BAD"), text.count("OK"), text.count("WARN"),
    )


def main(argv):
    if len(argv) != 3:
        sys.stderr.write("usage: verify-split.py <original.sh> <orchestrator.sh>\n")
        return 2
    original, orchestrator = os.path.abspath(argv[1]), os.path.abspath(argv[2])

    if subprocess.run(["bash", "-n", orchestrator],
                      capture_output=True).returncode != 0:
        sys.stderr.write("FAIL the orchestrator does not parse\n")
        return 1

    # Swap the original into the orchestrator's path for the run, and guarantee a
    # restore even if the comparison raises.
    hold = orchestrator + ".verify-hold"
    shutil.move(orchestrator, hold)
    try:
        shutil.copyfile(original, orchestrator)
        rc_a, out_a, err_a = run(orchestrator)
    finally:
        shutil.move(hold, orchestrator)

    rc_b, out_b, err_b = run(orchestrator)

    failed = 0
    va, vb = verdicts(out_a), verdicts(out_b)

    if va[0] != vb[0]:
        failed = 1
        sys.stderr.write("FAIL the sequence of sections run differs\n")
        for i, (a, b) in enumerate(zip(va[0], vb[0])):
            if a != b:
                sys.stderr.write("  first difference at section %d:\n"
                                 "    original: %s\n    split:    %s\n" % (i + 1, a, b))
                break
        if len(va[0]) != len(vb[0]):
            sys.stderr.write("  original ran %d sections, split ran %d\n"
                             % (len(va[0]), len(vb[0])))

    for i, label in ((1, "BAD"), (2, "OK"), (3, "WARN")):
        if va[i] != vb[i]:
            failed = 1
            sys.stderr.write("FAIL %s count differs: original=%d split=%d\n"
                             % (label, va[i], vb[i]))

    if normalise(out_a) != normalise(out_b):
        failed = 1
        sys.stderr.write("FAIL output differs beyond file:line coordinates\n")
        la, lb = normalise(out_a).splitlines(), normalise(out_b).splitlines()
        for i in range(max(len(la), len(lb))):
            a = la[i] if i < len(la) else "<missing>"
            b = lb[i] if i < len(lb) else "<missing>"
            if a != b:
                sys.stderr.write("  line %d:\n    original: %s\n    split:    %s\n"
                                 % (i + 1, a, b))
                break

    if rc_a != rc_b:
        failed = 1
        sys.stderr.write("FAIL exit code differs: original=%d split=%d\n"
                         % (rc_a, rc_b))

    if not failed:
        sys.stderr.write(
            "OK  %d sections, %d BAD / %d OK / %d WARN, exit %d -- identical\n"
            % (len(va[0]), va[1], va[2], va[3], rc_a))
    return failed


if __name__ == "__main__":
    sys.exit(main(sys.argv))
