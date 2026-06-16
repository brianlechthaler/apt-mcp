use anyhow::Result;
use apt_mcp::{config::Config, runtime, server::AptMcpServer};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> Result<()> {
    runtime::init_tracing();

    let config = Config::from_env();
    let server = AptMcpServer::from_config(config);
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}
