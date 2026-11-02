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

Five separate outages in nine days, all of the same shape: the build depended on
a URL that a third party owned. The last two prove the point the first three made
— a repin is a repair, not a fix. The fifth followed the fourth by under an hour.

| Date | What disappeared | Cost |
|---|---|---|
| 2026-10-26 | `alpha-94b13ae`, the mihomo alpha, deleted when the prerelease was re-rolled | The only copy anywhere. The client could not compile. |
| 2026-10-28 | Four `meta-rules-dat` asset IDs, republished under new IDs | Both `Build (linux)` and `Build (windows)` stopped. |
| 2026-10-29 | `alpha-6780eee`, re-rolled to `alpha-ca8d7c4` | macOS and Windows stopped; linux survived **only** because it was the one platform vendored. |
| 2026-11-02 | The same four `meta-rules-dat` IDs again, after a 2026-11-01 rebuild — and this time the **content changed** too | Every `Build (platform)` job stopped in "Fetch sidecars and geo data". Repinned. |
| 2026-11-02 | **Those replacement IDs, again, within the hour** (2026-11-02 01:42 rebuild) | Same step, same four assets. Repinned again — five in nine days. **This is the row that says repinning is not a fix.** |

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

## 4. The four geo databases: mirrored as of 2026-11-03

`ASN.mmdb`, `Country.mmdb`, `geoip.dat` and `geosite.dat` live in
`vendor/build-inputs/resources/` and are **committed**. `prebuild.mjs` copies
them into `client/src-tauri/resources/` (gitignored) and verifies each against
its pin; the asset ID in the lock is what the bytes were fetched from once, not
something a build reaches for. The earlier "declared but no committed copy"
warning path in `prebuild.mjs` is now a defect if it fires.

**Why it was done now, and not earlier.** The lock's `vendored.resources` entry
was written on 2026-10-29 with the directory empty, deliberately: the risk was
recorded before the fix. Five outages in nine days followed, the fifth within an
hour of the fourth, and the 2026-11-02 CI runs showed the real cost — `Build
(macos-arm)` could not fetch `geosite.dat` at all (403 from the API), while
`Build (linux)` succeeded only because that runner's cache still held the bytes.
A build that passes because a cache is warm is not a build.

**Verified, in the order it matters.** The four committed files match the pins
the lock already held, so **no repin happened**; `prebuild.mjs` run with
`HTTPS_PROXY` pointed at a dead port reported all four *"is vendored in the
repository and matches its pin"* and printed no `download finished` line naming
them; and the real vendored engine loaded them — `GeoSite cn records: 110177`,
`GeoIP cn records: 10012`.

**Cost:** 35.8 MiB committed.


**Repinned twice on 2026-11-02.** The 2026-10-28 pins returned `404` after
MetaCubeX rebuilt the rolling release on 2026-11-01, which failed every
`Build (platform)` job. That repin was itself dead within the hour: a third
rebuild on 2026-11-02 at 01:42 deleted those IDs too, and the lock now carries
the **2026-11-02** asset IDs. Five repins in nine days.

**The content moved, so the hash alone was not enough.** Unlike the 2026-10-28
event — where three of four files came back byte-identical — the 2026-11-01
rebuild changed all four (`Country.mmdb` +1290 bytes, `ASN.mmdb` +18994,
`geosite.dat` +997, `geoip.dat` −20810). A `--write` would have recorded any bytes
that happened to download. The repin was accepted only after the real vendored
engine loaded each one: `geosite.dat` as `Load GeoSite rule: cn … records:
110177`, and `Country.mmdb`/`ASN.mmdb` as `Load MMDB file` / `Load ASN file`
while matching live traffic through a `GEOIP` rule.

**Verifying a geo repin — the short version.** Point the vendored engine at a
config with a `GEOIP` and a `GEOSITE` rule, run it against the new files, and
confirm it logs both a GeoSite record count and an MMDB load. A stub, a truncated
download or an HTML error page fails to parse; "the size looks plausible" does
not distinguish them.

**What is still fetched from upstream.** Mirroring these four closed the input
that actually broke builds. Two remain downloaded when a checkout lacks a
vendored copy, and neither is protected by this change:

- **`clash-verge-service-ipc` — the real remaining gap.** This is the `.tar.gz`
  that `prebuild.mjs` unpacks into the three service helpers. It is downloaded
  **unconditionally, with no lock entry and no hash check** (`resolveServiceRelease`
  derives the URL from the Cargo manifest), so it is the one build input that
  both depends on an upstream URL and cannot be verified when it arrives. Its
  upstream has been stable, which is the only reason it has not caused an
  outage. It is the next thing to pin and mirror.
- the engines themselves, when `vendor/build-inputs/` is absent — but there they
  are committed, so a fresh clone reads them from disk.

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
