use tracing_subscriber::{fmt, EnvFilter};

/// Initialize stderr tracing (idempotent).
pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_tracing_does_not_panic() {
        init_tracing();
        init_tracing();
    }
}
