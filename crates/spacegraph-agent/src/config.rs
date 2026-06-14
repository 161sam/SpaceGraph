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
    pub uds_path: Option<PathBuf>,
    /// Network source (procfs sockets → process/socket/remote-host topology).
    pub net_enabled: bool,
    pub net_poll_secs: u64,
    pub net_include: Vec<String>, // CIDR allowlist for remote hosts (empty = all)
    pub net_exclude: Vec<String>, // CIDR blocklist for remote hosts
    /// Suricata EVE JSON file to tail for alerts (None disables the source).
    pub eve_file: Option<PathBuf>,
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
    let mut uds_path = None;
    let mut net_enabled = true;
    let mut net_poll_secs = 2u64;
    let mut net_include = Vec::new();
    let mut net_exclude = Vec::new();
    let mut eve_file = None;
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
        } else if arg == "--no-net" {
            net_enabled = false;
        } else if arg == "--net-poll-secs" {
            let Some(value) = args.next() else {
                anyhow::bail!("--net-poll-secs expects a number");
            };
            net_poll_secs = value
                .to_string_lossy()
                .parse()
                .map_err(|_| anyhow::anyhow!("--net-poll-secs expects a number"))?;
        } else if arg == "--net-include" {
            let Some(value) = args.next() else {
                anyhow::bail!("--net-include expects a CIDR");
            };
            net_include.push(value.to_string_lossy().to_string());
        } else if arg == "--net-exclude" {
            let Some(value) = args.next() else {
                anyhow::bail!("--net-exclude expects a CIDR");
            };
            net_exclude.push(value.to_string_lossy().to_string());
        } else if arg == "--eve-file" {
            let Some(path) = args.next() else {
                anyhow::bail!("--eve-file expects a path");
            };
            eve_file = Some(PathBuf::from(path));
        } else if arg == "--mode" {
            let Some(value) = args.next() else {
                anyhow::bail!("--mode expects user|privileged");
            };
            let value = value.to_string_lossy();
            mode = AgentMode::parse(&value)?;
        } else if arg == "--uds" || arg == "--socket" {
            let Some(path) = args.next() else {
                anyhow::bail!("--uds expects a path");
            };
            uds_path = Some(PathBuf::from(path));
        } else {
            anyhow::bail!("unknown argument: {:?}", arg);
        }
    }

    Ok(AgentConfig {
        mode,
        includes,
        excludes,
        uds_path,
        net_enabled,
        net_poll_secs: net_poll_secs.max(1),
        net_include,
        net_exclude,
        eve_file,
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
            PathBuf::from("node_modules"),
            PathBuf::from(".git"),
            PathBuf::from("target"),
            PathBuf::from(".cache"),
        ],
        AgentMode::Privileged => vec![
            PathBuf::from("/proc"),
            PathBuf::from("/sys"),
            PathBuf::from("/dev"),
            PathBuf::from("node_modules"),
            PathBuf::from(".git"),
            PathBuf::from("target"),
            PathBuf::from(".cache"),
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

    #[test]
    fn parses_uds_flag() {
        let args = vec![OsString::from("--uds"), OsString::from("/tmp/test.sock")];
        let config = parse_args_from(args).expect("config parsed");
        assert_eq!(config.uds_path, Some(PathBuf::from("/tmp/test.sock")));
    }
}
