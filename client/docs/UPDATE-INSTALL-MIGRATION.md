# Updating ACROSS an identity move — the installer side

```
audience:    builder
status:      design-record
authoritative-for: how an update from a previous product identity is installed without losing data or leaving two products
verified-against: client/src-tauri/src/chrysalis/update/predecessor.rs, client/src-tauri/packages/windows/migrate-predecessor.nsh, @tauri-apps/cli 2.11.4's embedded installer.nsi
```

> Read [`UPDATE-ARCHITECTURE.md`](UPDATE-ARCHITECTURE.md) first. That file is
> about *getting* an update; this one is about *installing* it when the version
> on disk belongs to a different product identity — and where the boundary of
> what a client-side change can fix actually falls.

---

## 1. The defect this exists for

`v3.5.1` was offered to a fleet that had `Locus` installed. A device that took it
end up with **both** applications, and with the activation code for neither. A
code is the thing a student paid for and cannot re-derive, so the recovery is not
"activate again" — it is support. The release was withdrawn from the hub rather
than left to repeat.

**Nothing in the client's own Rust could have prevented it, and nothing in a
future client's Rust can.** Two separate reasons, and both are worth stating
precisely because the intuitive fix — "handle it in the updater" — is wrong for
each:

1. **The erasing instructions are not ours.** Setup and uninstall are generated
   by `tauri-bundler` from the vendored CLI. The uninstaller deletes user data
   **by default** (§2.1), and the updater hands it a command line that selects
   that path. The client cannot change the script it is not compiled from, and on
   Windows the plugin calls `process::exit(0)` before any line of ours after the
   hand-off could run (`tauri-plugin-updater` 2.11.0, `updater.rs:876`).
2. **A released client's Rust cannot be changed at all.** The update path is
   exactly the path that has to work *before* an update can fix anything. So the
   only code that can guard a cross-grade is the code already on the device —
   which is why §4 exists.

## 2. Ground truth, taken from the bundler rather than from its documentation

The vendored `@tauri-apps/cli` (2.11.4) embeds its NSIS template as a string. It
was extracted and read, because every paragraph of design below depends on it and
the documentation does not say any of it. Two facts decide the whole design:

### 2.1 The uninstaller erases `%APPDATA%\<bundle-id>` by default

```nsis
; un.ConfirmShow — a checkbox, DEFAULT CHECKED (created, never sent BM_SETCHECK)
System::Call 'user32::CreateWindowEx(... w "$(deleteAppData)" ...)'
...
; Uninstall section
${If} $DeleteAppDataCheckboxState = 1
${AndIf} $UpdateMode <> 1
  RmDir /r "$APPDATA\${BUNDLEID}"
  RmDir /r "$LOCALAPPDATA\${BUNDLEID}"
${EndIf}
```

Three consequences that are not obvious from any Tauri doc:

