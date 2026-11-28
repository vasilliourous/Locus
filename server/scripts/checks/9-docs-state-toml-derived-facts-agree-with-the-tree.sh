#!/usr/bin/env bash
# Section 9 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_9() {
    # ─────────────────────────────────────────────────────────────
    # 9. docs/state.toml — the derived facts still agree with the tree.
    #
    # WHY THIS EXISTS
    #
    # docs/STATE.md used to hold a table of "volatile facts" — a version, an IP, host
    # specs, a HEAD commit — and call itself the single source of truth for facts
    # that change. It was wrong in a way worth stating precisely, because it is the
    # second-most-common defect in this repo after cross-language drift:
    #
    #   The IP in that table went stale THREE times and finished by naming an address
    #   that did not exist. Nothing failed. Nothing noticed. A document asserting a
    #   travelling value in the present tense is indistinguishable, to a reader, from
    #   a document asserting a stable one.
    #
    # The fix is structural, not editorial: the facts moved into docs/state.toml
    # where each one declares a `source` and a `provenance`, and this check recomputes
    # every `provenance = "derived"` fact against the tree. A fact that cannot be
    # recomputed cannot be `derived`, and anything marked `unverified` is reported as
    # a world claim rather than silently trusted.
    #
    # WHAT IT ENFORCES: the value of each derived fact.
    # WHAT IT REPORTS: asserted facts (true by decision) and unverified ones (world
    # claims we could not check) — loudly, but without failing the build, because a
    # world claim being unverified is the honest state, not a defect. Failing on it
    # would make the honest choice the expensive one, and the next person would just
    # delete the marker.
    # ─────────────────────────────────────────────────────────────
    echo
    echo "9. docs/state.toml — derived facts agree with the tree"

    if [ ! -f "$REPO/docs/state.toml" ]; then
        bad "docs/state.toml is missing — the derived facts have no home"
    else
        # Parsed with tomllib (Python 3.11+) rather than grepped: the file is nested
        # TOML, and a regex reader would silently mis-read it the first time someone
        # reformats. If Python cannot parse it, that is itself the finding.
        state_report=$(cd "$REPO" && python3 - <<'PY'
import sys
try:
    import tomllib
except ModuleNotFoundError:
    print("PARSE_FAIL\tpython3 has no tomllib (needs 3.11+)")
    sys.exit(0)

try:
    with open("docs/state.toml", "rb") as fh:
        data = tomllib.load(fh)
except Exception as exc:
    print(f"PARSE_FAIL\t{exc}")
    sys.exit(0)

# Flatten to dotted paths, keeping only fact subtables (those with a `value`).
facts = {}
for section, body in data.items():
    if not isinstance(body, dict):
        continue
    for name, entry in body.items():
        if isinstance(entry, dict) and "value" in entry:
            facts[f"{section}.{name}"] = entry

# ── Recompute every derived fact from the tree. ──
def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return ""

import json, re

def manifest_version():
    try:
        with open("client/package.json", encoding="utf-8") as fh:
            return json.load(fh).get("version")
    except Exception:
        return None

def setup_domain():
    # setup.sh requires DOMAIN from the environment; the deployed value lives in
    # setup.sh's own invocation. We look for the literal the repo actually uses.
    m = re.search(r'DOMAIN="?([a-z0-9.-]+\.[a-z]{2,})"?', read("server/setup.sh"))
    if m:
        return m.group(1)
    # Fall back to the example in the required-env error text. This is the
    # canonical name now — the hub moved off duckdns in 3.3.2 because N4L blocked
    # it, and every other default in the tree points at the new name.
    m = re.search(r'([a-z0-9-]+\.jadedns\.uk)', read("server/setup.sh"))
    return m.group(1) if m else None

def legacy_hub_domain():
    """The hub's previous name, from the client's own constant.

    It is NOT dead: every build before 3.3.2 has it compiled in and cannot learn
    a new address, so the hub must keep answering on it. The Rust constant is the
    source because it is the thing that makes it load-bearing; §32 then asserts
    the server actually still serves it.
    """
    m = re.search(r'pub const LEGACY_HUB_URL: &str = "https://([^"]+)"',
                  read("client/src-tauri/src/chrysalis/contract.rs"))
    return m.group(1) if m else None

def publish_api_base():
    m = re.search(r'PB_API:-(\S+?)\}', read("server/scripts/publish-release.sh"))
    return m.group(1) if m else None

def setup_log_path(marker):
    """Pull the console/UI path out of setup.sh's own success log lines.

    setup.sh prints `Admin console: https://${DOMAIN}/admin/` and
    `PocketBase UI: https://${DOMAIN}/_/`. Those lines are the deploy's own
    statement of where it put the files, so they are the right source — not a
    constant we would have to keep in two places.
    """
    m = re.search(re.escape(marker) + r':\s*https://\$\{DOMAIN\}(/[^\s"\']*)', read("server/setup.sh"))
    return m.group(1) if m else None

def platform_keys():
    """The four platform keys, from the Rust contract — the frozen definition."""
    found = re.findall(r'pub const PLATFORM_[A-Z_]+: &str = "([a-z_]+)";',
                       read("client/src-tauri/src/chrysalis/contract.rs"))
    return found or None

def landing_domain():
    """The landing page hostname, from the Caddyfile generator.

    modules/05-caddy.sh is what actually writes the site block, so its
    LANDING_DOMAIN default is the source. The template and the page's canonical
    link are checked for AGREEMENT with this in section 26 — this function only
    answers "what does the deployed config say".
    """
    m = re.search(r'LANDING_DOMAIN="\$\{LANDING_DOMAIN:-([^}]+)\}"',
                  read("server/modules/05-caddy.sh"))
    return m.group(1) if m else None

def landing_domain_legacy():
    """The landing page's PREVIOUS hostname, from the Caddyfile generator.

    It is `derived` on purpose, for the same reason `hub.domain_legacy` is: the
    value is load-bearing only while an old DNS name resolves, so it must agree
    with what the daemon is actually told to serve. Empty means "dropped", which
    this returns as the empty string rather than None, so `value = ""` is a
    checkable answer and not an unverifiable one.
    """
    m = re.search(r'LANDING_DOMAIN_LEGACY="\$\{LANDING_DOMAIN_LEGACY-([^}]*)\}"',
                  read("server/modules/05-caddy.sh"))
    if m is None:
        return None
    return m.group(1)

def updater_artifacts():
    """The updater artefact names, current and pre-move, from the two sides.

    Recomputed from the files that DECIDE them rather than from a copy: the Rust
    constants for the current names, and the hub resolver's own `LEGACY_PLATFORMS`
    for the aliases. The `.app.tar.gz` shape is asserted inside those sources by
    §24, so this only has to agree about the spelling.

    The Windows installer is version-bearing, so it is recorded with
    `<v>` where the version goes — the recorded fact is the TEMPLATE, not a
    release. Matching on a literal version would fail on every bump, which is a
    check that is wrong about the wrong thing.
    """
    contract = read("client/src-tauri/src/chrysalis/contract.rs")
    current = dict(re.findall(r'b"([a-z_]+)" => Some\("(chrysalis-[^"]+)"\)', contract))
    if len(current) != 4:
        return None
    fetch_src = read("server/scripts/fetch-release.py")
    legacy_block = re.search(r"^LEGACY_PLATFORMS = \[(.*?)^\]", fetch_src, re.S | re.M)
    legacy = dict(re.findall(r'\("([a-z_]+)",\s*"([^"]+)"', legacy_block.group(1))) if legacy_block else {}
    # Windows is resolved through the template, not a literal, on both sides.
    tmpl = re.search(r'^INSTALLER_NAME_TEMPLATE = "([^"]+)"', fetch_src, re.M)
    tmpl_legacy = re.search(r'^LEGACY_INSTALLER_NAME_TEMPLATE = "([^"]+)"', fetch_src, re.M)
    if not tmpl or not tmpl_legacy:
        return None
    # The WINDOWS UPDATE PAYLOAD IS THE INSTALLER, not the bare binary: that is the
    # distinction the whole §1a block exists for. So the recorded `windows` fact is
    # the installer template on both sides, and it is NOT read from `artifact_for`
    # — `artifact_for` returns the raw `chrysalis-windows-amd64.exe`, which is
    # correct for the *artifact mapping* and wrong for the *update slot*. Recording
    # the raw name here would make this fact agree with the client constant and
    # disagree with what the hub serves.
    current["windows"] = tmpl.group(1).replace("%s", "<v>")
    legacy["windows"] = tmpl_legacy.group(1).replace("%s", "<v>")
    return {
        "current": {
            "windows": current["windows"],
            "linux": current["linux"],
            "macos_intel": current["macos_intel"],
            "macos_arm": current["macos_arm"],
        },
        "legacy": {
            "windows": legacy.get("windows", ""),
            "linux": legacy.get("linux", ""),
            "macos_intel": legacy.get("macos_intel", ""),
            "macos_arm": legacy.get("macos_arm", ""),
        },
    }

def contract_matches_hub():
    """`artifact_for` in the client vs the names the hub actually serves.

    THIS PAIR SILENTLY DISAGREED UNTIL 2026-11-25. `contract.rs` returned the bare
    `chrysalis-darwin-*` Mach-O for the macOS slots while `fetch-release.py`'s
    `NEW_PLATFORMS` served the `.app.tar.gz` — and a bare Mach-O is exactly what
    `install.rs` refuses, because the plugin extracts a `.app` from a gzip
    container. Neither side was "wrong" in isolation, which is the same shape as
    the `uot_port` / `download_<platform>` bugs this script exists for.

    Windows is the ONE intentional difference and it is asserted as such:
    `artifact_for` names the raw PE artifact (what the `windows-*` build produces),
    while the update SLOT carries the NSIS installer. Comparing them would be
    comparing an artifact to an update payload; the distinction is the whole of
    §1a.

    Returns True when every platform agrees, or a list of the disagreements.
    """
    contract = read("client/src-tauri/src/chrysalis/contract.rs")
    rust = dict(re.findall(r'b"([a-z_]+)" => Some\("([^"]+)"\)', contract))
    if len(rust) != 4:
        return None
    fetch_src = read("server/scripts/fetch-release.py")
    block = re.search(r"^NEW_PLATFORMS = \[(.*?)^\]", fetch_src, re.S | re.M)
    if not block:
        return None
    hub = dict(re.findall(r'\("([a-z_]+)",\s*"([^"]+)"', block.group(1)))
    if not hub:
        return None
    disagree = [k for k, v in hub.items() if rust.get(k) != v]
    return True if not disagree else disagree

def client_identity():
    """The app identity and its adoption candidates, from `dirs.rs`.

    Both halves matter and for different reasons: `value` is where new installs
    write, and `legacy` is the ONLY thing that lets current installs find their
    activation code. A recomputer that returned only the current value would
    report OK on a tree whose migration list had been emptied.
    """
    src = read("client/src-tauri/src/utils/dirs.rs")
    # The release arm only: the dev arm is a `cfg(feature = "verge-dev")` sibling
    # and taking the first match would silently be either one.
    m = re.search(r'#\[cfg\(not\(feature = "verge-dev"\)\)\]\s*pub static APP_ID: &str = "([^"]+)"', src)
    if not m:
        return None
    value = m.group(1)
    # `(?:pub\(crate\)\s+)?` because the constant is crate-visible: the
    # updater's predecessor detection reads it (a test asserts every adoptable
    # data root has an application row). The guard must match the code's actual
    # spelling, not force the code back to a narrower one to satisfy a regex.
    block = re.search(
        r'#\[cfg\(not\(feature = "verge-dev"\)\)\]\s*(?:pub\(crate\)\s+)?const LEGACY_APP_IDS: &\[&str\] = &\[(.*?)\];',
        src,
        re.S,
    )
    if not block:
        return None
    legacy = re.findall(r'"([^"]+)"', block.group(1))
    return {"current": value, "legacy": legacy}

def client_product_name():
    """`productName` and the pre-move one, from the two config files.

    The legacy value is not a constant anywhere — it is the historical spelling,
    recorded here so the pair is visible next to the current one. If the current
    name ever equals it again, this returns None rather than a pass.
    """
    try:
        with open("client/src-tauri/tauri.conf.json", encoding="utf-8") as fh:
            current = json.load(fh).get("productName")
    except Exception:
        return None
    if current == "Locus":
        # The move has been undone, or never happened: there is no legacy pair to
        # record, so this is not recomputable rather than "agrees".
        return None
    return {"current": current, "legacy": "Locus"}

def landing_docroot():
    """The landing page document root, from deploy_site()'s own target.

    Scoped to the deploy_site() function deliberately. A bare search for
    `local target="/var/www/..."` matches deploy_console()'s `/var/www/admin`
    FIRST (it appears earlier in the file), so the recorded root would silently
    have been the console's — a checker that returns a plausible wrong answer is
    worse than one that returns None, because it reports OK on a wrong fact.
    """
    src = read("server/modules/05-caddy.sh")
    i = src.find("deploy_site() {")
    if i < 0:
        return None
    end = src.find("\n# ", i)          # the next section comment ends the function
    body = src[i:end if end > 0 else len(src)]
    m = re.search(r'^\s*local target="(/var/www/[a-z]+)"', body, re.M)
    return m.group(1) + "/" if m else None

def landing_download_path():
    """The shape of a download URL, from the renderer's own format string.

    Reconstructed rather than stored, so the recorded fact cannot survive a
    change to how deploy-site.sh builds the URL — which is the only thing that
    can make the documented shape wrong.
    """
    src = read("server/scripts/deploy-site.sh")
    if '"%s/updates/%s/%s"' not in src:
        return None
    m = re.search(r'HUB_BASE:-(\S+?)\}', src)
    if not m:
        return None
    return m.group(1) + "/updates/<version>/<file>"
def free_tier():
    """The free tier's port, cap and tier-row names, from the tree.

    Recomputed from the two files that DEFINE it rather than from either alone:
    the cap and port come from `04-tc.sh`'s applied class, and the tier-row names
    from `seed-pb.py`'s seed loop. §23 asserts the same agreement from the other
    direction (and adds the reboot unit and the heartbeat key); this is what
    `state.toml` is checked against, so the data file cannot go stale about a
    value the docs point at.

    Returns a dict matching the TOML shape, or None if either file has moved.
    """
    tc = read("server/modules/04-tc.sh")
    m = re.search(r'apply_tc_now\s+(\d+)\s+"[0-9:]+"\s+"([0-9a-z]+)"', tc)
    if not m:
        return None
    port, cap = m.group(1), m.group(2)

    seed = read("server/scripts/seed-pb.py")
    rows = re.findall(r'\("([a-z_]+)",\s*"ECO_PASS",\s*' + re.escape(port) + r'\)', seed)
    if not rows:
        return None
    # Sorted: the two rows describe one endpoint and their order in the seed loop
    # carries no meaning, so comparing by position would fail on a harmless
    # reorder. Compare as a set.
    return {"port": int(port), "cap": cap, "tier_rows": sorted(rows)}
def budget_tier():
    """Budget's port, cap, UoT port and row names, from the tree.

    Same shape and same reasoning as `free_tier()`/`paid_tier()`, with two
    differences that are deliberate:

    * The CAP and the TCP PORT come from the applied class in `04-tc.sh`; the
      UoT port comes from the `write_uot_config` call in `02-shadowsocks.sh`.
      Both files, because a tier is two listeners and each has its own file that
      creates it.
    * The row name is read from `seed-pb.py`'s loop, where the row, its password
      and its port appear together on one line — so the three cannot be checked
      independently and quietly disagree.

    8444 (the retired stealth slot) is asserted NOWHERE here on purpose; the
    fact that Budget is not on it is carried by §27's negative, not by this
    recomputer.

    Returns a dict matching the TOML shape, or None if either file has moved.
    """
    tc = read("server/modules/04-tc.sh")
    m = re.search(r'apply_tc_now\s+8448\s+"[0-9:]+"\s+"([0-9a-z]+)"', tc)
    if not m:
        return None
    cap = m.group(1)

    ss = read("server/modules/02-shadowsocks.sh")
    u = re.search(r'write_uot_config\s+"budget"\s+"\$UOT_PORT_BUDGET"', ss)
    # The default is the number that matters; resolve it from the assignment so
    # this cannot agree with a stale literal in `state.toml` by coincidence.
    d = re.search(r'UOT_PORT_BUDGET="\$\{UOT_PORT_BUDGET:-(\d+)\}"', ss)
    if not u or not d:
        return None
    uot_port = int(d.group(1))

    seed = read("server/scripts/seed-pb.py")
    rows = re.findall(r'\("(budget)",\s*"BUDGET_PASS",\s*8448\)', seed)
    if not rows:
        return None
    return {"port": 8448, "uot_port": uot_port, "cap": cap, "tier_rows": sorted(rows)}
def paid_tier():
    """The paid tier's port, cap and row names, from the tree.

    Same shape and same reasoning as `free_tier()`: the cap and port come from
    `04-tc.sh`'s applied class, the seeded row name from `seed-pb.py`. §27
    asserts the same agreement from the other direction (reboot unit, udp_relay
    gating, the console placeholder, and the negative that the retired tier is
    not seeded).

    NOTE the cap regex differs from `free_tier`'s: `free_tier` takes the FIRST
    `apply_tc_now` (8443) and this takes the one for the paid port. Both are
    anchored on the apply line so a comment cannot satisfy them.

    Returns a dict matching the TOML shape, or None if either file has moved.
    """
    tc = read("server/modules/04-tc.sh")
    m = re.search(r'apply_tc_now\s+8445\s+"[0-9:]+"\s+"([0-9a-z]+)"', tc)
    if not m:
        return None
    cap = m.group(1)

    seed = read("server/scripts/seed-pb.py")
    rows = re.findall(r'\("(strike)",\s*"STRIKE_PASS",\s*8445\)', seed)
    if not rows:
        return None
    # The retirement is part of the fact: `state.toml` says Stealth is not seeded,
    # and §9 must be able to FAIL when it is seeded again.
    retired = [t for t in ("stealth",) if not re.search(r'\(' + t + r'",\s*"STEALTH_PASS"', seed)]
    return {"port": 8445, "cap": cap, "tier_rows": sorted(rows), "retired_rows": sorted(retired)}

def licence_client():
    """The client/ licence, as asserted by the root LICENSE scope table.
    Derived from the LICENSE table rather than from either manifest, because the
    manifests are what drift. §11 then checks the manifests against the same row,
    so all three are bound to one authority.
    """
    m = re.search(r'^\|\s*`client/`\s*\|\s*\**`?([A-Za-z0-9.\-]+)`?\**\s*\|',
                  read("LICENSE"), re.MULTILINE)
    return m.group(1) if m else None

def version_sites():
    """The files `pnpm release-version` rewrites, from the bump script itself.

    Derived by scanning `release-version.mjs` for the path each `update*Version()`
    call targets, so the list cannot be restated into state.toml and drift. §17
    asserts the same set from the other direction (that every site has a live
    call); this is what the data file is checked against, so prose that links
    here ("ALL FIVE version sites") can never disagree with the code.

    Sorted, because the order the script happens to call the updaters in carries
    no meaning and comparing by position would fail on a harmless reorder.
    """
    src = read("client/scripts/release-version.mjs")
    # Each updater call names its target as a repo-relative path literal. Match
    # the call sites, not the definitions, so a renamed-but-uncalled helper is
    # not counted (the same distinction §17 makes and documents).
    found = set()
    for fn, rel in (
        ("updatePackageVersion",      "client/package.json"),
        ("updateCargoVersion",        "client/src-tauri/Cargo.toml"),
        ("updateTauriConfigVersion",  "client/src-tauri/tauri.conf.json"),
        ("updateCargoLockVersion",    "client/Cargo.lock"),
        ("updateStateTomlVersion",    "docs/state.toml"),
    ):
        if re.search(re.escape(fn) + r'\s*\(', src):
            found.add(rel)
    return sorted(found) or None

def activation_tunnel_port(kind):
    """The bootstrap listener and path-filter ports, from BOTH ends.

    Two values that must agree across a language boundary: the server module sets
    `ACTIVATION_TUNNEL_PORT` / `ACTIVATION_FORWARD_PORT`, and the client names the
    same two as `TUNNEL_PORT` / `FILTER_PORT` (the client names the FILTER as its
    destination, because `ssserver` dials whatever the client asks for). This
    returns the pair only when both sides exist and agree, so a one-sided edit
    fails here rather than at a school.
    """
    srv = read("server/modules/09-activation-tunnel.sh")
    cli = read("client/src-tauri/src/chrysalis/activation_tunnel.rs")
    if kind == "bootstrap":
        m1 = re.search(r'TUNNEL_PORT="\$\{ACTIVATION_TUNNEL_PORT:-([0-9]+)\}?"', srv) or \
             re.search(r'ACTIVATION_TUNNEL_PORT="?([0-9]+)"?', srv)
        m2 = re.search(r'pub const TUNNEL_PORT: u16 = ([0-9]+);', cli)
    else:
        m1 = re.search(r'FORWARD_PORT="\$\{ACTIVATION_FORWARD_PORT:-([0-9]+)\}?"', srv) or \
             re.search(r'ACTIVATION_FORWARD_PORT="?([0-9]+)"?', srv)
        m2 = re.search(r'pub const FILTER_PORT: u16 = ([0-9]+);', cli)
    if not m1 or not m2:
        return None
    server_side, client_side = int(m1.group(1)), int(m2.group(1))
    return server_side if server_side == client_side else None

# Map fact -> callable returning the recomputed value, or None if not checkable.
CHECKERS = {
    "client.version":       manifest_version,
    "client.version_sites": version_sites,
    "hub.domain":           setup_domain,
    "hub.domain_legacy":    legacy_hub_domain,
    "hub.api_base":         publish_api_base,
    "hub.console_path":     lambda: setup_log_path("Admin console"),
    "hub.pocketbase_ui":    lambda: setup_log_path("PocketBase UI"),
    "platforms.keys":       platform_keys,
    "licence.client":       licence_client,
    "tiers.free":           free_tier,
    "tiers.budget":         budget_tier,
    "tiers.paid":           paid_tier,
    "site.domain":           landing_domain,
    "site.domain_legacy":    landing_domain_legacy,
    "site.document_root":    landing_docroot,
    "site.download_path":    landing_download_path,
    "activation_tunnel.bootstrap_port": lambda: activation_tunnel_port("bootstrap"),
    "activation_tunnel.filter_port":    lambda: activation_tunnel_port("filter"),
    # The identity move: BOTH the current value and the adoption list. A checker
    # that returned only the value would pass on a tree whose migration had been
    # emptied, which is the failure that loses every install's activation code.
    "client.identity":      client_identity,
    "client.product_name":  client_product_name,
    # `artifacts.updater` is one entry with two halves (current + legacy), so it is
    # compared as a pair rather than as a flat value.
    "artifacts.updater":    updater_artifacts,
}

for path, entry in sorted(facts.items()):
    prov = entry.get("provenance", "")
    value = entry.get("value")
    source = entry.get("source", "")

    if not source:
        print(f"BAD\t{path}\tdeclares no `source`, so it cannot be checked")
        continue

    if prov == "derived":
        checker = CHECKERS.get(path)
        if checker is None:
            # A derived fact with no checker is worse than an unverified one: it
            # looks verified and is not. This is how `STATE.md` felt to a reader.
            print(f"BAD\t{path}\tmarked `derived` with no checker in this script — "
                  f"add one, or mark it `asserted`/`unverified`")
            continue
        actual = checker()
        if actual is None:
            print(f"BAD\t{path}\tcould not recompute (source: {source})")
        elif actual != value:
            # Structural compare, not `str() == str()`: a nested table (like
            # `tiers.free`) has no guaranteed key order on either side, so a
            # string comparison would fail on a harmless reordering — a check
            # that is wrong about the wrong thing is one people learn to ignore.
            print(f"BAD\t{path}\tstate says {value!r}, tree says {actual!r} (source: {source})")
        else:
            print(f"OK\t{path}\t{value!r} agrees")
    elif prov == "asserted":
        print(f"NOTE\t{path}\tasserted by decision: {value!r} ({source})")
    elif prov == "unverified":
        print(f"WARN\t{path}\tUNVERIFIED world claim: {value!r} — see docs/operate/CLAIMS.md")
    else:
        print(f"BAD\t{path}\thas no valid `provenance` (got {prov!r})")
PY
    )

        if [ -z "$state_report" ]; then
            bad "could not read docs/state.toml"
        else
            printf '%s\n' "$state_report" | while IFS="$(printf '\t')" read -r level detail rest; do
                case "$level" in
                    OK)   ok   "$detail — $rest" ;;
                    NOTE) warn "  $detail — $rest" ;;
                    WARN) warn "$detail" ;;
                    BAD)  bad  "$detail" ;;
                esac
            done
            # The while-above runs in a subshell, so FAIL set inside it would be lost.
            # Re-derive the failure condition here rather than rely on it.
            if printf '%s\n' "$state_report" | grep -q '^BAD'; then
                FAIL=1
            fi
            derived=$(printf '%s\n' "$state_report" | grep -c '^OK' || true)
            unver=$(printf '%s\n' "$state_report" | grep -c '^WARN' || true)
            ok "state.toml: ${derived} derived fact(s) agree with the tree${unver:+, ${unver} unverified world claim(s) reported}"
        fi
    fi
}
