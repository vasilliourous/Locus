
```
audience:    human-operator
status:      live
authoritative-for: how server credentials are encrypted, and what must never be committed
verified-against: server/secrets.env.age, server/setup.sh, server/scripts/secrets-encrypt.sh
```

> How credentials are deployed with the server — encrypted in the repo, decrypted
> at deploy time with a passphrase.

---

## Problem

The Locus server needs several credentials to run:

| Credential | Used by |
|---|---|
| `DOMAIN` | Every module — the VPS hostname |
| `ADMIN_API_TOKEN` | Admin unbind endpoint + code generator |
| `RELEASE_FETCH_SECRET` | HMAC key for one-shot release-fetch trigger links |
| `B2_APPLICATION_KEY_ID` | Backups module (Backblaze B2 auth) |
| `B2_APPLICATION_KEY` | Backups module (Backblaze B2 auth) |
| `B2_BUCKET` | Backups module (bucket name) |
| `ECO_PASS` | The **free** plan's Shadowsocks password (port 8443) |
| `STRIKE_PASS` | The **paid** plan's Shadowsocks password (port 8445) |
| `STEALTH_PASS` | **Retired tier's password (port 8444).** Still validated and written to `/root/.tier_passwords` for compatibility, but no service consumes it on a fresh box. Keep it in the secrets file so an upgraded host does not fork, and so removing the old service is not blocked by a missing key. |
| `PB_ADMIN_EMAIL`, `PB_ADMIN_PASS` | PocketBase admin credentials |

The bundle also carries one **non-credential** on the same terms: a base64 copy of
the update signing key (`LOCUS_UPDATE_KEY_B64`, added 2026-10-16 — see the FAQ).

**That list is stated as a list, not a number, on purpose:** a count in a sentence
cannot be recomputed and drifts (this document's predecessor said "12" while
listing 11). To get the live count, decrypt the file and count it.

These must be available at deploy time, but **must not be committed in
plain-text**. At the same time, one-command deploy is important — nobody wants to
type 11 environment variables every time they provision a VPS.

---

## Solution: passphrase-encrypted secrets file

