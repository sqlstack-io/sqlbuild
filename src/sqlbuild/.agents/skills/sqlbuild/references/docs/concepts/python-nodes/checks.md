<!-- generated-by: sqlbuild skills -->

# Checks

> Validate tasks, assets, loaders, and SQL relations with Python checks.

Online: https://sqlbuild.com/docs/concepts/python-nodes/checks/

## Contents

- Defining a check
- Results
- Severity
- What checks can depend on
- Decorator parameters
- Running checks
- Checks vs audits

Checks are Python nodes that validate the output of tasks, assets, and loaders, or the data in SQL models, sources, and seeds. They are the Python analog of SQL [audits](../audits.md). See [Python Nodes](overview.md) for the shared model.

Checks are separate graph nodes, not callbacks embedded in a task or asset. A check declares what it validates through `depends_on`.

## Defining a check

Place Python files under `python/` (the playgrounds use `python/checks/`) and decorate functions with `@check`. `depends_on` is required:

```python
# python/checks/orders.py
from sqlbuild.checks import check, CheckContext
from python.tasks.orders import export_orders

@check(depends_on=export_orders)
def check_orders_exported(ctx: CheckContext):
    """Check orders exported."""
    result = ctx.result_of(export_orders)
    if result.metadata.get("rows", 0) == 0:
        return ctx.fail(message="no orders exported")
    return ctx.pass_(message="orders exported")
```

The check receives a `CheckContext` and reads its dependencies' persisted results with `ctx.result_of(...)`.

A check can also validate SQL relations. Declare them with `model()`, `source()`, or `seed()` from `sqlbuild.refs`, and resolve them with `ctx.relation(...)`:

```python
from sqlbuild.checks import check
from sqlbuild.refs import model

@check(depends_on=model("fact_orders"))
def orders_present(ctx):
    """Orders present."""
    orders = ctx.relation(model("fact_orders"))
    count = ctx.query(f"SELECT count(*) FROM {orders}").fetchone()[0]
    return ctx.pass_() if count else ctx.fail(message="no orders")
```

Do not hard-code relation names in check SQL; see [Hard-coded relation names](sql-references.md#hard-coded-relation-names).

## Results

Return a result through the context helpers, or a bool shorthand:

```python
@check(depends_on=orders_asset)
def rows_present(ctx):
    """Rows present."""
    return ctx.result_of(orders_asset).payload["rows"] > 0   # True -> pass, False -> fail
```

| Return | Meaning |
|--------|---------|
| `ctx.pass_(message=None, metadata=None)` | Passing |
| `ctx.fail(message, metadata=None)` | Failing, using the check's severity |
| `ctx.warn(message, metadata=None)` | Warning, regardless of severity |
| `True` | Pass |
| `False` | Fail |

Returning `None` is not allowed - checks must be explicit.

## Severity

`@check` takes a `severity` of `error` (default) or `warn`:

```python
@check(depends_on=export_orders, severity="warn")
def orders_freshness(ctx):
    """Orders freshness."""
    if stale():
        return ctx.fail(message="export is stale")   # recorded as a warning, does not fail the build
    return ctx.pass_()
```

- `error` (default) - a failing check fails `sqb build`.
- `warn` - a failing check is reported but does not fail the build.

`ctx.warn(...)` always produces a warning regardless of the declared severity.

## What checks can depend on

- Checks may depend on **tasks, assets, and loaders**.
- Checks may depend on SQL **models, sources, and seeds** through `model()`, `source()`, and `seed()` references. They may **not** depend on functions.
- Checks may **not** depend on other checks.
- Checks may **not** depend on a terminal source loader directly. Validate loaded source data with a source audit instead.

A check that depends on a single node is displayed grouped under that node. Multi-dependency checks are shown as standalone validation nodes, grouped by `group`, tags, or path.

## Decorator parameters

| Parameter | Description |
|-----------|-------------|
| `depends_on` | Required. Tasks/assets/loaders to validate, and `model()`/`source()`/`seed()` references (function, reference, tuple, or list) |
| `name` | Override the node name (defaults to the function name) |
| `severity` | `error` (default) or `warn` |
| `tags` | Labels for selection and grouping |
| `group` | Display/catalog grouping |
| `description` | Docs (defaults to docstring) |
| `meta` | Freeform JSON metadata |

Checks do not support `columns`, `column_lineage`, or `retry`.

## Running checks

Checks run automatically at the end of `sqb build` when all their Python and SQL dependencies ran in that build. They are skipped when `--no-audits` is passed.

A `sqb build` selector that matches a check must also select the nodes the check depends on. Otherwise the build fails with an error that names the check and the missing node; select that node too, or narrow the selector so it no longer matches the check. A check on a source that no loader writes never runs in a build, so a selector that matches it fails too; run that check with `sqb check`. Jobs and server runs resolve selectors the same way.

To run checks on their own, use [`sqb check`](../../cli/check.md):

```bash
# Run all checks
sqb check

# Run a specific check (and its required dependencies)
sqb check --select +check_orders_exported

# Run checks by tag
sqb check --select tag:exports
```

`sqb check` rejects selecting non-check nodes; use `sqb build` to run tasks and assets. The models, sources, and seeds a check depends on must already exist; `sqb check` does not build them. Check results are written to `target/run/checks/python_checks.json`, and `sqb check --json` prints them to stdout.

## Checks vs audits

| | Checks | Audits |
|---|--------|--------|
| Validates | Python tasks, assets, loaders, and SQL relations | SQL relations |
| Authored in | `python/` (Python) | `MODEL()` headers / `audits/` (SQL) |
| Run by | `sqb build`, `sqb check` | `sqb build`, `sqb audit` |
| Severity | `error`, `warn` | `error`, `warn` |

`sqb build` runs both. `sqb audit` runs SQL audits only; `sqb check` runs Python checks only.