- **`BUNDLEID` is the app identifier** (`com.locus.client` / `com.chrysalis.client`
  — `tauri.conf.json`'s `identifier`), so the *data root* is what gets removed,
  which is where the activation code and device fingerprint live.
- **`/UPDATE` suppresses it** (`$UpdateMode <> 1`) — but `un.onInit` reads `/P`
  and `/UPDATE` **without clearing `$PassiveMode` first**, so a `/UPDATE` uninstall
  also forces `PassiveMode = 1`, and `un.SkipIfPassive` skips the Confirm page.
  The checkbox never renders, `$DeleteAppDataCheckboxState` is **unset** (not
  set-to-unchecked), and the `= 1` guard does *not* fire. **A self-update's
  uninstaller preserves data, structurally.**
- The failure therefore is **not** an uninstaller deletion in the self-update
  path. It is the *other* path: a **second product's** installer running a
  non-update-uninstall of the first, or a user running the first's uninstaller.
  §2.2 is where that comes from.

Archived-and-kept, never deleted, is still the right policy for a predecessor —
see §3's rule — but it is justified by "we cannot prove which path ran on which
device", not by "the uninstaller deletes by default".

### 2.2 There can be TWO uninstallers, and only one belongs to us

The template writes its registration to `UNINSTKEY = Software\Microsoft\Windows\
CurrentVersion\Uninstall\${PRODUCTNAME}`, `InstallLocation` = `$INSTDIR` (from
`MANUPRODUCTKEY` → `Software\<manufacturer>\<product>`), `UninstallString` =
`$INSTDIR\uninstall.exe`, and copies `MANUFACTURER`, `PRODUCTNAME`, `VERSION`,
`MAINBINARYNAME`, `BUNDLEID` from `bundle`/`tauri.conf.json`.

**The same CLI ships an `nsis_tauri_utils` plugin and a second, separate NSIS
setup for the privileged service** (`clash-verge-service`, fetched by
`prebuild.mjs`, emitting its own `.exe` with its own `ExecShell` of
`clash-verge-service-install.exe`). Whether *that* registration's
`UninstallString` points at the app's `uninstall.exe` or its own cannot be
established from this repository — its build inputs are downloaded at build time.

So the system has, at most, two candidates for "the uninstaller for this install
location", and only one of them is ours. A migration that simply ran
`$LOCUS_UNINSTALL` would be running an unverified binary with the user's
privileges. That is what §3.2's allowlist is for.

### 2.3 macOS replaces the bundle in place; Linux installs nothing

From `tauri-plugin-updater` 2.11.0:

- **macOS** (`updater.rs:1288+`): extracts `<Name>.app.tar.gz`, renames the
  existing bundle to a temp backup, moves the new one to `extract_path`. A
  cross-grade therefore **replaces `Locus.app` in place** and needs no second
  uninstall step. What it cannot do is migrate the data root, because it runs
  before elevation.
- **Linux** (`updater.rs:771`): `fn install_inner(&self, _bytes: &[u8]) -> Ok(())`.
  The raw `chrysalis-linux-amd64` **installs nothing**. A Linux self-update with a
  raw payload is a no-op that exits 0.
- **Windows** (`updater.rs:835+`): `ShellExecuteW("open", <setup>, "/P /R [/UPDATE
  on update]")` then `process::exit(0)`.

### 2.5 Linux: nothing to migrate, and why that is an observation rather than an omission

`install_inner` is `Ok(())` on Linux (§2.3), so the *raw payload* path installs
nothing and cannot create a second application. What a real Linux install is, is a
`.deb`/`.rpm` from the release, and those are upgraded by the package manager.

The legacy identity was never distributed as a Linux package with clients on it —
**no deployed client runs `Locus` on Linux** — so there is no data root under
`~/.local/share/com.locus.client` on a Linux machine, and nothing for a `postinst`
to move. That is a statement about the fleet, not about the code, so it belongs
here as a **dated observation** and not in `state.toml`:

> **2026-12 —** the operator states no Linux client runs the legacy identity. Not
> independently verifiable from this checkout; if it stops being true, this
> section is what makes a Linux `postinst` migration the next change, and
> `check-consistency.sh` §56's table is where the row already exists for the
> client half.

The one Linux path that *does* touch user files is `packages/linux/pre-remove.sh`,
which runs the **service** uninstaller on package removal. It does not touch the
app-data root, and it should not start: removing a package is the user's decision
about the application, and the data root holds a purchase.

### 2.6 What this means for each platform

| Platform | Duplicate app? | Where migration can run | Decision |
|---|---|---|---|
| Windows | Yes — two install dirs, two registrations | The installer, `NSIS_HOOK_PREINSTALL` (elevated, before `File` overwrites) | **Built.** `packages/windows/migrate-predecessor.nsh` |
| macOS | No — `Locus.app` is replaced in place | Only in our process, before the hand-off (the plugin elevates itself) | **Not needed inside the installer**; the data adoption is `dirs.rs`'s job and already exists |
| Linux | Yes in principle (a `.deb` leaves the other package installed) | The package's `postinst` | **Out of scope by decision, with a basis rather than a shrug:** no client runs the legacy identity on Linux — see §2.5. Recorded so the gap stays deliberate |

## 3. The Windows migration, as built

`client/src-tauri/packages/windows/migrate-predecessor.nsh`, included through
`bundle.windows.nsis.installerHooks`.

### 3.1 Where it hooks, and why exactly there

`NSIS_HOOK_PREINSTALL` is inserted at the **top of `Section Install`, immediately
before `File "${MAINBINARYSRCPATH}"`**. Reading the template in order:

```nsis
Section Install
  SetOutPath $INSTDIR
  !ifmacrodef NSIS_HOOK_PREINSTALL
    !insertmacro NSIS_HOOK_PREINSTALL     ; <-- runs here
  !endif
  !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
  File "${MAINBINARYSRCPATH}"             ; <-- writes over our own executable
```

Two things follow, and both are load-bearing:

- The hook runs **before** our binary is overwritten, so it can detect its own
  payload by hash (§3.3).
- The hook runs **after** `SetOutPath $INSTDIR` but **before** `File`, i.e. after
  `$INSTDIR` holds the target. It must not rely on anything the `File` line
  writes.

### 3.2 The allowlist, and the version anti-rollback

The predecessor is found by **our** registry identities (`Software\Windows\
CurrentVersion\Uninstall\Locus` under `HKLM`, `HKLM\WOW6432Node`, `HKCU` — the
same hive order the template itself uses at `ReadRegStr $R0 SHCTX "${UNINSTKEY}"`),
which carries `InstallLocation`, `DisplayVersion`, `UninstallString`.

**The uninstaller is never executed from `UninstallString`.** It is executed only
if all of these hold, and otherwise the product is left installed and reported:

| Check | Why |
|---|---|
| `UninstallString` resolves to a **file** | it may be a quoted command with arguments, or nothing |
| It lives **inside** the predecessor's `InstallLocation` | an uninstaller that can point outside its own product is an arbitrary-code primitive, and §2.2 says a second registration can exist |
| There is **no `..` component** in the path | same, spelled against traversal rather than trusting normalisation |
| It is `uninstall.exe` | the name the template creates |

**Anti-rollback, and why it is the real guard.** The migration only runs when the
installer's own `VERSION` is **strictly newer** than the predecessor's
`DisplayVersion`. That is what stops a `Locus`-era build from being installed
over `Chrysalis` and then "migrating" a newer install backwards: the guard lives
in the installer being run, so it protects whatever it is run over. Note the
guard's limit honestly — the *fleet* is protected because the hub only ever
offers a newer version (`update.pb.js`'s `isNewer`, and the client's own
`version.rs`), not because this check could stop a hand-run installer.

### 3.3 The payload guard (the "two app copies" mechanism, named)

A predecessor `Locus` install contains a **byte-identical** `locus.exe`: the
updater payload is the same Tauri app under the old binary name. If migration
runs while that process is alive, or if the uninstaller is skipped, the copy
remains and the user is running the old build from a different directory.

The uninstaller's built-in `CheckIfAppIsRunning "${MAINBINARYNAME}.exe"
"${PRODUCTNAME}"` only searches **one executable name**, and that name is
`Locus`'s, not ours — so it cannot see a `locus.exe` that a mis-advertised
manifest dropped under another name. Hence the hook waits for the exit path
deterministically: it kills the predecessor's main binary by the name recorded in
**its** `MainBinaryName` registry value (falling back to `Locus.exe`), waits, and
then proceeds. Our side additionally refuses to hand over while our own process
is running (`chrysalis/update/install.rs`), which is what makes the `.exe` hash
tuple stable enough to be worth hashing.

**Verified vs structural, stated plainly:** `native_utils.dll` (the
`NSISdl`-era `NativeUtils::Has`/`NativeUtils::isAnswerIgnore`) is **not**
available in the Tauri NSIS bundle — it ships `nsis_tauri_utils.dll` only — so
any inclusion-line assertion is `error`-free but only catches the cheap mistake.
The substantive guards are structural: the macro's own text, and the bundler's
contract that a listed hook is `!include`d and inserted at that point.

## 4. What the client does (and what it cannot)

### 4.1 Detection and the pre-hand-off stop

`chrysalis/update/predecessor.rs` answers one question: *is a previous identity
of this product installed here?* — a table of known identities
(`com.locus.client`, and the upstream `io.clash-verge-rev…` the fork was copied
from) so a future move is a table row and not a new code path.

It is deliberately **pure**: the OS probing is a thin layer that produces an
`Observation`, and every decision is a function of that observation. That makes
all four platforms testable from one machine, which matters because this logic
runs on the path that is hardest to observe after the fact.

`ReadyInstall::install()` uses it to (a) write a stage *before* the hand-off, and
(b) refuse to proceed if the predecessor is **currently running**, because the
plugin is about to `ShellExecute` the setup and exit — anything we would do after
that is unreachable (`install.rs` already documents this at `InstallStage::Launched`).

### 4.2 The honest limit

A client released **before** this change cannot use any of it: it has no
predecessor detection, and it is the one being replaced. The guard arrives with
the fix, not before it. That is why the only safe sequence for the fleet is the
one in `docs/operate/RELEASING.md` §"cross-grade releases":

1. ship the client change (detection, stage, refusal) **with the hub still
   withdrawn** — it reaches the fleet only as an explicit install;
2. publish the release that carries the installer hook **and** the client change
   together;
3. re-enable the `update_config` row last.

Nothing about a *future* identity move then needs new Rust: the table row and the
`.nsh`'s constant block are generated from the same place (§5), and the flow is
otherwise identical.

### 4.3 How this fleet is actually being moved: supervised, not automatic

The three-step sequence above is the general answer and stays the answer for any
*unattended* fleet. **This fleet is not unattended** — it is beta testers, a small
and known group, and that changes what is safe rather than what is correct:

- **The offer stays withdrawn while each device is migrated under supervision.**
  Each tester's cross-grade is driven with the operator present, so the device is
  in a state someone has looked at *before* the installer runs.
- **The reason this is acceptable here and would not be for a general fleet** is
  the one this section keeps repeating: nothing can be verified off-device. A
  supervised run is the only place the macro's behaviour becomes observable, so for
  a small group it is strictly *better* evidence than a silent roll-out to a large
  one — a silent roll-out produces the same defect with no witness.
- **What supervision is for, concretely**, and this is the checklist a run
  follows — it is also the answer to §"unverified" in
  `docs/reference/FIXES.md`, which says the erasing path was never identified:

  1. Before: record whether `%APPDATA%\com.locus.client` exists, its size, and
     whether `Locus` appears in Add/Remove Programs.
  2. Run the upgrade. Read the setup's own log for the `Chrysalis:` `DetailPrint`
     lines — every branch in the macro names itself.
  3. After: `Locus` gone from Add/Remove, `%APPDATA%\com.chrysalis.client` present
     and non-empty (or `...adopted-from-com.locus.client` present, which is the
     moved-aside case and is the *expected* outcome when the new build started
     first), and the app opens **on the tester's existing code**.
  4. If the code is missing from the app but the aside directory exists, the data
     is intact and moveable by hand — that is the recovery, and it is why the macro
     renames rather than deletes.

  Step 3's last clause is the one that closes the unverified question: **if a
  supervised run shows the code surviving, the defect's erasing path is either gone
  or bypassed, and the evidence is a session that was observed** rather than an
  inference from the script. Record each run's outcome as a dated world claim in
  `docs/operate/CLAIMS.md` §5. Until at least one run has done that, `FIXES.md`'s
  entry stays "cause unidentified" and says so.

### 4.4 The one thing supervision cannot fix

`Locus` and `Chrysalis` both being **open at once** is a state only the student
creates, and no installer can observe it. The client refuses to hand over while a
predecessor is running (§4.1) and the macro kills its binary before proceeding
(§3.3), so the *installer* is covered. What is not covered, and should not be, is a
person launching the old app afterwards from a shortcut that points at a
directory which no longer exists — that fails harmlessly, and the support answer is
"uninstall it, or ignore it; it is gone".

## 5. The drift guard

Two files must agree and they live in different languages:

- `predecessor.rs`'s `PREDECESSORS` table (app id, product name, display name);
- `migrate-predecessor.nsh`'s `!define` block.

They are compared by `server/scripts/check-consistency.sh` §54, which reads both
and fails when a row exists in one and not the other. It is a **grep-shaped**
guard on purpose, in the style of §14: it cannot prove the NSIS macro compiles,
but it fails at the exact moment the two lists diverge, which is the change that
would silently stop migrating a fleet.

---

## 6. Related

- [`UPDATE-ARCHITECTURE.md`](UPDATE-ARCHITECTURE.md) — how an update is *found* and *verified*
- [`IDENTITY-MIGRATION.md`](IDENTITY-MIGRATION.md) — the data-root adoption this complements
- [`SIGNING.md`](SIGNING.md) — why the installer carries its own signature
- [`../../docs/reference/NAME-MIGRATION.md`](../../docs/reference/NAME-MIGRATION.md) §3.2 — the identities, and what retires them
