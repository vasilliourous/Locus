# `.locus-keys/` — what this directory holds, and what it is worth

**This file is committed. The keys beside it are not** — the whole directory is
ignored by [`.gitignore`](../.gitignore) (`.locus-keys/`), so a fresh clone has
this directory and nothing in it. That is why the file you are reading exists: a
directory that appears to hold a secret and explains nothing is how a secret gets
deleted by someone tidying up.

## The two files

| File | What it is | Can it be replaced? |
|---|---|---|
| `locus_update.key` | The **minisign private key** that signs every updater artifact. | **No.** |
| `locus_update.pub` | Its public half. The same string `client/src-tauri/tauri.conf.json` publishes in `plugins.updater.pubkey`. | Yes — regenerating it alone does nothing. |

**`locus_update.key` is the single most dangerous file to lose in this
repository.** Every installed client verifies updates against the public key
compiled into it and the Tauri updater has no bypass. Lose the private key and no
already-installed client can ever be updated again — the fleet is stranded on
whatever version it is running, and a new build cannot reach it.

> **Before generating a replacement, read
> [`docs/operate/RELEASE-RECOVERY.md`](../docs/operate/RELEASE-RECOVERY.md).**
> Generating a new keypair is not recovery: it changes the key ID the updater
> trusts, so the clients that need the fix are precisely the ones that cannot be
> told about it.

## How it is checked

`server/scripts/release-gate.sh` step 5 compares the key ID in `locus_update.pub`
with the ID decoded from the published pubkey in `tauri.conf.json`, and fails when
they differ — `CI would sign with a key the updater rejects.` The two currently
agree on key ID `4DA9B48F39251CEF`.

The key is **not** in this directory on CI. Actions builds sign with the
`LOCUS_UPDATE_KEY` repository secret, which is the same key, base64-encoded. A
fresh clone has no such secret and every signed build fails until it is set — this
is recorded as a trap in the root [`AGENTS.md`](../AGENTS.md).

## Handling rules

- **Never `git add -f` anything here.** Check `git ls-files .locus-keys` — it
  must print only this README.
- **Keep the directory `0700` and both keys `0600`.** Nothing enforces it; it is
  the one thing between the signing key and every other process on the machine.
- **Back the private key up somewhere offline**, separately from this checkout,
  and record where in the operator's own vault — not in this repository.
- The plaintext operator secrets are a *different* secret, with a different home:
  `server/secrets.env.age` (committed ciphertext) is decrypted by
  `server/scripts/secrets-decrypt.sh` with a passphrase. Losing the passphrase is
  recoverable from the deployed host; losing this key is not.
