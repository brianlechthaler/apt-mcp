use std::sync::Arc;

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, ErrorData as McpError},
    schemars::JsonSchema,
    tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::apt::{AptCommand, AptExecutor, MockAptExecutor, RealAptExecutor, SimulateAction};
use crate::audit::{AuditEvent, AuditLogger};
use crate::auth::{Authorizer, Scope};
use crate::config::Config;
use crate::error::AptMcpError;
use crate::sanitize::sanitize_output;
use crate::validation::{
    require_confirmation, validate_limit, validate_package_name, validate_package_names,
    validate_search_pattern,
};

/// Shared service state for all apt MCP tools.
#[derive(Clone)]
pub struct AptMcpServer {
    pub tool_router: rmcp::handler::server::tool::ToolRouter<Self>,
    executor: Arc<dyn AptExecutor>,
    authorizer: Authorizer,
    audit: AuditLogger,
    config: Config,
}

impl AptMcpServer {
    pub fn new(config: Config, executor: Arc<dyn AptExecutor>, audit: AuditLogger) -> Self {
        let authorizer = config.authorizer();
        Self {
            tool_router: Self::tool_router(),
            executor,
            authorizer,
            audit,
            config,
        }
    }

    pub fn from_config(config: Config) -> Self {
        Self::new(
            config.clone(),
            Arc::new(RealAptExecutor),
            AuditLogger::default(),
        )
    }

    pub fn with_mock(executor: MockAptExecutor, config: Config, audit: AuditLogger) -> Self {
        Self::new(config, Arc::new(executor), audit)
    }

    async fn run_tool(
        &self,
        tool_name: &str,
        scope: Scope,
        params_json: &str,
        command: AptCommand,
    ) -> Result<CallToolResult, McpError> {
        let correlation_id = Uuid::new_v4().to_string();
        let invoke = AuditEvent::tool_invoke(
            tool_name,
            scope.as_str(),
            params_json,
            &self.config.session_id,
            &correlation_id,
        );
        let _ = self.audit.log(&invoke);

        if let Err(e) = self.authorizer.check(scope) {
            let result = invoke.with_result("denied", 0);
            let _ = self.audit.log(&result);
            return Err(mcp_error(e));
        }

        match self.executor.execute(&command).await {
            Ok(apt_result) => match apt_result.into_output_or_error() {
                Ok(output) => match sanitize_output(&output, self.config.max_output_bytes) {
                    Ok(clean) => {
                        let result = invoke.with_result("success", clean.len());
                        let _ = self.audit.log(&result);
                        Ok(CallToolResult::success(vec![Content::text(clean)]))
                    }
                    Err(e) => {
                        let result = invoke.with_result("error", 0);
                        let _ = self.audit.log(&result);
                        Err(mcp_error(e))
                    }
                },
                Err(e) => {
                    let result = invoke.with_result("error", 0);
                    let _ = self.audit.log(&result);
                    Err(mcp_error(e))
                }
            },
            Err(e) => {
                let result = invoke.with_result("error", 0);
                let _ = self.audit.log(&result);
                Err(mcp_error(e))
            }
        }
    }
}

fn mcp_error(err: AptMcpError) -> McpError {
    match err {
        AptMcpError::PermissionDenied(msg) => McpError::invalid_request(msg, None),
        AptMcpError::Validation(msg) => McpError::invalid_params(msg, None),
        AptMcpError::ConfirmationRequired => {
            McpError::invalid_params("confirmation required for mutating operation", None)
        }
        other => McpError::internal_error(other.to_string(), None),
    }
}

fn params_json<T: Serialize>(params: &T) -> String {
    serde_json::to_string(params).unwrap_or_else(|_| "{}".into())
}

