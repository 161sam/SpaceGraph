//! ATT&CK coverage (D5, ADR-0006 §3): detected vs undetected techniques from the
//! rule registry, grouped by tactic — the "how well am I covered" view. Pure read
//! of the registry + the **vendored** `TECHNIQUES` table; **no live ATT&CK fetch**
//! (O-7'). Coverage is honest by construction: a technique with no mapped rule
//! shows as a gap.

use std::collections::HashSet;

use crate::graph::rules::{RuleRegistry, Tactic, TECHNIQUES};

/// One technique's coverage status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechniqueCoverage {
    pub id: &'static str,
    pub name: &'static str,
    pub detected: bool,
}

/// A tactic column of the coverage heatmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TacticCoverage {
    pub tactic: Tactic,
    pub techniques: Vec<TechniqueCoverage>,
    pub detected: usize,
    pub total: usize,
}

/// The set of techniques a registry rule maps to (the `technique ↔ rule` map).
fn covered_techniques() -> HashSet<&'static str> {
    RuleRegistry::default()
        .rules()
        .iter()
        .map(|r| r.technique())
        .collect()
}

/// Coverage grouped by tactic (kill-chain order); tactics with no vendored
/// technique are omitted. Pure — the single source of truth for the heatmap view.
pub fn coverage() -> Vec<TacticCoverage> {
    let covered = covered_techniques();
    let mut out = Vec::new();
    for tactic in Tactic::ALL {
        let techniques: Vec<TechniqueCoverage> = TECHNIQUES
            .iter()
            .filter(|t| t.tactic == tactic)
            .map(|t| TechniqueCoverage {
                id: t.id,
                name: t.name,
                detected: covered.contains(t.id),
            })
            .collect();
        if techniques.is_empty() {
            continue;
        }
        let detected = techniques.iter().filter(|t| t.detected).count();
        let total = techniques.len();
        out.push(TacticCoverage {
            tactic,
            techniques,
            detected,
            total,
        });
    }
    out
}

/// Overall detection coverage ratio (detected techniques / vendored techniques),
/// in `0.0..=1.0`.
pub fn coverage_ratio() -> f32 {
    let total = TECHNIQUES.len();
    if total == 0 {
        return 0.0;
    }
    let covered = covered_techniques();
    let detected = TECHNIQUES.iter().filter(|t| covered.contains(t.id)).count();
    detected as f32 / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_marks_mapped_techniques_detected() {
        let cov = coverage();
        // Flatten to (id, detected).
        let all: Vec<(&str, bool)> = cov
            .iter()
            .flat_map(|t| t.techniques.iter().map(|x| (x.id, x.detected)))
            .collect();
        // The 3 rules map T1021/T1571/T1071 (detected); T1041 has no rule (gap).
        assert!(all.contains(&("T1021", true)));
        assert!(all.contains(&("T1571", true)));
        assert!(all.contains(&("T1071", true)));
        assert!(
            all.contains(&("T1041", false)),
            "T1041 is an honest coverage gap"
        );
    }

    #[test]
    fn coverage_is_tactic_grouped_and_counted() {
        let cov = coverage();
        // CommandAndControl groups T1071 + T1571, both detected.
        let c2 = cov
            .iter()
            .find(|t| t.tactic == Tactic::CommandAndControl)
            .expect("C2 tactic present");
        assert_eq!(c2.total, 2);
        assert_eq!(c2.detected, 2);
        // Exfiltration has T1041, undetected.
        let exfil = cov
            .iter()
            .find(|t| t.tactic == Tactic::Exfiltration)
            .expect("Exfiltration present");
        assert_eq!(exfil.detected, 0);
        assert_eq!(exfil.total, 1);
    }

    #[test]
    fn coverage_ratio_reflects_gaps() {
        // 3 of 4 vendored techniques are mapped → 0.75.
        assert!((coverage_ratio() - 0.75).abs() < 1e-6);
    }
}
