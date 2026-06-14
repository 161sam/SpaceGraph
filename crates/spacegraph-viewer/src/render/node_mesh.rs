//! Per-type node geometry — distinct silhouettes so a node's kind is readable
//! from its shape, not only its colour.
//!
//! Each kind has a solid emissive **core** (lit `StandardMaterial`, so it needs
//! normals) and some kinds add a holographic **wireframe shell** built as a
//! `LineList` with an unlit emissive material — the same constructor pattern as
//! `render::edges`. Cores stay within a ~0.30 envelope (Alert slightly larger)
//! so a single bounding-sphere pick still covers them. No new dependency.

use bevy::prelude::*;
use bevy::render::mesh::{CylinderMeshBuilder, MeshBuilder, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::render::theme::NodeKind;

/// Solid core mesh for a node kind (lit; carries position/normal/uv).
pub fn node_core(kind: NodeKind) -> Mesh {
    match kind {
        // Process — octahedron: an active compute core.
        NodeKind::Process => octahedron_solid(0.26),
        // File — thin hexagonal prism: a passive data plate.
        NodeKind::File => CylinderMeshBuilder::new(0.17, 0.08, 6).build(),
        // User — upward cone: identity / authority.
        NodeKind::User => Mesh::from(bevy::math::primitives::Cone {
            radius: 0.18,
            height: 0.34,
        }),
        // Socket — small torus: a port aperture.
        NodeKind::Socket => Mesh::from(bevy::math::primitives::Torus::new(0.12, 0.22)),
        // RemoteHost — small sphere inside a diamond shell: a distant station.
        NodeKind::RemoteHost => Mesh::from(Sphere::new(0.16)),
        // Alert — sphere inside a spiked shell: blooms hardest.
        NodeKind::Alert => Mesh::from(Sphere::new(0.20)),
    }
}

/// Optional wireframe shell (unlit `LineList`) drawn around the core in the
/// Standard theme for kinds that benefit from extra silhouette.
pub fn node_shell(kind: NodeKind) -> Option<Mesh> {
    match kind {
        NodeKind::RemoteHost => Some(octahedron_wire(0.30)),
        NodeKind::Alert => Some(spiked_star_wire(0.34, 0.30)),
        _ => None,
    }
}

/// Build a solid, flat-shaded `TriangleList` mesh from a list of triangles,
/// orienting each face outward from the origin (winding-independent input).
fn solid_from_tris(tris: &[[Vec3; 3]]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(tris.len() * 3);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(tris.len() * 3);
    for &[a, b, c] in tris {
        let mut n = (b - a).cross(c - a).normalize_or_zero();
        let centroid = (a + b + c) / 3.0;
        // Flip the face (and normal) so it faces away from the origin.
        let (v0, v1, v2) = if n.dot(centroid) < 0.0 {
            n = -n;
            (a, c, b)
        } else {
            (a, b, c)
        };
        for v in [v0, v1, v2] {
            positions.push(v.to_array());
            normals.push(n.to_array());
        }
    }
    let uvs = vec![[0.0_f32, 0.0]; positions.len()];
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

/// Build an unlit `LineList` wireframe mesh from a list of segments.
pub fn wire_from_edges(edges: &[(Vec3, Vec3)]) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(edges.len() * 2);
    for &(a, b) in edges {
        positions.push(a.to_array());
        positions.push(b.to_array());
    }
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh
}

fn octahedron_verts(r: f32) -> [Vec3; 6] {
    [
        Vec3::new(0.0, r, 0.0),  // 0 top
        Vec3::new(0.0, -r, 0.0), // 1 bottom
        Vec3::new(r, 0.0, 0.0),  // 2 e0
        Vec3::new(0.0, 0.0, r),  // 3 e1
        Vec3::new(-r, 0.0, 0.0), // 4 e2
        Vec3::new(0.0, 0.0, -r), // 5 e3
    ]
}

fn octahedron_solid(r: f32) -> Mesh {
    let v = octahedron_verts(r);
    let (top, bottom) = (v[0], v[1]);
    let (e0, e1, e2, e3) = (v[2], v[3], v[4], v[5]);
    let tris = [
        [top, e0, e1],
        [top, e1, e2],
        [top, e2, e3],
        [top, e3, e0],
        [bottom, e1, e0],
        [bottom, e2, e1],
        [bottom, e3, e2],
        [bottom, e0, e3],
    ];
    solid_from_tris(&tris)
}

pub fn octahedron_wire(r: f32) -> Mesh {
    let v = octahedron_verts(r);
    let (top, bottom) = (v[0], v[1]);
    let (e0, e1, e2, e3) = (v[2], v[3], v[4], v[5]);
    let edges = [
        (top, e0),
        (top, e1),
        (top, e2),
        (top, e3),
        (bottom, e0),
        (bottom, e1),
        (bottom, e2),
        (bottom, e3),
        (e0, e1),
        (e1, e2),
        (e2, e3),
        (e3, e0),
    ];
    wire_from_edges(&edges)
}

/// Spikes radiating from the centre to the 6 axis tips and 8 cube-corner tips —
/// a star/threat silhouette around the Alert core.
fn spiked_star_wire(axis_len: f32, corner_len: f32) -> Mesh {
    let mut edges: Vec<(Vec3, Vec3)> = Vec::with_capacity(14);
    for axis in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        edges.push((Vec3::ZERO, axis * axis_len));
    }
    for sx in [-1.0_f32, 1.0] {
        for sy in [-1.0_f32, 1.0] {
            for sz in [-1.0_f32, 1.0] {
                let dir = Vec3::new(sx, sy, sz).normalize();
                edges.push((Vec3::ZERO, dir * corner_len));
            }
        }
    }
    wire_from_edges(&edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_a_core_mesh() {
        for kind in NodeKind::ALL {
            let mesh = node_core(kind);
            let count = mesh
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .map(|a| a.len())
                .unwrap_or(0);
            assert!(count > 0, "{kind:?} core has vertices");
        }
    }

    #[test]
    fn only_remote_and_alert_have_shells() {
        for kind in NodeKind::ALL {
            let has = node_shell(kind).is_some();
            let want = matches!(kind, NodeKind::RemoteHost | NodeKind::Alert);
            assert_eq!(has, want, "{kind:?} shell presence");
        }
    }
}
