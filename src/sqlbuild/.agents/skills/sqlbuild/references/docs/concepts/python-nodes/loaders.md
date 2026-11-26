<!-- generated-by: sqlbuild skills -->

# Loaders

> Load external data into source tables with Python functions.

Online: https://sqlbuild.com/docs/concepts/python-nodes/loaders/

## Contents

- How it works
- Defining a loader
- Binding to a source
- Column types
- Schema changes
- Write strategies
- Loading from an HTTP API
- Self-managed loaders
- Loader context
- Loader dependencies
- Decorator parameters
- Auto-load during builds
- Source deferral
- Project structure
- Config reference
- Limitations

Loaders are Python functions that load data into source tables. You write a function that returns
or yields rows, and SQLBuild loads them with [SQLLoad](https://github.com/sqlstack-io/sqlload)'s
engine as part of the build, with incremental write strategies, cursors and schema changes handled
for you. Loaders are one of the four [Python node](overview.md) kinds, and
the only one that writes into a SQL source.

## How it works

1. Write a Python function under `python/nodes/` decorated with `@loader`
2. Declare a managed source in `sql/sources/*.yml` with `managed: true` and **the same name as the loader function**
3. SQLBuild runs the function as a SQLLoad Python table and loads what it returns or yields into
   the source's table, with the write strategy set on `@loader`

Loaders participate in the build lifecycle. When `sqb build` runs, managed sources are loaded before any dependent model is materialized.

## Defining a loader

Place Python files under `python/nodes/` in your project directory (the playgrounds use `python/nodes/loaders/`). Each file can contain one or more loader functions:

```python
# python/nodes/loaders/raw_customers/loader.py
from sqlbuild.loaders import loader

@loader(write_strategy="replace")
def raw_customers(ctx):
    """Load customers from the CRM export."""
    return [
        {"id": 1, "name": "Leslie Knope", "email": "leslie@pawnee.gov"},
        {"id": 2, "name": "Ron Swanson", "email": "ron@pawnee.gov"},
    ]
```

The function receives the [loader context](#loader-context) as `ctx`. It returns rows, or yields
them in pieces, which keeps memory flat for large loads:

```python
@loader(write_strategy="append")
def raw_orders(ctx):
    """Orders, a page at a time."""
    for page in fetch_order_pages():
        yield page
```

| Return or yield | Loads |
|-----------------|-------|
| A list of dicts | One row per dict |
| A dict (yield only) | One row |
| Arrow data (yield only): a `pyarrow` table, record batch or reader, or a `polars` or `pandas` frame | Its rows and columns |
| `sl.json(...)` or `sl.ndjson(...)` from `sqlload_api` (yield only) | The rows SQLLoad parses from the JSON |
| An [HTTP job](#loading-from-an-http-api), such as `ctx.http().paginate(...)` | Every page's rows, fetched by SQLLoad |
| `sl.checkpoint(...)` (yield only) | Nothing; SQLLoad commits the rows so far |
| `None` | Nothing: a [self-managed loader](#self-managed-loaders) writes its own table |

A returned value other than a list or an HTTP job is iterated, so return a dict or Arrow data
inside a list, or yield it.

## Binding to a source

Declare a managed source in `sql/sources/*.yml`. A managed source is bound to the loader function **with the same name** - there is no separate `loader` field:

```yaml
sources:
  - name: raw_customers
    description: Customers
    managed: true
    columns:
      - name: id
        type: BIGINT
      - name: name
        type: VARCHAR
      - name: email
        type: VARCHAR
```

Setting `managed: true` makes this a **managed source** - SQLBuild owns both the loading and the schema.
The source declares the columns and [how the load meets schema changes](#schema-changes); how the
rows are written is set on `@loader` (see [Write strategies](#write-strategies)). A managed source
can't set `write_strategy`, `cursor_column`, `unique_key` or `load_batch_size` in its sources file;
compile stops naming the `@loader` option to use instead. The binding is by name: the source `raw_customers` is populated by the `@loader` function named `raw_customers`. SQLBuild raises an error if a managed source has no loader function of the same name.

Models reference managed sources the same way as any other source:

```sql
SELECT id, name FROM __source("raw_customers")
```

## Column types

A column declared in the source's `columns` with a `type` gets that type, and its values are cast
to it. Declare the type SQLLoad creates, such as `BIGINT` for whole numbers on DuckDB. SQLLoad
infers the type of every other column from its values:

| Python value | Column type (DuckDB) |
|--------------|----------------------|
| `bool` | `BOOLEAN` |
| `int` | `BIGINT` |
| `float` | `DOUBLE` |
| `Decimal` | `DECIMAL` wide enough for the column's values |
| `str` | `VARCHAR` |
| `bytes` | `BLOB` |
| `date` | `DATE` |
| `datetime` without a time zone | `TIMESTAMP` |
| `datetime` with a time zone | `TIMESTAMP WITH TIME ZONE` |
| `dict` or `list` | `JSON` |

Other adapters use their own types for the same values, such as `NUMBER` and `TIMESTAMP_NTZ` on
Snowflake.

SQLLoad adds two columns to every table it loads: `_sqlload_load_id`, the load that wrote the row,
and `_sqlload_loaded_at`, when it was written.

Details:

- A value of any other type, such as a `UUID`, loads as its text.
- An undeclared column holding `int` and `float` values is a `DOUBLE` column. Any other mix, such
  as `int` and `str`, makes it a text column.
- A value that can't be cast to the declared type, such as `"two"` in a `BIGINT` column, fails the
  load, naming the column.

## Schema changes

`on_schema_change` on the source says what a load does when the rows don't match the table:

```yaml
sources:
  - name: raw_events
    description: Events
    managed: true
    on_schema_change: fail
    columns:
      - name: event_id
        type: BIGINT
```

| `on_schema_change` | A column the rows add | A declared column the rows lack |
|--------------------|-----------------------|---------------------------------|
| `append_new_columns` (default) | Added to the table | Loads as NULL, with a warning |
| `ignore` | Left out | Loads as NULL, with a warning |
| `fail` | Fails the load | Fails the load |

A column whose values change type, such as `amount` holding numbers in the table and text in the
new rows, fails the load whatever `on_schema_change` says. SQLLoad's error names the column and
the type to declare.

Details:

- `on_schema_change` is set on the source, not on `@loader`. Only an
  [intermediate loader](#loader-dependencies), which fills no source, sets it on `@loader`.
- A source nothing loads can't set `on_schema_change`; compile stops naming the source.
- `contract: enforced` checks how models use the source's columns, and doesn't change how it loads.

## Write strategies

`@loader` takes SQLLoad's table options as keyword arguments, and compile checks them as SQLLoad
reads a Python table's: an option SQLLoad doesn't know, or a value of the wrong kind, stops compile
naming the `@loader`. The most used are `write_strategy`, `cursor_column` and `unique_key`. The
source's own sources file declares the columns and `on_schema_change`, and the source names the
table, so `@loader` can't set `columns`, `on_schema_change`, `table_name` or `destination_path` for
a source it fills.

`write_strategy` controls how the rows are written to the target table. SQLLoad's rules for
combining it with a cursor and keys are checked at compile.

### replace

Full replace. The target is recreated from the loader output on every run. It takes no
`cursor_column`.

```python
@loader(write_strategy="replace")
def raw_countries(ctx):
    """Countries."""
    return [{"country_id": 1, "country_code": "GB"}]
```

### append

Insert all returned rows into the target. No deduplication.

```python
@loader(write_strategy="append")
def raw_webhook_events(ctx):
    """Webhook events."""
    return fetch_new_events()
```

### delete_insert

Replace the rows in the cursor window: delete the target's rows from `ctx.cursor_value` up to
`ctx.cursor_end`, then insert the loader's rows. Needs a `cursor_column`.

```python
@loader(write_strategy="delete_insert", cursor_column="event_at")
def raw_order_events(ctx):
    """Order events."""
    if ctx.cursor_value is None:
        return fetch_all_events()
    return fetch_events_since(ctx.cursor_value)
```

`ctx.cursor_value` is the current `MAX(cursor_column)` of the target, or `None` on the first load,
so the loader fetches only new or updated data. `--start-cursor` and `--end-cursor` on
[`sqb load`](../../cli/load.md) and `sqb build` set the window instead. A row outside the window fails
the load, so a loader fetches only what the window covers. The source it fills declares its columns
as any managed source does:

```yaml
sources:
  - name: raw_order_events
    description: Order events
    managed: true
    columns:
      - name: event_id
        type: BIGINT
      - name: event_at
        type: TIMESTAMP_TZ
      - name: amount_cents
        type: BIGINT
```

### merge

Upsert based on `unique_key`, which `merge` needs.

```python
@loader(write_strategy="merge", unique_key="customer_id", cursor_column="updated_at")
def raw_customers(ctx):
    """Customers."""
    return fetch_customers_since(ctx.cursor_value)
```

Existing rows matching the unique key are updated; new rows are inserted.

### Saved state

A loader whose position isn't a column, such as an API's sync token, keeps it in `ctx.state` with
`state="logged"`. `ctx.state` is the value the last successful load left, or `None` on the first
one; set it to the new position, any JSON value, and SQLLoad saves it with the rows:

```python
@loader(write_strategy="append", state="logged")
def raw_payments(ctx):
    """Payments since the last sync."""
    page = fetch_payments(sync_token=ctx.state)
    yield page.rows
    ctx.state = page.next_sync_token
```

## Loading from an HTTP API

A loader that names an `http` [source connection](../project-configuration.md#warehouse-and-source-connections) with
`connection=` can return or yield `ctx.http()` jobs. SQLLoad sends every request, follows the
pages and parses the responses itself, with the connection's authentication, rate limits and
retries:

```toml
# sqlbuild_project.toml
[connections.shop]
kind = "http"
base_url = "https://shop.example.com/api"
auth = { kind = "bearer" }
token = "${env:SHOP_TOKEN}"
rate_limit = "10/s"
```

```python
import sqlload_api as sl
from sqlbuild.loaders import loader

@loader(connection="shop", write_strategy="merge", unique_key="order_id")
def raw_orders(ctx):
    """Orders from the shop API."""
    return ctx.http().paginate(
        "/orders", paginator=sl.next_link("$.next"), select="$.items"
    )
```

`ctx.http(headers={...})` adds headers to the connection's. `paginate` and `fan_out` take
SQLLoad's job options. SQLLoad checks every job, and a literal path such as `"/orders"`, before it
sends the first request. `ctx.http()` without a `connection=` on `@loader` fails the load, saying what it needs.

## Self-managed loaders

If a loader sets no `write_strategy` and returns `None`, it writes the target itself, using whatever approach makes sense - `ctx.execute_sql()`, an external library, a subprocess, or anything else:

```python
@loader
def raw_status(ctx):
    """Load the order status lookup."""
    ctx.execute_sql(f"DROP TABLE IF EXISTS {ctx.destination}")
    ctx.execute_sql(
        f"CREATE TABLE {ctx.destination} AS "
        "SELECT 1 AS status_id, 'loaded' AS status_name"
    )
```

The source is still declared as managed:

```yaml
sources:
  - name: raw_status
    description: Order status lookup
    managed: true
    columns:
      - name: status_id
        type: INTEGER
      - name: status_name
        type: VARCHAR
```

Self-managed loaders are useful when you want to use adapter-specific SQL (e.g. `COPY INTO`, external tables), call an external ingestion tool, or handle writes in a way that doesn't fit returning rows. SQLLoad doesn't run them, so they have no cursor window, saved state or `ctx.http()`.

Details:

- A loader with a `write_strategy` that returns `None` fails with "Loader 'raw_orders' defines
  write_strategy but returned no rows; return an empty list to load none".
- `ctx.skip(reason=...)` skips the loader: nothing loads, and the source keeps its data.

## Loader context

Every loader function receives a context as its first argument: SQLLoad's table context, with
SQLBuild's warehouse, references and results added.

**Coming from SQLLoad.** `ctx.connection` is the warehouse connection, as everywhere in SQLBuild.
The source connection SQLLoad's `ctx.connection` holds is `ctx.loader_connection`, and
`ctx.source(name)` references a project source.

### From SQLLoad

| Property / Method | Description |
|-------------------|-------------|
| `table` | The loader's table name |
| `cursor_column` | The `cursor_column` set on `@loader`, or `None` |
| `cursor_value` | Where to read from: the target's `MAX(cursor_column)`, or the `--start-cursor` value; `None` on a first load or without a cursor |
| `cursor_end` | Where to stop, not included: the `--end-cursor` value, or `None` for open |
| `state` | The saved position of a `state="logged"` loader; assign to save a new one ([Saved state](#saved-state)) |
| `is_reload` | `True` when `--reload` was passed |
| `logger` | Python logger scoped to the loader |
| `loader_connection` | The settings of the source connection `@loader(connection=...)` names, a mapping (empty without one); `ctx.loader_connection("other")` looks up another source connection |
| `http(headers=...)` | HTTP jobs on the loader's connection ([Loading from an HTTP API](#loading-from-an-http-api)) |

### From SQLBuild

| Property / Method | Description |
|-------------------|-------------|
| `destination` | Fully-qualified destination relation name (where rows are written) |
| `destination_database` | Destination database |
| `destination_schema` | Destination schema |
| `destination_name` | Unqualified destination table name |
| `run_id` | Unique identifier for this execution run |
| `target` | Active target name (e.g. `dev`, `prod`) |
| `target_config` | The active target's database, schema and loader schema ([Target settings](overview.md#target-settings)) |
| `vars` | Project variables (merged from project, target, and local config) |
| `adapter` | The database adapter instance |
| `connection` | The live warehouse connection |
| `execute_sql(sql)` | Execute a SQL statement against the warehouse |
| `query(sql)` | Execute a SQL query against the warehouse and return the cursor |
| `log.info(message)`, `log.warning(message)`, `log.error(message)` | Log a line at that level; it shows with the run's events, while anything the loader prints stays its output |
| `qualify_name(name)` | Return a fully-qualified relation name in the destination database/schema |
| `skip(reason, mode=...)` | Skip this loader. `mode` is `"soft"` (default, skip only this loader) or `"hard"` (also block dependents) |
| `result(payload=, metadata=, materialized=)` | Return a structured result for a self-managed loader |
| `result_of(node_fn)` | Read the result of an upstream asset or loader: its result from this run, else its latest successful one; raises naming the upstream if it never succeeded |
| `results_of(node_fn, limit=N)` | Read the last N successful results of an upstream node, newest first |
| `loader(loader_fn)` | Return a `LoaderRelationRef` for an upstream loader dependency; an asset is refused with a pointer to `result_of` |
| `source(source_name)` | Return a `LoaderRelationRef` for a project source by YAML name |

### LoaderRelationRef

Returned by `ctx.loader()` and `ctx.source()`. Provides access to an upstream relation:

| Property / Method | Description |
|-------------------|-------------|
| `destination` | Fully-qualified relation name |
| `current_cursor_value` | Current `MAX(cursor_column)` from the relation |
| `max(column)` | Return the `MAX` of any column from the relation |

## Loader dependencies

Loaders can depend on other loaders and assets using `depends_on`. Dependencies are executed first. An upstream loader's destination relation is available via `ctx.loader()`:

```python
from sqlbuild.loaders import loader

@loader
def raw_accounts(ctx):
    """Load customer accounts."""
    return [
        {"account_id": 1, "account_name": "Pawnee Parks"},
        {"account_id": 2, "account_name": "Eagleton"},
    ]

@loader(depends_on=[raw_accounts])
def raw_account_metrics(ctx):
    """Account metrics."""
    accounts = ctx.loader(raw_accounts)
    rows = ctx.query(f"SELECT account_id FROM {accounts.destination}")
    return [
        {"account_id": row[0], "metric": "active"}
        for row in rows.fetchall()
    ]
```

Read upstream relations through `ctx.loader(...)` and `ctx.source(...)`, not by writing their table names in SQL. SQLBuild rejects literal SQL that names a project source, model, or seed with `P008`, and warns at run time when dynamic SQL does; loaders cannot read models and seeds. To use model data in a loader, read it in an upstream asset and pass it on through the asset's result. See [Hard-coded relation names](sql-references.md#hard-coded-relation-names).

Dependencies form a DAG. SQLBuild schedules loaders in topological order and executes independent loaders concurrently when `--concurrency` is set.

### Depending on assets

A loader reads an upstream asset's result with `ctx.result_of(...)`; `ctx.loader(...)`
takes loaders only. A failed or hard-skipped upstream asset skips the loader and everything
downstream of the source it fills. Otherwise the loader runs when an asset it depends on,
directly or through other loaders, succeeded; when all of them skip softly or did not run, only the
load is skipped, so the source keeps its last loaded data and its readers still build when another
of their inputs ran. A loader selected without its upstream asset reads that upstream's
latest successful result, and fails naming the upstream if it never succeeded.

```python
from sqlbuild.loaders import loader
from sqlbuild.assets import asset

@asset
def fetch_partner_scores(ctx):
    """Download the scores a partner published."""
    return ctx.result(payload={"scores": [[1, 10], [2, 20]]})

@loader(depends_on=[fetch_partner_scores])
def partner_scores(ctx):
    """Scores the partner returned for each order."""
    scores = ctx.result_of(node_function=fetch_partner_scores).payload["scores"]
    return [{"order_id": order_id, "score": score} for order_id, score in scores]
```

When the asset reads SQL, directly or through other Python nodes, the loader makes a
[warehouse round trip](overview.md#warehouse-round-trips), which the project
must allow with `allow_warehouse_round_trips`. The loader then loads after the SQL the asset reads
and before the models that read its source, in the same `sqb build`.

### Intermediate loaders

Intermediate loaders (those referenced only via `depends_on`, with no managed source of the same name) are given synthetic source entries and write to `__loader__<name>` tables by default. Only the terminal loader - the one whose name matches a managed source - populates that source; intermediate loaders feed it. An intermediate loader without a `write_strategy` that returns rows replaces its table. It declares its own `columns` and `on_schema_change` on `@loader`, as no source declares them. Use the `destination` parameter on the decorator to override the intermediate relation:

```python
@loader(destination="staging.shared_accounts")
def raw_accounts(ctx):
    """Load customer accounts."""
    ...
```

An intermediate loader without a schema in its `destination` writes to the target's schema. When
the target's schema is `preserve`, which keeps each model's own schema, the loader has no schema to
keep, so `compile`, `build` and `load` stop with `S103` until its `destination` names one, such as
`destination="staging.__loader__raw_accounts"`.

## Decorator parameters

| Parameter | Description |
|-----------|-------------|
| `depends_on` | List of loader and asset functions this loader depends on, and [`job()` or `tag()` sets](sql-references.md#depending-on-a-job-or-a-tag), as for assets. Depending on one that reads SQL is a [warehouse round trip](overview.md#warehouse-round-trips) |
| `connection` | The [source connection](../project-configuration.md#warehouse-and-source-connections) the loader reads through, as `ctx.loader_connection` and `ctx.http()`; a target's warehouse connection is refused |
| `destination` | Override the destination relation name of an intermediate loader (can include schema or database); a loader that fills a managed source can't set it |
| `columns` | Column specifications of an intermediate loader; a loader that fills a managed source can't set them, as its source declares them |
| `description` | What the loaded rows are; defaults to the function docstring. Required unless the source YAML entry has a `description` ([Descriptions](../models/configuration.md#descriptions)) |
| any other keyword | One of SQLLoad's table options, such as `write_strategy` (`replace`, `append`, `delete_insert` or `merge`), `cursor_column`, `unique_key` (a column name or a list of them) and `state`, checked at compile |

## Auto-load during builds

By default, `sqb build` automatically loads managed sources before building dependent models. This is controlled by the `auto_load_sources` setting:

```toml
[settings]
auto_load_sources = true   # default
```

You can also control this per-run with CLI flags:

```bash
# Explicitly load sources before building
sqb build --load

# Skip source loading
sqb build --no-load

# Reload sources (passes is_reload=True to loaders)
sqb build --reload
```

With loading off, through `--no-load` or `auto_load_sources = false`, no loader runs, even for a
source selected by name such as `sqb build --no-load --select raw_orders`. Managed sources in the
selection stay in the plan, and their audits check the data they already hold, at the end of the
build if no model reads them. A source outside the selection is neither loaded nor audited.

When `--reload` is passed, `ctx.is_reload` is `True` in the loader function. This lets loaders implement different behavior for full reloads versus normal incremental loads.

## Source deferral

Loader writes and managed source reads are resolved separately. `loader_schema` controls
where a target's loaders write; `defer_sources_to` optionally selects another target whose
managed sources models should read.

```toml
[targets.dev]
schema = "analytics_dev"
loader_schema = "raw_dev"
defer_sources_to = "prod"

[targets.prod]
schema = "analytics_prod"
loader_schema = "raw_prod"
```

With this config:

- `sqb load --target dev` writes to `raw_dev`.
- `sqb load --target prod` writes to `raw_prod`.
- Models built in dev read from `raw_prod` because dev defers source reads to prod.
- Models built in prod read from `raw_prod` because omitted deferral defaults to the active target.

If `loader_schema` is omitted, loaders fall back to the active target's model `schema`, then
the adapter default schema. An explicit `schema` on a managed source overrides the target
default. SQLBuild validates final managed-loader write namespaces and rejects two targets
on the same warehouse/database that could write to the same schema.

`defer_sources_to` never redirects loader writes. It only changes `__source()` and Python
`source()` reads. This lets developers test loaders in an isolated schema without making
development models consume partial development source data.

## Project structure

```
my-project/
  python/
    nodes/
      raw/
        raw_customers/
          loader.py          # one @loader
          main.py
        api_sources/
          loaders.py         # a @factory that makes several loaders
  sql/
    sources/
      raw.yml                # managed source declarations (managed: true)
    models/
      staging/
        stg_customers.sql    # __source("raw_customers")
```

SQLBuild imports all `.py` files under `python/nodes/` recursively (excluding `__init__.py`). Functions decorated with `@loader` become loaders; undecorated helpers do not become nodes. A `@loader` is declared in a file named `loader.py`, and a factory that makes only loaders in `loaders.py`; any other file name fails compilation with `D011` naming the file the declaration belongs in.

## Config reference

### Source YAML fields for managed sources

| Field | Description |
|-------|-------------|
| `managed` | Set to `true` to bind the source to the `@loader` function of the same name |
| `columns` | Column declarations with types |
| `on_schema_change` | `append_new_columns` (default), `ignore` or `fail` ([Schema changes](#schema-changes)) |
| `contract` | `enforced` or `none` |

How the rows are written (`write_strategy`, `cursor_column`, `unique_key` and SQLLoad's other table
options) is set on `@loader`, not in the sources file.

## Limitations

- Loaders write to DuckDB, MotherDuck, Postgres, Snowflake, BigQuery and ClickHouse warehouses. A
  project on SQLite or SQL Server can't declare one, and a Databricks loader fails when it runs, as
  a [Databricks load](../sources.md#loads) does.
