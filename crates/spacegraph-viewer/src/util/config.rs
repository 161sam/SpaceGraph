use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ViewerViewMode {
    #[default]
    Spatial,
    Tree,
    Timeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LodEdgesMode {
    Off,
    #[default]
    FocusOnly,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    #[default]
    User,
    Privileged,
}

impl AgentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Privileged => "privileged",
        }
    }
}

/// Visual theme selection. `Standard` is the neon "Ghost in the Shell" look
/// (HDR + bloom, per-type emissive, fog, grid); `Minimal` is the flat,
/// accessibility/perf fallback (no bloom, plain materials) matching the
/// pre-visual-pass behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VisualTheme {
    #[default]
    Standard,
    Minimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentEndpointKind {
    UdsPath(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentEndpoint {
    pub name: String,
    pub kind: AgentEndpointKind,
    pub auto_connect: bool,
    pub mode_override: Option<AgentMode>,
}

impl Default for AgentEndpoint {
    fn default() -> Self {
        Self {
            name: "local".to_string(),
            kind: AgentEndpointKind::UdsPath(default_uds_path()),
            auto_connect: true,
            mode_override: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathPolicyConfig {
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

/// Cyberspace post-process intensities (Standard theme). Effective only when the
/// theme is Standard and `enabled` (see `render::postfx::postfx_active`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PostFxConfig {
    pub enabled: bool,
    pub scanline: f32,
    pub vignette: f32,
    pub aberration: f32,
    pub grain: f32,
}

impl Default for PostFxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            scanline: 0.12,
            vignette: 0.35,
            aberration: 0.4,
            grain: 0.06,
        }
    }
}

/// Node-detail (v0.4.1) config block. Drives the two-level node detail: Level-1
/// face icons (all nodes) and Level-2 focused-node previews. Clamped at runtime
/// to the detected `DetailCapability` (`render::capability::resolve_detail`).
/// `level` overrides auto-detection ("low" / "mid" / "high"; `None` = auto).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeDetailConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    pub max_preview_panels: usize,
    pub thumbnail_px: u32,
    pub max_image_bytes: usize,
    pub max_text_bytes: usize,
    pub enable_image: bool,
    pub enable_video_card: bool,
}

impl Default for NodeDetailConfig {
    fn default() -> Self {
        Self {
            level: None,
            max_preview_panels: 3,
            thumbnail_px: 256,
            max_image_bytes: 2 * 1024 * 1024,
            max_text_bytes: 256 * 1024,
            enable_image: true,
            enable_video_card: true,
        }
    }
}

/// IDE-shell (v0.5.0, spec §3.2 / §6) layout persistence. Native egui panels —
/// no docking crate. Widths in logical px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub left_open: bool,
    pub left_width: f32,
    pub right_open: bool,
    pub right_width: f32,
    pub bottom_open: bool,
    /// The collapsible "Technician" tuning section (collapsed by default).
    pub technician_open: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            left_open: true,
            left_width: 290.0,
            right_open: true,
            right_width: 320.0,
            bottom_open: false,
            technician_open: false,
        }
    }
}

/// Quality-tier (v0.5.0, spec §2.7) config block. `tier` =
/// `auto`|`potato`|`low`|`medium`|`high` (auto = detect from the GPU adapter);
/// `adaptive` toggles the runtime FPS-feedback tier stepping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityConfig {
    pub tier: String,
    pub adaptive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_fps_override: Option<u32>,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            tier: "auto".to_string(),
            adaptive: true,
            target_fps_override: None,
        }
    }
}

/// Focus Mode (v0.5.1) presentation. `dim` = background dim strength (0..1) applied
/// on **all tiers** when a node is focused; `dof` enables the High-tier
/// depth-of-field blur (an enhancement — **deferred** in v0.5.1: dim-only ships,
/// see RUNLOG); `freeze_layout` freezes the force layout while focused (calmer +
/// cheaper; reversible, determinism-exempt).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusConfig {
    pub dim: f32,
    pub dof: bool,
    pub freeze_layout: bool,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            dim: 0.62,
            dof: false, // High-tier DoF deferred in v0.5.1 (dim-only ships)
            freeze_layout: true,
        }
    }
}

