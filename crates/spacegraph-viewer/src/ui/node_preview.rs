//! Level-2 focused-node rich preview (v0.4.1) — type-dispatched content for the
//! focused node (+ ≤ `max_preview_panels` pinned). Cost is **O(focused)**, never
//! O(visible):
//!
//! - Content is read viewer-locally, **path-policy-gated** and **size-capped**,
//!   and decoded on the **`AsyncComputeTaskPool`** (never on the main schedule).
//! - Results are **LRU-cached** keyed by path (+ mtime for staleness), with a
//!   memory cap and eviction.
//! - Dispatch: image → thumbnail (Bevy `Image::from_buffer`, capped, downscaled);
//!   text/code/json/log → monospace head; process → terminal-styled read-only
//!   readout; video/audio/archive/binary → card; user/socket/host/alert → card.
//!
//! **v0.5.0 seam:** this focused preview is the *centre* that v0.5.0's radial
//! command HUD (WP-3) wraps with command/path rings.

use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::time::UNIX_EPOCH;

use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::texture::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use bevy_egui::{egui, EguiContexts};
use spacegraph_core::{FileKind, Node, NodeId};

use crate::graph::{GraphState, ViewMode};
use crate::render::capability::{resolve_detail, DetailCapability, EffectiveDetail};
use crate::render::node_icon::{file_subtype, IconId};
use crate::ui::egui_color;
use crate::util::config::VisualTheme;
use crate::util::ids::{node_label_long, node_label_short};

/// LRU capacity (entries) for decoded previews — bounds resident memory.
const PREVIEW_CACHE_CAP: usize = 16;

// ---------------------------------------------------------------------------
// Pure dispatch + IO helpers (unit-tested)
// ---------------------------------------------------------------------------

/// Path policy mirroring the agent's spirit: an exclude prefix always denies;
/// empty includes allow everything else; otherwise an include prefix is required.
pub fn preview_path_allowed(path: &str, includes: &[String], excludes: &[String]) -> bool {
    if excludes
        .iter()
        .any(|e| !e.is_empty() && path.starts_with(e.as_str()))
    {
        return false;
    }
    if includes.is_empty() {
        return true;
    }
    includes
        .iter()
        .any(|i| !i.is_empty() && path.starts_with(i.as_str()))
}

/// What to render for a node (decided without touching the filesystem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewPlan {
    Image(String),
    Text(String),
    /// Metadata card (non-file, denied, or non-previewable file type).
    Card,
}

fn file_path_of(node: &Node) -> Option<&str> {
    match node {
        Node::File { path, kind, .. } if *kind != FileKind::Dir => Some(path),
        _ => None,
    }
}

/// Decide the preview plan for a node given the effective detail caps and whether
/// its path passed the policy. Image previews require `enable_image` (off on Low).
pub fn plan_preview(node: &Node, eff: &EffectiveDetail, allowed: bool) -> PreviewPlan {
    let Some(path) = file_path_of(node) else {
        return PreviewPlan::Card;
    };
    if !allowed {
        return PreviewPlan::Card; // denied → no read
    }
    match file_subtype(path) {
        IconId::FileImage if eff.enable_image => PreviewPlan::Image(path.to_string()),
        IconId::FileText | IconId::FileCode | IconId::FileJson | IconId::FileLog => {
            PreviewPlan::Text(path.to_string())
        }
        // image-on-Low, video, audio, archive, binary, generic → card
        _ => PreviewPlan::Card,
    }
}

fn file_mtime(path: &str) -> Option<u64> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// Decoder output (built off-thread; `Image` is uploaded to `Assets` on poll).
pub enum DecodeResult {
    Image(Image),
    Text(String),
    /// Could not produce content — render a card with this reason.
    Note(&'static str),
}

/// Read at most `max_bytes` from a text file (truncating, never the whole file if
/// large). Lossy UTF-8. Marks truncation.
pub fn read_text_head(path: &str, max_bytes: usize) -> DecodeResult {
    let Ok(file) = std::fs::File::open(path) else {
        return DecodeResult::Note("unreadable");
    };
    let mut buf = Vec::new();
    // Read one extra byte so we can detect (and report) truncation.
    if file
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut buf)
        .is_err()
    {
        return DecodeResult::Note("unreadable");
    }
    let truncated = buf.len() > max_bytes;
    buf.truncate(max_bytes);
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        s.push_str("\n… (truncated)");
    }
    DecodeResult::Text(s)
}

