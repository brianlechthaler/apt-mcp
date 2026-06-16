# Getting started

## Install

### Docker (recommended)

```bash
git clone https://github.com/brianlechthaler/apt-mcp.git
cd apt-mcp
docker compose build
```

The production image runs as non-root user `aptmcp` (UID 10001) with apt tooling installed.

### Native build

Requires Rust 1.88+:

```bash
cargo build --release
```

Binary: `target/release/apt-mcp`

## Configure MCP client

### Cursor / Claude Desktop (stdio)

```json
{
  "mcpServers": {
    "apt": {
      "command": "/path/to/apt-mcp",
      "env": {
        "APT_MCP_SCOPES": "read",
        "RUST_LOG": "info"
      }
    }
  }
}
```

### Docker stdio

```json
{
  "mcpServers": {
    "apt": {
      "command": "docker",
      "args": [
        "run", "--rm", "-i",
        "-e", "APT_MCP_SCOPES=read",
        "ghcr.io/brianlechthaler/apt-mcp:latest"
      ]
    }
  }
}
```

Mutating operations need `APT_MCP_SCOPES=read,mutate` and `confirm: true` on each tool call.

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `APT_MCP_SCOPES` | `read` | Comma-separated: `read`, `mutate` |
| `APT_MCP_MAX_OUTPUT_BYTES` | `1048576` | Max tool output size (1 MiB) |
| `APT_MCP_SESSION_ID` | `default` | Session ID in audit logs |
| `RUST_LOG` | `info` | Tracing filter |

## Development

```bash
make test      # unit tests in container
make lint      # fmt + clippy
make coverage  # 100% line coverage gate
make build     # production image
```

## Troubleshooting

**Permission denied on install/remove**

Default scope is read-only. Set `APT_MCP_SCOPES=read,mutate` and pass `confirm: true`.

**confirmation required for mutating operation**

Mutating tools require `confirm: true` in parameters. This is intentional.

**Output too large**

Raise `APT_MCP_MAX_OUTPUT_BYTES` or narrow queries (e.g. use `apt_search` with a specific pattern).

## Related

- [Tools reference](features/tools.md)
- [Security](security.md)
