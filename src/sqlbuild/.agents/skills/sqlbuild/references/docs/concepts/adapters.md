<!-- generated-by: sqlbuild skills -->

# Overview

> Supported database engines and their connection configuration.

Online: https://sqlbuild.com/docs/concepts/adapters/

SQLBuild uses adapters to connect to different database engines. Snowflake is the main target.

- **Supported:** ready for production use.
- **Beta:** builds, tests and plans work, but with less production use so far. Expect rough edges, and please [report issues](https://github.com/sqlstack-io/sqlbuild/issues).

| Adapter | Status | Install |
|---------|--------|---------|
| [Snowflake](adapters/snowflake.md) | Supported | `sqlbuild[snowflake]` |
| [DuckDB](adapters/duckdb.md) | Supported | included by default |
| [MotherDuck](adapters/motherduck.md) | Supported | included by default (uses DuckDB) |
| [PostgreSQL](adapters/postgres.md) | Supported | `sqlbuild[postgres]` |
| [BigQuery](adapters/bigquery.md) | Beta | `sqlbuild[bigquery]` |
| [Databricks](adapters/databricks.md) | Beta | `sqlbuild[databricks]` |
| [SQL Server](adapters/sqlserver.md) | Beta | `sqlbuild[sqlserver]` |
| [SQLite](adapters/sqlite.md) | Beta | included by default |
| [ClickHouse](adapters/clickhouse.md) | Beta | `sqlbuild[clickhouse]` |
| Redshift | Planned | |
| Trino | Planned | |
| Spark | Planned | |
| Athena | Planned | |

Set the adapter in `sqlbuild_project.toml`:

```toml
name = "my_project"
adapter = "duckdb"
```

Or override it per developer in `sqlbuild_local.toml`:

```toml
adapter = "duckdb"
```

## Custom adapters

To extend a built-in adapter or add another database engine, see
[Custom adapters](adapters/custom.md).