// --- Parameter structs ---

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct SearchParams {
    #[schemars(description = "Package search pattern (apt-cache search)")]
    pattern: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PackageParams {
    #[schemars(description = "Debian package name")]
    package: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PackagesParams {
    #[schemars(description = "List of Debian package names")]
    packages: Vec<String>,
    #[schemars(description = "Must be true to execute mutating operations")]
    confirm: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct ListInstalledParams {
    #[schemars(description = "Maximum number of packages to return")]
    limit: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct ConfirmParams {
    #[schemars(description = "Must be true to execute mutating operations")]
    confirm: bool,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct SimulateParams {
    #[schemars(description = "Action to simulate")]
    action: SimulateAction,
    #[schemars(description = "Package names (required for install/remove/purge)")]
    packages: Option<Vec<String>>,
}

#[tool_router]
impl AptMcpServer {
    #[tool(description = "Search for packages using apt-cache search")]
    async fn apt_search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_search_pattern(&params.pattern).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_search",
            Scope::Read,
            &json,
            AptCommand::Search {
                pattern: params.pattern,
            },
        )
        .await
    }

    #[tool(description = "Show detailed package information (apt-cache show)")]
    async fn apt_show(
        &self,
        Parameters(params): Parameters<PackageParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_package_name(&params.package).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_show",
            Scope::Read,
            &json,
            AptCommand::Show {
                package: params.package,
            },
        )
        .await
    }

    #[tool(description = "Show package version policy (apt-cache policy)")]
    async fn apt_policy(
        &self,
        Parameters(params): Parameters<PackageParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_package_name(&params.package).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_policy",
            Scope::Read,
            &json,
            AptCommand::Policy {
                package: params.package,
            },
        )
        .await
    }

    #[tool(description = "Show package dependencies (apt-cache depends)")]
    async fn apt_depends(
        &self,
        Parameters(params): Parameters<PackageParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_package_name(&params.package).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_depends",
            Scope::Read,
            &json,
            AptCommand::Depends {
                package: params.package,
            },
        )
        .await
    }

    #[tool(description = "Show reverse dependencies (apt-cache rdepends)")]
    async fn apt_rdepends(
        &self,
        Parameters(params): Parameters<PackageParams>,
    ) -> Result<CallToolResult, McpError> {
        validate_package_name(&params.package).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_rdepends",
            Scope::Read,
            &json,
            AptCommand::RDepends {
                package: params.package,
            },
        )
        .await
    }

    #[tool(description = "List installed packages with versions")]
    async fn apt_list_installed(
        &self,
        Parameters(params): Parameters<ListInstalledParams>,
    ) -> Result<CallToolResult, McpError> {
        let limit = validate_limit(params.limit).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_list_installed",
            Scope::Read,
            &json,
            AptCommand::ListInstalled { limit },
        )
        .await
    }

    #[tool(description = "List packages with available upgrades")]
    async fn apt_list_upgradable(&self) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "apt_list_upgradable",
            Scope::Read,
            "{}",
            AptCommand::ListUpgradable,
        )
        .await
    }

    #[tool(
        description = "Update package index (apt-get update). Requires mutate scope and confirm=true"
    )]
    async fn apt_update(
        &self,
        Parameters(params): Parameters<ConfirmParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool("apt_update", Scope::Mutate, &json, AptCommand::Update)
            .await
    }

    #[tool(
        description = "Upgrade installed packages (apt-get upgrade). Requires mutate scope and confirm=true"
    )]
    async fn apt_upgrade(
        &self,
        Parameters(params): Parameters<ConfirmParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_upgrade",
            Scope::Mutate,
            &json,
            AptCommand::Upgrade { simulate: false },
        )
        .await
    }

    #[tool(
        description = "Install packages (apt-get install). Requires mutate scope and confirm=true"
    )]
    async fn apt_install(
        &self,
        Parameters(params): Parameters<PackagesParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        validate_package_names(&params.packages).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_install",
            Scope::Mutate,
            &json,
            AptCommand::Install {
                packages: params.packages,
                simulate: false,
            },
        )
        .await
    }

    #[tool(
        description = "Remove packages (apt-get remove). Requires mutate scope and confirm=true"
    )]
    async fn apt_remove(
        &self,
        Parameters(params): Parameters<PackagesParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        validate_package_names(&params.packages).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_remove",
            Scope::Mutate,
            &json,
            AptCommand::Remove {
                packages: params.packages,
                simulate: false,
                purge: false,
            },
        )
        .await
    }

    #[tool(
        description = "Purge packages and config (apt-get purge). Requires mutate scope and confirm=true"
    )]
    async fn apt_purge(
        &self,
        Parameters(params): Parameters<PackagesParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        validate_package_names(&params.packages).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_purge",
            Scope::Mutate,
            &json,
            AptCommand::Remove {
                packages: params.packages,
                simulate: false,
                purge: true,
            },
        )
        .await
    }

    #[tool(
        description = "Remove unused dependencies (apt-get autoremove). Requires mutate scope and confirm=true"
    )]
    async fn apt_autoremove(
        &self,
        Parameters(params): Parameters<ConfirmParams>,
    ) -> Result<CallToolResult, McpError> {
        require_confirmation(params.confirm).map_err(mcp_error)?;
        let json = params_json(&params);
        self.run_tool(
            "apt_autoremove",
            Scope::Mutate,
            &json,
            AptCommand::Autoremove { simulate: false },
        )
        .await
    }

    #[tool(description = "Dry-run install/remove/upgrade/purge/autoremove (--simulate)")]
    async fn apt_simulate(
        &self,
        Parameters(params): Parameters<SimulateParams>,
    ) -> Result<CallToolResult, McpError> {
        let json = params_json(&params);
        let packages = params.packages.unwrap_or_default();
        if matches!(
            params.action,
            SimulateAction::Install | SimulateAction::Remove | SimulateAction::Purge
        ) {
            validate_package_names(&packages).map_err(mcp_error)?;
        }
        let command = AptCommand::from_simulate(params.action, packages);
        self.run_tool("apt_simulate", Scope::Read, &json, command)
            .await
    }

    #[tool(description = "List configured apt sources (/etc/apt/sources.list)")]
    async fn apt_sources_list(&self) -> Result<CallToolResult, McpError> {
        self.run_tool(
            "apt_sources_list",
            Scope::Read,
            "{}",
            AptCommand::SourcesList,
        )
        .await
    }

    #[tool(description = "Show apt version information")]
    async fn apt_version(&self) -> Result<CallToolResult, McpError> {
        self.run_tool("apt_version", Scope::Read, "{}", AptCommand::Version)
            .await
    }
}

