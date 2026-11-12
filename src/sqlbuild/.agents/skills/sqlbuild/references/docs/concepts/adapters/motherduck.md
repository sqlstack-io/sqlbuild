<!-- generated-by: sqlbuild skills -->

# MotherDuck

> MotherDuck adapter configuration for SQLBuild.

Online: https://sqlbuild.com/docs/concepts/adapters/motherduck/

MotherDuck uses DuckDB's built-in `md:` connection support. No extra installation needed beyond the core DuckDB dependency.

## DuckDB 2.0 support

SQLBuild requires DuckDB 2.0, and MotherDuck does not support DuckDB 2.0 yet: its extension
supports DuckDB up to 1.5.6 and refuses to load on 2.0. Connecting reports
`MotherDuck does not support DuckDB 2.0.0 yet`. If you use MotherDuck, stay on SQLBuild 0.182.0,
the last release that supports DuckDB 1.5, until MotherDuck supports DuckDB 2.0.

## Connection config

```toml
adapter = "motherduck"
default_target = "dev"

[connections.motherduck]
database = "my_database"
token = "your_motherduck_token"

[targets.dev]
connection = "motherduck"
database = "my_database"
schema = "dev"
```

| Field | Description |
|-------|-------------|
| `database` | MotherDuck database name. Automatically prefixed with `md:` if not already present. Defaults to `md:` (your default MotherDuck database). |
| `token` | MotherDuck access token. Can also be set via environment variable. |

## Authentication

MotherDuck requires an access token. Generate one from the MotherDuck UI and pass it via the connection config or an environment variable:

```toml
[connections.motherduck]
database = "my_database"
token = "${ENV:MOTHERDUCK_TOKEN}"
```

## Shared connections across targets

Use targets to separate production and development databases on MotherDuck:

```toml
adapter = "motherduck"

[connections.motherduck]
token = "${ENV:MOTHERDUCK_TOKEN}"
database = "my_database"

[targets.prod]
connection = "motherduck"
database = "prod_db"
schema = "prod"

[targets.dev]
connection = "motherduck"
database = "dev_db"
schema = "dev"
```

## Local development with DuckDB

Use `sqlbuild_local.toml` to override the adapter for local development against a plain DuckDB file:

```toml
adapter = "duckdb"

[connections.local]
database = "local_dev.duckdb"

[targets.dev]
connection = "local"
schema = "dev"
```

This lets you develop and test locally with zero MotherDuck compute cost, then deploy to MotherDuck in production. SQLBuild's [scenario replay](../scenarios.md) also runs locally in DuckDB regardless of the production adapter.

## Functions in unit tests

`sqb test` copies each SQL UDF and table function the selected tests call into a session
`TEMP MACRO`, the same as [DuckDB](duckdb.md#functions-in-unit-tests), so the
functions in your MotherDuck database are not replaced. See [Functions in `sqb test`](../testing-reference.md#functions-in-sqb-test).
