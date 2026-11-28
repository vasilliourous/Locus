#!/usr/bin/env python3
"""Split check-consistency.sh into one sourced file per section.

WHY THIS EXISTS

`check-consistency.sh` reached 9,482 lines and 510 KB in a single file. An agent
asked to change one guard had to read a 10k-line file, and every grep it ran
against the repository landed in it. The cost was context, not runtime.

WHY IT IS A FUNCTION-WRAPPING SPLIT, NOT A LINE SLICE

The script is flat top-level shell with 60+ sections that share one global
namespace (`FAIL`, `ok`, `bad`, `REPO`, `HOOKS`, ...) and, in practice, each
other's variables. Slicing it into standalone scripts and running them as
subprocesses would silently break a section that reads a variable a previous
section set -- and it would not fail loudly, it would just check the wrong thing.

So each section becomes a FUNCTION, and the functions are `source`d into the
orchestrator's shell. Same process, same globals, same semantics. `local` is
used for locals where the original had none, which is not a behaviour change:
the script is single-threaded and every section runs once, top to bottom.

WHY THE SECTION NUMBER IS AN ARGUMENT

Section 9 in the script is actually "9 then 1c". A later change inserted 1c
inside 9 rather than renumbering, because renumbering would invalidate every
`§N` cross-reference in the docs and in FIXES.md. This transform preserves that
order exactly, so the output numbering matches the input -- verified by the
orchestrator diffing itself against the original.

USAGE

    extract-sections.py <input.sh> <output-dir>

Writes `<output-dir>/NN-<slug>.sh`, one per section, in source order.
Refuses to write if the extraction does not round-trip; see `verify.py`.
"""
import os
import re
import sys

# A section banner: a top-level `echo "N. text"`. The number may carry a suffix
# (1c, 1g-bis). The banner is what the run PRINTS, so it is the anchor the
# orchestrator diff relies on.
BANNER = re.compile(r'^echo "(\d+[a-z]?(?:-bis)?)\. (.+)"$')

# The rule line that opens each section's evidence comment when present.
RULE = re.compile(r'^# ─{10,}$')


def slug_for(number, title):
    """A short, sortable filename stem. Order is carried by the NN- prefix."""
    text = title.lower()
    text = re.sub(r'[^a-z0-9]+', '-', text)
    text = re.sub(r'-+', '-', text).strip('-')
    return "%s-%s" % (number, text[:52].rstrip('-'))


def extract(lines):
    """Return [(number, title, start, end)] where end is exclusive."""
    starts = []
    for i, line in enumerate(lines):
        m = BANNER.match(line.rstrip('\n'))
        if not m:
            continue
        number, title = m.group(1), m.group(2)
        # A section's evidence comment opens with a rule line above the banner.
        # Walk back over blanks and comments so the doc block travels with the
        # section it documents -- leaving it behind would orphan 5,000 lines of
        # "why this exists" in the orchestrator.
        # Walk back over blanks and comments to find where this section's doc
        # block opens. Stop AT a rule line (do not consume it) -- the rule is the
        # block's own delimiter and belongs with the block.
        # Two block shapes exist in the source:
        #   (a) rule / comment... / rule / echo / banner        <- most sections
        #   (b) rule / comment... / rule / (blank) / banner     <- §1 and a few others
        # Walking back over comments and blanks and stopping at the first
        # non-comment naturally takes the whole block in both, because the opening
        # rule is itself a comment. The `echo`-only line has to be skipped too, or
        # case (a) stops one line early and orphans the block's closing rule.
        j = i
        while j > 0:
            prev = lines[j - 1].rstrip('\n')
            if prev.strip() == '' or prev.lstrip().startswith('#'):
                j -= 1
                continue
            if re.fullmatch(r'\s*echo\s*', prev):
                j -= 1
                continue
            break
        starts.append((number, title, j, i))

    # The closing summary is not part of the last section. It lives after the
    # final rule line, and the orchestrator owns it -- leaving it inside the last
    # section function made that function `exit "$FAIL"` mid-loop, which skipped
    # nothing today only because it happened to be last.
    # Anchored on the summary TEXT, not on "the last rule line". Section 56's
    # header comment is itself opened by a rule line and sits near the end of the
    # file, so the rule-line heuristic cut into the section and silently deleted
    # its closing `ok`. A guard that loses its OK line still exits 0 -- it just
    # stops reporting, which is the failure mode this whole exercise is about.
    tail = len(lines)
    summary = next((i for i, l in enumerate(lines)
                    if "All consistency checks passed" in l), None)
    if summary is not None:
        # Walk back to the rule line that opens the summary block.
        j = summary
        while j > 0 and '══' not in lines[j - 1]:
            j -= 1
        j -= 1
        while j > 0 and lines[j - 1].strip() == '':
            j -= 1
        tail = j

    sections = []
    for k, (number, title, start, banner_idx) in enumerate(starts):
        end = starts[k + 1][2] if k + 1 < len(starts) else tail
        sections.append((number, title, start, end))
    return sections, starts


