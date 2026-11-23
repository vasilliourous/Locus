# Contributing — Chrysalis client

```
audience:    builder
status:      live
authoritative-for: how to set up and submit a change to the Chrysalis client
verified-against: client/package.json, client/scripts/, client/docs/
```

> **Front-matter added 2026-10-05.** This file was an orphan: nothing referenced
> it — not the root `README.md`, not `client/AGENTS.md`, not the documentation
> index — so nothing checked its links either. It is now indexed from
> [`docs/README.md`](../docs/README.md) and covered by the link and reachability
> guards.

This directory is a **fork of Clash Verge Rev v2.5.5**, adapted into the Chrysalis
desktop client. It is the shipping Chrysalis client. The predecessor Wails client is
archived at `legacy/wails-client/` and is not the product.

Agent-facing rules live in [`AGENTS.md`](AGENTS.md); read that first if you are
an AI agent. This file is the human-facing setup and submission guide.

## Scope

This directory is **GPL-3.0-only**, inherited from its Clash Verge Rev v2.5.5
fork base and recorded in `client/LICENSE`. It is **not** covered by the root
`CC BY-NC-ND 4.0` licence, which carves `client/` out explicitly — a GPL fork
cannot be relicensed. The identifier is kept in step in three places
(`client/package.json`, `client/src-tauri/Cargo.toml`, and the root `LICENSE`
scope table); `server/scripts/check-consistency.sh` §11 fails if they disagree.

Upstream's contribution process (issue-first gating, the `ai-slop` review
workflow, signed-commit requirements) does **not** apply here — those files were
removed from the fork. Practically, that means:

- No issue-first requirement. Describe the problem in the commit message.
- No automated PR screening, and no required commit signing.
- Keep diffs minimal and mapped to a real problem. No drive-by refactors,
  renames, formatting churn, or dependency bumps.
- Do not pad with tests or speculative defensive code. Add a test when it
  reproduces a real regression or guards behaviour whose breakage would
  otherwise go unnoticed, and say why in the commit.

## Internationalization (i18n)

For contributing translations, see [docs/CONTRIBUTING_i18n.md](docs/CONTRIBUTING_i18n.md).

> **Note:** the fork still contains inherited `Clash Verge` strings, mostly in
> `src/locales/`. Several name the privileged system service in user-facing error
> messages. Do not blind find-replace them — a message a student cannot act on is
> worse than a stale brand name. See `client/docs/UPSTREAM-CHANGES.md` §4 and
> `client/docs/ARCHITECTURE.md`.

## Development Setup

Before contributing, you need to set up your development environment. Follow the steps below carefully.

### Prerequisites

1. **Install Rust and Node.js**  
   Our project requires both Rust and Node.js. Follow the official installation instructions [here](https://tauri.app/start/prerequisites/).

### Windows Users

> [!NOTE]  
> **Windows ARM users must also install [LLVM](https://github.com/llvm/llvm-project/releases) (including clang) and set the corresponding environment variables.**  
> The `ring` crate depends on `clang` when building on Windows ARM.

Additional steps for Windows:

- Ensure Rust and Node.js are added to your system `PATH`.

- Install the GNU `patch` tool.

- Use the MSVC toolchain for Rust:

```bash
rustup target add x86_64-pc-windows-msvc
rustup set default-host x86_64-pc-windows-msvc
```

### Install Node.js Package Manager

Enable `corepack`:

```bash
corepack enable
```

### Install Project Dependencies

Node.js dependencies:

```bash
pnpm install
```

Ubuntu-only system packages:

```bash
sudo apt-get install -y libxslt1.1 libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```

### Prepare the Build Inputs (Automatic, Offline)

```bash
pnpm run prebuild
pnpm run prebuild --force  # Re-verify every input, replacing anything that does not match its pin
```

**All twelve build inputs are committed** under `vendor/build-inputs/` — the two
mihomo engines, the privilege-service bundle and the four geo databases — so
`prebuild` copies them into `src-tauri/sidecar/` and `src-tauri/resources/` and
verifies each against its sha256 in `server/data/sidecar.lock.json`. **It does not
need the network.** If a pin check fails, do not delete the lock entry to get past
it — see [`../docs/operate/VENDORED-BUILD-INPUTS.md`](../docs/operate/VENDORED-BUILD-INPUTS.md),
which also covers where a new vendored file goes and how to prove the build reads
the committed copy rather than the network.

> **Run this before `cargo test`, not just before `pnpm build`.** The Rust build
> script requires `src-tauri/sidecar/<service-binary>` to exist, and `cargo` does
> not invoke `prebuild` — so a fresh clone fails with
> `resource path 'sidecar/clash-verge-service-…' doesn't exist`. See
> [`../docs/operate/SESSION.md`](../docs/operate/SESSION.md) §3 for the full
> ordering, including the `dist/` requirement below.

### Run the Development Server

```bash
pnpm dev           # Standard
pnpm dev:diff      # If an app instance already exists
pnpm dev:tauri     # Run Tauri development mode
```

### Build the Project

Standard build:

```bash
pnpm build
```

Fast build for testing:

```bash
pnpm build:fast
```

### Clean Build

```bash
cargo clean --manifest-path src-tauri/Cargo.toml && rm -rf dist
```

### Portable Version (Windows Only)

```bash
pnpm build -- --no-bundle
```


## Contributing Your Changes

### Before Committing

**Code quality checks:**

```bash
# Rust — four gates, and passing tests says nothing about the other three
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --features clippy -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
bash ../scripts/check-rust-deps.sh     # the fourth, and it is a CARGO gate
# Frontend
pnpm lint
```

> **`check-rust-deps.sh` is not redundant with `-D warnings`.** It fails on a
> `[dependency]` that nothing uses. Clippy prints that warning and exits **0**,
> because `cargo::unused_dependencies` is a *cargo* lint and `-D warnings` is a
> *rustc* flag — so the class is invisible to every compiler gate. Run all four
> before pushing Rust; this repository has already paid for the discovery that
> three of them can be green.

**Code formatting:**

```bash
# Rust backend
cargo fmt --manifest-path src-tauri/Cargo.toml --check
# Frontend
pnpm format:check
```

> `pnpm lint` is eslint over `src/` and does **not** format `scripts/*.mjs`.
> `pnpm format:check` (biome) is the gate that covers them, and the `lint-staged`
> hook runs `biome format --write` on the same files at commit time — so a script
> that fails `format:check` gets rewritten silently as part of an unrelated diff.
> Run it before committing a script change.

### Submitting Your Changes

This is a private, single-maintainer repository — there is no public fork-and-PR
process. Branch, commit with a clear message, and push. The release path is
[`../docs/operate/RELEASING.md`](../docs/operate/RELEASING.md).