**[age](https://age-encryption.org/)** encrypts a `.env` file with all the
credentials, and the **encrypted** version is committed. At deploy time
`setup.sh` decrypts it with a **passphrase** that is not in the repo.

```
server/
├── secrets.env.age      ← Encrypted with a PASSPHRASE. Committed on purpose.
├── setup.sh             ← Decrypts secrets.env.age on start.
├── restore.sh           ← Same logic.
└── scripts/
    ├── secrets-encrypt.sh   ← plaintext → ciphertext
    └── secrets-decrypt.sh   ← ciphertext → stdout / --check
```

### Why a passphrase and not a key file

Until 2026-10-16 this used an age **keypair**: the private half lived in
`age-key.txt`, a gitignored file, and `age -d -i age-key.txt` decrypted the
ciphertext. The key was lost along with the rest of the gitignored working tree,
and was only recovered because an unencrypted-for-deletion copy happened to still
be sitting on the deployed host.

**A secret that can only be recovered from a box you might delete is not a
backup.** The passphrase model trades a little security for recoverability:

| | Keypair (before) | Passphrase (now) |
|---|---|---|
| Recover a fresh clone | Impossible without the key file | `git clone` + passphrase |
| Where the secret lives | A gitignored file that can be lost | In your head / password manager |
| Protection strength | 128-bit key, not guessable | Whatever you choose — **tunable, and weak if you make it weak** |
| Rotation | New keypair + re-encrypt | Change the passphrase + re-encrypt |

### Threat model

| Attack vector | Mitigation |
|---|---|
| Repo leak (attacker gets source) | Credentials are encrypted. **Because the repo is public, treat the ciphertext as downloadable and attackable offline forever** — the passphrase is the only defence. |
| Attacker brute-forces the ciphertext | age's passphrase mode uses scrypt with a deliberately slow work factor. This buys time; it does not make a short passphrase safe. |
| VPS compromise (attacker gets root) | Credentials are on-disk as env vars — acceptable (root has access anyway) |
| Loss of local files | **Solved.** The ciphertext is in git; only the passphrase must be remembered. |
| Forgotten passphrase | **Not recoverable.** There is no recovery key. Re-encrypting requires the plaintext, which you will not have. Keep the passphrase in a password manager — not only in your head — and treat a lost passphrase as equivalent to the old lost-key incident. |

> ⚠️ **Make the passphrase long.** `age -p` derives its key with scrypt, which
> makes each guess expensive but cannot rescue a low-entropy passphrase. Since the
> ciphertext is committed to a repository that is at least sometimes **public**,
> assume an attacker has all the time they want. Use a generated multi-word or
> 20+ character random passphrase, not something you invented.

---

## One-time setup

### Step 1: Install age

```bash
# Debian/Ubuntu — the simplest path
sudo apt-get install -y age

# Or from a release binary
curl -sLO "https://github.com/FiloSottile/age/releases/download/v1.2.1/age-v1.2.1-linux-amd64.tar.gz"
tar -xzf age-v1.2.1-linux-amd64.tar.gz
sudo cp age/age age/age-keygen /usr/local/bin/
rm -rf age age-v1.2.1-linux-amd64.tar.gz

# Verify
age --version
```

> `setup.sh` and `restore.sh` install age themselves on the VPS if it is missing,
> so this step is only needed on the machine that **creates** the secrets.

### Step 2: Choose a passphrase

There is no key generation step any more. Generate a strong passphrase once:

```bash
# 24 random bytes, base64 — about 32 characters of high-entropy text
openssl rand -base64 24
```

Store it in your password manager. **Do not** commit it, and do not put it in
`server/`. `.gitignore` covers the usual accidental names (`encryption-password*`,
`secrets-pass*`, `passphrase.txt`, …) and
`server/scripts/check-consistency.sh` §29 fails the build if one of them is ever
tracked — but neither of those is a substitute for not writing it down nearby.

### Step 3: Create the plain-text secrets file

```bash
cat > .secrets.env << 'EOF'
# Locus Production Secrets
DOMAIN=networkingguides.jadedns.uk
ADMIN_API_TOKEN=<generate with: openssl rand -base64 32>
B2_APPLICATION_KEY_ID=your-backblaze-key-id
B2_APPLICATION_KEY=your-backblaze-application-key
B2_BUCKET=vpsvpnbackup
ECO_PASS=your-base64-of-32-bytes-eco-key
STEALTH_PASS=your-base64-of-32-bytes-stealth-key   # retired tier; see the table above
STRIKE_PASS=your-base64-of-32-bytes-strike-key
PB_ADMIN_EMAIL=admin@networkingguides.jadedns.uk
PB_ADMIN_PASS=your-secure-pb-admin-password
RELEASE_FETCH_SECRET=<generate with: openssl rand -hex 32>
EOF

chmod 600 .secrets.env
```

> **Note on the live token.** Earlier revisions of this file and of
> `CONTEXT.md` embedded the literal `ADMIN_API_TOKEN`. It has been redacted, but
> it remains **in git history** — so treat the deployed token as potentially
> exposed. Rotate it (`openssl rand -base64 32` → update `.secrets.env` →
> re-encrypt → re-run `setup.sh`, then update `/root/.admin_api_token` on the
> VPS and any `/admin/?token=…` bookmark). The token is a bearer credential for
> the entire admin API, so rotation invalidates every existing console session.

> **The admin token is not a PocketBase admin JWT.** It only authenticates the
> custom `*.pb.js` hooks (`X-Admin-Token` / `admin_token` body field). For
> record-level API access (including `generate_codes.sh`) you need the PocketBase
> admin JWT from `/root/.pb_admin_creds` (`PB_TOKEN`) — PB 0.22 returns 401 for
> the `ADMIN_API_TOKEN`.

> **Important:** Pre-generate stable tier passwords (`ECO_PASS`, `STRIKE_PASS`,
> and `STEALTH_PASS` while an upgraded host still names it) and the PB admin
> password. Stable passwords mean a redeploy to a new VPS produces the **exact
> same config** — existing clients don't break, and backup restore keeps working.
>
> **Tier passwords are NOT passphrases — they are Shadowsocks 2022 keys.** The
> deployment runs SS2022 (`2022-blake3-aes-256-gcm`), whose key must be the
> **base64 encoding of exactly 32 bytes**. Generate each one with:
>
> ```bash
> openssl rand -base64 32 | tr -d '\n'   # run once per tier
> ```
>
> The `tr -d '\n'` is **mandatory**. `openssl rand -base64 32` appends a
> newline, and a key carrying one fails at handshake time with no useful error —
> the server starts and then refuses every connection.
>
> **Do not use `openssl rand -hex 16`.** That was the pre-SS2022 form (16 ASCII
> characters / 24 decoded bytes) and SS2022 rejects it outright. Module `00-env.sh`
> validates the decoded length and fails the deploy rather than letting a host
> come up with keys its listeners refuse.
>
> To generate a PB admin password (a normal passphrase, not a key):
> `openssl rand -base64 24`

### Step 4: Encrypt the secrets file

Use the script — it is the supported path, and it verifies its own output:

```bash
cd server/scripts
LOCUS_SECRETS_PASSPHRASE_FILE=~/passphrase.txt ./secrets-encrypt.sh ../../.secrets.env
```

Or, interactively (age will prompt twice):

```bash
cd server/scripts
./secrets-encrypt.sh ../../.secrets.env
```

The script encrypts to `server/secrets.env.age`, then **decrypts it again and
compares it byte-for-byte with the input** before reporting success. If that
round trip fails, it stops and tells you — an encrypt step that cannot be shown
to round-trip is not verified.

It also backs up any existing ciphertext to `secrets.env.age.bak.<timestamp>`
(which is gitignored) and refuses to run if the plaintext is missing `DOMAIN` or
`ADMIN_API_TOKEN`.

Then shred the plaintext:

```bash
shred -u .secrets.env
```

### Step 5: Commit the encrypted file

```bash
git add server/secrets.env.age
git commit -m "Add passphrase-encrypted production secrets"

# Confirm the passphrase itself is not staged — this is the one file that matters
git status
```

---

## How decryption works at deploy time

`setup.sh` and `restore.sh` share the same block. They read the passphrase from
two sources, in order, and **fail** if neither is set:

### Source 1: `LOCUS_SECRETS_PASSPHRASE`

```bash
LOCUS_SECRETS_PASSPHRASE='...' ssh root@vps '/root/server/setup.sh'
```

### Source 2: `LOCUS_SECRETS_PASSPHRASE_FILE`

```bash
# Better: the passphrase stays out of your shell history and out of `ps`
scp passphrase.txt root@vps:/root/.pw
ssh root@vps "LOCUS_SECRETS_PASSPHRASE_FILE=/root/.pw /root/server/setup.sh"
```

Use `chmod 600` on that file, and delete it when the deploy is done.

### Source 3: nothing — a hard failure

There is deliberately **no interactive prompt**. Both scripts run
non-interactively over `ssh` and in CI, where a prompt would hang until the
connection times out with a misleading error. Instead they print a banner naming
both variables and exit 1.

### Why there is a `script(1)` invocation in the deploy path

`age` has **no non-interactive passphrase input**. Verified against age 1.3.2 on
2026-10-16, by reading the binary and by test:

- there is no `AGE_PASSPHRASE`-style environment variable;
- it does not read the passphrase from stdin or from an arbitrary file
  descriptor (`3< file` still hangs waiting for a terminal);
- it opens `/dev/tty` directly, and fails with
  `standard input is not a terminal, and /dev/tty is not available`;
- `-i/--identity` is explicitly rejected for passphrase files:
  `remove all -i/--identity/-j flags to decrypt passphrase-encrypted files`.

So the passphrase must be delivered through a terminal. `script -qec` allocates a
pseudo-terminal, which satisfies age while keeping the passphrase out of `argv`.
That is what both deploy scripts do, and the reason this document is more involved
than "run `age -d`".

`script` comes from util-linux (`apt-get install bsdutils` if absent) and is
present by default on Ubuntu 22.04.

---

## Deploy commands

### From local machine (passphrase in a file)

```bash
scp -r server root@your-vps:/root/
scp passphrase.txt root@your-vps:/root/.pw
ssh root@your-vps "chmod 600 /root/.pw && LOCUS_SECRETS_PASSPHRASE_FILE=/root/.pw /root/server/setup.sh && rm -f /root/.pw"
```

### From local machine (passphrase inline)

```bash
scp -r server root@your-vps:/root/
ssh root@your-vps "LOCUS_SECRETS_PASSPHRASE='...' /root/server/setup.sh"
```

### Disaster recovery (`restore.sh`)

```bash
scp -r server root@new-vps:/root/
ssh root@new-vps "LOCUS_SECRETS_PASSPHRASE='...' /root/server/restore.sh"
```

### Manual env vars (no secrets file at all)

```bash
DOMAIN=... ADMIN_API_TOKEN=... B2_APPLICATION_KEY_ID=... \
  ssh root@new-vps 'bash -s' < server/restore.sh
```

---

## Inspecting and updating secrets

```bash
# Verify the ciphertext decrypts without printing it
server/scripts/secrets-decrypt.sh --check

# Read it into a file you will shred (never leave this on disk)
server/scripts/secrets-decrypt.sh > /tmp/.secrets.env

# Edit, then re-encrypt and shred
$EDITOR /tmp/.secrets.env
LOCUS_SECRETS_PASSPHRASE_FILE=~/passphrase.txt server/scripts/secrets-encrypt.sh /tmp/.secrets.env
shred -u /tmp/.secrets.env

git add server/secrets.env.age
git commit -m "Update secrets"
```

---

## Passphrase rotation

```bash
# 1. Decrypt with the CURRENT passphrase
LOCUS_SECRETS_PASSPHRASE_FILE=~/old-passphrase.txt server/scripts/secrets-decrypt.sh > /tmp/.secrets.env

# 2. Re-encrypt with the NEW one
LOCUS_SECRETS_PASSPHRASE_FILE=~/new-passphrase.txt server/scripts/secrets-encrypt.sh /tmp/.secrets.env
shred -u /tmp/.secrets.env

# 3. Commit, and update your password manager
git add server/secrets.env.age && git commit -m "Rotate secrets passphrase"
```

**Rotation does not change any credential**, so no client and no deployed service
is disturbed. If you are rotating because the *ciphertext* may have been exposed
while the passphrase was weak, that is a different action: rotate the credentials
themselves (`ADMIN_API_TOKEN`, tier passwords, PB admin password, B2 keys) and
re-encrypt.

> **Rotating the tier passwords is disruptive.** They are baked into every
> activated client's profile; changing `ECO_PASS`/`STRIKE_PASS` requires
> re-activating clients. Treat that as a last resort, not housekeeping.

---

## FAQ

### Can I use a passphrase instead of a key file?

That is now the only supported mode — see "Why a passphrase and not a key file".
The age keypair path (`age-key.txt`, `AGE_KEY`) was removed on 2026-10-16 and both
deploy scripts no longer read either. An old checkout that still exports `AGE_KEY`
will fail loudly, not silently use the wrong path.

### What if I lose the passphrase?

There is no recovery mechanism, by design — a recovery key would be another
gitignored file to lose, which is the problem this design removes. Without the
passphrase the ciphertext is unrecoverable and you must rebuild `.secrets.env`
with new credentials. **Millions of CPUs cannot help you**, and the ciphertext
being public does not change that either way. Keep the passphrase in a password
manager.

### Should the signing key go in here too?

**It does — since the 2026-10-16 rotation.** Read why it did not before, because
the reason is the whole lesson.

The update signing key (`LOCUS_UPDATE_KEY`, `client/docs/SIGNING.md`) used to
live *only* in the GitHub Actions secret store. Actions secrets are write-only —
no API, `gh` command or UI action returns the value — and no offline copy was
ever made, so there was nothing on any machine to encrypt. When the key was
needed **it could not be retrieved**: no release could be signed, and the fleet
could not be offered an update. The rotation to `4DA9B48F39251CEF` (3.3.0)
replaced the key; it did not recover it.

That was a custody failure, not a tooling one, and the fix is an order of
operations:

1. **Generate the key locally first**, into the repo root's `.locus-keys/`.
2. **Back that copy up immediately** — offline, and as `LOCUS_UPDATE_KEY_B64` in
   this bundle.
3. **Then** set the Actions secret.

Anything set into an Actions secret before it exists somewhere readable is a
secret you can lose without noticing. The three recovery procedures — key file
missing, Actions secret missing, fleet still on the old key — are
[`RELEASE-RECOVERY.md`](RELEASE-RECOVERY.md).

### How is the key stored in the bundle?

`LOCUS_UPDATE_KEY_B64` is **base64 of the whole key file**, not a literal key
line. That encoding is not decoration: this bundle is `source`d by bash, and the
key's raw form is a comment line followed by the payload, so `source` would try
to execute the comment as a command and abort the deploy. Base64 makes it one
`KEY=value` line that `set -a` exports harmlessly.

Decode it back, check the trailing newline, and **verify the recovered key with a
sign/verify round trip before setting any secret** —
[`RELEASE-RECOVERY.md`](RELEASE-RECOVERY.md) §2 has the commands.

### Why not SOPS / Vault / git-crypt?

| Tool | Issue for this project |
|---|---|
| **SOPS** | Powerful but adds YAML/JSON schema overhead. We just need to `source` env vars. |
| **HashiCorp Vault** | Requires a server, complex ACLs, token management. Overkill for one VPS. |
| **git-crypt** | Requires GPG, git filter setup. Breaks if someone clones without it — which is close to the failure this migration fixed. |
| **GPG directly** | Terrible UX, painful key management, and its default symmetric mode is easy to get wrong. |
| **age keypair** | **The previous design, and the reason for this change**: the private half is a file that can be lost, and was. |
| **Docker secrets** | You're not using Docker. |

age in passphrase mode is the simplest tool that does what we need: encrypt a file
so that `bash` can `source` the decrypted output, with the decrypt capability held
by a human rather than by a file.

### Why pre-generate tier passwords instead of auto-generating?

The current setup auto-generates `ECO_PASS`, `STRIKE_PASS` (and `STEALTH_PASS`,
kept for compatibility with the retired port) on each deploy via
`02-shadowsocks.sh`. This means:
- Redeploying to a new VPS produces **different passwords**
- All existing clients would need to re-activate
- Backup restore from a different deploy would have mismatched passwords

By putting **stable, pre-generated** passwords in the encrypted secrets file,
you ensure that every deploy produces the exact same config — existing clients
keep working, and disaster recovery just works.

To pre-generate:
```bash
openssl rand -base64 32 | tr -d '\n'   # → SS2022 key, once per tier
```
