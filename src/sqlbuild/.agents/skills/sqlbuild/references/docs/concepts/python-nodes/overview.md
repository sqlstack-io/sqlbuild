<!-- generated-by: sqlbuild skills -->

# Overview

> Loaders, assets, and checks as first-class nodes in the SQLBuild graph.

Online: https://sqlbuild.com/docs/concepts/python-nodes/overview/

## Contents

- The `python/nodes/` folder
- The SQL boundary
- Decorators
- Identity tracking
- Runtime context
- Returns and skips
- Result persistence
- Selection
- Lifecycle: run, build, check
- Try it

Python nodes let your project grow beyond warehouse-only SQL while keeping the SQL graph clean. They are ordinary Python functions, decorated to become nodes in the same DAG as your SQL models, and they run as part of `sqb build`.

There are three kinds. The decorator says what each function is, so all of them live under one `python/nodes/` folder:

| Kind | Decorator | Purpose |
|------|-----------|---------|
| Loader | `@loader` | Load external data into a managed source |
| Asset | `@asset` | Run Python computation or side effects, or produce an external artifact |
| Check | `@check` | Validate assets and loaders |

All three share the same decorator conventions, dependency model, selection syntax, and runtime context helpers. The pages for each kind cover their specifics:

- [Loaders](loaders.md) - load data into managed sources
- [Assets](assets.md) - Python computation, side effects and external artifacts
- [Checks](checks.md) - Python validations
- [Factories](factories.md) - generate nodes programmatically with `@factory`
- [SQL references](sql-references.md) - read SQL models and sources from Python

Nodes can also be generated programmatically with [`@factory`](factories.md) instead of authored one at a time.

## The `python/nodes/` folder

Anything that runs as part of the pipeline graph goes in `python/nodes/`: loaders, assets, checks, [factories](factories.md), and the helper modules they import. SQLBuild imports every `.py` file under `python/nodes/` recursively (except `__init__.py`) and registers each decorated function by its decorator. Undecorated functions and helper-only modules are allowed and never become nodes.

Each node lives in a folder of its own under `python/nodes/`, in a file named for what it declares:

| File | Declares |
|------|----------|
| `loader.py` | one `@loader` |
| `asset.py` | one `@asset` |
| `checks.py` | `@check` functions, or a factory that makes only checks |
| `loaders.py` | a `@factory` that makes only loaders |
| `assets.py` | a `@factory` that makes only assets |
| `factory.py` | a `@factory` that makes more than one kind |

A folder holds one of these declaration files, plus `checks.py` for the checks of its node. Any other
file name for a declaration fails compilation with `D011`, which names the file the declaration
belongs in:

```text
error[D011]: @asset 'orders_export' is declared in python/nodes/orders/export/node.py; an @asset is declared in asset.py
```

Helper modules, `main.py`, and role files such as `constants.py` and `models.py` may sit beside the
declaration file. `@discover` and `@audit_factory` functions are not node declarations and may sit in
any module under `python/nodes/`. The playgrounds group nodes by kind:

```
python/nodes/
  loaders/orders/loader.py
  assets/prepare_raw_orders/asset.py
  assets/orders_export/asset.py
  checks/orders_export/checks.py
```

