use regex::Regex;
use std::sync::LazyLock;

use crate::error::AptMcpError;

static PACKAGE_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9+.\-]*$").expect("valid package name regex"));

static SEARCH_PATTERN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9+.\-_\s*?]+$").expect("valid search pattern regex"));

/// Maximum packages per mutate request.
pub const MAX_PACKAGES_PER_REQUEST: usize = 50;

/// Maximum search pattern length.
pub const MAX_SEARCH_PATTERN_LEN: usize = 256;

/// Validate a Debian package name.
pub fn validate_package_name(name: &str) -> Result<(), AptMcpError> {
    if name.is_empty() {
        return Err(AptMcpError::validation("package name must not be empty"));
    }
    if name.len() > 256 {
        return Err(AptMcpError::validation("package name too long"));
    }
    if !PACKAGE_NAME_RE.is_match(name) {
        return Err(AptMcpError::validation(format!(
            "invalid package name: {name}"
        )));
    }
    Ok(())
}

/// Validate a list of package names.
pub fn validate_package_names(names: &[String]) -> Result<(), AptMcpError> {
    if names.is_empty() {
        return Err(AptMcpError::validation(
            "at least one package name is required",
        ));
    }
    if names.len() > MAX_PACKAGES_PER_REQUEST {
        return Err(AptMcpError::validation(format!(
            "too many packages (max {MAX_PACKAGES_PER_REQUEST})"
        )));
    }
    for name in names {
        validate_package_name(name)?;
    }
    Ok(())
}

/// Validate apt-cache search pattern.
pub fn validate_search_pattern(pattern: &str) -> Result<(), AptMcpError> {
    if pattern.is_empty() {
        return Err(AptMcpError::validation("search pattern must not be empty"));
    }
    if pattern.len() > MAX_SEARCH_PATTERN_LEN {
        return Err(AptMcpError::validation("search pattern too long"));
    }
    if !SEARCH_PATTERN_RE.is_match(pattern) {
        return Err(AptMcpError::validation("invalid search pattern characters"));
    }
    Ok(())
}

/// Validate list limit parameter.
pub fn validate_limit(limit: Option<u32>) -> Result<u32, AptMcpError> {
    match limit {
        None => Ok(100),
        Some(0) => Err(AptMcpError::validation("limit must be greater than 0")),
        Some(n) if n > 10_000 => Err(AptMcpError::validation("limit must not exceed 10000")),
        Some(n) => Ok(n),
    }
}

/// Require explicit confirmation for mutating operations.
pub fn require_confirmation(confirmed: bool) -> Result<(), AptMcpError> {
    if confirmed {
        Ok(())
    } else {
        Err(AptMcpError::ConfirmationRequired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_package_names() {
        assert!(validate_package_name("curl").is_ok());
        assert!(validate_package_name("libssl3").is_ok());
        assert!(validate_package_name("python3-pip").is_ok());
        assert!(validate_package_name("g++").is_ok());
    }

    #[test]
    fn rejects_invalid_package_names() {
        assert!(validate_package_name("").is_err());
        assert!(validate_package_name("Bad-Name").is_err());
        assert!(validate_package_name(";rm -rf").is_err());
        assert!(validate_package_name(&"a".repeat(300)).is_err());
        assert!(validate_package_name(&"x".repeat(257)).is_err());
    }

    #[test]
    fn validates_package_list() {
        assert!(validate_package_names(&["curl".into(), "wget".into()]).is_ok());
        assert!(validate_package_names(&[]).is_err());
        let many: Vec<String> = (0..51).map(|i| format!("pkg{i}")).collect();
        assert!(validate_package_names(&many).is_err());
    }

    #[test]
    fn validates_search_pattern() {
        assert!(validate_search_pattern("nginx").is_ok());
        assert!(validate_search_pattern("lib*ssl*").is_ok());
        assert!(validate_search_pattern("").is_err());
        assert!(validate_search_pattern(";drop").is_err());
        assert!(validate_search_pattern(&"a".repeat(300)).is_err());
    }

    #[test]
    fn validates_limit() {
        assert_eq!(validate_limit(None).unwrap(), 100);
        assert_eq!(validate_limit(Some(50)).unwrap(), 50);
        assert!(validate_limit(Some(0)).is_err());
        assert!(validate_limit(Some(20_000)).is_err());
    }

    #[test]
    fn requires_confirmation() {
        assert!(require_confirmation(true).is_ok());
        assert_eq!(
            require_confirmation(false).unwrap_err(),
            AptMcpError::ConfirmationRequired
        );
    }
}
