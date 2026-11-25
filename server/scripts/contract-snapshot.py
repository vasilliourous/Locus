#!/usr/bin/env python3
"""Chrysalis wire contract — extract, verify, or refresh the frozen snapshot.

WHY THIS EXISTS

This project's worst bugs have all had one shape: a name that must agree
across a boundary with no mechanism enforcing agreement. `uot_port` vs
`server_port_uot`, `download_*` vs `update_*`, `bound_this_device` vs
`already_used`. Each was found in production, each was documented in
`docs/reference/FIXES.md`, and each was fixed by adding another greppy
assertion to `check-consistency.sh`.

Those assertions work while someone writes them. They do not survive a
rewrite, because the thing they protect — the set of names deployed clients
depend on — was never written down as data. It lived in prose, in a 4,600-line
shell script, and in the heads of whoever shipped 3.3.x.

This script is that data's generator and its enforcement point.

MODES

  --write    Regenerate docs/reference/contract.lock.json from the tree.
             Run this ONLY when you intend to change the contract. The diff is
             the review artifact: every added, removed or re-spelled name in it
             is a decision about installed clients, not a side effect.

  --check    (default) Compare the tree against the lock. Exit 1 on any
             difference that is not a documented, additive change.

  --explain  Print what the lock holds, in readable form.

WHAT IS "FROZEN" AND WHAT IS "ADDITIVE"

A name in the lock that disappears from the tree is a BREAK: a deployed
client, or a deployed hub, still sends or reads it. That fails.

A name that appears in the tree and not in the lock is ADDITIVE, and additive
is allowed without touching the lock — but it is reported, so nobody adds a
wire field to one side and forgets the other. Additive changes still have to
pass §33 (Rust struct vs TS interface) and §34 (IPC command defined /
registered / called); this script does not replace those, it catches the case
they were never scoped to cover: fields that cross the *hub* boundary.

Run `--write` after an intentional additive change so the next person reviews
a diff rather than a silence.
"""

import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
LOCK = REPO / "docs" / "reference" / "contract.lock.json"

CONTRACT_RS = REPO / "client" / "src-tauri" / "src" / "chrysalis" / "contract.rs"
TIER_RS = REPO / "client" / "src-tauri" / "src" / "chrysalis" / "tier.rs"
CMD_RS = REPO / "client" / "src-tauri" / "src" / "cmd" / "chrysalis.rs"
CHRYSALIS_RS = REPO / "client" / "src-tauri" / "src" / "chrysalis"
LIB_RS = REPO / "client" / "src-tauri" / "src" / "lib.rs"
# The hub's own artefact resolver. Its `LEGACY_PLATFORMS` is the authority on
# which pre-move names the hub still serves — see the legacy_artifacts block.
FETCH_RELEASE_PY = REPO / "server" / "scripts" / "fetch-release.py"
TS_SRC = REPO / "client" / "src"
HOOKS = REPO / "server" / "pb_hooks"


def read(path):
    # A missing file is reported by the caller with a specific message; an
    # exception here would surface as a traceback, which reads like a crash in
    # the checker rather than a defect in the tree.
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return None


def camel(name):
    parts = name.split("_")
    return parts[0] + "".join(p.capitalize() for p in parts[1:])


def rust_structs(src):
    """Every `pub struct NAME { ... }` in a Rust source, with its fields.

    The zero-width lookahead on the field pattern (`(?:\\n    pub )`) is
    deliberate: a naive `pub (\\w+):` also matches inside `impl` blocks and
    doc comments. Fields are indented exactly four spaces in this tree; the
    generator fails loudly rather than silently returning nothing if a struct
    is found with zero fields, because an empty result here would make the
    guard pass by accident.
    """
    out = {}
    for name, body in re.findall(r"pub struct (\w+)(?:<'a>)? \{(.*?)\n\}", src, re.S):
        fields = re.findall(r"\n {4}pub (\w+):", body)
        attrs = re.findall(r'\n {4}#\[serde\(([^)]*)\)\]\s*\n {4}pub (\w+):', body)
        by_field = {}
        for attr, field in attrs:
            entry = {}
            m = re.search(r'rename = "(\w+)"', attr)
            if m:
                entry["wire_name"] = m.group(1)
            m = re.search(r'alias = "(\w+)"', attr)
            if m:
                entry["alias"] = m.group(1)
            if "skip_serializing_if" in attr:
                entry["optional"] = True
            if re.search(r"\bdefault\b", attr):
                entry["default"] = True
            if entry:
                by_field[field] = entry
        out[name] = {"fields": fields, "attrs": by_field}
    return out


