<!-- generated-by: sqlbuild skills -->

# SQL Server

> Microsoft SQL Server adapter configuration for SQLBuild.

Online: https://sqlbuild.com/docs/concepts/adapters/sqlserver/

The SQL Server adapter is in beta: builds, tests and plans work, but it has had less production use so far. Please [report issues](https://github.com/sqlstack-io/sqlbuild/issues).

SQL Server requires the optional `pymssql` dependency:

```bash
pip install 'sqlbuild[sqlserver]'
# or
uv pip install 'sqlbuild[sqlserver]'
```

## Connection config

```toml
adapter = "sqlserver"
default_target = "dev"

[connections.sqlserver]
host = "localhost"
port = 1433
user = "sa"
password = "my_password"
database = "my_database"

[targets.dev]
connection = "sqlserver"
database = "my_database"
schema = "analytics_dev"
```

| Field | Description |
|-------|-------------|
| `host` | SQL Server hostname (default: `localhost`). Also accepts `server` as an alias. |
| `port` | SQL Server port (default: `1433`) |
| `user` | Database user (default: `sa`). Also accepts `username` as an alias. |
| `password` | Database password |
| `database` | Database name (default: `master`). Also accepts `dbname` as an alias. |

Connection fields are passed to `pymssql.connect()`. See the [pymssql documentation](https://pymssql.readthedocs.io/en/stable/ref/pymssql.html) for all available options. The target remains authoritative for database and schema qualification.

SQL Server supports schema-only, full-row, and bounded `sqb diff` comparisons.

## Typed constants

SQL Server supports scalar and parenthesized `value_list` constants. Strings use Unicode literals where required, and objects render through a JSON expression such as:

```sql
JSON_QUERY(N'{"GB":"Great Britain"}')
```

SQL Server has no supported first-class native array expression. Any list or set that resolves to `render_as array`, whether through its declaration or `[constants].collection_rendering`, fails at compile time. SQLBuild does not silently substitute a value list or quoted JSON string.

See [Collections and Rendering](../constants/collections-and-rendering.md#native-array-rendering) for declarations, rendering precedence, and the cross-adapter matrix.

## Shared connections across targets

```toml
adapter = "sqlserver"

[connections.sqlserver]
host = "localhost"
user = "sa"
password = "${ENV:MSSQL_PASSWORD}"
database = "analytics"

[targets.prod]
connection = "sqlserver"
database = "analytics"
schema = "prod"

[targets.dev]
connection = "sqlserver"
database = "analytics"
schema = "dev"
```

## Functions in unit tests

SQL Server has no session-scoped functions, so `sqb test` calls the built function. Run `sqb build`
before testing a new or changed function. See [Functions in `sqb test`](../testing.md#functions-in-sqb-test).
