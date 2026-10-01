# Debugging method

```
audience:    builder
status:      live
authoritative-for: how a defect was found, proven and fixed — the practice, not the log
verified-against: docs/reference/FIXES.md (the dated defects this distils)
```

`FIXES.md` records **what** broke. [`STILL-OPEN.md`](../reference/STILL-OPEN.md)
records **what is not proven**. This document records **how to work** — the
sequence that repeatedly turned a confusing student report into a located defect,
and the specific traps that cost real releases.

It exists because the same class of mistake has now shipped four times
(3.2.9–3.2.12, then 3.2.14), each time through a fully green pipeline. The
recurrence is not bad luck; it is a method gap. Read this before diagnosing
anything that "should work".

---

## 1. The order that works

**Reproduce → locate → prove → fix → guard → verify the guard.**

Most of the cost in this repository has come from running those out of order, and
the expensive inversion is always the same: **fixing before reproducing**, which
produces a plausible change, a green pipeline, and no defect actually removed.

### 1.1 Reproduce against the live system, not the source

The single highest-value habit here. A report names a symptom; the source names
an intention; only the running system names the truth. For anything touching the
hub, the reproduction is a `curl`:

```sh
# What does the hub ACTUALLY tell a client? Not what should it.
curl -s "https://<domain>/api/update?version=<installed>&platform=windows" | python3 -m json.tool
```

The 2026-10-01 Windows defect was located **before reading a single line of Rust**,
because the response contained `locus-windows-amd64.exe` where the installer
belonged. The byte count in the student's error (`51741696`) matched that asset
exactly, which turned a vague "updater is broken" into a specific, located fact.

**Read the reported numbers as evidence.** Sizes, versions, hashes and offsets in
an error message are usually the identifier of the artifact involved. Matching
`51741696` against the release asset list is what proved which file was served.

### 1.2 Prefer the artefact over the configuration

A config says what someone intended; the artifact says what was built. When they
disagree, the artifact wins — and the disagreement **is** the defect.

```sh
# The manifest inside the release, not the manifest-generating code:
curl -sL https://github.com/<owner>/<repo>/releases/download/vX.Y.Z/manifest.json
```

This is the same rule as `docs/README.md` rule 3 ("the code wins"), applied to
produced artifacts rather than source.

### 1.3 Confirm the claim can be false before trusting it

Ask: *what observation would contradict this?* If nothing could, it is not a
finding. When the client refused the payload it said "not an installer" — that is
a checkable claim, and checking it (`MZ` present, `NullsoftInstaller` absent)
confirmed the client was behaving correctly and the **hub** was wrong. Had the
refusal been treated as the bug, the fix would have been to relax the guard,
which would have re-shipped the original defect.

**A guard firing is not a guard being broken.** Establish which side is wrong
before touching either.

---

## 2. Two-sided contracts

The most valuable generalisation in this file. When two programs must agree on a
value — a filename, a wire key, a version, an encoding — that value is a
**contract**, and the failure mode is not "side A is wrong" but **"A and B
disagree, and nothing compares them."**

Locus has several, all frozen and all easy to break by "tidying":

| Contract | Sides |
|---|---|
| Artifact filenames | CI's `manifest.json` ↔ `fetch-release.py` ↔ `publish-release.sh` ↔ the client's `artifact_for()` |
| Wire field names | hub hooks ↔ `contract.rs` (`uot_port`, `download_*`→`update_*`, the `macos_*`/`darwin-*` asymmetry) |
| Signature encoding | `fetch-release.py` ↔ `releases.publish` ↔ the plugin's `base64_to_string` |
| Version | five files ↔ the git tag ↔ `state.toml` |

**How to check a contract:** find every site, then compare them **to each other**.
A test that each site is self-consistent proves nothing — that is precisely the
trap described in §3.

**How to guard a contract:** assert the *agreement*, not the parties. The useful
guard for the Windows defect was never "does `fetch-release.py` mention windows"
(it always did) but "does the filename it would serve equal the one CI
advertised". See `check-consistency.sh` §1a and `smoke-publish.sh` case 5b.

---

## 3. Checks that cannot fail

