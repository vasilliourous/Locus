# Locus Server Deployment Guide

```
audience:    human-operator
status:      live
authoritative-for: deploying the hub to a blank VPS
verified-against: server/setup.sh, server/modules/
```

> Deploy the complete Locus server infrastructure on a blank Ubuntu 22.04 VPS.

> **Rebrand note:** The product is now **Locus**, but the live VPS and every
> deployment script under `server/` still name their installed artifacts
> **`myvpn-*`** (`/etc/myvpn`, `/usr/local/bin/myvpn-*.sh`, `/var/log/myvpn-*.log`,
> `/root/…`, systemd units). Those literal `myvpn` paths remain **correct as
> written** until the server is redeployed/renamed — do not "fix" them to `locus`.

---

## 1. Prerequisites

- **VPS:** Ubuntu 22.04, x86_64. **512MB RAM works** — the live host runs 454MB
  with 1GB swap, but that is the floor; if you can afford 1–2GB it will be more
  comfortable. 10GB disk minimum.
- **Domain:** A domain name pointing to your VPS IP (A record)
- **Backblaze B2 account** (optional, for backups)
- **Age encryption key** (see `SECRETS-MANAGEMENT.md` for one-time setup)

> **Memory gotcha:** `00-env.sh` has a memory guard. It previously compared
> against 512 **MiB**, which made a "512MB" droplet (reports 454MB) impossible to
> pass — fixed 2026-09-19. If the guard fails on a small box, check that fix is
> present before assuming the host is too small.

### Recommended VPS Providers

| Provider | Plan | Spec | Cost |
|----------|------|------|------|
| DigitalOcean (**currently live**) | Basic | 1 vCPU, 512MB + 1GB swap | ~$6/mo |
| Hetzner | CCX13 | 2 vCPU, 4GB RAM | ~$8/mo |
| Vultr | Regular Cloud | 1 vCPU, 2GB RAM | ~$12/mo |

---

## 2. DNS Setup

Create an A record for your VPS IP:

```
networkingguides.jadedns.uk  A  YOUR_VPS_IP
```

TTL of 60s recommended during initial setup. Increase to 300s+ after stable.

### Optional: a second hub hostname, for filtered networks

**Added 2026-10.** A school network can refuse the hub hostname outright. When it
does, it takes down activation, heartbeat **and** the update path together — they
are all served from that one name — so the device cannot fetch the build that
would relieve it.

> **There are now TWO second-name mechanisms, and confusing them is an outage.**
> `LEGACY_DOMAIN` (below) is **served** and **on by default**; `HUB_ALIAS_DOMAIN`
> (this section) is a **redirect** and **opt-in**. A redirect is useless for a name
> whose certificate is the thing that broke, so the two cannot be swapped.
>
> ### `LEGACY_DOMAIN` — the previous hub name, served on the same block
>
> Set automatically to `networkingguides.duckdns.org` when `DOMAIN` moves. Caddy
> puts both names on one site block, so there is one set of handles, one rate-limit
> zone and one admin check. It is **not optional while any pre-3.3.2 build is in
> the field**: those clients have the old name compiled in and the heartbeat — the
> only channel that could tell them otherwise — needs a working hub to reach. Set
> `LEGACY_DOMAIN=""` only once no earlier build remains.
>
> `check-consistency.sh` **§32** asserts the block is addressed by
> `${HUB_BLOCK_NAMES}` and that the alias block did not swallow the legacy name.

There are two halves, and either alone does nothing:

1. **The server must answer on a second name.**
   ```sh
   # On the box, or passed through setup.sh's environment:
   HUB_ALIAS_DOMAIN=vpn.<your-domain>      # a name you control
   ```
   `modules/05-caddy.sh` appends a site block that `308`s every request to the
   same path on `DOMAIN`. It redirects rather than repeating the handles, so the
   rate-limit zones and the admin token check cannot drift between the two names.
   Point the name's DNS at the same box first — Caddy obtains the certificate on
   first request, and a name that does not resolve fails loudly at that point.
   **A redirect requires the alias to present its own valid certificate first**,
   so this mechanism helps only when the alias name's TLS is sound; it cannot
   rescue a fleet whose *alias* certificate has expired.

