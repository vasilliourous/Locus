fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(
        tauri_build::WindowsAttributes::new().app_manifest(WINDOWS_APP_MANIFEST),
    ))?;

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
