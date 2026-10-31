# Vendored build inputs

```
audience:    builder + human-operator
status:      live
authoritative-for: which build inputs are committed to the repository, where they live, and how to move one
data:        server/data/sidecar.lock.json (`vendored`) — the machine-readable half
guard:       server/scripts/check-consistency.sh §39, server/scripts/test-contract-guard.sh
```

The client build needs eight binaries and four databases that are not source.
They are pinned by sha256 in [`sidecar.lock.json`](../../server/data/sidecar.lock.json),
and as of **2026-10-29 the eight binaries are also committed**, under
`vendor/build-inputs/`. The four databases are pinned but **not** mirrored — see
"What is not vendored yet", which is the honest half of this page.

---

## 1. Why a build input is in the repository at all

Three separate outages in one week, all of the same shape: the build depended on
a URL that a third party owned.

| Date | What disappeared | Cost |
|---|---|---|
| 2026-10-26 | `alpha-94b13ae`, the mihomo alpha, deleted when the prerelease was re-rolled | The only copy anywhere. The client could not compile. |
| 2026-10-28 | Four `meta-rules-dat` asset IDs, republished under new IDs | Both `Build (linux)` and `Build (windows)` stopped. |
| 2026-10-29 | `alpha-6780eee`, re-rolled to `alpha-ca8d7c4` | macOS and Windows stopped; linux survived **only** because it was the one platform vendored. |

That third row is the important one, and it is a lesson about the shape of the
fix rather than the size of it: **vendoring one platform converted a fleet-wide
outage into a single-platform success**, and the success hid the other three.
A partial vendoring is not a smaller version of a full one; it is a different
failure that reports green.

`STILL-OPEN.md` recorded the trade nobody had made — "roughly 240 MiB of
repository size". The measured cost is **464.7 MiB**, in three groups of
229.7 / 229.3 / 5.7 MiB. The estimate was low by roughly half because it counted
four copies of the *stable* engine and treated the alpha as one platform; the
alpha alone is four copies at the same size again. Measured with `du -sb`, not
estimated — the previous number in this repository was an estimate, and it is the
reason the trade looked cheaper than it is.

---

## 2. What is vendored

| Group | Triples | Size | Why |
|---|---|---|---|
| `verge-mihomo` | all four | 229.7 MiB | `externalBin` in `tauri.conf.json`, so every platform bundles it. A stable GitHub release asset is the more durable upstream, but it is still an upstream. |
| `verge-mihomo-alpha` | all four | 229.3 MiB | Published as a single **rolling** prerelease: MetaCubeX deletes the previous build the moment a new one lands. All four, because one is what lost the other three. |
| `clash-verge-service` | `x86_64-unknown-linux-gnu` | 5.7 MiB | The privileged helper the client installs. Its upstream is stable, so this is not an outage measure: without it `--check` reports the three binaries as `MISSING` on a tree with no download directory — a check that fails on a correct tree, which is how a check stops being read. |

The layout mirrors the lock's `vendored` keys exactly, so a reader can go from a
lock entry to the bytes without guessing:

```text
vendor/build-inputs/
  verge-mihomo/           verge-mihomo-<triple>[.exe]
  verge-mihomo-alpha/     verge-mihomo-alpha-<triple>[.exe]
  clash-verge-service/    clash-verge-service{,-install,-uninstall}-<triple>
  resources/              NOT PRESENT — see §4
```

`prebuild.mjs` **copies** a vendored group into its normal build location
(`client/src-tauri/sidecar/`, `client/src-tauri/resources/`) and verifies the
copy against the pin. It does not build from `vendor/` directly, because
`externalBin` names a path inside `sidecar/` and the bundler reads that.

---

## 3. The rules

1. **The lock decides what is vendored, the filesystem decides where.** A file is
   vendored if `vendor/build-inputs/<group>/<file>` exists for a group named in
   the lock's `vendored` table. One function answers this for sidecars and for
   resources, so the two cannot diverge — they already did once, and
   `verge-mihomo` was vendored in every sense except the one the code checked.
2. **A declared group with no bytes warns; it never silently downloads.** Today
   `resources` is declared and empty, which is legitimate and expected. If any
   *other* declared group loses its files, the build is loud about it rather than
   quietly returning to the network — that is the failure the vendoring exists to
   prevent, and it would look exactly like success.
3. **A vendored file is verified wherever it is found.** `sidecar-lock.py --check`
   hashes `sidecar/` when prebuild has run and the committed copy when it has
   not. A fresh clone must pass that check; before 2026-10-29 it did not.
