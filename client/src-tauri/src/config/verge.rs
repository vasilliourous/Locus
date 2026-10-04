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

    /// `light` or `dark` or `system`
    pub theme_mode: Option<String>,

    /// The selected named theme, e.g. `default-dark` or `midnight`.
    ///
    /// Deliberately a plain `String` and NOT an enum with serde variants: an id
    /// this build does not know must survive a round-trip rather than fail to
    /// deserialise, because the registry is the client's and a file can be edited
    /// by hand. An unknown or absent id is resolved to the default theme by
    /// `resolveTheme` in `src/pages/_themes.ts` -- never an error.
    ///
    /// `None` means "no theme selected", which is every install upgrading from a
    /// version before themes existed, and it behaves exactly as `theme_mode`
    /// alone did. See `docs/reference/THEMES.md` section 2.
    pub theme_id: Option<String>,

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

    // ── Locus product state ──────────────────────────────────────────────────
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
    /// never send the full value to the frontend — see `cmd::locus`.
    pub activation_code: Option<String>,

    /// The tier the hub granted (`eco`, `stealth`, `strike`).
    pub locus_tier: Option<String>,

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
    pub locus_tier_server: Option<String>,

    /// Whether this tier should carry UDP over TCP.
    pub locus_udp_relay: Option<bool>,

    /// When this code lapses, exactly as the hub last told us.
    ///
    /// Stored raw rather than as a timestamp so the value the UI displays is the
    /// value the hub sent: re-serialising here would mean the client and the
    /// admin console could disagree about the same code by a timezone. Parsing
    /// happens at display time in [`crate::locus::expiry`], which is also where
    /// the "do not invent a date" rule lives.
    ///
    /// Refreshed by every heartbeat, so it stays accurate for as long as the app
    /// runs — unlike the activation-time value, which was all we had before and
    /// went stale the moment a subscription renewed.
    ///
    /// `None` means the hub told us nothing (older hub, or no expiry recorded).
    /// It does NOT mean expired, and the UI must render nothing.
    pub locus_expires_at: Option<String>,

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
    pub locus_refusal_reason: Option<String>,

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
    pub locus_update_offered: Option<String>,

    /// Why the last heartbeat did not produce an update offer, if it did not.
    ///
    /// A human-readable sentence derived from
    /// [`crate::locus::update::NoOfferReason`], shown on the Account page so a
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
    pub locus_update_check_reason: Option<String>,

    // ── The free tier's usage window ─────────────────────────────────────────
    //
    // The allowance is counted CLIENT-SIDE, because the hub has no per-user
    // accounting (one shared shadowsocks password per tier). These three fields
    // are that counter's persisted state; the arithmetic lives in
    // [`crate::locus::usage`], which is pure and fake-clock tested.
    //
    // Stored as a serialised [`crate::locus::usage::Usage`] rather than three
    // loose fields on purpose: the counter is one value with internal
    // consistency (a count is meaningless without the window it belongs to), and
    // splitting it across keys is how a half-written window happens — a new
    // start with the old count, which is a free allowance reset.

    /// The free-tier usage window, serialised JSON.
    ///
    /// `None` for a paying tier, or a client that has never recorded a byte.
    /// Absence means "no window yet", which [`crate::locus::usage::Usage`] starts
    /// lazily on the first reading — not "zero used", which would report a
    /// full allowance to a student mid-window.
    pub locus_usage: Option<String>,

    /// The allowance the hub last advertised, in **mebibytes**.
    ///
    /// Cached from the heartbeat so a status read can classify usage without a
    /// live beat — the Connection screen polls `locus_status` far more often than
    /// the client beats. This is the *server's* value, refreshed on every beat;
    /// changing it on the hub takes effect on the student's next beat without a
    /// release, which is the whole reason it is a wire field.
    ///
    /// `None` means the hub has never sent one — a paying tier, or an older hub.
    /// It reads as "no allowance", never as zero, so it cannot throttle anyone.
    pub locus_allowance_mb: Option<u64>,

    /// The speed a throttled free student drops to, in Mbps, as the hub last
    /// advertised it (`free_throttle_mbps`).
    ///
    /// Stored with the allowance because the two are one policy: `allowance`
    /// says how much data, this says how slow once it is gone.
    ///
    /// **Persisted but not yet applied.** Nothing lowers a live Core's speed
    /// from this value yet; see the note on [`crate::locus::store::throttle_mbps`]
    /// and `STILL-OPEN.md`. It is stored now so the value survives a restart and
    /// the applying step has something to read when it lands.
    ///
    /// `None` means the hub did not send one, and the caller must apply **no**
    /// throttle — absence is not "zero speed".
    pub locus_throttle_mbps: Option<u32>,

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
    pub locus_device_id: Option<String>,

    /// Retired. The device's secret. Never read; if a build ever did, the
    /// never-log/never-export rules would still apply.
    pub locus_device_secret: Option<String>,

    /// Retired. Where the identity was found: `"machine"` or `"app"`.
    pub locus_identity_store: Option<String>,

    /// Retired. The session token the hub minted at recognition.
    pub locus_device_token: Option<String>,

    /// Retired. When that token expired.
    pub locus_token_expires_at: Option<String>,
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
            help::save_yaml(&config_path, &config, Some("# Locus Config")).await?;
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
            theme_mode: Some("system".into()),
            // No `theme_id` in the template on purpose: a fresh install uses
            // `theme_mode`, and the resolver falls back to `default-dark` when no
            // theme is selected. Writing a default here would make the field
            // non-`None` on every install and blur the "unset means legacy
            // behaviour" property the upgrade path depends on.

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
        help::save_yaml(&dirs::verge_path()?, &self, Some("# Locus Config")).await
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
        patch!(theme_mode);
        patch!(theme_id);
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