def rust_enum_variants(src, enum_name):
    m = re.search(r"pub enum %s \{(.*?)\n\}" % enum_name, src, re.S)
    if not m:
        return None
    body = m.group(1)
    body = re.sub(r"^\s*///.*$", "", body, flags=re.M)
    variants = re.findall(r"\n {4}([A-Z]\w*),", body)
    renames = dict(re.findall(r'#\[serde\(rename = "(\w+)"\)\]\s*\n {4}(\w+),', body))
    out = {}
    for v in variants:
        wire = renames.get(v)
        if wire is None:
            # `rename_all = "snake_case"` is applied by the caller.
            wire = None
        out[v] = wire
    return out


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def collect():
    contract = read(CONTRACT_RS)
    if contract is None:
        raise SystemExit("contract-snapshot: missing %s" % CONTRACT_RS)

    structs = rust_structs(contract)
    for required in (
        "CodeRequest",
        "HeartbeatRequest",
        "TierConfig",
        "ActivateResponse",
        "LookupResponse",
        "ReleaseManifest",
        "ReleaseAsset",
    ):
        if required not in structs:
            raise SystemExit(
                "contract-snapshot: struct %s not found in contract.rs — the "
                "extractor and the tree have diverged; fix the extractor" % required
            )
        if not structs[required]["fields"]:
            raise SystemExit(
                "contract-snapshot: struct %s extracted with zero fields — a guard "
                "over an empty set passes vacuously" % required
            )

    # ── struct field names, as they appear on the wire ──
    # contract.rs uses serde's DEFAULT (Rust identity) naming: no
    # `rename_all`, which is a load-bearing fact — `device_fingerprint` and
    # `server_config` go over the wire in snake_case and the hub reads them
    # that way. A `rename_all = "camelCase"` added here would break every
    # deployed client and this snapshot is what makes that visible.
    wire_structs = {}
    for name, data in structs.items():
        fields = {}
        for field in data["fields"]:
            entry = dict(data["attrs"].get(field, {}))
            entry["wire_name"] = entry.get("wire_name", field)
            fields[field] = entry
        wire_structs[name] = fields

    # ── enums ──
    lookup_status = rust_enum_variants(contract, "LookupStatus") or {}
    lookup_wire = sorted(
        snake(v) for v in lookup_status.keys() if v != "Unknown"
    )

    # ── endpoints ──
    # Collected from BOTH sides. The hub is the authority on what it serves;
    # the client is the authority on what it calls. An endpoint the client
    # calls and the hub no longer serves is the defect this catches, so they
    # are recorded separately and compared in --check, not merged here.
    hub_routes = set()
    hub_src = ""
    if HOOKS.is_dir():
        for path in sorted(HOOKS.rglob("*.js")):
            text = read(path) or ""
            hub_src += text
            for method, route in re.findall(
                r'routerAdd\(\s*"(\w+)",\s*"(/[^"]+)"', text
            ):
                hub_routes.add("%s %s" % (method, route))

    client_calls = set()
    chrysalis_rs_src = ""
    if CHRYSALIS_RS.is_dir():
        for path in sorted(CHRYSALIS_RS.rglob("*.rs")):
            text = read(path) or ""
            chrysalis_rs_src += text
            for route in re.findall(r'"(/api/[a-z0-9/_-]+)"', text):
                client_calls.add(route)
            # A call site built with format! carries the verb in the method
            # chain, not in the string; the verb list below is what makes a
            # format!-built call visible at all.
            for verb, route in re.findall(
                r'\.(post|get|put|delete)\(format!\("\{[a-z_]+\}(/api/[a-z0-9/_-]+)"',
                text,
            ):
                client_calls.add("%s %s" % (verb.upper(), route))

    # ── platforms and their artifact filenames ──
    platforms = {}
    for key, artifact in re.findall(
        r'pub const PLATFORM_(\w+): &str = "([^"]+)"', contract
    ):
        platforms[key.lower()] = {"key": artifact}
    for key, artifact in re.findall(
        r'b"([a-z_]+)" => Some\("([^"]+)"\)', contract
    ):
        if key in platforms:
            platforms[key]["artifact"] = artifact

    # ── the DEPRECATED ALIAS artefact names, from the hub's own resolver ──
    #
    # `platforms[*]["artifact"]` holds ONE name per platform, which cannot express
    # "two names are live at once" — the state a rename is in for as long as a
    # parked fleet resolves the old spelling. Without this, the lock would say
    # `locus-*` is gone on the same day the hub started serving it, and the next
    # reader would take the lock's silence as permission to delete the alias.
    #
    # The source is `LEGACY_PLATFORMS` in fetch-release.py — the hub's resolver —
    # rather than the Rust, because it is the hub that must keep resolving both.
    # Recorded as data, so `--check` notices the alias disappearing.
    fetch_src = read(FETCH_RELEASE_PY) or ""
    _legacy_block = ""
    _m = re.search(r"^LEGACY_PLATFORMS = \[(.*?)^\]", fetch_src, re.S | re.M)
    if _m:
        _legacy_block = _m.group(1)
    for key, name in re.findall(r'\("([a-z_]+)",\s*"([^"]+)"', _legacy_block):
        if key in platforms:
            platforms[key].setdefault("legacy_artifacts", []).append(name)
    # The Windows entry in `LEGACY_PLATFORMS` is `None` (it resolves through the
    # installer template, not a literal), so the loop above cannot see it. Read the
    # legacy installer template directly, or the lock would record three of four
    # aliases and read as if Windows had no pre-move name to keep serving.
    _win = re.search(r'^LEGACY_INSTALLER_NAME_TEMPLATE = "([^"]+)"', fetch_src, re.M)
    if _win and "windows" in platforms:
        platforms["windows"].setdefault("legacy_artifacts", []).append(
            _win.group(1).replace("%s", "<version>")
        )

    # ── proxy names and the loop trap ──
    tier = read(TIER_RS) or ""
    proxy_names = {}
    for const, value in re.findall(r'pub const (\w+_NAME): &str = "([^"]+)"', tier):
        proxy_names[const] = value

    # ── hub URLs ──
    hub_urls = dict(re.findall(r'pub const (\w*HUB_URL): &str = "([^"]+)"', contract))

    # ── IPC commands ──
    # The same three-way set §34 checks, recorded as data so a rename shows up
    # in a diff rather than only in a failing assertion.
    defined, registered, called = [], [], []
    lib = read(LIB_RS) or ""
    for path in sorted((REPO / "client" / "src-tauri" / "src").rglob("*.rs")):
        text = read(path) or ""
        for name in re.findall(
            r"#\[tauri::command\]\s*\npub (?:async )?fn (\w+)", text
        ):
            if name.startswith("chrysalis_"):
                defined.append(name)
        for name in re.findall(r'invoke\(\s*[\'"](chrysalis_\w+)[\'"]', text):
            called.append(name)
    for name in re.findall(r"\bcmd::(\w+)", lib):
        if name.startswith("chrysalis_"):
            registered.append(name)
    if TS_SRC.is_dir():
        for path in sorted(TS_SRC.rglob("*.ts*")):
            text = read(path) or ""
            for name in re.findall(
                r"invoke(?:<[^>]*>)?\(\s*['\"](chrysalis_\w+)['\"]", text
            ):
                called.append(name)

    return {
        "version": 1,
        "what": "The Chrysalis wire contract. See server/scripts/contract-snapshot.py.",
        "boundary": {
            "client_rust": "client/src-tauri/src/chrysalis/contract.rs",
            "hub_routes": "server/pb_hooks/*.js — routerAdd() calls",
            "ipc": "client/src-tauri/src/cmd/chrysalis.rs + lib.rs + client/src/**/*.ts*",
        },
        "structs": wire_structs,
        "enums": {"LookupStatus": {"wire_values": lookup_wire}},
        "endpoints": {
            "hub_serves": sorted(hub_routes),
            "client_calls": sorted(client_calls),
        },
        "platforms": platforms,
        "proxy_names": proxy_names,
        "hub_urls": hub_urls,
        "ipc_commands": {
            "defined": sorted(set(defined)),
            "registered": sorted(set(registered)),
            "called": sorted(set(called)),
        },
    }


