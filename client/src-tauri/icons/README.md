# The application and tray marks

```
audience:    builder
status:      live
authoritative-for: which icon file is generated, from what, and which one still needs a Mac
verified-against: client/scripts/generate-icons.mjs, client/src-tauri/tauri.conf.json, server/scripts/check-consistency.sh §54
```

**Every mark in this directory is the FlClash glyph.** The glyph is
`client/src/assets/image/glyph.svg` — two bars and a dot on a 30° lattice — copied
from FlClash's own vector master and unmodified. It carries its own three
gradients (top bar, middle bar, dot) and those are the artwork, not a palette: they
are drawn unchanged on every surface. The one colour this directory takes from the
palette is the app icon's rounded-square ground, which is `background` out of
`client/src/pages/_theme.tsx`.

## What replaced what

| Was | Is | When |
|---|---|---|
| Clash Verge Rev cat (white on grey) | the FlClash glyph on the palette's ground | every raster mark, 2026-11-23 |
| Clash Verge status discs (green / orange / blue) | FlClash's four `status_*` state assets | tray, 2026-11-23 |
| Clash Verge cat in Clash Verge green | the glyph on the same ground | `server/site/favicon.ico` and the browser tab |
| `clash-verge.icon/` (cat SVG + Icon Composer bundle) | deleted | 2026-11-23 |
| `locus-icon-dropin/` (a lossy partial copy of the glyph, with tray files whose colours had been flattened) | deleted; the real master is in-repo | 2026-11-23 |

The cat shipped in the window, the taskbar, the dock, the tray and both favicons
for every release up to and including 3.4.4.

## Generated, and checkable

`node scripts/generate-icons.mjs` (from `client/`) rewrites every file below.
`node scripts/generate-icons.mjs --check` regenerates them **in memory** and fails if
anything on disk disagrees — which is both "someone hand-edited an icon" and "nobody
re-ran the generator". `pnpm icons` / `pnpm icons:check` are the same two commands.

`server/scripts/check-consistency.sh` §54 runs that `--check` itself, so a stale
mark fails the consistency gate rather than waiting for someone to remember. It
needs `node` and `rsvg-convert`; without them it prints a named `WARN` instead of
passing silently.

| File | Frames | Purpose |
|---|---|---|
| `icon.png` | 512 | the source most tooling reads |
| `32x32.png`, `128x128.png`, `128x128@2x.png` | 32, 128, 256 | `bundle.icon` |
| `icon.ico` | 8 (16–256) | Windows exe, window, taskbar, `nsis.installerIcon` |
| `icon.icns` | 8 (32–1024) | macOS Dock/Finder — generated here, see below |
| `Square*Logo.png`, `StoreLogo.png` | 30–310 | Windows Store / MSIX tile set |

The tray is a **state × platform matrix**, matching the three directories FlClash
ships. A single `.ico` per state would load everywhere but be wrong on two
platforms, so each host gets the format and ladder it actually reads:

| Output | Host | State → upstream asset |
|---|---|---|
| `tray-icon.ico`, `tray-icon-sys.ico`, `tray-icon-tun.ico`, `tray-icon-safe.ico` | Windows | `1`, `2`, `3`, `4`; 8 frames each |
| `tray-icon{,-sys,-tun,-safe}.png` and `@2x`/`@3x`/`@4x` | Linux | `1`, `2`, `3`, `4`; 18/36/54/72 px |
| `tray-icon-mono.png` and `@2x`/`@3x`/`@4x` | macOS template | `1` only — see below |
| `tray-icon-safe-mono.png` and `@2x`/`@3x`/`@4x` | macOS template | `4` only |

**Why macOS has two template states and not four.** A template image is tinted by
the OS from its alpha channel, which discards the colour that distinguishes
states 2 and 3. FlClash's launcher reflects that (`lib/common/tray.dart`: the
second `switch` arm is `_isTemplate(monochrome) || !isStart => 1`), and its
`macos/` directory ships `status_1` and `status_4` only. Two is the complete set,
not a truncation — inventing templated 2/3 files would be inventing assets
upstream does not have.

The templates are **black** on alpha, matching upstream's own macOS PNGs. White is
the other convention and looks identical on screen, because the OS discards the
RGB either way; black is chosen so this repo's asset and the one it came from are
the same picture.

**The scale ladder is selected, not decorative.** Tauri's `Image` carries one RGBA
bitmap with no per-scale variants, so `core/tray/mod.rs` picks the ladder step
from `GDK_SCALE` on Linux and from the backing scale on macOS, and hands the host
an already-sharp bitmap. Windows needs none of this: it gets the `.ico`, and the
`image` crate's `best_entry` selects the largest frame inside it.

`client/src/assets/image/logo.ico` (the favicon `src/index.html` points at) and
`server/site/favicon.ico` (the landing page's) are generated from the same source
by the same script. The landing one is in the table because it lives outside the
client entirely and was the last place the cat survived.

**The tray is FlClash's own state set, not a recolour.** FlClash draws four
`status_*` assets and picks between them by run state
(`lib/common/tray.dart`): `1` idle or macOS-templated, `2` running with TUN off,
`3` running with TUN on, `4` safe mode. States 1–3 share a silhouette and differ
only by colour; state 4 is a different shape. The assets live in
`client/src/assets/image/mark/` and are copied verbatim.

Our tray has three `IconKind`s, so the mapping is `Common → 1`, `SysProxy → 2`,
`Tun → 3`. **State 4 has no consumer**: this client has no safe-mode branch, and
`status_4.svg` is committed anyway because it is part of the set that was adopted —
if a safe mode is added, the asset is already there.

**Each size is rendered at its own resolution** from the vector, never downscaled
from a 512 master: a single master scaled to 16 px erodes the margin and softens
the two bars into one mass. `insetFor()` grows the ground as the canvas shrinks
for the same reason. The app icon uses FlClash's own framing, measured from its
1024 master: the tile is 84.8 % of the canvas with a 22.4 % corner radius, and the
ink is 72 % of the tile.

## `icon.icns`, and the catalog that is gone

`icon.icns` is the one mark the generator script does not write, because Node has
no `iconutil`. It is **generated from `glyph.svg`** all the same, by
`.github/workflows/generate-macos-icons.yml` on a macOS runner, and the artifact is
committed by hand. Its eight frames (`ic07`–`ic14`, 32–1024 px) were rendered from
the same vector, so a Mac and a PC show the same mark.

**`Assets.car` was removed from `bundle.icon` on 2026-11-23 and deleted.** It was
1.9 MB of compiled CoreUI art holding the previous mark; only Apple's `actool` can
author one, and — this is the part that mattered — a catalog **takes precedence
over `icon.icns`** inside a bundle. Leaving a stale one would have made the
regenerated `.icns` irrelevant: the Dock would still draw the cat while every file
in this directory looked correct. Removing it means macOS resolves the icon from
`icon.icns`, which is the normal arrangement for a Tauri app distributed outside
the App Store.

§54 asserts that `icon.icns` exists and that nothing in this directory is a file
the generator does not write; it cannot verify an `.icns`'s bytes. If a future
change wants a catalog back, it has to be regenerated by `actool` in the same
change — the guard now fails on a stray `.car` rather than letting it through.

## The trap this directory has already paid for

An icon is not code: nothing fails when it is wrong. A stale mark compiles, signs,
publishes and installs, and the only symptom is a student looking at another
program's logo. That is why `--check` compares **bytes** rather than trusting that
somebody re-ran a generator, and why §54 runs it: before that, the landing page's
favicon was still the green cat a full migration after the app had moved on,
because every other path it checked lived under `client/`.
