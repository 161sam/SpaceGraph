//! In-house ranking for filesystem search hits (spec §4). No new crate: a tiered
//! substring/subsequence scorer over the candidate path list.
//!
//! Tiers (best → worst): exact filename · filename prefix · path substring ·
//! fuzzy subsequence over the path. Ties break by recency (newer `mtime` first)
//! then path depth (shallower first) then the path itself (stable, deterministic
//! order). Pure functions — fixture-testable, no I/O.

use spacegraph_core::{FileKind, SearchHit};

/// A pre-ranking candidate: a path plus the metadata a [`SearchHit`] carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: String,
    pub kind: FileKind,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
    pub readable: bool,
}

/// Match quality tier. A better tier always outranks a worse one regardless of
/// the secondary (recency/depth) signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Fuzzy = 0,
    Substring = 1,
    Prefix = 2,
    Exact = 3,
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether every char of `needle` appears in order in `haystack` (both already
/// lowercased by the caller).
fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut hay = haystack.chars();
    'next: for nc in needle.chars() {
        for hc in hay.by_ref() {
            if hc == nc {
                continue 'next;
            }
        }
        return false;
    }
    true
}

/// Score `path` against a lowercased `query`. `None` when it matches no tier.
/// The score encodes the tier in the high digits so tier dominates; a tighter
/// (shorter) filename gets a small within-tier bonus.
fn score(query_lc: &str, path: &str) -> Option<i64> {
    if query_lc.is_empty() {
        return Some(0); // empty query matches everything at the lowest tier
    }
    let path_lc = path.to_lowercase();
    let name_lc = file_name(&path_lc).to_string();
    let tier = if name_lc == query_lc {
        Tier::Exact
    } else if name_lc.starts_with(query_lc) {
        Tier::Prefix
    } else if path_lc.contains(query_lc) {
        Tier::Substring
    } else if is_subsequence(query_lc, &path_lc) {
        Tier::Fuzzy
    } else {
        return None;
    };
    let name_bonus = 64i64.saturating_sub(name_lc.len() as i64).max(0);
    Some((tier as i64) * 1_000 + name_bonus)
}

fn depth(path: &str) -> usize {
    path.bytes().filter(|b| *b == b'/').count()
}

/// Rank `candidates` against `query`, returning the top `limit` as ordered hits
/// and a `truncated` flag (true when matches exceeded the cap). Deterministic.
pub fn rank_hits(query: &str, candidates: Vec<Candidate>, limit: usize) -> (Vec<SearchHit>, bool) {
    let query_lc = query.to_lowercase();
    let mut scored: Vec<(i64, Candidate)> = candidates
        .into_iter()
        .filter_map(|c| score(&query_lc, &c.path).map(|s| (s, c)))
        .collect();

    // score desc · mtime desc (recency) · depth asc (shallower) · path asc.
    scored.sort_by(|(sa, a), (sb, b)| {
        sb.cmp(sa)
            .then_with(|| b.mtime.cmp(&a.mtime))
            .then_with(|| depth(&a.path).cmp(&depth(&b.path)))
            .then_with(|| a.path.cmp(&b.path))
    });

    let truncated = scored.len() > limit;
    let results = scored
        .into_iter()
        .take(limit)
        .map(|(_, c)| SearchHit {
            path: c.path,
            kind: c.kind,
            size: c.size,
            mtime: c.mtime,
            readable: c.readable,
        })
        .collect();
    (results, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(path: &str, mtime: Option<i64>) -> Candidate {
        Candidate {
            path: path.to_string(),
            kind: FileKind::Regular,
            size: None,
            mtime,
            readable: true,
        }
    }

    #[test]
    fn ranks_exact_over_prefix_over_substring_over_fuzzy() {
        let candidates = vec![
            cand("/a/b/report_old.txt", None), // substring of "report"
            cand("/x/report", None),           // exact filename
            cand("/deep/reporting.md", None),  // prefix "report"
            cand("/r/e/p/o/r/t/zzz", None),    // fuzzy subsequence r-e-p-o-r-t
        ];
        let (hits, truncated) = rank_hits("report", candidates, 10);
        assert!(!truncated);
        let order: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "/x/report",           // exact
                "/deep/reporting.md",  // prefix
                "/a/b/report_old.txt", // substring
                "/r/e/p/o/r/t/zzz",    // fuzzy
            ]
        );
    }

    #[test]
    fn non_matches_are_dropped() {
        let candidates = vec![cand("/etc/hosts", None), cand("/var/log/syslog", None)];
        let (hits, _) = rank_hits("report", candidates, 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn cap_truncates_and_flags() {
        let candidates: Vec<Candidate> = (0..5)
            .map(|i| cand(&format!("/d/report{i}.txt"), None))
            .collect();
        let (hits, truncated) = rank_hits("report", candidates, 2);
        assert_eq!(hits.len(), 2);
        assert!(truncated, "more matches than the cap must set truncated");
    }

    #[test]
    fn ties_break_by_recency_then_depth() {
        // Same tier (exact "f"); newer mtime wins, then shallower path.
        let candidates = vec![
            cand("/a/b/c/f", Some(100)),
            cand("/a/f", Some(100)),
            cand("/z/f", Some(200)),
        ];
        let (hits, _) = rank_hits("f", candidates, 10);
        let order: Vec<&str> = hits.iter().map(|h| h.path.as_str()).collect();
        assert_eq!(order, vec!["/z/f", "/a/f", "/a/b/c/f"]);
    }
}
