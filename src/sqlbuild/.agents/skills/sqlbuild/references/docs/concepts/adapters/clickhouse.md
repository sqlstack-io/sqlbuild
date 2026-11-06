<!-- generated-by: sqlbuild skills -->

# ClickHouse

> ClickHouse adapter configuration for SQLBuild.

Online: https://sqlbuild.com/docs/concepts/adapters/clickhouse/

## Contents

- Connection config
- Schemas are databases
- How SQLBuild writes
- Physical layout
- Reading ReplacingMergeTree sources
- Functions
- Clones
- Nullable columns
- Settings SQLBuild pins
- SQL differences to watch
- Limits
- Recall store

The ClickHouse adapter is in beta and supports a single server. Clusters (`ON CLUSTER`, replicated and distributed tables) are not supported yet. Please [report issues](https://github.com/sqlstack-io/sqlbuild/issues).

ClickHouse requires the optional `clickhouse-connect` dependency:

```bash
pip install 'sqlbuild[clickhouse]'
# or
uv pip install 'sqlbuild[clickhouse]'
```

SQLBuild talks to ClickHouse over HTTP or HTTPS. The oldest supported server is ClickHouse 25.8; older servers fail to connect with `A703`.

## Connection config

```toml
adapter = "clickhouse"
default_target = "dev"

[connections.clickhouse]
host = "localhost"
port = 8123
user = "default"
password = "${ENV:CLICKHOUSE_PASSWORD}"

[targets.dev]
connection = "clickhouse"
schema = "analytics_dev"
```

| Field | Description |
|-------|-------------|
| `host` | Server hostname (default: `localhost`) |
| `port` | HTTP port (default: `8123`, or `8443` with `secure = true`) |
| `user` | User name (default: `default`). Also accepts `username`. |
| `password` | Password |
| `database` | Database for unqualified names (default: `default`) |
| `secure` | Use HTTPS (default: `false`) |
| `verify` | Verify the server certificate over HTTPS (default: `true`) |
| `settings` | Extra ClickHouse settings applied to every query, as a table |
| `connect_timeout`, `send_receive_timeout` | Client timeouts in seconds |

Any other key fails with `A702`.

## Schemas are databases

ClickHouse has no level between a database and a table, so a SQLBuild **schema** is a ClickHouse **database**. Set `schema` on targets, models and sources; setting `database` fails with `A704`. SQLBuild creates model schemas with `CREATE DATABASE IF NOT EXISTS`.

Identifiers are case-sensitive in ClickHouse, quoted or not.

## How SQLBuild writes

SQLBuild never relies on background merges for correctness, and it never updates or deletes rows in place.

- **Tables** build in a staging table and replace the live table with `EXCHANGE TABLES`, which is atomic. This needs databases with the `Atomic` engine, the default; other engines fail with `A705`.
- **Views** use `CREATE OR REPLACE VIEW`.
- **Incremental `append`** inserts the new rows.
- **Incremental `merge`, `delete_insert` and cursor windows** rewrite whole partitions. SQLBuild stages the incoming rows and finds the partitions they touch and the partitions holding the rows they replace. One partition is rebuilt in a staging table and swapped in with `ALTER TABLE ... REPLACE PARTITION`. Several partitions are published together: SQLBuild copies the table by hard-linking its parts (no data is rewritten), rebuilds the touched partitions in the copy and swaps the copy in with one `EXCHANGE TABLES`. Either way the swap is atomic, so a build interrupted at any point leaves the table as it was before the run or with every partition of the run, never part of them, and the next run's cursor window cannot skip rows.
- **Microbatches** use the same partition rewrite for each batch, so each batch is published atomically. Concurrent batches (`batch_concurrency`) swap different single partitions in parallel; batches of one `sqb` process that share a partition, or that publish several partitions with an exchange, take turns.

### One writer per model

A partition rewrite reads the table, rebuilds partitions and swaps them in. ClickHouse has no transaction that spans those steps, and SQLBuild only serializes rewrites of one table inside one `sqb` process. If two `sqb` processes write the same model at the same time, the later swap wins and rows the other process wrote can be lost.

Run each model from one process at a time: one scheduled job per model or per project, with no overlapping runs. Concurrency within a run (`--concurrency`, `batch_concurrency`) is safe.
- **Snapshots** apply their changes to a private copy of the snapshot table and exchange it in. Closing a version rebuilds the copy rather than updating rows, so the live snapshot changes only at the final exchange.

The cost of a rewrite is proportional to the partitions it touches. Without a `partition_by`, a table is one partition, so every incremental run rewrites the whole table; partition large incremental models by the column their new rows arrive on.

`merge` matches rows on `unique_key` exactly as other adapters do: incoming rows replace matched rows, `merge_exclude_columns` keep their stored values, and unmatched rows are inserted.

## Physical layout

Table and incremental models set their table engine, sorting key, partitioning, primary key and table settings in the MODEL header:

```sql
MODEL (
  description 'One row per order event.',
  materialized incremental,
  incremental_strategy delete_insert,
  unique_key [event_id],
  cursor event_time,
  cursor_type timestamp,
  cursor_inputs (raw_events event_time),
  engine 'ReplacingMergeTree(version)',
  order_by [event_id, event_time],
  partition_by 'toYYYYMM(event_time)',
  primary_key [event_id],
  table_settings (index_granularity 4096),
);
```

| Field | Value | Default |
|-------|-------|---------|
| `engine` | A non-replicated MergeTree-family engine, with its arguments | `MergeTree` |
| `order_by` | A sorting expression, or a list of them | `tuple()` (unsorted) |
| `partition_by` | A partitioning expression | No partitioning |
| `primary_key` | A prefix of `order_by` | The sorting key |
| `table_settings` | Setting names mapped to strings, integers or booleans | None |

Quote expressions that are not plain column names. Compile rejects malformed values, `Replicated*` engines, and these fields on views and snapshots; other adapters reject them with `A011` when planning.

ClickHouse Cloud's `Shared*` engines (`SharedMergeTree`, `SharedReplacingMergeTree` and the other `SharedMergeTree` variants) are accepted and handled like their `MergeTree` counterparts. Support is best effort: SQLBuild is not tested on ClickHouse Cloud. ClickHouse Cloud already creates a `MergeTree` table as `SharedMergeTree`, so naming the plain engine works there too.

The fields are part of the model's version, so `sqb plan` reports a changed layout as a config change. A table model is rebuilt with the new layout. An incremental model keeps writing into its existing table with the old layout, and `sqb plan` warns with `PHYSICAL_LAYOUT_PENDING` until a full refresh (`sqb build <model> --full-refresh`, with `--start-cursor-ts` and `--end-cursor-ts` for a cursor model) rebuilds the table with the new layout.

ClickHouse rejects nullable sorting and partitioning keys by default. Wrap the column in `assumeNotNull()` or `coalesce()` in the expression, or set `table_settings (allow_nullable_key true)`.

## Reading ReplacingMergeTree sources

A `ReplacingMergeTree` table can hold several versions of a row until the server merges them in the background. Set `final true` on the source so every read SQLBuild generates for it uses `FINAL` and sees one version per key:

```yaml
sources:
  - name: customers
    description: Customer master data.
    schema: raw
    table: customers
    final: true
```

## Functions

SQL functions deploy as ClickHouse SQL user-defined functions (`CREATE FUNCTION name AS (args) -> expression`). ClickHouse function names are global to the server, not scoped to a database, so SQLBuild names each function `<schema>__<function>`. A function `half_amount` in schema `analytics` is created as `analytics__half_amount`, and `__udf("half_amount")` renders that name.

SQLBuild does not deploy Python functions (`A707`) or SQL table functions (`A708`) to ClickHouse: executable functions need server configuration files. Install them on the server and call them directly.

## Clones

`sqb clone` creates the clone with `CREATE TABLE ... AS` and attaches every part of the origin with `ALTER TABLE ... ATTACH PARTITION ALL FROM`, which hard-links the data instead of copying it. The clone keeps the origin's engine, keys and settings and stays valid after the origin is dropped.

## Nullable columns

ClickHouse columns hold `NULL` only when their type is `Nullable`, and a table built from a query takes each column's nullability from the query. When an incremental model later produces `NULL` in a column its table created as non-nullable, what happens follows `on_schema_change`:

- `append_new_columns` (the default) and `sync_all_columns`: SQLBuild changes the column to `Nullable` with `ALTER TABLE ... MODIFY COLUMN` before writing, so the `NULL` is stored as `NULL`. A column in the sorting, partitioning or primary key cannot become `Nullable` unless the table sets `allow_nullable_key`, so that run fails with `A712`; keep the key non-null with `coalesce()` or `assumeNotNull()`, or set `table_settings (allow_nullable_key true)` and rebuild with `--full-refresh`.
- `fail`: the run fails as a schema change and the table is untouched.
- `ignore`: the table is not changed, and writing a `NULL` into the non-nullable column fails with `A713` instead of storing `0`, `''` or the epoch.

## Settings SQLBuild pins

Every query runs with these settings so SQL behaves the same as on other adapters:

| Setting | Value | Why |
|---------|-------|-----|
| `join_use_nulls` | `1` | Outer joins return `NULL` for missing rows |
| `cast_keep_nullable` | `1` | `CAST` keeps `NULL` values |
| `union_default_mode`, `except_default_mode`, `intersect_default_mode` | `DISTINCT` | Bare `UNION`, `EXCEPT` and `INTERSECT` follow standard SQL |
| `default_table_engine` | `MergeTree` | Tables created without an engine clause are MergeTree tables |
| `mutations_sync`, `lightweight_deletes_sync` | `2` | Statements finish before the next one starts |
| `date_time_input_format`, `cast_string_to_date_time_mode` | `best_effort` | ISO 8601 timestamps with `T` and offsets parse |
| `insert_null_as_default` | `0` | Inserting `NULL` into a column that cannot hold it fails instead of storing `0`, `''` or the epoch |

The connection's `settings` may repeat these values but not change them: a different value fails to connect with `A710`, because SQLBuild's generated SQL depends on them.

A [Recall](../observability/recall.md) `read_connection` whose user is `readonly = 1`, and so may not change settings, sends none of them: its `sqb recall query` SQL runs with the server's own settings. Every other reader gets them.

ClickHouse returns a type's default value (`0`, the epoch, an empty string) from `lag` and `lead` past the edge of a window frame unless the argument is `Nullable`. SQLBuild's generated snapshot SQL wraps those arguments in `toNullable` so they return `NULL` as on other adapters. Write the same in your own models when you rely on `NULL` there.

## SQL differences to watch

- In a query that joins three or more tables, ClickHouse names a selected column such as `c.customer_id` literally `c.customer_id` when another joined table also has a `customer_id`. Alias such columns (`c.customer_id AS customer_id`) so the table gets the column name other warehouses give it. When SQL analysis knows the model's output columns, a build that produces such a name fails with `A711` naming the column and the alias to add.
- `CURRENT_TIMESTAMP` needs parentheses: write `CURRENT_TIMESTAMP()` or `now()`.

## Limits

- Single server only. `ON CLUSTER`, `ReplicatedMergeTree` and `Distributed` tables are not managed. ClickHouse Cloud and its `Shared*` engines are best effort and untested.
- No transactions. Each step above is designed so a rerun converges instead.
- Concurrent `sqb` processes writing the same model can lose rows; see [One writer per model](#one-writer-per-model).
- Snapshots always use an unsorted `MergeTree` table.
- Seeds load through the HTTP insert interface as CSV, so large seeds never hit the `max_query_size` limit.
- Retention and time travel are not available.

## Recall store

A ClickHouse connection can also hold [Recall](../observability/recall.md)'s command
history for a whole team, whichever adapter the project builds with. Set
`[recall] write_connection` and `read_connection` to it, one connection or separate writer and
reader users. Recall creates its database, tables and views on first use, keeps every output byte by default, and needs
only `SELECT` and `INSERT` once the tables exist; readers need only `SELECT` and may be
`readonly = 1` users. See
[Sharing history in ClickHouse](../observability/recall.md#sharing-history-in-clickhouse).
