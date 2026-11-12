fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Expose the target triple as a compile-time constant so code that must name a
    // built sidecar can derive it instead of hard-coding a second copy of the
    // platform list. `TARGET` is set for every build target, including a
    // cross-compile, which is the case where a hard-coded name would be wrong.
    println!(
        "cargo:rustc-env=LOCUS_BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );

    // ── The pre-activation bootstrap key ──
    //
    // REWRITTEN 2026-11-06. This used to rely on cargo's automatic tracking of
    // `option_env!`: the value was read straight from the environment in
    // `activation_tunnel.rs`. Cargo only rebuilds a crate whose *tracked inputs*
    // changed, and an `option_env!` that was absent last time and present now is
    // not reliably one of them — so introducing the secret could leave every
    // already-compiled object untouched. CI masks the other half of the same
    // problem: a secret that a fork or a fresh clone does not carry is reported
    // as a warning and the release ships **keyless**, silently.
    //
    // Emitting it as a `rustc-env` here makes the dependency an explicit
    // `cargo:rerun-if-env-changed`, so a value that appears or changes forces a
    // rebuild of this crate and everything that reads the constant.
    //
    // Still not `env!`: a missing key must make the TUNNEL unavailable, never
    // break the BUILD. The failure it prevents is the quiet one — a shipped
    // binary that cannot reach the one transport built for a blocked network, with
    // nothing in the artifact saying so. `activation_tunnel.rs` therefore also
    // stamps the compiled-in state into the binary so support can read it back.
    match std::env::var("LOCUS_ACTIVATION_TUNNEL_KEY") {
        Ok(key) if !key.is_empty() => {
            println!("cargo:rustc-env=LOCUS_ACTIVATION_TUNNEL_KEY={key}");
            println!("cargo:rustc-env=LOCUS_ACTIVATION_TUNNEL_KEY_PRESENT=1");
        }
        _ => {
            println!("cargo:rustc-env=LOCUS_ACTIVATION_TUNNEL_KEY_PRESENT=0");
        }
    }
    println!("cargo:rerun-if-env-changed=LOCUS_ACTIVATION_TUNNEL_KEY");

    // ── The free tier's key, for the maintenance tunnel ──
    //
    // WHY THIS IS A SECOND SECRET AND NOT THE ONE ABOVE. `ACTIVATION_TUNNEL_KEY`
    // authenticates to the hub's `:8453` listener, whose forwarder admits exactly
    // three paths (`ALLOWED` in `server/modules/09-activation-tunnel.sh:196`) and
    // refuses everything else with an empty 403. It is a bootstrap for ENROLMENT,
    // not a relay: asking it to carry a plan's TCP stream to `:8445` is precisely
    // what its allowlist exists to prevent. `ECO_PASS` authenticates to `:8443`
    // (`server/modules/02-shadowsocks.sh:279`), which is a general relay, and it
    // is the credential the maintenance tunnel must carry.
    //
    // Same shape as the key above, for the same reason it is `option_env!` in the
    // source rather than `env!`: a build without it must still produce a working
    // client that simply has no maintenance tunnel, never a broken build.
    match std::env::var("LOCUS_FREE_TIER_KEY") {
        Ok(key) if !key.is_empty() => {
            // No `_PRESENT` companion, unlike the activation key above: that one
            // exists because `build_stamp()` renders it for a support log and the
            // *absence* of the tunnel is invisible in the artifact. Here a missing
            // key is visible where it matters — `Bootstrap::maintenance()` returns
            // `None` and the routing rule takes the direct branch — so a second
            // constant would have no reader.
            println!("cargo:rustc-env=LOCUS_FREE_TIER_KEY={key}");
        }
        _ => {}
    }
    println!("cargo:rerun-if-env-changed=LOCUS_FREE_TIER_KEY");

    // ── The bootstrap ADDRESS ──
    //
    // The compiled fallback the tunnel dials when the hub's name cannot be used
    // (a DNS-filtering network — see `contract::ACTIVATION_TUNNEL_ADDR`). It is an
    // ordinary `option_env!` in the source with the current value checked in, so a
    // fresh clone builds a working candidate and nothing has to be set.
    //
    // A build may OVERRIDE it — that is the point of reading it here. The hub
    // moving is an operational event that must not need a source edit reviewed and
    // merged, and the release workflow passes the value it wants. Empty or absent
    // is ignored rather than allowed to blank the compiled default: an empty
    // address would silently reduce the fallback to name-only, which is the
    // failure this candidate exists to fix.
    if let Ok(addr) = std::env::var("LOCUS_ACTIVATION_TUNNEL_ADDR")
        && !addr.trim().is_empty()
    {
        println!("cargo:rustc-env=LOCUS_ACTIVATION_TUNNEL_ADDR_OVERRIDE={}", addr.trim());
    }
    println!("cargo:rerun-if-env-changed=LOCUS_ACTIVATION_TUNNEL_ADDR");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && std::env::var_os("CARGO_FEATURE_CLIPPY").is_some()
    {
        // Tauri's resource is linked to the app binary, but not to Rust unit-test harnesses.
        // Give mock-context builds their own manifest so tests can resolve TaskDialogIndirect.
        let out_dir = std::env::var_os("OUT_DIR").ok_or_else(|| std::io::Error::other("OUT_DIR is not set"))?;
        let manifest_path = std::path::PathBuf::from(out_dir).join("windows-test-manifest.xml");
        std::fs::write(
            &manifest_path,
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
        )?;
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest_path.display());
    }

    #[cfg(feature = "clippy")]
    {
        println!("cargo:warning=Skipping tauri_build during Clippy");
    }

    #[cfg(not(feature = "clippy"))]
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(WINDOWS_APP_MANIFEST)),
    )?;

    Ok(())
}

