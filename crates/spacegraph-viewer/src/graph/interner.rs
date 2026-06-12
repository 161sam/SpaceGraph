//! Node-ID interning: project `NodeId` (string-keyed truth identity) onto dense
//! `u32` indices for the viewer's hot paths.
//!
//! `GraphModel` keeps `NodeId` as the public, deterministic identity. The
//! interner is a **viewer-internal projection**: per-node spatial state
//! (positions, velocities, glow) is stored in flat `Vec`s indexed by
//! [`NodeIndex`], so layout can do array indexing instead of `HashMap<NodeId,
//! _>` lookups and per-frame `NodeId` clones.
//!
//! Slots are reused via a free list. Reuse is safe **because all per-node state
//! keyed by an index is cleared when the slot is released** (see
//! `SpatialState::release` / `clear_slot`), so a recycled index never inherits
//! a previous node's position or glow.

use spacegraph_core::NodeId;
use std::collections::HashMap;

/// Dense index for a node within the viewer's spatial storage.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct NodeIndex(pub u32);

impl NodeIndex {
    #[inline]
    pub fn slot(self) -> usize {
        self.0 as usize
    }
}

/// Bidirectional `NodeId` ⇄ `NodeIndex` map with free-list slot reuse.
#[derive(Default)]
pub struct NodeInterner {
    to_index: HashMap<NodeId, NodeIndex>,
    /// `slot → NodeId`. `None` marks a freed slot awaiting reuse.
    to_id: Vec<Option<NodeId>>,
    free: Vec<NodeIndex>,
}

impl NodeInterner {
    /// Map `id` to its index, allocating (or reusing a freed) slot if new.
    pub fn intern(&mut self, id: &NodeId) -> NodeIndex {
        if let Some(&idx) = self.to_index.get(id) {
            return idx;
        }
        let idx = if let Some(slot) = self.free.pop() {
            self.to_id[slot.slot()] = Some(id.clone());
            slot
        } else {
            let slot = NodeIndex(self.to_id.len() as u32);
            self.to_id.push(Some(id.clone()));
            slot
        };
        self.to_index.insert(id.clone(), idx);
        idx
    }

    /// Resolve an index back to its `NodeId`, or `None` if the slot is free.
    pub fn resolve(&self, idx: NodeIndex) -> Option<&NodeId> {
        self.to_id.get(idx.slot()).and_then(|slot| slot.as_ref())
    }

    /// Look up an existing index without allocating.
    pub fn index_of(&self, id: &NodeId) -> Option<NodeIndex> {
        self.to_index.get(id).copied()
    }

    /// Release `id`'s slot (returned index is pushed onto the free list for
    /// reuse). Callers must clear the per-index state for the returned slot.
    pub fn release(&mut self, id: &NodeId) -> Option<NodeIndex> {
        let idx = self.to_index.remove(id)?;
        self.to_id[idx.slot()] = None;
        self.free.push(idx);
        Some(idx)
    }

    /// Number of slots ever allocated (live + freed). Per-index `Vec`s are sized
    /// to this so every live index is addressable.
    pub fn capacity(&self) -> usize {
        self.to_id.len()
    }

    /// Number of live (interned) nodes.
    pub fn len(&self) -> usize {
        self.to_index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.to_index.is_empty()
    }

    /// Iterate live `(index, id)` pairs in slot order. Freed slots are skipped.
    pub fn iter(&self) -> impl Iterator<Item = (NodeIndex, &NodeId)> + '_ {
        self.to_id
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|id| (NodeIndex(i as u32), id)))
    }

    pub fn clear(&mut self) {
        self.to_index.clear();
        self.to_id.clear();
        self.free.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> NodeId {
        NodeId(s.to_string())
    }

    #[test]
    fn intern_is_idempotent_and_resolves() {
        let mut it = NodeInterner::default();
        let a = it.intern(&id("a"));
        let a2 = it.intern(&id("a"));
        let b = it.intern(&id("b"));
        assert_eq!(a, a2);
        assert_ne!(a, b);
        assert_eq!(it.resolve(a), Some(&id("a")));
        assert_eq!(it.resolve(b), Some(&id("b")));
        assert_eq!(it.index_of(&id("b")), Some(b));
        assert_eq!(it.len(), 2);
    }

    #[test]
    fn release_frees_slot_and_reuses_it() {
        let mut it = NodeInterner::default();
        let a = it.intern(&id("a"));
        let _b = it.intern(&id("b"));
        assert_eq!(it.capacity(), 2);

        let freed = it.release(&id("a"));
        assert_eq!(freed, Some(a));
        assert_eq!(it.resolve(a), None);
        assert_eq!(it.index_of(&id("a")), None);
        assert_eq!(it.len(), 1);

        // New node reuses the freed slot — capacity does not grow.
        let c = it.intern(&id("c"));
        assert_eq!(c, a, "freed slot should be reused");
        assert_eq!(it.capacity(), 2);
        assert_eq!(it.resolve(c), Some(&id("c")));
    }

    #[test]
    fn iter_skips_freed_slots() {
        let mut it = NodeInterner::default();
        it.intern(&id("a"));
        let b = it.intern(&id("b"));
        it.intern(&id("c"));
        it.release(&id("b"));
        let live: Vec<&NodeId> = it.iter().map(|(_, id)| id).collect();
        assert_eq!(live.len(), 2);
        assert!(!live.contains(&&id("b")));
        // b's slot is gone from iteration but still counted in capacity.
        assert_eq!(it.index_of(&id("b")), None);
        let _ = b;
    }
}
