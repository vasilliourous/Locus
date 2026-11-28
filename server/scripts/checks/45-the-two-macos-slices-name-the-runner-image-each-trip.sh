#!/usr/bin/env bash
# Section 45 -- extracted from check-consistency.sh by
# server/scripts/lib/extract-sections.py. Do not add a shebang-only
# preamble: this file is SOURCED into the orchestrator's shell and
# shares its globals (`FAIL`, `ok`, `bad`, `REPO`, ...).
# A syntax error here does NOT stop the run -- see the orchestrator's
# syntax pre-check, and the note at the top of check-consistency.sh.

section_45() {
    echo "45. The two macOS slices name the runner image each triple needs"
    # ─────────────────────────────────────────────────────────────
    # WHY THIS EXISTS
    #
    # GitHub hands out TWO macOS runner labels per OS version and they are not
    # interchangeable: the bare label (`macos-15`, `macos-26`) is the **arm64**
    # machine, and the x64 one carries an `-intel` suffix (`macos-15-intel`). Ask
    # for the wrong one and you do not get a warning — you get a job cancelled with
    # zero steps, or one queued forever, and the two are documented as different
    # failures in `client.yml`'s matrix comment because they cost different amounts.
    #
    # That is not hypothetical here. The Intel slice spent months on `macos-13`
    # ("Waiting for a runner to pick up this job") and then both slices moved to a
    # bare `macos-14`, which stopped existing on 2026-11-02 when GitHub retired the
    # macOS 14 images. Both were silent: the run looked green on the other platforms
    # and no release could be published.
    #
    # WHAT IT ASSERTS, AND WHY EACH ONE CAN ACTUALLY FAIL
    #
    #   (a) Both entries are macOS labels at all. Fails if one slice is deleted,
    #       pointed at ubuntu, or the block is rewritten.
    #   (b) Both name the SAME OS family. Fails the realistic mistake — bumping one
    #       label of the pair and leaving the other on the retired image.
    #   (c) The arm slice is NOT `-intel`, and the Intel slice IS. This is the
    #       arch/image swap, which no other check can see: the triples still look
    #       right and the build fails 20 minutes in (or links the wrong slice).
    #   (d) The arm slice is not a `-latest`/`-large`/`-xlarge` alias. The file's own
    #       rule is that labels are pinned, so an alias here is a second moving tag
    #       next to the ones this project deliberately removed.
    #
    # WHAT IT CANNOT DO: it does not know which images GitHub still hosts. A retired
    # label passes all four. The retirement itself is caught by the tag pre-flight in
    # `client.yml` ("Probe the matrix runners against recent history"), which reads
    # real job history — that is the only evidence available, since GitHub exposes no
    # "is this label alive?" endpoint.
    #
    # --self-test runs the SAME predicate against a fixture that must fail, so the
    # guard can be shown firing rather than assumed to work.
    # ─────────────────────────────────────────────────────────────
    macos_matrix_check() {
        # $1 = path to the workflow, $2 = "real" | "self-test"
        python3 - "$1" "$2" <<'PY'
import re, sys, os, tempfile, textwrap

path, mode = sys.argv[1], sys.argv[2]
fail = []

def parse(text):
    """The darwin matrix entries, read the way the job reads them."""
    block = text.split("      matrix:", 1)[-1]
    entries, cur, seen_steps = [], None, False
    for line in block.splitlines():
        if re.match(r"^    steps:", line):
            seen_steps = True
            break
        m = re.match(r"^\s*-\s*os:\s*(\S+)\s*$", line)
        if m:
            cur = {"os": m.group(1)}
            entries.append(cur)
            continue
        m = re.match(r"^\s*label:\s*(\S+)\s*$", line)
        if m and cur is not None:
            cur["label"] = m.group(1)
    if not seen_steps:
        return None
    return entries

def check(entries):
    out = []
    mac = [e for e in entries if "macos" in e.get("os", "") or "darwin" in e.get("label", "")]
    if len(mac) != 2:
        return [f"expected exactly 2 macOS matrix entries, found {len(mac)}"]
    intel = [e for e in mac if "intel" in e.get("label", "")]
    arm = [e for e in mac if e.get("label", "").endswith("-arm") or e.get("label", "").endswith("arm")]
    if len(intel) != 1 or len(arm) != 1:
        return [f"the macOS entries are not one Intel + one arm slice: {[(e.get('label'), e.get('os')) for e in mac]}"]

    i, a = intel[0], arm[0]

    # (b) same OS family
    fam = lambda osname: re.match(r"macos-(\d+)", osname)
    fi, fa = fam(i["os"]), fam(a["os"])
    if not fi or not fa:
        out.append(f"a macOS label does not name an OS version: {i['os']!r} / {a['os']!r}")
    elif fi.group(1) != fa.group(1):
        out.append(f"the two slices are on DIFFERENT macOS images: intel={i['os']} arm={a['os']}")

    # (c) the arch/image swap
    if not i["os"].endswith("-intel"):
        out.append(f"the {i['label']} slice asks for {i['os']!r}, which is not an -intel image"
                   f" (a bare macOS label is arm64)")
    if a["os"].endswith("-intel"):
        out.append(f"the {a['label']} slice asks for {a['os']!r}, which is the x64 image")

    # (d) aliases
    if re.search(r"-(latest|large|xlarge)", a["os"]) or re.search(r"-(latest|large|xlarge)", i["os"]):
        out.append(f"a macOS runner label is a moving alias: {i['os']!r} / {a['os']!r}")
    return out

if mode == "self-test":
    # A fixture that MUST fail: the shape this guard was written for -- one slice
    # bumped, the other left on the retired image, both naming the bare (arm64)
    # label because the bump was done by search-and-replace.
    bad = textwrap.dedent("""\
          matrix:
            include:
              - os: ubuntu-24.04
                label: linux
                triple: x86_64-unknown-linux-gnu
              - os: macos-15
                label: macos-intel
                triple: x86_64-apple-darwin
              - os: macos-14
                label: macos-arm
                triple: aarch64-apple-darwin
        steps:
    """)
    entries = parse(bad)
    problems = check(entries) if entries else ["fixture did not parse"]
    if not problems:
        print("SELF-TEST FAILED: the fixture that must be rejected was accepted")
        sys.exit(1)
    for p in problems:
        print(f"  fixture rejected: {p}")
    print("  self-test OK: a drifted matrix is rejected")
    sys.exit(0)

if not os.path.exists(path):
    print(f"workflow not found at {path}")
    sys.exit(2)
entries = parse(open(path, encoding="utf-8", errors="replace").read())
if entries is None:
    print("could not find the `steps:` that ends the build matrix")
    sys.exit(1)
for p in check(entries):
    print(p)
    fail.append(p)
sys.exit(1 if fail else 0)
PY
    }

    if [ ! -f "$WORKFLOW" ]; then
        bad "workflow not found at ${WORKFLOW}; cannot check the macOS runner labels"
    elif ! command -v python3 >/dev/null 2>&1; then
        warn "python3 unavailable — cannot read the macOS matrix entries"
    elif ! macos_matrix_check "$WORKFLOW" self-test >/dev/null 2>&1; then
        bad "the macOS-image guard does not reject a drifted matrix — it cannot fail, so it proves nothing"
    else
        ok "the macOS-image guard rejects a drifted matrix (self-test)"
        if macos_problems="$(macos_matrix_check "$WORKFLOW" real)" && [ -z "$macos_problems" ]; then
            ok "both macOS slices name the runner image their triple needs"
        else
            bad "the macOS build matrix names an image that does not match its triple:"
            printf '%s\n' "$macos_problems" | while IFS= read -r line; do
                [ -n "$line" ] && bad "  $line"
            done
            bad "  The bare macOS label is the ARM machine; the x64 image carries -intel."
            bad "  A wrong one is rejected in ~2s with zero steps (or queued forever),"
            bad "  and the release cannot publish without both darwin artifacts."
        fi
    fi

    echo
    echo "════════════════════════════════════════════════════════════"
}
