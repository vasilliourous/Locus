; Migrate a PREVIOUS IDENTITY of this product before installing over it.
;
; WHY THIS FILE EXISTS
;
; The product was renamed (Locus -> Chrysalis) and its bundle identifier moved
; (com.locus.client -> com.chrysalis.client). On Windows both the install
; directory and the app-data root derive from those two names, so updating across
; the move installs a DIFFERENT application rather than a new version: a student
; ends up with both apps installed and their activation code stranded in the
; older one's data root. The release that did this was withdrawn rather than left
; to repeat. Full reasoning, including why no client-side Rust could prevent it:
; client/docs/UPDATE-INSTALL-MIGRATION.md.
;
; WHAT THIS FILE CAN AND CANNOT DO — because the difference decides whether a
; future maintainer should extend it or replace it:
;
;   * It RUNS ELEVATED, at install time, inside the setup the updater launched.
;     That is the only place the uninstaller can be run with the user's consent
;     already granted.
;   * It runs at `NSIS_HOOK_PREINSTALL`, which the vendored bundler
;     (@tauri-apps/cli 2.11.4, embedded installer.nsi) inserts at the TOP of
;     `Section Install`, after `SetOutPath $INSTDIR` and BEFORE
;     `File "${MAINBINARYSRCPATH}"`. So it can see the target directory and our
;     own binary, and it runs before either is overwritten.
;   * It CANNOT report back to the app: on Windows the plugin calls
;     `ShellExecuteW` and then `process::exit(0)` (tauri-plugin-updater 2.11.0,
;     updater.rs:876). Anything this macro learns is written to the installer's
;     own log and nowhere else.
;
; THE ONE RULE THAT MATTERS MORE THAN THE REST:
; NEVER DELETE A DATA ROOT. Move it, or leave it. A lost activation code is a
; lost purchase, and the student cannot recover it — they bought it somewhere
; else. `RmDir /r` on an app-data path appears nowhere in this file on purpose.

; ---------------------------------------------------------------------------
; Identity table
;
; THIS BLOCK IS COMPARED AGAINST THE RUST TABLE BY A GUARD, NOT BY HOPE.
; `server/scripts/check-consistency.sh` §54 reads
; `src/chrysalis/update/identity_table.rs` (through the `predecessors` binary, so
; it reads what the compiler reads) and fails when a row exists in one file and
; not the other. Add a row in both or the tree goes red.
;
; Shape: !define MIGRATE_<i>_<FIELD>, because NSIS has no arrays. <i> counts from
; 1, and MIGRATE_COUNT must match the number of rows — also asserted by §54.
; ---------------------------------------------------------------------------
!define MIGRATE_COUNT 2

!define MIGRATE_1_APPID "com.locus.client"
!define MIGRATE_1_PRODUCT "Locus"
!define MIGRATE_1_EXE "locus.exe"

!define MIGRATE_2_APPID "io.github.clash-verge-rev.clash-verge-rev"
!define MIGRATE_2_PRODUCT "Clash Verge"
!define MIGRATE_2_EXE "clash-verge.exe"

; ---------------------------------------------------------------------------
; Migration
; ---------------------------------------------------------------------------
!macro ChrysalisMigratePredecessors
  Push $R0
  Push $R1
  Push $R2
  Push $R3
  Push $R4
  Push $R5

  StrCpy $R0 1 ; loop counter

  chrysalis_migrate_loop:
    IntCmp $R0 ${MIGRATE_COUNT} chrysalis_migrate_next chrysalis_migrate_done chrysalis_migrate_done

    chrysalis_migrate_next:
    ; Resolve this row's !defines into registers, using IntCmp rather than an
    ; array because NSIS has none.
    IntCmp $R0 1 chrysalis_row_1 0 chrysalis_row_2
    chrysalis_row_1:
      StrCpy $R1 "${MIGRATE_1_PRODUCT}"
      StrCpy $R2 "${MIGRATE_1_APPID}"
      StrCpy $R3 "${MIGRATE_1_EXE}"
      Goto chrysalis_row_ready
    chrysalis_row_2:
      StrCpy $R1 "${MIGRATE_2_PRODUCT}"
      StrCpy $R2 "${MIGRATE_2_APPID}"
      StrCpy $R3 "${MIGRATE_2_EXE}"
    chrysalis_row_ready:

    !insertmacro ChrysalisMigrateOne

    IntOp $R0 $R0 + 1
    Goto chrysalis_migrate_loop

  chrysalis_migrate_done:
  Pop $R5
  Pop $R4
  Pop $R3
  Pop $R2
  Pop $R1
  Pop $R0
