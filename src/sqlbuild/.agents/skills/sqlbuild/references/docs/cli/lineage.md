<!-- generated-by: sqlbuild skills -->

# sqb lineage

> Explore model and column-level dependency graphs from the command line.

Online: https://sqlbuild.com/docs/cli/lineage/

## Contents

- Usage
- Flags
- Model lineage
- Column lineage
- Examples
- Depth limiting

Inspect upstream and downstream dependencies for any model, source, seed, or function in your project. Supports both model-level lineage (dependency graph) and column-level lineage (tracing individual columns through transformations). Outputs as a tree, edge list, or structured JSON.

`sqb compile` computes lineage for validation and includes per-model summaries in its JSON report. Use this command when you need to inspect or export the actual lineage graph.

## Usage

```bash
# Model lineage: dependency graph around a single resource
sqb lineage <target> [flags]

# Column lineage: trace a specific column
sqb lineage <model>.<column> [flags]

# Selector mode: lineage for a selected scope
sqb lineage --select <selector> [flags]
```

Exactly one of a positional target or `--select` is required.

## Flags

| Flag | Description |
|------|-------------|
| `--direction` | `upstream` (default), `downstream`, or `both`. `both` is only available for model lineage. |
| `--depth` | How many hops to traverse. An integer or `all` (default: `all`). |
| `--format` | Output format: `tree` (default), `list`, or `json`. |
| `--json` | Shorthand for `--format json`. |
| `--mode` | Column lineage analysis mode: `rich` (default) or `fast`. |
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias). |
| `--select`, `-s` | Select resources using the same selector syntax as `sqb compile`, `sqb plan` and `sqb build`, for example `path:models/marts`, `tag:finance` or `stg_*`. |
| `--exclude` | Exclude resources from the selection. |

## Model lineage

When the target is a plain resource name, lineage shows the model-level dependency graph.

### Tree

The default. Shows an indented dependency tree with resource types and file paths:

```bash
sqb lineage fact_orders --direction both
```

```
Lineage  model  fact_orders  models/marts/fact_orders.sql  both
upstream
├── model  stg_orders  models/staging/stg_orders.sql
│   └── source  raw__orders  sources/raw.yml
├── model  stg_payments  models/staging/stg_payments.sql
│   └── source  raw__payments  sources/raw.yml
├── seed  waffle_types  seeds/waffle_types.csv
└── udf  udf__is_completed_order
downstream
├── model  customer_status_snapshot  models/intermediate/customer_status_snapshot.sql
├── model  hourly_order_activity  models/marts/hourly_order_activity.sql
│   ├── model  daily_activity_rollup  models/marts/daily_activity_rollup.sql
│   │   └── model  hourly_activity_with_daily_context  models/marts/hourly_activity_with_daily_context.sql
│   └── model  hourly_activity_with_daily_context  models/marts/hourly_activity_with_daily_context.sql (already shown)
├── model  order_status_index  models/intermediate/order_status_index.sql
└── table_fn  table_fn__customer_orders
```

Cycles and repeated nodes are annotated with "(already shown)" to avoid infinite recursion.

### List

An edge list showing each dependency as a directed pair:

```bash
sqb lineage fact_orders --format list
```

```
udf:udf__is_completed_order -> model:fact_orders
model:stg_orders            -> model:fact_orders
seed:waffle_types           -> model:fact_orders
model:stg_payments          -> model:fact_orders
source:raw__orders          -> model:stg_orders
source:raw__payments        -> model:stg_payments
```

### JSON

Structured output with nodes, edges, and metadata:

```bash
sqb lineage fact_orders --json
```

```json
{
  "nodes": [
    {
      "id": "model:fact_orders",
      "name": "fact_orders",
      "resource_type": "model",
      "relative_path": "models/marts/fact_orders.sql",
      "qualified_name": "dev.fact_orders"
    },
    {
      "id": "source:raw__orders",
      "name": "raw__orders",
      "resource_type": "source",
      "relative_path": "sources/raw.yml"
    }
  ],
  "edges": [
    {"from": "source:raw__orders", "to": "model:stg_orders"},
    {"from": "model:stg_orders", "to": "model:fact_orders"}
  ],
  "focus": ["model:fact_orders"],
  "direction": "upstream"
}
```

## Column lineage

When the target uses `model.column` syntax, lineage traces the specific column through upstream or downstream transformations. Each edge is annotated with a transform type (`direct`, `expression`, `aggregation`, `cast`, `star`, `constant`) and a confidence level. See [Column Lineage](../concepts/column-lineage.md) for a full explanation of transform types, confidence levels, and analysis modes.

```bash
# Where does fact_orders.total_cents come from?
sqb lineage fact_orders.total_cents

# What consumes fact_orders.order_id downstream?
sqb lineage fact_orders.order_id --direction downstream
```

Column lineage supports `upstream` and `downstream` directions (not `both`). The `--mode` flag selects the analysis mode: `rich` (default, full SQL analysis) or `fast` (lightweight, faster on large projects).

### Tree

```bash
sqb lineage fact_orders.total_cents
```

```
Column trace  fact_orders.total_cents  upstream

└── stg_payments.amount_cents (expression)
    └── raw__payments.amount_cents (direct)
```

### List

```bash
sqb lineage fact_orders.total_cents --format list
```

```
Column dependencies

stg_payments.amount_cents  -> fact_orders.total_cents   expression
raw__payments.amount_cents -> stg_payments.amount_cents direct
```

### JSON

```bash
sqb lineage fact_orders.total_cents --json
```

```json
{
  "target": {
    "resource_type": "model",
    "resource_name": "fact_orders",
    "column_name": "total_cents"
  },
  "direction": "upstream",
  "metadata": {
    "mode": "rich",
    "max_depth": null,
    "analyzed_models": 5,
    "truncated": false
  },
  "trace": [
    {
      "source": {
        "resource_type": "model",
        "resource_name": "stg_payments",
        "column_name": "amount_cents"
      },
      "target": {
        "resource_type": "model",
        "resource_name": "fact_orders",
        "column_name": "total_cents"
      },
      "transform": "expression",
      "confidence": "high"
    }
  ]
}
```

## Examples

```bash
# Upstream dependencies of a model (default)
sqb lineage fact_orders

# Downstream dependents
sqb lineage fact_orders --direction downstream

# Both directions, limited to 1 hop
sqb lineage fact_orders --direction both --depth 1

# Column lineage: trace a specific column upstream
sqb lineage fact_orders.total_cents

# Column lineage: trace downstream consumers
sqb lineage fact_orders.order_id --direction downstream

# Column lineage with depth limit
sqb lineage fact_orders.total_cents --depth 1

# Lineage for all models in a path
sqb lineage --select path:models/marts

# Lineage between two models (path-between)
sqb lineage --select "stg_orders~daily_activity_rollup"

# Upstream expansion from a model
sqb lineage --select "+fact_orders"

# JSON output for programmatic consumption
sqb lineage fact_orders --json --direction upstream
```

## Depth limiting

`--depth` controls how many hops from the focus node(s) to include. In selector mode without `--direction`, `--depth` requires name (including glob patterns), source, seed, or path-between selectors - it cannot be combined with tag or path selectors or comma-intersection selectors.

```bash
# Only immediate parents
sqb lineage fact_orders --depth 1

# Parents and grandparents
sqb lineage fact_orders --depth 2

# Only direct column dependencies
sqb lineage fact_orders.total_cents --depth 1
```
