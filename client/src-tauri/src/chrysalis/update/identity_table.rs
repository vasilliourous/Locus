// Identity rows for products this application used to be.
//
// THIS FILE IS COMPILED TWICE, AND THAT IS THE POINT.
//
//   1. `chrysalis::update::predecessor` includes it, next to the detection logic.
//   2. `src/bin/predecessors.rs` includes it, so a host-side tool can print the
//      table without linking the GUI crate — which is what
//      `server/scripts/check-consistency.sh` §54 reads.
//
// A row added here therefore reaches both the updater and the guard that keeps
// this table in step with `packages/windows/migrate-predecessor.nsh`, with no
// second edit. It is data only: no `use`, no logic, no `cfg`. Anything that
// needs code belongs in `predecessor.rs`.

/// A product identity this build may find already installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PredecessorIdentity {
    /// The bundle identifier / app-data directory name it used.
    pub app_id: &'static str,
    /// The `productName` it installed as. This is what names the install
    /// directory (`%PROGRAMFILES64%\<productName>`, `/Applications/<productName>.app`)
    /// and its uninstall registry key, so it is the field the probes use.
    pub product_name: &'static str,
    /// The executable name inside the install directory.
    ///
    /// Recorded rather than derived: the retired client's binary was `locus.exe`,
    /// and a wrong guess here is a process that does not get stopped.
    pub executable: &'static str,
}

/// Identities this product used **before** it was Chrysalis.
///
/// Historical literals, like `crate::utils::dirs::LEGACY_APP_IDS` and for the
/// same reason: the whole point is to look for the name that is no longer
/// written. They must not be "tidied" into the current `APP_ID`, and they must
/// stay in step with `packages/windows/migrate-predecessor.nsh` (guarded by
/// §54).
///
/// **Order is meaningful** — most recent first, mirroring `LEGACY_APP_IDS`, so a
/// machine that somehow has two predecessors reports the one whose data root the
/// current adopted root most likely came from.
pub const PREDECESSORS: &[PredecessorIdentity] = &[
    // The identity this product shipped under until 2026-11-25.
    PredecessorIdentity {
        app_id: "com.locus.client",
        product_name: "Locus",
        executable: "locus.exe",
    },
    // The upstream Clash Verge Rev identity the fork was copied from. Included
    // because `LEGACY_APP_IDS` already admits a data root from it: if a machine
    // can have one of those, it can have the application too.
    PredecessorIdentity {
        app_id: "io.github.clash-verge-rev.clash-verge-rev",
        product_name: "Clash Verge",
        executable: "clash-verge.exe",
    },
];
