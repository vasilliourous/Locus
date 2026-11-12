# NetBird icon export

Icons extracted from the NetBird repository, deduplicated and renamed for reuse.

- **Source repo:** netbirdio/netbird (this checkout)
- **Export:** 96 files, ~1.3 MB
- All SVGs preserved verbatim as byte copies of the originals.
- `.icns` / `.ico` containers were unpacked into individual PNGs (see `platform/`).
- Icons embedded as `data:` URIs in HTML templates were decoded back to files.

## Layout

```
netbird-icons/
├── svg/                  Vector masters (start here)
├── appicon-1024.png      Master app icon, 1024x1024
├── logo-mark-400.png     Bird mark, 400x400
├── logo-full-704x119.png Glyph + wordmark lockup
├── app/                  Desktop agent assets
│   ├── tray/             System tray states, menu icons, status dots
│   ├── tray-darwin.png   430x370 UI screenshot/mock (macOS)
│   ├── tray-linux.png    430x370 UI screenshot/mock (Linux)
│   └── tray-windows.png  430x370 UI screenshot/mock (Windows)
├── platform/             Packaged OS icons, unpacked
│   ├── windows/          icon_<size>.png x6 (16–256)
│   └── macos/            ic07–ic14 x8 (32–1024); ic08/ic13 and ic09/ic14 are dupes
├── favicons/             Browser favicons (dex IdP, proxy, auth page)
└── idp-providers/        13 identity-provider brand icons (third-party)
```

## `svg/` — vector masters

| File | Size | Description |
|---|---|---|
| `netbird-mark.svg` | 31x23 | Bird glyph only, no wordmark |
| `netbird-full.svg` | 133x23 | Glyph + "netbird" wordmark, dark theme |
| `netbird-full-light.svg` | 133x23 | Same lockup, light theme |
| `netbird-menu-32.svg` | 32x32 | Square menu logo |
| `netbird-needs-login-32.svg` | 32x32 | Same mark at 0.7 opacity |
| `appicon-1024.svg` | 1024x1024 | macOS Icon Composer source |

Brand orange is `#F68330`.

For most uses pick `netbird-full.svg` (lockup) or `netbird-mark.svg` (glyph).

## `app/tray/` — naming scheme

Filenames follow `<kind>-<state>[-<variant>].png`.

- `app-256.png` — full app icon, 256x256
- `tray-<state>[-dark|-macos|-mono|-mono-dark].png`
  - base / `-dark` — 256x256, colored
  - `-macos` — 366x366, grayscale+alpha (macOS template image style)
  - `-mono` / `-mono-dark` — 48x48, monochrome
- States: `connected`, `connecting`, `disconnected`, `error`, `needs-login`,
  `update-connected`, `update-disconnected`
  - `disconnected` has no colored 256px base (only `-macos` / `-mono*`)
  - `needs-login` likewise
- `dot-<state>[-16|-22].png` — status dots
  - states: `connected`, `connecting`, `error`, `idle`, `offline`
  - plain name = 24x24; `-16` = 16x16; `-22` = 22x22
- `menu-16.png`, `menu-24.png`, `menu-about-18.png` — menu glyphs

## `platform/`

Unpacked from `client/ui/build/`.

- `windows/icon_<N>x<N>.png` — 6 sizes: 16, 32, 48, 64, 128, 256
- `macos/ic<type>_<N>px.png` — 8 entries: 32, 64, 128, 256, 512, 1024
  - `ic08` (256) duplicates `ic13` (256)
  - `ic09` (512) duplicates `ic14` (512)

To rebuild the containers, use `png2icns`/`icnsutil` and `png2ico`.

## `favicons/`

| File | Source |
|---|---|
| `dex-favicon.ico` | `idp/dex/web/themes/light/favicon.ico` (6 sizes to 256) |
| `dex-favicon-16.png` | `idp/dex/web/themes/light/favicon.png` |
| `dex-logo-16.png` | `idp/dex/web/themes/light/logo.png` |
| `proxy-favicon.ico` | `proxy/web/src/assets/favicon.ico` (3 sizes) |
| `pkce-auth-favicon.ico` | decoded from `client/internal/templates/pkce-auth-msg.html` |

## `idp-providers/`

Decoded from `data:image/svg+xml` URIs in `idp/dex/web/templates/header.html`.

`google`, `github`, `microsoft`, `azure`, `entra`, `okta`, `jumpcloud`,
`pocketid`, `zitadel`, `authentik`, `keycloak`, `email`, `default`.

**These are third-party brand logos**, not NetBird assets. Each is subject to its
own owner's trademark/brand guidelines — do not redistribute them as NetBird
iconography.

## License note

NetBird's own marks come from the NetBird repository and are covered by the
project's licensing/terms. Third-party provider logos in `idp-providers/`
belong to their respective owners.
