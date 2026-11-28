#!/usr/bin/env bash
# Section 8 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_8() {
    # ─────────────────────────────────────────────────────────────
    # 8. Documentation cross-references resolve.
    #
    # WHY THIS EXISTS: the business docs navigate by cross-reference —
    # `19-cross-reference.md` is nothing but links — so a dead anchor silently
    # removes a reader's route to the detail. The docs use `file.md#NN` to mean
    # "§N.N" (`04-tiers.md#44` = section 4.4), and Markdown has no idea what that
    # is: the real anchor for "## 4.4 The free tier, in full" is
    # `#44-the-free-tier-in-full`. 71 such links were dead before this check.
    #
    # The rule: a link's target file must exist, and if it carries a fragment the
    # fragment must match a real heading's slug.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "8. Documentation cross-references"

    doc_links=$(cd "$REPO" && python3 - <<'PY'
import os, re, sys

# A guard that BANS a path must be able to NAME it in order to explain itself.
# `guard_self` is that one rule, shared with §4 and §53 — see its docstring for
# why the old per-guard filename exemption stopped working.
sys.path.insert(0, os.path.join('server', 'scripts', 'lib'))
from guard_self import is_explanation

def slug(h):
    h = h.strip().lower()
    h = re.sub(r'[^a-z0-9 \-_]', '', h)
    return h.replace(' ', '-')

def md_files(root):
    for dirpath, _dirs, files in os.walk(root):
        # Never walk generated or dependency trees.
        if any(part in dirpath for part in ('node_modules', 'target', '.git', 'dist')):
            continue
        for fn in files:
            if fn.endswith('.md'):
                yield os.path.join(dirpath, fn)

# ── WHAT IS IN SCOPE, AND WHY IT IS NOT "EVERY .md IN THE REPO" ──
#
# The doc trees, plus the files a reader enters by. `client/docs/` alone was not
# enough: `client/CONTRIBUTING.md` sits one level up and carried a live ambiguous
# reference that nothing checked, while its sibling
# `client/docs/CONTRIBUTING_i18n.md` was covered. A contributor guide is a
# document like any other.
#
# Deliberately NOT walked, each for a reason:
#   legacy/                     retired client; its links point into its own tree
#   .whale/, .aider*            local agent tooling, not part of the project
#   node_modules, target, dist  generated
#
# `docs/archive/` and `docs/history/` ARE walked — retired material may still be
# linked TO, and a reader who follows one must not hit a dead end.
def top_level_docs():
    """Top-level contributor docs and the script indexes people read directly."""
    out = []
    for d in ('client', 'scripts', 'server/scripts'):
        if not os.path.isdir(d):
            continue
        for dirpath, _dirs, files in os.walk(d):
            if any(s in dirpath for s in ('node_modules', 'target', 'dist')):
                continue
            for fn in files:
                if not fn.endswith('.md'):
                    continue
                rel = os.path.join(dirpath, fn)
                # `client/docs/**` is already walked above; take only the rest.
                if rel.startswith('client/docs/'):
                    continue
                # A tool guide under an otherwise-code tree is entered by hand.
                out.append(rel)
    return out

# Root `*.md`, from 2026-11-22 (audit F-02): §28 now holds root files to the
# index, so §8 must resolve links out of them or it reports a target it cannot
# see. `AGENTS.md` and the front door are project documents and belong here.
#
# THE FRONT DOOR IS `README.md` SINCE 2026-11-22, and it is NOT exempt from this
# walk. It used to be `Readmedon'tdisplayonpublicpage.md` — a name chosen so
# GitHub would not render it — and the file was renamed to `README.md` precisely
# so the first page a reader opens is the real one. Carrying the old exemption
# over to the new path would promote a file to the front door while leaving it
# the one document nothing link-checks; that is the silent-regression shape this
# whole renovation exists to prevent.
def root_docs():
    return ['README.md', 'AGENTS.md', 'CLAUDE.md', 'GEMINI.md']

docs = [p for p in md_files('docs')] + \
       [p for p in md_files('client/docs')] + \
       top_level_docs() + root_docs()
docs = sorted(set(p for p in docs if os.path.exists(p)))

anchors = {}
for p in docs:
    s = set()
    with open(p, encoding='utf-8', errors='replace') as fh:
        for line in fh:
            m = re.match(r'^#{1,6}\s+(.*)$', line.strip())
            if m:
                s.add(slug(m.group(1)))
    anchors[os.path.normpath(p)] = s

bad = []
for p in docs:
    base = os.path.dirname(p)
    with open(p, encoding='utf-8', errors='replace') as fh:
        text = fh.read()
    for m in re.finditer(r'\]\(([^)\s]+?\.md)(#([^)\s]+))?\)', text):
        target = os.path.normpath(os.path.join(base, m.group(1)))
        if not os.path.exists(target):
            bad.append(f"{p}: missing target {m.group(1)}")
            continue
        frag = m.group(3)
        if frag and target in anchors and frag not in anchors[target]:
            bad.append(f"{p}: {m.group(1)}#{frag} matches no heading")

# Plain-text references: a doc or code comment that names a doc path the way a
# reader would navigate to it (e.g. "see docs/DEPLOY.md"), but by a path that
# no longer exists. These are NOT markdown links, so the check above cannot see
# them; the 2026-09 reorg moved ten docs into operate/ and reference/ and this
# is the class of reference that silently rots when a path changes.
#
# We only flag a reference that carries an explicit `docs/` prefix AND names a
# `.md` file that does not exist there — that is precise enough not to fire on
# prose ("FIXES.md is a dated log") or on historical citations.
#
# Scoped to the basenames the reorgs actually moved, so it flags exactly the
# reorg-rot class and does not cry wolf on incidental mentions or files that
# never existed in docs/ (private research notes, the client's own docs).
#
# ── CODE FILES ARE SCANNED TOO (added 2026-10-05) ──
#
# The first version of this check walked only `docs` — markdown. That is the gap
# that let TWO live defects survive a full year of green pipelines:
# `client/src-tauri/src/feat/tun.rs` and `feat/core_upgrade.rs` both carried
# "see `docs/ARCHITECTURE.md`" long after the 2026-09 reorg moved that file to
# `docs/archive/ARCHITECTURE-wails.md`. A code comment is a reference a reader
# follows exactly like a link, and nothing was looking at it.
#
# The scan is deliberately the SAME regex and the SAME basename allow-list as the
# markdown pass, extended over source files rather than widened. That is the §7
# rule: cover the class, do not broaden the pattern. Two exclusions keep it
# honest:
#
#   * `check-consistency.sh` itself — its comments QUOTE stale paths as examples
#     of what it catches, and a guard that flags its own documentation is a guard
#     the next reader disables.
#   * `docs/history/`, `docs/archive/` and `docs/reference/FIXES.md` — historical
#     logs cite old paths on purpose; they record what happened. (FIXES.md is now
#     a summary index rather than a transcript, so this exemption is smaller than
#     it was; it is kept because the summary still names the old paths the rules
#     were learned on.)
#
# The extensions are the languages that can carry a path in a comment or string.
# Deliberately NOT included: `.json`/lockfiles (generated, and full of unrelated
# paths) and `server/console/dist/` (build output, regenerated from src/).
MOVED_BASENAMES = (
    'DEPLOY.md', 'OPS.md', 'POCKETBASE-SETUP.md', 'SECRETS-MANAGEMENT.md',
    'UPDATE-SYSTEM.md', 'RELEASING.md', 'CI-CD.md', 'API.md', 'FIXES.md',
    'STILL-OPEN.md', 'ARCHITECTURE.md', 'ENGINE-SWAP-ANALYSIS.md',
    'GAMING-UDP.md', 'CONTEXT.md', 'DEVICE-IDENTITY.md', 'EGRESS-READINESS.md',
    'DEBUGGING-METHOD.md', 'THEMES.md',
)
STALE_PATHS = re.compile(r'(?<![A-Za-z0-9/._-])docs/([A-Za-z0-9._/-]+\.md)')

CODE_EXTS = ('.rs', '.ts', '.tsx', '.js', '.mjs', '.py', '.sh', '.yml', '.yaml',
             '.toml', '.html', '.vue')
CODE_ROOTS = ('client/src', 'client/src-tauri/src', 'client/src-tauri/tests',
              'client/src-tauri/scripts', 'client/scripts', 'server', '.github',
              'scripts')
# Never walk generated or dependency trees, and never the guard's own quoting.
CODE_SKIP = ('node_modules', 'target', '.git', '/dist/', 'gen/schemas',
             '__pycache__', 'console/dist')

def code_files():
    for root in CODE_ROOTS:
        for dirpath, _dirs, files in os.walk(root):
            if any(s in dirpath for s in CODE_SKIP):
                continue
            for fn in files:
                if fn.endswith(CODE_EXTS):
                    yield os.path.join(dirpath, fn)

def stale_in(path, text):
    """Yields (lineno, rel) for a stale `docs/…md` reference on a line.

    The MOVED_BASENAMES list above is itself a list of dead paths, so this guard
    finds itself before it finds anything else. `guard_self.is_explanation` is the
    one licence every such guard shares: a COMMENT in `server/scripts/checks/` may
    name a banned path, a code line may not. This function therefore yields the
    line too, so its caller can ask.

    THE OLD EXEMPTION WAS DEAD, and that is the whole reason this changed. It read
    `if p == 'server/scripts/check-consistency.sh': continue` — correct until the
    sections were split into one file each under `server/scripts/checks/`, after
    which the guard's own explanation lived in a file the exemption did not name.
    It reported four of its own lines on 2026-11-28. See guard_self.py.
    """
    for lineno, line in enumerate(text.splitlines(), 1):
        for m in STALE_PATHS.finditer(line):
            rel = m.group(1)
            if os.path.basename(rel) not in MOVED_BASENAMES:
                continue
            if os.path.exists(os.path.join('docs', rel)):
                continue
            yield lineno, rel, line

for p in docs:
    # Historical logs cite old paths on purpose — they record what happened.
    if p.startswith('docs/reference/FIXES.md') or p.startswith('docs/history/') \
            or p.startswith('docs/archive/'):
        continue
    with open(p, encoding='utf-8', errors='replace') as fh:
        for lineno, rel, line in stale_in(p, fh.read()):
            # A reference into another tree (client/docs/…) is written with
            # its own prefix and will not match `docs/…` here.
            if is_explanation(p, line):
                continue
            bad.append(f"{p}:{lineno}: stale path docs/{rel} (no such file)")

for p in code_files():
    try:
        with open(p, encoding='utf-8', errors='replace') as fh:
            text = fh.read()
    except OSError:
        continue
    for lineno, rel, line in stale_in(p, text):
        # The code-comment case, and the reason `is_explanation` exists rather
        # than a filename skip: this guard's own MOVED_BASENAMES list and its
        # evidence block are code and comments IN THE SAME FILE, and only the
        # comments are allowed to name a dead path.
        if is_explanation(p, line):
            continue
        bad.append(f"{p}:{lineno}: stale path docs/{rel} (no such file) — "
                   f"the file moved; repoint the reference")

for b in sorted(set(bad)):
    print(b)
PY
    )

    if [ -z "$doc_links" ]; then
        ok "every documentation link and anchor resolves"
    else
        bad "dead documentation link(s) — a reader following these finds nothing:"
        printf '%s\n' "$doc_links" | sed 's/^/         /'
    fi
}
