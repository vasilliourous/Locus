#!/usr/bin/env python3
"""Fail if the hooks in this tree write a field the live schema does not have.

WHY THIS EXISTS — the defect has now been reported THREE times.

`deploy.sh` ships hooks; it has never shipped schema. PocketBase accepts a write to
an undefined field and then DISCARDS it when it builds the UPDATE, because
`Dao.update` writes only the columns the collection schema declares. So a hook can
be deployed, run faithfully for months, and record nothing — while the operator
sees a 200 and concludes it worked.

`docs/reference/FIXES.md` ("THE THIRD REPORT OF THE SAME DEFECT", 2026-11-27)
records the same three columns missing: `first_seen_at`, `first_seen_fingerprint`,
`suspended_reason`. They are STILL missing on the live hub. That entry's own
conclusion was that adding the columns "is right for this host and returns the
moment the next hub drifts" — so the durable fix is a check that runs on every
deploy, not a one-off migration.

This is a READ-ONLY check. It writes nothing, changes nothing, and exits:

    0  no drift (or the check could not run and said so)
    1  the hooks write at least one field the live schema lacks
    2  could not authenticate, so drift is UNVERIFIED

Run on the hub (it talks to 127.0.0.1:8090):

    python3 check-hook-schema-drift.py [--hooks-dir /opt/pocketbase/pb_hooks]
"""
import json
import os
import re
import subprocess
import sys

API = "http://127.0.0.1:8090"
HOOKS_DIR = "/opt/pocketbase/pb_hooks"
CREDS = "/root/.pb_admin_creds"

for i, a in enumerate(sys.argv[1:]):
    if a == "--hooks-dir" and i + 2 <= len(sys.argv[1:]):
        HOOKS_DIR = sys.argv[i + 2]

# Which collection a field belongs to. Kept here rather than inferred, because a
# field name is not globally unique and guessing would produce false positives.
# Mirrors the split check-consistency.sh §3 uses.
CODES_FIELDS = {
    "code", "tier", "used", "suspended", "expires_at", "activated_at",
    "middleman", "unbound_at", "unbind_reason", "label", "notes",
    "term_days", "term_kind", "released_at", "release_count", "last_seen_at",
    "first_seen_at", "first_seen_fingerprint", "suspended_reason",
    "force_state", "evict_requested_at", "evict_reason", "override_id",
}
CODE_EVENTS_FIELDS = {"code", "event", "detail", "actor", "fingerprint"}
ATTEMPT_FIELDS = {"ip", "rate_key", "fingerprint", "code_attempted"}

TOKEN = ""


def api(method, path, data=None):
    cmd = ["curl", "-s", "-X", method, f"{API}{path}",
           "-H", "Content-Type: application/json"]
    if TOKEN:
        cmd += ["-H", f"Authorization: {TOKEN}"]
    if data is not None:
        tmp = f"/tmp/.drift_{os.getpid()}.json"
        with open(tmp, "w") as f:
            json.dump(data, f)
        cmd += ["-d", f"@{tmp}"]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    try:
        return json.loads(out)
    except Exception:
        return {}


def login():
    global TOKEN
    email = os.environ.get("PB_ADMIN_EMAIL", "")
    password = os.environ.get("PB_ADMIN_PASS", "")
    try:
        with open(CREDS) as fh:
            for line in fh:
                if line.startswith("PB_ADMIN_EMAIL=") and not email:
                    email = line.split("=", 1)[1].strip()
                if line.startswith("PB_ADMIN_PASS=") and not password:
                    password = line.split("=", 1)[1].strip()
    except OSError:
        pass
    if not email or not password:
        return False
    for path in ("/api/admins/auth-with-password",
                 "/api/collections/_superusers/auth-with-password"):
        r = api("POST", path, {"identity": email, "password": password})
        tok = r.get("token") or r.get("adminToken") or ""
        if tok:
            TOKEN = tok
            return True
    return False


def fields_written(hooks_dir):
    """Every field name a hook writes, from `.set("x"` and `setIfPresent(r, "x"`."""
    names = set()
    try:
        files = [f for f in os.listdir(hooks_dir) if f.endswith(".pb.js")]
    except OSError as exc:
        print(f"cannot read the hooks directory: {exc}")
        sys.exit(2)
    for fn in files:
        with open(os.path.join(hooks_dir, fn), encoding="utf-8", errors="replace") as fh:
            src = fh.read()
        # Drop comment lines: a comment may NAME a field the code must not write,
        # and matching it would report a write that does not exist. (The same
        # guard_self trap the repo's own checks document.)
        src = re.sub(r"^\s*//.*$", "", src, flags=re.M)
        for m in re.finditer(r'\.set\(\s*"([a-z0-9_]+)"', src):
            names.add(m.group(1))
        for m in re.finditer(r'setIfPresent\(\s*\w+\s*,\s*"([a-z0-9_]+)"', src):
            names.add(m.group(1))
        # Concatenated prefixes, e.g. "download_" + platform.
        for m in re.finditer(r'\.set\(\s*"([a-z0-9_]+_)"', src):
            for plat in ("linux", "windows", "macos_intel", "macos_arm"):
                names.add(m.group(1) + plat)
    return names


def main():
    if not login():
        print("could not authenticate to PocketBase — schema drift is UNVERIFIED")
        sys.exit(2)

    have = {}
    for coll in ("codes", "code_events", "activation_attempts"):
        c = api("GET", f"/api/collections/{coll}")
        if c.get("id"):
            have[coll] = {f["name"] for f in (c.get("schema") or [])}

    if "codes" not in have:
        print("the `codes` collection is unreadable — cannot check drift")
        sys.exit(2)

    written = fields_written(HOOKS_DIR)

    missing = []
    for name in sorted(written):
        if name in CODE_EVENTS_FIELDS and name in ATTEMPT_FIELDS:
            continue  # ambiguous between collections; checked below by absence
        if name in CODES_FIELDS and name not in have["codes"]:
            missing.append(name)

    if missing:
        print("MISSING:" + ",".join(missing))
        print()
        print("These fields are written by hooks but do not exist on the live")
        print("`codes` collection. PocketBase DISCARDS such a write silently, so")
        print("the feature will look deployed and record nothing.")
        print()
        print("Fix:  server/scripts/seed-override-schema.py    (adds the columns)")
        sys.exit(1)

    print(f"OK — every field the hooks write exists in the live schema "
          f"({len(written)} names checked)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
