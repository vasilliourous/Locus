# Locus Scripts

Utility scripts for Locus code generation, printing, and operations.

## Files

| Script | Purpose |
|--------|---------|
| `generate_codes.sh` | Generate Luhn-mod-N activation codes and import to PocketBase |
| `print_codes.sh`    | Format codes into printable PDF card sheets |
| `vps-test/`         | Read-only network probes run against the live VPS |

**Publishing** lives under `server/scripts/`: `publish-release.sh` (the correct
publish path) and `fetch-release.py` (the GitHub-fetch path).

> **Removed:** `release-cut.sh`, root `bump.sh`, `server/scripts/bump-version.sh`,
> `stamp-syso.py`, and `smoke-bump.sh`. They versioned the **archived** Wails
> client and were the source of the repo's tagging confusion. The shipping fork
> has its own version authority now: `pnpm release-version` in `client/` writes
> every version site (the list is `[client.version_sites]` in `docs/state.toml`),
> and `docs/operate/RELEASING.md` is the procedure.

> **Retired:** `publish-update.sh` was moved to
> `legacy/publish-update.sh.broken`. It wrote no `sha256_<platform>` columns and
> its macOS URLs did not match CI's filenames. Use `publish-release.sh`.

## Quick Start

### Generate Codes

```bash
# Generate 50 free-plan codes and import to PocketBase
PB_TOKEN=$(grep PB_TOKEN /root/.pb_admin_creds | cut -d= -f2)
./scripts/generate_codes.sh https://networkingguides.duckdns.org $PB_TOKEN free 50

# Paid plan — the row is `strike`, the card prints "Full"
./scripts/generate_codes.sh https://networkingguides.duckdns.org $PB_TOKEN strike 20

# Dry run (no import)
DRY_RUN=1 PB_TOKEN=$(grep PB_TOKEN /root/.pb_admin_creds | cut -d= -f2)
./scripts/generate_codes.sh https://networkingguides.duckdns.org $PB_TOKEN free 50

# Custom expiry (30 days)
EXPIRY_DAYS=30 PB_TOKEN=$(grep PB_TOKEN /root/.pb_admin_creds | cut -d= -f2)
./scripts/generate_codes.sh https://networkingguides.duckdns.org $PB_TOKEN free 10
```

### Print Code Cards

```bash
# Generate PDF from codes file
./scripts/print_codes.sh free-codes.txt free-cards.pdf

# Pipe codes directly
cat free-codes.txt | ./scripts/print_codes.sh -o free-cards.pdf
```

## Code Format

```
RQ-XXXX-XXXX-XXXX-C
```

- **RQ**: Static prefix
- **XXXX**: Random base-32 segments (charset: `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`, no I/O/0/1)
- **C**: Luhn-mod-N checksum character (computed over the full body including the prefix)

The Luhn-mod-N checksum is validated client-side before sending to server,
preventing mistakes (typos) and reducing server load from invalid codes.

## Dependencies

- **generate_codes.sh**: bash, curl, python3 (for API import, optional)
- **print_codes.sh**: enscript + ghostscript (for PDF), or pandoc + wkhtmltopdf
