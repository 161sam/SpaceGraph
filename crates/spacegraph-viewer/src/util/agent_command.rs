use crate::util::config::{AgentEndpoint, AgentMode, PathPolicyConfig};

pub fn build_agent_command(
    _agent: &AgentEndpoint,
    policy: &PathPolicyConfig,
    mode: AgentMode,
) -> String {
    let exe = if cfg!(windows) {
        "spacegraph-agent.exe"
    } else {
        "spacegraph-agent"
    };
    let mut parts = vec![
        exe.to_string(),
        "--mode".to_string(),
        mode.as_str().to_string(),
    ];

    let mut includes = policy.includes.clone();
    includes.sort();
    includes.dedup();
    for include in includes {
        parts.push("--include".to_string());
        parts.push(include);
    }

    let mut excludes = policy.excludes.clone();
    excludes.sort();
    excludes.dedup();
    for exclude in excludes {
        parts.push("--exclude".to_string());
        parts.push(exclude);
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::config::{AgentEndpointKind, PathPolicyConfig};

    #[test]
    fn build_agent_command_includes_mode_and_paths() {
        let agent = AgentEndpoint {
            name: "local".to_string(),
            kind: AgentEndpointKind::UdsPath("/tmp/spacegraph.sock".to_string()),
            auto_connect: false,
            mode_override: None,
        };
        let policy = PathPolicyConfig {
            includes: vec!["/var".to_string(), "/etc".to_string()],
            excludes: vec!["/sys".to_string(), "/proc".to_string()],
        };

        let cmd = build_agent_command(&agent, &policy, AgentMode::Privileged);

        assert_eq!(
            cmd,
            "spacegraph-agent --mode privileged --include /etc --include /var --exclude /proc --exclude /sys"
        );
    }
}