def load_lock():
    try:
        return json.loads(LOCK.read_text(encoding="utf-8"))
    except OSError:
        return None


def diff(lock, current):
    """Return (breaks, additions) between a lock and a fresh extraction.

    A "break" is something the lock asserts that the tree no longer has.
    An "addition" is something the tree has that the lock does not assert.
    Both directions are reported; only breaks fail.
    """
    breaks, additions = [], []

    # The hub's resolver, read once: it is the authority on which deprecated
    # artefact aliases the hub still serves. Read here rather than passed in so
    # the alias-survival check below cannot be satisfied by stale `current` data
    # (the extraction reads the Rust, and the aliases live in the Python).
    fetch_tree_text = read(FETCH_RELEASE_PY) or ""

    # structs and their fields
    for sname, fields in lock.get("structs", {}).items():
        if sname not in current["structs"]:
            breaks.append("struct %s is gone" % sname)
            continue
        cur = current["structs"][sname]
        for fname, meta in fields.items():
            if fname not in cur:
                breaks.append(
                    "struct %s no longer has field %s (wire name %r)"
                    % (sname, fname, meta.get("wire_name", fname))
                )
                continue
            if cur[fname].get("wire_name") != meta.get("wire_name"):
                breaks.append(
                    "struct %s field %s changed wire name %r -> %r"
                    % (
                        sname,
                        fname,
                        meta.get("wire_name"),
                        cur[fname].get("wire_name"),
                    )
                )
        for fname in cur:
            if fname not in fields:
                additions.append(
                    "struct %s gained field %s (wire %r)"
                    % (sname, fname, cur[fname].get("wire_name"))
                )

    # enum wire values
    for ename, data in lock.get("enums", {}).items():
        cur = current["enums"].get(ename, {}).get("wire_values", [])
        for v in data.get("wire_values", []):
            if v not in cur:
                breaks.append("enum %s no longer accepts wire value %r" % (ename, v))
        for v in cur:
            if v not in data.get("wire_values", []):
                additions.append("enum %s gained wire value %r" % (ename, v))

    # endpoints — compared as two separate sets, which is the whole point:
    # a route added to the hub and not called is dead surface, and a route
    # called by a client that the hub dropped is a runtime 404.
    for key, label in (
        ("hub_serves", "the hub no longer serves"),
        ("client_calls", "the client no longer calls"),
    ):
        locked = lock.get("endpoints", {}).get(key, [])
        cur = current["endpoints"].get(key, [])
        for v in locked:
            if v not in cur:
                breaks.append("%s %r" % (label, v))
        for v in cur:
            if v not in locked:
                additions.append("new in %s: %r" % (key, v))

    # a client call with no matching hub route is a break in the other
    # direction — recorded even when the caller is new.
    hub = set(current["endpoints"].get("hub_serves", []))
    routes = {r.split(" ", 1)[1] for r in hub if " " in r}
    for call in current["endpoints"].get("client_calls", []):
        route = call.split(" ", 1)[1] if " " in call else call
        if route not in routes:
            breaks.append("the client calls %s but the hub defines no such route" % route)

    # platforms and artifacts
    for pkey, pdata in lock.get("platforms", {}).items():
        if pkey not in current["platforms"]:
            breaks.append("platform %r is gone" % pkey)
            continue
        curp = current["platforms"][pkey]
        if curp.get("key") != pdata.get("key"):
            breaks.append(
                "platform %r key changed %r -> %r"
                % (pkey, pdata.get("key"), curp.get("key"))
            )
        if pdata.get("artifact") and curp.get("artifact") != pdata.get("artifact"):
            breaks.append(
                "platform %r artifact changed %r -> %r"
                % (pkey, pdata.get("artifact"), curp.get("artifact"))
            )
        # THE DEPRECATED ALIASES MUST STILL RESOLVE.
        #
        # A rename is a two-sided state for as long as a parked fleet resolves the
        # old spelling: the hub serves both, CI publishes both, and the lock records
        # both. This asserts the recorded alias is still present in the tree, so
        # deleting `LEGACY_PLATFORMS` (or the legacy installer template) fails here
        # rather than silently making every pre-move release un-fetchable — the
        # exact silent-no-update failure the whole lock exists to prevent.
        #
        # Retiring an alias is a deliberate act: drop it from the lock AND from the
        # resolver in the same commit, which is what `--write` producing a diff is
        # for. §1e and §1c then refuse the half-removal.
        for alias in pdata.get("legacy_artifacts", []) or []:
            # A literal alias (`locus-linux-amd64`) is checked verbatim. A template
            # (`installer-Locus_<version>_x64-setup.exe`) is checked by its PRODUCT
            # TOKEN, `installer-Locus_`, because the version is filled at runtime
            # and never appears in the source. Matching the whole template string
            # would fail on every release, which is a guard that always fires and
            # therefore reports nothing.
            if "<version>" in alias:
                needle = alias.split("<version>")[0]
            else:
                needle = alias
            if needle not in fetch_tree_text:
                breaks.append(
                    "platform %r lost its deprecated alias %r — the hub no longer "
                    "resolves it, so a release built before the move cannot be "
                    "fetched (NAME-MIGRATION.md §3.1)"
                    % (pkey, alias)
                )
    for pkey in current["platforms"]:
        if pkey not in lock.get("platforms", {}):
            additions.append("platform %r is new" % pkey)

    # proxy names — the loop trap (group name must differ from proxy name)
    for cname, value in lock.get("proxy_names", {}).items():
        if current["proxy_names"].get(cname) != value:
            breaks.append(
                "proxy constant %s changed %r -> %r"
                % (cname, value, current["proxy_names"].get(cname))
            )
    cur_proxy = current["proxy_names"]
    if cur_proxy.get("PROXY_NAME") and cur_proxy.get("PROXY_NAME") == cur_proxy.get(
        "GROUP_NAME"
    ):
        breaks.append(
            "PROXY_NAME == GROUP_NAME (%r) — mihomo treats a proxy group named "
            "like its member as a reference loop and refuses the whole config, "
            "which is still valid YAML" % cur_proxy.get("PROXY_NAME")
        )

    # hub urls
    for cname, value in lock.get("hub_urls", {}).items():
        if current["hub_urls"].get(cname) != value:
            breaks.append(
                "hub constant %s changed %r -> %r"
                % (cname, value, current["hub_urls"].get(cname))
            )

    # IPC commands
    lock_ipc = lock.get("ipc_commands", {})
    cur_ipc = current["ipc_commands"]
    for key in ("defined", "registered", "called"):
        locked = set(lock_ipc.get(key, []))
        cur = set(cur_ipc.get(key, []))
        for v in sorted(locked - cur):
            breaks.append("IPC command %r is no longer %s" % (v, key))
        for v in sorted(cur - locked):
            additions.append("IPC command %r is newly %s" % (v, key))

    return breaks, additions


def main(argv):
    mode = "--check"
    for arg in argv[1:]:
        if arg in ("--write", "--check", "--explain"):
            mode = arg
        elif arg in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            print("contract-snapshot: unknown argument %r" % arg)
            return 2

    current = collect()

    if mode == "--explain":
        print(json.dumps(current, indent=2, sort_keys=True))
        return 0

    if mode == "--write":
        LOCK.parent.mkdir(parents=True, exist_ok=True)
        LOCK.write_text(
            json.dumps(current, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
        print("wrote %s" % LOCK.relative_to(REPO))
        return 0

    lock = load_lock()
    if lock is None:
        print("BAD\tthe contract lock is missing: %s" % LOCK.relative_to(REPO))
        print("BAD\t  run: python3 server/scripts/contract-snapshot.py --write")
        return 1

    breaks, additions = diff(lock, current)
    for b in breaks:
        print("BAD\t%s" % b)
    for a in additions:
        print("NEW\t%s" % a)
    return 1 if breaks else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
