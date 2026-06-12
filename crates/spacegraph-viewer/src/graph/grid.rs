//! Uniform spatial grid for neighbour-only force repulsion.
//!
//! Replaces the O(N²) all-pairs repulsion: nodes are bucketed into a uniform
//! grid whose cell size equals the repulsion cutoff radius, so each node only
//! needs to consider candidates in its own cell plus the 8 (2D) / 26 (3D)
//! adjacent cells. Rebuild is O(N); a repulsion pass is O(N · k) for `k`
//! candidates per node (≈ constant at bounded density).
//!
//! The grid is backed by a **dense linear array** (cell index = x + y·nx +
//! z·nx·ny) rather than a hash map, so cell lookups are plain arithmetic with no
//! per-lookup hashing — the 27 neighbour-cell probes per node per frame would
//! otherwise dominate the frame budget.
//!
//! In 2D mode (`show_3d == false`) the layout lives on the X/Z plane (Y is
//! pinned to 0), so cells bucket by (x, z). In 3D mode they bucket by (x, y, z).

use bevy::prelude::Vec3;

use crate::graph::interner::NodeIndex;

/// Upper bound on cells per axis (safety against a far-flung outlier blowing up
/// the grid). Normal layouts use a few dozen per axis.
const MAX_AXIS: usize = 1024;
/// Hard ceiling on total cells; beyond this the grid collapses to a single cell
/// (degrades to O(N²) but never OOMs). Normal total ≈ node count.
const MAX_CELLS: usize = 262_144;

#[derive(Default)]
pub struct Grid {
    cell_size: f32,
    min: Vec3,
    nx: usize,
    ny: usize,
    nz: usize,
    three_d: bool,
    cells: Vec<Vec<NodeIndex>>,
}

impl Grid {
    /// Rebuild the grid for the given active node positions.
    pub fn rebuild(
        &mut self,
        positions: &[Vec3],
        active: &[NodeIndex],
        cell_size: f32,
        three_d: bool,
    ) {
        self.cell_size = cell_size.max(1e-4);
        self.three_d = three_d;

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for &idx in active {
            let p = positions[idx.slot()];
            min = min.min(p);
            max = max.max(p);
        }
        if !min.is_finite() {
            min = Vec3::ZERO;
            max = Vec3::ZERO;
        }
        self.min = min;
        let span = max - min;

        let axis = |s: f32| -> usize {
            (((s / self.cell_size).floor() as i64) + 1).clamp(1, MAX_AXIS as i64) as usize
        };
        if three_d {
            self.nx = axis(span.x);
            self.ny = axis(span.y);
            self.nz = axis(span.z);
        } else {
            self.nx = axis(span.x);
            self.ny = axis(span.z); // 2D second axis is Z
            self.nz = 1;
        }
        let mut total = self.nx * self.ny * self.nz;
        if total > MAX_CELLS {
            self.nx = 1;
            self.ny = 1;
            self.nz = 1;
            total = 1;
        }

        if self.cells.len() < total {
            self.cells.resize_with(total, Vec::new);
        }
        for cell in self.cells.iter_mut().take(total) {
            cell.clear();
        }

        for &idx in active {
            let li = self.linear_index(positions[idx.slot()]);
            self.cells[li].push(idx);
        }
    }

    fn cell_coords(&self, p: Vec3) -> (usize, usize, usize) {
        let rel = (p - self.min) / self.cell_size;
        let clamp = |v: f32, n: usize| (v.floor().max(0.0) as usize).min(n.saturating_sub(1));
        if self.three_d {
            (
                clamp(rel.x, self.nx),
                clamp(rel.y, self.ny),
                clamp(rel.z, self.nz),
            )
        } else {
            (clamp(rel.x, self.nx), clamp(rel.z, self.ny), 0)
        }
    }

    fn linear_index(&self, p: Vec3) -> usize {
        let (cx, cy, cz) = self.cell_coords(p);
        cx + cy * self.nx + cz * self.nx * self.ny
    }

    /// Collect candidate neighbour indices for position `p` from its cell and
    /// the adjacent cells into `out`. Deterministic order (cells visited in a
    /// fixed z/y/x sweep; each bucket is in `active` insertion order).
    pub fn neighbors_into(&self, p: Vec3, out: &mut Vec<NodeIndex>) {
        out.clear();
        if self.cells.is_empty() {
            return;
        }
        let (cx, cy, cz) = self.cell_coords(p);
        let x0 = cx.saturating_sub(1);
        let x1 = (cx + 1).min(self.nx - 1);
        let y0 = cy.saturating_sub(1);
        let y1 = (cy + 1).min(self.ny - 1);
        let (z0, z1) = if self.three_d {
            (cz.saturating_sub(1), (cz + 1).min(self.nz - 1))
        } else {
            (0, 0)
        };
        for z in z0..=z1 {
            for y in y0..=y1 {
                let base = y * self.nx + z * self.nx * self.ny;
                for x in x0..=x1 {
                    out.extend_from_slice(&self.cells[base + x]);
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.cells.clear();
        self.nx = 0;
        self.ny = 0;
        self.nz = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_and_finds_local_neighbours() {
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),  // idx 0
            Vec3::new(0.5, 0.0, 0.5),  // idx 1 (same cell as 0)
            Vec3::new(50.0, 0.0, 0.0), // idx 2 (far away)
        ];
        let active = vec![NodeIndex(0), NodeIndex(1), NodeIndex(2)];
        let mut grid = Grid::default();
        grid.rebuild(&positions, &active, 2.0, false);

        let mut out = Vec::new();
        grid.neighbors_into(positions[0], &mut out);
        assert!(out.contains(&NodeIndex(0)));
        assert!(out.contains(&NodeIndex(1)));
        assert!(
            !out.contains(&NodeIndex(2)),
            "far node must not be a neighbour"
        );

        grid.neighbors_into(positions[2], &mut out);
        assert!(out.contains(&NodeIndex(2)));
        assert!(!out.contains(&NodeIndex(0)));
    }

    #[test]
    fn three_d_buckets_use_y_axis() {
        let positions = vec![
            Vec3::new(0.0, 0.0, 0.0),  // idx 0
            Vec3::new(0.0, 50.0, 0.0), // idx 1 (far in Y)
        ];
        let active = vec![NodeIndex(0), NodeIndex(1)];
        let mut grid = Grid::default();
        grid.rebuild(&positions, &active, 2.0, true);

        let mut out = Vec::new();
        grid.neighbors_into(positions[0], &mut out);
        assert!(out.contains(&NodeIndex(0)));
        assert!(
            !out.contains(&NodeIndex(1)),
            "node far in Y must not be a 3D neighbour"
        );
    }

    #[test]
    fn every_node_is_its_own_neighbour() {
        // Dense-grid sanity: each node must find itself in its neighbour sweep.
        let positions: Vec<Vec3> = (0..50)
            .map(|i| Vec3::new(i as f32 * 1.5, 0.0, (i % 7) as f32 * 1.5))
            .collect();
        let active: Vec<NodeIndex> = (0..50).map(|i| NodeIndex(i as u32)).collect();
        let mut grid = Grid::default();
        grid.rebuild(&positions, &active, 3.0, false);
        let mut out = Vec::new();
        for (i, &p) in positions.iter().enumerate() {
            grid.neighbors_into(p, &mut out);
            assert!(
                out.contains(&NodeIndex(i as u32)),
                "node {i} missing from its own cell"
            );
        }
    }
}
