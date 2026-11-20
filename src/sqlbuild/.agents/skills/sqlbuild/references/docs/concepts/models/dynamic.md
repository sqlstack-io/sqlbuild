<!-- generated-by: sqlbuild skills -->

# Dynamic models

> Let the warehouse keep a model up to date on a schedule, paused safely while builds run.

Online: https://sqlbuild.com/docs/concepts/models/dynamic/

A dynamic model is one the warehouse keeps up to date on a schedule you set, between builds. A
build creates it, changes it when its definition changes, and otherwise leaves the refreshing to
the warehouse; on Snowflake it is a dynamic table.

```sql
MODEL (
  description "One row per customer per order date",
  materialized dynamic,
  refresh (lag 1h),
);

SELECT customer_id, CAST(ordered_at AS DATE) AS order_date, SUM(amount) AS revenue
FROM __ref("stg_orders")
GROUP BY customer_id, CAST(ordered_at AS DATE)
```

Only Snowflake supports dynamic models. See [Snowflake](../adapters/snowflake.md#dynamic-models)
for the grants and change tracking they need.

## Settings

| Field | Default | Description |
|-------|---------|-------------|
| `refresh (lag <duration>)` | none | How far behind its inputs the model may fall: a fixed duration such as `5m`, `1h` or `1d`, at least `1m` on Snowflake and no shorter than the lag of a dynamic model it reads. Without it, the model refreshes when a dynamic model that reads it refreshes |
| `refresh_mode` | `incremental` | `incremental`, `full`, `auto` or `adaptive`. Snowflake rejects `incremental` for a query it can't refresh incrementally; use `full` for that query |
| `refresh_warehouse` | the target's build warehouse | Warehouse the scheduled refreshes run on; a job's `warehouse` never sets it |
| `refresh_on_build` | `false` | Refresh the model in every build that selects it, not only on its schedule |

## What the plan shows

| Change | Plan | What the build does |
|--------|------|---------------------|
| First build, or the dynamic table is missing | `create` | Creates it and fills it |
| Query or `refresh_mode` changed | `recreate, full recompute` | Replaces it; every row is computed again |
| `refresh` lag or `refresh_warehouse` differs from the warehouse | `change refresh settings in place` | Alters the schedule; rows stay |
| Nothing | `dynamic` | Leaves it alone |

A model the plan keeps can carry one more line:

- `recomputes in full: stg_orders is rebuilt`: the build replaces a model it reads, so its next refresh computes every row again.
- `paused: scheduled refreshes are suspended`: Snowflake lists it as suspended, so it is not refreshing.

## During a build

A build pauses the dynamic models it changes, and those that read a model it rebuilds, directly
or through views, so they never refresh from half-built inputs. It resumes them when it ends,
whether it succeeds or fails.

- Dynamic tables outside the project that read a paused model pause too, until the build resumes it.
- A model someone suspended before the build stays suspended, and the build warns about it.
- A suspend made by someone else during the build is undone when the build resumes the model.
- A kept model refreshes during the build when a model built later in the same build reads it.
- `refresh_on_build` refreshes the model in every build that selects it.
- A target with `pause_dynamic = true` leaves its dynamic models suspended between builds; see [Targets](../project-configuration.md#targets).

## When a build dies

- **One build at a time.** A build that needs a dynamic model another build holds fails with
  `A718`, naming that run. Wait for it to finish. Two builds that start at the same moment can
  both fail this way; run one of them again.
- **A stopped build leaves its models suspended.** A build killed before it resumes them, for
  example by `SIGKILL`, can't resume them itself.
- **The location's sync resumes them.** Once the stopped build has been silent for
  `[dynamic] heartbeat_grace`, 30 minutes by default, the next completed sync of a served location
  resumes its models and closes its hold, and the location's log says which. A sync never leaves
  a model running while another build holds it, and it leaves models suspended outside SQLBuild
  alone. A build that takes a model over while a sync is resuming it waits for the sync, at most
  2 minutes, then pauses the model again if the sync resumed it. A model it fails to resume, for example for a missing privilege, keeps its hold and is retried
  on the next sync. See
  [Dynamic](../project-configuration.md#dynamic) and
  [location syncs](../../cli/server.md#sync-from-the-warehouse-now).
- **Or the next build does.** Without a served location, the next build that pauses dynamic
  models on the target resumes them after the same grace.
- **Taking over at once.** Run `sqb build --ignore-dynamic-grace` to take the models over
  without waiting. It needs `[dynamic] allow_ignore_grace = true`; without it the flag fails
  with `A719`. You can also resume the models in Snowflake by hand.

## Limitations

- A resume that takes longer than its 2-minute claim can still start a model a build has just paused: the last `ALTER` of a sync killed at the limit, or a build's resume of another model on a queued warehouse.
- Unit tests and scenarios treat a dynamic model as a table.
- On a target with `schema = "preserve"`, builds see each other's pauses only when their dynamic models share a schema.