2. **The client must be told to use it.**
   ```sh
   # Console → Tiers → "Hub address to offer"
   https://vpn.<your-domain>
   ```
   The value rides the heartbeat, so an installed device picks it up on its next
   beat and dials it from the *next launch*. The compiled default keeps working
   for everyone else, so nothing breaks if this is blank.

**What this does not fix.** The classification itself. A dynamic-DNS subdomain is
structurally likely to be judged uncategorised, and the durable answer is a name
with a reputation that can be recategorised on request. If a second name on the
same domain family is still blocked, the problem is the family, not the name.

**Verify it from the network that was failing**, not from anywhere else:

```sh
curl -sS https://vpn.<your-domain>/api/release | head -c 120
```

A redirect to the canonical hub is what success looks like. See
[`../reference/STILL-OPEN.md`](../reference/STILL-OPEN.md) — "the hub hostname is
blocked on a school network" — for why this is a world claim and not a code fact.

---

## 3. Secrets Setup (One-Time)

Before your first deploy, create an age-encrypted secrets file. This eliminates
the need to manually pass environment variables on every deploy.

**See the full guide:** [`SECRETS-MANAGEMENT.md`](SECRETS-MANAGEMENT.md)

Quick summary:

```bash
# 1. Install age
sudo apt-get install -y age

# 2. Choose a passphrase (there is no key pair any more)
openssl rand -base64 24    # store it in your password manager

# 3. Create .secrets.env with all credentials:
#    DOMAIN, ADMIN_API_TOKEN, B2_APPLICATION_KEY_ID/KEY, B2_BUCKET,
#    ECO_PASS, STRIKE_PASS, STEALTH_PASS (retired port, still validated),
#    PB_ADMIN_EMAIL, PB_ADMIN_PASS, RELEASE_FETCH_SECRET
#    (See SECRETS-MANAGEMENT.md for the full template)

# 4. Encrypt it — the script verifies its own round trip
LOCUS_SECRETS_PASSPHRASE_FILE=~/passphrase.txt \
  server/scripts/secrets-encrypt.sh .secrets.env
shred -u .secrets.env

# 5. Commit the encrypted file
git add server/secrets.env.age
```

Once this is done, **all credentials are auto-injected at deploy time** by
decrypting `secrets.env.age` with the passphrase. You never type the individual
credentials again.

---

## 4. Deploy

### Staging the server tree (read this before Option A or B)

`setup.sh` reads **two things that a bare `scp -r server` does not put
there for you**. The first is a hard failure; the second already ships in the
repo, but is worth knowing about because module 05 refuses to deploy without
it:

| Path on the VPS | Required by | Produce it with |
|---|---|---|
| `/root/server/console-dist.tar.gz` | `05-caddy.sh` → `deploy_console()` | `server/scripts/deploy-console.sh` (builds the SPA with npm, uploads the tarball) |
| `/root/server/site-dist.tar.gz` | `05-caddy.sh` → `deploy_site()` | `server/scripts/deploy-site.sh` (renders the download URLs from the release manifest, uploads the tarball) |
| `/root/server/scripts/fetch-release.py` | `05-caddy.sh` → `install_fetch_service()` | already in the repo — it arrives with `scp -r server` |

So on a **fresh** host the order is:

```bash
# 1. Stage the server tree (fetch-release.py comes along; the bundles do not)
#    NOTE: no key file any more — the deploy scripts read a passphrase.
scp -r server root@your-vps:/root/server/

# 2. Build + upload the console bundle to /root/server/console-dist.tar.gz
VPS=root@your-vps DOMAIN=hub.example.com \
  server/scripts/deploy-console.sh

# 3. Render + upload the landing page to /root/server/site-dist.tar.gz
VPS=root@your-vps \
  server/scripts/deploy-site.sh

# 4. Deploy
ssh root@your-vps "/root/server/setup.sh"
```

