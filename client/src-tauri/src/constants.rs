use std::time::Duration;

pub mod network {
    pub const DEFAULT_EXTERNAL_CONTROLLER: &str = "127.0.0.1:9097";

    pub mod ports {
        #[cfg(not(target_os = "windows"))]
        pub const DEFAULT_REDIR: u16 = 7895;
        #[cfg(target_os = "linux")]
        pub const DEFAULT_TPROXY: u16 = 7896;
        pub const DEFAULT_MIXED: u16 = 7897;
        pub const DEFAULT_SOCKS: u16 = 7898;
        pub const DEFAULT_HTTP: u16 = 7899;
    }
}

pub mod timing {
    use super::Duration;

    pub const CONFIG_UPDATE_DEBOUNCE: Duration = Duration::from_millis(300);
    pub const STARTUP_ERROR_DELAY: Duration = Duration::from_secs(2);

    // How long a re-asked staging request is worth waiting for. One that already landed answers in
    // milliseconds; anything slower means the Service is still working, so ReplaceCore beats waiting.
    pub const STAGE_CONFIRM_TIMEOUT: Duration = Duration::from_secs(5);

    pub const RUNTIME_PROVIDER_SYNC_DELAY: Duration = Duration::from_secs(15);
    pub const RUNTIME_PROVIDER_SYNC_RETRY_DELAY: Duration = Duration::from_secs(15);
    // mihomo rewrites provider caches in place.
    pub const RUNTIME_PROVIDER_SETTLE: Duration = Duration::from_secs(2);

    // The Windows service takes a while to cold-start, so a start gives it this
    // long to become ready before the attempt is reported as waiting.
    #[cfg(target_os = "windows")]
    pub const SERVICE_WAIT_MAX: Duration = Duration::from_millis(30000);
    #[cfg(target_os = "windows")]
    pub const SERVICE_WAIT_INTERVAL: Duration = Duration::from_millis(200);

    // The Windows service releases the ext-controller channel slightly after the
    // stop returns, so the next start retries briefly rather than racing it.
    #[cfg(target_os = "windows")]
    pub const SERVICE_START_RETRIES: usize = 5;
    #[cfg(target_os = "windows")]
    pub const SERVICE_START_RETRY_DELAY: Duration = Duration::from_millis(300);
}

pub mod profile {
    /// Floor, in minutes, for how often a profile may auto-update (24 hours).
    pub const MIN_UPDATE_INTERVAL: u64 = 1440;
}

pub mod files {
    pub const RUNTIME_CONFIG: &str = "clash-verge.yaml";
    pub const CHECK_CONFIG: &str = "clash-verge-check.yaml";
    pub const DNS_CONFIG: &str = "dns_config.yaml";
    pub const WINDOW_STATE: &str = "window_state.json";
}

pub mod tun {
    pub const DEFAULT_STACK: &str = "gvisor";

    pub const DNS_HIJACK: &[&str] = &["any:53"];

    /// The TUN interface MTU.
    ///
    /// **1280, not the conventional 1500.** This is a deliberate product
    /// decision for the network Locus is built for, not a default inherited
    /// from upstream.
    ///
    /// Why it matters on N4L: a TUN at 1500 hands the stack an effective
    /// payload of ~1472 bytes (1500 minus the 20-byte IPv4 and 8-byte UDP
    /// headers). Shadowsocks AEAD adds ~32 bytes of its own, so a packet that
    /// was already at the boundary fragments *inside* the tunnel — and
    /// fragmented UDP is the classic signature a stateful DPI drops silently.
    /// The symptom on the device is exactly the one reported for Strike: a
    /// connection that looks up, with traffic that stalls and recovers
    /// ("rubberbanding") as individual datagrams vanish.
    ///
    /// 1280 is the floor every IPv6-capable path must support (RFC 8200), so it
    /// is safe for v4 and v6 alike, and it leaves ~200 bytes of headroom for the
    /// encapsulation after the SS AEAD overhead. It costs nothing measurable for
    /// game or voice traffic, which is small-packet to begin with — those games
    /// live at 50-100 bytes per datagram, nowhere near either limit.
    ///
    /// The value is applied through the app's clash config rather than the tier
    /// profile on purpose: `mtu` is a `GUI_KEYS` entry, and `enhance()` captures
    /// those keys and re-imposes them after every profile merge. A value written
    /// only into the tier document would be silently overwritten — inert
    /// configuration, which is the failure mode this codebase has hit before.
    pub const DEFAULT_MTU: u16 = 1280;

    /// Saved dialog fields; `enable` belongs to the verge switch.
    pub const GUI_KEYS: &[&str] = &[
        "stack",
        "device",
        "auto-route",
        "route-exclude-address",
        #[cfg(target_os = "linux")]
        "auto-redirect",
        "auto-detect-interface",
        "dns-hijack",
        "strict-route",
        "mtu",
    ];
}
