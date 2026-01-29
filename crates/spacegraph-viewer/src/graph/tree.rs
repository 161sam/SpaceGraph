use bevy::prelude::Vec3;
use spacegraph_core::{Node, NodeId};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub const ROW_SPACING: f32 = 2.0;
const COL_SPACING: f32 = 2.4;
const ROOT_SPACING_UNITS: f32 = 2.0;

pub fn parent_path(path: &str) -> Option<String> {
    Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
}

pub fn layout_tree_positions(
    nodes: &HashMap<NodeId, Node>,
    visible: &HashSet<NodeId>,
    include_roots: &[String],
) -> HashMap<NodeId, Vec3> {
    let mut path_by_id: HashMap<NodeId, String> = HashMap::new();
    let mut non_file_ids: Vec<NodeId> = Vec::new();
    for id in visible.iter() {
        match nodes.get(id) {
            Some(Node::File { path, .. }) => {
                path_by_id.insert(id.clone(), path.clone());
            }
            Some(_) => non_file_ids.push(id.clone()),
            None => {}
        }
    }
    non_file_ids.sort_by(|a, b| a.0.cmp(&b.0));

    let mut positions: HashMap<NodeId, Vec3> = HashMap::new();
    let mut cursor_units = 0.0;
    let mut has_group = false;

    let mut assigned: HashSet<NodeId> = HashSet::new();
    for root in ordered_roots(include_roots) {
        let root_path = Path::new(&root);
        let group_ids: Vec<NodeId> = path_by_id
            .iter()
            .filter(|(id, path)| !assigned.contains(*id) && Path::new(path).starts_with(root_path))
            .map(|(id, _)| id.clone())
            .collect();
        if group_ids.is_empty() {
            continue;
        }
        for id in &group_ids {
            assigned.insert(id.clone());
        }
        if has_group {
            cursor_units += ROOT_SPACING_UNITS;
        }
        cursor_units = layout_group(cursor_units, &group_ids, &path_by_id, &mut positions);
        has_group = true;
    }

    let unassigned: Vec<NodeId> = path_by_id
        .keys()
        .filter(|id| !assigned.contains(*id))
        .cloned()
        .collect();
    if !unassigned.is_empty() {
        if has_group {
            cursor_units += ROOT_SPACING_UNITS;
        }
        cursor_units = layout_group(cursor_units, &unassigned, &path_by_id, &mut positions);
        has_group = true;
    }

    if !non_file_ids.is_empty() {
        if has_group {
            cursor_units += ROOT_SPACING_UNITS;
        }
        layout_non_files(cursor_units, &non_file_ids, &mut positions);
    }

    positions
}

fn ordered_roots(include_roots: &[String]) -> Vec<String> {
    if include_roots.is_empty() {
        return vec!["/".to_string()];
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for root in include_roots {
        if seen.insert(root.as_str()) {
            out.push(root.clone());
        }
    }
    out
}

fn layout_group(
    start_units: f32,
    group_ids: &[NodeId],
    path_by_id: &HashMap<NodeId, String>,
    positions: &mut HashMap<NodeId, Vec3>,
) -> f32 {
    let mut path_to_id: HashMap<String, NodeId> = HashMap::new();
    for id in group_ids {
        if let Some(path) = path_by_id.get(id) {
            path_to_id.insert(path.clone(), id.clone());
        }
    }

    let mut children: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut roots: Vec<NodeId> = Vec::new();
    for id in group_ids {
        let Some(path) = path_by_id.get(id) else {
            continue;
        };
        let parent = parent_path(path).and_then(|parent| path_to_id.get(&parent).cloned());
        if let Some(parent_id) = parent {
            children.entry(parent_id).or_default().push(id.clone());
        } else {
            roots.push(id.clone());
        }
    }

    for list in children.values_mut() {
        sort_children_by_path(list, path_by_id);
    }
    sort_children_by_path(&mut roots, path_by_id);

    let mut widths: HashMap<NodeId, f32> = HashMap::new();
    for root in &roots {
        compute_width(root, &children, &mut widths);
    }

    let mut cursor = start_units;
    for root in &roots {
        cursor = layout_subtree(root, 0, cursor, &children, &widths, positions);
    }
    cursor
}

fn layout_non_files(
    start_units: f32,
    nodes: &[NodeId],
    positions: &mut HashMap<NodeId, Vec3>,
) -> f32 {
    let mut cursor = start_units;
    for id in nodes {
        let x_units = cursor + 0.5;
        positions.insert(id.clone(), Vec3::new(x_units * COL_SPACING, 0.0, 0.0));
        cursor += 1.0;
    }
    cursor
}

fn sort_children_by_path(children: &mut [NodeId], paths: &HashMap<NodeId, String>) {
    children.sort_by(|a, b| {
        let pa = paths.get(a).map(String::as_str).unwrap_or("");
        let pb = paths.get(b).map(String::as_str).unwrap_or("");
        pa.cmp(pb).then_with(|| a.0.cmp(&b.0))
    });
}

fn compute_width(
    id: &NodeId,
    children: &HashMap<NodeId, Vec<NodeId>>,
    widths: &mut HashMap<NodeId, f32>,
) -> f32 {
    if let Some(width) = widths.get(id) {
        return *width;
    }
    let width = match children.get(id) {
        Some(kids) if !kids.is_empty() => kids
            .iter()
            .map(|kid| compute_width(kid, children, widths))
            .sum(),
        _ => 1.0,
    };
    widths.insert(id.clone(), width);
    width
}

fn layout_subtree(
    id: &NodeId,
    depth: usize,
    start_units: f32,
    children: &HashMap<NodeId, Vec<NodeId>>,
    widths: &HashMap<NodeId, f32>,
    positions: &mut HashMap<NodeId, Vec3>,
) -> f32 {
    let width = widths.get(id).copied().unwrap_or(1.0);
    let mut cursor = start_units;
    if let Some(kids) = children.get(id) {
        for kid in kids {
            cursor = layout_subtree(kid, depth + 1, cursor, children, widths, positions);
        }
    }
    let x_units = start_units + width / 2.0;
    positions.insert(
        id.clone(),
        Vec3::new(x_units * COL_SPACING, depth as f32 * ROW_SPACING, 0.0),
    );
    start_units + width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_path_derivation() {
        assert_eq!(
            parent_path("/home/user/report.txt"),
            Some("/home/user".to_string())
        );
        assert_eq!(parent_path("/home"), Some("/".to_string()));
        assert_eq!(parent_path("/"), None);
    }

    #[test]
    fn children_sorted_lexicographically() {
        let a = NodeId("a".to_string());
        let b = NodeId("b".to_string());
        let c = NodeId("c".to_string());
        let mut paths = HashMap::new();
        paths.insert(a.clone(), "/root/a".to_string());
        paths.insert(b.clone(), "/root/b".to_string());
        paths.insert(c.clone(), "/root/c".to_string());
        let mut children = vec![b.clone(), c.clone(), a.clone()];

        sort_children_by_path(&mut children, &paths);

        assert_eq!(children, vec![a, b, c]);
    }
}