/// Edge level-of-detail (v0.5.1) — render-side edge thinning to cut overdraw /
/// bloom (the FPS lever). Distant edges **dim** past `near_dist` and **cull** past
/// `far_dist`; in Focus Mode, edges not incident to the focused node are culled
/// when `focus_cull`. Purely render-side: `force_step` (layout truth) is untouched.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EdgeLodConfig {
    /// Edges with a midpoint nearer than this render at full brightness.
    pub near_dist: f32,
    /// Edges with a midpoint farther than this are culled (not drawn).
    pub far_dist: f32,
    /// Brightness multiplier for edges in the dim band `(near_dist, far_dist]`.
    pub far_dim: f32,
    /// Cull edges not incident to the focused node while Focus Mode is active.
    pub focus_cull: bool,
}

impl Default for EdgeLodConfig {
    fn default() -> Self {
        Self {
            near_dist: 70.0,
            far_dist: 160.0,
            far_dim: 0.35,
            focus_cull: true,
        }
    }
}

/// Perimeter & exposure visual toggles (D0, ADR-0012). `aperture_by_state` and
/// `anomaly_focus` are Standard-only render cues; `exposure_depth` is
/// informational placement that applies in both themes. All derive from data
/// already on the wire — no `spacegraph-core` change.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SocketDisplayConfig {
    /// Render the socket aperture form per port state (LISTEN/ESTABLISHED/…).
    pub aperture_by_state: bool,
    /// Place sockets on a radial shell by exposure (Public outer … Loopback core).
    pub exposure_depth: bool,
    /// Localize the post-fx around the most severe/recent alerts.
    pub anomaly_focus: bool,
    /// Strength of the anomaly-focus distortion (0..=1).
    pub anomaly_intensity: f32,
}

impl Default for SocketDisplayConfig {
    fn default() -> Self {
        Self {
            aperture_by_state: true,
            exposure_depth: true,
            anomaly_focus: true,
            anomaly_intensity: 0.6,
        }
    }
}

