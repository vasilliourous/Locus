# Starting a working session

```
audience:    builder
status:      live
authoritative-for: the order to bring a checkout up to a state where you can build, test and sign
verified-against: .locus-keys/README.md, server/scripts/locus-key-extract.sh, server/scripts/secrets-decrypt.sh
```

The steps below are in order because each one's failure mode is confusing if you
hit it cold. Nothing here is a secret, and nothing here is remembered: **a fresh
clone plus one passphrase is enough**, and this page is the whole of what that
buys you.

> **Secrets never travel with the repository.** Not with a clone, not with a copy
> of the directory, not with a new repository made from this one, and — this is
> the one that keeps catching people — **not with the gitignored files beside the
> code either.** The only durable copy of every secret is
> `server/secrets.env.age`, which *is* in git.

---

## 1. Check out, and clear the environment

```sh
git clone <repo> && cd Locus
direnv allow            # or: set -a; . ./.envrc; set +a
```

`.envrc` unsets `LD_LIBRARY_PATH` and `LD_PRELOAD`. This is not tidiness: a shell
that inherited a webkit2gtk **4.0** toolchain from somewhere makes `cargo test`
and `tauri dev` fail at **link** time with a symbol error that names neither
webkit nor the environment. The repo cannot fix it for you, because it belongs to
the parent process — so the file clears it instead of explaining it.

## 2. Install dependencies

```sh
(cd client && pnpm install)
(cd server/console && pnpm install)
```

The vendored engines are already committed under `vendor/build-inputs/`, so the
build does not reach the network for mihomo or the geo databases. `prebuild.mjs`
copies them into place on first build.

## 3. Unlock the secrets — only if this session needs them

Most sessions do not. Reading a file, running tests and editing the UI need no
secret at all. You need this step to **deploy**, **seed the hub**, or **sign an
update**.

```sh
export LOCUS_SECRETS_PASSPHRASE_FILE=<path to the passphrase file>

# Prove the bundle is readable and current before relying on it:
server/scripts/secrets-decrypt.sh --check
```

The passphrase is **not** in the repository, and losing it is unrecoverable — it
is the one thing in this list that a clone cannot replace. Keep it in a password
manager as well as on disk. See
[`SECRETS-MANAGEMENT.md`](SECRETS-MANAGEMENT.md).

> **`age` has no non-interactive passphrase input.** It opens `/dev/tty` directly
> and rejects `-i` for passphrase files; the scripts work around this with
> `script -qec`. You do not need to know that to use them — you do need to know
> it before you "simplify" them into something CI can call.

## 4. Extract the signing key — only if this session will sign

`.locus-keys/` is **empty after a clone, and that is correct.** It is a cache, not
the key's home. The durable copy is `LOCUS_UPDATE_KEY_B64` inside the bundle.

```sh
server/scripts/locus-key-extract.sh --check   # verify, write nothing
server/scripts/locus-key-extract.sh           # extract: 0700 dir, 0600 files
```

The script refuses to write a key that does not match
`plugins.updater.pubkey` in `tauri.conf.json`. That check matters more than it
looks: a key which signs perfectly but is the *wrong* key produces updates every
installed client refuses, and nothing fails until a student tries to update.

**When the session is over:**

```sh
server/scripts/locus-key-extract.sh --remove
```

Removing it is the point, not housekeeping. The 2026-10-16 incident happened
because this directory was treated as the key's home; when the working tree was
lost, the key went with it, and the fleet paid a manual reinstall. Keeping the
cache empty by default means the *recovery* path is the path used every day — so
it is exercised rather than discovered broken.

[`.locus-keys/README.md`](../../.locus-keys/README.md) owns the detail.

## 5. Build and run the gates

Run all of these before pushing. The Linux build runs three separate Rust gates,
and **passing tests says nothing about the other two** — a change with 685 passing
tests went red because it was never formatted.

```sh
cd client

# Frontend
pnpm typecheck
pnpm lint
pnpm format:check
pnpm test

# Rust — all three
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features clippy -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check

# Cross-language consistency. RUN IT WITH bash — under sh/dash it aborts with
# "Bad substitution" and prints false BAD lines.
bash ../server/scripts/check-consistency.sh
```

> **A config that passes every test can still be refused by the engine.** For
> anything touching generated mihomo YAML, run the real sidecar against the
> output: `verge-mihomo -t -f <config>`. Valid YAML is not a valid config — a
> proxy group sharing a name with a proxy inside it is a reference loop the engine
> rejects and no unit test sees.

## 6. Before you push

- **`main` and the tag are separate pushes**, and CI does not bump versions. See
  [`RELEASING.md`](RELEASING.md) before tagging anything.
- **A repo fix is inert until the piece that runs it is redeployed** — the hub
  runs its own copy of the hooks; `server/scripts/hooks-sync.sh` compares them.
- **Say what you did not verify.** Verified / structural / unverified are
  different claims, and this repository's documents are graded on that.

---

## The short version

If you already know the repo and want the minimum:

```sh
direnv allow
export LOCUS_SECRETS_PASSPHRASE_FILE=~/locus-passphrase   # only if deploying/signing
server/scripts/locus-key-extract.sh                        # only if signing
#  ...work...
server/scripts/locus-key-extract.sh --remove
```
