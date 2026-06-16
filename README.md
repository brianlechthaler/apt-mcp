# apt-mcp

MCP server that exposes apt package management tools for Debian-based Linux. Agents connect over stdio and call tools to search, inspect, and (optionally) modify packages.

## Quick start

```bash
# Build and run tests (Docker)
make test

# Run the server (read-only scope)
docker compose build run
docker compose run --rm run
```

Add to your MCP client config:

```json
{
  "mcpServers": {
    "apt": {
      "command": "docker",
      "args": ["run", "--rm", "-i", "apt-mcp-run", "apt-mcp"],
      "env": {
        "APT_MCP_SCOPES": "read"
      }
    }
  }
}
```

For install/remove/upgrade, set `APT_MCP_SCOPES` to `read,mutate` and pass `confirm: true` on mutating tools.

See [Getting started](docs/getting-started.md) for native binary install and Cursor setup.

## Documentation

- [Getting started](docs/getting-started.md)
- [Architecture](docs/architecture.md)
- [Security](docs/security.md)
- [Tools reference](docs/features/tools.md)

## Requirements

- Debian or derivative with `apt`, `apt-get`, `apt-cache`, and `dpkg`
- Docker (for containerized dev/test)
- Rust 1.88+ (for native builds)

## License

MIT
