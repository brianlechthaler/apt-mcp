use std::fmt;

use serde::{Deserialize, Serialize};

/// Authorization scope for tool invocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Read,
    Mutate,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Mutate => "mutate",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "read" => Some(Self::Read),
            "mutate" => Some(Self::Mutate),
            _ => None,
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Per-call authorizer enforcing least privilege.
#[derive(Debug, Clone)]
pub struct Authorizer {
    allowed: Vec<Scope>,
}

impl Authorizer {
    pub fn new(allowed: Vec<Scope>) -> Self {
        Self { allowed }
    }

    pub fn read_only() -> Self {
        Self::new(vec![Scope::Read])
    }

    pub fn full() -> Self {
        Self::new(vec![Scope::Read, Scope::Mutate])
    }

    pub fn allowed_scopes(&self) -> &[Scope] {
        &self.allowed
    }

    pub fn may(&self, required: Scope) -> bool {
        self.allowed.contains(&required)
    }

    pub fn check(&self, required: Scope) -> Result<(), crate::error::AptMcpError> {
        if self.may(required) {
            Ok(())
        } else {
            Err(crate::error::AptMcpError::permission(format!(
                "scope '{required}' not granted; allowed: {}",
                self.allowed
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            )))
        }
    }
}

impl Default for Authorizer {
    fn default() -> Self {
        Self::read_only()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_denies_mutate() {
        let auth = Authorizer::read_only();
        assert!(auth.may(Scope::Read));
        assert!(!auth.may(Scope::Mutate));
        assert!(auth.check(Scope::Read).is_ok());
        assert!(auth.check(Scope::Mutate).is_err());
    }

    #[test]
    fn full_allows_both() {
        let auth = Authorizer::full();
        assert!(auth.check(Scope::Read).is_ok());
        assert!(auth.check(Scope::Mutate).is_ok());
    }

    #[test]
    fn parses_scope_strings() {
        assert_eq!(Scope::parse("read"), Some(Scope::Read));
        assert_eq!(Scope::parse("MUTATE"), Some(Scope::Mutate));
        assert_eq!(Scope::parse("admin"), None);
    }

    #[test]
    fn displays_scope() {
        assert_eq!(Scope::Read.to_string(), "read");
        assert_eq!(Scope::Mutate.to_string(), "mutate");
    }

    #[test]
    fn authorizer_api() {
        let auth = Authorizer::new(vec![Scope::Mutate]);
        assert_eq!(auth.allowed_scopes(), &[Scope::Mutate]);
        assert_eq!(Authorizer::default().allowed_scopes(), &[Scope::Read]);
        let err = auth.check(Scope::Read).unwrap_err().to_string();
        assert!(err.contains("read"));
        assert!(err.contains("mutate"));
    }
}
