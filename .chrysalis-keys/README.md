# `.chrysalis-keys/` — a cache, not a home

**This directory is empty after a clone, and that is the intended state.** The
only committed file here is this README.

The signing key used to live here, and that is what cost the fleet a manual
reinstall on 2026-10-16: the directory is gitignored, so it vanished with the
working tree, and the key survived only because an unencrypted copy happened to
still be on the deployed host. **A secret that lives only in a gitignored
directory is not backed up.**

So the arrangement is now:

| Copy | Where | Recoverable? |
|---|---|---|
| **Durable** | `LOCUS_UPDATE_KEY_B64` (a DEPRECATED ALIAS key name inside the bundle — see [`../docs/reference/NAME-MIGRATION.md`](../docs/reference/NAME-MIGRATION.md)) in [`server/secrets.env.age`](../server/secrets.env.age) (committed) | **Yes** — clone + passphrase |
| CI | GitHub Actions secret `LOCUS_UPDATE_KEY` (also a DEPRECATED ALIAS name; renaming it is an operator action, not a code change) | No — Actions secrets are write-only by design |
| Offline | `~/.chrysalis-secrets/chrysalis_update.key`, outside the repo | Yes |
| **Cache** | `chrysalis_update.key` in this directory | **Don't care** — regenerated every session |

## Extracting the key (every session that signs)

`.chrysalis-keys/chrysalis_update.key` is **not kept between sessions.** Rehydrate it
from the bundle when you need to sign, and remove it when you are done:

```sh
# 1. Put the passphrase somewhere the script can read it (or let it prompt).
#    This is the same passphrase secrets-decrypt.sh uses.
export CHRYSALIS_SECRETS_PASSPHRASE_FILE=decryptionkeyforsecretsdeleteafteruse.txt

# 2. Verify the bundle's key is the one the client trusts, writing nothing.
./server/scripts/chrysalis-key-extract.sh --check

# 3. Extract it. Writes .chrysalis-keys/ as 0700 and both files as 0600.
./server/scripts/chrysalis-key-extract.sh

# ...sign what you need to sign...

# 4. Remove the cache. This is not optional housekeeping; it is the point.
./server/scripts/chrysalis-key-extract.sh --remove
```

The script does three things worth knowing about, and they are the reason to use
it rather than `base64 -d` by hand:

1. **It refuses to write on a wrong passphrase.** Nothing is left behind.
2. **It verifies the extracted key is the key the client trusts**, by deriving
   the public half with `minisign -R` and comparing its key ID against
   `plugins.updater.pubkey` in `tauri.conf.json`. A key that signs correctly but
   is the *wrong* key produces updates every installed client refuses, and
   nothing fails until a student tries to update — so this check is the one that
   actually protects the fleet.
3. **It asserts the file modes** (0700 / 0600) rather than inheriting a `umask`.

## Handling rules

- **Never `git add -f` anything here except this README.** `git ls-files
  .chrysalis-keys/` must print exactly `.chrysalis-keys/README.md`; `check-consistency.sh`
  §5b fails the build if it prints anything else.
- **Never treat the file here as the backup.** If you find yourself copying it
  somewhere "just in case", the thing to do instead is confirm
  `LOCUS_UPDATE_KEY_B64` in the bundle is current — that is the copy that
  survives.
- **Never generate a replacement keypair to fix a missing key.** Generating is not
  recovery: it changes the key ID the updater trusts, so the clients that need the
  fix are precisely the ones that cannot be told about it. Read
  [`docs/operate/RELEASE-RECOVERY.md`](../docs/operate/RELEASE-RECOVERY.md) first.
- **The public half is not secret.** It is published in `tauri.conf.json`; it
  lives here only so a signing session has both halves in the place `minisign`
  expects.

## How it is checked

- `server/scripts/chrysalis-key-extract.sh --check` — re-derives the public half and
  compares key IDs. Fails loudly on a stale bundle entry or a rotated pubkey.
- `server/scripts/release-gate.sh` step 5 — the same comparison for the on-disk
  file, and the gate that fails a release rather than signing with a wrong key.
- `server/scripts/check-consistency.sh` §5b — asserts this README is the only
  tracked file here, that it names the bundle, and that the extraction script
  exists; §5c asserts the plaintext `.secrets.env` is never tracked.

The two published and bundled copies currently agree on key ID
`4DA9B48F39251CEF`, shipped from `3.3.0`.

## The other secret, and its different home

The **operator secrets** are a separate secret with a separate file:
`server/secrets.env.age` (committed ciphertext), decrypted by
`server/scripts/secrets-decrypt.sh` with the same passphrase. Nothing about them
belongs in this directory.

> **Losing the passphrase is unrecoverable**, and it takes this key with it. Keep
> it in a password manager, not only on the box you are about to lose. See
> [`docs/operate/SECRETS-MANAGEMENT.md`](../docs/operate/SECRETS-MANAGEMENT.md).