# NOTE: `\s`, not `[[:space:]]`. Python's `re` does not implement POSIX
# character classes; it treats `[[:space:]]` as a nested set and silently
# matches the wrong thing (a FutureWarning, not an error). It matched nothing
# here, so no terminator was preserved -- and every section with a heredoc
# failed to parse. Caught by the syntax pre-check, not by review.
HEREDOC_OPEN = re.compile(r"<<-?\s*'?([A-Za-z_][A-Za-z0-9_]*)'?")


def heredoc_terminators(body):
    """Indices of lines inside a heredoc body (including its terminator).

    These must NOT be re-indented. Two reasons, both learned by running it:

    1. A TERMINATOR must be the bare delimiter at column 0. Indent it and bash
       keeps reading; the heredoc swallows the rest of the function and the file
       fails to parse.

    2. A PYTHON BODY must start at column 0. `python3 - <<'PY'` receives the body
       verbatim, so indenting it turns every script into
       `IndentationError: unexpected indent` -- which the consumer usually
       swallows with `2>/dev/null`, so the guard reads an empty string and reports
       a failure about something else entirely.

    Shell heredoc bodies do not care about indentation, but keeping one rule for
    all of them is simpler than classifying interpreters, and preserving the
    author's layout is the conservative choice.

    Indices of lines that terminate a heredoc and must stay at column 0.

    WHY THIS MATTERS

    Each section is indented into a function body. A heredoc terminator must be
    the bare delimiter with NO leading whitespace (unless the operator is `<<-`,
    which strips tabs only -- and these bodies are indented with spaces). Indent
    a terminator and bash keeps reading, the heredoc swallows the rest of the
    function, and the file fails to parse with "here-document delimited by
    end-of-file".

    That is exactly what happened on the first run of this transform: section 1
    opens a `<<'PY'` inside a `$(...)`, the terminator was indented, and the
    orchestrator's syntax pre-check caught it. The pre-check is what turned a
    silent skip into a loud failure -- which is why it exists.
    """
    pending = []
    out = set()
    for i, line in enumerate(body):
        if pending:
            # Every body line is kept verbatim, not just the terminator.
            out.add(i)
            if line.strip() == pending[0]:
                pending.pop(0)
            continue
        # A COMMENT IS NOT AN OPERATOR. This repo's guards explain themselves at
        # length -- §43's own comment reads "RUN FROM A FILE, NOT `bash -
        # <<HEREDOC`", and scanning that as a heredoc opener put a phantom
        # `HEREDOC` on the stack, which then ate the real `PREC` terminator as if
        # it were a body line. The terminator got indented, the file stopped
        # parsing, and the section silently vanished. Prose about an operator is
        # not the operator.
        if line.lstrip().startswith("#"):
            continue
        for m in HEREDOC_OPEN.finditer(line):
            pending.append(m.group(1))
    return out


def emit(sections, lines, out_dir):
    """Write one file per section, each a function holding its original lines."""
    written = []
    for number, title, start, end in sections:
        fn = "section_%s" % number.replace('-', '_').replace('bis', 'bis')
        stem = slug_for(number, title)
        path = os.path.join(out_dir, stem + ".sh")
        body = lines[start:end]

        # Drop leading/trailing blanks so the concatenation is clean.
        while body and body[0].strip() == '':
            body.pop(0)
        while body and body[-1].strip() == '':
            body.pop()

        # The banner `echo` stays in the body: it is what the run prints, and the
        # orchestrator diff compares printed output. Removing it would change the
        # output and defeat the verification.
        out = []
        out.append("#!/usr/bin/env bash\n")
        out.append("# Section %s -- extracted from check-consistency.sh by\n"
                   "# server/scripts/lib/extract-sections.py. Do not add a shebang-only\n"
                   "# preamble: this file is SOURCED into the orchestrator's shell and\n"
                   "# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).\n" % number)
        out.append("# A syntax error here does NOT stop the run -- see the orchestrator's\n"
                   "# syntax pre-check, and the note at the top of check-consistency.sh.\n")
        keep_col0 = heredoc_terminators(body)
        out.append("\n%s() {\n" % fn)
        for i, line in enumerate(body):
            if not line.strip():
                out.append("\n")
            elif i in keep_col0:
                # A heredoc terminator: column 0 or bash will not close the body.
                out.append(line)
            else:
                out.append("    " + line)
        out.append("}\n")

        with open(path, "w", encoding="utf-8") as fh:
            fh.write("".join(out))
        written.append((number, fn, stem))
    return written


def main(argv):
    if len(argv) != 3:
        sys.stderr.write("usage: extract-sections.py <input.sh> <output-dir>\n")
        return 2
    src, out_dir = argv[1], argv[2]
    with open(src, encoding="utf-8") as fh:
        lines = fh.readlines()

    sections, _ = extract(lines)
    if not sections:
        sys.stderr.write("no section banners found in %s -- refusing to write\n" % src)
        return 1

    os.makedirs(out_dir, exist_ok=True)
    written = emit(sections, lines, out_dir)

    manifest = os.path.join(out_dir, "ORDER")
    with open(manifest, "w", encoding="utf-8") as fh:
        fh.write("# Section execution order, in source order. Generated by\n"
                 "# extract-sections.py; read by check-consistency.sh.\n")
        for number, fn, _stem in written:
            fh.write("%s %s\n" % (number, fn))

    sys.stderr.write("extracted %d sections into %s\n" % (len(written), out_dir))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
