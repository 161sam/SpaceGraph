use anyhow::Context;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerViewMode {
    Spatial,
    Timeline,
}

impl Default for ViewerViewMode {
    fn default() -> Self {
        Self::Spatial
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LodEdgesMode {
    Off,
    FocusOnly,
    All,
}

impl Default for LodEdgesMode {
    fn default() -> Self {
        Self::FocusOnly
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
    pub focus_hops: usize,
    pub max_visible_nodes: usize,
    pub progressive_nodes_per_frame: usize,
    pub layout_force: bool,
    pub link_distance: f32,
    pub repulsion: f32,
    pub damping: f32,
    pub max_step: f32,
    pub timeline_window_secs: u64,
    pub timeline_scale: f32,
    pub lod_enabled: bool,
    pub lod_threshold_nodes: usize,
    pub lod_edges_mode: LodEdgesMode,
    pub glow_duration_ms: u64,
    pub gc_enabled: bool,
    pub gc_ttl_secs: u64,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            view_mode: ViewerViewMode::Spatial,
            show_3d: true,
            show_edges: true,
            show_raw_edges: false,
            show_agg_edges: true,
            focus_hops: 2,
            max_visible_nodes: 1200,
            progressive_nodes_per_frame: 250,
            layout_force: true,
            link_distance: 6.0,
            repulsion: 22.0,
            damping: 0.92,
            max_step: 0.35,
            timeline_window_secs: 60,
            timeline_scale: 0.35,
            lod_enabled: true,
            lod_threshold_nodes: 1500,
            lod_edges_mode: LodEdgesMode::FocusOnly,
            glow_duration_ms: 900,
            gc_enabled: true,
            gc_ttl_secs: 30,
        }
    }
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
}