`deploy-console.sh` needs `npm` on **your** machine (not the VPS — the VPS has
no Node and the console source is not shipped to it). Both `VPS` and `DOMAIN`
MUST be set explicitly: the script has no usable default for a new host.

> **On a fresh host, `deploy-console.sh` may report `HTTP 000` and stop — that
> is expected, not a failure.** Its final check fetches `https://$DOMAIN/admin/`,
> but on a blank box Caddy is not serving the domain until step 3 installs it.
> The script now treats a non-200 there as a warning and exits 0 once the bundle
> is safely staged at `/root/server/console-dist.tar.gz`; the console goes live
> during `setup.sh`. If you are running it against a host whose hub is **already
> up**, a non-200 is a real fault and should be investigated.

The same nuance applies to `deploy-site.sh`'s final check on the landing
hostname, and for the same reason.

### The landing page (`site-dist.tar.gz`)

The public landing page lives on a **second hostname** on the same box —
`[site.domain]` in [`../state.toml`](../state.toml), currently
`locusvpn.jadedns.uk`. It is the one hostname here that is meant to be indexed;
the hub keeps its own name because that name is baked into every deployed client.

| Piece | Where |
|---|---|
| Source | `server/site/` — one page, no build step |
| Renderer | `server/scripts/deploy-site.sh` |
| Host-side extractor | `05-caddy.sh` → `deploy_site()` |
| Document root | `/var/www/site/` (`[site.document_root]`) |
| Caddy block | the second site block in `05-caddy.sh` (`LANDING_DOMAIN`) |

**The download URLs are rendered, never hand-written.** The four buttons point
at the hub's own `/updates/<version>/<file>` path, and the version comes from the
release manifest over the same `platforms` map that `publish-release.sh` and
`fetch-release.py` resolve against. That makes the buttons a **derived fact**:
editing one by hand would leave the page offering a version the hub does not
serve, and nothing would fail — a stale link looks exactly like a correct one
until someone clicks it. Section 26 of `check-consistency.sh` guards the
agreement.

> **Why the buttons point at the hub and not at GitHub.** GitHub only serves a
> release asset from a URL on the repository
> (`/releases/download/<tag>/<file>`, or `/releases/latest/download/<file>`), so
> a link to a GitHub asset lands the visitor on the repo's download page. The hub
> already serves the identical files for the in-app updater, so the page links
> there instead and the visitor gets the file directly.

**To disable the landing page on a host**, set `LANDING_DOMAIN=` (empty). That
skips both the Caddy block and the bundle requirement — the two are gated
together on purpose, because a block with no page and a page with no block are
both half-deployed states that look finished.

**Hand-packing the bundle (only if you are not using `deploy-console.sh`).**
The tarball must contain the **contents** of `server/console/dist/`, with
`index.html` at the archive root — *not* the `dist/` directory itself. The
one-character difference is the whole bug:

```bash
# RIGHT — index.html at archive root
cd server/console && npm run build
tar czf /tmp/console-dist.tar.gz -C dist .

# WRONG — yields ./dist/index.html, and 05-caddy.sh fails with
# "Console bundle has no index.html"
tar czf /tmp/console-dist.tar.gz -C server/console dist
```

Then upload it: `scp /tmp/console-dist.tar.gz root@your-vps:/root/server/`.

If you cannot build the console, deploy with `SKIP_CONSOLE=1` — the hub, the
tiers and the API all come up without it, and you can add the console later by
running `deploy-console.sh` and re-running `setup.sh` (module 05 extracts the
bundle if it is present).

### Routine changes to a running hub: `deploy.sh`

