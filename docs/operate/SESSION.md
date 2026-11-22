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
git clone https://github.com/vasilliourous/Locus.git && cd Locus
direnv allow            # or: set -a; . ./.envrc; set +a
```

`.envrc` unsets `LD_LIBRARY_PATH` and `LD_PRELOAD`. This is not tidiness: a shell
that inherited a webkit2gtk **4.0** toolchain from somewhere makes `cargo test`
and `tauri dev` fail at **link** time with a symbol error that names neither
webkit nor the environment. The repo cannot fix it for you, because it belongs to
the parent process — so the file clears it instead of explaining it.

## 2. Install dependencies and stage the build inputs

```sh
(cd client && pnpm install)
(cd server/console && pnpm install)

# Stage the build inputs. OFFLINE — everything is committed under vendor/.
(cd client && pnpm run prebuild)

# Build the frontend once. cargo test needs ../dist to exist.
(cd client && pnpm web:build)
```

Those last two lines are not optional, and `cargo` will not run them for you.
Leave either out and a fresh clone fails in a way that names neither:

| Skipped | What you see | Why |
|---|---|---|
| `prebuild` | `resource path 'sidecar/clash-verge-service-x86_64-unknown-linux-gnu' doesn't exist` | `tauri.conf.json` declares `externalBin`, and `src-tauri/build.rs` requires each one to be present before it compiles. `prebuild` copies them out of `vendor/build-inputs/`. |
| `web:build` | `The 'frontendDist' configuration is set to "../dist" but this path doesn't exist` | `frontendDist` is `../dist`, produced by Vite. The Tauri build script validates it even when you are only running `cargo test`. |

Both were hit while verifying a fresh clone against this page, and both are cheap
to avoid once known — which is the whole reason this section exists.

`prebuild` needs **no network access** and needs **no secrets**: every input is
committed and pinned by sha256 in
[`../../server/data/sidecar.lock.json`](../../server/data/sidecar.lock.json). A pin
mismatch means the bytes on disk are wrong, not that the network is down — do not
delete the lock entry to get past it. See
[`VENDORED-BUILD-INPUTS.md`](VENDORED-BUILD-INPUTS.md).

> The privilege-service binaries in `sidecar/` are kept executable with
> `chmod +x` by `prebuild`. A copy of the tree that loses that bit fails at
> install time on the hub, not locally, so it is easy to miss.

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

## What a fresh clone can do, and with what

**The target state: delete this directory, `git clone`, supply the passphrase, and
keep developing.** Everything below was verified by cloning the repository into an
empty directory and running the workflow from this page, not reasoned about.

| Task | Needs passphrase? | Verified on a fresh clone |
|---|---|---|
| Read, search, edit code | No | — |
| `pnpm typecheck` / `lint` / `format:check` | No | ✅ all exit 0 |
| `pnpm test` (117 tests) | No | ✅ 13 files, 117 passed |
| `check-consistency.sh` (49 sections) | No | ✅ all passed |
| `cargo test --lib` (684 tests) | No | ✅ 684 passed — **after `prebuild` + `web:build`** |
| `cargo clippy` / `cargo fmt` | No | ✅ exit 0 |
| Build the app (`pnpm build`) | No | ✅ inputs are vendored |
| Extract the signing key | **Yes** | ✅ verified the key ID matches |
| Sign a release | **Yes** | ✅ once extracted |
| Deploy the hub | **Yes** | ✅ `setup.sh` decrypts the bundle |
| Seed / migrate hub data | **Yes** | ✅ same bundle |

**The only thing a clone cannot reproduce is the passphrase itself.** Every
credential, including the update-signing private key, is inside
`server/secrets.env.age`, which is committed. Nothing needs to be requested from
another person or another machine.

### What is *not* recoverable, and why it is correct

| Lost | Consequence | Recoverable? |
|---|---|---|
| The passphrase | Every secret, including the signing key. The ciphertext is public — the passphrase is its only protection. | **No.** Keep it in a password manager. |
| `.locus-keys/*` | Nothing. Re-extract. | ✅ by design |
| `.secrets.env` | Nothing. It should not exist. | ✅ by design |
| `client/dist/`, `src-tauri/sidecar/`, `node_modules/` | Nothing. All regenerated. | ✅ |
| `vendor/build-inputs/` | Would require the network — but it is **committed**, so this cannot happen from a clone. | ✅ |

> **Two secrets are deliberately *not* in the bundle**, and both are Actions-only:
> `GITHUB_TOKEN` (provided by Actions itself) and the repository secret
> `LOCUS_UPDATE_KEY`, which is a **copy** of the bundled key for CI to sign with.
> Actions secrets are write-only, so that copy can never be read back — the bundle
> is the only recoverable one. If a repo is copied or recreated, the Actions
> secret must be re-set from an extracted key
> ([`RELEASE-RECOVERY.md`](RELEASE-RECOVERY.md) §3).

---

## The short version

If you already know the repo and want the minimum:

```sh
direnv allow
(cd client && pnpm install && pnpm run prebuild && pnpm web:build)

# Only if deploying or signing:
export LOCUS_SECRETS_PASSPHRASE_FILE=~/locus-passphrase
server/scripts/locus-key-extract.sh        # only if signing
#  ...work...
server/scripts/locus-key-extract.sh --remove
```