The costly lesson, stated plainly: **a check that cannot distinguish a right
answer from a wrong one reads exactly like a check that passed.**

Three guards reported healthy through the release that no Windows client could
install:

- **A presence check.** `grep "\"windows\"" fetch-release.py` — the string was
  present before, during and after the bug.
- **A self-referential check.** The manifest cross-check hashed *whichever file
  the manifest named*. Manifest says `installer…`, check hashes `installer…`,
  hashes match, `ok`. It never compared the name the hub would **serve**.
- **A scoped check.** `check-consistency.sh` §15 grepped the workflow, which was
  correct, and never looked at the hub.

### The rule

> **Every new guard must be shown to fail against the code it is meant to
> catch.** Not "the tests pass" — the guard must be run against the pre-fix
> state and observed going red.

In practice: reintroduce the defect (or stub the fix), run the guard, confirm it
fails, restore. Both guards added on 2026-10-01 were verified this way and the
evidence recorded:

```console
# Guard against the pre-fix tree — three BAD lines, non-zero exit:
BAD  fetch-release.py still names 'locus-windows-amd64.exe' as a fetchable asset
BAD  publish-release.sh does not resolve the Windows installer by name
BAD  publish-release.sh still publishes the raw Windows binary as the update

# And the same smoke test against the pre-fix cross-check:
FAIL reports the name mismatch      # passes only with the fix
```

Also worth rehearsing: **`SKIP != PASS`.** A guard that silently skips reports
green while covering nothing. If a check skips on a condition, the skip must be
loud.

---

## 4. Where the truth lives (hub edition)

The hub is **three separate things, deployed three separate ways**. Confusing
them is the reason "I fixed it" and "it is still broken" were simultaneously
true on 2026-10-01.

| Piece | Lives at | Deployed by | Owns |
|---|---|---|---|
| PocketBase hooks | `/opt/pocketbase/pb_hooks/` | `hooks-sync.sh`, `setup.sh` | `/api/*` routes, `update_config` writes |
| Release fetcher | `/root/server/scripts/fetch-release.py` | `hooks-sync.sh --fetch-service`, `setup.sh` | which asset lands in which slot; bytes on disk |
| Static assets | `/var/www/updates/<version>/` | written by the fetcher | the downloadable files |
| Client | the student's machine | a release | the guard that refuses a bad payload |

**A fix committed in the repo is inert until the piece that runs it is
redeployed.** The repo is canonical, but the host has its own copy, and nothing
compared them — which is exactly how a corrected fetcher sat in the tree while
the hub served the raw binary.

### Verify the deployment, do not assume it

```sh
# The host's copy, by hash — not "the service is active".
ssh root@<host> 'sha256sum /root/server/scripts/fetch-release.py'
sha256sum server/scripts/fetch-release.py        # must match

# Does the host's copy can even load? An `active` service with a syntax error
# serves every request as a failure.
ssh root@<host> 'python3 -m py_compile /root/server/scripts/fetch-release.py'

# Is the right name resolved IN the copy that runs?
ssh root@<host> 'cd /root/server/scripts && python3 -c "
import importlib.util
s=importlib.util.spec_from_file_location(\"fr\",\"fetch-release.py\")
m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
print(m.resolve_platform_names(\"<version>\"))"'
```

`hooks-sync.sh --fetch-service --check` does all of the above, plus a `/health`
probe, and is the sanctioned way to ask the question.

> **`active` is not `correct`.** `systemctl is-active` says the process started.
> It says nothing about which file it is running or whether that file works.

---

## 5. Reading a defect report

Reported symptoms are honest; proposed causes usually are not. The same
`ERR_FILE_NOT_FOUND` text was reported three times, with three different causes
(FIXES.md, 2026-09-30). **A matching error string is not a matching cause.**

Ask two questions before accepting any cause:

1. **When** does the symptom appear — at install, on launch, on update?
2. **Could the proposed path have fired**, given the configuration in the report?

The withdrawn `start_page` diagnosis failed question 2: the reporter's config
carried `start_page: /`, so the described code path could not run. It was
inferred from source twice and never checked against the report's own facts.

**Keep the report's specifics.** They are the only ground truth available, and
they discriminate between candidate causes.

