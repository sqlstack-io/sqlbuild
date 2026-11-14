<!-- generated-by: sqlbuild skills -->

# Snowflake

> Snowflake adapter configuration for SQLBuild.

Online: https://sqlbuild.com/docs/concepts/adapters/snowflake/

## Contents

- Connection config
- Session initialization
- Command-group warehouses
- Case-sensitive column names
- Metadata reads
- State tables
- Shared connections across targets
- Cost estimates
- Functions in unit tests

Snowflake requires the optional `snowflake-connector-python` dependency:

```bash
pip install 'sqlbuild[snowflake]'
# or
uv pip install 'sqlbuild[snowflake]'
```

## Connection config

```toml
adapter = "snowflake"
default_target = "dev"

[connections.warehouse]
account = "my_org-my_account"
user = "my_user"
password = "my_password"
role = "TRANSFORM_ROLE"
warehouse = "TRANSFORM_WH"

[targets.dev]
connection = "warehouse"
database = "ANALYTICS"
schema = "RAW"
```

Connection fields are passed directly to `snowflake.connector.connect()`. See the [Snowflake Connector documentation](https://docs.snowflake.com/en/developer-guide/python-connector/python-connector-connect) for all available options, including key-pair authentication, OAuth, and SSO. Put the authoritative database and schema on the target.

SQLBuild turns off the connector's platform detection by setting
`SNOWFLAKE_DISABLE_PLATFORM_DETECTION=true` when the variable is unset. Detection runs on every
connect: it probes cloud metadata endpoints, calls AWS to identify the caller for client
telemetry, and can print `botocore` warnings when an AWS SSO session has expired. To keep it on,
set the variable yourself before running `sqb`, for example `SNOWFLAKE_DISABLE_PLATFORM_DETECTION=false`.

## Session initialization

On connect, SQLBuild applies the connection's role and warehouse, then applies the active
target's authoritative database and schema. These ensure the session context is set correctly
regardless of the user's default settings.

## Command-group warehouses

Builds and ad-hoc queries usually need different compute. A target can set a warehouse for each
group of commands, so `sqb build` runs on a large warehouse while `sqb query` and `sqb diff` use a
small one:

```toml
[connections.warehouse]
account = "my_org-my_account"
warehouse = "ANALYTICS_WH"            # fallback for every command

[targets.dev]
connection = "warehouse"
database = "ANALYTICS"
schema = "DEV"

[targets.dev.warehouses]
build = "BUILD_WH"
query = "ADHOC_WH"
```

| Group | Commands |
|-------|----------|
| `build` | `audit`, `build`, `check`, `clone`, `dbt` (SQLBuild's own work), `load`, `plan`, `seed` |
| `query` | `contract`, `cost`, `debug`, `diff`, `freshness`, `janitor`, `query`, `scenario`, `test` |

Each command uses the first of these that is set:

1. `--warehouse <name>`, for that run only
2. The target's warehouse for the command's group
3. The connection's `warehouse`
4. The Snowflake user's default warehouse

Every connection a command opens uses the same warehouse. Tests and audits that run inside
`sqb build` use the `build` warehouse. Under `sqb dbt`, dbt's own nodes use the warehouse in their
dbt profile.

A group warehouse wins over a connection `warehouse` from either file. To change one for yourself,
set the group in `sqlbuild_local.toml`; the other group keeps its project value:

```toml
# sqlbuild_local.toml
[targets.dev.warehouses]
query = "ALICE_XS_WH"
```

Values accept `${ENV:...}` and `${my_var}` templates and must be Snowflake identifiers, unquoted
(`ADHOC_WH`) or double-quoted (`"Adhoc WH"`). `sqb debug` shows each group's warehouse and where it
came from.

Groups need the Snowflake adapter or a custom adapter built on it; on other adapters they are an
error. If you switch `adapter` locally, for example to DuckDB, the shared groups are ignored and
`sqb debug` reports them as `not used`.

## Case-sensitive column names

Snowflake stores an unquoted column name in upper case, so `order_id` and `ORDER_ID` name the
same column. A column created with a quoted name in mixed or lower case, such as `"orderId"`,
keeps its exact case, and only the quoted name reads it.

Model settings that name such a column (`unique_key`, `updated_at`, `observed_at`,
`check_columns`, `cursor` and `merge_exclude_columns`) take the quoted name inside a string:

```sql
MODEL (
  description 'Order history.',
  materialized snapshot,
  unique_key ['"orderId"'],
  snapshot_strategy timestamp,
  updated_at '"updatedAt"',
);
```

A snapshot or microbatch build stops when an unquoted setting names a case-sensitive column, with
an error such as
`unique_key 'orderId' names the case-sensitive column "orderId"; write it with its quotes, as '"orderId"'`.

## Metadata reads

Planning and builds read table, view and column metadata with `SHOW` commands, which don't need a
running warehouse. Source freshness, which reads `LAST_ALTERED` from `INFORMATION_SCHEMA`, and
columns of a schema with 10,000 or more columns need one. Probes for sources the listing didn't
find may need one too. Run `sqb plan -v` to see each metadata query with its time
and row count.

## State tables

SQLBuild records its state in `_sqlbuild_*` tables. On Snowflake it creates them as permanent tables
with 90 days of time travel, or 1 day on editions that allow only that. It never alters an existing
state table. Models are not affected: their table type follows the model's `table_type` setting.

## Shared connections across targets

Use multiple targets to reuse one Snowflake connection while selecting different authoritative
databases and schemas:

```toml
adapter = "snowflake"

[connections.warehouse]
account = "my_org-my_account"
user = "my_user"
password = "my_password"
role = "TRANSFORM_ROLE"
warehouse = "TRANSFORM_WH"

[targets.prod]
connection = "warehouse"
database = "PROD_DB"
schema = "prod"

[targets.dev]
connection = "warehouse"
database = "DEV_DB"
schema = "dev"
```

## Cost estimates

Snowflake builds show a compact per-run busy-compute estimate. SQLBuild attributes visible
overlapping query intervals fairly across active queries, converts attributed seconds using the
warehouse-size credit rate, and estimates USD from the configured rate:

```toml
[cost]
usd_per_credit = 3.00
```

The default is `3.00` USD per credit and is visibly marked as a default. Set it to your Snowflake
contract rate. Use `sqb cost`, `sqb cost latest`, `sqb cost <run_id>`, or
`sqb cost history --since 7d` to inspect persisted records. `--json` and `--json-output PATH`
provide a versioned, decimal-safe output contract. Pending detail records are refreshed from
Snowflake when inspected again.

Under [`sqb server`](../../cli/server.md), the UI labels the same figures "estimated query cost":

| Page | Shows |
| --- | --- |
| Run | The run's total, then each model and each warehouse, most expensive first |
| Model, Runs tab | The model's cost and query time in each build over the last 30 days |
| Overview | The last 7 or 30 days by job, folder and warehouse size |

Figures are in dollars when `usd_per_credit` is set, and in credits with the default rate. A run
whose cost isn't final shows "Cost pending" or "Partial", never $0.

The estimate is read from `INFORMATION_SCHEMA.QUERY_HISTORY` when the run finishes. If it can't be
read within 30 seconds, the build is unaffected and its cost is recorded as `pending`;
`sqb cost <run_id>` collects it later. Any other failure is recorded as `collection_failed` with the
Snowflake error. Each later build in the target also re-reads up to three pending, partial or failed
runs from the last six days while its warehouse is still running, so the server's figures complete
without resuming a suspended warehouse. A failed collection is tried at most three times in all.

The estimate is attributed compute, not your Snowflake bill. It sees only query history visible to
the executing role, and leaves out warehouse resume and idle time, the 60-second minimum,
cloud-services credits, contract adjustments and multi-cluster billing.

A statement run with your own `QUERY_TAG` may report missing warehouse metadata. Executed SQL is
kept under `target/run/`, which can hold sensitive data; the cost records under
`target/executions/` store only a digest of each statement.

## Functions in unit tests

`sqb test` creates each UDF and table function the selected tests call, SQL or Python, as a
`TEMPORARY FUNCTION __SQB_TEST__<NAME>` in the session's current schema. The session needs a current
schema and permission to create functions there; otherwise tests call the built function. See
[Functions in `sqb test`](../testing-reference.md#functions-in-sqb-test).
