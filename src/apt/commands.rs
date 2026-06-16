use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// High-level apt operations mapped to argv (no shell).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AptCommand {
    Search {
        pattern: String,
    },
    Show {
        package: String,
    },
    Policy {
        package: String,
    },
    Depends {
        package: String,
    },
    RDepends {
        package: String,
    },
    ListInstalled {
        limit: u32,
    },
    ListUpgradable,
    Update,
    Upgrade {
        simulate: bool,
    },
    Install {
        packages: Vec<String>,
        simulate: bool,
    },
    Remove {
        packages: Vec<String>,
        simulate: bool,
        purge: bool,
    },
    Autoremove {
        simulate: bool,
    },
    SourcesList,
    Version,
}

/// Action type for simulate tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SimulateAction {
    Install,
    Remove,
    Upgrade,
    Purge,
    Autoremove,
}

impl AptCommand {
    /// Build argv for execution. Never uses a shell.
    pub fn argv(&self) -> Vec<String> {
        match self {
            Self::Search { pattern } => vec!["apt-cache".into(), "search".into(), pattern.clone()],
            Self::Show { package } => {
                vec!["apt-cache".into(), "show".into(), package.clone()]
            }
            Self::Policy { package } => {
                vec!["apt-cache".into(), "policy".into(), package.clone()]
            }
            Self::Depends { package } => {
                vec!["apt-cache".into(), "depends".into(), package.clone()]
            }
            Self::RDepends { package } => {
                vec!["apt-cache".into(), "rdepends".into(), package.clone()]
            }
            Self::ListInstalled { .. } => vec![
                "dpkg-query".into(),
                "-W".into(),
                "-f".into(),
                "${Package}\t${Version}\t${Status}\n".into(),
            ],
            Self::ListUpgradable => vec!["apt".into(), "list".into(), "--upgradable".into()],
            Self::Update => vec!["apt-get".into(), "update".into(), "-qq".into()],
            Self::Upgrade { simulate } => {
                let mut args = vec!["apt-get".into(), "upgrade".into(), "-y".into()];
                if *simulate {
                    args.push("--simulate".into());
                }
                args
            }
            Self::Install { packages, simulate } => {
                let mut args = vec!["apt-get".into(), "install".into(), "-y".into()];
                if *simulate {
                    args.push("--simulate".into());
                }
                args.extend(packages.iter().cloned());
                args
            }
            Self::Remove {
                packages,
                simulate,
                purge,
            } => {
                let sub = if *purge { "purge" } else { "remove" };
                let mut args = vec!["apt-get".into(), sub.into(), "-y".into()];
                if *simulate {
                    args.push("--simulate".into());
                }
                args.extend(packages.iter().cloned());
                args
            }
            Self::Autoremove { simulate } => {
                let mut args = vec!["apt-get".into(), "autoremove".into(), "-y".into()];
                if *simulate {
                    args.push("--simulate".into());
                }
                args
            }
            Self::SourcesList => vec!["cat".into(), "/etc/apt/sources.list".into()],
            Self::Version => vec!["apt-get".into(), "--version".into()],
        }
    }

    pub fn from_simulate(action: SimulateAction, packages: Vec<String>) -> Self {
        match action {
            SimulateAction::Install => Self::Install {
                packages,
                simulate: true,
            },
            SimulateAction::Remove => Self::Remove {
                packages,
                simulate: true,
                purge: false,
            },
            SimulateAction::Purge => Self::Remove {
                packages,
                simulate: true,
                purge: true,
            },
            SimulateAction::Upgrade => Self::Upgrade { simulate: true },
            SimulateAction::Autoremove => Self::Autoremove { simulate: true },
        }
    }
}

/// Post-process list-installed output with limit.
pub fn limit_installed_output(output: &str, limit: u32) -> String {
    output
        .lines()
        .take(limit as usize)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_argv() {
        let cmd = AptCommand::Search {
            pattern: "curl".into(),
        };
        assert_eq!(
            cmd.argv(),
            vec!["apt-cache", "search", "curl"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn install_simulate_argv() {
        let cmd = AptCommand::Install {
            packages: vec!["curl".into()],
            simulate: true,
        };
        let argv = cmd.argv();
        assert!(argv.contains(&"--simulate".to_string()));
        assert!(argv.contains(&"curl".to_string()));
    }

    #[test]
    fn purge_argv() {
        let cmd = AptCommand::Remove {
            packages: vec!["oldpkg".into()],
            simulate: false,
            purge: true,
        };
        assert!(cmd.argv().contains(&"purge".to_string()));
    }

    #[test]
    fn from_simulate_actions() {
        let cmd = AptCommand::from_simulate(SimulateAction::Install, vec!["a".into()]);
        assert!(matches!(cmd, AptCommand::Install { simulate: true, .. }));
    }

    #[test]
    fn limits_installed_output() {
        let out = "a\nb\nc\nd";
        assert_eq!(limit_installed_output(out, 2), "a\nb");
    }

    #[test]
    fn argv_for_all_commands() {
        assert!(!AptCommand::Show {
            package: "curl".into()
        }
        .argv()
        .is_empty());
        assert!(AptCommand::Policy {
            package: "curl".into()
        }
        .argv()
        .contains(&"policy".to_string()));
        assert!(AptCommand::Depends {
            package: "curl".into()
        }
        .argv()
        .contains(&"depends".to_string()));
        assert!(AptCommand::RDepends {
            package: "curl".into()
        }
        .argv()
        .contains(&"rdepends".to_string()));
        assert_eq!(
            AptCommand::ListInstalled { limit: 1 }.argv()[0],
            "dpkg-query"
        );
        assert!(AptCommand::ListUpgradable
            .argv()
            .contains(&"--upgradable".to_string()));
        assert!(AptCommand::Update.argv().contains(&"update".to_string()));
        assert!(!AptCommand::Upgrade { simulate: false }
            .argv()
            .contains(&"--simulate".to_string()));
        assert!(AptCommand::Upgrade { simulate: true }
            .argv()
            .contains(&"--simulate".to_string()));
        assert!(!AptCommand::Install {
            packages: vec!["a".into()],
            simulate: false,
        }
        .argv()
        .contains(&"--simulate".to_string()));
        assert!(AptCommand::Remove {
            packages: vec!["a".into()],
            simulate: true,
            purge: false,
        }
        .argv()
        .contains(&"remove".to_string()));
        assert!(AptCommand::Autoremove { simulate: true }
            .argv()
            .contains(&"--simulate".to_string()));
        assert!(AptCommand::SourcesList
            .argv()
            .contains(&"/etc/apt/sources.list".to_string()));
    }

    #[test]
    fn from_simulate_all_actions() {
        for action in [
            SimulateAction::Install,
            SimulateAction::Remove,
            SimulateAction::Purge,
            SimulateAction::Upgrade,
            SimulateAction::Autoremove,
        ] {
            let cmd = AptCommand::from_simulate(action, vec!["pkg".into()]);
            assert!(!cmd.argv().is_empty());
        }
    }
}
