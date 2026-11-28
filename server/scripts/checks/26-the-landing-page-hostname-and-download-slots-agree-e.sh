#!/usr/bin/env bash
# Section 26 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_26() {
    # ─────────────────────────────────────────────────────────────
    # 25. Front-matter conformance — status is structure, not a banner.
    #
    # WHY THIS EXISTS
    #
    # `docs/README.md` rule 1 and `docs/STATE.md` rule 3 both say a document's status
    # is a front-matter FIELD (`audience:` / `status:`), not a prose banner, and that
    # a retired document lives in `archive/`/`history/`, where the FOLDER carries the
    # status. Nothing enforced it, so the rule held only where someone remembered it.
    #
    # It came apart in the redesign corpus: 17 files under `docs/business/redesign/`
    # and its `implementation/` subdirectory had a banner but no `status:` field, so
    # a reader — and any guard — had to parse prose to learn whether a file described
    # live code or a removed design. Three of them described the *removed* device
    # binding as if live. That is exactly the `dev-vs-code-drift` class this repo
    # keeps re-learning: the doc was right when written and the world moved.
    #
    # WHAT IT ENFORCES, and why each is safe to require:
    #   * Every `.md` under `docs/` and `client/docs/` EXCEPT `archive/` and
    #     `history/` opens with a fenced front-matter block carrying `audience:` and
    #     `status:`.
    #   * A file under `archive/` or `history/` need NOT (the folder is the status) —
    #     but if it DOES, that is fine (LAYOUT.md keeps its own).
    #   * `status:` is one of the three values the rule names.
    #   * Also: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md` at the repo root are
    #     exempt by path — they are the entry points and predate the convention.
    #
    # ─────────────────────────────────────────────────────────────
    # 26. The landing page: one hostname, four download slots, two sides agreeing.
    #
    # WHY THIS EXISTS
    #
    # The landing page is a second hostname on the hub box, and it introduces exactly
    # the class of drift this script is for. Two things must agree and NOTHING in
    # the tree made them:
    #
    #   (a) THE HOSTNAME. It is written in four places with four different jobs:
    #       the Caddyfile generator (which block to serve), the manual-reference
    #       template (so a hand-built hub matches), docs/state.toml (the recorded
    #       fact), and the page itself (the canonical link, which is what a search
    #       engine treats as the page's identity). Three of those can be edited
    #       without the fourth, and the failure is quiet: a canonical link naming a
    #       host that serves nothing tells Google the real page is elsewhere.
    #
    #   (b) THE DOWNLOAD FILENAMES. The page's four buttons are rendered from the
    #       release manifest's platforms map — the same map publish-release.sh's
    #       PLATFORMS list and fetch-release.py resolve against. The macOS entries
    #       name .app.tar.gz tarballs, and the Windows entry is the NSIS installer
    #       rather than the raw .exe. Getting one wrong is not a visible break: the
    #       button renders, the page loads, and the download 404s.
    #
    # THIS SECTION CHECKS THE AGREEMENT, NOT EACH SIDE. That is section 24's lesson,
    # applied here: a check that each half is individually valid passes happily while
    # the two halves disagree — which is how the v3.2.12 Windows outage shipped.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "26. The landing page — hostname and download slots agree everywhere"

    # Read the entry by scanning from its header to the next header, NOT a fixed
    # number of lines: this entry carries a long comment block, and a fixed offset
    # silently returned nothing (the guard fired with "<missing>" on its first run,
    # which is how the bug was found — worth keeping the shape that exposed it).
    #
    # The value is taken with an explicit quote split rather than a greedy gsub: the
    # first attempt used `gsub(/.*"|".*/,"")`, which strips the whole line because
    # the second alternative matches a quote followed by anything, including the
    # closing quote and the value between them.
    LANDING_TOML=$(awk '/^\[site\.domain\]/{f=1;next} /^\[/{f=0} f && /^value/{n=split($0,a,"\"");print a[2];exit}' \
        "$REPO/docs/state.toml" 2>/dev/null)
    LANDING_MODULE=$(grep -oE 'LANDING_DOMAIN="\$\{LANDING_DOMAIN:-[^}]*\}"' "$REPO/server/modules/05-caddy.sh" 2>/dev/null \
        | sed 's/.*:-\(.*\)}"/\1/' | head -1)
    # The landing block's address line may carry SEVERAL names (the canonical one and
    # the deprecated alias kept serving until DNS moves), so take the first of the
    # comma-separated list on the one block that is not the hub's placeholder.
    #
    # Read with awk, not grep: this grep build needs a bracket expression's last
    # element to be an ordinary member (a trailing `-` or `.` makes the reader match
    # nothing, silently), and the address line may carry several comma-separated
    # names. awk has no such ambiguity.
    LANDING_TEMPLATE=$(awk '
        /^[[:alnum:].-]+(, *[[:alnum:].-]+)* *\{$/ && $0 !~ /domain/ {
            sub(/ *\{$/, ""); split($0, names, ","); gsub(/ /, "", names[1]); print names[1]; exit
        }' "$REPO/server/templates/Caddyfile" 2>/dev/null)
    LANDING_PAGE=$(grep -oE '<link rel="canonical" href="https://[^/"]+' "$REPO/server/site/index.html" 2>/dev/null \
        | sed 's|.*https://||' | head -1)

    echo "  state.toml      : ${LANDING_TOML:-<missing>}"
    echo "  05-caddy.sh     : ${LANDING_MODULE:-<missing>}"
    echo "  templates/      : ${LANDING_TEMPLATE:-<missing>}"
    echo "  site/index.html : ${LANDING_PAGE:-<missing>}"

    if [ -z "$LANDING_TOML" ] || [ -z "$LANDING_MODULE" ] || [ -z "$LANDING_TEMPLATE" ] || [ -z "$LANDING_PAGE" ]; then
        bad "landing hostname could not be read from all four sites — one is missing"
    else
        LAND_OK=1
        [ "$LANDING_MODULE" = "$LANDING_TOML" ]   || { bad "05-caddy.sh LANDING_DOMAIN='${LANDING_MODULE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
        [ "$LANDING_TEMPLATE" = "$LANDING_TOML" ] || { bad "templates/Caddyfile block '${LANDING_TEMPLATE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
        [ "$LANDING_PAGE" = "$LANDING_TOML" ]     || { bad "site/index.html canonical '${LANDING_PAGE}' != state.toml site.domain='${LANDING_TOML}'"; LAND_OK=0; }
        [ "$LAND_OK" = "1" ] && ok "landing hostname agrees across all four sites"
    fi

    # The four slots must exist in the page, one per FROZEN platform key. A missing
    # slot is three buttons instead of four, which no other check would notice.
    MISSING_SLOTS=""
    for k in linux windows macos_intel macos_arm; do
        grep -q "data-download=\"${k}\"" "$REPO/server/site/index.html" 2>/dev/null \
            || MISSING_SLOTS="$MISSING_SLOTS $k"
    done
    if [ -n "$MISSING_SLOTS" ]; then
        bad "server/site/index.html is missing download slot(s):${MISSING_SLOTS}"
        bad "  Each platform key needs one — they are the keys section 1 pins."
    else
        ok "index.html carries all four download slots"
    fi

    # And the renderer must cover the same set. A slot the renderer does not know
    # about ships as href="#" — a dead button on a page that otherwise looks right.
    RENDER_SLOTS=$(grep -oE '^SLOTS = \[.*\]' "$REPO/server/scripts/deploy-site.sh" 2>/dev/null \
        | grep -oE '"[a-z_]+"' | tr -d '"' | sort | tr '\n' ' ')
    EXPECT_SLOTS="linux macos_arm macos_intel windows "
    if [ "$RENDER_SLOTS" != "$EXPECT_SLOTS" ]; then
        bad "deploy-site.sh SLOTS='${RENDER_SLOTS:-<missing>}' != expected '${EXPECT_SLOTS}'"
        bad "  The renderer and the page must cover the same four platform keys."
    else
        ok "deploy-site.sh renders the same four slots the page carries"
    fi

    # The landing-host rewrite must run AFTER the rendered html is written to disk.
    #
    # This ordering is invisible in the output whenever the source page already names
    # LANDING_DOMAIN, which it did for the whole life of the script — so the bug that
    # the write sat last (`index.write_text(html)` *after* the re.sub loop) was
    # undetectable by reading a rendered page. It surfaced only when the Chrysalis
    # rename made the hardcoded host and the deployed host disagree, and the served
    # page then advertised a canonical URL, an og:image and a twitter:image all on a
    # hostname with no DNS. The symptom of a canonical pointing somewhere dead is
    # nothing at all — it is a hint to a crawler, not a request the page makes.
    #
    # Asserted on line order because that is the fact: the write must precede the
    # rewrite, and the guard must follow both.
    DS_WRITE=$(grep -n '^index\.write_text(html)$' "$REPO/server/scripts/deploy-site.sh" 2>/dev/null | head -1 | cut -d: -f1)
    DS_REWRITE=$(grep -n '^LANDING_URL = ' "$REPO/server/scripts/deploy-site.sh" 2>/dev/null | head -1 | cut -d: -f1)
    DS_GUARD=$(grep -n '^final = (pathlib\.Path(stage) / "index\.html")\.read_text()$' "$REPO/server/scripts/deploy-site.sh" 2>/dev/null | head -1 | cut -d: -f1)
    if [ -z "$DS_WRITE" ] || [ -z "$DS_REWRITE" ] || [ -z "$DS_GUARD" ]; then
        bad "deploy-site.sh: cannot locate the html write, the landing-URL pattern or the guard"
        bad "  write='${DS_WRITE:-<missing>}' rewrite='${DS_REWRITE:-<missing>}' guard='${DS_GUARD:-<missing>}'"
        bad "  Renaming any of these without re-pointing this check removes the only"
        bad "  thing that notices the rewrite being clobbered."
    elif [ "$DS_WRITE" -ge "$DS_REWRITE" ] || [ "$DS_GUARD" -le "$DS_REWRITE" ]; then
        bad "deploy-site.sh writes html at line ${DS_WRITE}, rewrites hosts at ${DS_REWRITE}, guards at ${DS_GUARD}"
        bad "  Required order: write < rewrite < guard. The write carries an in-memory"
        bad "  copy taken before the rewrite, so writing it after silently discards the"
        bad "  rewrite and the served page names whatever host the source hardcoded."
    else
        ok "deploy-site.sh writes html (${DS_WRITE}) before rewriting hosts (${DS_REWRITE}), guard after (${DS_GUARD})"
    fi

    # The landing block must never serve the hub's surfaces. This is the property a
    # future "just add a handle" edit removes with no symptom on the page itself.
    #
    # Read by LINE RANGE from the LAST non-hub address line to EOF, capturing
    # `handle` only at the block's own indentation. Scoping matters here: the hub
    # block precedes this one and legitimately contains handles, so reading the whole
    # file (or every chunk) would report the hub's handles as the landing page's.
    if awk '
        { line[NR] = $0; if ($0 ~ /^[[:alnum:].-]+(, *[[:alnum:].-]+)* *\{$/ && $0 !~ /domain/) last = NR }
        END { for (j = last; j <= NR; j++) print line[j] }
    ' "$REPO/server/templates/Caddyfile" 2>/dev/null \
        | grep -qE '^    (handle|handle_path|reverse_proxy|try_files)'; then
        bad "the landing block in templates/Caddyfile contains a handle/proxy directive"
        bad "  It must be a bare file_server: the hub's /updates/, /api/* and /admin/"
        bad "  must not be reachable from the indexable hostname."
    else
        ok "landing block serves static files only (no handle, no proxy)"
    fi
}
