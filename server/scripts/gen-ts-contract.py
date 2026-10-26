#!/usr/bin/env python3
"""Generate the TypeScript mirror of the Rust IPC types, and verify it.

WHY THIS EXISTS

`client/src/services/locus.ts` hand-writes a TypeScript interface for every Rust
type that crosses the Tauri IPC boundary. Nine of them today:

    LocusStatus  ValidateCodeResult  ActivationResult  ConnectionResult
    UpdateStatus  UpdateProgress  UdpStatus  AllowanceStatus  SubscriptionStatus

Nothing makes them agree. `check-consistency.sh` §33 asserts that for
`LocusStatus` alone — one type out of nine — and it does so by regex-matching
two files. The other eight have no check at all, so the drift there is silent by
construction: a field added to Rust and not to TypeScript serialises and is
ignored (a feature that does nothing); a field read by TypeScript that Rust
never sends renders as `undefined` (blank, or `NaN`).

This is the `uot_port` class one layer in, and it is the layer that changes most
often. `docs/reference/FIXES.md` records the identical failure for `udp` —
added to the Rust struct, forgotten in the interface — which is what §33 was
written for. Checking one type by hand does not scale to nine, and it does not
survive a tenth.

WHY GENERATION AND NOT ANOTHER CHECK

A check detects disagreement after both sides were written. Generation means
there is only one side to write: the TypeScript is derived from the Rust on
every build, so disagreement is not "caught", it is unrepresentable. §33 becomes
a check on the generator rather than a check on nine hand-maintained files.

MODES

  --write   Regenerate client/src/services/locus.generated.ts from the Rust.
  --check   (default) Regenerate into memory and fail if the committed file
            differs. This is what CI runs: a developer who edits the Rust types
            without running the generator fails the build with a diff.
  --print   Write to stdout, for inspection.

WHAT IS EMITTED, AND WHAT IS NOT

Emitted: the type declarations themselves — interfaces, field types, tagged
unions from the Rust enums, doc comments. That is the part that mirrors Rust
mechanically.

Not emitted: the `invoke()` call sites and their wrapper functions. Those carry
argument marshalling, error handling and cancellation behaviour that is a
developer's decision at each site, not a mechanical mirror of a type. §34 already
asserts every command is called.

The generated file must not be hand-edited. It is a build product that happens to
be committed, for the same reason `Cargo.lock` is: so a diff shows the wire shape
changing, reviewably.
"""

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SOURCE = REPO / "client" / "src-tauri" / "src" / "cmd" / "locus.rs"
OUTPUT = REPO / "client" / "src" / "services" / "locus.generated.ts"

HEADER = """\
// AUTO-GENERATED — do not edit by hand.
//
// Source: client/src-tauri/src/cmd/locus.rs
// Tool:   server/scripts/gen-ts-contract.py
//
// Every type in this file is a mechanical mirror of a Rust type that crosses the
// Tauri IPC boundary. There is exactly one side to write — the Rust — so the two
// cannot disagree. Editing this file directly is a bug: the change is erased by
// the next regeneration and fails `--check` in CI before it can ship.
//
// To change a wire shape, change the Rust type and run:
//   python3 server/scripts/gen-ts-contract.py --write
//
// Prose documentation for these types lives in services/locus.ts beside the
// functions that return them. This file carries the Rust doc comments as
// context, but the decision to document a field belongs with the hand-written
// file, which is not generated.

"""

# Rust primitive -> TypeScript. `u64`/`i64` become `number`: JSON numbers are
# IEEE-754 doubles, so a full-range u64 does not round-trip exactly. That is a
# real limitation, and the types that carry it (allowance byte counts) are
# bounded far below 2^53, so `number` is the honest type here. It is stated so
# nobody later assumes bigint semantics.
PRIMITIVES = {
    "bool": "boolean",
    "String": "string",
    "str": "string",
    "u8": "number",
    "u16": "number",
    "u32": "number",
    "u64": "number",
    "i8": "number",
    "i16": "number",
    "i32": "number",
    "i64": "number",
    "f32": "number",
    "f64": "number",
    "usize": "number",
    "isize": "number",
}


def camel(name):
    parts = name.split("_")
    return parts[0] + "".join(p.capitalize() for p in parts[1:])


def rust_doc(block):
    """Strip `///` doc comments from a Rust item body into plain lines.

    Rust docs use `[`Link`]` and backticks; both are valid inside a TS block
    comment, so they are passed through unchanged rather than "cleaned" —
    a mangled doc comment is worse than a raw one.
    """
    lines = []
    for raw in block.splitlines():
        s = raw.strip()
        if s.startswith("///"):
            lines.append(s[3:].strip() if s[3:].strip() else "")
    return lines


def emit_doc(lines, indent=""):
    if not lines:
        return []
    out = [indent + "/**"]
    for line in lines:
        out.append((indent + " * " + line).rstrip())
    out.append(indent + " */")
    return out