/// The Windows application manifest embedded in the app binary.
///
/// On Windows this is the whole reason the file is more than one line: the app
/// binary declares `requireAdministrator`, so Windows shows the UAC prompt
/// **when the app is launched**, before any Locus code runs.
///
/// # Why the app asks for elevation up front
///
/// Locus creates a TUN device, which requires administrator rights on Windows
/// (`configure tun interface: operation not permitted` without them). The
/// inherited Clash Verge design avoided running the GUI elevated by installing a
/// privileged *service* and talking to it over IPC — but that means the honest
/// first-run experience is "the app opens, looks fine, and then a UAC prompt
/// appears a few seconds later for reasons the student was never told about",
/// and if they dismiss it the app silently loses the ability to connect. The
/// product decision here is the opposite: ask once, at launch, so the privilege
/// the app needs is granted before the window is useful and never in the middle
/// of a task.
///
/// # What this does not change
///
/// The service is still supported and still used. Elevation makes
/// `tun_capable()` true on its own (`is_admin || service_usable()`), so the
/// service becomes optional rather than load-bearing, but an installed service
/// is still what lets a *non*-elevated launch work. This trades "may need a
/// prompt later" for "always prompts once, up front" — it does not remove the
/// service path.
///
/// # The manifest is not only the elevation level
///
/// `app_manifest` **replaces** Tauri's default manifest rather than merging with
/// it, so everything the default carried must be reproduced here or it silently
/// disappears. That default is a single Common-Controls v6 dependency, and
/// losing it breaks the modern dialog/task-dialog APIs Tauri uses — a failure at
/// runtime on Windows only, which is exactly the kind of breakage that ships.
/// The DPI-awareness hint is added to match the installer (which sets
/// `ManifestDPIAware`), so a per-monitor scaling change does not blur the window.
///
/// Kept as a named constant so the elevation level is greppable — a reader
/// asking "does the app request UAC?" should find the answer in one search.
#[cfg(not(feature = "clippy"))]
const WINDOWS_APP_MANIFEST: &str = r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true</dpiAware>
    </windowsSettings>
  </application>
</assembly>
"#;
