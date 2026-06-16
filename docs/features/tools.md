# Tools reference

All tools are versioned under server `apt-mcp@0.1.0`.

## Read scope (default)

### apt_search

Search packages (`apt-cache search`).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | yes | Search pattern |

### apt_show

Package metadata (`apt-cache show`).

| Parameter | Type | Required |
|-----------|------|----------|
| `package` | string | yes |

### apt_policy

Version policy (`apt-cache policy`).

| Parameter | Type | Required |
|-----------|------|----------|
| `package` | string | yes |

### apt_depends

Dependencies (`apt-cache depends`).

| Parameter | Type | Required |
|-----------|------|----------|
| `package` | string | yes |

### apt_rdepends

Reverse dependencies (`apt-cache rdepends`).

| Parameter | Type | Required |
|-----------|------|----------|
| `package` | string | yes |

### apt_list_installed

Installed packages with versions (`dpkg-query`).

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 100 | Max lines returned |

### apt_list_upgradable

Packages with upgrades available (`apt list --upgradable`).

No parameters.

### apt_simulate

Dry-run mutating operations (`--simulate`). Does not require mutate scope.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | enum | yes | `install`, `remove`, `upgrade`, `purge`, `autoremove` |
| `packages` | string[] | for install/remove/purge | Package names |

### apt_sources_list

Contents of `/etc/apt/sources.list`.

No parameters.

### apt_version

`apt-get --version` output.

No parameters.

## Mutate scope

Requires `APT_MCP_SCOPES=read,mutate` and `confirm: true`.

### apt_update

Refresh package index (`apt-get update`).

| Parameter | Type | Required |
|-----------|------|----------|
| `confirm` | boolean | yes (must be `true`) |

### apt_upgrade

Upgrade installed packages (`apt-get upgrade -y`).

| Parameter | Type | Required |
|-----------|------|----------|
| `confirm` | boolean | yes |

### apt_install

Install packages (`apt-get install -y`).

| Parameter | Type | Required |
|-----------|------|----------|
| `packages` | string[] | yes |
| `confirm` | boolean | yes |

### apt_remove

Remove packages (`apt-get remove -y`).

| Parameter | Type | Required |
|-----------|------|----------|
| `packages` | string[] | yes |
| `confirm` | boolean | yes |

### apt_purge

Purge packages and config (`apt-get purge -y`).

| Parameter | Type | Required |
|-----------|------|----------|
| `packages` | string[] | yes |
| `confirm` | boolean | yes |

### apt_autoremove

Remove unused dependencies (`apt-get autoremove -y`).

| Parameter | Type | Required |
|-----------|------|----------|
| `confirm` | boolean | yes |

## Examples

Search:

```json
{ "pattern": "nginx" }
```

Simulate install:

```json
{ "action": "install", "packages": ["curl"] }
```

Install (mutate):

```json
{ "packages": ["curl"], "confirm": true }
```

## Related

- [Getting started](../getting-started.md)
- [Security](../security.md)
