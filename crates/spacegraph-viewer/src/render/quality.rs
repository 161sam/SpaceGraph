//! Quality-tier system (v0.5.0 spine, spec §2) — a cost axis `QualityTier
//! {Potato, Low, Medium, High}` orthogonal to the aesthetic `VisualTheme`.
//!
//! Auto-detected from the GPU adapter, runtime-adaptive (FPS feedback with
//! hysteresis), and manually overridable. Only *expensive GPU effects* are
//! tier-gated (HDR bloom, post-FX, MSAA, node budgets, 3D silhouettes/rings); the
//! GitS identity (neon palette, gate-glyphs, reticle, radial HUD, ripples) is
//! tier-independent, so a Raspberry Pi at `Potato` still reads as Ghost-in-the-
//! Shell (spec §2.3). `VisualTheme::Minimal` forces the cheapest path regardless
//! of tier.
//!
//! This supersedes the v0.4.1 `DetailCapability` as the authority: the effective
//! tier derives the `DetailCapability` that drives node-detail (`apply_quality`).

use bevy::core_pipeline::bloom::BloomSettings;
use bevy::prelude::*;

use crate::graph::GraphState;
use crate::render::capability::{AdapterKind, DetailCapability};
use crate::util::config::{QualityConfig, VisualTheme};

/// Bloom intensity when HDR bloom is on for the tier (matches the v0.4.0 value).
const BLOOM_ON: f32 = 0.25;

/// Cost tier. Ordered cheapest → richest (`Potato < Low < Medium < High`).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum QualityTier {
    Potato,
    Low,
    Medium,
    High,
}

impl QualityTier {
    pub const ALL: [QualityTier; 4] = [
        QualityTier::Potato,
        QualityTier::Low,
        QualityTier::Medium,
        QualityTier::High,
    ];

    /// Target FPS for this tier (drives the adaptive step thresholds).
    pub fn target_fps(self) -> f32 {
        match self {
            QualityTier::Potato => 30.0,
            QualityTier::Low => 30.0,
            QualityTier::Medium => 45.0,
            QualityTier::High => 60.0,
        }
    }

    /// One tier cheaper, or `None` at the floor.
    pub fn down(self) -> Option<QualityTier> {
        match self {
            QualityTier::Potato => None,
            QualityTier::Low => Some(QualityTier::Potato),
            QualityTier::Medium => Some(QualityTier::Low),
            QualityTier::High => Some(QualityTier::Medium),
        }
    }

    /// One tier richer, or `None` at the ceiling.
    pub fn up(self) -> Option<QualityTier> {
        match self {
            QualityTier::Potato => Some(QualityTier::Low),
            QualityTier::Low => Some(QualityTier::Medium),
            QualityTier::Medium => Some(QualityTier::High),
            QualityTier::High => None,
        }
    }

    /// Node-detail capability derived from the tier (the v0.4.1 axis follows it).
    pub fn detail_capability(self) -> DetailCapability {
        match self {
            QualityTier::Potato | QualityTier::Low => DetailCapability::Low,
            QualityTier::Medium => DetailCapability::Mid,
            QualityTier::High => DetailCapability::High,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            QualityTier::Potato => "potato",
            QualityTier::Low => "low",
            QualityTier::Medium => "medium",
            QualityTier::High => "high",
        }
    }
}

/// Post-FX cost mode per tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostFxMode {
    Off,
    /// Scanline only, half-res (Low).
    ScanlineHalf,
    /// Full effect stack, half-res (Medium).
    FullHalf,
    /// Full effect stack, full-res (High).
    FullFull,
}

impl PostFxMode {
    pub fn is_on(self) -> bool {
        self != PostFxMode::Off
    }
}

/// The render-flag preset a tier (× theme) resolves to. Consumed where v0.4.0
/// already branches on `VisualTheme`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TierGates {
    pub hdr_bloom: bool,
    pub postfx: PostFxMode,
    /// Orbital-ring meshes allowed (off at Potato).
    pub rings: bool,
    /// 3D per-type silhouette meshes near-LOD allowed; when false the gate-glyph
    /// is the primary node representation (Potato/Low).
    pub silhouettes: bool,
    pub max_nodes: usize,
    /// MSAA samples (1 = off).
    pub msaa: u32,
    pub target_fps: f32,
}

