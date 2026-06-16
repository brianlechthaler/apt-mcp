use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use tokio::process::Command;

use super::commands::{limit_installed_output, AptCommand};
use crate::error::AptMcpError;

/// Result of an apt command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AptResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl AptResult {
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }

    pub fn into_output_or_error(self) -> Result<String, AptMcpError> {
        if self.exit_code == 0 {
            Ok(self.combined_output())
        } else {
            Err(AptMcpError::CommandFailed(format!(
                "exit {}: {}",
                self.exit_code,
                self.combined_output()
            )))
        }
    }
}

/// Trait for executing apt commands (mockable for tests).
#[async_trait::async_trait]
pub trait AptExecutor: Send + Sync {
    async fn execute(&self, command: &AptCommand) -> Result<AptResult, AptMcpError>;
}

/// Real executor using tokio::process (no shell).
pub struct RealAptExecutor;

#[async_trait::async_trait]
impl AptExecutor for RealAptExecutor {
    async fn execute(&self, command: &AptCommand) -> Result<AptResult, AptMcpError> {
        Self::execute_argv(command.argv(), command).await
    }
}

impl RealAptExecutor {
    async fn execute_argv(
        argv: Vec<String>,
        command: &AptCommand,
    ) -> Result<AptResult, AptMcpError> {
        if argv.is_empty() {
            return Err(AptMcpError::Internal("empty argv".into()));
        }

        let program = &argv[0];
        let args: Vec<&str> = argv[1..].iter().map(String::as_str).collect();

        let output = Command::new(program)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| AptMcpError::CommandFailed(e.to_string()))?;

        let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if let AptCommand::ListInstalled { limit } = command {
            stdout = limit_installed_output(&stdout, *limit);
        }

        Ok(AptResult {
            stdout,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

/// Mock executor for unit tests.
#[derive(Default)]
pub struct MockAptExecutor {
    responses: Arc<Mutex<HashMap<String, AptResult>>>,
}

impl MockAptExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_response(self, key: &str, result: AptResult) -> Self {
        self.responses
            .lock()
            .expect("lock")
            .insert(key.to_string(), result);
        self
    }

    fn key(command: &AptCommand) -> String {
        command.argv().join(" ")
    }
}

#[async_trait::async_trait]
impl AptExecutor for MockAptExecutor {
    async fn execute(&self, command: &AptCommand) -> Result<AptResult, AptMcpError> {
        let key = Self::key(command);
        self.responses
            .lock()
            .expect("lock")
            .get(&key)
            .cloned()
            .ok_or_else(|| AptMcpError::CommandFailed(format!("no mock for {key}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apt::commands::SimulateAction;

    #[test]
    fn apt_result_success() {
        let r = AptResult::success("ok");
        assert_eq!(r.exit_code, 0);
        assert!(r.into_output_or_error().is_ok());
    }

    #[test]
    fn apt_result_failure() {
        let r = AptResult {
            stdout: String::new(),
            stderr: "fail".into(),
            exit_code: 1,
        };
        assert!(r.into_output_or_error().is_err());
    }

    #[test]
    fn combined_output_merges_streams() {
        let r = AptResult {
            stdout: "out".into(),
            stderr: "err".into(),
            exit_code: 0,
        };
        assert!(r.combined_output().contains("out"));
        assert!(r.combined_output().contains("err"));
    }

    #[tokio::test]
    async fn mock_executor_returns_configured_response() {
        let cmd = AptCommand::Search {
            pattern: "curl".into(),
        };
        let executor = MockAptExecutor::new().with_response(
            "apt-cache search curl",
            AptResult::success("curl - command line tool"),
        );
        let result = executor.execute(&cmd).await.unwrap();
        assert!(result.stdout.contains("curl"));
    }

    #[tokio::test]
    async fn mock_executor_errors_on_missing_key() {
        let executor = MockAptExecutor::new();
        let cmd = AptCommand::Version;
        assert!(executor.execute(&cmd).await.is_err());
    }

    #[tokio::test]
    async fn simulate_command_key() {
        let cmd = AptCommand::from_simulate(SimulateAction::Upgrade, vec![]);
        let _ = MockAptExecutor::key(&cmd);
    }

    #[tokio::test]
    async fn real_executor_runs_apt_version() {
        let executor = RealAptExecutor;
        let result = executor.execute(&AptCommand::Version).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("apt"));
    }

    #[tokio::test]
    async fn real_executor_limits_installed_list() {
        let executor = RealAptExecutor;
        let result = executor
            .execute(&AptCommand::ListInstalled { limit: 3 })
            .await
            .unwrap();
        assert!(result.stdout.lines().count() <= 3);
    }

    #[test]
    fn combined_output_stdout_only() {
        let r = AptResult {
            stdout: "only".into(),
            stderr: String::new(),
            exit_code: 0,
        };
        assert_eq!(r.combined_output(), "only");
    }

    #[tokio::test]
    async fn execute_argv_rejects_empty() {
        let err = RealAptExecutor::execute_argv(vec![], &AptCommand::Version)
            .await
            .unwrap_err();
        assert!(matches!(err, AptMcpError::Internal(_)));
    }

    #[tokio::test]
    async fn execute_argv_spawn_failure() {
        let err = RealAptExecutor::execute_argv(
            vec!["/nonexistent-apt-mcp-binary".into()],
            &AptCommand::Version,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, AptMcpError::CommandFailed(_)));
    }
}
