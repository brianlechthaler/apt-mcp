//! apt-mcp: MCP server for apt on Debian-based Linux distributions.

pub mod apt;
pub mod audit;
pub mod auth;
pub mod config;
pub mod error;
pub mod runtime;
pub mod sanitize;
pub mod server;
pub mod validation;

pub use config::Config;
pub use error::AptMcpError;