impl QualityTier {
    /// The Standard-theme preset for this tier (spec §2.2).
    pub fn gates(self) -> TierGates {
        match self {
            QualityTier::Potato => TierGates {
                hdr_bloom: false,
                postfx: PostFxMode::Off,
                rings: false,
                silhouettes: false,
                max_nodes: 400,
                msaa: 1,
                target_fps: 30.0,
            },
            QualityTier::Low => TierGates {
                hdr_bloom: true,
                postfx: PostFxMode::ScanlineHalf,
                rings: true,
                silhouettes: false,
                max_nodes: 800,
                msaa: 1,
                target_fps: 30.0,
            },
            QualityTier::Medium => TierGates {
                hdr_bloom: true,
                postfx: PostFxMode::FullHalf,
                rings: true,
                silhouettes: true,
                max_nodes: 1200,
                msaa: 2,
                target_fps: 45.0,
            },
            QualityTier::High => TierGates {
                hdr_bloom: true,
                postfx: PostFxMode::FullFull,
                rings: true,
                silhouettes: true,
                max_nodes: 2500,
                msaa: 4,
                target_fps: 60.0,
            },
        }
    }
}

/// The effective preset after folding in the aesthetic theme. `Minimal` forces
/// the cheapest path (flat look) regardless of tier — only the node budget and
/// FPS target are kept from the tier.
pub fn effective_gates(tier: QualityTier, theme: VisualTheme) -> TierGates {
    let g = tier.gates();
    match theme {
        VisualTheme::Standard => g,
        VisualTheme::Minimal => TierGates {
            hdr_bloom: false,
            postfx: PostFxMode::Off,
            rings: false,
            silhouettes: false,
            max_nodes: g.max_nodes,
            msaa: 1,
            target_fps: g.target_fps,
        },
    }
}

/// Pure tier classifier from adapter facts (spec §2.4). `backend` is the wgpu
/// `Backend` `Debug` string (e.g. "Gl", "Vulkan") — passed as text to avoid a
/// direct wgpu dependency, mirroring `capability::adapter_kind_from_debug`.
pub fn detect_tier(name: &str, kind: AdapterKind, backend: &str) -> QualityTier {
    let n = name.to_lowercase();
    // Software / Pi / embedded by name → Potato regardless of reported kind.
    let potato_name = ["v3d", "videocore", "llvmpipe", "swiftshader", "software"]
        .iter()
        .any(|m| n.contains(m));
    if potato_name {
        return QualityTier::Potato;
    }
    let is_gl = backend.eq_ignore_ascii_case("gl") || backend.to_lowercase().contains("gl");
    match kind {
        AdapterKind::Discrete => QualityTier::High,
        AdapterKind::Integrated => {
            // GL-backend or weak-iGPU name → Low; otherwise Medium.
            let weak = [
                "hd graphics",
                "uhd graphics 6",
                "gma",
                "vega 3",
                "vega 6",
                "radeon hd",
                "mali",
                "adreno",
            ]
            .iter()
            .any(|m| n.contains(m));
            if is_gl || weak {
                QualityTier::Low
            } else {
                QualityTier::Medium
            }
        }
        // CPU / virtual / unknown → Potato (spec §2.4).
        AdapterKind::Cpu | AdapterKind::Other => QualityTier::Potato,
    }
}

/// Parse the `[quality] tier` config string. `None` = auto-detect.
pub fn parse_tier(s: &str) -> Option<QualityTier> {
    match s.to_lowercase().as_str() {
        "potato" => Some(QualityTier::Potato),
        "low" => Some(QualityTier::Low),
        "medium" | "mid" => Some(QualityTier::Medium),
        "high" => Some(QualityTier::High),
        _ => None, // "auto" or unknown
    }
}

