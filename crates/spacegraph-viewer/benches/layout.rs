//! Layout / visibility hot-path benchmarks.
//!
//! Runs the two per-frame cost centres against deterministic synthetic graphs:
//!   * `force_step`         — the spatial force-layout integration step.
//!   * `visible_set_capped` — projection of the model to the capped visible set.
//!
//! Sizes 500/1000/2000/5000 mirror the gates in `docs/perf/BASELINE.md` and
//! `docs/ACCEPTANCE.md`. Sample size is kept small because the pre-optimisation
//! `force_step` is O(N²) and a single iteration at 5000 nodes is expensive.

use std::collections::HashSet;
use std::hint::black_box;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use spacegraph_core::NodeId;
use spacegraph_viewer::graph::GraphState;

const SIZES: [usize; 4] = [500, 1000, 2000, 5000];

/// Build a state with all `n` nodes placed, ready for `force_step`.
fn placed_state(n: usize) -> (GraphState, HashSet<NodeId>) {
    let mut st = GraphState::default();
    st.cfg.max_visible_nodes = n + 16;
    st.cfg.progressive_nodes_per_frame = n + 16;
    st.load_synthetic_graph(n);
    let vis = st.visible_set_capped();
    st.progressive_prepare(&vis); // place every node in one pass
    (st, vis)
}

fn bench_force_step(c: &mut Criterion) {
    let mut group = c.benchmark_group("force_step");
    for &n in SIZES.iter() {
        let (mut st, vis) = placed_state(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                st.force_step(black_box(&vis), 0.016);
            });
        });
    }
    group.finish();
}

fn bench_visible_set_capped(c: &mut Criterion) {
    let mut group = c.benchmark_group("visible_set_capped");
    for &n in SIZES.iter() {
        let mut st = GraphState::default();
        // Cap below total so the capping path actually runs.
        st.cfg.max_visible_nodes = (n * 3 / 4).max(1);
        st.load_synthetic_graph(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(st.visible_set_capped());
            });
        });
    }
    group.finish();
}

fn config() -> Criterion {
    Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(3))
}

criterion_group! {
    name = benches;
    config = config();
    targets = bench_force_step, bench_visible_set_capped
}
criterion_main!(benches);
