<!-- generated-by: sqlbuild skills -->

# sqb debug

> Validate project configuration and test the warehouse connection.

Online: https://sqlbuild.com/docs/cli/debug/

Checks that your project config is valid, the adapter is resolvable, and the warehouse connection works. Useful for diagnosing setup issues.

For detailed progress from any other command, use the global [`--debug`](build.md#global-options) option instead, as in `sqb --debug build`.

## Usage

```bash
sqb debug [flags]
```

## What it checks

The command reports four groups of checks:

**Runtime** - SQLBuild version, Python version, Python path, OS info.

**Configuration** - Finds and validates `sqlbuild_project.toml`, loads `sqlbuild_local.toml` if present, resolves the adapter, active target, connection, database, and schema. For Snowflake it also shows the warehouse each [command group](../concepts/project-configuration.md#command-group-warehouses) connects with and where that value came from.

**Providers** - Lists the [providers](../concepts/python-nodes/providers.md) the project defines.

**Connection** - Displays connection settings as `sqb debug` itself uses them, with the `query` warehouse group applied (secrets are masked), attempts to connect to the warehouse, and runs `SELECT 1` to verify query execution.

## Flags

| Flag | Description |
|------|-------------|
| `--no-connection` | Skip the connection and query tests. Useful for validating config without warehouse access. |
| `--json` | Output results as JSON instead of formatted text. |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/project-configuration.md#command-group-warehouses) and the connection warehouse |

## Example

```bash
sqb debug
```

```
SQLBuild Diagnostics

Runtime:
  sqlbuild version: 0.134.8
  python version: 3.14.3
  python path: /home/user/.venv/bin/python3
  os info: Linux-6.6.87

Configuration:
  project file: /home/user/waffle_shop/sqlbuild_project.toml [OK found and valid]
  local config: /home/user/waffle_shop/sqlbuild_local.toml [OK found]
  project: waffle_shop [OK loaded]
  adapter: snowflake [OK found]
  target: dev [OK resolved]
  connection: snowflake_dev [OK resolved]
  database: ANALYTICS [OK resolved]
  schema: DEV [OK resolved]
  build warehouse: BUILD_WH [OK sqlbuild_project.toml [targets.dev.warehouses] build]
  query warehouse: TRANSFORM_WH [OK connection warehouse]

Providers:
  providers: none discovered [OK none discovered]

Connection:
  account: myorg-myaccount
  authenticator: programmatic_access_token
  database: ANALYTICS
  role: TRANSFORMER
  schema: DEV
  token: ****
  user: SQLBUILD_USER
  warehouse: TRANSFORM_WH
  connection test: [OK connected]
  query test: [OK SELECT 1]
```

## Exit codes

Returns `0` when all checks pass, `1` when any check fails (e.g. connection refused, invalid config).
A failed run is recorded in [Recall](../concepts/observability/recall.md) with the first failed
check as its cause:

| `error_code` | `error_type` | Meaning |
|------|------|---------|
| `C980` | `ConnectionError` | The connection test failed |
| `C981` | `QueryError` | The connection worked but `SELECT 1` failed |
| `C982` | `ConfigurationError` | Another check failed, such as an invalid configuration file |