Grouping by domain (`python/nodes/orders/`, `python/nodes/customers/`) works just as well. A project that selects the [Python Rules](../rules/python-rules.md#laying-out-pythonnodes) commits to grouping by domain, with each node in its own [unit folder](../rules/python-rules.md#writing-nodes-as-units) holding its declaration file and `main.py`: `SQBRPYTHON001` reports node-kind folders such as `python/nodes/loaders/` and modules directly under `python/nodes/`. Import other project modules by their path, for example `from python.nodes.assets.prepare_raw_orders.asset import prepare_raw_orders` or `from python.providers.warehouse_client.provider import WarehouseClient`.

Code that extends SQLBuild itself keeps its own folder, such as `rules/`, `python/materializations/`, `python/adapters/`, `python/providers/`, `python/event_sinks/`, `sql/hooks/python/`, `sql/functions/python/`, and `sql/macros/`. Python outside the supported locations fails compilation with `D016`; see [Project layout](../project-configuration.md#project-layout) for the full list.

## The SQL boundary

The most important rule: **SQL models never depend on Python nodes.** The dependency direction is strictly one way.

- SQL models depend only on other SQL resources (models, sources, seeds, functions).
- Python nodes may depend on other Python nodes; a loader may depend on any Python node, including assets.
- Python data reaches SQL only through a **loader filling a source**: `loader -> source -> model`.
- Assets and checks may **read** SQL models, sources, and seeds at runtime through typed references (see [SQL references](sql-references.md)), but reading a model does not make it a SQL dependency.

This keeps the SQL graph fully analyzable and testable on its own, while letting Python participate around the edges.

```
            depends_on
  asset ---------------> asset
   |                       |
   | (read only)           v
   |                     asset
   v                       |
 source <-- loader         | check (validates assets/loaders/SQL)
   |
   v
 model (SQL) ----> model (SQL)
```

### Warehouse round trips

A loader that depends on an asset that reads SQL, directly or through other Python nodes,
makes a warehouse round trip: data leaves the warehouse and comes back through the loader. For
example, an asset reads `orders`, sends them to a partner's scoring service, and a loader loads the
scores it returned into the `partner_scores` source that `scored_orders` reads:

```
 model orders --> asset score_orders --> loader partner_scores --> source partner_scores --> model scored_orders
```

Round trips are off by default. Turn them on with `allow_warehouse_round_trips` in
`sqlbuild_project.toml` (see [Settings](../project-configuration.md#settings)); without it,
a plan or build that includes such a loader stops with an error naming the loader and the asset
that reads SQL. A loader that depends on an asset that reads no SQL is ordinary
ingestion and is always allowed.

```toml
[settings]
allow_warehouse_round_trips = true
```

With the setting on:

- One `sqb build` runs the SQL the asset reads, then the asset, then the loader, then the models that
  read the loader's source. The loader follows the usual rule for its upstream assets: if
  one fails or skips with `mode="hard"`, the loader and everything downstream of its source are
  skipped; otherwise it runs when any of them succeeded. When all of them skip softly, only the load
  is skipped: the source keeps its last loaded data, and models reading it still build when another
  of their inputs ran.
- Lineage and column lineage restart at the source: the source's columns come from the loader, not
  from the models the asset read.
- Job groups run a member that builds the asset before the member that runs the loader, and that one
  before the members that build models reading its source.
- `--changed` follows the chain: when the SQL the asset reads is rebuilt, the asset and loader run and
  the models reading the source rebuild. When it is not rebuilt, the asset does not run and the
  loader is skipped softly, so the source keeps its data and models selected for their own changes
  still build.
- An asset that reads anything built from the source its own downstream loader fills is a cycle,
  which stops the plan or build with the path, such as
  `loader 'partner_scores' -> asset 'score_orders' -> model 'scored_orders' -> source 'partner_scores' -> loader 'partner_scores'`.

## Decorators

Every decorator accepts the same organizational metadata:

| Argument | Description |
|----------|-------------|
| `name` | The node's name; defaults to the function's name |
| `depends_on` | A single function, tuple, or list of upstream nodes (and, where allowed, `model()`/`source()`/`seed()` references or a `job()`/`tag()`; see [SQL References](sql-references.md#depending-on-a-job-or-a-tag)) |
| `tags` | Labels for selection, filtering, and catalog grouping |
| `group` | A display/catalog grouping string |
| `description` | Required human-readable docs; defaults to the function docstring ([Descriptions](../models/configuration.md#descriptions)) |
| `meta` | Freeform JSON metadata for catalogs and integrations |

`@asset` also accepts a `retry` policy, `columns` and `column_lineage`. Node kind is inferred from the decorator - you never pass `kind=`.

## Identity tracking

Every Python node is fingerprinted by source code hash, decorator config hash, and transitive dependency hashes (scoped to the git root, so third-party package updates don't affect identities). The plan shows source and dependency diffs when a node's identity changes, giving you visibility into what changed in your Python code.

Unlike SQL models, Python nodes may depend on external inputs the framework cannot observe (APIs, files, third-party services). Skip/run decisions are therefore user-controlled via `ctx.skip()`: the node's own logic decides whether it needs to run. See [Planning and Change Detection](../planning.md#python-nodes) for details.

## Runtime context

Each node receives a context object as its first argument (`AssetContext`, `CheckContext`, or `LoaderContext`). They share these helpers:

| Helper | Description |
|--------|-------------|
| `ctx.run_id` | Unique identifier for this run; in a worker, the worker's own ID rather than the server's run ID |
| `ctx.target` | Active target name |
| `ctx.target_config` | The active target's database and schemas, as SQLBuild resolves them (see [Target settings](#target-settings)) |
| `ctx.vars` | Project variables |
| `ctx.is_reload` | `True` when `--reload` was passed |
| `ctx.adapter` / `ctx.connection` | Adapter and live connection |
| `ctx.log.info(message)`, `ctx.log.warning(message)`, `ctx.log.error(message)` | Log a line at that level; it shows with the run's events while the node runs, while anything the node prints stays its output (see [Logging](#logging)) |
| `ctx.query(sql)` / `ctx.execute_sql(sql)` | Run SQL on the connection |
| `ctx.qualify_name(name)` | Qualify a relation name |
| `ctx.relation(ref)` | Resolve a declared `model()`/`source()` reference to a relation |
| `ctx.result_of(node_fn)` | Read the result of an upstream node: its result from this run, else its latest successful one |
| `ctx.results_of(node_fn, limit=N)` | Read the last N successful results of an upstream node, newest first |
| `ctx.providers` | Access discovered [providers](providers.md) by name |

Asset contexts add `ctx.result(...)`, `ctx.skip(...)`, `ctx.root_run_id` and `ctx.discovered` (see [Discovered nodes](discovered-nodes.md#in-a-node)). Check contexts add `ctx.pass_(...)`, `ctx.fail(...)`, and `ctx.warn(...)`.

Providers can also be injected directly as function parameters by name. See [Providers](providers.md) for details.

### Target settings

Read the active target's locations from `ctx.target_config` instead of keeping each target's schema names in Python. The values are resolved the way SQLBuild resolves them for models and loaders, including `sqlbuild_local.toml` overrides, and are read-only:

| Field | Value |
|-------|-------|
| `name` | Active target name, or `None` without targets |
| `database` | Database models build into, or `None` when the adapter has none or the value is `preserve` |
| `schema` | Schema models build into, or `None` when it is `preserve` |
| `loader_schema` | Schema a managed source without its own `schema` loads into: `loader_schema`, else the target's `schema`; `None` when that schema uses a `${CTX:...}` template |
| `database_preserved` / `schema_preserved` | `True` when the target sets that value to `preserve` |

```python
@asset
def partner_export(ctx):
    """Export orders to the partner staging table."""
    target = ctx.target_config
    staging = f"{target.database}.{target.loader_schema}.partner_staging"
    ctx.execute_sql(f"CREATE OR REPLACE TABLE {staging} AS SELECT * FROM orders")
    return ctx.result()
```

Connection settings and secrets are not part of `ctx.target_config`.

### Logging

`ctx.log` is for milestones: a few lines that say how far a node has got, such as `loaded 3 orders`
or `step 2 of 5`. Each line shows as soon as it is logged: a local `sqb build` prints it, and on a
server it reaches the run page's Log tab within a few seconds, while the node is still running.

A node keeps its first 200 `ctx.log` lines as log lines. When it logs more, the rest are counted
rather than kept, and one last warning says how many: `N more lines not kept as log lines; ctx.log
keeps 200 per node, and print() output is kept in full in Output`. The count is also the node's
`dropped_log_lines` in `--json` output.

Send a long stream, such as a subprocess's output, to `print()` instead. Everything a node prints
is kept in full and shows in the run's Output tab as it arrives:

```python
import subprocess

@asset
def partner_export(ctx):
    """Run the partner export tool, keeping its full output."""
    ctx.log.info("export started")
    tool = subprocess.Popen(["partner-export"], stdout=subprocess.PIPE, text=True)
    for line in tool.stdout:
        print(line, end="")
    tool.wait()
    ctx.log.info(f"export finished with exit code {tool.returncode}")
    return ctx.result()
```

A subprocess that writes to the node's own standard output directly, without the node reading its
output, isn't captured; read its output and print it, as above.

## Returns and skips

Assets return through `ctx.result(...)`:

```python
@asset
def export_orders(ctx):
    """Export orders."""
    return ctx.result(payload={"rows": 120}, metadata={"rows": 120})
```

- A plain value or `None` is also accepted and normalized to a successful result.
- `ctx.skip(reason, mode=...)` skips the node. `mode` accepts `"soft"` (default, skips only this node) or `"hard"` (also blocks dependents), as a string or the `SkipMode` enum from `sqlbuild.assets`.
- Assets may pass `materialized=True`/`False` to record whether an artifact was produced.

Within one run, downstream nodes run only if at least one upstream succeeded. If all upstreams are skipped, the downstream is skipped. A failed or hard-skipped upstream blocks its dependents.

## Result persistence

Node results (payload, metadata, status, errors) are persisted after each execution in a
`_sqlbuild_node_results` table. Results persist across runs, so they are available for
observability, debugging, and downstream consumption through `ctx.result_of`.

Results of loaders, assets and checks all go to one schema, chosen by the target's `schema`:

| Target `schema` | Results table schema |
|-----------------|----------------------|
| A schema name, such as `analytics` | That schema, in the target's database |
| `preserve` | The target's `loader_schema`; commands that compile the project stop with `S105` until it is set |
| Unset, or resolved per model, such as `${CTX:...}` | The adapter's default schema, such as `main` on DuckDB or `public` on PostgreSQL |

A target `database` of `preserve` keeps results in the connection's database.

## Selection

Python nodes are selected like SQL resources, by bare name or typed selector:

```bash
sqb build --select export_orders          # bare name
sqb build --select asset:orders_export     # typed
sqb build --select export_orders check:check_orders_exported  # a check with the node it checks
sqb build --select tag:exports             # by tag
sqb build --select +orders_export           # with upstreams
```

Names are globally unique across models, sources, seeds, functions, loaders, assets, checks, providers, hooks, unit tests, scenarios, jobs, job groups, sensors, and calendars.

## Lifecycle: run, build, check

Python nodes run in two phases relative to SQL:

- **Ingress (pre-SQL):** loaders, and assets that feed sources, run before SQL models are built.
- **Read-side (post-SQL):** assets that read SQL run after their SQL dependencies are built.
  A loader in a [warehouse round trip](#warehouse-round-trips) loads after them, and before the
  models that read its source.

Read-side assets follow the same selection rule as SQL models. A SQL dependency that is
part of the run gates the node: it waits for that model, seed, or source load, and is skipped if the
dependency fails or is skipped. A dependency that is not part of the run (not selected, or an
unmanaged source) is read as it currently exists, so `sqb build --select asset:export_orders` runs
the asset against the relations already in the warehouse.

Python dependencies follow the same rule. An asset selected without its Python upstream
reads that upstream's latest successful result through `ctx.result_of`. If the upstream has never
succeeded for the target, the node fails with an error that names the upstream, for example:

```text
Upstream asset 'prepare_orders' has no successful result to read. Build it first with
'sqb build --select asset:prepare_orders', or build both with 'sqb build --select +asset:export_orders'.
```

Jobs, sensors, roles and ad-hoc runs on the server resolve selectors the same way, so a job can
select one Python node and a job group orders it after the job that builds its upstream.

The commands differ in what they include by default:

| Command | SQL | Loaders / assets | Checks | Audits |
|---------|-----|-----------------|--------|--------|
| `sqb build` | Yes | Yes | Yes | Yes |
| `sqb build --no-tests --no-audits` | Yes | Yes | No | No |
| `sqb check` | No | No | Selected checks only | No |

- `sqb build` is the complete build-and-validate command: it runs SQL, the required Python nodes, SQL audits, and Python checks.
- `sqb build --no-tests --no-audits` executes the DAG without validation, for fast iteration.
- `sqb check` runs Python checks only. See [`sqb check`](../../cli/check.md).

Use `--no-python` on `plan` and `build` to suppress read-side assets. Loader-side Python required to populate selected sources still runs (use `--no-load` to skip source loading). See the [`sqb build`](../../cli/build.md) reference.

## Try it

The `python_nodes` playground is a small working project with assets, a loader, a model, and a check:

```bash
sqb playground --template python_nodes
cd sqlbuild-playground
sqb build --select +fact_orders --select +orders_export
sqb check --select +check_orders_export
```

See [`sqb playground`](../../cli/playground.md).
