# Repo restructure: separating client, server, and legacy

```
audience:    builder
status:      design-record
authoritative-for: the current repo layout and the names that must not be trusted from before the 2026-09 move
verified-against: the tree (top-level), server/scripts/check-consistency.sh §8
```

The 2026-09 move put the shipping client in `client/`, the hub in `server/`, and
everything retired in `legacy/`. `git mv` throughout, so history follows — use
`git log --follow` rather than trusting an old path in an old message.

## The layout, and what is not there

```
client/            the fork (Clash Verge Rev base); product logic in src-tauri/src/locus/
server/            the hub: modules/, pb_hooks/, scripts/, console/, site/
docs/              this set — index at ../docs/README.md
legacy/            v4/ and wails-client/ — reference only, never built
scripts/           the code generator and the card printer
.github/workflows/client.yml    the only CI, at the repo root
```

**Never created, and not worth looking for:** `shared/contracts/`, `vendor/`,
`packages/` inside `client/`, and any root `VERSION` file. A workflow placed under
`client/.github/` is ignored by GitHub — that is why CI lives at the repo root.

## Why `legacy/wails-client/` still exists

Its Go tests encode contract behaviour the fork must reproduce — Luhn, version
comparison, the `uot_port` rename, artifact names, grace arithmetic — and
[`LOGIC-INVENTORY.md`](LOGIC-INVENTORY.md) lists the specific tests. Deleting it
before those are ported and covered by the fork's own tests loses the specification.
Once a behaviour is covered in `client/src-tauri/tests/`, the legacy copy stops
being load-bearing.

## The rule this move left behind

**A path that a code comment or a document names is a reference a reader follows.**
The move left two comments pointing at a file that had gone to `docs/archive/`,
and nothing noticed for a year — `check-consistency.sh` §8 now scans the source
trees for exactly that class, with the same regex as the markdown pass. When you
move a file, grep for its old name across `client/src`, `client/src-tauri/src`,
`server`, `.github` and `scripts`, not just `docs/`.
