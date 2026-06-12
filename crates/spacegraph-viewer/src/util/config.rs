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
    #[serde(default = "default_agents")]
    pub agents: Vec<AgentEndpoint>,
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
            agents: vec![AgentEndpoint::default()],
        }
    }
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
