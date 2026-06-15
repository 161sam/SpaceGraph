//! ARCHIVED (MP-UI-GitS-polish, P1): reverted out of the live build. The P5 3D
//! focus core dominated and occluded the focused node; the polish pass replaced it
//! with a clean focus treatment (reticle brackets + the node's own per-type
//! silhouette + one thin indicator ring) and a segmented radial action ring. Kept
//! here (not deleted) per the archive-not-delete discipline; not compiled (lives
//! outside any crate). To restore, move back under `render/` and re-wire the three
//! call sites in `app/mod.rs` + the `render/mod.rs` mod/re-export.
//!
//! Focused-node 3D layered core (MP-UI-GitS, P5) — a real depth-tested, emissive
//! (bloom-eligible) holographic core spawned around the **single** focused node:
//! three gyroscopic glowing rings (solid tori) on orthogonal planes, a wireframe
//! octahedron shell, and a bright pulsing inner pip. O(1) (one focused node);
//! animated only while Focus Mode is active (no idle cost). Standard + Spatial
//! only — Minimal stays flat.
//!
//! Entities are spawned **flat** (no hierarchy), each tagged [`FocusCore`] with
//! the focused node id, exactly like the gate-glyph / edge layers — shared
//! meshes + unlit-HDR-emissive materials, no per-frame allocation.

use std::sync::atomic::Ordering;

use bevy::math::primitives::Torus;
use bevy::prelude::*;

use spacegraph_core::NodeId;

use crate::graph::{GraphState, ViewMode};
use crate::render::node_glyph::glyph_layer_active;
use crate::render::{node_mesh, theme};

/// HDR emissive multiplier so the core blooms strongly (above the glyph layer).
const CORE_GLOW: f32 = 6.5;
/// World-space scale of the rig around the node (rings ≈ this × their radius) —
/// sized so the gyroscope reads as the outer cage around the radial commands.
const CORE_SCALE: f32 = 1.35;

/// Shared focus-core meshes + emissive materials (built once; no per-frame alloc).
#[derive(Resource)]
pub struct FocusCoreResources {
    ring: Handle<Mesh>,
    shell: Handle<Mesh>,
    inner: Handle<Mesh>,
    ring_mat: Handle<StandardMaterial>,
    shell_mat: Handle<StandardMaterial>,
    inner_mat: Handle<StandardMaterial>,
}

/// Tags every focus-core entity with the node it frames (rebuild on change).
#[derive(Component)]
pub struct FocusCore(pub NodeId);

/// A spinning ring/shell — rotates about its own local `axis`.
#[derive(Component)]
pub struct FocusCoreSpin {
    axis: Vec3,
    speed: f32,
}

/// The bright inner pip — pulses in scale.
#[derive(Component)]
pub struct FocusCorePulse {
    base: f32,
}

fn emissive(color: Color) -> StandardMaterial {
    let c = color.to_linear();
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::rgb(c.red * CORE_GLOW, c.green * CORE_GLOW, c.blue * CORE_GLOW),
        unlit: true,
        ..default()
    }
}

pub fn setup_focus_core_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    // A thin glowing tube ring (solid → blooms strongly); unit major radius, lies
    // in the XZ plane (axis = local Y).
    let ring = meshes.add(Mesh::from(Torus {
        minor_radius: 0.045,
        major_radius: 1.0,
    }));
    commands.insert_resource(FocusCoreResources {
        ring,
        shell: meshes.add(node_mesh::octahedron_wire(1.0)),
        inner: meshes.add(Mesh::from(Sphere::new(0.16))),
        ring_mat: mats.add(emissive(theme::FOCUS_CORE_RING)),
        shell_mat: mats.add(emissive(theme::FOCUS_CORE_SHELL)),
        inner_mat: mats.add(emissive(theme::FOCUS_CORE_INNER)),
    });
}

