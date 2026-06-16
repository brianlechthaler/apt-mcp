# Security

apt-mcp follows [NSA CSI MCP Security Design Considerations](https://www.nsa.gov/Portals/75/documents/Cybersecurity/CSI_MCP_SECURITY.pdf) (PP-26-1834).

## Threat model

| Asset | Risk | Mitigation |
|-------|------|------------|
| Host package state | Agent installs/removes arbitrary packages | Scope gating, `confirm: true`, package name validation |
| System integrity | Command injection via tool params | No shell; strict argv; regex validation |
| Secrets in apt output | Leak to agent or logs | Output sanitization, size caps |
| Tool definition drift | Rug-pull attacks | Versioned server (`apt-mcp@0.1.0`), pinned tool schemas |
| Privilege escalation | Over-broad default access | Default `read` scope only |

## Controls implemented

### Least privilege (per call)

- `APT_MCP_SCOPES` limits granted scopes at startup
- Each tool declares required scope (`read` or `mutate`)
- Mutating tools require `confirm: true`

### Input validation

- Package names: `^[a-z0-9][a-z0-9+.\-]*$`
- Search patterns: alphanumeric plus limited wildcards
- Max 50 packages per mutate request
- Max output: `APT_MCP_MAX_OUTPUT_BYTES` (default 1 MiB)

### Audit logging

Structured JSON-lines to stderr (one event per invoke + result):

| Field | Purpose |
|-------|---------|
| `event_type` | `mcp.tool.invoke` / `mcp.tool.result` |
| `tool_name` | Tool identifier |
| `authorization_scope` | Scope for this call |
| `params_fingerprint` | SHA-256 of params (not raw secrets) |
| `result_status` | `success` / `denied` / `error` |
| `correlation_id` | Trace ID per invocation |

### Sandboxing

Docker image:

- Non-root user (`aptmcp`, UID 10001)
- Minimal Debian slim base
- No network bind (stdio transport)
- Read-only root filesystem recommended at deploy time

### Fail closed

Validation, auth, and policy errors deny the call and emit audit events. No silent downgrade to broader access.

### Gateway placement

Deploy agents to reach apt-mcp through a gateway with DLP and rate limits. Do not expose apt-mcp directly to untrusted networks.

### Message integrity

TLS applies to HTTP gateways only. Per-message MCP signing is not yet universal; verify at gateway when available.

## Operator checklist

- [ ] Set `APT_MCP_SCOPES=read` unless mutate is required
- [ ] Route MCP traffic through a filtering proxy
- [ ] Ship audit logs to SIEM
- [ ] Pin server image digest in production
- [ ] Run container with dropped capabilities and no host root mount

## Related

- [Architecture](architecture.md)
- [Tools](features/tools.md)