/// Decode an image file to a capped thumbnail. Oversize files are **skipped**
/// (stat only, no content read); undecodable formats fall back to a card. Uses
/// only Bevy's built-in image support — formats whose feature is not enabled
/// return `Note` (card fallback), never a panic.
pub fn decode_image(path: &str, ext: &str, max_bytes: usize, thumb_px: u32) -> DecodeResult {
    let Ok(meta) = std::fs::metadata(path) else {
        return DecodeResult::Note("unreadable");
    };
    if max_bytes == 0 || meta.len() as usize > max_bytes {
        return DecodeResult::Note("image too large");
    }
    let Ok(bytes) = std::fs::read(path) else {
        return DecodeResult::Note("unreadable");
    };
    match Image::from_buffer(
        &bytes,
        ImageType::Extension(ext),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::Default,
        RenderAssetUsages::RENDER_WORLD,
    ) {
        Ok(img) => thumbnail(img, thumb_px),
        Err(_) => DecodeResult::Note("unsupported image format"),
    }
}

/// Nearest-neighbour downscale to at most `max` px on the longest side. Bounds
/// resident memory regardless of source resolution. Rgba8 only.
fn thumbnail(img: Image, max: u32) -> DecodeResult {
    let format = img.texture_descriptor.format;
    if !matches!(
        format,
        TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb
    ) {
        return DecodeResult::Note("unsupported image format");
    }
    let w = img.texture_descriptor.size.width;
    let h = img.texture_descriptor.size.height;
    if w == 0 || h == 0 || img.data.len() < (w * h * 4) as usize {
        return DecodeResult::Note("unreadable");
    }
    if w <= max && h <= max {
        return DecodeResult::Image(img);
    }
    let scale = max as f32 / w.max(h) as f32;
    let nw = ((w as f32 * scale) as u32).max(1);
    let nh = ((h as f32 * scale) as u32).max(1);
    let src = &img.data;
    let mut dst = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let sy = (y * h / nh).min(h - 1);
        for x in 0..nw {
            let sx = (x * w / nw).min(w - 1);
            let si = ((sy * w + sx) * 4) as usize;
            let di = ((y * nw + x) * 4) as usize;
            dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    DecodeResult::Image(Image::new(
        Extent3d {
            width: nw,
            height: nh,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        dst,
        format,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

// ---------------------------------------------------------------------------
// LRU cache (unit-tested)
// ---------------------------------------------------------------------------

/// A resolved preview ready to display.
pub enum CachedPreview {
    /// Decoded thumbnail handle + display dimensions.
    Image(Handle<Image>, u32, u32),
    Text(String),
    /// Card with a one-line reason (oversize / unsupported / unreadable).
    Note(String),
}

struct CacheEntry {
    mtime: u64,
    content: CachedPreview,
}

/// Path-keyed LRU with a fixed capacity. Front of `order` = most recently used.
struct PreviewCache {
    cap: usize,
    order: VecDeque<String>,
    map: HashMap<String, CacheEntry>,
}

impl PreviewCache {
    fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            order: VecDeque::new(),
            map: HashMap::new(),
        }
    }

    fn mtime_of(&self, path: &str) -> Option<u64> {
        self.map.get(path).map(|e| e.mtime)
    }

    fn bump(&mut self, path: &str) {
        if let Some(pos) = self.order.iter().position(|p| p == path) {
            self.order.remove(pos);
            self.order.push_front(path.to_string());
        }
    }

    fn peek(&self, path: &str) -> Option<&CachedPreview> {
        self.map.get(path).map(|e| &e.content)
    }

    fn insert(&mut self, path: String, mtime: u64, content: CachedPreview) {
        if self.map.contains_key(&path) {
            self.bump(&path);
        } else {
            self.order.push_front(path.clone());
        }
        self.map.insert(path, CacheEntry { mtime, content });
        while self.map.len() > self.cap {
            if let Some(old) = self.order.pop_back() {
                self.map.remove(&old); // drops Handle<Image> → frees the asset
            } else {
                break;
            }
        }
    }
}

struct PendingDecode {
    mtime: u64,
    task: Task<DecodeResult>,
}

/// Preview cache + in-flight decode tasks. O(focused) by construction.
#[derive(Resource)]
pub struct PreviewState {
    cache: PreviewCache,
    pending: HashMap<String, PendingDecode>,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            cache: PreviewCache::new(PREVIEW_CACHE_CAP),
            pending: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Decode targets: focused node (+ pinned), capped to `cap`. These are the only
/// nodes whose content we read/decode. Visual-only; iteration order over the
/// visible set is not determinism-relevant.
fn decode_set(st: &GraphState, cap: usize) -> Vec<NodeId> {
    let mut out: Vec<NodeId> = Vec::new();
    if let Some(f) = st.ui.selected.clone().or_else(|| st.ui.focus.clone()) {
        out.push(f);
    }
    // O(pins), not O(visible): iterate the compact pinned-id index, keeping only
    // those currently rendered. `is_pinned` re-validates against the authoritative
    // slot state (the index is kept exact, this is belt-and-braces).
    for id in st.spatial.pinned_ids.iter() {
        if out.len() >= cap {
            break;
        }
        if st.is_pinned(id) && st.is_visible_rendered(id) && !out.contains(id) {
            out.push(id.clone());
        }
    }
    out.truncate(cap);
    out
}

/// Display targets: the decode set plus the hovered node (a peek). Hover never
/// triggers a file read — it shows a card, or cached content if already decoded.
fn display_set(st: &GraphState, cap: usize) -> Vec<NodeId> {
    let mut out = decode_set(st, cap);
    if out.len() < cap {
        if let Some(h) = &st.ui.hovered {
            if !out.contains(h) {
                out.push(h.clone());
            }
        }
    }
    out
}

fn preview_enabled(st: &GraphState) -> bool {
    st.ui.view_mode != ViewMode::Timeline && st.cfg.visual_theme == VisualTheme::Standard
}

/// Build the preview request set and spawn decode tasks for cache misses. Never
/// reads content inline — the read/decode runs on the `AsyncComputeTaskPool`.
pub fn update_preview_requests(
    st: Res<GraphState>,
    cap: Res<DetailCapability>,
    mut preview: ResMut<PreviewState>,
) {
    let eff = resolve_detail(&st.cfg.node_detail, *cap);
    if !preview_enabled(&st) || eff.max_preview_panels == 0 {
        if !preview.pending.is_empty() {
            preview.pending.clear();
        }
        return;
    }

    let pool = AsyncComputeTaskPool::get();
    for id in decode_set(&st, eff.max_preview_panels) {
        let Some(node) = st.model.nodes.get(&id) else {
            continue;
        };
        let allowed = file_path_of(node)
            .map(|p| preview_path_allowed(p, &st.cfg.path_includes, &st.cfg.path_excludes))
            .unwrap_or(false);
        let (path, is_image) = match plan_preview(node, &eff, allowed) {
            PreviewPlan::Image(p) => (p, true),
            PreviewPlan::Text(p) => (p, false),
            PreviewPlan::Card => continue, // no file work
        };
        // Stat for staleness (not a content read). Failure → no read at all.
        let Some(mtime) = file_mtime(&path) else {
            continue;
        };
        if preview.pending.contains_key(&path) {
            continue;
        }
        if preview.cache.mtime_of(&path) == Some(mtime) {
            preview.cache.bump(&path); // fresh hit → keep alive, no re-decode
            continue;
        }

        let task = if is_image {
            let p = path.clone();
            let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
            let (mb, tp) = (eff.max_image_bytes, eff.thumbnail_px);
            pool.spawn(async move { decode_image(&p, &ext, mb, tp) })
        } else {
            let p = path.clone();
            let mb = eff.max_text_bytes;
            pool.spawn(async move { read_text_head(&p, mb) })
        };
        preview.pending.insert(path, PendingDecode { mtime, task });
    }
}

/// Drain finished decode tasks into the cache, uploading decoded images to
/// `Assets<Image>` on the main thread and evicting the LRU.
pub fn poll_preview_decodes(mut preview: ResMut<PreviewState>, mut images: ResMut<Assets<Image>>) {
    let finished: Vec<(String, u64, DecodeResult)> = {
        let mut v = Vec::new();
        for (path, pd) in preview.pending.iter_mut() {
            if let Some(res) = block_on(poll_once(&mut pd.task)) {
                v.push((path.clone(), pd.mtime, res));
            }
        }
        v
    };
    for (path, mtime, res) in finished {
        preview.pending.remove(&path);
        let content = match res {
            DecodeResult::Image(img) => {
                let (w, h) = (
                    img.texture_descriptor.size.width,
                    img.texture_descriptor.size.height,
                );
                CachedPreview::Image(images.add(img), w, h)
            }
            DecodeResult::Text(s) => CachedPreview::Text(s),
            DecodeResult::Note(n) => CachedPreview::Note(n.to_string()),
        };
        preview.cache.insert(path, mtime, content);
    }
}

/// Resolved per-node view for one frame.
enum PreviewView {
    Image(egui::TextureId, f32, f32),
    Text(String),
    Note(String),
    Loading,
    Card,
}

/// Render the focused-node preview panel (cards + cached content). Read-only.
pub fn node_preview_overlay(
    mut contexts: EguiContexts,
    st: Res<GraphState>,
    cap: Res<DetailCapability>,
    preview: Res<PreviewState>,
    expand: Res<crate::render::PreviewExpand>,
) {
    let eff = resolve_detail(&st.cfg.node_detail, *cap);
    if !preview_enabled(&st) || eff.max_preview_panels == 0 {
        return;
    }
    let set = display_set(&st, eff.max_preview_panels);
    if set.is_empty() {
        return;
    }

    // Resolve each node's view up front — image registration needs `&mut contexts`
    // before the egui ctx borrow. Index parallels `set`.
    let views: Vec<PreviewView> = set
        .iter()
        .map(|id| match cached_path(&st, id) {
            None => PreviewView::Card,
            Some(path) => match preview.cache.peek(&path) {
                Some(CachedPreview::Image(h, w, ht)) => {
                    // Weak handle: the LRU cache owns the only strong handle, so on
                    // eviction the asset frees and bevy_egui's AssetEvent::Removed
                    // cleanup fires (a strong handle here would pin it forever).
                    PreviewView::Image(contexts.add_image(h.clone_weak()), *w as f32, *ht as f32)
                }
                Some(CachedPreview::Text(s)) => PreviewView::Text(s.clone()),
                Some(CachedPreview::Note(n)) => PreviewView::Note(n.clone()),
                None if preview.pending.contains_key(&path) => PreviewView::Loading,
                None => PreviewView::Card,
            },
        })
        .collect();

    // GitS framed panel: dark fill + thin neon stroke (the "screen frame").
    let frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(8, 14, 22, 235))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgb(60, 180, 200),
        ))
        .inner_margin(egui::Margin::same(8.0))
        .rounding(2.0);
    let ctx = contexts.ctx_mut();
    egui::Window::new("◈ PREVIEW")
        .frame(frame)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-12.0, -12.0])
        .resizable(false)
        .show(ctx, |ui| {
            for (id, view) in set.iter().zip(views) {
                let Some(node) = st.model.nodes.get(id) else {
                    continue;
                };
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new(node_label_short(node))
                            .strong()
                            .monospace(),
                    );
                    match view {
                        PreviewView::Image(tid, w, h) => {
                            // Double-click expands the focused preview (larger).
                            let disp =
                                eff.thumbnail_px as f32 * if expand.expanded { 2.0 } else { 1.0 };
                            let scale = disp / w.max(h);
                            let scale = if expand.expanded {
                                scale
                            } else {
                                scale.min(1.0)
                            };
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                tid,
                                egui::vec2(w * scale, h * scale),
                            )));
                        }
                        PreviewView::Text(s) => {
                            ui.add(egui::Label::new(
                                egui::RichText::new(s).monospace().size(11.0),
                            ));
                        }
                        PreviewView::Note(n) => {
                            ui.weak(n);
                            render_card(ui, node, &eff);
                        }
                        PreviewView::Loading => {
                            ui.weak("decoding…");
                        }
                        PreviewView::Card => render_card(ui, node, &eff),
                    }
                });
            }
        });
}

