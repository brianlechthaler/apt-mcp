use std::env;

use crate::auth::{Authorizer, Scope};

/// Server configuration from environment variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub scopes: Vec<Scope>,
    pub max_output_bytes: usize,
    pub session_id: String,
}

impl Config {
    pub fn from_env() -> Self {
        let scopes = env::var("APT_MCP_SCOPES")
            .map(|s| parse_scopes(&s))
            .unwrap_or_else(|_| vec![Scope::Read]);

        let max_output_bytes = env::var("APT_MCP_MAX_OUTPUT_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1_048_576);

        let session_id = env::var("APT_MCP_SESSION_ID").unwrap_or_else(|_| "default".into());

        Self {
            scopes,
            max_output_bytes,
            session_id,
        }
    }

    pub fn authorizer(&self) -> Authorizer {
        Authorizer::new(self.scopes.clone())
    }
}

fn parse_scopes(s: &str) -> Vec<Scope> {
    s.split(',').filter_map(Scope::parse).collect::<Vec<_>>()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scopes: vec![Scope::Read],
            max_output_bytes: 1_048_576,
            session_id: "default".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_read_only() {
        let cfg = Config::default();
        assert_eq!(cfg.scopes, vec![Scope::Read]);
        assert_eq!(cfg.max_output_bytes, 1_048_576);
    }

    #[test]
    fn parses_scopes() {
        assert_eq!(
            parse_scopes("read,mutate"),
            vec![Scope::Read, Scope::Mutate]
        );
        assert_eq!(parse_scopes("mutate"), vec![Scope::Mutate]);
        assert_eq!(parse_scopes("invalid"), vec![]);
    }

    #[test]
    fn builds_authorizer() {
        let cfg = Config {
            scopes: vec![Scope::Read, Scope::Mutate],
            ..Default::default()
        };
        let auth = cfg.authorizer();
        assert!(auth.may(Scope::Mutate));
    }

    #[test]
    fn from_env_reads_and_defaults() {
        unsafe {
            env::remove_var("APT_MCP_SCOPES");
            env::remove_var("APT_MCP_MAX_OUTPUT_BYTES");
            env::remove_var("APT_MCP_SESSION_ID");
        }
        let defaults = Config::from_env();
        assert_eq!(defaults.scopes, vec![Scope::Read]);
        assert_eq!(defaults.max_output_bytes, 1_048_576);
        assert_eq!(defaults.session_id, "default");

        unsafe {
            env::set_var("APT_MCP_SCOPES", "read,mutate");
            env::set_var("APT_MCP_MAX_OUTPUT_BYTES", "2048");
            env::set_var("APT_MCP_SESSION_ID", "sess-1");
        }
        let cfg = Config::from_env();
        assert_eq!(cfg.scopes, vec![Scope::Read, Scope::Mutate]);
        assert_eq!(cfg.max_output_bytes, 2048);
        assert_eq!(cfg.session_id, "sess-1");

        unsafe {
            env::remove_var("APT_MCP_SCOPES");
            env::remove_var("APT_MCP_MAX_OUTPUT_BYTES");
            env::remove_var("APT_MCP_SESSION_ID");
        }
    }
}
