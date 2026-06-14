//! System locate backend (spec §2, decision D-1). Prefer `plocate`, then
//! `locate`, then `mlocate`. The shell-out sits behind [`LocateBackend`] so it
//! is mockable in tests; detection and parsing are pure.

use std::io;
use std::process::Command;

/// Which system locate binary to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocateKind {
    Plocate,
    Locate,
    Mlocate,
}

impl LocateKind {
    pub fn binary(self) -> &'static str {
        match self {
            LocateKind::Plocate => "plocate",
            LocateKind::Locate => "locate",
            LocateKind::Mlocate => "mlocate",
        }
    }
}

/// A queryable system locate index. The real implementation shells out; tests
/// substitute a mock.
pub trait LocateBackend: Send + Sync {
    /// Return up to `limit` paths matching `pattern` (case-insensitive
    /// substring — locate semantics).
    fn query(&self, pattern: &str, limit: u32) -> io::Result<Vec<String>>;
}

/// Detect the preferred locate binary, using `present` to test whether a binary
/// name resolves. Preference order: plocate > locate > mlocate (D-1).
pub fn detect_locate_with(present: impl Fn(&str) -> bool) -> Option<LocateKind> {
    [LocateKind::Plocate, LocateKind::Locate, LocateKind::Mlocate]
        .into_iter()
        .find(|kind| present(kind.binary()))
}

/// Detect the preferred locate binary on the real `PATH`.
pub fn detect_locate() -> Option<LocateKind> {
    detect_locate_with(binary_on_path)
}

fn binary_on_path(bin: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file())
}

/// Parse newline-separated locate stdout into a trimmed, non-empty path list.
pub fn parse_locate_output(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

/// The real system locate backend (shells out via `std::process`).
pub struct SystemLocate {
    kind: LocateKind,
}

impl SystemLocate {
    pub fn new(kind: LocateKind) -> Self {
        Self { kind }
    }
}

impl LocateBackend for SystemLocate {
    fn query(&self, pattern: &str, limit: u32) -> io::Result<Vec<String>> {
        // `<bin> -i -l <limit> -- <pattern>`: case-insensitive, capped, with `--`
        // guarding a pattern that starts with '-'. plocate/locate/mlocate share
        // these flags. locate exits 1 on "no matches" — that is not an error, we
        // just parse whatever stdout it produced.
        let output = Command::new(self.kind.binary())
            .arg("-i")
            .arg("-l")
            .arg(limit.to_string())
            .arg("--")
            .arg(pattern)
            .output()?;
        Ok(parse_locate_output(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }
}

/// In-memory locate backend for tests: a fixed path list filtered by
/// case-insensitive substring.
#[cfg(test)]
pub(crate) struct MockLocate {
    pub paths: Vec<String>,
}

#[cfg(test)]
impl LocateBackend for MockLocate {
    fn query(&self, pattern: &str, limit: u32) -> io::Result<Vec<String>> {
        let needle = pattern.to_lowercase();
        Ok(self
            .paths
            .iter()
            .filter(|p| p.to_lowercase().contains(&needle))
            .take(limit as usize)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_prefers_plocate_then_locate_then_mlocate() {
        // plocate present → plocate, regardless of the others.
        assert_eq!(
            detect_locate_with(|b| b == "plocate" || b == "locate"),
            Some(LocateKind::Plocate)
        );
        // no plocate, locate present → locate.
        assert_eq!(
            detect_locate_with(|b| b == "locate" || b == "mlocate"),
            Some(LocateKind::Locate)
        );
        // only mlocate → mlocate.
        assert_eq!(
            detect_locate_with(|b| b == "mlocate"),
            Some(LocateKind::Mlocate)
        );
    }

    #[test]
    fn detect_absent_is_none() {
        assert_eq!(detect_locate_with(|_| false), None);
    }

    #[test]
    fn parse_locate_fixture_stdout() {
        let stdout = "/etc/hosts\n/home/u/report.pdf\n\n/var/log/syslog\r\n";
        let paths = parse_locate_output(stdout);
        assert_eq!(
            paths,
            vec![
                "/etc/hosts".to_string(),
                "/home/u/report.pdf".to_string(),
                "/var/log/syslog".to_string(),
            ],
            "blank lines dropped, CR trimmed"
        );
    }

    #[test]
    fn mock_locate_filters_and_caps() {
        let mock = MockLocate {
            paths: vec![
                "/a/report1".to_string(),
                "/a/report2".to_string(),
                "/a/other".to_string(),
            ],
        };
        let hits = mock.query("report", 1).unwrap();
        assert_eq!(hits.len(), 1, "limit caps the mock too");
        assert!(hits[0].contains("report"));
    }
}