`setup.sh` is the blank-box path. For changes to an **already-running** hub, use
`server/scripts/deploy.sh` instead — one command for hooks + console + landing
page + staging + verify, **idempotent and diff-based** (unchanged hooks are not
uploaded and nothing is restarted unless something changed). It never writes a
record, so deploying code cannot withdraw a release or reset a rollout.

```bash
server/scripts/deploy.sh              # hooks + console + site + staging + verify
server/scripts/deploy.sh --check      # dry run, changes nothing
server/scripts/deploy.sh --hooks      # hooks only
server/scripts/deploy.sh --console    # console only
server/scripts/deploy.sh --site       # landing page only
server/scripts/deploy.sh --no-site    # everything except the landing page
```

It relies on the installed SSH key (see below); it probes for the `locus-hub`
alias and falls back to `root@<domain>`. Full step table and the staging/live
drift check are in [`OPS.md`](OPS.md) → "One-command deploy".

### SSH access to the hub

Key auth is installed, so nothing prompts:

```bash
ssh locus-hub 'uptime'
```

- The deploy key is `.deploy/locus_deploy` — **passphrase-free**, gitignored,
  installed in `/root/.ssh/authorized_keys`.
- `~/.ssh/config` defines `Host locus-hub` (HostName = the hub domain, User
  root, IdentityFile the deploy key).
- This dedicated key exists because the machine's only other usable private key
  was passphrase-protected (so `ssh` invoked `ssh-askpass`, which is not
  installed) and the `.pub` beside it did not even match — so no tooling could
  authenticate non-interactively. See [`OPS.md`](OPS.md) for the askpass
  fallback and the pre-flight deadman-switch technique.

### Option A: Local Machine with Passphrase File (Recommended)

```bash
# Copy server code, then the passphrase to a temp path you delete afterwards
scp -r server root@your-vps:/root/server/
scp passphrase.txt root@your-vps:/root/.pw

# One-command deploy — secrets auto-decrypt from the passphrase file
ssh root@your-vps "chmod 600 /root/.pw && \
  LOCUS_SECRETS_PASSPHRASE_FILE=/root/.pw /root/server/setup.sh && \
  rm -f /root/.pw"
```

`LOCUS_SECRETS_PASSPHRASE_FILE` is preferred over passing the passphrase inline:
it keeps it out of your shell history and out of `ps`.

`DOMAIN`, `ADMIN_API_TOKEN`, B2 creds, tier passwords, and PB admin credentials
are all decrypted from `secrets.env.age` automatically.

### Option B: Pipe with the Passphrase Inline

```bash
LOCUS_SECRETS_PASSPHRASE='...' ssh root@your-vps 'bash -s' < server/setup.sh
```

> ⚠️ An inline passphrase is visible to anything that can read your shell history
> or the process table. Use it for one-off deploys, not in a shared terminal.

### Option C: Manual Env Vars (No Secrets File)

If you haven't set up the age-encrypted secrets file yet:

```bash
DOMAIN=networkingguides.jadedns.uk \
ADMIN_API_TOKEN=your-token \
B2_APPLICATION_KEY_ID=xxx \
B2_APPLICATION_KEY=xxx \
B2_BUCKET=my-vpn-backup-bucket \
ssh root@your-vps 'bash -s' < server/setup.sh
```

The setup script does **everything** automatically:
1. Validates environment (OS, arch, root, disk, memory, DNS)
2. Enables BBR + TCP kernel tuning
3. Installs 2 ssserver instances (eco:8443 the free plan, strike:8445 the paid one)
4. Applies tc traffic shaping (Free 1 Mbps, Full 100 Mbps,
   each with an `fq_codel` leaf qdisc to keep latency flat under load)
5. Installs **sing-box UDP-over-TCP on :8446** for the paid tier
   (open the firewall for it too; `ENABLE_UOT=0` opts out)
6. Installs Caddy with rate limiting, Let's Encrypt TLS, `/admin/` and `/updates/`
7. Installs PocketBase with JS hooks and SQLite WAL mode, **creates the admin,
   the collections, the schema and the tier configs** (a failure here is fatal —
   it used to be a warning, which made a hub serving 500s look deployed)