4. **Do not commit the same input twice.** The alpha used to live in
   `client/src-tauri/sidecar/` as well. Two committed copies of a 62 MiB binary
   can disagree, and the build bundled whichever the copy step last wrote.
5. **Every addition here needs a proof in `test-contract-guard.sh`.** Three
   exist: a tampered vendored copy is caught, a fresh clone verifies instead of
   reporting `MISSING`, and an untracked vendored binary is caught (§39b).
6. **A new binary must be added to `.gitignore`'s exception list.** The root
   `.gitignore` excludes `*.exe`, `*.zip` and `*.tar.gz` to keep build output out
   of the repository, and that rule silently excluded the two Windows engines on
   the first commit of this vendoring — `git add -A` staged **9 of 11** files and
   the tree still looked complete. A fresh clone would have rebuilt on Windows by
   downloading its engine, and every build on the committing machine would have
   passed. `check-consistency.sh` §39b now fails on it, which is why the rule is
   written down here as well as enforced there.

---

## 4. What is not vendored yet: the four geo databases

`ASN.mmdb`, `Country.mmdb`, `geoip.dat` and `geosite.dat` are **pinned by
GitHub release-asset ID, and not mirrored**. Their group is declared in the lock
and the directory does not exist, on purpose:

- **I could not fetch the pinned bytes.** All four asset IDs from the 2026-10-28
  republish now return `404`; the `latest` URL serves different bytes
  (`Country.mmdb` hashes `9a485c40…` against a pin of `ccec4d6f…`). Committing
  the wrong bytes under a correct pin would be strictly worse than downloading
  the right ones, and repinning to `latest` would reintroduce exactly the
  drift-by-republish the asset-ID pin exists to stop.
- **One of them differs by design.** The republish that touched `geosite.dat`
  was a genuine upstream routing-data update (4,307,247 → 4,353,539 bytes).
  "The bytes that passed their pin" and "the newest bytes" are different files,
  and choosing the second silently is a data change nobody reviewed.

**What would close it.** On a host that still holds the pre-2026-10-28 bytes —
a CI cache, or an image built before the republish — run
`server/scripts/sidecar-lock.py --check`, copy the four files it verifies into
`vendor/build-inputs/resources/`, and re-run `--check` to confirm. Then the
`declaredButAbsent` warning in `prebuild.mjs` becomes a defect rather than the
expected state, and that step should be removed in the same change.

**Why the pin still protects in the meantime.** Asset IDs are immutable
per-upload even when not permanent; §39 fails the build when the bytes stop
matching the pin. The gap is durability, not integrity.

---

## 5. Moving a vendored input

```bash
# 1. Fetch the new bytes INTO vendor/, by triple. If the file's extension is on
#    the root .gitignore's artifact list (.exe/.zip/.tar.gz), add a narrow
#    `!vendor/build-inputs/<group>/<file>` exception or it will not be committed.
# 2. Regenerate the pins. The diff IS the review artifact.
python3 server/scripts/sidecar-lock.py --write
git diff server/data/sidecar.lock.json      # a sha256 change per file
# 2b. Confirm git actually carries them. This is the check that would have
#     caught 9-of-11, and §39b runs it too.
bash server/scripts/check-consistency.sh | grep -A2 '^39b'

# 3. Prove the guards still fail on the defects they claim to catch.
bash server/scripts/test-contract-guard.sh

# 4. Prove the build reads the committed copy and not the network.
mv client/src-tauri/sidecar/* /tmp/         # remember to put them back
HTTPS_PROXY=http://127.0.0.1:9 node client/scripts/prebuild.mjs x86_64-unknown-linux-gnu
#   → "<name>" is vendored in the repository and matches its pin
#   → NO "download finished" line naming that file
```

Step 4 is not ceremony. It is how the divergence in §3 rule 1 was found: the
bytes were right, the log said `download finished`, and **only a dead proxy
distinguishes "read from disk" from "happened to get the same bytes"**.

> **A vendored sidecar is never re-downloaded.** `--force` does not delete it and
> does not fetch it. A stale vendored copy therefore fails its pin forever rather
> than healing itself, which is deliberate — the failure names the file and the
> two hashes, and the fix in §5 is one command.

---

## 6. Related

- [`RELEASING.md`](RELEASING.md) — the release order this feeds into.
- [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md) — the geo-database
  gap above, and the alpha's history.
- [`../reference/FIXES.md`](../reference/FIXES.md) — the three outages, dated.
- `client/scripts/prebuild.mjs` — the only consumer.
- `server/scripts/sidecar-lock.py` — the pin, and the check.