!macroend

; ---------------------------------------------------------------------------
; One predecessor, in registers:
;   $R1 = productName, $R2 = app id, $R3 = its executable
; ---------------------------------------------------------------------------
!macro ChrysalisMigrateOne
  Push $R6
  Push $R7
  Push $R8
  Push $R9

  StrCpy $R6 "" ; $R6 = its InstallLocation, "" when not registered
  StrCpy $R7 "" ; $R7 = its DisplayVersion
  StrCpy $R8 "" ; $R8 = its UninstallString

  ; ── Find its registration ──
  ;
  ; The bundler's template writes
  ;   HKLM|HKCU \Software\Microsoft\Windows\CurrentVersion\Uninstall\<PRODUCTNAME>
  ; with InstallLocation, DisplayVersion and UninstallString. WOW6432Node is
  ; included because an x86 install lands there.
  ;
  ; Hive order mirrors the template's own restore logic. perMachine is this
  ; product's installMode (tauri.windows.conf.json), so HKLM is checked first;
  ; HKCU costs nothing and covers a machine that installed it as a current user.
  ReadRegStr $R6 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "InstallLocation"
  ReadRegStr $R7 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "DisplayVersion"
  ReadRegStr $R8 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "UninstallString"

  StrCmp $R6 "" 0 chrysalis_hm_found
    ReadRegStr $R6 HKLM "Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "InstallLocation"
    ReadRegStr $R7 HKLM "Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "DisplayVersion"
    ReadRegStr $R8 HKLM "Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "UninstallString"
  chrysalis_hm_found:
  StrCmp $R6 "" 0 chrysalis_hu_found
    ReadRegStr $R6 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "InstallLocation"
    ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "DisplayVersion"
    ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\$R1" "UninstallString"
  chrysalis_hu_found:

  ; Not installed. The normal case on a machine that never had it.
  StrCmp $R6 "" chrysalis_one_done

  DetailPrint "Chrysalis: found $R1 $R7 at $R6"

  ; ── ANTI-ROLLBACK ──
  ;
  ; Only migrate FORWARD. A Locus-era build installed over a Chrysalis one must
  ; not "migrate" the newer install backwards, and — more importantly — this is
  ; the guard that protects the fleet when a *newer* installer runs, because the
  ; guard travels with the installer being run.
  ;
  ; Its limit, stated so nobody over-trusts it: this cannot stop a hand-run
  ; installer. The fleet is protected by the hub only ever offering a newer
  ; version (update.pb.js `isNewer`, the client's own version.rs), which is a
  ; different mechanism two layers up.
  ${VersionCompare} "$R7" "${VERSION}" $R9
  StrCmp $R9 "1" 0 chrysalis_not_a_downgrade
    DetailPrint "Chrysalis: $R1 $R7 is not older than ${VERSION}; not migrating it"
    Goto chrysalis_one_done
  chrysalis_not_a_downgrade:

  ; ── Stop it, if it is running ──
  ;
  ; The template's own `CheckIfAppIsRunning` is keyed on
  ; "${MAINBINARYNAME}.exe" — OUR executable — so a live predecessor is invisible
  ; to it, and running its uninstaller underneath it produces a half-removed
  ; product. The app also refuses to hand over while this is true
  ; (chrysalis/update/install.rs); this is the belt-and-braces half for the
  ; non-update path (a person running the setup .exe directly).
  ;
  ; `taskkill` rather than the template's nsis_tauri_utils::KillProcess because
  ; this runs at PREINSTALL and must not depend on a plugin macro's include order
  ; — taskkill is present on every supported Windows and exits non-zero when the
  ; process is absent, which is the answer we want there.
  ;
  ; `nsExec::Exec` returns the exit code on the stack, so it must be popped into a
  ; register this macro SAVED — $R9 is pushed once at the top of
  ; `ChrysalisMigrateOne` and popped once at `chrysalis_one_done`. Popping it
  ; without saving clobbers the caller's $R9, which is the kind of fault that shows
  ; up as an unrelated branch misbehaving in the outer macro.
  nsExec::Exec 'taskkill /F /IM "$R3" /T'
  Pop $R9
  ; taskkill: 0 = killed at least one, 128 = no such process (already stopped),
  ; 1 = "not found"/other. Anything unexpected is reported and NOT treated as
  ; fatal: the uninstaller below has its own running-app check, and refusing to
  ; install because a probe was inconclusive would strand the device.
  StrCmp $R9 "0" 0 chrysalis_kill_reported
    DetailPrint "Chrysalis: stopped $R1 ($R3)"
    Goto chrysalis_kill_done
  chrysalis_kill_reported:
    StrCmp $R9 "128" 0 chrysalis_kill_unknown
      DetailPrint "Chrysalis: $R1 was not running"
      Goto chrysalis_kill_done
  chrysalis_kill_unknown:
    DetailPrint "Chrysalis: could not confirm whether $R1 was running (taskkill returned $R9)"
  chrysalis_kill_done:

  ; ── Preserve its data root, never delete it ──
  ;
  ; THIS IS THE WHOLE POINT OF THE FILE. The data root is
  ; %APPDATA%\$R2 — the bundle identifier, which is exactly what the template's
  ; uninstaller would `RmDir /r` if its "delete app data" checkbox were left
  ; checked. That checkbox is the default the bundler creates, and in a
  ; *non-update* uninstall it is live.
  ;
  ; Two cases, and the second is the destructive one:
  ;
  ;   1. The Chrysalis root does not exist yet -> RENAME the old one into place.
  ;      A rename is atomic within a volume, so a crash cannot leave two
  ;      half-populated roots. This mirrors utils::dirs::adopt_legacy_app_data_dir
  ;      exactly, including its refusal to degrade into a copy.
  ;   2. The Chrysalis root ALREADY exists (the app ran first and created it
  ;      empty) -> move the old one ASIDE under a name a person can find. It is
  ;      then the app's job to notice and say so (InstallStage::AdoptedAside).
  ;      Deleting it here would throw away the only copy of the code.
  ;
  ; `${BUNDLEID}` is this installer's identifier (com.chrysalis.client), expanded
  ; by the bundler. Rename /SHFileOperation? Neither: `Rename` is a single NSIS
  ; instruction and cannot recurse into a wrong tree.
  SetShellVarContext current ; %APPDATA% of the *interactive* user, not the elevated profile
  StrCmp $R2 "${BUNDLEID}" chrysalis_same_identity

    IfFileExists "$APPDATA\${BUNDLEID}\*.*" chrysalis_root_exists chrysalis_root_free
    chrysalis_root_free:
      ClearErrors
      Rename "$APPDATA\$R2" "$APPDATA\${BUNDLEID}"
      IfErrors chrysalis_rename_failed chrysalis_rename_ok
      chrysalis_rename_ok:
        DetailPrint "Chrysalis: moved $R1 application data into the Chrysalis data folder"
        Goto chrysalis_data_done
      chrysalis_rename_failed:
        DetailPrint "Chrysalis: could NOT move $R1 application data; leaving it where it is"
        Goto chrysalis_data_done

    chrysalis_root_exists:
      ; A dated/versioned leaf so a second run cannot collide with the first.
      StrCpy $R4 "$APPDATA\$R2.migrated-from-$R1-$R7"
      ClearErrors
      Rename "$APPDATA\$R2" "$R4"
      IfErrors chrysalis_aside_failed chrysalis_aside_ok
      chrysalis_aside_ok:
        DetailPrint "Chrysalis: kept the existing $R1 data at $R4 (a Chrysalis data folder already existed)"
        Goto chrysalis_data_done
      chrysalis_aside_failed:
        DetailPrint "Chrysalis: could NOT set aside $R1 application data; it is untouched at $APPDATA\$R2"
        Goto chrysalis_data_done

  chrysalis_same_identity:
    DetailPrint "Chrysalis: $R1 uses the same application identity; nothing to relocate"

  chrysalis_data_done:

  ; ── Remove the old application, keeping its data ──
  ;
  ; Only after the data has been preserved above. The uninstaller is executed
  ; ONLY when all four checks pass; otherwise the product is left installed and
  ; reported, because a wrong uninstaller is arbitrary code with the elevation
  ; this macro already holds.
  ;
  ; WHY THE UNINSTALLSTRING IS NOT TRUSTED AS GIVEN: the registry value is a
  ; command line, may be quoted, may carry arguments, and — see
  ; UPDATE-INSTALL-MIGRATION.md §2.2 — a SECOND product (the privileged service
  ; setup) can leave a registration whose uninstaller is not the app's. An
  ; uninstaller lived outside its own product is a privilege boundary, not a
  ; naming detail.
  StrCmp $R8 "" chrysalis_keep_app
  ; Quoted form: "$R6\uninstall.exe"
  StrCpy $R4 '$"$R6\uninstall.exe$"'
  StrCmp $R8 $R4 0 chrysalis_uninst_quoted_variant
    Goto chrysalis_uninst_shape_ok
  chrysalis_uninst_quoted_variant:
  ; Unquoted form: $R6\uninstall.exe
  StrCpy $R4 "$R6\uninstall.exe"
  StrCmp $R8 $R4 0 chrysalis_keep_app
  chrysalis_uninst_shape_ok:

  ${IfNot} ${FileExists} "$R6\uninstall.exe"
    DetailPrint "Chrysalis: $R1 has no uninstaller at its install location; leaving it installed"
    Goto chrysalis_one_done
  ${EndIf}

  ; /S silent, /R do not relaunch, /_?= keep $INSTDIR (an uninstaller that
  ; rebuilds $INSTDIR would move the product).
  ;
  ; NOT /UPDATE: that suppresses the "delete app data" checkbox, and here we WANT
  ; the Confirm page's LEAVE step to run with the box UNCHECKED rather than
  ; skipped — but a passive `/P` also skips the page entirely, leaving
  ; `$DeleteAppDataCheckboxState` unset, which the template's `= 1` guard already
  ; refuses. So data is preserved either way; the reason not to pass /UPDATE is
  ; narrower and worth writing down: /UPDATE also sets `$PassiveMode`, and a
  ; passive uninstall skips `un.ConfirmShow` — meaning we would be relying
  ; entirely on the guard's unset-not-one behaviour. Belt and braces is cheaper
  ; than being clever, and data was already moved out from under the checkbox
  ; name above.
  DetailPrint "Chrysalis: uninstalling $R1 $R7, keeping its data"
  ExecWait '$"$R6\uninstall.exe$" /S /R' $R9
  StrCmp $R9 "0" 0 chrysalis_uninstall_failed
    DetailPrint "Chrysalis: removed $R1"
    Goto chrysalis_uninstall_cleanup

  chrysalis_uninstall_failed:
    DetailPrint "Chrysalis: $R1's uninstaller returned $R9; leaving it in Add/Remove Programs"
    Goto chrysalis_uninstall_cleanup

  chrysalis_uninstall_cleanup:
    ; The uninstall directory outlives /S when files are locked; /REBOOTOK so a
    ; held handle does not silently keep a dead product directory.
    RMDir /r /REBOOTOK "$R6"
    Goto chrysalis_one_done

  chrysalis_keep_app:
    DetailPrint "Chrysalis: $R1 is registered without a recognisable uninstaller; leaving it installed"

  chrysalis_one_done:
  Pop $R9
  Pop $R8
  Pop $R7
  Pop $R6
!macroend

; ---------------------------------------------------------------------------
; Hooks
;
; Only PREINSTALL is defined: this migration is about what exists BEFORE we
; overwrite, and a post-install hook would run after `File` had already replaced
; our own executable — too late to reason about what was there.
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_PREINSTALL
  !insertmacro ChrysalisMigratePredecessors
!macroend