/// The cache key (path) for a node if it currently maps to a file-backed plan.
fn cached_path(st: &GraphState, id: &NodeId) -> Option<String> {
    let node = st.model.nodes.get(id)?;
    file_path_of(node).map(|p| p.to_string())
}

fn render_card(ui: &mut egui::Ui, node: &Node, eff: &EffectiveDetail) {
    match node {
        Node::Process { .. } => terminal_readout(ui, node),
        Node::File { .. } => file_card(ui, node, eff),
        _ => type_card(ui, node),
    }
}

/// Terminal-styled, **read-only** process readout (not an interactive PTY — that
/// is the v0.7.0 AdminBot control plane behind the approval layer).
fn terminal_readout(ui: &mut egui::Ui, node: &Node) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(6, 10, 16))
        .inner_margin(egui::Margin::same(6.0))
        .show(ui, |ui| {
            for line in node_label_long(node) {
                ui.label(
                    egui::RichText::new(format!("$ {line}"))
                        .monospace()
                        .size(11.0)
                        .color(egui::Color32::from_rgb(120, 230, 170)),
                );
            }
        });
}

fn file_card(ui: &mut egui::Ui, node: &Node, eff: &EffectiveDetail) {
    // Distinguish the deferred-decode cases (video etc.) from the rest with a note.
    if let Node::File { path, .. } = node {
        // Video is card-only (no decoder); the card itself is toggleable.
        if file_subtype(path) == IconId::FileVideo && !eff.enable_video_card {
            ui.weak("video — card disabled");
            return;
        }
        let note = match file_subtype(path) {
            IconId::FileVideo => Some("video — card only (no decoder; v0.4.1 boundary)"),
            IconId::FileAudio => Some("audio file"),
            IconId::FileArchive => Some("archive"),
            IconId::FileBinary => Some("binary"),
            _ => None,
        };
        if let Some(n) = note {
            ui.weak(n);
        }
    }
    for line in node_label_long(node) {
        ui.label(egui::RichText::new(line).monospace().size(11.0));
    }
}

