<!-- generated-by: sqlbuild skills -->

# Assets

> Run Python computation, side effects and external artifacts as nodes in the SQLBuild graph.

Online: https://sqlbuild.com/docs/concepts/python-nodes/assets/

## Contents

- Defining an asset
- Dependencies
- Returning results
- Materialization
- Columns and column lineage
- Skipping
- Retries
- Decorator parameters
- Running assets

Assets are Python functions that run as part of the DAG, for computation, side effects and external artifacts such as an exported file, a published dataset, a dashboard refresh or an ML model. If you are coming from a task-based tool, an asset is a task that may not describe its output.

See [Python Nodes](overview.md) for the shared model and the SQL boundary rules.

## Defining an asset

Place Python files under `python/nodes/` and decorate functions with `@asset`:

```python
# python/nodes/exports/orders_export.py
from sqlbuild.assets import asset, AssetContext

@asset
def orders_export(ctx: AssetContext):
    """Orders export."""
    path = write_orders_csv()
    return ctx.result(payload={"path": path}, metadata={"rows": 1200}, materialized=True)
```

The asset receives an `AssetContext` and returns through `ctx.result(...)`. A plain return value or `None` is also accepted and normalized to a successful result.

## Dependencies

Assets declare dependencies with `depends_on`, accepting a single function, a tuple, or a list. They may depend on other assets and on loaders:

```python
@asset
def fetch_orders(ctx: AssetContext):
    """Fetch orders."""
    return ctx.result(payload=download_orders())

@asset(depends_on=fetch_orders)
def summarize_orders(ctx: AssetContext):
    """Summarize orders."""
    result = ctx.result_of(fetch_orders)
    return ctx.result(metadata={"count": len(result.payload)})
```

`ctx.result_of(node_fn)` reads the latest persisted result of an upstream node, returning a `NodeResultEnvelope` with `payload`, `metadata`, `status`, and `ts` fields. Results persist across runs. Use `ctx.results_of(node_fn, limit=N)` to read result history. Reading a missing or unsuccessful upstream raises unless you pass `default=`.

To read a SQL model or source, declare a typed reference and resolve it at runtime - see [SQL references](sql-references.md):

```python
from sqlbuild.assets import asset
from sqlbuild.refs import model

@asset(depends_on=model("fact_orders"))
def orders_extract(ctx):
    """Orders extract."""
    relation = ctx.relation(model("fact_orders"))
    rows = ctx.query(f"SELECT count(*) FROM {relation}").fetchone()[0]
    return ctx.result(metadata={"rows": rows}, materialized=True)
```

## Returning results

```python
@asset
def build_export(ctx: AssetContext):
    """Build export."""
    return ctx.result(
        payload={"path": "/exports/orders.csv"},
        metadata={"rows": 1200},
        materialized=True,
    )
```

- `payload` is the value downstream nodes read with `ctx.result_of(...)`.
- `metadata` is structured JSON for catalogs and downstream reads.
- `materialized` records whether the asset produced its artifact this run (see below).

## Materialization

Set `materialized` when the asset produces or observes an artifact:

```python
@asset
def orders_export(ctx: AssetContext):
    """Orders export."""
    if nothing_changed():
        return ctx.result(metadata={"status": "unchanged"}, materialized=False)
    return ctx.result(metadata={"path": export()}, materialized=True)
```

- `materialized=True` - the artifact was produced this run.
- `materialized=False` - the asset ran but produced nothing (e.g. it only observed state). This is **not** a skip; the node still succeeded.
- Leave it unset for an asset that runs computation or a side effect without an artifact.

## Columns and column lineage

Assets can declare a schema for catalog and lineage purposes. This does not enforce anything at runtime; it describes the artifact:

```python
@asset(
    columns=[
        {"name": "order_id", "type": "INTEGER"},
        {"name": "total_cents", "type": "INTEGER", "nullable": False},
    ],
    column_lineage={
        "order_id": [{"node": "fact_orders", "column": "order_id"}],
        "total_cents": [{"node": "fact_orders", "column": "total_cents"}],
    },
)
def orders_export(ctx):
    """Orders export."""
    ...
```

- `columns` - column declarations with `name`, optional `type`, `nullable`, `description`, and `meta`.
- `column_lineage` - maps each asset column to upstream `{node, column}` references, surfaced in the DAG artifact and integrations.

Checks do not support `columns` or `column_lineage`.

## Skipping

Return `ctx.skip(...)` to skip an asset:

```python
@asset
def export_if_present(ctx: AssetContext):
    """Export if present."""
    if not new_files_available():
        return ctx.skip("no new files")
    return ctx.result(payload=do_export())
```

- `"soft"` (default) skips only this asset; dependents may still run if another upstream succeeded.
- `"hard"` skips this asset and blocks its dependents.

`mode` accepts either a plain string or the `SkipMode` enum:

```python
from sqlbuild.assets import asset, SkipMode

@asset
def optional_step(ctx):
    """Optional step."""
    return ctx.skip("nothing to do", mode=SkipMode.HARD)  # or mode="hard"
```

## Retries

`@asset` accepts a `retry` policy for transient failures:

```python
from sqlbuild.assets import asset
from sqlbuild.retries import RetryPolicy

@asset(retry=RetryPolicy(max_attempts=3, retry_on=(ConnectionError,)))
def call_api(ctx):
    """Call api."""
    return ctx.result(payload=fetch_from_flaky_api())
```

| Field | Default | Description |
|-------|---------|-------------|
| `max_attempts` | `3` | Total attempts including the first |
| `retry_on` | `Exception` | Exception class, tuple, or list to retry on |
| `initial_delay_seconds` | `1.0` | Delay before the first retry |
| `backoff_multiplier` | `2.0` | Exponential backoff multiplier |
| `max_delay_seconds` | `30.0` | Cap on any single delay |
| `max_elapsed_seconds` | `None` | Overall time bound for retries |
| `jitter` | `True` | Randomize delays to avoid synchronized retries |

The default is no retry. Set `retry_on` explicitly rather than relying on the broad default when you can. The original exception is preserved if all attempts fail.

## Decorator parameters

| Parameter | Description |
|-----------|-------------|
| `name` | Override the node name (defaults to the function name) |
| `depends_on` | Upstream nodes (function, tuple, or list); `model()`/`source()`/`seed()` for read-only SQL refs |
| `columns` | Column declarations for the produced artifact |
| `column_lineage` | Map of asset column to upstream `{node, column}` references |
| `tags` | Labels for selection and grouping |
| `group` | Display/catalog grouping |
| `description` | Docs (defaults to docstring) |
| `meta` | Freeform JSON metadata |
| `retry` | A `RetryPolicy` |
| `for_each` | A `@discover` function; the asset runs once per item it returns. See [Discovered nodes](discovered-nodes.md) |

## Running assets

```bash
# Run an asset and its required graph
sqb build --select orders_export --no-tests --no-audits

# Include in a full build with its upstreams
sqb build --select +orders_export
```

Assets run during `sqb build`. Use `--no-python` to suppress read-side assets while still loading sources. They are not validated unless you also write a [check](checks.md) that depends on them.