/// Filesystem search (v0.5.2, spec §7). `full_system` (D-2) opts into the
/// system-wide scope; `result_limit` caps the hits requested; `debounce_ms` is
/// the search-box debounce before a query is sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub full_system: bool,
    pub result_limit: u32,
    pub debounce_ms: u64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            full_system: false,
            result_limit: 200,
            debounce_ms: 120,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewerConfig {
    pub view_mode: ViewerViewMode,
    pub show_3d: bool,
    pub show_edges: bool,
    pub show_raw_edges: bool,
    pub show_agg_edges: bool,
    pub demo_mode: bool,
    pub path_includes: Vec<String>,
    pub path_excludes: Vec<String>,
    pub focus_hops: usize,
    pub max_visible_nodes: usize,
    pub progressive_nodes_per_frame: usize,
    #[serde(default = "default_max_visible_alerts")]
    pub max_visible_alerts: usize,
    pub layout_force: bool,
    pub link_distance: f32,
    pub repulsion: f32,
    pub repulsion_radius: f32,
    pub damping: f32,
    pub max_step: f32,
    pub layout_budget_ms: f32,
    pub timeline_window_secs: u64,
    pub timeline_scale: f32,
    pub lod_enabled: bool,
    pub lod_threshold_nodes: usize,
    pub lod_edges_mode: LodEdgesMode,
    pub glow_duration_ms: u64,
    pub gc_enabled: bool,
    pub gc_ttl_secs: u64,
    pub default_agent_mode: AgentMode,
    #[serde(default)]
    pub visual_theme: VisualTheme,
    #[serde(default)]
    pub fog_of_war: bool,
    // ---- Gameplay / exploration ----
    #[serde(default = "default_reveal_radius")]
    pub reveal_radius: f32,
    #[serde(default = "default_scan_speed")]
    pub scan_speed: f32,
    #[serde(default = "default_scan_max")]
    pub scan_max: f32,
    #[serde(default = "default_fly_speed")]
    pub fly_speed: f32,
    #[serde(default = "default_fly_boost")]
    pub fly_boost: f32,
    #[serde(default = "default_fly_sensitivity")]
    pub fly_sensitivity: f32,
    // ---- In-world labels + detail (Standard theme) ----
    #[serde(default = "default_micro_tags")]
    pub micro_tags: bool,
    #[serde(default = "default_micro_tag_max")]
    pub micro_tag_max: usize,
    #[serde(default = "default_node_rings")]
    pub node_rings: bool,
    #[serde(default = "default_ring_min_degree")]
    pub ring_min_degree: usize,
    /// Edge-pick hit threshold in world units (ray-vs-segment distance).
    #[serde(default = "default_edge_pick_threshold")]
    pub edge_pick_threshold: f32,
    #[serde(default)]
    pub postfx: PostFxConfig,
    // ---- Node detail (v0.4.1): face icons + focused-node previews ----
    #[serde(default)]
    pub node_detail: NodeDetailConfig,
    // ---- Quality tier (v0.5.0): GPU-cost axis, Pi → desktop ----
    #[serde(default)]
    pub quality: QualityConfig,
    // ---- Edge LOD (v0.5.1): render-side edge thinning (overdraw/bloom lever) ----
    #[serde(default)]
    pub edge_lod: EdgeLodConfig,
    pub socket_display: SocketDisplayConfig,
    // ---- Focus Mode (v0.5.1): dim/DoF/layout-freeze for the centred node ----
    #[serde(default)]
    pub focus: FocusConfig,
    // ---- IDE shell (v0.5.0): native-panel layout persistence ----
    #[serde(default)]
    pub shell: ShellConfig,
    // ---- Audio (effective only in builds with the `audio` feature) ----
    #[serde(default = "default_audio_enabled")]
    pub audio_enabled: bool,
    #[serde(default = "default_audio_volume")]
    pub audio_volume: f32,
    #[serde(default = "default_agents")]
    pub agents: Vec<AgentEndpoint>,
    // ---- Filesystem search (v0.5.2): on-disk index query + materialise ----
    #[serde(default)]
    pub search: SearchConfig,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            view_mode: ViewerViewMode::Spatial,
            show_3d: true,
            show_edges: true,
            show_raw_edges: false,
            show_agg_edges: true,
            demo_mode: false,
            path_includes: vec!["/etc".to_string(), "/home".to_string(), "/var".to_string()],
            path_excludes: vec![
                "/proc".to_string(),
                "/sys".to_string(),
                "/dev".to_string(),
                "/run".to_string(),
            ],
            focus_hops: 2,
            max_visible_nodes: 3000,
            progressive_nodes_per_frame: 250,
            max_visible_alerts: default_max_visible_alerts(),
            layout_force: true,
            link_distance: 6.0,
            repulsion: 400.0,
            repulsion_radius: 8.0,
            damping: 0.92,
            max_step: 0.35,
            layout_budget_ms: 6.0,
            timeline_window_secs: 60,
            timeline_scale: 0.35,
            lod_enabled: true,
            lod_threshold_nodes: 2500,
            lod_edges_mode: LodEdgesMode::FocusOnly,
            glow_duration_ms: 900,
            gc_enabled: true,
            gc_ttl_secs: 30,
            default_agent_mode: AgentMode::User,
            visual_theme: VisualTheme::Standard,
            fog_of_war: false,
            reveal_radius: default_reveal_radius(),
            scan_speed: default_scan_speed(),
            scan_max: default_scan_max(),
            fly_speed: default_fly_speed(),
            fly_boost: default_fly_boost(),
            fly_sensitivity: default_fly_sensitivity(),
            micro_tags: default_micro_tags(),
            micro_tag_max: default_micro_tag_max(),
            node_rings: default_node_rings(),
            ring_min_degree: default_ring_min_degree(),
            edge_pick_threshold: default_edge_pick_threshold(),
            postfx: PostFxConfig::default(),
            node_detail: NodeDetailConfig::default(),
            quality: QualityConfig::default(),
            edge_lod: EdgeLodConfig::default(),
            socket_display: SocketDisplayConfig::default(),
            focus: FocusConfig::default(),
            shell: ShellConfig::default(),
            audio_enabled: default_audio_enabled(),
            audio_volume: default_audio_volume(),
            agents: vec![AgentEndpoint::default()],
            search: SearchConfig::default(),
        }
    }
}

fn default_reveal_radius() -> f32 {
    55.0
}
fn default_scan_speed() -> f32 {
    70.0
}
fn default_scan_max() -> f32 {
    500.0
}
fn default_fly_speed() -> f32 {
    24.0
}
fn default_fly_boost() -> f32 {
    4.0
}
fn default_fly_sensitivity() -> f32 {
    0.0025
}
fn default_micro_tags() -> bool {
    true
}
fn default_micro_tag_max() -> usize {
    24
}
fn default_node_rings() -> bool {
    true
}
fn default_ring_min_degree() -> usize {
    6
}
fn default_edge_pick_threshold() -> f32 {
    0.15
}
fn default_audio_enabled() -> bool {
    true
}
fn default_audio_volume() -> f32 {
    0.6
}

