#!/usr/bin/env bash
# Section 50 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_50() {
    echo "50. Every hub route the hooks serve is documented in API.md, and nothing else is"

    # ── WHY THIS GUARD EXISTS ──
    #
    # `§33`, `§34`, `§37` and `§38` all guard a pairing across a boundary: the status
    # struct against its TS interface, the IPC names across three sites, the frozen
    # snapshot, the generated types. The hub's OWN route table had no such pairing.
    # A `routerAdd` added tomorrow would be live on the hub and absent from
    # `docs/reference/API.md`, and every gate would stay green — the same shape as
    # `uot_port` vs `server_port_uot`, one layer up.
    #
    # ── WHAT THIS CHECKS, AND THE TWO EXCEPTIONS ──
    #
    # (a) The `## N.` route sections of API.md and the `routerAdd` set are equal,
    #     after removing the routes something OTHER than a hook serves. Those are
    #     named, closed, and each with its owner:
    #
    #       /api/health          PocketBase builtin (modules/06-pocketbase.sh:143)
    #       /_/                  PocketBase admin UI (state.toml [hub.pocketbase_ui])
    #       /api/admin/fetch-release, /api/admin/fetch-link
    #                            fetch-release.py, a separate systemd service
    #                            (Caddyfile:86 proxies /api/admin/fetch-release to it)
    #       /update.json         static file (API.md §8: "informational only")
    #
    #     The exclusion is a PROXY, not an exemption: each of those four still has
    #     to appear in the summary block, which (b) asserts.
    #
    # (b) Every route in the protocol summary block (`## 10.`) is a known route or a
    #     declared static path. That block is the table a reader actually reads, and
    #     it is where a route used to be added without a section.
    #
    # NOT checked: that a documented route's SEMANTICS match the hook. Same limit as
    # `§28` — this is a name pairing, not an oracle.
    # ─────────────────────────────────────────────────────────────
    route_report=$(python3 - "$REPO" <<'PY'
import glob, re, sys

repo = sys.argv[1]
hooks = sorted(glob.glob(f"{repo}/server/pb_hooks/*.pb.js"))
api = open(f"{repo}/docs/reference/API.md", encoding="utf-8").read()

served = set()
for path in hooks:
    src = open(path, encoding="utf-8").read()
    for method, route in re.findall(
            r'routerAdd\(\s*"([A-Z]+)"\s*,\s*"([^"]+)"', src):
        served.add(route)

# Served by PocketBase itself, by fetch-release.py, or as a static file.
NON_HOOK = {
    '/api/health':                'PocketBase builtin (modules/06-pocketbase.sh)',
    '/_/':                        'PocketBase admin UI (state.toml [hub.pocketbase_ui])',
    '/api/admin/fetch-release':   'fetch-release.py service (Caddyfile:86)',
    '/api/admin/fetch-link':      'fetch-release.py service',
    '/update.json':               'static file (API.md section 8)',
}

# (a) Route sections. A route is documented EITHER as a `### `METHOD /path``
# heading (the normal shape) OR as a bare `METHOD /path` line inside a fenced
# block — `/api/admin/console` is the second, because it is one endpoint with an
# `action` discriminator rather than one route per operation.
#
# So the scan is over the whole `## N.` section body, not over its subheadings.
# The first draft read only `##` titles and found zero routes; the second read
# only `###` headings and missed this one. Both were the same error in opposite
# directions — scoping the reader narrower than the thing it claims to cover,
# which is §15's lesson and `§34` carries a comment about it one section up.
#
# A section is bounded by the next `## ` line, so a route cannot be attributed
# to two sections.
sections = set()
blocks = re.split(r'^## ', api, flags=re.M)[1:]
for block in blocks:
    title, _, body = block.partition('\n')
    if not re.match(r'[0-9]+[a-z]?\.', title):
        continue          # "Base URL", "Error Response Format", …
    for route in re.findall(r'(?:GET|POST|PUT|DELETE)\s+(/[A-Za-z0-9/_.-]*)', body):
        if route and route != '/':
            sections.add(route)

# ── HEAD GUARDS: NAME THE FAILING EXTRACTOR, NOT THE DOCUMENT ──
#
# Added 2026-11-22 (audit F-20). Without these, a scanner that returned nothing
# reported as the REVERSE complaint: `served` empty surfaced as "API.md documents
# N routes and no hook or named service serves it" — nine lines blaming the
# document when the fault was the regex. A guard whose failure message points at
# the wrong file is the F-12/F-08 defect class in a new place.
#
# They run AFTER both sets are built, because a set that is not computed yet
# cannot be tested — the first draft sat above the `sections` loop and raised
# `NameError: name 'sections' is not defined`, which the shell reported as a
# Python traceback and a green §50. That is itself the defect: a broken scanner
# whose failure looked like a pass one line up.
#
# `NON_HOOK` has no guard on purpose: it is a literal dict in this heredoc, not a
# parse of anything. Five hand-written keys cannot come back empty, and a guard
# over a literal is the always-passes shape §29 warns about.
if not served:
    print("BAD\tthe routerAdd extraction returned nothing — the scanner is broken, not the hooks")
if not sections:
    print("BAD\tthe API.md section scan returned nothing — the scanner is broken, not the document")

# ── EXTRACTOR SELF-TEST ──
#
# Same pattern as §29's "a guard that always passes ... reads exactly like one that
# passed". The guards above catch an EMPTY result; they cannot tell a working
# scanner from one that silently matches the wrong thing. So the extractor runs
# over a fixture with known contents and must return exactly them.
_FIXTURE = '''
routerAdd("POST", "/api/alpha", function (e) { return e.json(200, {}); })
routerAdd("GET", "/api/beta", function(e) { return e.json(200, {}); })
routerAdd("GET", "/api/alpha", function (e) {})
'''
_expect = {'/api/alpha', '/api/beta'}
_got = {route for _method, route in
        re.findall(r'routerAdd\(\s*"([A-Z]+)"\s*,\s*"([^"]+)"', _FIXTURE)}
if _got != _expect:
    print("BAD\tthe routerAdd scanner is not reading what it claims: fixture expected "
          "%s, extracted %s" % (sorted(_expect), sorted(_got)))

for route in sorted(served - sections):
    print("BAD\tthe hooks serve %s and API.md has no section for it" % route)

# A documented path that is a DIRECTORY of static files is not a route, and it is
# named here rather than exempted by regex: `/updates/` is the prefix the release
# artifacts are served under (`API.md` section 7), and the summary block already
# states it as `/updates/<version>/<file>`.
STATIC_PREFIXES = ('/updates/',)

for route in sorted(sections - served - set(NON_HOOK)):
    if any(route == p or route.startswith(p) for p in STATIC_PREFIXES):
        continue
    print("BAD\tAPI.md documents %s and no hook or named service serves it" % route)

# (b) The protocol summary block: fenced lines carrying METHOD /path.
m = re.search(r'^## [0-9]+[a-z]?\.\s+Client[^\\n]*Summary', api, re.M)
if not m:
    print("BAD\tthe protocol summary section has moved or been renamed — (b) cannot run")
else:
    block = api[m.end():]
    fence = re.search(r'```(.*?)```', block, re.S)
    if not fence:
        print("BAD\tthe protocol summary section has no code block — (b) cannot run")
    else:
        # The block is padded with │/─ box-drawing characters, so the path token
        # must stop at the first character that cannot be in a route.
        for verb, route in re.findall(
                r'\b(GET|POST|PUT|DELETE)\s+(/[A-Za-z0-9/_.<>-]*)', fence.group(1)):
            if not route or route == '/':
                continue
            if route in ('/updates/<version>/<file>',):
                continue          # a static directory listing, not an endpoint
            if route not in served and route not in NON_HOOK:
                print("BAD\tthe protocol summary lists %s %s, which nothing serves" % (verb, route))
PY
    )

    if [ -z "$route_report" ]; then
        ok "every route the hooks serve is documented, and every documented route is served"
    else
        while IFS="$(printf '\t')" read -r _kind message; do
            bad "routes: $message"
        done <<EOF
$route_report
EOF
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