#[tool_handler(
    name = "apt-mcp",
    version = "0.1.0",
    instructions = "MCP server for apt on Debian-based Linux. Read tools work with read scope; install/remove/upgrade require mutate scope and confirm=true."
)]
impl ServerHandler for AptMcpServer {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apt::AptResult;
    use std::io::Write;

    struct TestWriter {
        lines: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let s = String::from_utf8_lossy(buf);
            self.lines.lock().expect("lock").push(s.into_owned());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn test_server_with_config(
        executor: MockAptExecutor,
        config: Config,
    ) -> (AptMcpServer, Arc<std::sync::Mutex<Vec<String>>>) {
        let lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut writer = TestWriter {
            lines: Arc::clone(&lines),
        };
        writer.flush().expect("flush");
        let server = AptMcpServer::with_mock(executor, config, AuditLogger::new(Box::new(writer)));
        (server, lines)
    }

    fn test_server(
        executor: MockAptExecutor,
        scopes: Vec<Scope>,
    ) -> (AptMcpServer, Arc<std::sync::Mutex<Vec<String>>>) {
        test_server_with_config(
            executor,
            Config {
                scopes,
                max_output_bytes: 1_048_576,
                session_id: "test".into(),
            },
        )
    }

    #[tokio::test]
    async fn search_returns_results() {
        let mock = MockAptExecutor::new()
            .with_response("apt-cache search curl", AptResult::success("curl - tool"));
        let (server, _) = test_server(mock, vec![Scope::Read]);
        let result = server
            .apt_search(Parameters(SearchParams {
                pattern: "curl".into(),
            }))
            .await
            .unwrap();
        assert!(format!("{result:?}").contains("curl"));
    }

