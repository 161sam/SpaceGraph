//! Detail-capability gate — scales node detail (Level-1 icons + Level-2 previews)
//! to the GPU class so v0.4.1 stays cheap down to a Raspberry Pi.
//!
//! **v0.5.0 seam:** this `DetailCapability {Low, Mid, High}` is the precursor to
//! v0.5.0's richer `QualityTier {Potato, Low, Medium, High}` (`detect_tier`,
//! `docs/spec_v0.5.0.md` §2.4). WP-0 of v0.5.0 will drive node detail from the
//! tier; keep `resolve_detail` as the single clamp point so it can be re-pointed.

use bevy::prelude::Resource;

use crate::util::config::NodeDetailConfig;

/// Detected GPU detail class. Resource so systems read it directly.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailCapability {
    Low,
    Mid,
    High,
}

/// Coarse adapter device class. Fed from the wgpu `DeviceType` `Debug` string so
/// we need no direct wgpu dependency (it is not re-exported by bevy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Discrete,
    Integrated,
    Cpu,
    Other,
}

/// Map a wgpu `DeviceType` `Debug` string (e.g. "DiscreteGpu") to `AdapterKind`.
pub fn adapter_kind_from_debug(device_type_debug: &str) -> AdapterKind {
    match device_type_debug {
        "DiscreteGpu" => AdapterKind::Discrete,
        "IntegratedGpu" => AdapterKind::Integrated,
        "Cpu" => AdapterKind::Cpu,
        _ => AdapterKind::Other, // VirtualGpu / Other
    }
}

/// Pure capability classifier. Pi-class / software / mobile / GLES adapter names
/// force `Low` regardless of reported device type; otherwise device type decides.
pub fn detect_capability(adapter_name: &str, kind: AdapterKind) -> DetailCapability {
    let n = adapter_name.to_lowercase();
    let low_name = [
        "v3d",
        "videocore",
        "llvmpipe",
        "swiftshader",
        "software",
        "gles",
        "mali",
        "adreno",
    ]
    .iter()
    .any(|m| n.contains(m));
    if low_name {
        return DetailCapability::Low;
    }
    match kind {
        AdapterKind::Discrete => DetailCapability::High,
        AdapterKind::Integrated => DetailCapability::Mid,
        AdapterKind::Cpu => DetailCapability::Low,
        AdapterKind::Other => DetailCapability::Mid,
    }
}

/// Config `level` override string → capability (`None` = auto-detect).
pub fn parse_override(level: &Option<String>) -> Option<DetailCapability> {
    match level.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("low") => Some(DetailCapability::Low),
        Some("mid") | Some("medium") => Some(DetailCapability::Mid),
        Some("high") => Some(DetailCapability::High),
        _ => None,
    }
}

/// Runtime-effective node-detail settings after clamping the config to the
/// detected capability. This is the single clamp point (v0.5.0 will re-point it
/// at `QualityTier`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveDetail {
    /// Decode image thumbnails (Level-2). Forced off on `Low`.
    pub enable_image: bool,
    pub enable_video_card: bool,
    pub max_preview_panels: usize,
    pub thumbnail_px: u32,
    pub max_image_bytes: usize,
    pub max_text_bytes: usize,
    /// `Low`: previews are text-only (cards/readouts), never image decode.
    pub text_only: bool,
}

pub fn resolve_detail(cfg: &NodeDetailConfig, cap: DetailCapability) -> EffectiveDetail {
    match cap {
        DetailCapability::Low => EffectiveDetail {
            enable_image: false,
            enable_video_card: cfg.enable_video_card,
            max_preview_panels: cfg.max_preview_panels.min(1),
            thumbnail_px: cfg.thumbnail_px.min(96),
            max_image_bytes: 0,
            max_text_bytes: cfg.max_text_bytes.min(64 * 1024),
            text_only: true,
        },
        DetailCapability::Mid => EffectiveDetail {
            enable_image: cfg.enable_image,
            enable_video_card: cfg.enable_video_card,
            max_preview_panels: cfg.max_preview_panels.min(3),
            thumbnail_px: cfg.thumbnail_px.min(256),
            max_image_bytes: cfg.max_image_bytes.min(2 * 1024 * 1024),
            max_text_bytes: cfg.max_text_bytes,
            text_only: false,
        },
        DetailCapability::High => EffectiveDetail {
            enable_image: cfg.enable_image,
            enable_video_card: cfg.enable_video_card,
            max_preview_panels: cfg.max_preview_panels,
            thumbnail_px: cfg.thumbnail_px,
            max_image_bytes: cfg.max_image_bytes,
            max_text_bytes: cfg.max_text_bytes,
            text_only: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pi_and_software_are_low() {
        // Raspberry Pi (V3D / VideoCore) and software rasterizers → Low even if
        // the reported device type is not Cpu.
        assert_eq!(
            detect_capability("V3D 4.2", AdapterKind::Other),
            DetailCapability::Low
        );
        assert_eq!(
            detect_capability("VideoCore VI", AdapterKind::Other),
            DetailCapability::Low
        );
        assert_eq!(
            detect_capability("llvmpipe (LLVM 15)", AdapterKind::Cpu),
            DetailCapability::Low
        );
    }

    #[test]
    fn discrete_high_integrated_mid() {
        assert_eq!(
            detect_capability("NVIDIA GeForce RTX 4070", AdapterKind::Discrete),
            DetailCapability::High
        );
        assert_eq!(
            detect_capability("Intel(R) HD Graphics 520", AdapterKind::Integrated),
            DetailCapability::Mid
        );
    }

    #[test]
    fn adapter_kind_maps_debug_strings() {
        assert_eq!(
            adapter_kind_from_debug("DiscreteGpu"),
            AdapterKind::Discrete
        );
        assert_eq!(
            adapter_kind_from_debug("IntegratedGpu"),
            AdapterKind::Integrated
        );
        assert_eq!(adapter_kind_from_debug("Cpu"), AdapterKind::Cpu);
        assert_eq!(adapter_kind_from_debug("VirtualGpu"), AdapterKind::Other);
    }

    #[test]
    fn override_parses() {
        assert_eq!(
            parse_override(&Some("low".into())),
            Some(DetailCapability::Low)
        );
        assert_eq!(
            parse_override(&Some("HIGH".into())),
            Some(DetailCapability::High)
        );
        assert_eq!(
            parse_override(&Some("medium".into())),
            Some(DetailCapability::Mid)
        );
        assert_eq!(parse_override(&None), None);
        assert_eq!(parse_override(&Some("bogus".into())), None);
    }

    #[test]
    fn low_disables_image_and_caps_panels() {
        let cfg = NodeDetailConfig::default(); // panels 3, image on
        let low = resolve_detail(&cfg, DetailCapability::Low);
        assert!(!low.enable_image, "Low must disable image decode");
        assert!(low.text_only);
        assert!(low.max_preview_panels <= 1);
        assert_eq!(low.max_image_bytes, 0);

        let high = resolve_detail(&cfg, DetailCapability::High);
        assert!(high.enable_image);
        assert_eq!(high.max_preview_panels, 3);
    }
}
