<!-- generated-by: sqlbuild skills -->

# Factories

> Generate Python nodes programmatically with @factory.

Online: https://sqlbuild.com/docs/concepts/python-nodes/factories/

A factory is a function that **generates** Python nodes instead of authoring them one at a time. Use `@factory` when you want to create many similar [assets](assets.md), [loaders](loaders.md), or [checks](checks.md) from a list, a config, or a loop, rather than hand-writing each one.

Factories are an advanced feature. Most projects author nodes directly; reach for a factory when you find yourself copy-pasting near-identical node definitions.

## Defining a factory

A `@factory` function takes **no arguments** and returns one or more decorated node functions:

```python
# python/nodes/factories/exports.py
from sqlbuild.factories import factory
from sqlbuild.assets import asset

TABLES = ["orders", "customers", "payments"]

@factory
def export_assets():
    nodes = []
    for table in TABLES:
        @asset(name=f"export_{table}", tags=("export",))
        def export(ctx, table=table):
            """Export one table to object storage."""
            return ctx.result(metadata={"table": table}, materialized=True)
        nodes.append(export)
    return nodes
```

The returned nodes are discovered and added to the graph exactly as if you had written them by hand. They participate in selection, lifecycle, the DAG artifact, and integrations like any other node.

A factory may return a single node or a list, tuple, or set of nodes. Every returned item must be a decorated `@asset`, `@loader`, or `@check` function.

## Where factories live

Factories live anywhere under `python/nodes/`, like every other Python node. The decorators on the returned functions decide each node's kind, so a factory may emit a single kind or a mix, and it can sit next to hand-written nodes in the same file. The playgrounds keep factories in `python/nodes/factories/`.

Factory modules are normal Python: constants, classes, and undecorated helpers are allowed, and shared helpers can live in their own modules, for example `python/nodes/factories/_helpers.py` imported as `from python.nodes.factories._helpers import TABLES`.

## Mixed-kind factories

A single factory can generate a whole related pipeline at once - a loader, the asset that reads it, and a check on the result:

```python
# python/nodes/factories/orders.py
from sqlbuild.factories import factory
from sqlbuild.loaders import loader
from sqlbuild.assets import asset
from sqlbuild.checks import check

@factory
def orders_pipeline():
    @loader(name="raw_orders")
    def load(ctx):
        """Load orders from the storefront API."""
        return fetch_orders()

    @asset(name="orders_export", depends_on=load)
    def export(ctx):
        """Export the order summary."""
        return ctx.result(materialized=True)

    @check(depends_on=export)
    def orders_export_check(ctx):
        """Check that the orders export is ready."""
        return ctx.pass_(message="export ready")

    return [load, export, orders_export_check]
```

Generated nodes follow the same rules as directly-authored ones, including the [SQL boundary](overview.md#the-sql-boundary): a factory-generated loader still binds to a managed source, and a factory-generated check still may not depend on SQL models.

## Naming

Generated nodes need unique names across the project. Pass an explicit `name=` to each node a factory creates (factories almost always generate names from a loop variable or config), since relying on the function's own name would produce duplicates.
