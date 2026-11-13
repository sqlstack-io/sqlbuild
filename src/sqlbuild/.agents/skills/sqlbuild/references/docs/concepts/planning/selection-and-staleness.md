<!-- generated-by: sqlbuild skills -->

# Selection and staleness

> How selection interacts with change detection, and the stale warnings that stop silent partial rebuilds.

Online: https://sqlbuild.com/docs/concepts/planning/selection-and-staleness/

When you select part of the project with `--select`, SQLBuild runs only what you selected, but it
still checks the whole graph. The case it watches for: a selected model whose upstream changed but
isn't selected, so the model would build on stale input.

## Only selected resources appear in the plan

The plan lists only the selected resources, not the upstreams they read. SQLBuild still looks at
those upstreams: a selected model runs when an upstream changed. `--full-refresh` also applies only
to the selected models.

## Stale upstreams

`sqb plan` and `sqb build` run every selected model, even when an unselected upstream changed. Pass
`--selection-diagnostics` to list those stale upstreams. After changing `stg_orders`,
`sqb plan --select fact_orders --selection-diagnostics` shows:

```
Warnings (1)
└── fact_orders
    ├── selected model 'fact_orders' will build on 1 stale upstream(s) not selected for rebuild:
    ├──     - stg_orders
    └──     rebuild the closure to refresh them: --select +fact_orders
```

To include the changed upstreams, select the closure: `--select +fact_orders` builds `stg_orders`
and everything between it and `fact_orders`.

A changed seed outside the selection is treated the same way as a changed model.

## With dbt

`sqb dbt plan`, `run` and `build` are stricter. A selected model whose upstream changed outside the
selection is not rebuilt: it stays current, and the plan warns, naming the changed upstreams.

```bash
sqb dbt plan --select agg_daily_revenue
```

```
Plan ready (1 selected resources)

dbt (1 selected resources)
  planned models: 0 run, 1 current, 0 blocked

  Model plan
    Current (1)
      model.jaffle_analytics.agg_daily_revenue  no change

Warnings (1)
  - selected dbt model 'agg_daily_revenue' is stale: upstream stg_orders changed
    but will not be rebuilt or is stale; rebuild with a closure selector
    (e.g. +model) to incorporate it
```

Select the closure to pull in the changed upstreams and rebuild:

```bash
sqb dbt plan --select +agg_daily_revenue
```

```
Plan ready (8 selected resources)
  planned models: 5 run, 1 current, 0 blocked

  Model plan
    Checksum changed (1)
      model.jaffle_analytics.stg_orders         checksum changed
    Upstream changed (4)
      model.jaffle_analytics.agg_daily_revenue  upstream changed
      model.jaffle_analytics.fct_orders         upstream changed
      model.jaffle_analytics.int_order_payments upstream changed
      model.jaffle_analytics.stg_order_statuses upstream changed
```
