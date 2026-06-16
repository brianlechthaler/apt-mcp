use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AptMcpError {
    #[error("validation error: {0}")]
    Validation(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("apt command failed: {0}")]
    CommandFailed(String),

    #[error("output too large: {bytes} bytes exceeds limit {limit}")]
    OutputTooLarge { bytes: usize, limit: usize },

    #[error("confirmation required for mutating operation")]
    ConfirmationRequired,

    #[error("internal error: {0}")]
    Internal(String),
}

impl AptMcpError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn permission(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }
}