fn default_uds_path() -> String {
    static CACHED: OnceLock<String> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            #[cfg(unix)]
            {
                if let Some(dir) = preferred_runtime_dir() {
                    return format!("{dir}/spacegraph.sock");
                }
            }
            "/tmp/spacegraph.sock".to_string()
        })
        .clone()
}

#[cfg(unix)]
fn preferred_runtime_dir() -> Option<String> {
    let uid = uid_from_proc().or_else(uid_from_env)?;
    let run_dir = format!("/run/user/{uid}");
    if std::path::Path::new(&run_dir).is_dir() {
        Some(run_dir)
    } else {
        None
    }
}

#[cfg(unix)]
fn uid_from_env() -> Option<u32> {
    std::env::var("UID").ok()?.parse().ok()
}

#[cfg(unix)]
fn uid_from_proc() -> Option<u32> {
    let contents = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in contents.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            let uid = rest.split_whitespace().next()?;
            return uid.parse().ok();
        }
    }
    None
}

fn default_agents() -> Vec<AgentEndpoint> {
    vec![AgentEndpoint::default()]
}

fn default_max_visible_alerts() -> usize {
    200
}

fn config_file_path() -> Option<PathBuf> {
    let proj = ProjectDirs::from("", "", "spacegraph")?;
    Some(proj.config_dir().join("viewer.toml"))
}

pub fn load_or_default() -> ViewerConfig {
    let Some(path) = config_file_path() else {
        return ViewerConfig::default();
    };
    load_or_default_from_path(&path)
}

fn load_or_default_from_path(path: &Path) -> ViewerConfig {
    let Ok(contents) = fs::read_to_string(path) else {
        return ViewerConfig::default();
    };
    toml::from_str(&contents).unwrap_or_else(|_| ViewerConfig::default())
}

pub fn save(cfg: &ViewerConfig) -> anyhow::Result<()> {
    let Some(path) = config_file_path() else {
        return Err(anyhow::anyhow!("no config directory available"));
    };
    save_to_path(cfg, &path)
}

