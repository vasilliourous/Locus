#!/usr/bin/env bash
# Section 53 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_53() {
    # -------------------------------------------------------------
    # 53. The retained `locus` names are exactly the ones the register documents
    #
    # WHY THIS EXISTS. On 2026-11-23 the product was renamed Chrysalis and a set of
    # low-level names deliberately stayed where a deployed client can see them. Each
    # one is load-bearing: an installed client's update identity, a published
    # artefact name the hub resolves, the signing-secret key names, the header that
    # keeps tunnelled clients in separate rate-limit buckets, and one landing-page
    # hostname that resolves today while its replacement does not yet.
    #
    # That makes the tree fail the obvious completeness test: a later reader greps
    # for `locus`, finds these, and concludes the rename was left half-finished. Two
    # failure modes follow, and this section catches both:
    #
    #   1. **Completing the rename.** Someone renames `com.locus.client` or
    #      `locus-linux-amd64`. The tree still builds and every Rust and frontend
    #      test still passes; installed clients stop updating and stop activating.
    #      No test can catch it, because the client that breaks is not in this
    #      checkout - only the fleet has it.
    #   2. **Deleting the register.** Someone decides the document is stale, removes
    #      it, and the reason those names stayed goes with it.
    #
    # WHAT IT DOES NOT DO: forbid every `locus` in the tree. `legacy/`,
    # `docs/archive/`, `vendor/` (a geosite category tag) and dated quotations in the
    # defect log are all legitimately old. This asserts only the deployed set named
    # in the register's section 3.
    #
    # See docs/reference/NAME-MIGRATION.md.
    # -------------------------------------------------------------
    echo
    echo "53. Retained 'locus' names match the register"

    REGISTER="$REPO/docs/reference/NAME-MIGRATION.md"
    if [ ! -f "$REGISTER" ]; then
        bad "docs/reference/NAME-MIGRATION.md is missing - the record of WHICH names stay"
        bad "  'locus', and why, is gone. The next grep will 'finish' the rename and break"
        bad "  the deployed fleet. Restore it from git."
    else
        retain_report=$(cd "$REPO" && python3 - <<'PY'
import os
import re

def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return None

# (register anchor, file, expected literal)
#
# The anchor is the substring the register uses to introduce that name, so a row
# dropped from the register fails here separately: the two halves cannot drift
# into agreeing on a shorter list.
#
# The literal is what the file must hold, and it is NOT always the old spelling.
# Three kinds of row live in this one list:
#
#   * a name that was KEPT -> the literal is the `locus` spelling;
#   * a name that was RETIRED -> the literal is the `chrysalis` spelling, and a
#     `locus` reappearing there is the defect, because the deployed half and the
#     fleet still read or write it (the bootstrap header, /etc/chrysalis,
#     chrysalis-fetch.service, the ssh-alias probe);
#   * a name with a FORM check -> a sentinel, inspected below.
#
# WHY THE LIST GREW. The register named the ssh alias as `chrysalis-hub` for all
# three scripts while deploy.sh, publish-release.sh and verify-release.sh each
# still said `locus-hub`: a documented fact disagreeing with the code, which is
# the one thing this page exists to prevent. A single row per NAME covers the
# class and not the instances, so every row that repeats a file exists on purpose.
CHECKS = [
    # ── Retained spellings: the `locus` form must survive ──
    # The identifier moved on 2026-11-25. What must still hold is the part that
    # makes the move survivable: the OLD value is still a migration candidate in
    # `dirs.rs` (`LEGACY_APP_IDS`), because every install before that date keeps
    # its activation code, device fingerprint and tier under it. The config files
    # now carry the current value, so they are asserted separately below.
    ("com.locus.client",         "client/src-tauri/src/utils/dirs.rs",        "com.locus.client"),
    ("com.locus.client.dev",     "client/src-tauri/src/utils/dirs.rs",        "com.locus.client.dev"),
    ("com.chrysalis.client",     "client/src-tauri/tauri.conf.json",          "com.chrysalis.client"),
    ("com.chrysalis.client",     "client/src-tauri/src/utils/dirs.rs",        "com.chrysalis.client"),
    ("locus-backup",             "client/src-tauri/src/utils/dirs.rs",        "locus-backup"),
    # ── The artefact names moved on 2026-11-25: BOTH spellings are live ──
    #
    # Each file below must name the CURRENT (`chrysalis-*`) spelling, because that
    # is what CI now builds and what the hub prefers. `contract.rs` is the one
    # exception: it names the current spelling only, and its doc comment states
    # that nothing resolves a URL through it — so it has no legacy twin by design.
    #
    # The legacy spellings are asserted separately, as a class, by the
    # `LEGACY_PLATFORMS` block check further down and by contract-snapshot.py's
    # `legacy_artifacts`, rather than as per-file rows: a row per file per name
    # generation is 8 rows that all say the same thing, and the pair check states
    # the property (both resolve) once instead of eight times.
    ("chrysalis-linux-amd64",     "client/src-tauri/src/chrysalis/contract.rs", "chrysalis-linux-amd64"),
    ("chrysalis-linux-amd64",     "server/scripts/fetch-release.py",           "chrysalis-linux-amd64"),
    ("chrysalis-linux-amd64",     "server/scripts/publish-release.sh",         "chrysalis-linux-amd64"),
    ("chrysalis-linux-amd64",     "server/scripts/verify-release.sh",          "chrysalis-linux-amd64"),
    # The console's release read-out moved under Settings in the redesign. This
    # row follows the file: a row pointing at a path that no longer exists fails
    # as "missing" — correct, but it stops checking the literal it is here for.
    ("chrysalis-linux-amd64",     "server/console/src/views/settings/ReleasesPanel.vue", "chrysalis-linux-amd64"),
    ("LOCUS_UPDATE_KEY_B64",     "server/scripts/chrysalis-key-extract.sh",   "LOCUS_UPDATE_KEY_B64"),
    # The encrypt-side FORM CHECK, not just a doc: a required-var list naming the
    # wrong spelling refuses every re-encrypt with "LOCUS_UPDATE_KEY_B64(absent)".
    # Scoped to the `REQUIRED_VARS` block, because a plain substring match would be
    # satisfied by the explanatory COMMENT above it — which is how the first draft
    # of this row passed against the very defect it exists to catch.
    ("LOCUS_UPDATE_KEY_B64",     "server/scripts/secrets-encrypt.sh",          "REQUIRED_VARS"),
    ("LOCUS_UPDATE_KEY",         ".github/workflows/client.yml",              "LOCUS_UPDATE_KEY"),
    ("vasilliourous/Locus",      "server/scripts/fetch-release.py",           "vasilliourous/Locus"),

    # ── Retired spellings: the `chrysalis` form must be what is READ/WRITTEN ──
    #
    # The bootstrap header is an ASYMMETRIC pair, and asserting it symmetrically
    # was wrong. The path filter (09-activation-tunnel.sh) only ever SETS the
    # header, so it names the current spelling and nothing else. The hooks only
    # ever READ it, so during a deploy window they accept BOTH — a deployed hub
    # and a deployed client can be on opposite sides of the rename, and if the
    # reader accepts only one spelling the header is silently absent and every
    # tunnelled client collapses into one rate-limit bucket.
    #
    # So: the writer must send the current name, and each reader must still contain
    # the old one. The last two rows are the `X-Locus-Bootstrap` rows re-expressed
    # as what they actually need to be.
    ("X-Locus-Bootstrap",        "server/modules/09-activation-tunnel.sh",    "X-Chrysalis-Bootstrap"),
    ("x-locus-bootstrap",        "server/pb_hooks/activation.pb.js",          "x-locus-bootstrap"),
    ("x-locus-bootstrap",        "server/pb_hooks/code_lookup.pb.js",         "x-locus-bootstrap"),
    ("x-chrysalis-bootstrap",    "server/pb_hooks/activation.pb.js",          "x-chrysalis-bootstrap"),
    ("x-chrysalis-bootstrap",    "server/pb_hooks/code_lookup.pb.js",         "x-chrysalis-bootstrap"),
    ("/etc/locus/",              "server/modules/09-activation-tunnel.sh",    "/etc/chrysalis/"),
    ("locus-fetch.service",      "server/modules/05-caddy.sh",                "chrysalis-fetch.service"),
    # The ssh alias. The probe tries the NEW alias; the bare host is the fallback
    # for a machine that has not added it yet.
    ("locus-hub",                "server/scripts/deploy.sh",                  "chrysalis-hub"),
    ("locus-hub",                "server/scripts/publish-release.sh",         "chrysalis-hub"),
    ("locus-hub",                "server/scripts/verify-release.sh",          "chrysalis-hub"),
    # The config field a client is pointed at the hub alias with. Renamed in the
    # rename; the log line that tells an operator to set it said `locus_...`
    # until the stale-prose check below was added.
    ("chrysalis_hub_override",   "server/modules/05-caddy.sh",                "chrysalis_hub_override"),
    ("chrysalis_hub_override",   "client/src-tauri/src/config/verge.rs",      "chrysalis_hub_override"),
]

# The FOUR scripts that accept the pre-rename passphrase variable. Asserted as a
# class: each must name BOTH spellings. A fallback deleted from one of them is a
# host that can no longer deploy, and nothing else in this tree would notice.
PASSPHRASE_FALLBACK_FILES = [
    "server/setup.sh",
    "server/restore.sh",
    "server/scripts/secrets-decrypt.sh",
    "server/scripts/secrets-encrypt.sh",
]

# Files that still WRITE a `locus` spelling where the rename moved it. These are
# not retained names and have no register row: they are prose telling an operator
# or a reader to use an identifier that no longer exists. A named list, not a
# blanket ban - the register explicitly cannot promise "no locus anywhere"
# (legacy/, vendor/, docs/archive/ are all legitimately old).
NO_STALE_PROSE = [
    ("server/modules/05-caddy.sh",       r"locus_hub_override"),
    ("client/src/services/chrysalis.ts", r"locus[A-Za-z_]*"),
]

reg = read("docs/reference/NAME-MIGRATION.md") or ""

def required_vars_block(body):
    """The `REQUIRED_VARS="` list on its own, without any surrounding comment.

    Deliberately narrow: the list is what the script ACTUALLY checks, so naming
    the wrong spelling there is the defect. A comment that explains the old
    spelling must not make this guard pass.
    """
    m = re.search(r'^REQUIRED_VARS="(.*?)^"', body, re.S | re.M)
    return m.group(1) if m else None

for anchor, path, literal in CHECKS:
    body = read(path)
    if body is None:
        print("BAD\t%s is missing - the retained name it holds cannot be checked" % path)
        continue
    if literal == "REQUIRED_VARS":
        # THE ORIGINAL FORM OF THIS CHECK IS NO LONGER THE RIGHT ONE, and the
        # reason is the defect it was written for. It asserted that
        # `REQUIRED_VARS` lists the literal `LOCUS_UPDATE_KEY_B64`, because a
        # bundle of that era carries that spelling and a re-encrypt would have
        # failed the form check with `LOCUS_UPDATE_KEY_B64(absent)`.
        #
        # On 2026-11-25 the entry left `REQUIRED_VARS` and became an EITHER-OF
        # check just above the loop, for the reason the old row could not express:
        # the name lives INSIDE the committed ciphertext, so a bundle written
        # before the move carries one spelling and one re-encrypted after it
        # carries the other. Demanding either literal refuses half of all bundles.
        #
        # So the assertion moves to what must now be true, and it is stated as a
        # property rather than a spelling: the script must (a) still locate its
        # form check, (b) accept BOTH names, and (c) refuse the ambiguous case —
        # both present — because "last definition wins" is how a shadowed key
        # ships silently. Each half is asserted separately so deleting either one
        # fails, which is the failure mode the old row existed to prevent.
        block = required_vars_block(body)
        if block is None:
            print("BAD\t%s has no REQUIRED_VARS list - the encrypt form check cannot be located" % path)
        elif "LOCUS_UPDATE_KEY_B64" in block:
            print("BAD\t%s lists LOCUS_UPDATE_KEY_B64 in REQUIRED_VARS again." % path)
            print("BAD\t  The key name is either-of (bundle-dependent), so a required")
            print("BAD\t  literal refuses every bundle of the other generation.")
        elif "CHRYSALIS_UPDATE_KEY_B64" not in body or "LOCUS_UPDATE_KEY_B64" not in body:
            print("BAD\t%s does not accept BOTH signing-key spellings." % path)
            print("BAD\t  Each bundle carries exactly one, so both must be readable.")
        elif "ambiguous" not in body:
            print("BAD\t%s does not refuse a bundle carrying BOTH spellings." % path)
            print("BAD\t  The last definition wins, so a shadowed key would ship.")
        else:
            print("OK\t%s - either-of signing-key check present (both spellings, ambiguity refused)" % path)
        continue
    if literal not in body:
        print("BAD\t%s no longer contains `%s`." % (path, literal))
        print("BAD\t  Either a retained name was renamed (a contract with deployed")
        print("BAD\t  clients or a live hostname) or a retired one was not applied.")
        print("BAD\t  If it was deliberately retired, change its row in")
        print("BAD\t  docs/reference/NAME-MIGRATION.md in the SAME change.")
    else:
        print("OK\t%s - `%s` present" % (path, literal))

# ── The passphrase fallback, as a class rather than four rows ──
#
# THE OBVIOUS VERSION OF THIS CHECK DOES NOT WORK, and it shipped first anyway.
# `"LOCUS_SECRETS_PASSPHRASE" in body` is satisfied by the alias's own NAME being
# mentioned anywhere: the error string, the PASS_SRC label, the comment above the
# block. Deleting the fallback branch therefore left SEVEN other mentions of the
# identifier in restore.sh and the check still said OK. That is the same defect
# §53's REQUIRED_VARS row was narrowed for one section earlier, so the rule is
# applied the same way here: match the `elif` LINE that makes the variable live,
# not the file.
FALLBACK_RE = re.compile(
    r'el(?:if|se if)\s+\[\s+-n\s+"\$\{(LOCUS_SECRETS_PASSPHRASE(?:_FILE)?)[^}]*\}"\s+\]')


def fallback_lines(body):
    """The `elif` branches that actually make the old spellings live."""
    return set(m.group(1) for m in FALLBACK_RE.finditer(body))


for path in PASSPHRASE_FALLBACK_FILES:
    body = read(path)
    if body is None:
        print("BAD\t%s is missing - the passphrase fallback cannot be checked" % path)
        continue
    found = fallback_lines(body)
    reads_new = re.search(r'CHRYSALIS_SECRETS_PASSPHRASE(?:_FILE)?', body) is not None
    if not reads_new:
        print("BAD\t%s does not read `CHRYSALIS_SECRETS_PASSPHRASE*`." % path)
        print("BAD\t  The old spelling is a FALLBACK, not the primary: the name the")
        print("BAD\t  register documents must work on its own.")
        continue
    if "LOCUS_SECRETS_PASSPHRASE" in found:
        print("OK\t%s - falls back to `LOCUS_SECRETS_PASSPHRASE`" % path)
    elif "LOCUS_SECRETS_PASSPHRASE_FILE" in found:
        print("BAD\t%s falls back only to the _FILE spelling." % path)
        print("BAD\t  A host that sets the variable directly is no longer accepted.")
    else:
        print("BAD\t%s has no live `LOCUS_SECRETS_PASSPHRASE` fallback branch." % path)
        print("BAD\t  A host that was never re-provisioned still passes the old")
        print("BAD\t  name, and this script would now fail there. A mention in a")
        print("BAD\t  comment or an error string does not count - only the `elif`")
        print("BAD\t  that tests the variable does.")

# ── Prose that names something the rename moved ──
for path, pattern in NO_STALE_PROSE:
    body = read(path)
    if body is None:
        print("BAD\t%s is missing - its stale-name check cannot run" % path)
        continue
    hits = []
    for n, line in enumerate(body.splitlines(), 1):
        m = re.search(pattern, line)
        if m:
            hits.append((n, m.group(0), line.strip()))
    if not hits:
        print("OK\t%s - no stale `locus` spelling" % path)
        continue
    for n, word, line in hits[:4]:
        print("BAD\t%s:%d names `%s`, which the rename moved." % (path, n, word))
        print("BAD\t  %s" % line[:110])
    print("BAD\t  A reader is being told to use an identifier that does not exist.")

# ── No live document names an IPC command the client no longer registers ──
#
# WHY THIS EXISTS: `docs/reference/STILL-OPEN.md` told a future reader to "trace
# what happens after `locus_recognise` returns". There was no such command: device
# recognition was removed from both sides, and `cmd/chrysalis.rs` says so where it
# stood ("`chrysalis_recognise`, `resolve_identity`, `RecognitionResult` —
# REMOVED"). The entry survived every rename pass because §53's per-file rows are
# POSITIVE — they assert a retained name is still present where it is required —
# and a name that was *deleted* rather than *renamed* has no row to fail.
#
# The consequence is not cosmetic. An instruction to trace a function that does not
# exist cannot be followed, cannot be marked done, and quietly teaches a reader to
# distrust every other instruction on the page. This is the same class as
# "`locus` in a command that no longer resolves" (the repository check below), with
# a different subject.
#
# WHAT IT CHECKS: a live document that names a command must name a registered one.
# The subject is restricted, deliberately, to names the document marks as a COMMAND
# — `chrysalis_*`, `locus_*`, or a bare backticked name that is ALSO registered — so
# that a config field (`theme_mode`), a wire name (`uot_port`) or a Rust attribute
# (`deny_unknown_fields`) is never mistaken for one. The first draft of this guard
# did not restrict the subject and reported eleven BAD lines across `AGENTS.md` and
# `COLOURS.md`, every one of them a field or an attribute: a guard that cries wolf
# on correct prose is a guard somebody switches off, which is worse than absent.
#
# The `locus_*` arm is the one that catches the original defect class: a command
# that was *removed* leaves its old spelling in the prose, and the old spelling is
# exactly what the register keeps a row for.
#
# KNOWN LIMIT, stated rather than hidden: a command that was removed from the source
# AND is spelled under its NEW prefix (`chrysalis_recognise`) is indistinguishable
# from an unregistered field by name alone. That residual class is covered by the
# `cmd::`-registration scan in §34, which asserts every `chrysalis_*` name the client
# registers is one the docs may use — the two guards are complements, not duplicates.
import re as _re_ipc

_lib_path = "client/src-tauri/src/lib.rs"
try:
    _lib = open(_lib_path, encoding="utf-8", errors="replace").read()
except OSError:
    _lib = ""
# The handler body only — a name mentioned in a comment above the macro is not a
# registration, and taking the whole file would let a comment satisfy the check.
_m = _re_ipc.search(r"generate_handler!\[(.*?)\n\s*\]", _lib, _re_ipc.S)
_handler_body = _m.group(1) if _m else ""
_registered = set(_re_ipc.findall(r"(?:^|::)([a-z_][a-z0-9_]*)\s*,", _handler_body, _re_ipc.M))
# A few IPC names are registered from inside the host plugin, not this macro.
_registered |= {"get_system_info", "get_app_uptime", "export_diagnostic_info"}

# THE CONFIG FIELDS. `chrysalis_` prefixes BOTH the IPC commands and the persisted
# `IVerge` fields, so prefix alone cannot tell a reader's "call this" from a
# document's "this setting exists". Every field is declared in `config/verge.rs` as
# `pub <name>:`; parsing those declarations is what separates the two, instead of
# the ad-hoc exception list that the first draft would have needed and that would
# have grown one line per new field until it allowed everything.
try:
    _verge = open("client/src-tauri/src/config/verge.rs", encoding="utf-8", errors="replace").read()
except OSError:
    _verge = ""
_fields = set(_re_ipc.findall(r"^\s*pub\s+([a-z_][a-z0-9_]*)\s*:", _verge, _re_ipc.M))

_id_shaped = _re_ipc.compile(r"`((?:chrysalis|locus)_[a-z0-9_]+)`")
# TWO documents have old IPC names as their SUBJECT, and naming them is the point:
# `NAME-MIGRATION.md` is the register of what moved, and `FIXES.md` is an append-only
# defect log where "the cause was `locus_connect`" is a true record of a past state
# rather than an instruction to a reader. An exemption list is safe here — unlike a
# scope that stops at a fixed depth, it fails closed: a NEW document is scanned. Both
# are live, so §28 already keeps them from being silently deleted.
#
# `STILL-OPEN.md` is NOT exempt: it is instructions to a successor, and an
# unnameable target there is exactly the defect this guard was written for.
_IPC_EXEMPT_DOCS = {
    "docs/reference/NAME-MIGRATION.md",
    "docs/reference/FIXES.md",
}
_live_docs = []
for _r, _d, _fs in os.walk("."):
    _d[:] = [x for x in _d if x not in
             ("node_modules", ".git", "target", "dist", "legacy", "vendor",
              "archive", "history", ".hubtmp")]
    for _f in _fs:
        if not _f.endswith(".md"):
            continue
        _p = os.path.join(_r, _f).replace(os.sep, "/")
        if _p.startswith("./"):
            _p = _p[2:]
        if _p in _IPC_EXEMPT_DOCS:
            continue
        _live_docs.append(_p)

_dead_ipc = []
for _p in _live_docs:
    try:
        _body = open(_p, encoding="utf-8", errors="replace").read()
    except OSError:
        continue
    for _n, _line in enumerate(_body.splitlines(), 1):
        # A cross-reference to another document's section, e.g. `operate/page`.
        if _re_ipc.search(r"\.(md|sh|toml|json|rs|ts)\b", _line):
            continue
        # A HISTORICAL ALIAS, marked by the prose itself. `FIXES.md` and this doc set
        # record the previous spelling on purpose (`chrysalis_suspended` (then
        # `locus_suspended)`), and a defect log that says a name was always the new
        # one is a false record. The marker is required to be IN THE SAME LINE as the
        # old name, so it cannot be applied in bulk to a whole page — the first
        # instance of this defect (`STILL-OPEN.md`'s `locus_recognise`, which named a
        # command that had been *removed* rather than renamed and was marked
        # "(then …)" nowhere) is correctly still a BAD line.
        if "then `" in _line or "(then " in _line:
            continue
        for _cand in set(_id_shaped.findall(_line)):
            if _cand in _registered or _cand in _fields:
                continue
            _dead_ipc.append((_p, _n, _cand, _line.strip()))

if _dead_ipc:
    for _p, _n, _cand, _line in _dead_ipc[:6]:
        print("BAD\t%s:%d names `%s`, which no IPC handler registers." % (_p, _n, _cand))
        print("BAD\t  %s" % _line[:110])
    print("BAD\t  A reader is being told to call or trace a command that does not exist.")
else:
    print("OK\tno live document names an IPC command absent from `generate_handlers!` (scanned %d documents against %d registrations)"
          % (len(_live_docs), len(_registered)))

# ── The repository name is Locus, and it must never be written Chrysalis ──
#
# THE INVERSE OF EVERY ROW ABOVE, and the rename got it wrong. The product is
# Chrysalis; the GitHub repository was never renamed and is still
# `vasilliourous/Locus`. The 2026-11-23 pass applied `Locus -> Chrysalis` to the
# repository path in operator documents and in the fetch service's unit template,
# producing twelve commands that resolve a repository that does not exist:
#
#   gh secret set LOCUS_UPDATE_KEY --repo vasilliourous/Chrysalis      (SIGNING.md)
#   git clone https://github.com/vasilliourous/Chrysalis.git           (SESSION.md)
#   Environment=GITHUB_REPO=vasilliourous/Chrysalis                    (the template)
#
# `gh repo view vasilliourous/Chrysalis` answers "Could not resolve to a
# Repository", so every one of these fails for an operator following the docs.
#
# WHY IT SURVIVED. §53's CHECKS only assert that a retained name is PRESENT where
# it is required — a positive check reads nothing about a place the name was
# wrongly replaced. The rows are exhaustive by construction, so a new spelling
# introduced by a rename has no row to catch it. This is the missing direction,
# and it is written as a tree-wide ban because the name is global: unlike the
# per-file rows above, there is no file where `vasilliourous/Chrysalis` is right.
#
# The repository template's own `Documentation=` URL is included on purpose — it
# is a link an operator clicks, and a 404 there is the same defect as a failing
# command.
BAD_REPO = "vasilliourous/Chrysalis"
SCAN_SUFFIXES = (".md", ".sh", ".py", ".js", ".service", ".yml", ".json", ".toml", ".rs")
bad_repo_hits = []
for root, dirs, files in os.walk("."):
    dirs[:] = [d for d in dirs if d not in
               ("node_modules", ".git", "target", "dist", "legacy", "vendor",
                "archive", "history", ".hubtmp")]
    for fn in files:
        if not fn.endswith(SCAN_SUFFIXES):
            continue
        # The checker is excluded: this paragraph HAS to name the bad string to
        # explain it, so scanning itself would make the guard permanently red.
        if fn == "check-consistency.sh":
            continue
        p = os.path.join(root, fn).replace(os.sep, "/")
        # NOT `.lstrip("./")`. That strips CHARACTERS, not a prefix, so a dotfile
        # like `.renovation-handover.md` becomes `renovation-handover.md`, the read
        # below raises, and the whole §53 block dies — taking every check after it
        # with it, silently, while the run still printed "All consistency checks
        # passed" for the sections that had already run. Caught by planting the
        # defect this check exists for and watching nothing happen.
        if p.startswith("./"):
            p = p[2:]
        try:
            body = open(p, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        for n, line in enumerate(body.splitlines(), 1):
            if BAD_REPO in line:
                bad_repo_hits.append((p, n, line.strip()))
if bad_repo_hits:
    for p, n, line in bad_repo_hits[:6]:
        print("BAD\t%s:%d names `%s`, which is not a repository." % (p, n, BAD_REPO))
        print("BAD\t  %s" % line[:110])
    print("BAD\t  The GitHub repository is still `vasilliourous/Locus` — the product")
    print("BAD\t  was renamed, the remote was not. Every command here fails.")
    print("BAD\t  See docs/reference/NAME-MIGRATION.md §3.7.")
else:
    print("OK\tno file names the non-existent repository `%s`" % BAD_REPO)

# The anchor must appear on a LINE OF ITS OWN SECTION, not merely somewhere in the
# prose. A plain `anchor in reg` was satisfied by a passing mention: the register
# explains `com.locus.client` inside the row for `com.locus.client` itself, so a
# row deleted from the table still passed because the paragraph above quoted it.
# Requiring the anchor inside a `|`-led table row (or a heading/bold line for the
# prose-only rows) is what makes the check about the ROW.
seen_anchor = set()
for anchor, path_arg, literal in CHECKS:
    key = (anchor, path_arg)
    if key in seen_anchor:
        continue
    seen_anchor.add(key)
    row_like = [ln for ln in reg.splitlines()
                if anchor in ln and (ln.lstrip().startswith('|')
                                     or ln.lstrip().startswith('#')
                                     or ln.lstrip().startswith('*'))]
    if not row_like:
        print("BAD\tthe register has no table row documenting `%s`." % anchor)
        print("BAD\t  A mention in prose is not a row: it carries no 'Where', no")
        print("BAD\t  'Why' and no 'Retires when', which is the whole of section 3.")

# The register must keep its REASONING, not just its names: a stub that lists
# identifiers without a retirement condition is a second copy of a fact that can
# disagree with section 3 the moment someone edits one of them.
for phrase in ("Retires when", "updater identity", "LANDING_DOMAIN_LEGACY"):
    if phrase.lower() not in reg.lower():
        print("BAD\tthe register has lost its '%s' reasoning - it is listing names, not explaining them" % phrase)

# Every alias a reader will grep for and then "fix" needs a ROW. These four were
# in the tree but missing from the register before this guard existed; the tag
# says which half of the register they belong to, so a failure names the file to
# edit rather than repeating one message four times.
for phrase, where in (
    ("com.locus.client.dev",       "3.2"),
    ("chrysalis_hub_override",     "3.8"),
    ("LOCUS_SECRETS_PASSPHRASE_FILE", "3.5"),
):
    if phrase not in reg:
        print("BAD\tregister %s does not document `%s`." % (where, phrase))
        print("BAD\t  It is a retained alias with no stated reason, so the next")
        print("BAD\t  grep will 'finish' the rename and find out from a ticket.")

# The retirement rule needs a home, and it has to be dateable. Per-name conditions
# ("setup.sh re-run", "DNS moves") are the right shape for a claim about the
# world, but none of them is checkable and none names a version. Section 3.0 is
# where the release that drops the aliases is recorded.
if "## 3.0" not in reg:
    print("BAD\tthe register has no section 3.0 - the release that drops the")
    print("BAD\t  aliases has no home, so 'retires when' is prose and cannot be held to.")
else:
    print("OK\tthe register carries section 3.0, the dated retirement rule")
PY
    )
        if [ -z "$retain_report" ]; then
            bad "the retained-name check produced no output - it did not run"
        elif printf '%s' "$retain_report" | grep -q 'Traceback (most recent call last)'; then
            bad "the retained-name check CRASHED - a crashing guard reads like a passing one:"
            printf '%s\n' "$retain_report" | sed 's/^/        /' >&2
        else
            while IFS="$(printf '\t')" read -r _kind message; do
                case "$_kind" in
                    OK)  ok "$message" ;;
                    BAD) bad "$message" ;;
                    *)   bad "retained-names: $message" ;;
                esac
            done <<EOF
$retain_report
EOF
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