8. Deploys the **admin console** to `/admin/` (required, not optional — a
   missing bundle fails the deploy; see "Staging the server tree" below)
   and installs the **release fetch service** (`locus-fetch` on
   `127.0.0.1:8091`, required — it is what the console's Releases page
   calls to pull a GitHub Release onto the hub)
9. Configures hourly B2 backup systemd timer
10. Sets up UFW firewall (including 8446 TCP+UDP) + **fail2ban** for SSH

**A fresh deploy needs no follow-up steps.** The console is live, tier configs
are seeded, backups are scheduled, and Strike clients receive `uot_port` on
their next heartbeat. The one thing a deploy does *not* do is invent activation
codes for you — see the optional batch flag below.

### Optional: generate a first batch of codes during deploy

```bash
FIRST_BATCH=50 FIRST_BATCH_MIDDLEMAN=Sarah \
  ssh root@your-vps "/root/server/setup.sh"
```

| Variable | Meaning |
|----------|---------|
| `FIRST_BATCH` | Codes per plan (applies to both) |
| `FIRST_BATCH_FREE` / `_STRIKE` | Per-plan overrides (`strike` is the paid row) |
| `FIRST_BATCH_MIDDLEMAN` | Recorded against every code in the batch |
| `FIRST_BATCH_EXPIRES` | Optional expiry, `YYYY-MM-DD` |

Minted **once** and recorded in `/root/.first_batch_done`, so a re-run never
creates a second batch of unsold inventory. Delete that file to re-arm.

### Opting out

| Variable | Effect |
|----------|--------|
| `ENABLE_UOT=0` | Skips the sing-box UoT endpoint *and* its firewall rule. Re-run `seed-pb.py` so the strike tier stops advertising `uot_port`. |
| `SKIP_CONSOLE=1` | Skips deploying the admin console (and stops requiring its bundle). Not recommended. |
| `SKIP_DNS_CHECK=1` | Proceeds even if the domain does not resolve to this host. |

---

## 5. Post-Deployment Verification

After deployment, verify:

- [ ] `systemctl is-active caddy` → active
- [ ] `systemctl is-active pocketbase` → active
- [ ] `systemctl is-active shadowsocks-eco` → active
- [ ] `systemctl is-active shadowsocks-strike` → active
- [ ] `sysctl net.ipv4.tcp_congestion_control` → bbr
- [ ] `tc -s class show dev eth0` → classes 1:10 (Free 1 Mbps), 1:30 (Full 100 Mbps)
      — and **no** 1:20 class
- [ ] `systemctl is-enabled shadowsocks-stealth 2>&1` → **not-found / disabled** on an
      upgraded box (retired; remove by hand — `business/14-risks.md` §14.8)
- [ ] `systemctl is-active pocketbase-backup.timer` → active (hourly B2 backups; setup auto-runs the first backup)
- [ ] `tail -5 /var/log/myvpn-backup.log` → last line "Backup completed (exit 0)"
- [ ] `systemctl is-active locus-fetch` → active (only if publishing releases)
- [ ] `curl -sf https://networkingguides.jadedns.uk/api/health` → 200
- [ ] `curl -s -o /dev/null -w '%{http_code}' https://networkingguides.jadedns.uk/admin/` → 200 (and `/admin` → 301)
- [ ] `ufw status` → active with all rules
- [ ] `systemctl is-active fail2ban` → active
- [ ] `openssl s_client -connect networkingguides.jadedns.uk:443 -servername networkingguides.jadedns.uk </dev/null 2>/dev/null | openssl x509 -noout -dates` → valid cert
- [ ] **The legacy hub name answers too** — every pre-3.3.2 build dials it:
      `curl -sf https://networkingguides.duckdns.org/api/health` → 200, **and**
      `openssl s_client -connect networkingguides.duckdns.org:443 -servername networkingguides.duckdns.org </dev/null 2>/dev/null | openssl x509 -noout -enddate`
      → a date in the future. A valid cert here is not optional while earlier
      builds are in the field; an expired one strands them at the activation gate
      with no path to an update. Set `LEGACY_DOMAIN=""` only once none remain.

