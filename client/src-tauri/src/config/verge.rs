use crate::config::Config;
use crate::{
    config::{DEFAULT_PAC, deserialize_encrypted, serialize_encrypted},
    constants::network,
    utils::{dirs, help},
};
use anyhow::Result;
use clash_verge_logging::{Type, logging};
use log::LevelFilter;
use serde::{Deserialize, Serialize};
use smartstring::alias::String;

/// ### `verge.yaml` schema
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IVerge {
    /// silent | error | warn | info | debug | trace
    pub app_log_level: Option<String>,

    /// Maximum app log size in KiB.
    pub app_log_max_size: Option<u64>,

    pub app_log_max_count: Option<usize>,

    pub language: Option<String>,

    // `theme_mode` and `theme_id` were removed on 2026-11-19, together with the
    // theme registry, the light appearance and the theme hook. Chrysalis has one
    // permanent appearance (/COLOURS.md section 0), so there is no mode and no
    // theme to store.
    //
    // They are gone from the struct rather than kept and ignored, because a field
    // nobody reads is a field the next reader assumes is load-bearing. This is
    // safe on upgrade: `IVerge` derives `Deserialize` with no
    // `deny_unknown_fields`, so a `verge.yaml` written by an older build still
    // carrying either key simply ignores it. Nothing migrates the stored choice
    // -- that is the documented cost of the decision, not an oversight.
    //
    // FROZEN NAMES NOTE: neither was a wire name. They live in a local file, not
    // in the hub contract, so removing them does not strand a deployed client.
    pub tray_event: Option<String>,

    pub env_type: Option<String>,

    pub start_page: Option<String>,
    pub startup_script: Option<String>,

    pub traffic_graph: Option<bool>,

    /// show memory info (only for Clash Meta)
    pub enable_memory_usage: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_group_icon: Option<bool>,

    pub pause_render_traffic_stats_on_blur: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_tray_icon: Option<bool>,

    #[cfg(target_os = "macos")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tray_icon: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_icon: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_order: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_group_tools_position: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice_position: Option<String>,

    pub collapse_navbar: Option<bool>,

    pub sysproxy_tray_icon: Option<bool>,

    pub tun_tray_icon: Option<bool>,

    pub enable_tun_mode: Option<bool>,

    pub enable_auto_launch: Option<bool>,

    pub enable_silent_start: Option<bool>,

    pub enable_system_proxy: Option<bool>,

    pub enable_proxy_guard: Option<bool>,

    pub enable_bypass_check: Option<bool>,

    /// Initial DNS override preference for profiles without a saved setting.
    pub enable_dns_settings: Option<bool>,

    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub profile_dns_settings: std::collections::BTreeMap<String, super::dns::ProfileDnsSettings>,

    pub use_default_bypass: Option<bool>,

    pub system_proxy_bypass: Option<String>,

    pub proxy_guard_duration: Option<u64>,

    pub proxy_auto_config: Option<bool>,

    pub pac_file_content: Option<String>,

    pub proxy_host: Option<String>,

    pub theme_setting: Option<IVergeTheme>,

    pub web_ui_list: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub clash_core: Option<String>,

    /// format: {func},{key}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkeys: Option<Vec<String>>,

    pub enable_global_hotkey: Option<bool>,

    pub home_cards: Option<serde_json::Value>,

    pub auto_close_connection: Option<bool>,

    pub auto_check_update: Option<bool>,

    pub default_latency_test: Option<String>,

    pub default_latency_timeout: Option<i16>,

    pub enable_auto_delay_detection: Option<bool>,

    /// 自动检测当前节点延迟的间隔（分钟）
    pub auto_delay_detection_interval_minutes: Option<u64>,

    pub enable_builtin_enhanced: Option<bool>,

    pub proxy_layout_column: Option<u8>,

    pub test_list: Option<Vec<IVergeTestItem>>,

    /// 0: 不清理; 1: 1天；2: 7天; 3: 30天; 4: 90天
    pub auto_log_clean: Option<i32>,

    pub enable_auto_backup_schedule: Option<bool>,

    /// Automatic backup interval in hours
    pub auto_backup_interval_hours: Option<u64>,

    pub auto_backup_on_change: Option<bool>,

    /// verge 的各种 port 用于覆盖 clash 的各种 port
    #[cfg(not(target_os = "windows"))]
    pub verge_redir_port: Option<u16>,

    #[cfg(not(target_os = "windows"))]
    pub verge_redir_enabled: Option<bool>,

    #[cfg(target_os = "linux")]
    pub verge_tproxy_port: Option<u16>,

    #[cfg(target_os = "linux")]
    pub verge_tproxy_enabled: Option<bool>,

    pub verge_mixed_port: Option<u16>,

    pub verge_socks_port: Option<u16>,

    pub verge_socks_enabled: Option<bool>,

    pub verge_port: Option<u16>,

    pub verge_http_enabled: Option<bool>,

    #[serde(
        serialize_with = "serialize_encrypted",
        deserialize_with = "deserialize_encrypted",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub webdav_url: Option<String>,

    #[serde(
        serialize_with = "serialize_encrypted",
        deserialize_with = "deserialize_encrypted",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub webdav_username: Option<String>,

    #[serde(
        serialize_with = "serialize_encrypted",
        deserialize_with = "deserialize_encrypted",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub webdav_password: Option<String>,

    #[cfg(target_os = "macos")]
    pub enable_tray_speed: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tray_proxy_groups_display_mode: Option<String>,
    pub tray_inline_outbound_modes: Option<bool>,

    pub enable_auto_light_weight_mode: Option<bool>,

    /// 自动进入轻量模式的延迟（分钟）
    pub auto_light_weight_minutes: Option<u64>,

    pub enable_hover_jump_navigator: Option<bool>,

    /// 代理页面自动滚动延迟（毫秒）
    pub hover_jump_navigator_delay: Option<u64>,

    pub enable_external_controller: Option<bool>,

    // ── Chrysalis product state ──────────────────────────────────────────────────
    //
    // Additive on purpose: every field here is `Option<T>` with a serde default,
    // which is exactly the shape this struct already uses, so adding them needs
    // no migration and inherits Verge's existing draft/transaction/save
    // machinery — one store, one backup mechanism, one thing that can corrupt.
    /// The activation code this device was activated with.
    ///
    /// This is a BEARER CREDENTIAL for the student's tier, and it is stored in
    /// plaintext in a user-writable config. That is deliberate (the retired
    /// client did the same, and the code must survive restarts), but it has
    /// consequences: never log it, never include it in a diagnostics export, and
    /// never send the full value to the frontend — see `cmd::chrysalis`.
    pub activation_code: Option<String>,

    /// The tier the hub granted (`free`/`eco` for the free plan, `strike` for
    /// the paid one). A **frozen wire name**: the hub resolves it against
    /// `tier_configs`, so it is never renamed to match what the UI displays.
    /// The student-visible label lives in one place (`tier-badge.tsx`).
    pub chrysalis_tier: Option<String>,

    /// The device fingerprint this code is bound to.
    pub device_fingerprint: Option<String>,

    /// Unix seconds of the last successful heartbeat. `None` (or 0) means the
    /// device has never had one, which the grace period treats as "full window
    /// from first launch" rather than "expired".
    pub last_heartbeat_ok: Option<i64>,

    /// Consecutive heartbeat failures, used to compute the backoff. Persisted so
    /// a device that restarts repeatedly during an outage does not reset to
    /// beating every 5 minutes and hammer the hub.
    pub heartbeat_failures: Option<u32>,

    /// A version the hub has offered, remembered so the UI can still show it
    /// after the heartbeat that carried it has been forgotten.
    pub update_pending_version: Option<String>,

    /// The checksum for that pending version, so a retry does not have to
    /// re-derive it (and cannot silently retry with a different one).
    pub update_pending_sha256: Option<String>,

    /// The tier's connection details, as JSON.
    ///
    /// Stored as a serialised blob rather than seven more fields because it
    /// arrives as one payload, is replaced as one payload, and is meaningless
    /// in pieces. A schema change on the hub side then cannot leave a partially
    /// populated config behind.
    pub chrysalis_tier_server: Option<String>,

    /// Whether this tier should carry UDP over TCP.
    pub chrysalis_udp_relay: Option<bool>,

    /// An override for the hub's base URL, when the built-in one cannot be
    /// reached from this network.
    ///
    /// # Why this exists
    ///
    /// The hub address is compiled in (`chrysalis::contract::HUB_URL`), which is
    /// correct — a client that cannot find its hub cannot activate, heartbeat or
    /// update, so the address must not be a per-machine guess. But it is also the
    /// one value that can become *entirely* unreachable for reasons that have
    /// nothing to do with us: a school network that classifies the hostname as
    /// uncategorised refuses the DNS/TLS the whole product depends on. N4L does
    /// exactly that to `*.duckdns.org`, and it did so to this project.
    ///
    /// When that happens the compiled address is unreachable *and so is the
    /// update path*, because the update path is served from the same host — the
    /// failure prevents escaping the failure. This field is the escape hatch: a
    /// hub move becomes a stored value the next launch honours, with no release
    /// required, so a stranded fleet can be repointed by one operator action
    /// rather than by shipping a build it cannot download.
    ///
    /// # Rules for the value
    ///
    ///   * **Absolute and https**, or it is ignored. A relative value, a bare
    ///     hostname or an `http://` URL is refused by
    ///     `contract::resolve_hub_url` — an override that cannot be trusted to be
    ///     a hub URL is worse than the compiled default.
    ///   * **A trailing slash is stripped**, so `format!("{hub}/api/...")` cannot
    ///     produce a double slash.
    ///   * **Empty means "use the built-in"**, which is the state of every device
    ///     that has never needed this.
    ///
    /// Deliberately *not* settable from the activation screen: a student typing a
    /// hub address is a phishing surface and a support nightmare. It is set by an
    /// operator (config file, or a future console-driven push on an authenticated
    /// path), which is also why the value alone can never degrade the compiled
    /// default into something insecure.
    pub chrysalis_hub_override: Option<String>,

    /// When this code lapses, exactly as the hub last told us.
    ///
    /// Stored raw rather than as a timestamp so the value the UI displays is the
    /// value the hub sent: re-serialising here would mean the client and the
    /// admin console could disagree about the same code by a timezone. Parsing
    /// happens at display time in [`crate::chrysalis::expiry`], which is also where
    /// the "do not invent a date" rule lives.
    ///
    /// Refreshed by every heartbeat, so it stays accurate for as long as the app
    /// runs — unlike the activation-time value, which was all we had before and
    /// went stale the moment a subscription renewed.
    ///
    /// `None` means the hub told us nothing (older hub, or no expiry recorded).
    /// It does NOT mean expired, and the UI must render nothing.
    pub chrysalis_expires_at: Option<String>,

    /// Why the hub last refused this device, when it refused.
    ///
    /// This is the missing half of a refusal. `store::clear()` removes the
    /// entitlement — correctly, because a refused device must not keep a tunnel
    /// up on a stale record — but clearing the code also erases every trace of
    /// *why*. The student was left with an app that had silently deactivated
    /// itself, which reads as "the app broke" rather than "your subscription
    /// ended" or "an operator suspended this code". Those need different actions
    /// from the student, so they cannot share one silent outcome.
    ///
    /// Stored because the refusal arrives from a background heartbeat, at a
    /// moment nobody is looking, and must still be explainable when the student
    /// next opens the window. It survives the clear on purpose: it is the record
    /// of an event, not part of the entitlement.
    ///
    /// Cleared on a successful activation, so a renewed student does not keep
    /// seeing the reason their old code was refused.
    pub chrysalis_refusal_reason: Option<String>,

    /// Whether an operator has suspended this code, with the credential kept.
    ///
    /// Distinct from `chrysalis_refusal_reason` being set, which covers every
    /// refusal. This is the one refusal that is REVERSIBLE: the code, tier and
    /// expiry are all still stored, so clearing this flag restores the session
    /// without the student re-entering anything. See
    /// `chrysalis::store::record_suspension` for the trade that makes it safe.
    ///
    /// `None` (not `false`) when absent, so an older config file is "not
    /// suspended" rather than needing a migration.
    ///
    /// **It is only meaningful while a code is held.** A stored `true` with no
    /// activation behind it used to render the kick-out screen for a device that
    /// was not signed in — a screen with no controls, by design — and the only
    /// way out was wiping the app's data. The writers keep the pair consistent
    /// (`chrysalis::store::clear_entitlement` clears this) and
    /// `chrysalis::store::is_suspended` refuses the impossible pair on read, so the
    /// UI never has to defend against it.
    pub chrysalis_suspended: Option<bool>,

    /// Which version of the hub's enforcement logic last answered this device.
    ///
    /// **This is how a stale hub hook becomes visible instead of mysterious.**
    /// `heartbeat.pb.js` sends `enforcement_version`, and the hooks that live on
    /// the box are deployed from `/root/server/` rather than from this repository
    /// — so "is the kick-out fix actually live?" was a question only someone with
    /// shell access could answer. A suspension the hub never enforces looks
    /// exactly like a suspension that was never set.
    ///
    /// Recorded on every successful beat. `None` means no beat has delivered a
    /// value yet; a value BELOW [`crate::chrysalis::contract::ENFORCEMENT_VERSION`]
    /// means the hub predates this client, so the client's enforcement features
    /// (suspension) cannot work no matter what the console shows. That is a
    /// deployment fact, not a student's problem, and saying so plainly is the
    /// difference between a support call and a support call fixable in one line.
    ///
    /// The number never gates anything client-side: refusing service to a student
    /// because the operator's box is behind would be a worse defect than the one
    /// it reports.
    pub chrysalis_hub_enforcement: Option<u32>,

    /// The version the hub has offered this device, if any.
    ///
    /// Persisted rather than kept in memory because the offer arrives on a
    /// heartbeat, at an arbitrary moment, and the prompt has to be able to appear
    /// on the next launch — a student who is offered an update and closes the app
    /// must not lose the offer. It is the *only* thing needed to show the prompt;
    /// the download URL, checksum and signature are re-fetched from
    /// `/api/update` at install time, so nothing security-relevant is cached here.
    ///
    /// Cleared once the update is installed (or the version is no longer newer),
    /// so a stale offer cannot be shown forever.
    ///
    /// `None` means nothing has been offered. It does NOT mean "up to date" — the
    /// hub may simply not have been reached yet.
    pub chrysalis_update_offered: Option<String>,

    /// Why the last heartbeat did not produce an update offer, if it did not.
    ///
    /// A human-readable sentence derived from
    /// [`crate::chrysalis::update::NoOfferReason`], shown on the Account page so a
    /// student (or support) can answer "why is this device not updating?"
    /// without reading a log.
    ///
    /// `None` means either the last beat offered an update, or none has run yet.
    /// Both are "no complaint to make", which is why they share a value: the
    /// question this field answers is only ever asked when nothing appeared.
    ///
    /// Written on EVERY beat, including the healthy case (to `None`), so a
    /// problem that is fixed stops being reported. A stale reason that outlived
    /// its cause would send a student to change a setting that is already right.
    pub chrysalis_update_check_reason: Option<String>,

    // ── The free tier's usage window ─────────────────────────────────────────
    //
    // The allowance is counted CLIENT-SIDE, because the hub has no per-user
    // accounting (one shared shadowsocks password per tier). These three fields
    // are that counter's persisted state; the arithmetic lives in
    // [`crate::chrysalis::usage`], which is pure and fake-clock tested.
    //
    // Stored as a serialised [`crate::chrysalis::usage::Usage`] rather than three
    // loose fields on purpose: the counter is one value with internal
    // consistency (a count is meaningless without the window it belongs to), and
    // splitting it across keys is how a half-written window happens — a new
    // start with the old count, which is a free allowance reset.
    /// The free-tier usage window, serialised JSON.
    ///
    /// `None` for a paying tier, or a client that has never recorded a byte.
    /// Absence means "no window yet", which [`crate::chrysalis::usage::Usage`] starts
    /// lazily on the first reading — not "zero used", which would report a
    /// full allowance to a student mid-window.
    pub chrysalis_usage: Option<String>,

    /// The allowance the hub last advertised, in **mebibytes**.
    ///
    /// Cached from the heartbeat so a status read can classify usage without a
    /// live beat — the Connection screen polls `chrysalis_status` far more often than
    /// the client beats. This is the *server's* value, refreshed on every beat;
    /// changing it on the hub takes effect on the student's next beat without a
    /// release, which is the whole reason it is a wire field.
    ///
    /// `None` means the hub has never sent one — a paying tier, or an older hub.
    /// It reads as "no allowance", never as zero, so it cannot throttle anyone.
    pub chrysalis_allowance_mb: Option<u64>,

    /// The speed a throttled free student drops to, in Mbps, as the hub last
    /// advertised it (`free_throttle_mbps`).
    ///
    /// Stored with the allowance because the two are one policy: `allowance`
    /// says how much data, this says how slow once it is gone.
    ///
    /// **Persisted but not yet applied.** Nothing lowers a live Core's speed
    /// from this value yet; see the note on [`crate::chrysalis::store::throttle_mbps`]
    /// and `STILL-OPEN.md`. It is stored now so the value survives a restart and
    /// the applying step has something to read when it lands.
    ///
    /// `None` means the hub did not send one, and the caller must apply **no**
    /// throttle — absence is not "zero speed".
    pub chrysalis_throttle_mbps: Option<u32>,

    // ── Retired: device identity and the recognition token ───────────────────
    //
    // These five fields belonged to device recognition — a durable device
    // identity the hub matched on to skip the code prompt, and the session token
    // it minted for such a device. Both are gone (see `server/pb_hooks/`), and
    // nothing reads or writes these any more.
    //
    // They are KEPT as `Option<T>` fields rather than deleted, because a
    // deserializer that meets an unknown key in an existing `verge.yaml` must not
    // fail: removing the fields would make an upgrade from a build that wrote
    // them either error or silently discard the rest of the config. They are
    // inert, and a later cleanup can drop them once no shipped build writes them.
    /// Retired. The device's non-secret identifier.
    pub chrysalis_device_id: Option<String>,

    /// Retired. The device's secret. Never read; if a build ever did, the
    /// never-log/never-export rules would still apply.
    pub chrysalis_device_secret: Option<String>,

    /// Retired. Where the identity was found: `"machine"` or `"app"`.
    pub chrysalis_identity_store: Option<String>,

    /// Retired. The session token the hub minted at recognition.
    pub chrysalis_device_token: Option<String>,

    /// Retired. When that token expired.
    pub chrysalis_token_expires_at: Option<String>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IVergeTestItem {
    pub uid: Option<String>,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub url: Option<String>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IVergeTheme {
    pub primary_color: Option<String>,
    pub secondary_color: Option<String>,
    pub primary_text: Option<String>,
    pub secondary_text: Option<String>,

    pub info_color: Option<String>,
    pub error_color: Option<String>,
    pub warning_color: Option<String>,
    pub success_color: Option<String>,

    pub font_family: Option<String>,
    pub css_injection: Option<String>,
}

impl IVerge {
    pub const VALID_CLASH_CORES: &'static [&'static str] = &["verge-mihomo", "verge-mihomo-alpha"];

    pub async fn validate_and_fix_config() -> Result<()> {
        let config_path = dirs::verge_path()?;
        let mut config = match help::read_yaml::<Self>(&config_path).await {
            Ok(config) => config,
            Err(_) => Self::template(),
        };

        let mut needs_fix = false;

        if let Some(ref core) = config.clash_core {
            let core_str = core.trim();
            if core_str.is_empty() || !Self::VALID_CLASH_CORES.contains(&core_str) {
                logging!(
                    warn,
                    Type::Config,
                    "启动时发现无效的clash_core配置: '{}', 将自动修正为 'verge-mihomo'",
                    core
                );
                config.clash_core = Some("verge-mihomo".into());
                needs_fix = true;
            }
        } else {
            logging!(
                info,
                Type::Config,
                "启动时发现未配置clash_core, 将设置为默认值 'verge-mihomo'"
            );
            config.clash_core = Some("verge-mihomo".into());
            needs_fix = true;
        }

        if needs_fix {
            logging!(debug, Type::Config, "正在保存修正后的配置文件...");
            help::save_yaml(&config_path, &config, Some("# Chrysalis Config")).await?;
            logging!(info, Type::Config, "配置文件修正完成，需要重新加载配置");

            Self::reload_config_after_fix(config).await;
        } else {
            logging!(debug, Type::Config, "clash_core配置验证通过: {:?}", config.clash_core);
        }

        Ok(())
    }

    async fn reload_config_after_fix(updated_config: Self) {
        logging!(
            info,
            Type::Config,
            "内存配置已强制更新，新的clash_core: {:?}",
            &updated_config.clash_core
        );

        let config_draft = Config::verge().await;
        config_draft.edit_draft(|d| {
            *d = updated_config;
        });
        config_draft.apply();
    }

    pub fn get_valid_clash_core(&self) -> String {
        self.clash_core.clone().unwrap_or_else(|| "verge-mihomo".into())
    }

    pub async fn new() -> Self {
        match dirs::verge_path() {
            Ok(path) => match help::read_yaml::<Self>(&path).await {
                Ok(mut config) => {
                    if let Some(start_page) = config.start_page.clone()
                        && start_page == "/home"
                    {
                        config.start_page = Some(String::from("/"));
                    }
                    config
                }
                Err(err) => {
                    logging!(error, Type::Config, "failed to read verge config: {err:#}");
                    Self::template()
                }
            },
            Err(err) => {
                logging!(error, Type::Config, "failed to get verge config path: {err:#}");
                Self::template()
            }
        }
    }

    pub fn template() -> Self {
        Self {
            app_log_max_size: Some(128),
            app_log_max_count: Some(8),
            clash_core: Some("verge-mihomo".into()),
            language: Some(clash_verge_i18n::system_language().into()),
            #[cfg(not(target_os = "windows"))]
            env_type: Some("bash".into()),
            #[cfg(target_os = "windows")]
            env_type: Some("powershell".into()),
            start_page: Some("/".into()),
            traffic_graph: Some(true),
            enable_memory_usage: Some(true),
            enable_group_icon: Some(true),
            pause_render_traffic_stats_on_blur: Some(true),
            #[cfg(target_os = "macos")]
            tray_icon: Some("monochrome".into()),
            menu_icon: Some("monochrome".into()),
            notice_position: Some("top-right".into()),
            collapse_navbar: Some(false),
            common_tray_icon: Some(false),
            sysproxy_tray_icon: Some(false),
            tun_tray_icon: Some(false),
            enable_auto_launch: Some(false),
            enable_silent_start: Some(false),
            enable_hover_jump_navigator: Some(true),
            hover_jump_navigator_delay: Some(280),
            enable_system_proxy: Some(false),
            proxy_auto_config: Some(false),
            pac_file_content: Some(DEFAULT_PAC.into()),
            proxy_host: Some("127.0.0.1".into()),
            #[cfg(not(target_os = "windows"))]
            verge_redir_port: Some(7895),
            #[cfg(not(target_os = "windows"))]
            verge_redir_enabled: Some(false),
            #[cfg(target_os = "linux")]
            verge_tproxy_port: Some(7896),
            #[cfg(target_os = "linux")]
            verge_tproxy_enabled: Some(false),
            verge_mixed_port: Some(network::ports::DEFAULT_MIXED),
            verge_socks_port: Some(7898),
            verge_socks_enabled: Some(false),
            verge_port: Some(7899),
            verge_http_enabled: Some(false),
            enable_proxy_guard: Some(false),
            enable_bypass_check: Some(true),
            use_default_bypass: Some(true),
            proxy_guard_duration: Some(30),
            auto_close_connection: Some(true),
            auto_check_update: Some(true),
            enable_builtin_enhanced: Some(true),
            auto_log_clean: Some(2), // 1: 1天, 2: 7天, 3: 30天, 4: 90天
            enable_auto_backup_schedule: Some(false),
            auto_backup_interval_hours: Some(24),
            auto_backup_on_change: Some(true),
            webdav_url: None,
            webdav_username: None,
            webdav_password: None,
            #[cfg(target_os = "macos")]
            enable_tray_speed: Some(false),
            tray_proxy_groups_display_mode: Some("default".into()),
            tray_inline_outbound_modes: Some(false),
            enable_global_hotkey: Some(true),
            enable_auto_light_weight_mode: Some(false),
            auto_light_weight_minutes: Some(10),
            enable_dns_settings: Some(false),
            home_cards: None,
            enable_external_controller: Some(false),
            ..Self::default()
        }
    }

    pub async fn save_file(&self) -> Result<()> {
        help::save_yaml(&dirs::verge_path()?, &self, Some("# Chrysalis Config")).await
    }

    #[allow(clippy::cognitive_complexity)]
    pub fn patch_config(&mut self, patch: &Self) {
        macro_rules! patch {
            ($key: tt) => {
                if patch.$key.is_some() {
                    self.$key = patch.$key.clone();
                }
            };
        }

        patch!(app_log_level);
        patch!(app_log_max_size);
        patch!(app_log_max_count);

        patch!(language);
        patch!(tray_event);
        patch!(env_type);
        patch!(start_page);
        patch!(startup_script);
        patch!(traffic_graph);
        patch!(enable_memory_usage);
        patch!(enable_group_icon);
        patch!(pause_render_traffic_stats_on_blur);
        #[cfg(target_os = "macos")]
        patch!(tray_icon);
        patch!(menu_icon);
        patch!(menu_order);
        patch!(proxy_group_tools_position);
        patch!(notice_position);
        patch!(collapse_navbar);
        patch!(common_tray_icon);
        patch!(sysproxy_tray_icon);
        patch!(tun_tray_icon);

        patch!(enable_tun_mode);
        patch!(enable_auto_launch);
        patch!(enable_silent_start);
        patch!(enable_hover_jump_navigator);
        patch!(hover_jump_navigator_delay);
        #[cfg(not(target_os = "windows"))]
        patch!(verge_redir_port);
        #[cfg(not(target_os = "windows"))]
        patch!(verge_redir_enabled);
        #[cfg(target_os = "linux")]
        patch!(verge_tproxy_port);
        #[cfg(target_os = "linux")]
        patch!(verge_tproxy_enabled);
        patch!(verge_mixed_port);
        patch!(verge_socks_port);
        patch!(verge_socks_enabled);
        patch!(verge_port);
        patch!(verge_http_enabled);
        patch!(enable_system_proxy);
        patch!(enable_proxy_guard);
        patch!(enable_bypass_check);
        patch!(use_default_bypass);
        patch!(system_proxy_bypass);
        patch!(proxy_guard_duration);
        patch!(proxy_auto_config);
        patch!(pac_file_content);
        patch!(proxy_host);
        patch!(theme_setting);
        patch!(web_ui_list);
        patch!(clash_core);
        patch!(hotkeys);
        patch!(enable_global_hotkey);

        patch!(auto_close_connection);
        patch!(auto_check_update);
        patch!(default_latency_test);
        patch!(default_latency_timeout);
        patch!(enable_auto_delay_detection);
        patch!(auto_delay_detection_interval_minutes);
        patch!(enable_builtin_enhanced);
        patch!(proxy_layout_column);
        patch!(test_list);
        patch!(auto_log_clean);
        patch!(enable_auto_backup_schedule);
        patch!(auto_backup_interval_hours);
        patch!(auto_backup_on_change);

        patch!(webdav_url);
        patch!(webdav_username);
        patch!(webdav_password);
        #[cfg(target_os = "macos")]
        patch!(enable_tray_speed);
        patch!(tray_proxy_groups_display_mode);
        patch!(tray_inline_outbound_modes);
        patch!(enable_auto_light_weight_mode);
        patch!(auto_light_weight_minutes);
        patch!(enable_dns_settings);
        patch!(home_cards);
        patch!(enable_external_controller);
    }

    pub fn get_log_level(&self) -> LevelFilter {
        if let Some(level) = self.app_log_level.as_ref() {
            match level.to_lowercase().as_str() {
                "silent" => LevelFilter::Off,
                "error" => LevelFilter::Error,
                "warn" => LevelFilter::Warn,
                "info" => LevelFilter::Info,
                "debug" => LevelFilter::Debug,
                "trace" => LevelFilter::Trace,
                _ => LevelFilter::Info,
            }
        } else {
            LevelFilter::Info
        }
    }
}
