use regex::Regex;
use std::sync::LazyLock;

use crate::error::AptMcpError;

static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)(password|passwd|secret|token|api[_-]?key)\s*[:=]\s*\S+")
            .expect("secret pattern"),
        Regex::new(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----").expect("pem pattern"),
    ]
});

/// Cap output size and redact known secret patterns.
pub fn sanitize_output(output: &str, max_bytes: usize) -> Result<String, AptMcpError> {
    let bytes = output.as_bytes();
    let truncated = if bytes.len() > max_bytes {
        String::from_utf8_lossy(&bytes[..max_bytes]).into_owned()
    } else {
        output.to_string()
    };

    if bytes.len() > max_bytes {
        return Err(AptMcpError::OutputTooLarge {
            bytes: bytes.len(),
            limit: max_bytes,
        });
    }

    let mut result = truncated;
    for pattern in SECRET_PATTERNS.iter() {
        result = pattern.replace_all(&result, "[REDACTED]").into_owned();
    }
    Ok(result)
}

/// Truncate output for display when over limit (used before error path).
pub fn truncate_for_error(output: &str, max_bytes: usize) -> String {
    if output.len() <= max_bytes {
        output.to_string()
    } else {
        format!("{}...[truncated]", &output[..max_bytes.min(output.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_clean_output() {
        let out = sanitize_output("package curl installed", 1024).unwrap();
        assert_eq!(out, "package curl installed");
    }

    #[test]
    fn rejects_oversized_output() {
        let big = "x".repeat(200);
        let err = sanitize_output(&big, 100).unwrap_err();
        assert!(matches!(err, AptMcpError::OutputTooLarge { .. }));
    }

    #[test]
    fn redacts_secrets() {
        let out = sanitize_output("password: s3cret\nok", 1024).unwrap();
        assert!(!out.contains("s3cret"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_private_keys() {
        let out = sanitize_output("-----BEGIN RSA PRIVATE KEY-----\ndata", 1024).unwrap();
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn truncates_for_error() {
        let s = "hello world";
        assert_eq!(truncate_for_error(s, 100), s);
        let t = truncate_for_error("abcdefghij", 5);
        assert!(t.contains("truncated"));
    }
}