> **`/update.json` is not a health check.** It is a stale placeholder written by
> `05-caddy.sh`. The client updater reads the **`update_config`** record, not
> that file — do not use it to judge whether updates are working. See
> `OPS.md` → "Client Update System" → "Verify it is actually working".

> **⚠️ Do not rate-limit SSH with ufw.** Earlier revisions used
> `ufw limit 22/tcp` (6 conns/30s), which locked out legitimate automation, and a
> custom limiter chain in `/etc/ufw/before.rules` locked SSH out **completely**
> (port 22 timed out while 80/443 served), needing provider-console recovery.
> SSH protection is **fail2ban** (`/etc/fail2ban/jail.d/locus-sshd.local`,
> 6 fails/10m → 1h ban, `banaction=ufw`).

> **⚠️ Backup timer gotcha:** the timer has `Requires=pocketbase.service`, so
> **stopping PocketBase also stops the timer** (systemd `Requires=` propagates
> stops, not starts). After any `systemctl stop pocketbase`, bring it back with
> `systemctl restart pocketbase-backup.timer` (or `enable --now` on a fresh
> install). `restore.sh` does this automatically.

> **Smoke test:** `setup.sh` runs `server/scripts/smoke-test.sh` automatically
> at the end (log: `/var/log/myvpn-smoke-test.log`). A fresh deploy should end
> with **23 passed / 0 failed / 0 warnings**.

> **Consistency check:** `setup.sh` also runs
> `server/scripts/check-consistency.sh` **before** any module, and CI runs it on
> every push. It checks the source, not the host: that the platform-key list is
> identical in all six hooks, that frozen wire names (`uot_port`,
> `update_<platform>`) are still emitted and still read, that every field a hook
> writes exists in the schema, that the cipher is defined only once, and that no
> tracked path contains `:` (which breaks the Windows checkout entirely).
>
> Each of those is a bug this project has already shipped once. A failure warns
> and the deploy continues — set `STRICT_CONSISTENCY=1` to make it fatal.
>
> Run it by hand before any change touching a wire name, a platform key, the
> schema, or the cipher:
>
> ```bash
> server/scripts/check-consistency.sh
> ```

> **SS2022 verification:** after deploying, `server/scripts/verify-ss2022.sh`
> confirms the migration actually took effect — that every config carries the
> SS2022 cipher, that each tier key decodes to the right length, that the UoT
> inbound agrees with the Strike tier, and that the listeners are up without
> handshake errors. See `../reference/FIXES.md` (2026-09-28, SS2022 entry).

### Create PocketBase Admin (First Run Only)

1. Visit `https://networkingguides.jadedns.uk/_/` in a browser
2. Create your admin account (first-run wizard)

> If `PB_ADMIN_EMAIL` and `PB_ADMIN_PASS` were set in `secrets.env.age`, the
> admin account is created automatically by `06-pocketbase.sh` /
> `seed-pb.py`. Check `/root/.pb_admin_creds` on the VPS.
>
> **PocketBase 0.22 note:** `06-pocketbase.sh` can only bootstrap the first
> admin from the **CLI** (`pocketbase admin create … --dir /opt/pocketbase/pb_data`).
> `/api/collections/_superusers/*` returns **404** on this build — an earlier
> `seed-pb.py` used exactly that path and therefore seeded *nothing* while
> reporting success. `admin create` never updates an existing admin, so if the
> credentials file disagrees with the live password, `seed-pb.py` forces an
> `admin update` when the env value fails to log in.

### Deploy the Admin Console (Recommended)

