#!/usr/bin/env bash
# Section 28 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_28() {
    # ─────────────────────────────────────────────────────────────
    # 28. The documentation index is COMPLETE, and no live document lies about itself.
    #
    # WHY THIS EXISTS
    #
    # The 2026-10-05 audit found the doc set green but fragmented, in a way no
    # existing guard could see. §8 checks that a link RESOLVES, §25 that front-matter
    # EXISTS. Neither asks the two questions that had actually gone wrong:
    #
    #   (a) IS EVERY LIVE DOCUMENT REACHABLE? docs/README.md covered the operate/ and
    #       reference/ trees and stopped. All 19 business files, the whole redesign
    #       corpus and client/docs/CONTRIBUTING_i18n.md had no entry — reachable only
    #       by browsing the filesystem. A reader who trusts the index does not learn
    #       those documents exist, and a document nobody can find is one nobody
    #       maintains. There were also TWO indexes (README.md and STATE.md) whose
    #       contents had drifted apart, which is worse than one incomplete one.
    #
    #   (b) DOES THE DOCUMENT AGREE WITH ITS OWN LABEL? docs/GAMING-UDP.md was
    #       `status: live`, titled "Implementation Plan", and its OWN BODY opened
    #       with two banners saying it was superseded. The front-matter was not
    #       wrong by accident — nothing connected the two, so a document could
    #       announce its own retirement and stay labelled current forever.
    #
    # WHAT THIS CHECKS, AND WHAT IT DELIBERATELY DOES NOT
    #
    #   (a) Every live .md under docs/ and client/docs/ is linked from docs/README.md.
    #       Exit is by FILE, not by heading, so a document may be covered by a
    #       directory entry (the retired `redesign/archive/` is one) as long as
    #       something on the index names it.
    #   (b) A document whose TITLE OR OPENING says it is retired — while its
    #       front-matter says `status: live` — is a contradiction. The fix is one of:
    #       move it to archive/ (folder carries the status), correct the opening, or
    #       correct the status.
    #
    #   (b) IS SCOPED TO THE HEAD ON PURPOSE, and the first draft was not. A whole-body
    #       scan for words like REMOVED/retired fired on seven healthy documents, and
    #       every hit was legitimate: FIXES.md is a log whose heading records that
    #       something *else* was removed; STILL-OPEN.md's job is to mark superseded
    #       items; CONTEXT.md and the tier docs describe retired mechanisms in their
    #       history sections. That is §7's lesson exactly — a guard that cries wolf
    #       trains the reader to ignore the one that matters. What actually went wrong
    #       in GAMING-UDP.md was that the retirement was announced in the head of the
    #       document while the head's own status field said `live`. So that is what
    #       this checks: the first 25 lines, and the title.
    #
    #       WHAT IT ACTUALLY CATCHES, so this comment cannot claim more than the code
    #       does: exactly four capitalised markers, in the first 25 lines of a
    #       `status: live` document — SUPERSEDED, ARCHIVED, NOT SHIPPED, RETIRED. It
    #       is case-SENSITIVE and it does not read `removed`, `no longer`, `dead`,
    #       `obsolete`, `historical` or `superseded` in lower case. A case-insensitive
    #       scan for those wider words finds 12 live documents and every one is
    #       legitimate — each is about something *else* being retired, which is the
    #       same false-positive class the whole-body scan was abandoned for. The
    #       marker set is deliberately narrow; do not widen it without triaging 12
    #       correct hits first.
    #
    # It does NOT check that a declared status is TRUE — a guard cannot read a design
    # record and know it is stale (§25 says the same). It catches the case where the
    # document states the contradiction ITSELF, in the place a reader looks first.
    #
    # Every exclusion below is a closed set with a reason, per §7's lesson: a warning
    # nobody can action trains the reader to ignore the ones that matter.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "28. The documentation index is complete, and live documents do not contradict themselves"

    doc_index_report=$(cd "$REPO" && python3 - <<'PY'
import os, re

ARCHIVE_PREFIXES = ('docs/archive/', 'docs/history/',
                    'docs/business/redesign/archive/')

# ── WHAT §28 HOLDS TO THE INDEX, AND WHAT IT DOES NOT ──
#
# `docs/**` and `client/docs/**` are project documentation: index them, always.
#
# `client/*.md` was added 2026-10-05, when `client/CONTRIBUTING.md` turned out to
# be a contributor guide that NOTHING referenced — not the root README, not
# `client/AGENTS.md`, not the index. A document a contributor is expected to read
# and cannot find is the same defect as an unlinked business file.
#
# Deliberately NOT held to the index, with the reason (§7: exclude precisely):
#   client/AGENTS.md, CLAUDE.md, GEMINI.md   entry points, not documentation —
#       AGENTS.md is the thing that POINTS AT the docs, and CLAUDE/GEMINI are
#       one-line shims delegating to it.
#   client/scripts/**/GUIDE.md, scripts/README.md, server/scripts/README.md
#       tool manuals. Their reader arrives from the tool, not from the doc index;
#       indexing them would put "how to run a perf harness" in the same list as
#       "the API contract", which is how an index stops being worth reading.
#       They are covered against LINK ROT by §8, which is the risk that matters.
#
# ROOT `*.md` JOINED THE SCOPE ON 2026-11-22 (audit F-02). It had been outside
# every walk — §8, §10 and this one — and the consequence was not theoretical:
# the front door was cited by nothing and unguarded by anything, while this
# section printed "every live document is linked from docs/README.md". A
# completeness claim that excludes the first file a reader opens is worse than no
# claim. The root exemptions are the same vocabulary as the client ones: entry
# points and shims delegate to AGENTS.md, and the index does not list its own
# front matter.
#
# The front door is `README.md` since the 2026-11-22 rename. It stays EXEMPT from
# the index — a front door is not a document to be listed inside itself — but it
# is NOT exempt from `§8`, which is the only guard that would notice a broken link
# on the first page a reader opens. The two exemptions are different in kind:
# `§28` asks "is this reachable from the index", `§8` asks "do its links resolve".
INDEX_EXEMPT = {
    'client/AGENTS.md', 'client/CLAUDE.md', 'client/GEMINI.md',
    'AGENTS.md', 'CLAUDE.md', 'GEMINI.md',
    # The front door.
    'README.md', 'CONTRIBUTING.md',
    # Not documentation in the index sense: a licence, and gitignored scratch
    # that quotes what it found (the audit file quotes addresses; a handover note
    # records unfinished work).
    'LICENSE', 'DOC-AUDIT-2026-11-22.md', '.renovation-handover.md',
}

def md_files(root):
    for dirpath, _dirs, files in os.walk(root):
        if any(part in dirpath for part in ('node_modules', 'target', '.git', 'dist')):
            continue
        for fn in files:
            if fn.endswith('.md'):
                yield os.path.join(dirpath, fn).replace(os.sep, '/')

def indexed_scope():
    """Top-level `client/*.md`, root `*.md`, and `client/src-tauri/**/*.md`.

    Root was added 2026-11-22 (audit F-02): the front door lived outside every
    walk, so the completeness claim on `docs/README.md` did not cover it. See the
    INDEX_EXEMPT comment for which root files are exempt and why.

    `client/src-tauri/` was added 2026-11-23, for the same reason one level in.
    The icon migration produced `client/src-tauri/icons/README.md` — a live,
    authoritative document saying which mark is generated and from what — and it
    sat outside EVERY walk: not `docs/`, not `client/docs/`, not a top-level
    `client/*.md`. It could therefore be orphaned or left contradicting the
    generator with nothing failing, which is the exact shape audit F-02 found at
    the root. Tool manuals stay excluded by name (`client/scripts/**/GUIDE.md`,
    whose reader arrives from the tool); a NEW document under `src-tauri/` is
    held to the index by default rather than silently skipped.
    """
    out = []
    for fn in sorted(os.listdir('client')):
        if not fn.endswith('.md'):
            continue
        rel = 'client/' + fn
        if not os.path.isfile(rel) or rel in INDEX_EXEMPT:
            continue
        out.append(rel)
    for fn in sorted(os.listdir('.')):
        if not fn.endswith('.md'):
            continue
        if not os.path.isfile(fn) or fn in INDEX_EXEMPT:
            continue
        out.append(fn)
    for dirpath, _dirs, files in os.walk('client/src-tauri'):
        if any(part in dirpath for part in ('target', 'node_modules')):
            continue
        for fn in sorted(files):
            if not fn.endswith('.md'):
                continue
            rel = os.path.join(dirpath, fn).replace(os.sep, '/')
            if rel in INDEX_EXEMPT:
                continue
            out.append(rel)
    # `scripts/` and `server/scripts/`, added 2026-11-25 for the same reason the
    # two walks above exist. BOTH are third top-level trees holding live,
    # authoritative documents — `server/scripts/README.md` is the index of every
    # command that can reach the production hub, and `scripts/README.md` documents
    # the code printers — and BOTH sat outside every walk, so either could be
    # orphaned from `docs/README.md` with nothing failing.
    #
    # This is the third time the same scope hole was found (root `*.md` 2026-11-22,
    # `client/src-tauri/**` 2026-11-23, now these). The lesson recorded then holds:
    # an exemption list is safe, a scope that stops at a fixed depth is not.
    for tree in ('scripts', 'server/scripts'):
        for dirpath, dirs, files in os.walk(tree):
            dirs[:] = [d for d in dirs if d not in ('node_modules', '__pycache__')]
            for fn in sorted(files):
                if not fn.endswith('.md'):
                    continue
                rel = os.path.join(dirpath, fn).replace(os.sep, '/')
                if rel in INDEX_EXEMPT:
                    continue
                out.append(rel)
    return out

def front_matter(path):
    lines = open(path, encoding='utf-8', errors='replace').read().splitlines()
    i = 0
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i < len(lines) and lines[i].startswith('# '):
        i += 1
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i >= len(lines) or not lines[i].startswith('```'):
        return {}
    i += 1
    keys = {}
    while i < len(lines) and not lines[i].startswith('```'):
        m = re.match(r'^([a-z-]+):\s*(.*)$', lines[i].strip())
        if m:
            keys[m.group(1)] = m.group(2).strip()
        i += 1
    return keys

index_text = open('docs/README.md', encoding='utf-8').read()

# Everything the index links to, as normalised repo-relative paths.
linked = set()
for m in re.finditer(r'\]\(([^)\s]+?\.md)(#[^)\s]+)?\)', index_text):
    linked.add(os.path.normpath(os.path.join('docs', m.group(1))))
# Directory links: a link to `dir/` covers every file beneath it.
for m in re.finditer(r'\]\(([^)\s]+?)/\)', index_text):
    d = os.path.normpath(os.path.join('docs', m.group(1)))
    linked.add(d + '/')

orphans, contradictions = [], []

for path in sorted(set(md_files('docs')) | set(md_files('client/docs'))
                   | set(indexed_scope())):
    if path.startswith(ARCHIVE_PREFIXES):
        continue
    if path == 'docs/README.md':
        continue
    keys = front_matter(path)
    status = keys.get('status', '')

    # (a) reachability — the index must name the file, or a directory above it.
    covered = path in linked or any(
        path.startswith(d) for d in linked if d.endswith('/'))
    if not covered:
        orphans.append(f"{path}  (status: {status or 'none'})")

    # (b) a live document may not announce its own retirement in its HEAD.
    #
    # Scoped to the title plus the first 25 lines, and to a closed marker set.
    # A whole-body scan was the first draft and it produced seven false positives,
    # all of them healthy documents describing someone else's retirement (see the
    # section header). The head is where the GAMING-UDP defect lived.
    if status != 'live':
        continue
    lines = open(path, encoding='utf-8', errors='replace').read().splitlines()
    head = '\n'.join(lines[:25])
    # Quoted markers inside a fenced block are exposition, not a claim.
    head = re.sub(r'```.*?```', '', head, flags=re.S)
    # Only UNAMBIGUOUS self-retirement words. `was retired` / `is retired` were
    # in the first draft and fired on `STILL-OPEN.md`'s own heading, which says
    # the *Stealth tier* was retired — ordinary prose about a different thing.
    for marker in ('SUPERSEDED', 'ARCHIVED', 'NOT SHIPPED', 'RETIRED'):
        if marker in head:
            contradictions.append(
                f"{path}  (status: live, but the head says {marker!r})")
            break

for f in orphans:
    print("ORPHAN\t" + f)
for f in contradictions:
    print("CONTRADICTION\t" + f)
PY
    )

    if [ -z "$doc_index_report" ]; then
        ok "every live document is linked from docs/README.md, and none contradicts its own status"
    else
        while IFS="$(printf '\t')" read -r kind detail; do
            case "$kind" in
                ORPHAN)
                    bad "live document not reachable from docs/README.md: $detail"
                    bad "  Add it to the index. A document nobody can find is one nobody maintains." ;;
                CONTRADICTION)
                    bad "a live document announces its own retirement: $detail"
                    bad "  Either move it to archive/ (the folder carries the status), or fix" ;;
            esac
        done <<EOF
$doc_index_report
EOF
        bad "  the front-matter — §25. Do not leave the two disagreeing."
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