/// Adaptive step thresholds (spec §2.5). Asymmetric (slow up, fast down) + a
/// margin band give hysteresis that prevents oscillation.
const STEP_DOWN_SECS: f32 = 3.0;
const STEP_UP_SECS: f32 = 10.0;
const UP_MARGIN_FPS: f32 = 15.0;

/// Pure adaptive accumulator: fed per-sample mean FPS, decides tier steps.
#[derive(Debug, Clone, Copy, Default)]
pub struct AdaptiveState {
    below_secs: f32,
    above_secs: f32,
}

impl AdaptiveState {
    /// Advance by one sample (`mean_fps` over the last window, `dt` seconds since
    /// the previous sample) and return the (possibly stepped) effective tier.
    /// Never below `Potato`; never above `base`. `target_fps_override` (if set)
    /// replaces the tier's nominal target.
    pub fn update(
        &mut self,
        effective: QualityTier,
        base: QualityTier,
        mean_fps: f32,
        dt: f32,
        target_fps_override: Option<f32>,
    ) -> QualityTier {
        let target = target_fps_override.unwrap_or_else(|| effective.target_fps());
        if mean_fps < target {
            self.above_secs = 0.0;
            self.below_secs += dt;
            if self.below_secs >= STEP_DOWN_SECS {
                if let Some(down) = effective.down() {
                    self.below_secs = 0.0;
                    return down;
                }
                self.below_secs = STEP_DOWN_SECS; // already at floor; clamp
            }
        } else if mean_fps > target + UP_MARGIN_FPS {
            self.below_secs = 0.0;
            self.above_secs += dt;
            if self.above_secs >= STEP_UP_SECS {
                if let Some(up) = effective.up() {
                    if up <= base {
                        self.above_secs = 0.0;
                        return up;
                    }
                }
                self.above_secs = STEP_UP_SECS; // at base ceiling; clamp
            }
        } else {
            // In the margin band → stable; reset both (no oscillation).
            self.below_secs = 0.0;
            self.above_secs = 0.0;
        }
        effective
    }
}

/// Runtime quality state (Resource). `base` is the detected/config ceiling;
/// `effective` is what adaptive currently applies.
#[derive(Resource, Debug, Clone)]
pub struct QualityState {
    pub base: QualityTier,
    pub effective: QualityTier,
    pub adaptive_on: bool,
    pub target_fps_override: Option<f32>,
    pub adaptive: AdaptiveState,
    /// Set when `effective` (or the theme) changes; consumed once by
    /// `apply_quality` so reconfiguration happens exactly once per change.
    dirty: bool,
    last_theme: Option<VisualTheme>,
}

impl QualityState {
    pub fn new(base: QualityTier, cfg: &QualityConfig) -> Self {
        Self {
            base,
            effective: base,
            adaptive_on: cfg.adaptive,
            target_fps_override: cfg.target_fps_override.map(|v| v as f32),
            adaptive: AdaptiveState::default(),
            dirty: true, // apply once on startup
            last_theme: None,
        }
    }

