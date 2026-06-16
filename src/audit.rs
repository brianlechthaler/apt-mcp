use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Structured audit event for SIEM ingestion.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub tool_name: String,
    pub caller_identity: String,
    pub authorization_scope: String,
    pub params_fingerprint: String,
    pub result_status: String,
    pub result_bytes: usize,
    pub session_id: String,
    pub correlation_id: String,
}

impl AuditEvent {
    pub fn tool_invoke(
        tool_name: &str,
        scope: &str,
        params_json: &str,
        session_id: &str,
        correlation_id: &str,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type: "mcp.tool.invoke".into(),
            tool_name: tool_name.into(),
            caller_identity: "apt-mcp-server".into(),
            authorization_scope: scope.into(),
            params_fingerprint: fingerprint(params_json),
            result_status: "pending".into(),
            result_bytes: 0,
            session_id: session_id.into(),
            correlation_id: correlation_id.into(),
        }
    }

    pub fn with_result(mut self, status: &str, bytes: usize) -> Self {
        self.event_type = "mcp.tool.result".into();
        self.result_status = status.into();
        self.result_bytes = bytes;
        self
    }
}

fn fingerprint(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

/// JSON-lines audit logger.
#[derive(Clone)]
pub struct AuditLogger {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl AuditLogger {
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    pub fn stderr() -> Self {
        Self::new(Box::new(std::io::stderr()))
    }

    pub fn log(&self, event: &AuditEvent) -> std::io::Result<()> {
        let line = serde_json::to_string(event).map_err(std::io::Error::other)?;
        let mut guard = self
            .writer
            .lock()
            .map_err(|_| std::io::Error::other("audit lock poisoned"))?;
        writeln!(guard, "{line}")?;
        Ok(())
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::stderr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_deterministic() {
        let a = fingerprint("test");
        let b = fingerprint("test");
        assert_eq!(a, b);
        assert_ne!(a, fingerprint("other"));
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn creates_invoke_event() {
        let event = AuditEvent::tool_invoke("apt_search", "read", r#"{"q":"curl"}"#, "s1", "c1");
        assert_eq!(event.event_type, "mcp.tool.invoke");
        assert_eq!(event.tool_name, "apt_search");
        assert_eq!(event.result_status, "pending");
    }

    #[test]
    fn with_result_updates_event() {
        let event = AuditEvent::tool_invoke("apt_install", "mutate", "{}", "s1", "c1")
            .with_result("success", 42);
        assert_eq!(event.event_type, "mcp.tool.result");
        assert_eq!(event.result_status, "success");
        assert_eq!(event.result_bytes, 42);
    }

    #[test]
    fn logs_json_line() {
        let buf: Vec<u8> = Vec::new();
        let logger = AuditLogger::new(Box::new(buf));
        let event = AuditEvent::tool_invoke("apt_show", "read", "{}", "s", "c");
        logger.log(&event).unwrap();
    }

    #[test]
    fn serializes_event() {
        let event = AuditEvent::tool_invoke("apt_show", "read", "{}", "s", "c");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("mcp.tool.invoke"));
        assert!(json.contains("apt_show"));
    }

    #[test]
    fn stderr_logger_default() {
        let _logger = AuditLogger::stderr();
        let _default = AuditLogger::default();
    }

    #[test]
    fn log_returns_error_on_poisoned_lock() {
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(Box::new(Vec::<u8>::new())));
        let poison_target = Arc::clone(&writer);
        let _ = std::thread::spawn(move || {
            let _guard = poison_target.lock().expect("lock");
            panic!("poison audit lock");
        })
        .join();
        assert!(writer.lock().is_err());
        let logger = AuditLogger { writer };
        let event = AuditEvent::tool_invoke("t", "read", "{}", "s", "c");
        assert!(logger.log(&event).is_err());
    }

    #[test]
    fn log_returns_error_when_write_fails() {
        struct FailingWriter;
        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("write failed"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let logger = AuditLogger::new(Box::new(FailingWriter));
        let event = AuditEvent::tool_invoke("t", "read", "{}", "s", "c");
        assert!(logger.log(&event).is_err());
        let mut writer = FailingWriter;
        assert!(writer.flush().is_ok());
    }
}