    #[tokio::test]
    async fn search_rejects_invalid_pattern() {
        let (server, _) = test_server(MockAptExecutor::new(), vec![Scope::Read]);
        assert!(server
            .apt_search(Parameters(SearchParams {
                pattern: ";bad".into(),
            }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn install_denied_without_mutate_scope() {
        let (server, lines) = test_server(MockAptExecutor::new(), vec![Scope::Read]);
        assert!(server
            .apt_install(Parameters(PackagesParams {
                packages: vec!["curl".into()],
                confirm: true,
            }))
            .await
            .is_err());
        let logged = lines.lock().expect("lock").join("");
        assert!(logged.contains("denied"));
    }

    #[tokio::test]
    async fn install_requires_confirmation() {
        let mock = MockAptExecutor::new()
            .with_response("apt-get install -y curl", AptResult::success("installed"));
        let (server, _) = test_server(mock, vec![Scope::Read, Scope::Mutate]);
        assert!(server
            .apt_install(Parameters(PackagesParams {
                packages: vec!["curl".into()],
                confirm: false,
            }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn install_succeeds_with_scope_and_confirm() {
        let mock = MockAptExecutor::new().with_response(
            "apt-get install -y curl",
            AptResult::success("installed curl"),
        );
        let (server, lines) = test_server(mock, vec![Scope::Read, Scope::Mutate]);
        let result = server
            .apt_install(Parameters(PackagesParams {
                packages: vec!["curl".into()],
                confirm: true,
            }))
            .await
            .unwrap();
        assert!(format!("{result:?}").contains("installed"));
        let logged = lines.lock().expect("lock").join("");
        assert!(logged.contains("success"));
    }

    #[tokio::test]
    async fn show_validates_package_name() {
        let (server, _) = test_server(MockAptExecutor::new(), vec![Scope::Read]);
        assert!(server
            .apt_show(Parameters(PackageParams {
                package: "INVALID".into(),
            }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn list_upgradable_calls_executor() {
        let mock = MockAptExecutor::new().with_response(
            "apt list --upgradable",
            AptResult::success("curl/upgradable"),
        );
        let (server, _) = test_server(mock, vec![Scope::Read]);
        let result = server.apt_list_upgradable().await.unwrap();
        assert!(format!("{result:?}").contains("upgradable"));
    }

    #[tokio::test]
    async fn simulate_install_dry_run() {
        let mock = MockAptExecutor::new().with_response(
            "apt-get install -y --simulate curl",
            AptResult::success("simulated"),
        );
        let (server, _) = test_server(mock, vec![Scope::Read]);
        let result = server
            .apt_simulate(Parameters(SimulateParams {
                action: SimulateAction::Install,
                packages: Some(vec!["curl".into()]),
            }))
            .await
            .unwrap();
        assert!(format!("{result:?}").contains("simulated"));
    }

    #[tokio::test]
    async fn update_requires_mutate_scope() {
        let mock = MockAptExecutor::new()
            .with_response("apt-get update -qq", AptResult::success("updated"));
        let (server, _) = test_server(mock, vec![Scope::Read]);
        assert!(server
            .apt_update(Parameters(ConfirmParams { confirm: true }))
            .await
            .is_err());
    }

    #[tokio::test]
    async fn policy_show_depends_rdepends() {
        let mock = MockAptExecutor::new()
            .with_response("apt-cache policy curl", AptResult::success("policy"))
            .with_response("apt-cache depends curl", AptResult::success("depends"))
            .with_response("apt-cache rdepends curl", AptResult::success("rdepends"))
            .with_response("apt-cache show curl", AptResult::success("show"));
        let (server, _) = test_server(mock, vec![Scope::Read]);
        server
            .apt_policy(Parameters(PackageParams {
                package: "curl".into(),
            }))
            .await
            .unwrap();
        server
            .apt_depends(Parameters(PackageParams {
                package: "curl".into(),
            }))
            .await
            .unwrap();
        server
            .apt_rdepends(Parameters(PackageParams {
                package: "curl".into(),
            }))
            .await
            .unwrap();
        server
            .apt_show(Parameters(PackageParams {
                package: "curl".into(),
            }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn mutate_tools_with_confirm() {
        let mock = MockAptExecutor::new()
            .with_response("apt-get upgrade -y", AptResult::success("upgraded"))
            .with_response("apt-get remove -y curl", AptResult::success("removed"))
            .with_response("apt-get purge -y curl", AptResult::success("purged"))
            .with_response("apt-get autoremove -y", AptResult::success("autoremoved"));
        let (server, _) = test_server(mock, vec![Scope::Read, Scope::Mutate]);
        server
            .apt_upgrade(Parameters(ConfirmParams { confirm: true }))
            .await
            .unwrap();
        server
            .apt_remove(Parameters(PackagesParams {
                packages: vec!["curl".into()],
                confirm: true,
            }))
            .await
            .unwrap();
        server
            .apt_purge(Parameters(PackagesParams {
                packages: vec!["curl".into()],
                confirm: true,
            }))
            .await
            .unwrap();
        server
            .apt_autoremove(Parameters(ConfirmParams { confirm: true }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn sources_and_version() {
        let mock = MockAptExecutor::new()
            .with_response(
                "cat /etc/apt/sources.list",
                AptResult::success("deb http://"),
            )
            .with_response("apt-get --version", AptResult::success("apt 2.4"));
        let (server, _) = test_server(mock, vec![Scope::Read]);
        server.apt_sources_list().await.unwrap();
        server.apt_version().await.unwrap();
    }

    #[tokio::test]
    async fn list_installed_with_limit() {
        let mock = MockAptExecutor::new().with_response(
            "dpkg-query -W -f ${Package}\t${Version}\t${Status}\n",
            AptResult::success("a\nb\nc"),
        );
        let (server, _) = test_server(mock, vec![Scope::Read]);
        server
            .apt_list_installed(Parameters(ListInstalledParams { limit: Some(2) }))
            .await
            .unwrap();
        server
            .apt_list_installed(Parameters(ListInstalledParams { limit: None }))
            .await
            .unwrap();
        assert!(server
            .apt_list_installed(Parameters(ListInstalledParams { limit: Some(0) }))
            .await
            .is_err());
    }

    #[test]
    fn mcp_error_mapping() {
        let _ = mcp_error(AptMcpError::PermissionDenied("x".into()));
        let _ = mcp_error(AptMcpError::Validation("x".into()));
        let _ = mcp_error(AptMcpError::ConfirmationRequired);
        let _ = mcp_error(AptMcpError::CommandFailed("x".into()));
        let _ = mcp_error(AptMcpError::OutputTooLarge {
            bytes: 10,
            limit: 5,
        });
        let _ = mcp_error(AptMcpError::Internal("x".into()));
    }

    #[test]
    fn params_json_serializes() {
        let s = params_json(&SearchParams {
            pattern: "curl".into(),
        });
        assert!(s.contains("curl"));
    }

    #[test]
    fn params_json_fallback_on_serialize_error() {
        use serde::ser::{Error as SerError, Serialize, Serializer};

        struct Bad;

        impl Serialize for Bad {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                Err(S::Error::custom("serialize failed"))
            }
        }

        assert_eq!(params_json(&Bad), "{}");
    }

    #[test]
    fn from_config_creates_server() {
        let _server = AptMcpServer::from_config(Config::default());
        let _direct = AptMcpServer::new(
            Config::default(),
            Arc::new(MockAptExecutor::new()),
            AuditLogger::stderr(),
        );
    }

    #[tokio::test]
    async fn command_failure_emits_error_audit() {
        let mock = MockAptExecutor::new().with_response(
            "apt-cache search curl",
            AptResult {
                stdout: String::new(),
                stderr: "failed".into(),
                exit_code: 1,
            },
        );
        let (server, lines) = test_server(mock, vec![Scope::Read]);
        assert!(server
            .apt_search(Parameters(SearchParams {
                pattern: "curl".into(),
            }))
            .await
            .is_err());
        assert!(lines.lock().expect("lock").join("").contains("error"));
    }

    #[tokio::test]
    async fn oversized_output_returns_error() {
        let big = "x".repeat(200);
        let mock =
            MockAptExecutor::new().with_response("apt-cache search curl", AptResult::success(big));
        let (server, _) = test_server_with_config(
            mock,
            Config {
                scopes: vec![Scope::Read],
                max_output_bytes: 100,
                session_id: "test".into(),
            },
        );
        assert!(server
            .apt_search(Parameters(SearchParams {
                pattern: "curl".into(),
            }))
            .await
            .is_err());
    }

    struct FailAptExecutor;

    #[async_trait::async_trait]
    impl AptExecutor for FailAptExecutor {
        async fn execute(&self, _command: &AptCommand) -> Result<AptResult, AptMcpError> {
            Err(AptMcpError::CommandFailed("executor failed".into()))
        }
    }

    fn fail_server() -> AptMcpServer {
        let lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer = TestWriter {
            lines: Arc::clone(&lines),
        };
        AptMcpServer::new(
            Config::default(),
            Arc::new(FailAptExecutor),
            AuditLogger::new(Box::new(writer)),
        )
    }

    #[tokio::test]
    async fn executor_error_is_reported() {
        let server = fail_server();
        assert!(server.apt_version().await.is_err());
    }

    #[tokio::test]
    async fn update_succeeds_with_mutate_scope() {
        let mock = MockAptExecutor::new()
            .with_response("apt-get update -qq", AptResult::success("updated"));
        let (server, _) = test_server(mock, vec![Scope::Read, Scope::Mutate]);
        server
            .apt_update(Parameters(ConfirmParams { confirm: true }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn simulate_upgrade_and_autoremove() {
        let mock = MockAptExecutor::new()
            .with_response("apt-get upgrade -y --simulate", AptResult::success("sim"))
            .with_response(
                "apt-get autoremove -y --simulate",
                AptResult::success("sim"),
            );
        let (server, _) = test_server(mock, vec![Scope::Read]);
        server
            .apt_simulate(Parameters(SimulateParams {
                action: SimulateAction::Upgrade,
                packages: None,
            }))
            .await
            .unwrap();
        server
            .apt_simulate(Parameters(SimulateParams {
                action: SimulateAction::Autoremove,
                packages: None,
            }))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn simulate_remove_and_purge() {
        let mock = MockAptExecutor::new()
            .with_response(
                "apt-get remove -y --simulate curl",
                AptResult::success("sim"),
            )
            .with_response(
                "apt-get purge -y --simulate curl",
                AptResult::success("sim"),
            );
        let (server, _) = test_server(mock, vec![Scope::Read]);
        server
            .apt_simulate(Parameters(SimulateParams {
                action: SimulateAction::Remove,
                packages: Some(vec!["curl".into()]),
            }))
            .await
            .unwrap();
        server
            .apt_simulate(Parameters(SimulateParams {
                action: SimulateAction::Purge,
                packages: Some(vec!["curl".into()]),
            }))
            .await
            .unwrap();
    }
}