/// Spawn / rebuild / despawn the focus core for the single focused node (O(1)).
pub fn sync_focus_core(
    mut commands: Commands,
    st: Res<GraphState>,
    res: Res<FocusCoreResources>,
    existing: Query<(Entity, &FocusCore)>,
) {
    let active = st.ui.view_mode == ViewMode::Spatial && glyph_layer_active(st.cfg.visual_theme);
    let target = if active {
        st.ui.focus_mode.clone()
    } else {
        None
    };
    let world = target.as_ref().and_then(|id| st.spatial.position_of(id));
    let current = existing.iter().next().map(|(_, c)| c.0.clone());

    match (target, world) {
        (Some(id), Some(pos)) => {
            if current.as_ref() == Some(&id) {
                return; // already framing this node
            }
            for (e, _) in &existing {
                commands.entity(e).despawn();
            }
            spawn_core(&mut commands, &res, &id, pos);
        }
        _ => {
            for (e, _) in &existing {
                commands.entity(e).despawn();
            }
        }
    }
}

fn ring_tf(pos: Vec3, plane: Quat, r: f32) -> Transform {
    Transform {
        translation: pos,
        rotation: plane,
        scale: Vec3::splat(r * CORE_SCALE),
    }
}

fn spawn_core(commands: &mut Commands, res: &FocusCoreResources, id: &NodeId, pos: Vec3) {
    use std::f32::consts::FRAC_PI_2;
    // Three gyroscopic rings on orthogonal planes; each spins about its own axis
    // (local Y = the torus normal), so it turns in-plane like a gyroscope wheel.
    let rings = [
        (1.00_f32, Quat::IDENTITY, 0.55_f32),
        (0.80, Quat::from_rotation_x(FRAC_PI_2), -0.85),
        (0.62, Quat::from_rotation_z(FRAC_PI_2), 0.70),
    ];
    for (r, plane, speed) in rings {
        commands.spawn((
            FocusCore(id.clone()),
            PbrBundle {
                mesh: res.ring.clone(),
                material: res.ring_mat.clone(),
                transform: ring_tf(pos, plane, r),
                ..default()
            },
            FocusCoreSpin {
                axis: Vec3::Y,
                speed,
            },
        ));
    }
    // Wireframe octahedron shell, slow tumble.
    commands.spawn((
        FocusCore(id.clone()),
        PbrBundle {
            mesh: res.shell.clone(),
            material: res.shell_mat.clone(),
            transform: Transform {
                translation: pos,
                scale: Vec3::splat(0.95 * CORE_SCALE),
                ..default()
            },
            ..default()
        },
        FocusCoreSpin {
            axis: Vec3::new(0.3, 1.0, 0.2).normalize(),
            speed: 0.35,
        },
    ));
    // Bright inner pip (pulses in scale).
    commands.spawn((
        FocusCore(id.clone()),
        PbrBundle {
            mesh: res.inner.clone(),
            material: res.inner_mat.clone(),
            transform: Transform::from_translation(pos),
            ..default()
        },
        FocusCorePulse { base: 1.0 },
    ));
}

/// Animate spin + pulse — only while Focus Mode is active, and request a redraw so
/// the animation advances under the reactive frame pacing (no cost when not focused).
pub fn animate_focus_core(
    time: Res<Time>,
    st: Res<GraphState>,
    mut spin_q: Query<(&mut Transform, &FocusCoreSpin), Without<FocusCorePulse>>,
    mut pulse_q: Query<(&mut Transform, &FocusCorePulse), Without<FocusCoreSpin>>,
) {
    if st.ui.focus_mode.is_none() {
        return;
    }
    let dt = time.delta_seconds();
    for (mut t, spin) in &mut spin_q {
        t.rotate_local(Quat::from_axis_angle(spin.axis, spin.speed * dt));
    }
    let pulse = 1.0 + 0.22 * (time.elapsed_seconds() * 3.0).sin();
    for (mut t, p) in &mut pulse_q {
        t.scale = Vec3::splat(p.base * pulse * CORE_SCALE);
    }
    st.needs_redraw.store(true, Ordering::Relaxed);
}