```bash
# Builds locally, uploads, extracts to /var/www/admin, reloads Caddy, verifies
server/scripts/deploy-console.sh
```

Day-to-day operations then happen at `https://<domain>/admin/` with the admin
token. Note `05-caddy.sh` extracts `console-dist.tar.gz` if present, so a full
`setup.sh` re-run keeps the deployed console rather than reverting it.

### Generate Activation Codes

```bash
# Generate codes for each tier (token = PocketBase ADMIN JWT from
# /root/.pb_admin_creds — NOT /root/.admin_api_token, which PocketBase 0.22
# rejects with 401)
PB_TOKEN=$(ssh root@your-vps "grep PB_TOKEN /root/.pb_admin_creds | cut -d= -f2")
./scripts/generate_codes.sh https://networkingguides.jadedns.uk "$PB_TOKEN" free 50
./scripts/generate_codes.sh https://networkingguides.jadedns.uk "$PB_TOKEN" strike 20
```

### Print Code Cards

```bash
./scripts/print_codes.sh free-codes.txt free-cards.pdf
./scripts/print_codes.sh strike-codes.txt full-cards.pdf
```

---

## 6. Maintaining Secrets

When credentials change (e.g., B2 application key rotated):

```bash
# Decrypt, edit, re-encrypt — use the scripts, which verify the round trip
LOCUS_SECRETS_PASSPHRASE_FILE=~/passphrase.txt server/scripts/secrets-decrypt.sh > /tmp/.secrets.env
vim /tmp/.secrets.env
LOCUS_SECRETS_PASSPHRASE_FILE=~/passphrase.txt server/scripts/secrets-encrypt.sh /tmp/.secrets.env
shred -u /tmp/.secrets.env
git add server/secrets.env.age
git commit -m "Update credentials"
```

See [`SECRETS-MANAGEMENT.md`](SECRETS-MANAGEMENT.md) for full details on
passphrase rotation, CI/CD integration, and troubleshooting.

---

## 7. Disaster Recovery

### Using Age Secrets (Recommended)

```bash
# With the passphrase in a temp file on the VPS:
scp -r server root@new-vps:/root/
ssh root@new-vps "LOCUS_SECRETS_PASSPHRASE='...' /root/server/restore.sh"
```

### Manual Override

```bash
B2_APPLICATION_KEY_ID=xxx \
B2_APPLICATION_KEY=xxx \
B2_BUCKET=my-vpn-backup-bucket \
DOMAIN=networkingguides.jadedns.uk \
ssh root@new-vps 'bash -s' < server/restore.sh
```

This will:
1. Provision the new VPS from scratch (runs full `setup.sh` — BBR, 2× Shadowsocks, tc caps 1/100 Mbps, Caddy + TLS, PocketBase, backups, UFW)
2. Install the b2 CLI and authenticate with the secrets' B2 key
3. Find the **latest** backup in `b2://<bucket>/backups/` (or use `BACKUP_PATH=...` to pick a specific one)
4. Download + verify the SHA256 checksum (aborts on mismatch)
5. Restore the PocketBase database (backs up the fresh-seed DB to `data.db.pre-restore`, removes stale `-wal`/`-shm`)
6. **Align the admin password with the secrets file** (`pocketbase admin update`) so the documented credentials work
7. Restart the backup timer (stopping PocketBase stops it via `Requires=`) and verify it is active
8. Verify: codes count from the restored DB, JS hooks loaded (400 "Missing code"), health endpoints

> **One-command recovery = plug-n-play.** A blank Ubuntu 22.04 VPS + this repo
> + the **passphrase** is all you need: `setup.sh` for a fresh deploy, `restore.sh`
> for a full migration/disaster recovery. No key file: the ciphertext is in the
> repo and the passphrase is the only thing you must supply. The scripts were
> originally validated live on 2026-08-01 (see `../reference/FIXES.md`); the
> passphrase path was verified by round trip on 2026-10-16 but has not yet been
> run against a blank host.
