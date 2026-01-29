use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PathPolicy {
    includes: Vec<PathBuf>,
    excludes: Vec<PathBuf>,
}

impl PathPolicy {
    pub fn new(includes: Vec<PathBuf>, excludes: Vec<PathBuf>) -> Self {
        Self { includes, excludes }
    }

    pub fn normalize(&mut self) {
        self.includes = self
            .includes
            .iter()
            .map(|path| normalize_path(path))
            .collect();
        self.excludes = self
            .excludes
            .iter()
            .map(|path| normalize_exclude_path(path))
            .collect();
    }

    pub fn includes(&self) -> &[PathBuf] {
        &self.includes
    }

    pub fn excludes(&self) -> &[PathBuf] {
        &self.excludes
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        let path = normalize_path(path);
        self.excludes.iter().any(|exclude| {
            if exclude.is_absolute() {
                path.starts_with(exclude)
            } else {
                path.components()
                    .any(|component| component.as_os_str() == exclude.as_os_str())
            }
        })
    }

    pub fn is_included(&self, path: &Path) -> bool {
        let path = normalize_path(path);
        self.includes
            .iter()
            .any(|include| path.starts_with(include))
    }

    pub fn should_watch(&self, path: &Path) -> bool {
        if self.is_excluded(path) {
            return false;
        }

        if self.includes.is_empty() {
            return true;
        }

        self.is_included(path)
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn normalize_exclude_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_path(path)
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::PathPolicy;
    use std::fs;
    use std::path::PathBuf;

    fn temp_root(name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        root.push(format!(
            "spacegraph-path-policy-{name}-{}",
            std::process::id()
        ));
        root
    }

    #[test]
    fn excludes_override_includes() {
        let root = temp_root("override");
        let include = root.join("include");
        let exclude = include.join("blocked");
        let target = exclude.join("file.txt");

        fs::create_dir_all(&exclude).unwrap();

        let mut policy = PathPolicy::new(vec![include], vec![exclude.clone()]);
        policy.normalize();

        assert!(!policy.should_watch(&target));
    }

    #[test]
    fn default_includes_empty_means_all_except_excludes() {
        let root = temp_root("default");
        let excluded = root.join("skip");
        let allowed = root.join("keep");

        fs::create_dir_all(&excluded).unwrap();
        fs::create_dir_all(&allowed).unwrap();

        let mut policy = PathPolicy::new(Vec::new(), vec![excluded.clone()]);
        policy.normalize();

        assert!(policy.should_watch(&allowed));
        assert!(!policy.should_watch(&excluded.join("child")));
    }

    #[test]
    fn prefix_matching_works() {
        let root = temp_root("prefix");
        let include = root.join("root");
        let included_child = include.join("child");
        let excluded_child = root.join("other");

        fs::create_dir_all(&include).unwrap();
        fs::create_dir_all(&excluded_child).unwrap();

        let mut policy = PathPolicy::new(vec![include], Vec::new());
        policy.normalize();

        assert!(policy.should_watch(&included_child));
        assert!(!policy.should_watch(&excluded_child));
    }

    #[test]
    fn canonicalize_best_effort_keeps_nonexistent_paths() {
        let include = PathBuf::from("/nonexistent-spacegraph-path-policy");
        let candidate = include.join("child");

        let mut policy = PathPolicy::new(vec![include], Vec::new());
        policy.normalize();

        assert!(policy.should_watch(&candidate));
    }

    #[test]
    fn relative_excludes_match_by_component() {
        let root = temp_root("relative");
        let excluded = root.join("node_modules");
        let target = excluded.join("file.txt");

        fs::create_dir_all(&excluded).unwrap();

        let mut policy = PathPolicy::new(Vec::new(), vec![PathBuf::from("node_modules")]);
        policy.normalize();

        assert!(!policy.should_watch(&target));
    }
}
