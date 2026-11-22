# Release recovery — the signing key, the CI secret, and a stranded fleet

```
audience:    human-operator
status:      live
authoritative-for: recovering the update signing key, the CI secret and a fleet that cannot verify updates
verified-against: .github/workflows/client.yml, client/docs/SIGNING.md, server/scripts/secrets-decrypt.sh
```

Three different failures get confused with each other, and they need different
repairs. Decide which one you have **before** touching anything, because the wrong
repair is the expensive one:

| Symptom | You have lost | Go to |
|---|---|---|
| `.locus-keys/` is empty and you need to sign | nothing — an empty cache is the normal state | §2 — refill it |
| CI fails at signing with *"LOCUS_UPDATE_KEY is not set"* | the **Actions secret**, but the key file exists | §3 — re-set |
| Clients download an update and refuse it: *"created with a different key"* | nothing — they are on an **old key** | §4 — reinstall |
| §2 cannot produce a key the fleet trusts | the **key itself** | §5 — rotate, the last resort |

**Generating a new key is §5, and it is a breaking change.** It is never a
recovery step: it strands every installed client into §4. The 2026-10-16 rotation
happened only because §2 and §3 had both become impossible, and it cost the whole
fleet a manual reinstall.

---

## 1. What the key is, and where its copies live

One keypair signs every client update, and its public half is **compiled into the
client**. A client therefore trusts exactly one key — the one it was built with —
and nothing on the hub can change that. That single fact drives everything below.

It must exist in **three** places. The 2026-10-16 loss had only the first:

| Copy | Where | Can it be read back? |
|---|---|---|
| CI secret | GitHub Actions `LOCUS_UPDATE_KEY` | **No** — Actions secrets are write-only, by design. No API, `gh` command or UI action returns the value. |
| Offline | `~/.locus-secrets/locus_update.key` (outside the repo), plus a password-manager copy | Yes |
| Encrypted, in-repo | `LOCUS_UPDATE_KEY_B64` in `server/secrets.env.age` | Yes, with the bundle passphrase |

The working copy is generated into the **repo root's** `.locus-keys/`, which is
gitignored (verified with `git check-ignore -v .locus-keys/locus_update.key`).
That directory is a **cache, never a backup**: it is emptied after every signing
session on purpose, so that "the key is missing" is the *normal* state rather than
a symptom. §2 refills it in one command.

**Treat its absence as expected.** Because the cache is emptied deliberately, an
empty `.locus-keys/` tells you nothing about whether the key is lost. Ask the
bundle instead — `server/scripts/locus-key-extract.sh --check` answers "is the
key still recoverable, and is it the right one" without writing anything.

The current key is `4DA9B48F39251CEF`; it has been in use since the rotation
(see `client/docs/SIGNING.md`). The key ID is in the comment line of the
`.pub`; the value compiled into the client is in
`client/src-tauri/tauri.conf.json` → `plugins.updater.pubkey`, and
`client/src-tauri/tests/update_signature_contract.rs` asserts the two agree.

> **Check before believing a key is lost.** The key file is gitignored
> (`.locus-keys/`), is resolved from the **repo root** (not `client/` — a copy
> there is silently ignored), and CI writes it to `$RUNNER_TEMP` from the secret,
> which never survives the job. Run `locus-key-extract.sh --check` before
> concluding anything.

---

## 2. The private-key file is missing — recover it from the bundle

Do this **before** §3. If the key can be recovered, nothing else needs to change.

**This is no longer a procedure — it is a single command.** Every session that
signs extracts the key from the bundle, because `.locus-keys/` is a cache and not
a home (see [`.locus-keys/README.md`](../../.locus-keys/README.md)). Recovery and
routine use are therefore the *same* action, which is deliberate: a recovery
route that is exercised only when something is already broken is a route nobody
has tested.

```sh
# 1. Point at the passphrase (or let the script prompt for it).
export LOCUS_SECRETS_PASSPHRASE_FILE=~/locus-passphrase

# 2. Verify the bundle's key is the one the client trusts. Writes nothing.
server/scripts/locus-key-extract.sh --check

# 3. Extract it: .locus-keys/ becomes 0700, both files 0600.
server/scripts/locus-key-extract.sh
```

The script refuses to write on a wrong passphrase, and it will not write a key
whose ID disagrees with `plugins.updater.pubkey` in `tauri.conf.json` — a key that
signs correctly but is the *wrong* key produces updates every installed client
refuses, and nothing fails until a student tries to update.

> **What the script replaced, and why the old recipe was dangerous.** It used to be
> a hand-run sequence: decrypt the bundle, `grep` out `LOCUS_UPDATE_KEY_B64`,
> `base64 -d` it into place, then append a newline if the file did not end in one.
> That last step was the tell — the manual route could produce a key that decodes
> fine and that `minisign` later rejects, and the only way to catch it was to
> round-trip a probe signature by hand. Worse, the newline rule was a property of
> *how the value was stored*, not of the key, so any change to the bundle's
> encoding would have silently started producing subtly-wrong keys. The script
> writes the bytes it decoded and verifies by re-deriving the public half, so there
> is no format trivia left to get wrong.