def ts_type(rust_type):
    t = rust_type.strip()
    t = t.replace("&'a str", "string").replace("&str", "string")

    # Option<T> -> T | null. `null`, not `undefined`: serde emits an absent or
    # null field, and the TypeScript callers already test against null.
    m = re.fullmatch(r"Option<(.+)>", t)
    if m:
        return ts_type(m.group(1)) + " | null"

    # Vec<T> / [T; N] -> T[]
    m = re.fullmatch(r"Vec<(.+)>", t)
    if m:
        inner = ts_type(m.group(1))
        return "(%s)[]" % inner if "|" in inner else inner + "[]"

    if t in PRIMITIVES:
        return PRIMITIVES[t]

    # A sibling type in this module, mirrored by name.
    return t


def scan_items(src):
    """Walk the source line by line, yielding each top-level item with the doc
    comments and attributes that immediately precede it.

    A regex over the whole file was tried first and abandoned: an alternation
    containing `^    [^\\n]*\\n` under `re.S` lets the prefix repeat across the
    entire 7,000-line file, and the matcher backtracks catastrophically instead
    of failing. A line scan has no such failure mode and states the structure
    plainly — doc comment, then attribute, then item, then indented body until
    the closing brace.
    """
    lines = src.splitlines()
    docs, attrs, i = [], [], 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        if stripped.startswith("///"):
            docs.append(stripped[3:].strip())
            i += 1
            continue
        if stripped.startswith("#["):
            attrs.append(stripped)
            i += 1
            continue
        if stripped.startswith("//"):
            # A plain comment ends a doc block — it is not documentation of the
            # item below it, and folding it in would misattribute it.
            docs = []
            i += 1
            continue
        if stripped == "":
            i += 1
            continue

        m = re.match(r"^pub (struct|enum) (\w+)(?:<'a>)? \{$", stripped)
        if not m:
            # A non-item statement. Reset: whatever followed a doc block that is
            # not an item did not consume it.
            docs, attrs = [], []
            i += 1
            continue

        kind, name = m.group(1), m.group(2)
        body = []
        i += 1
        while i < len(lines) and lines[i].rstrip() != "}":
            body.append(lines[i])
            i += 1
        i += 1  # consume the closing brace

        yield kind, name, "\n".join(body), docs, attrs
        docs, attrs = [], []


def serde_attrs(attrs):
    out = {}
    for a in attrs:
        m = re.search(r"#\[serde\((.*)\)\]$", a)
        if not m:
            continue
        for key, val in re.findall(r'(\w+)\s*=\s*"([\w]+)"', m.group(1)):
            out[key] = val
        for flag in re.findall(r"\b(tag|default|flatten)\b(?!\s*=)", m.group(1)):
            out.setdefault(flag, True)
    return out


def parse_structs(src):
    """Every `pub struct` whose fields are the IPC wire shape."""
    out = []
    for kind, name, body, docs, attrs in scan_items(src):
        if kind != "struct":
            continue
        serde = serde_attrs(attrs)
        fields = []
        fdocs = []
        for line in body.splitlines():
            s = line.strip()
            if s.startswith("///"):
                fdocs.append(s[3:].strip())
                continue
            if s.startswith("#["):
                continue
            fm = re.match(r"^pub (\w+): (.+),$", s)
            if fm:
                fields.append(
                    {"rust": fm.group(1), "type": fm.group(2).strip(), "docs": fdocs}
                )
                fdocs = []
        if fields:
            out.append(
                {
                    "kind": "struct",
                    "name": name,
                    "docs": docs,
                    "fields": fields,
                    "camel": serde.get("rename_all") == "camelCase",
                }
            )
    return out


