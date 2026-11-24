<!-- generated-by: sqlbuild skills -->

# SQL References

> Read SQL models, sources, and seeds from Python nodes without creating SQL dependencies.

Online: https://sqlbuild.com/docs/concepts/python-nodes/sql-references/

## Contents

- Declaring a reference
- Resolving with ctx.relation
- Hard-coded relation names
- Depending on a job or a tag
- model vs source
- Where references are allowed

Python nodes can **read** SQL models, sources, and seeds at runtime, but SQL cannot depend on Python nodes (see the [SQL boundary](overview.md#the-sql-boundary)). Typed references make this read-only access explicit and safe.

## Declaring a reference

Use `model()`, `source()`, and `seed()` from `sqlbuild.refs` in the `depends_on` of an asset or check:

```python
from sqlbuild.refs import model, seed, source
from sqlbuild.assets import asset

@asset(depends_on=model("fact_orders"))
def export_orders(ctx):
    """Export orders."""
    ...

@asset(depends_on=source("raw_orders"))
def inspect_raw(ctx):
    """Inspect raw."""
    ...

@asset(depends_on=seed("country_codes"))
def export_countries(ctx):
    """Export countries."""
    ...
```

Declaring the reference does two things:

1. It tells SQLBuild the node reads that SQL resource, so the node is scheduled **after** the resource is built (read-side).
2. It authorizes `ctx.relation(...)` to resolve that reference at runtime.

It does **not** make the SQL resource depend on the Python node. The dependency is one-way: Python reads SQL, never the reverse.

The reference is a graph edge. It appears in `sqb dag --json`, and selector expansion follows it: `+asset:export_countries` also selects the `country_codes` seed and its upstream, and `country_codes+` also selects the assets that read it. Checks are never added by expansion; they run when their dependencies run.

## Resolving with ctx.relation

Always resolve a reference to its concrete relation with `ctx.relation(...)` instead of hardcoding the table name:

```python
@asset(depends_on=model("fact_orders"))
def export_orders(ctx):
    """Export orders."""
    relation = ctx.relation(model("fact_orders"))
    rows = ctx.query(f"SELECT * FROM {relation}").fetchall()
    return ctx.result(metadata={"rows": len(rows)})
```

`ctx.relation(...)` returns the adapter-qualified relation name for the current run. Passing a reference that was not declared in `depends_on` raises an error.

Do not hardcode model or source names in raw SQL. A model's relation depends on the target, its schema and any deferral, so a query like `ctx.query("SELECT * FROM fact_orders")` can fail or read the wrong relation. Always use `ctx.relation(model("fact_orders"))`. SQLBuild checks for this; see [Hard-coded relation names](#hard-coded-relation-names).

## Hard-coded relation names

SQLBuild checks SQL passed to `ctx.query()` and `ctx.execute_sql()` in assets, checks,
loaders, and [Python hooks](../models/hooks/python.md) for names of project relations:
models, sources, and seeds.

**At compile time**, SQL written as a string literal, or as an f-string whose table names are
literal text, is parsed with the project's SQL dialect. Naming a project relation fails to compile
with `P008`:

```
error[P008]: asset:export_orders names model:fact_orders as 'fact_orders' in SQL passed to ctx.query()
  --> python/nodes/exports/export.py:9
  = help: declare it with depends_on=model("fact_orders") and use ctx.relation(model("fact_orders")) instead of the relation name
  = help: if 'fact_orders' is an external table that shares the name, qualify it with its schema
  = help: while migrating a project, allow hard-coded relation names with [references] enforce_explicit = false in sqlbuild_project.toml
```

An unqualified name that matches a project relation is treated as that relation. If you mean an
external table that shares the name, qualify it with its schema.

Loaders run before models and seeds, so a loader naming a model or seed fails with a help saying it
cannot read them. A loader naming a source is told to use `ctx.source("name")`, or
`ctx.loader(upstream_loader)` when the source is populated by a loader it depends on.

The old name of a [renamed model](../models/migrations.md#old-names) counts as a project
relation while a compatibility view is kept there, so literal SQL still reading it fails with
`P008` and names the new model. A declared `migrate_from` is checked at compile time; an
automatically discovered rename is checked when planning, and stops the build. A hook's own model
does not exempt its old name.

**At run time**, SQL built dynamically cannot be checked earlier. Before sending it,
`ctx.query()` and `ctx.execute_sql()` parse it in the adapter's dialect, resolve unqualified names
against the current schema, and compare them with the project relations resolved for the run. A
project relation that the node did not obtain through `ctx.relation(...)` (or, in a loader,
`ctx.source(...)` or `ctx.loader(...)`) produces a `P008` **warning**. The query still runs. The warning appears in the build summary, counts the node as
`WARN`, and is stored under the node's `warnings` in the `--json` run results.

Both checks ignore:

- Relations resolved through `ctx.relation(...)`, `ctx.source(...)`, or `ctx.loader(...)`, a hook's
  own model (`ctx.destination`), and a loader's own source
- Relations that are not project relations, such as `information_schema` tables, external tables,
  and temporary tables the node creates
- CTE names
- SQL that cannot be parsed; it is never blocked, only logged at debug level

This is a guard against accidental hard-coding, not a sandbox. SQL sent directly through
`ctx.connection` or `ctx.adapter` is not checked. Run-time checks apply to `sqb build`,
`sqb check`, and `sqb load`. `sqb load` does not compile models, so it checks loaders against
the project's sources. To turn both checks off while migrating a project,
set [`[references] enforce_explicit = false`](../project-configuration.md#references).

## Depending on a job or a tag

Naming every upstream model ties a node to model names that change with each rename or split.
`job()` and `tag()` from `sqlbuild.refs` depend on a whole set instead, in an asset's or a loader's
`depends_on`:

```python
from sqlbuild.refs import job, tag
from sqlbuild.assets import asset

@asset(depends_on=(job("orders_job", exclude="tag:experimental"), tag("finance_inputs")))
def publish_revenue(ctx):
    """Publish revenue."""
    ...
```

- `job(name)` depends on the job's **end nodes**: the models, seeds, sources, loaders and
  assets of its selection that nothing else in the selection reads. Everything else in the job is
  upstream of an end node, so this waits for the whole job without listing every edge. A job group
  name depends on the union of its jobs.
- `tag(name)` depends on the end nodes of every node carrying the tag.
- `exclude=` takes the usual [selectors](../selectors.md) (names, `tag:`, `path:`,
  `kind:`) and removes those nodes before the set is reduced to its end nodes.

Compile resolves each into ordinary edges to the end nodes, so planning, `--changed`, selector
expansion (`+asset:publish_revenue`), `sqb dag` and job group waits treat them like named
dependencies. A model named directly and also reached through a job is one edge. Jobs resolve
whatever targets they are declared for.

The edges only order the node. `ctx.relation(...)` still resolves only the `model()`, `source()`
and `seed()` references the node names, and column lineage stops at these edges.

When a job's or tag's end nodes change, the plan says so under the node, against the end nodes
it was last built with:

```text
Python read-side (1)
└── publish_revenue      asset (unchanged)
    └── publish_revenue now also waits on model 'refund_totals' through job 'orders_job'
```

`--json` lists the same lines as `wait_changes` on the Python node.

Compile fails when:

- the job or tag does not exist, or the set is empty after `exclude`;
- a node of the set reads the dependant's output, directly or through other nodes. The error names
  the node that reads back and suggests moving it to another job or excluding it;
- a [job group](../orchestration.md#group-jobs) would wait in a loop: job groups wait job
  by job, so if the dependant's job waits for the job it depends on while that job reads a node of
  the dependant's job, neither can start. The error names the nodes that close the loop; move
  them to another job;
- the name has a code location prefix such as `job("billing:nightly_job")`, which is reserved for
  depending on other locations' jobs.

## model vs source

| Reference | Resolves to |
|-----------|-------------|
| `model("name")` | The built model relation |
| `source("name")` | The source read relation, following deferred-source semantics |
| `seed("name")` | The loaded seed relation |

`source(...)` respects `defer_sources_to`, so a node reading a source in `dev` can read the deferred target's data just like SQL models do.

## Where references are allowed

- **Assets** and **checks** may declare `model()`, `source()`, and `seed()` references and read them with `ctx.relation(...)`, and depend on a `job()` or `tag()`.
- **Python hooks** declare the models, sources, and seeds they read with `@hook(reads=...)`; see [Python Hooks](../models/hooks/python.md#declared-reads).
- **Loaders** may not reference SQL resources; they read sources with `ctx.source(...)` and upstream loaders with `ctx.loader(...)`.
- A Python node referencing a SQL resource never turns into a SQL dependency; selector expansion will not pull Python outputs into SQL model dependencies.
