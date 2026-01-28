use anyhow::Result;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentMode {
    User,
    Privileged,
}

impl AgentMode {
    pub fn parse(input: &str) -> Result<Self> {
        match input {
            "user" => Ok(Self::User),
            "privileged" => Ok(Self::Privileged),
            _ => anyhow::bail!("invalid mode: {input} (expected user|privileged)"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub mode: AgentMode,
    pub includes: Vec<PathBuf>,
    pub excludes: Vec<PathBuf>,
}

pub fn parse_args() -> Result<AgentConfig> {
    parse_args_from(std::env::args_os().skip(1))
}

fn parse_args_from<I>(args: I) -> Result<AgentConfig>
where
    I: IntoIterator<Item = OsString>,
{
    let mut mode = AgentMode::User;
    let mut includes = Vec::new();
    let mut excludes = Vec::new();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        if arg == "--include" {
            let Some(path) = args.next() else {
                anyhow::bail!("--include expects a path");
            };
            includes.push(PathBuf::from(path));
        } else if arg == "--exclude" {
            let Some(path) = args.next() else {
                anyhow::bail!("--exclude expects a path");
            };
            excludes.push(PathBuf::from(path));
        } else if arg == "--mode" {
            let Some(value) = args.next() else {
                anyhow::bail!("--mode expects user|privileged");
            };
            let value = value.to_string_lossy();
            mode = AgentMode::parse(&value)?;
        } else {
            anyhow::bail!("unknown argument: {:?}", arg);
        }
    }

    Ok(AgentConfig {
        mode,
        includes,
        excludes,
    })
}

pub fn default_includes(mode: AgentMode) -> Vec<PathBuf> {
    match mode {
        AgentMode::User | AgentMode::Privileged => vec![
            PathBuf::from("/etc"),
            PathBuf::from("/home"),
            PathBuf::from("/var"),
        ],
    }
}

pub fn default_excludes(mode: AgentMode) -> Vec<PathBuf> {
    match mode {
        AgentMode::User => vec![
            PathBuf::from("/proc"),
            PathBuf::from("/sys"),
            PathBuf::from("/dev"),
            PathBuf::from("/run"),
            PathBuf::from("/etc/cni/net.d"),
        ],
        AgentMode::Privileged => vec![
            PathBuf::from("/proc"),
            PathBuf::from("/sys"),
            PathBuf::from("/dev"),
        ],
    }
}

pub fn should_warn_privileged_without_root(mode: AgentMode, euid: u32) -> bool {
    matches!(mode, AgentMode::Privileged) && euid != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn parses_mode_flag() {
        let args = vec![OsString::from("--mode"), OsString::from("privileged")];
        let config = parse_args_from(args).expect("config parsed");
        assert_eq!(config.mode, AgentMode::Privileged);
    }

    #[test]
    fn default_excludes_include_cni_only_in_user_mode() {
        let user = default_excludes(AgentMode::User);
        let privileged = default_excludes(AgentMode::Privileged);
        assert!(user.contains(&PathBuf::from("/etc/cni/net.d")));
        assert!(!privileged.contains(&PathBuf::from("/etc/cni/net.d")));
    }

    #[test]
    fn warns_when_privileged_without_root() {
        assert!(should_warn_privileged_without_root(
            AgentMode::Privileged,
            1000
        ));
        assert!(!should_warn_privileged_without_root(AgentMode::User, 0));
    }
}
