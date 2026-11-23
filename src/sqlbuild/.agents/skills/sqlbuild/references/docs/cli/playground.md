<!-- generated-by: sqlbuild skills -->

# sqb playground

> Create a self-contained SQLBuild project to explore locally.

Online: https://sqlbuild.com/docs/cli/playground/

Creates a self-contained waffle shop project with DuckDB. No warehouse credentials, no git clone, no external data - just a working project you can compile, build, test, and explore immediately.

## Usage

```bash
sqb playground [name]
```

The positional argument is the directory to create (default `sqlbuild-playground`). The template is chosen with `--template` (default `waffle_shop`).

## Templates

| Template | Description |
|----------|-------------|
| `waffle_shop` | Default. DuckDB-backed project with models, tests, scenarios, and macros. |
| `dagster` | A Dagster code location: a ready-to-run `definitions.py` next to the waffle shop SQLBuild project in `waffle_shop/`. |
| `rivers` | A Rivers code repository: a `definitions.py` repository definition next to the waffle shop SQLBuild project in `waffle_shop/`. |
| `python_nodes` | A small DuckDB project demonstrating [Python nodes](../concepts/python-nodes/overview.md): an asset feeding a loader, a model read by a Python asset through `ctx.relation(model(...))`, a soft-skip fan-in, `materialized=False`, and a Python check. |

## What it creates

A complete DuckDB-backed project with:

- Staging views, fact/dimension tables, and incremental models
- Sources with inline expression data (no external setup)
- Python source loaders in `python/nodes/loaders/`
- Seeds, SQL functions, and a custom materialization
- Built-in and custom audits
- SQL unit tests and multi-model tests
- E2E scenario tests
- Python macros
- AI agent skill files (auto-installed for OpenCode, Claude Code, and other agents)

The `dagster` and `rivers` templates keep orchestrator code outside the SQLBuild project:

```text
sqlbuild-playground/
  definitions.py     # Dagster or Rivers definitions pointing at waffle_shop/
  README.md          # setup instructions
  waffle_shop/       # the waffle shop SQLBuild project, including agent skill files
```

The Dagster `definitions.py` uses `sqlbuild_assets`, `sqlbuild_scenario_checks`, and `SqlBuildCliResource`; the Rivers `definitions.py` defines a `waffle_shop` job. Run `sqb` commands from `waffle_shop/` and the orchestrator from the top-level directory. Agent skill files are installed in `waffle_shop/`, where `sqb skills` and stale-skill checks maintain them.

The `python_nodes` template instead creates a focused Python-nodes project:

- `python/nodes/assets/orders_inputs.py`, `python/nodes/loaders/orders.py`, `python/nodes/assets/orders_export.py`, `python/nodes/checks/orders_export.py`
- A `fact_orders` SQL model over a managed `raw_orders` source
- Examples of the SQL boundary, `ctx.relation(model(...))`, soft-skip fan-in, and a Python check

## Examples

```bash
# Default waffle shop
sqb playground waffle-shop
cd waffle-shop
sqb build

# With Dagster integration
sqb playground --template dagster
cd sqlbuild-playground
uv pip install 'sqlbuild[dagster]'
dagster dev -f definitions.py

# With Rivers integration
sqb playground --template rivers
cd sqlbuild-playground
uv pip install 'sqlbuild[rivers]'
uv run rivers dev definitions

# With Python nodes
sqb playground --template python_nodes
cd sqlbuild-playground
sqb build --select +fact_orders --select +orders_export
sqb check --select +check_orders_export
```

## Notes

- The target directory must not already exist
- DuckDB is included as a core dependency - no extra installation needed
- The local DuckDB database file is created on the first build
- The Dagster template uses `prepare_if_dev()` to auto-generate the DAG artifact when Dagster starts in dev mode