fn save_to_path(cfg: &ViewerConfig, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }
    let data = toml::to_string_pretty(cfg).context("failed to serialize viewer config")?;
    fs::write(path, data)
        .with_context(|| format!("failed to write viewer config {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn viewer_config_roundtrip_save_load() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("viewer.toml");
        let cfg = ViewerConfig::default();

        save_to_path(&cfg, &path).expect("save config");
        let loaded = load_or_default_from_path(&path);

        assert_eq!(cfg, loaded);
    }

    #[test]
    fn node_detail_config_roundtrip() {
        // Default (level = None) round-trips, and an explicit override survives.
        let cfg = NodeDetailConfig::default();
        let encoded = toml::to_string(&cfg).expect("serialize node_detail");
        let decoded: NodeDetailConfig = toml::from_str(&encoded).expect("deserialize node_detail");
        assert_eq!(cfg, decoded);
        assert!(decoded.level.is_none());

        let overridden = NodeDetailConfig {
            level: Some("low".to_string()),
            max_preview_panels: 1,
            ..NodeDetailConfig::default()
        };
        let enc = toml::to_string(&overridden).expect("serialize override");
        let dec: NodeDetailConfig = toml::from_str(&enc).expect("deserialize override");
        assert_eq!(overridden, dec);
        assert_eq!(dec.level.as_deref(), Some("low"));
    }

    #[test]
    fn search_config_roundtrip() {
        // Defaults match the spec §7 block.
        let d = SearchConfig::default();
        assert!(!d.full_system);
        assert_eq!(d.result_limit, 200);
        assert_eq!(d.debounce_ms, 120);

        let cfg = SearchConfig {
            full_system: true,
            result_limit: 50,
            debounce_ms: 200,
        };
        let dec: SearchConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);

        // The [search] block round-trips inside a full ViewerConfig...
        let viewer = ViewerConfig {
            search: cfg.clone(),
            ..Default::default()
        };
        let dec: ViewerConfig = toml::from_str(&toml::to_string(&viewer).unwrap()).unwrap();
        assert_eq!(dec.search, cfg);

        // ...and a config file that omits [search] entirely decodes to the
        // default (serde(default)) — backward compatible with old viewer.toml.
        let dec: ViewerConfig = toml::from_str("show_3d = true").unwrap();
        assert_eq!(dec.search, SearchConfig::default());
    }

    #[test]
    fn shell_config_roundtrip() {
        let cfg = ShellConfig {
            left_open: true,
            left_width: 300.0,
            right_open: false,
            right_width: 280.0,
            bottom_open: true,
            technician_open: true,
        };
        let dec: ShellConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);
        // Defaults: Technician collapsed, right open.
        let d = ShellConfig::default();
        assert!(!d.technician_open && d.right_open && d.left_open);
    }

    #[test]
    fn quality_config_roundtrip() {
        let cfg = QualityConfig::default();
        let dec: QualityConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);
        assert_eq!(dec.tier, "auto");
        assert!(dec.adaptive);

        let overridden = QualityConfig {
            tier: "potato".into(),
            adaptive: false,
            target_fps_override: Some(30),
        };
        let dec2: QualityConfig = toml::from_str(&toml::to_string(&overridden).unwrap()).unwrap();
        assert_eq!(overridden, dec2);
    }

    #[test]
    fn edge_lod_config_roundtrip() {
        let cfg = EdgeLodConfig::default();
        let dec: EdgeLodConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);
        assert!(dec.focus_cull);
        assert!(dec.near_dist < dec.far_dist);

        let overridden = EdgeLodConfig {
            near_dist: 40.0,
            far_dist: 90.0,
            far_dim: 0.5,
            focus_cull: false,
        };
        let dec2: EdgeLodConfig = toml::from_str(&toml::to_string(&overridden).unwrap()).unwrap();
        assert_eq!(overridden, dec2);
    }

    #[test]
    fn socket_display_config_roundtrip() {
        let cfg = SocketDisplayConfig::default();
        let dec: SocketDisplayConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);
        assert!(dec.aperture_by_state && dec.exposure_depth && dec.anomaly_focus);

        let overridden = SocketDisplayConfig {
            aperture_by_state: false,
            exposure_depth: false,
            anomaly_focus: false,
            anomaly_intensity: 0.25,
        };
        let dec2: SocketDisplayConfig =
            toml::from_str(&toml::to_string(&overridden).unwrap()).unwrap();
        assert_eq!(overridden, dec2);

        // Present within the full ViewerConfig (nested serde(default)).
        let full = ViewerConfig::default();
        let dec3: ViewerConfig = toml::from_str(&toml::to_string(&full).unwrap()).unwrap();
        assert_eq!(full.socket_display, dec3.socket_display);
    }

    #[test]
    fn focus_config_roundtrip() {
        let cfg = FocusConfig::default();
        let dec: FocusConfig = toml::from_str(&toml::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(cfg, dec);
        assert!(dec.freeze_layout);
        assert!(!dec.dof, "DoF is deferred (off by default) in v0.5.1");

        let overridden = FocusConfig {
            dim: 0.4,
            dof: true,
            freeze_layout: false,
        };
        let dec2: FocusConfig = toml::from_str(&toml::to_string(&overridden).unwrap()).unwrap();
        assert_eq!(overridden, dec2);
    }

    #[test]
    fn agent_endpoint_roundtrip() {
        let endpoint = AgentEndpoint {
            name: "local".to_string(),
            kind: AgentEndpointKind::UdsPath("/tmp/spacegraph.sock".to_string()),
            auto_connect: false,
            mode_override: Some(AgentMode::Privileged),
        };

        let encoded = toml::to_string(&endpoint).expect("serialize endpoint");
        let decoded: AgentEndpoint = toml::from_str(&encoded).expect("deserialize endpoint");

        assert_eq!(endpoint, decoded);
    }

    #[test]
    fn agent_endpoint_rejects_unknown_kind() {
        let bad = r#"
name = "bad"
kind = "tcp"
value = "127.0.0.1:1234"
auto_connect = true
"#;

        let decoded: Result<AgentEndpoint, _> = toml::from_str(bad);
        assert!(decoded.is_err());
    }
}
