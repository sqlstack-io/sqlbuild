<!-- generated-by: sqlbuild skills -->

# Overview

> Supported database engines and their connection configuration.

Online: https://sqlbuild.com/docs/concepts/adapters/

## Contents

- Custom adapters

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

You can write your own adapter for any database engine by placing Python files under `adapters/` in your project directory.

### Extending a built-in adapter

The most common case is extending an existing adapter with custom behavior. Subclass the built-in adapter, set a new `adapter_name`, and override what you need:

```python
# adapters/duckdb_plus.py
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter

class DuckDbPlusAdapter(DuckDbAdapter):
    adapter_name = "duckdb_plus"

    def connect(self, config):
        connection = super().connect(config)
        # custom setup - load extensions, set pragmas, etc.
        self.execute(connection=connection, sql="SET enable_progress_bar = true")
        return connection
```

```toml
adapter = "duckdb_plus"
```

A subclass inherits the parent's capabilities. For example, an adapter extending the Snowflake
adapter keeps `supports_session_warehouse = True`, so it can use
[command-group warehouses](project-configuration.md#command-group-warehouses) and
`--warehouse`. An adapter written from scratch that sets this attribute receives the selected
warehouse as the connection's `warehouse` key.

### Writing an adapter from scratch

For a database engine with no built-in support, subclass `BaseAdapter`. It provides ANSI SQL defaults for most methods - you only need to implement `connect`, `execute`, `close`, and any methods where your engine differs from standard SQL:

```python
# adapters/my_database.py
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter

class MyDatabaseAdapter(BaseAdapter):
    adapter_name = "my_database"
    sql_analysis_dialect_name = "postgres"  # SQL dialect for validation and lineage

    def connect(self, config):
        ...

    def execute(self, *, connection, sql):
        ...

    def close(self, connection):
        ...
```

Set `sql_analysis_dialect_name` to the SQL dialect name that matches your engine's SQL syntax. This enables compile-time SQL validation, column inference, column lineage, and local scenario replay for your adapter. If omitted, SQLBuild uses generic SQL parsing. SQL analysis is powered by [Polyglot](https://github.com/tobilg/polyglot), a Rust reimplementation of SQLGlot supporting 32+ dialects.

For full control with no inherited defaults, subclass `StrictAdapter` instead. Every method is abstract and must be implemented explicitly. SQLBuild raises a clear error listing any unimplemented methods.

A few `StrictAdapter` methods are optional and have a documented default. `create_sql_test_output` and `drop_sql_test_output` let an adapter store a unit test's actual output once, in a temporary relation, so every comparison of that test reads it instead of recomputing the model chain. By default `create_sql_test_output` returns `None`, and SQLBuild runs the test's inline comparison query instead, so adapters that do not implement them keep working unchanged. `render_create_temporary_function` returns the call target and DDL for a session-scoped copy of a project function, which `sqb test` calls instead of the built function (see [Functions in `sqb test`](testing.md#functions-in-sqb-test)). By default it returns `None`, and tests call the built function. `render_drop_schema` returns the statements that drop a schema and everything in it, which [`sqb review`](../cli/review.md) uses to remove the schema it built into. By default it raises an error, and `sqb review` is unavailable for that adapter.

### Discovery rules

- SQLBuild discovers all `.py` files under `adapters/` recursively (excluding `__init__.py` and files starting with `_`)
- Each file is scanned for classes that define a string `adapter_name` and subclass `StrictAdapter` (or any of its subclasses like `BaseAdapter` or a built-in adapter)
- Adapter names must be unique across all adapter files - duplicates raise an error
- Custom adapter names cannot shadow built-in names (`duckdb`, `motherduck`, `snowflake`, `bigquery`, `databricks`, `postgres`, `sqlite`, `sqlserver`, `clickhouse`)

### Adapter class hierarchy

```
StrictAdapter          (fully abstract - all methods must be implemented)
  └── BaseAdapter      (ANSI SQL defaults - override only what differs)
        ├── DuckDbAdapter
        ├── MotherDuckAdapter
        ├── SnowflakeAdapter
        ├── BigQueryAdapter
        ├── DatabricksAdapter
        ├── PostgresAdapter
        ├── SqlServerAdapter
        └── ClickHouseAdapter
```

`StrictAdapter` composes four mixins that define the full adapter contract:

- **ConnectionMixin** - `connect`, `close`, `begin`, `commit`, `rollback`
- **SchemaMixin** - `relation_exists`, `list_relations`, `get_columns`, `ensure_schema`
- **MaterializationMixin** - `create_table_as`, `create_view_as`, `drop`, `rename`, `swap`, `load_seed`
- **DiffMixin** - `diff_schema`, `diff_rows`, `sample_unequal_rows`, `sample_side_only_rows`, `diff_unkeyed_rows`
