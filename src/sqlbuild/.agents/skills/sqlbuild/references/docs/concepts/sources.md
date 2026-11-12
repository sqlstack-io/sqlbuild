<!-- generated-by: sqlbuild skills -->

# Sources

> Declare external data inputs for your pipeline.

Online: https://sqlbuild.com/docs/concepts/sources/

## Contents

- Table sources
- Expression sources
- Source audits
- Type enforcement
- Managed sources (loaders)
- Source freshness
- Config reference
- YAML that SQLBuild reads

Sources declare external data that your models depend on. They are defined in YAML files under `sources/` in your project directory.

## Table sources

Point at an existing table or view in your warehouse:

```yaml
sources:
  - name: raw_events
    description: Events
    database: analytics
    schema: raw
    table: events
```

Reference it in models with `__source("raw_events")`.

## Expression sources

Define source data inline as a SQL expression. No external tables or setup scripts needed:

```yaml
sources:
  - name: raw__customers
    description: Customers
    expression: |
      SELECT * FROM (VALUES
        (1, 'Leslie', 'Knope', 'leslie@pawnee.gov', TIMESTAMP '2026-01-15 09:00:00'),
        (2, 'Ron', 'Swanson', 'ron@pawnee.gov', TIMESTAMP '2026-02-01 08:00:00')
      ) AS raw__customers(id, first_name, last_name, email, created_at)
```

Expression sources are resolved at compile time. They're the escape hatch for anything the framework doesn't natively model: external tables, warehouse-specific syntax, function calls, or any other relation type that doesn't fit a standard table reference.

An inline source expression uses macros, constants, and enums available from its source definition
under `sources/`. Declarations limited to one source folder are not available in sibling folders.
See [How Visibility Works](declaration-scopes/visibility.md).

## Source audits

Sources support the same audit system as models. Audits attached to sources run before any dependent model is built:

```yaml
sources:
  - name: raw_orders
    description: Orders
    columns:
      - name: id
        audits:
          - not_null
          - unique
    audits:
      - expression_is_true:
          name: no future orders
          expression: "ordered_at <= CURRENT_TIMESTAMP"
```

If a source audit with `error` severity fails, all downstream models that depend on that source are blocked.

A source's audits run before the first model in the build that reads it. A source no selected model
reads is audited at the end of the build only when loading is off (`--no-load` or
`auto_load_sources = false`) or when the source is selected by name, such as
`sqb build --select raw_orders`; otherwise its audits do not run.

