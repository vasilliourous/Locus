#!/usr/bin/env python3
"""Backfill `term_days` onto existing codes, without changing any expiry.

WHY THIS EXISTS
===============

The term model adds `term_days` to a code: how long ONE purchase lasts,
measured from activation. New codes get it at mint. Codes that already exist
have no term, and the migration must decide what to do about that.

THE ONE RULE: **derive a term where one is provable; never touch `expires_at`.**

The live hub holds real, paying customers (the "Wanzhen" Strike codes, per
docs/business/10-lifecycle.md §10.2). Shortening a paying student's remaining
time is the worst outcome this migration can produce. So this script:

  * NEVER writes `expires_at`. Not to set it, not to clear it, not to correct
    it. That column is not touched by this script at all.
  * writes `term_days` ONLY when it can be derived from the code's own history,
    i.e. when both `activated_at` and `expires_at` exist and produce a sane
    positive number of days.
  * leaves everything else alone and reports it, so a human decides.

A code with no `term_days` behaves exactly as it does today (never silently
becomes a dated code), so an un-backfilled row is not a broken row.

USAGE
=====

    # See what WOULD change. Writes nothing. Run this first.
    stamp-backfill-terms.py <admin_api_token> --dry-run

    # Apply it. Writes `term_days` and one code_events row per touched code.
    stamp-backfill-terms.py <admin_api_token> --apply

Optional: --term-kind term|month|year to label derived terms (display only;
never used in arithmetic).

SAFETY
======

Idempotent: a code that already has `term_days` is skipped, so re-running is
harmless. Talks to the admin console hook over loopback — the same audited path
the web console uses — so every write lands in `code_events`.
"""

import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone

ENDPOINT = "http://127.0.0.1:8090/api/admin/console"

# A derived term outside this range is not a term, it is a data accident. Below
# a day, the arithmetic is meaningless; above ~3 years, the code was almost
# certainly minted with a far-future date rather than sold as a real span, and
# guessing a term there would be inventing a policy the operator never chose.
MIN_TERM_DAYS = 1
MAX_TERM_DAYS = 1095

# Sentinel used to mean "leave this term alone".
UNSET = "0"


def rpc(token: str, action: str, **kwargs):
    """Call one admin console action. Returns the parsed JSON response."""
    body = {"action": action, "admin_token": token}
    body.update(kwargs)
    req = urllib.request.Request(
        ENDPOINT,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)


def parse_pb_date(value: str):
    """Parse a PocketBase date string to an aware datetime, or None.

    Mirrors the tolerant parsing the hooks use (`parsePBDate`): PocketBase
    stores a SPACE separator, which strict RFC3339 rejects, so a naive parser
    would return None for every real value.
    """
    text = (value or "").strip()
    if not text:
        return None
    candidates = [text, text.replace(" ", "T")]
    for candidate in candidates:
        for suffix in ("Z", "+00:00", ""):
            try:
                parsed = datetime.fromisoformat(candidate.rstrip("Z") + suffix) if suffix else datetime.fromisoformat(candidate)
                return parsed if parsed.tzinfo else parsed.replace(tzinfo=timezone.utc)
            except ValueError:
                continue
    return None


def derive_term(code: dict):
    """The term this code's own history implies, or None if not provable.

    Returns (term_days, note). A None term means the script leaves the code
    alone; `note` explains which case it fell into, so the dry-run output is
    actionable rather than just a count.
    """
    if code.get("term_days"):
        return None, "already has a term"

    expires = parse_pb_date(code.get("expires_at", ""))
    activated = parse_pb_date(code.get("activated_at", ""))
    expiry_raw = (code.get("expires_at") or "").strip()

    if not expiry_raw:
        return None, "no expiry recorded — operator must decide (a permanent code stays permanent)"
    if expires is None:
        return None, "unparseable expiry — left alone rather than guessed at"
    if activated is None:
        # The common case for stock that was minted but never activated: the
        # expiry is a mint-relative date, and there is no activation to measure
        # a term from. Deriving one here would invent a policy.
        return None, "never activated — expiry is mint-relative, no term provable"

    days = round((expires - activated).total_seconds() / 86400)
    if days < MIN_TERM_DAYS:
        return None, f"derived {days}d is below the sane minimum — left alone"
    if days > MAX_TERM_DAYS:
        return None, f"derived {days}d exceeds {MAX_TERM_DAYS}d — looks mint-relative, left alone"
    return days, f"{days}d from its own activated_at → expires_at span"


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    flags = {a for a in sys.argv[1:] if a.startswith("--")}

    if len(args) < 1:
        print(__doc__.strip().split("USAGE")[-1].strip())
        return 2

    token = args[0]
    apply_changes = "--apply" in flags
    dry_run = "--dry-run" in flags or not apply_changes
    term_kind = ""
    if "--term-kind" in sys.argv:
        try:
            term_kind = sys.argv[sys.argv.index("--term-kind") + 1].strip()
        except IndexError:
            print("--term-kind needs a value (term|month|year)")
            return 2

    if not token:
        print("backfill: no admin API token — nothing to do")
        return 0

    try:
        listing = rpc(token, "codes.list")
    except (urllib.error.URLError, OSError) as exc:
        print(f"backfill: codes.list failed: {exc}")
        return 1

    if not listing.get("ok"):
        print(f"backfill: codes.list rejected: {listing.get('message')}")
        return 1

    codes = listing.get("data", {}).get("codes") or []
    print(f"backfill: {len(codes)} code(s) on the hub")
    print(f"mode: {'DRY RUN (nothing will be written)' if dry_run else 'APPLY'}")
    print()

    derivable, untouched = [], []
    for code in codes:
        days, note = derive_term(code)
        if days is None:
            untouched.append((code, note))
        else:
            derivable.append((code, days, note))

    if derivable:
        print(f"WOULD SET a term on {len(derivable)} code(s):")
        for code, days, note in derivable:
            label = code.get("label") or "(no label)"
            print(f"  {code.get('code')}  {code.get('tier')}  {label}")
            print(f"      term_days={days}  ({note})")
        print()

    if untouched:
        print(f"LEAVING {len(untouched)} code(s) alone:")
        for code, note in untouched:
            label = code.get("label") or "(no label)"
            print(f"  {code.get('code')}  {code.get('tier')}  {label}")
            print(f"      {note}  expires_at={(code.get('expires_at') or '(none)')}")
        print()

    # The invariant this whole script exists to protect, stated in the output
    # so an operator reading a dry run sees it rather than having to trust it.
    print("No code's `expires_at` is read-modified by this script. Paying students")
    print("keep their exact remaining time.")
    print()

    if dry_run:
        print("Dry run complete. Re-run with --apply to write the terms above.")
        return 0

    if not derivable:
        print("Nothing to apply.")
        return 0

    written, failed = 0, 0
    for code, days, _note in derivable:
        try:
            result = rpc(token, "codes.set-term",
                         code=code.get("code"), term_days=days, term_kind=term_kind,
                         reason="migration: derived from activated_at → expires_at")
            if result.get("ok"):
                written += 1
            else:
                failed += 1
                print(f"  ✗ {code.get('code')}: {result.get('message')}")
        except (urllib.error.URLError, OSError) as exc:
            failed += 1
            print(f"  ✗ {code.get('code')}: {exc}")

    print(f"Applied terms to {written} code(s); {failed} failed.")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