---

## 6. Writing the fix

- **Fix the cause, not the symptom.** Relaxing `is_installer_payload` would have
  made the 2026-10-01 error disappear and re-shipped the 2026-09-30 defect.
- **Fail closed.** An unverifiable artifact must be refused, not executed. The
  direction matters: the original guard was `!is_bare_executable(bytes)`, which is
  true of a one-byte file — a guard for executing the wrong binary cannot be built
  out of double negatives.
- **One writer per fact.** `update_config` is written by `releases.publish` only.
  Hand-editing SQLite puts a URL in the row with no artifact on disk, which 404s
  every client. Going around the pipeline bypasses the verification the pipeline
  exists to perform.
- **Keep the name in one place.** `installer_name()` (Python) and
  `WINDOWS_INSTALLER` (shell) each define the filename once, so the guard has
  something to pin.
- **Record what you did not verify.** See §7.

---

## 7. Honesty about verification

The distinction that keeps this documentation useful:

| Claim | Meaning |
|---|---|
| **Verified** | observed, with the command and the date recorded |
| **Unverified** | not observed — state the specific missing observation |
| **Falsified** | observed *not* to hold; this is a finding, not a failure |
| **Structural argument** | reasoned from code, not run |

The 2026-10-01 work demonstrates why the distinction matters. It **falsified**
`CLAIMS.md` §5 A7 — an installed Windows client could not update — while making
the hub serve the correct bytes. Those are different claims and only the first
was observed. Recording "fixed" for both would have been false.

**Never let a green pipeline stand in for an observed outcome.** Four defects in
this repo shipped through fully green CI, because every check tested that a
bundle *builds*, not that it *runs*.

When something remains unverified, write down:
- the **claim** (`A7`),
- the **specific observation** that would settle it (install on Windows → take
  the offered update → relaunch from the Start Menu),
- and **why this machine cannot make it** (no Windows hardware; nothing in CI
  launches either artifact).

That converts an open risk into a task someone can actually pick up.

---

## 8. Working on a live hub

The hub is production even when it looks quiet. `active = 0` stops *offering* an
update and **nothing else** — clients that already installed a build stay on it,
and there is no server-driven downgrade.

- **The only real fix for a bad release is a higher version.**
- **Test before turning it on** — publishing *is* offering; there is no rollout
  percentage.
- **Change one thing at a time**, and verify after each. A deploy and a publish
  in one step makes an unexpected result ambiguous.
- **Order dependencies matter.** On 2026-10-01 the correction had to land in the
  order *sign → deploy → publish*: signing requires CI, the fetcher requires the
  signature, and publishing requires the fetcher. Reversing any pair produces
  either a refused publish or a silently wrong one.
- **Leave the box as you found it.** A temporary credential is removed and
  confirmed removed; a modified file is backed up; the state you altered is
  stated.

---

## 9. Checklist

Before calling a defect fixed:

- [ ] Reproduced against the live system, and the reproduction is in the change description.
- [ ] Located to a specific line/asset, identified by evidence (a hash, a size, a filename), not by inference.
- [ ] Every side of any contract it touches identified and compared.
- [ ] The fix addresses the cause; the symptom would not return under a different path.
- [ ] A guard exists, and **was observed failing** against the pre-fix code.
- [ ] `bash server/scripts/check-consistency.sh` exits 0 (run with **`bash`** — under `sh`/`dash` it aborts and prints false `BAD` lines).
- [ ] Any deployment actually verified by hash/content, not by service status alone.
- [ ] Docs that asserted the old behaviour corrected **in the same change**.
- [ ] `FIXES.md` entry written; `STILL-OPEN.md` and `CLAIMS.md` updated with what is *not* proven.
- [ ] The honest claim class stated: verified / unverified / falsified / structural argument.

---

## 10. Why this document is not in an agent's memory

This method lives in the repository, versioned with the code, for the same reason
`CLAIMS.md` does: **a note in one agent's memory cannot be reviewed, corrected, or
inherited.** Documentation carries a `verified-against` field and is checked by
`check-consistency.sh`; a memory entry carries neither and silently rots.

If you find this document wrong, **fix it here**. The next agent reads the repo.