A source audit may also read other resources; SQLBuild builds them first. See
[Audits that read other resources](audits.md#audits-that-read-other-resources).

## Type enforcement

Type enforcement is implicit for sources. If any column declares a `type`, SQLBuild activates source type enforcement, casts that column during source resolution, and uses declared types for schema-change detection:

```yaml
sources:
  - name: raw__customers
    description: Customers
    expression: |
      SELECT 1 AS id, 'Leslie' AS first_name, 'Knope' AS last_name
    columns:
      - name: id
        type: INTEGER
      - name: first_name
      - name: last_name
```

In this example, only `id` has a type declared, so only `id` is cast. Columns without a `type` are passed through unchanged.

For expression sources, SQLBuild probes the expression's output columns and builds a projection that casts typed columns while preserving the rest. For table sources, it uses warehouse metadata to validate that declared column names exist and applies casts accordingly.

You can explicitly set `type_enforcement: false` on a source to disable casting even when column types are declared.

## Managed sources (loaders)

Sources can be loaded by Python functions instead of pointing at existing tables or inline expressions. Set `managed: true` to bind a source to the `@loader` function **of the same name**, and SQLBuild will call it to populate the source table:

```yaml
sources:
  - name: raw_customers
    description: Customers
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
      - name: name
        type: VARCHAR
```

Managed sources support incremental write strategies (`table`, `append`, `delete_insert`, `merge`), cursor-based loading, and concurrent execution.

See [Loaders](python-nodes/loaders.md) for the full guide on writing loader functions, write strategies, the loader context API, and auto-load behavior during builds.

### Declarative integrations (no Python)

For common ingestion you can declare the source entirely in YAML, with no `@loader` function. SQLBuild generates the loader for you and runs it during the build:

- [dlt](../integrations/dlt.md) - declare `dlt_sources` for `rest_api`, `sql_database`, and `filesystem` sources.
- [ingestr](../integrations/ingestr.md) - add an `ingestr` block to a source to pull from 50+ sources.

## Source freshness

Source freshness lets SQLBuild observe whether a source's data changed between runs. This feeds into
[planning and change detection](planning.md): changed observations propagate to downstream
models and can alter their planned action.

Configure freshness per source with a `freshness:` block:

```yaml
sources:
  - name: raw_events
    description: Events
    schema: raw
    table: events
    freshness:
      strategy: column
      column: updated_at
      type: timestamp
      lag_tolerance: 15m
```

When the column is declared with a type under `columns:`, `strategy` and `type` can be left out:

```yaml
sources:
  - name: raw_events
    schema: raw
    table: events
    columns:
      - name: updated_at
        type: timestamp
    freshness:
      column: updated_at   # strategy column, type timestamp
      lag_tolerance: 15m
```

### Strategies

| Strategy | Description | Required fields |
|----------|-------------|-----------------|
| `adapter` | Uses warehouse metadata (e.g. Snowflake `LAST_ALTERED`). No query against the source table. | None |
| `column` | Reads `MAX(column)` from the source table. | `column`, `type` (inferred from a typed column declaration) |
| `sql` | Runs a custom query that returns a single scalar value. | `query`, `type` |

`strategy` is optional. Without it, SQLBuild infers the strategy from the keys you set: `column`
means `column`, `query` means `sql`, and neither means `adapter`, so `freshness: {}` observes
warehouse metadata. Setting both `column` and `query` without a `strategy` is an error, as is
setting `type` or `filter` without either. An explicit `strategy` keeps working and must agree
with the keys present.

#### adapter

```yaml
freshness:
  strategy: adapter
```

Uses adapter-level table metadata. Supported on Snowflake, BigQuery, Databricks, PostgreSQL, DuckDB, and SQL Server. Does not support `type`, `column`, or `query`.

#### column

```yaml
freshness:
  strategy: column
  column: updated_at
  type: timestamp
```

Queries `MAX(column)` from the source table. The column must be a plain column name (no expressions; use `sql` strategy for those). Requires `type`, unless the source declares the column with a type that SQLBuild can map (see [Type](#type)).

#### sql

```yaml
freshness:
  strategy: sql
  query: "SELECT MAX(version_id) FROM raw.events"
  type: integer
```

Runs an arbitrary SQL query that returns a single scalar. Requires `type`. Does not support `column`.

### Type

The `type` field declares the value kind for comparison:

| Type | Description |
|------|-------------|
| `timestamp` | Datetime value. Supports `lag_tolerance`. |
| `integer` | Integer value. Change detected by exact comparison. |
| `string` | String value. Change detected by exact comparison. |

For the `column` strategy, SQLBuild takes `type` from the column's declared type in the source's
`columns:` when it can map it:

| Declared column type | Freshness `type` |
|----------------------|------------------|
| `TIMESTAMP`, `TIMESTAMP_NTZ`, `TIMESTAMP_LTZ`, `TIMESTAMP_TZ`, `TIMESTAMPTZ`, `TIMESTAMP WITH TIME ZONE`, `DATETIME`, `DATETIME2`, `DATETIMEOFFSET`, `SMALLDATETIME` | `timestamp` |
| `INTEGER`, `INT`, `BIGINT`, `SMALLINT`, `TINYINT`, `INT2`, `INT4`, `INT8`, `INT64` | `integer` |
| `VARCHAR`, `STRING`, `TEXT`, `CHAR`, `CHARACTER`, `CHARACTER VARYING`, `NCHAR`, `NVARCHAR` | `string` |

Matching ignores case and length or precision arguments. Any other declared type, such as
`DATE` or `NUMBER(38,0)`, needs an explicit `type`; a column declared with an unmapped type and no
`type` is an error. An explicit `type` that contradicts a mapped declared type is also an error.

### Lag tolerance

`lag_tolerance` is optional and only valid with `type: timestamp`. It declares how much the observed timestamp can drift from the previous observation before being treated as a change:

```yaml
freshness:
  strategy: column
  column: updated_at
  type: timestamp
  lag_tolerance: 2h
```

Accepts positive durations: `15m` (minutes), `2h` (hours), `1d` (days). If the current observation is within the tolerance of the previous one, the source is treated as unchanged.

### Auto-observation

A table source without a `freshness:` block is observed with the `adapter` strategy when the adapter
supports it, unless it is managed. See [Source freshness](planning/source-freshness.md)
for how observations feed into plans, and [`sqb freshness`](../cli/freshness.md) to observe sources
without building.

### Freshness config reference

| Field | Description |
|-------|-------------|
| `strategy` | Observation strategy: `adapter`, `column`, or `sql` |
| `type` | Value kind: `timestamp`, `integer`, or `string` |
| `column` | Column name for `column` strategy |
| `query` | SQL query for `sql` strategy |
| `lag_tolerance` | Duration tolerance for timestamp comparisons (e.g. `15m`, `2h`, `1d`). Only valid with `type: timestamp`. |
| `filter` | Row filter for the `column` strategy |
| `age_policy` | `warn_after` and `error_after` durations |

## Config reference

| Field | Description |
|-------|-------------|
| `name` | Source name, used in `__source("name")` references |
| `database` | Target database (optional) |
| `schema` | Physical source schema (optional). For managed sources, overrides target `loader_schema`; otherwise managed loaders fall back to `loader_schema`, then target `schema`. |
| `table` | Target table name (defaults to `name` if omitted) |
| `expression` | Inline SQL expression (alternative to table reference) |
| `final` | ClickHouse only. Set to `true` to read the source with `FINAL`, so a `ReplacingMergeTree` table returns one row per key (see [ClickHouse](adapters/clickhouse.md#reading-replacingmergetree-sources)). Other adapters reject it with `A010`. |
| `managed` | Set to `true` to bind the source to the `@loader` function of the same name (see [Loaders](python-nodes/loaders.md)) |
| `write_strategy` | How the loader writes data: `table`, `append`, `delete_insert`, or `merge` (requires `managed: true`) |
| `cursor_column` | Column for incremental cursor tracking (required for `delete_insert` and `merge`) |
| `unique_key` | Merge key column(s) (required for `merge`) |
| `freshness` | Source freshness observation config (see [Source freshness](#source-freshness)) |
| `description` | Required human-readable description ([Descriptions](models/configuration.md#descriptions)) |
| `type_enforcement` | Override implicit type enforcement (`true`/`false`). Defaults to `true` when any column declares a type. |
| `contract` | `enforced` or `none`. When enforced, downstream models validate configured column references against source columns. |
| `columns` | Column declarations with `name`, `type`, `nullable`, `description`, `meta`, and `audits` |
| `audits` | Source-level audits |
| `meta` | Free-form metadata mapping |
| `load_batch_size` | Positive row batch size for managed loaders |
| `ingestr` | Declarative ingestr loader config (see [ingestr](../integrations/ingestr.md)) |

A sources file contains only `sources` and `dlt_sources`. Unknown keys in a sources file, a source,
a source column, `freshness`, `freshness.age_policy`, or `ingestr` fail discovery with `D006`,
naming the key and suggesting the nearest supported key.

## YAML that SQLBuild reads

Sources files, seed declarations and `schema.yml` files are YAML 1.2, read with the core schema,
plus merge keys (`<<`). Files must be UTF-8 and hold one document.

Plain values are typed by the core schema:

- `true` and `false` (also capitalized or uppercase) are booleans; `yes`, `no`, `on` and `off` are
  text
- `null`, `~` and an empty value are null
- decimal integers, `0o` octal and `0x` hexadecimal are integers; `017` is 17
- decimals with an optional exponent, `.inf` and `.nan` are floats
- everything else, including dates and timestamps, is text

Quote a value to keep it as text, for example `'2024-01-31'` or `'on'`.

Invalid YAML fails discovery with `D006` (sources) or `D005` (seed declarations and `schema.yml`),
naming the file, line and column. The following also fail with the same codes and a hint to
rewrite the value in plain YAML:

- tags other than the core schema's, such as `!!set` or `!custom`
- recursive aliases, and aliases that expand beyond SQLBuild's size limit
- nesting deeper than 256 levels, and integers longer than 4300 digits
