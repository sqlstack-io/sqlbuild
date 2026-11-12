<!-- generated-by: sqlbuild skills -->

# sqb diff

> Compare schemas and data between targets.

Online: https://sqlbuild.com/docs/cli/diff/

## Contents

- Usage
- Flags
- Size guard
- Examples
- Exit codes

Compares schemas and optionally row-level data between two targets (e.g. `prod:dev`), or between two read-only SQL queries. See [Data Diffs](../concepts/diff.md) for detailed usage.

## Usage

```bash
# Model mode
sqb diff <FROM>[:<TO>] [<mode>] --select <models> [flags]

# Query mode
sqb diff --left-query <sql> --right-query <sql> (--key <column>... | --unkeyed) [flags]
sqb diff --left-query <sql> --right-query <sql> --schema-only [flags]
```

In model mode, the first argument is a positional `FROM:TO` range, `--select` is required, and at most one mode can be given: `--full`, `--schema-only`, or `--bounded <duration>`. Without a mode, the diff is a full comparison guarded by table size (see [Size guard](#size-guard)).

`FROM` alone compares `FROM` with the active target, so with `dev` active, `sqb diff prod -s orders` runs as `sqb diff prod:dev -s orders`. This is the same default `sqb clone --from` uses. It fails when no target is active or `FROM` is the active target; pass `FROM:TO` in those cases.

`FROM` and `TO` are configured target names. Their database/schema namespaces remain
authoritative, while the `TO` target's named connection executes the complete comparison and must
be able to read both namespaces.

Full and bounded row comparisons match rows on the model's `unique_key`, or on `--key` columns when given; use `--unkeyed` for an exact full-row comparison. Bounded mode uses the model's cursor and falls back to a full row comparison when no cursor is configured.

In query mode, omit `FROM:TO` and model selectors; each query identifies its own input and the active target's connection (or `--target`) runs both. A full comparison is the default and needs `--key` or `--unkeyed`; `--schema-only` takes neither, and `--bounded` is model-only.

## Flags

| Flag | Description |
|------|-------------|
| `--full` | Compare both schema and all row data |
| `--schema-only` | Compare column names and types only |
| `--bounded` | Compare row data within a recent window (e.g. `14d`, `6h`) |
| `--verbose`, `-v` | Show more example rows (default: 3, verbose: 10) |
| `--max-column-examples` | Override maximum examples per changed column |
| `--max-row-only-examples` | Override maximum examples for side-only rows |
| `--sample-rows` | Override the deterministic unique-key sample size |
| `--sample-seed` | Override the deterministic sample seed |
| `--exhaustive` | Disable inherited sampling for this invocation |
| `--json-output` | Write structured comparison scope, coverage, and results to a JSON file |
| `--max-models` | Fail when the selected scope contains more models than this limit |
| `--max-columns` | Fail when either side of a model has more columns than this limit |
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias) |
| `--key` | Row key columns, for example `--key order_id line_number`. Repeating the flag also works. Overrides the model's `unique_key` |
| `--unkeyed` | Compare exact full-row multisets when there is no stable row key |
| `--exclude-column` | Leave columns out of the comparison; takes several values, and repeating the flag also works |
| `--tolerance` | Numeric tolerance such as `amount:absolute=1` or `rate:relative=0.001`; repeatable |
| `--left-query`, `--right-query` | Compare two read-only SQL queries instead of models |
| `--left-query-file`, `--right-query-file` | Read each query from a UTF-8 file |
| `--left-label`, `--right-label` | Names for the two query sides in the output |
| `--target` | Target whose connection runs both queries |
| `--max-value-length` | Maximum characters shown per example value (default 160) |
| `--no-example-values` | Keep keys and counts but hide example values |
| `--full-example-values` | Show complete example values |
| `--json` | Print one JSON document to stdout |
| `--select`, `-s` | Select specific models to diff (required in model mode) |
| `--exclude` | Exclude specific models from diffing |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Size guard

Without `--full`, `--bounded` or `--schema-only`, model mode first checks table sizes. Before it reads
any table data, SQLBuild reads each selected table's row count from warehouse metadata on both
sides and compares it with that target's
[`max_full_rows`](../concepts/project-configuration.md#diff-limits) limit. The limit is 10,000,000
rows unless configured, and `"unlimited"` turns the check off for that target. If every table is
within its limits, the full comparison runs as usual.

Otherwise, the diff stops before reading any rows and exits with `2`. The error lists each blocked model
with its row count and limit on both sides, then gives the commands to run instead:

```text
error[C270]: full diff stopped before reading data: 1 of 1 selected models exceed the full-comparison row limit or have an unknown size
  orders: prod 12,400,000 rows, limit 10,000,000; dev 12,400,000 rows, limit 10,000,000
Run one of these instead:
  sqb diff prod:dev --full --select orders
  sqb diff prod:dev --bounded <window> --select orders
  sqb diff prod:dev --schema-only --select orders
```

The `--bounded` command appears only when every blocked model has a cursor. With `--json` or
`--json-output`, the error document has `status` `incomplete` and a `size_guard` object holding the
same models, sizes, limits and commands.

A size counts as over the limit when the warehouse cannot report it, for example for views or for
tables without statistics. Sizes come from metadata only:

| Adapter | Source |
|---------|--------|
| DuckDB, MotherDuck | `duckdb_tables()` `estimated_size` |
| PostgreSQL | `pg_class.reltuples`; unknown until the table has been analyzed |
| Snowflake | `INFORMATION_SCHEMA.TABLES` `ROW_COUNT` |
| BigQuery | Tables API `numRows` |
| Databricks | Statistics from `DESCRIBE TABLE EXTENDED`; unknown until `ANALYZE TABLE ... COMPUTE STATISTICS` has run |
| SQL Server | `sys.partitions` row counts |
| Custom adapters | Unknown unless the adapter implements `estimate_relation_row_count` |

Metadata counts can be estimates. The guard only decides whether to start a full comparison; the
comparison itself always counts rows exactly.

## Examples

```bash
# Full diff against the active target, if the tables are within the size limit
sqb diff prod --select customer_status_snapshot

# Full diff of a specific model
sqb diff prod:dev --full --select customer_status_snapshot

# Schema-only diff of all marts
sqb diff prod:dev --schema-only --select path:models/marts

# Bounded diff of last 14 days
sqb diff prod:dev --bounded 14d --select hourly_order_activity

# Deterministic bounded sample
sqb diff prod:dev --bounded 14d --sample-rows 50000 --sample-seed 7 --select order_lines

# Force exhaustive comparison despite inherited sampling defaults
sqb diff prod:dev --bounded 14d --exhaustive --select order_lines

# Compare two queries on a key
sqb diff --left-query-file queries/orders_before.sql --right-query-file queries/orders_after.sql --key order_id

# Composite key and several excluded columns
sqb diff prod:dev --full --select order_lines --key order_id line_number --exclude-column updated_at loaded_at
```

## Exit codes

Returns `0` when all selected models have no differences, `1` when any model has schema or row differences, and `2` when the size guard stopped a default full diff before reading data. Query mode also returns `2` when a safety, schema, or key check prevented a complete comparison, and `3` when setup, execution, or cleanup failed; see [Query output and exit codes](../concepts/diff.md#query-output-and-exit-codes).