fn type_card(ui: &mut egui::Ui, node: &Node) {
    let tint = egui_color(crate::render::theme::NodeKind::of(node).base_color());
    for line in node_label_long(node) {
        ui.label(egui::RichText::new(line).monospace().size(11.0).color(tint));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacegraph_core::FileKind;
    use std::io::Write;

    fn img_node(path: &str) -> Node {
        Node::File {
            path: path.into(),
            inode: 1,
            kind: FileKind::Regular,
        }
    }

    fn eff(cap: DetailCapability) -> EffectiveDetail {
        resolve_detail(&crate::util::config::NodeDetailConfig::default(), cap)
    }

    #[test]
    fn path_policy_allows_and_denies() {
        let inc = vec!["/home".to_string(), "/etc".to_string()];
        let exc = vec!["/proc".to_string(), "/home/secret".to_string()];
        assert!(preview_path_allowed("/home/u/a.txt", &inc, &exc));
        assert!(preview_path_allowed("/etc/app.conf", &inc, &exc));
        assert!(!preview_path_allowed("/proc/1/maps", &inc, &exc)); // excluded
        assert!(!preview_path_allowed("/home/secret/x", &inc, &exc)); // exclude wins
        assert!(!preview_path_allowed("/var/log/x", &inc, &exc)); // not included
                                                                  // Empty includes → allow all but excludes.
        assert!(preview_path_allowed("/anything", &[], &exc));
        assert!(!preview_path_allowed("/proc/x", &[], &exc));
    }

    #[test]
    fn plan_dispatch_respects_capability_and_policy() {
        let high = eff(DetailCapability::High);
        let low = eff(DetailCapability::Low);
        // Image at High (allowed) → decode; at Low (image off) → card.
        assert_eq!(
            plan_preview(&img_node("/a/p.png"), &high, true),
            PreviewPlan::Image("/a/p.png".into())
        );
        assert_eq!(
            plan_preview(&img_node("/a/p.png"), &low, true),
            PreviewPlan::Card
        );
        // Text → text read; denied → card (no read).
        assert_eq!(
            plan_preview(&img_node("/a/m.rs"), &high, true),
            PreviewPlan::Text("/a/m.rs".into())
        );
        assert_eq!(
            plan_preview(&img_node("/a/m.rs"), &high, false),
            PreviewPlan::Card
        );
        // Video / non-file → card.
        assert_eq!(
            plan_preview(&img_node("/a/clip.mp4"), &high, true),
            PreviewPlan::Card
        );
        assert_eq!(
            plan_preview(
                &Node::User {
                    uid: 0,
                    name: "x".into()
                },
                &high,
                true
            ),
            PreviewPlan::Card
        );
    }

    #[test]
    fn text_head_truncates_oversize() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, "{}", "x".repeat(1000)).unwrap();
        let path = f.path().to_str().unwrap();
        match read_text_head(path, 100) {
            DecodeResult::Text(s) => {
                assert!(s.contains("truncated"));
                assert!(s.len() <= 100 + 32);
            }
            _ => panic!("expected text"),
        }
        // Small file → no truncation marker.
        match read_text_head(path, 5000) {
            DecodeResult::Text(s) => assert!(!s.contains("truncated")),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn image_oversize_is_skipped_without_decoding() {
        let mut f = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        f.write_all(&vec![0u8; 5000]).unwrap();
        let path = f.path().to_str().unwrap();
        // Over the byte budget → skipped (Note), never decoded.
        match decode_image(path, "png", 1000, 128) {
            DecodeResult::Note(n) => assert_eq!(n, "image too large"),
            _ => panic!("oversize image must be skipped"),
        }
        // Under budget but not a decodable format in this build → card fallback.
        match decode_image(path, "png", 1_000_000, 128) {
            DecodeResult::Note(_) => {}
            DecodeResult::Image(_) => {} // (only if a png feature were enabled)
            DecodeResult::Text(_) => panic!("image path must not yield text"),
        }
    }

    #[test]
    fn thumbnail_downscales_rgba8() {
        // 4×4 solid-red RGBA8 → downscaled to max 2 → 2×2, still red.
        let data = [255u8, 0, 0, 255].repeat(16);
        let img = Image::new(
            Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        match thumbnail(img, 2) {
            DecodeResult::Image(t) => {
                assert_eq!(t.texture_descriptor.size.width, 2);
                assert_eq!(t.texture_descriptor.size.height, 2);
                assert_eq!(&t.data[0..4], &[255, 0, 0, 255]);
            }
            _ => panic!("expected downscaled image"),
        }
    }

    #[test]
    fn lru_evicts_oldest_and_bumps_on_use() {
        let mut c = PreviewCache::new(2);
        c.insert("a".into(), 1, CachedPreview::Note("a".into()));
        c.insert("b".into(), 1, CachedPreview::Note("b".into()));
        c.insert("c".into(), 1, CachedPreview::Note("c".into())); // evicts "a"
        assert!(c.peek("a").is_none());
        assert!(c.peek("b").is_some());
        assert!(c.peek("c").is_some());
        assert_eq!(c.map.len(), 2);
        // Touch "b" so "c" becomes the LRU victim on the next insert.
        c.bump("b");
        c.insert("d".into(), 1, CachedPreview::Note("d".into())); // evicts "c"
        assert!(c.peek("c").is_none());
        assert!(c.peek("b").is_some());
        assert!(c.peek("d").is_some());
    }

    #[test]
    fn requests_spawn_a_task_not_inline_decode() {
        // A focused text file must produce a *pending task* after one frame — the
        // cache is not populated inline (decode is off-thread).
        bevy::tasks::AsyncComputeTaskPool::get_or_init(Default::default);
        let mut f = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        write!(f, "hello preview").unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let mut gs = GraphState::default();
        let id = NodeId("n".into());
        gs.model.nodes.insert(
            id.clone(),
            Node::File {
                path: path.clone(),
                inode: 1,
                kind: FileKind::Regular,
            },
        );
        // Allow this temp path through the policy.
        gs.cfg.path_includes = vec![path.clone()];
        gs.cfg.path_excludes = vec![];
        gs.ui.selected = Some(id);

        let mut app = App::new();
        app.insert_resource(gs)
            .insert_resource(PreviewState::default())
            .insert_resource(DetailCapability::High)
            .add_systems(Update, update_preview_requests);
        app.update();

        let preview = app.world().resource::<PreviewState>();
        assert!(
            preview.pending.contains_key(&path),
            "decode must be spawned as a task"
        );
        assert!(preview.cache.peek(&path).is_none(), "not decoded inline");
    }

    #[test]
    fn stable_focus_has_no_redecode_churn() {
        // Once a preview is cached, a stable focus must not re-spawn decode tasks.
        let mut f = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        write!(f, "stable content").unwrap();
        let path = f.path().to_str().unwrap().to_string();

        let mut gs = GraphState::default();
        let id = NodeId("n".into());
        gs.model.nodes.insert(
            id.clone(),
            Node::File {
                path: path.clone(),
                inode: 1,
                kind: FileKind::Regular,
            },
        );
        gs.cfg.path_includes = vec![path.clone()];
        gs.cfg.path_excludes = vec![];
        gs.ui.selected = Some(id);

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(gs)
            .insert_resource(PreviewState::default())
            .insert_resource(DetailCapability::High)
            .add_systems(Update, (update_preview_requests, poll_preview_decodes));

        // Pump until the background decode lands in the cache.
        let mut warmed = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<PreviewState>()
                .cache
                .peek(&path)
                .is_some()
            {
                warmed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(warmed, "decode should complete and cache");

        // Steady state: another frame must neither re-spawn nor lose the entry.
        app.update();
        let p = app.world().resource::<PreviewState>();
        assert!(
            p.pending.is_empty(),
            "stable focus must not re-decode (no churn)"
        );
        assert!(p.cache.peek(&path).is_some(), "cached entry stays resident");
    }

    #[test]
    fn preview_opens_on_focus_and_closes_when_cleared() {
        let mut gs = GraphState::default();
        let id = NodeId("n".into());
        gs.model.nodes.insert(
            id.clone(),
            Node::User {
                uid: 0,
                name: "u".into(),
            },
        );
        assert!(decode_set(&gs, 3).is_empty(), "no focus → closed");
        gs.ui.selected = Some(id.clone());
        assert_eq!(decode_set(&gs, 3), vec![id.clone()], "focus → open");
        gs.ui.selected = None;
        assert!(decode_set(&gs, 3).is_empty(), "unfocus → closed");
    }

    #[test]
    fn hover_is_display_only_never_a_decode_target() {
        let mut gs = GraphState::default();
        let id = NodeId("h".into());
        gs.ui.hovered = Some(id.clone());
        assert!(decode_set(&gs, 3).is_empty(), "hover is not decoded");
        assert_eq!(display_set(&gs, 3), vec![id], "hover opens a display peek");
    }

    #[test]
    fn decode_set_respects_panel_cap() {
        let mut gs = GraphState::default();
        gs.cfg.max_visible_nodes = 64;
        gs.cfg.progressive_nodes_per_frame = 64;
        gs.load_synthetic_graph(20);
        let vis = gs.visible_set_capped();
        gs.progressive_prepare(&vis);
        gs.spatial.vis_cache = vis;
        let ids: Vec<NodeId> = gs.spatial.vis_cache.iter().cloned().collect();
        gs.ui.selected = Some(ids[0].clone());
        for id in ids.iter().take(6) {
            gs.set_pin(id, Vec3::ZERO);
        }
        assert!(decode_set(&gs, 2).len() <= 2, "panel cap enforced");
        assert!(decode_set(&gs, 5).len() <= 5);
        // O(pins): the pinned-id index drives the scan, not the visible set.
        assert!(gs.spatial.pinned_ids.len() >= 5);
    }
}