**If the script reports the bundle's key does not match** `tauri.conf.json`, stop
and decide deliberately: either the bundle entry is stale (put the right key in
and re-encrypt) or the pubkey was rotated without re-encrypting. Only §5 — a
deliberate, fleet-breaking rotation — changes which key the clients trust.

---

## 3. The Actions secret is missing — re-set it from a key you can read

CI fails before signing with `LOCUS_UPDATE_KEY is not set`. Note this also fails
**branch** builds: signing is not tag-gated.

```sh
gh secret set LOCUS_UPDATE_KEY --repo <owner>/<repo> < .locus-keys/locus_update.key
gh secret list  --repo <owner>/<repo>     # prints names and dates, never values
```

The only real proof is a tag build that reaches the signing step. Do not tag a
release to test it — push a branch and read the `build` job's output, which
reports the secret's presence and length without printing it.

**Which operations lose the secret, and which do not:**

| Operation | Keeps the secret? |
|---|---|
| `git clone`, copying the working tree | No — it is not in git |
| Renaming the repository | **Yes** |
| Creating a new repository from this one | No |
| Transferring ownership | **Yes** |

---

## 4. Clients refuse the update — *"created with a different key"*

This is not a broken release and not a broken hub. The device is running a build
that trusts the **old** key, and the hub is correctly serving a release signed
with the **new** one. The client's verification is working.

There is no remote fix: a client cannot be told to trust a new key. The only path
is a manual **uninstall + reinstall** of the current build, per device.

What the rotation deliberately preserved, and why it matters here:

- **The app identity is unchanged** (`com.locus.client`). A reinstall is an update
  of the same app, not a migration to a new one, so no codes, storage or settings
  are orphaned by identity.
- **The version ladder is continuous** across the rotation (`3.2.30` → `3.3.0`).
  A reinstalled client can therefore auto-update to later releases normally.
  Resetting the ladder would have made *every future release* a manual install —
  that is the cost this choice avoids, and the reason not to reset it now.

For the operator: the affected devices are those built pre-rotation. There is
no server-side list of them — the hub holds no device identity by design
(`../reference/DEVICE-IDENTITY.md`). Expect the refusal report itself to be the
signal, and treat it as *"this device has not been reinstalled yet"* rather than
as a defect.

A device that reinstalls keeps its activation code: the client stores it
machine-scoped, so a reinstall usually restores access with no re-entry.

---

## 5. Rotating deliberately — the last resort

Only if §2 is impossible. This is a breaking change on the same level as changing
the app ID, and it must be planned as one (`client/docs/SIGNING.md` §5).

The order is not optional, and it is the reverse of what caused the last failure:

1. **Generate locally**, into the repo root's `.locus-keys/` — not `client/.locus-keys/`,
   which the test does not read.
2. **Encrypt the backup into the bundle** as `LOCUS_UPDATE_KEY_B64` (base64 of the
   whole file — `SECRETS-MANAGEMENT.md` FAQ says why the raw form breaks `source`).
3. **Take an offline copy** somewhere you control.
4. **Only now** set the Actions secret (§3).
5. Update `plugins.updater.pubkey` to base64 of the whole `.pub`, and the bare-key
   literal in `update_signature_contract.rs` — the test pins a value, and leaving
   the old one there makes it guard something that no longer exists.
6. **Bump the major** and ship it as the new key's first release.

Do the sign/verify round trip in §2 between steps 2 and 4. A key that reaches
GitHub unverified cannot be checked afterwards, which is exactly the state that
made the 2026-10-16 loss unrecoverable.

---

## 6. What is verified, and what is not

**Verified on 2026-10-16, for the current key** (the rotation the rest of this
file responds to). Recorded as a dated observation, not as a rule:

- **CI built and signed the release.** Run `38830217320` on branch `v3.3.0`:
  `Verify`, all four `Build` jobs, `Report` and `Release` all succeeded.
- **The release exists with its signatures.** `gh release view v3.3.0 --json assets`
  lists the four platforms plus a `.sig` for each and a `manifest.json`.
- **A real payload verifies against the new key** and is rejected by the retired
  one. The artifact is `locus-linux-amd64` (124,232,456 bytes) —
  `client/docs/SIGNING.md` §6 has the command.

**Not verified:**

- **A key recovered by §2 signing a release CI accepts.** §2's round trip proves
  the recovered key is intact and that its public half is the pair. It cannot prove
  the pair is the one the fleet trusts — that is the `.pub` comparison, and beyond
  it a tag.
- **A reinstalled client taking an update afterwards.** Argued from the continuous
  ladder — the reinstalled build is on the same line as the releases that follow it
  — not observed on hardware. Same open item as
  [`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md) ("no update has ever
  successfully been installed").