def parse_enums(src):
    """Every serde-tagged `pub enum`, emitted as a discriminated union.

    Only TAGGED enums are emitted. An untagged enum has no stable wire shape to
    mirror — serde picks a representation — so guessing one here would produce a
    TypeScript union that is confidently wrong. It is skipped, and the count of
    skipped enums is reported by `--print --verbose` so a silently ignored type
    cannot hide.
    """
    out = []
    for kind, name, body, docs, attrs in scan_items(src):
        if kind != "enum":
            continue
        serde = serde_attrs(attrs)
        tag = serde.get("tag")
        if not tag:
            continue
        variants = []
        vdocs, in_payload, payload = [], False, []
        for line in body.splitlines():
            s = line.strip()
            if s.startswith("///"):
                vdocs.append(s[3:].strip())
                continue
            if s.startswith("#["):
                continue
            if s == "":
                continue
            if in_payload:
                if s.startswith("}"):
                    # End of a struct-variant payload; the variant is already
                    # recorded, so just close it.
                    in_payload = False
                else:
                    fm = re.match(r"^(\w+): (.+),$", s)
                    if fm:
                        payload.append(
                            {"rust": fm.group(1), "type": fm.group(2).strip(), "docs": []}
                        )
                continue
            vm = re.match(r"^(\w+),$", s)
            if vm:
                variants.append({"rust": vm.group(1), "docs": vdocs, "fields": []})
                vdocs = []
                continue
            # A struct variant on ONE line: `Configured { port: u16 },`
            vm = re.match(r"^(\w+) \{ (.+) \},$", s)
            if vm:
                f = []
                for part in vm.group(2).split(","):
                    pm = re.match(r"^\s*(\w+): (.+?)\s*$", part)
                    if pm:
                        f.append(
                            {"rust": pm.group(1), "type": pm.group(2).strip(), "docs": []}
                        )
                variants.append({"rust": vm.group(1), "docs": vdocs, "fields": f})
                vdocs = []
                continue

            # A struct variant opened at end of line: `Metered {`
            vm = re.match(r"^(\w+) \{$", s)
            if vm:
                variants.append({"rust": vm.group(1), "docs": vdocs, "fields": []})
                vdocs = []
                in_payload = True
                payload = variants[-1]["fields"]
                continue
        out.append(
            {
                "kind": "enum",
                "name": name,
                "docs": docs,
                "variants": variants,
                "tag": tag,
                "camel": serde.get("rename_all") == "camelCase",
            }
        )
    return out


def render(items):
    parts = [HEADER]

    for it in items:
        if it["kind"] == "struct":
            parts.extend(emit_doc(it["docs"]))
            parts.append("export interface %s {" % it["name"])
            for f in it["fields"]:
                parts.extend(emit_doc(f["docs"], indent="  "))
                wire = camel(f["rust"]) if it["camel"] else f["rust"]
                parts.append("  %s: %s" % (wire, ts_type(f["type"])))
            parts.append("}")
            parts.append("")
        else:
            parts.extend(emit_doc(it["docs"]))
            parts.append("export type %s =" % it["name"])
            for v in it["variants"]:
                tag_value = v["rust"].lower() if it["camel"] else v["rust"]
                if not v["fields"]:
                    head = "  | { %s: '%s' }" % (it["tag"], tag_value)
                    if v["docs"]:
                        parts.extend(emit_doc(v["docs"], indent="  "))
                    parts.append(head)
                else:
                    if v["docs"]:
                        parts.extend(emit_doc(v["docs"], indent="  "))
                    parts.append("  | {")
                    parts.append("      %s: '%s'" % (it["tag"], tag_value))
                    for f in v["fields"]:
                        wire = camel(f["rust"]) if it["camel"] else f["rust"]
                        parts.append("      %s: %s" % (wire, ts_type(f["type"])))
                    parts.append("    }")
            parts.append("")

    return "\n".join(parts).rstrip() + "\n"


def collect():
    if not SOURCE.is_file():
        raise SystemExit("gen-ts-contract: missing %s" % SOURCE)
    src = SOURCE.read_text(encoding="utf-8")
    items = parse_structs(src) + parse_enums(src)
    if not items:
        # An empty generation would make every downstream check pass vacuously,
        # which is the failure this whole file exists to avoid.
        raise SystemExit(
            "gen-ts-contract: extracted zero types from %s — the extractor and "
            "the tree have diverged" % SOURCE
        )
    return items


def main(argv):
    mode = "--check"
    for arg in argv[1:]:
        if arg in ("--write", "--check", "--print"):
            mode = arg
        elif arg in ("-h", "--help"):
            print(__doc__)
            return 0
        else:
            print("gen-ts-contract: unknown argument %r" % arg)
            return 2

    rendered = render(collect())

    if mode == "--print":
        sys.stdout.write(rendered)
        return 0

    if mode == "--write":
        OUTPUT.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT.write_text(rendered, encoding="utf-8")
        print("wrote %s" % OUTPUT.relative_to(REPO))
        return 0

    try:
        current = OUTPUT.read_text(encoding="utf-8")
    except OSError:
        print("BAD\tthe generated TS contract is missing: %s" % OUTPUT.relative_to(REPO))
        print("BAD\t  run: python3 server/scripts/gen-ts-contract.py --write")
        return 1

    if current == rendered:
        return 0

    import difflib

    diff = list(
        difflib.unified_diff(
            current.splitlines(keepends=True),
            rendered.splitlines(keepends=True),
            fromfile="committed (%s)" % OUTPUT.relative_to(REPO),
            tofile="regenerated from the Rust types",
        )
    )
    print("BAD\tthe TypeScript IPC types no longer match the Rust types:")
    for line in diff[:60]:
        print("BAD\t  " + line.rstrip())
    if len(diff) > 60:
        print("BAD\t  … %d more lines" % (len(diff) - 60))
    print("BAD\t  run: python3 server/scripts/gen-ts-contract.py --write")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