    /// Set the effective tier; marks dirty (one reconfiguration) on a real change.
    pub fn set_effective(&mut self, tier: QualityTier) -> bool {
        if self.effective != tier {
            self.effective = tier;
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Note the active theme; a change marks dirty (gates depend on theme).
    pub fn note_theme(&mut self, theme: VisualTheme) {
        if self.last_theme != Some(theme) {
            self.last_theme = Some(theme);
            self.dirty = true;
        }
    }

    /// Consume the dirty flag — true at most once per change (churn-free steady
    /// state).
    pub fn take_dirty(&mut self) -> bool {
        let d = self.dirty;
        self.dirty = false;
        d
    }

    pub fn gates(&self, theme: VisualTheme) -> TierGates {
        effective_gates(self.effective, theme)
    }
}

impl Default for QualityState {
    fn default() -> Self {
        Self {
            base: QualityTier::Medium,
            effective: QualityTier::Medium,
            adaptive_on: true,
            target_fps_override: None,
            adaptive: AdaptiveState::default(),
            dirty: true,
            last_theme: None,
        }
    }
}

/// Reconfigure the renderer once per tier/theme change (spec §2.6): bloom,
/// MSAA, the node budget, and the derived `DetailCapability`. Post-FX and orbital
/// rings are gated at their own systems (`sync_postfx` / `sync_node_rings`) which
/// read `QualityState` each frame. No per-frame churn — guarded by `take_dirty`.
pub fn apply_quality(
    mut quality: ResMut<QualityState>,
    mut st: ResMut<GraphState>,
    mut bloom_q: Query<&mut BloomSettings>,
    mut msaa: ResMut<Msaa>,
    mut cap: ResMut<DetailCapability>,
) {
    quality.note_theme(st.cfg.visual_theme);
    if !quality.take_dirty() {
        return;
    }
    let gates = quality.gates(st.cfg.visual_theme);

    for mut bloom in bloom_q.iter_mut() {
        let want = if gates.hdr_bloom { BLOOM_ON } else { 0.0 };
        if (bloom.intensity - want).abs() > f32::EPSILON {
            bloom.intensity = want;
        }
    }

    let want_msaa = match gates.msaa {
        4 => Msaa::Sample4,
        2 => Msaa::Sample2,
        _ => Msaa::Off,
    };
    if *msaa != want_msaa {
        *msaa = want_msaa;
    }

    // The tier owns the node budget (spec §2.2: "max nodes (default)").
    st.cfg.max_visible_nodes = gates.max_nodes;
    // The v0.4.1 node-detail axis follows the effective tier.
    *cap = quality.effective.detail_capability();
}

/// Runtime-adaptive tier stepping (spec §2.5): a ~1 s mean-FPS window feeds the
/// pure `AdaptiveState`. Off when `quality.adaptive` is false.
pub fn adaptive_quality(
    time: Res<Time>,
    mut quality: ResMut<QualityState>,
    mut window: Local<(f32, u32)>,
) {
    if !quality.adaptive_on {
        return;
    }
    let dt = time.delta_seconds();
    window.0 += dt;
    window.1 += 1;
    if window.0 < 1.0 {
        return; // accumulate a ~1 s window
    }
    let mean_fps = window.1 as f32 / window.0;
    let elapsed = window.0;
    *window = (0.0, 0);

    let (eff, base, ovr) = (quality.effective, quality.base, quality.target_fps_override);
    let next = quality.adaptive.update(eff, base, mean_fps, elapsed, ovr);
    quality.set_effective(next);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_tier_fixtures() {
        // Raspberry Pi (V3D / VideoCore) → Potato regardless of kind/backend.
        assert_eq!(
            detect_tier("V3D 4.2", AdapterKind::Other, "Gl"),
            QualityTier::Potato
        );
        assert_eq!(
            detect_tier("VideoCore VI", AdapterKind::Integrated, "Gl"),
            QualityTier::Potato
        );
        assert_eq!(
            detect_tier("llvmpipe (LLVM 15)", AdapterKind::Cpu, "Vulkan"),
            QualityTier::Potato
        );
        // Discrete → High.
        assert_eq!(
            detect_tier("NVIDIA GeForce RTX 4070", AdapterKind::Discrete, "Vulkan"),
            QualityTier::High
        );
        // GL-backend integrated → Low; Vulkan integrated (modern) → Medium.
        assert_eq!(
            detect_tier("Intel Iris Xe", AdapterKind::Integrated, "Gl"),
            QualityTier::Low
        );
        assert_eq!(
            detect_tier("Intel Iris Xe", AdapterKind::Integrated, "Vulkan"),
            QualityTier::Medium
        );
        // Weak iGPU by name on Vulkan → Low.
        assert_eq!(
            detect_tier(
                "Intel(R) HD Graphics 520",
                AdapterKind::Integrated,
                "Vulkan"
            ),
            QualityTier::Low
        );
        // CPU / unknown → Potato.
        assert_eq!(
            detect_tier("Some Software Device", AdapterKind::Cpu, "Vulkan"),
            QualityTier::Potato
        );
    }

    #[test]
    fn parse_and_caps() {
        assert_eq!(parse_tier("auto"), None);
        assert_eq!(parse_tier("Potato"), Some(QualityTier::Potato));
        assert_eq!(parse_tier("HIGH"), Some(QualityTier::High));
        assert_eq!(
            QualityTier::Potato.detail_capability(),
            DetailCapability::Low
        );
        assert_eq!(
            QualityTier::High.detail_capability(),
            DetailCapability::High
        );
        assert!(QualityTier::Potato < QualityTier::High);
    }

    #[test]
    fn minimal_forces_cheapest_path() {
        let g = effective_gates(QualityTier::High, VisualTheme::Minimal);
        assert!(!g.hdr_bloom);
        assert_eq!(g.postfx, PostFxMode::Off);
        assert!(!g.rings);
        assert!(!g.silhouettes);
        assert_eq!(g.msaa, 1);
        // Standard High keeps the rich preset.
        let s = effective_gates(QualityTier::High, VisualTheme::Standard);
        assert!(s.hdr_bloom && s.postfx == PostFxMode::FullFull && s.msaa == 4);
    }

    #[test]
    fn adaptive_steps_down_after_low_window_then_floors() {
        let mut a = AdaptiveState::default();
        let mut eff = QualityTier::High;
        // 2 s below target → no step yet (needs 3 s).
        eff = a.update(eff, QualityTier::High, 20.0, 1.0, None);
        eff = a.update(eff, QualityTier::High, 20.0, 1.0, None);
        assert_eq!(eff, QualityTier::High, "no step before the 3 s window");
        // 3rd second → step down to Medium.
        eff = a.update(eff, QualityTier::High, 20.0, 1.0, None);
        assert_eq!(eff, QualityTier::Medium);
        // Keep starving → Low, then Potato, then floor (never below Potato).
        for _ in 0..20 {
            eff = a.update(eff, QualityTier::High, 5.0, 1.0, None);
        }
        assert_eq!(eff, QualityTier::Potato, "never steps below Potato");
    }

    #[test]
    fn adaptive_steps_up_after_high_window_capped_at_base() {
        let mut a = AdaptiveState::default();
        let mut eff = QualityTier::Low;
        let base = QualityTier::High;
        // Well above Low's target (30 + margin) for 9 s → no step yet (needs 10 s).
        for _ in 0..9 {
            eff = a.update(eff, base, 90.0, 1.0, None);
        }
        assert_eq!(eff, QualityTier::Low, "no up-step before the 10 s window");
        eff = a.update(eff, base, 90.0, 1.0, None);
        assert_eq!(eff, QualityTier::Medium);
        // Continue high → up to High, then capped at base (no further).
        for _ in 0..40 {
            eff = a.update(eff, base, 200.0, 1.0, None);
        }
        assert_eq!(eff, QualityTier::High, "capped at the base tier");
    }

    #[test]
    fn adaptive_in_band_does_not_oscillate() {
        let mut a = AdaptiveState::default();
        let mut eff = QualityTier::Medium;
        // Mean FPS in [target, target+margin] → stable, no steps over a long run.
        for _ in 0..100 {
            eff = a.update(eff, QualityTier::High, 50.0, 1.0, None); // Medium target 45, +15 = 60
        }
        assert_eq!(eff, QualityTier::Medium, "in-band stays put (hysteresis)");
    }

    #[test]
    fn dirty_fires_once_per_change() {
        let mut q = QualityState::new(QualityTier::High, &QualityConfig::default());
        assert!(q.take_dirty(), "startup applies once");
        assert!(!q.take_dirty(), "no change → churn-free");
        assert!(q.set_effective(QualityTier::Low));
        assert!(q.take_dirty(), "tier change → one reconfiguration");
        assert!(!q.take_dirty());
        assert!(!q.set_effective(QualityTier::Low), "same tier → no change");
        assert!(!q.take_dirty());
    }
}
