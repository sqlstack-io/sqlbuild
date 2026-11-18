<!-- generated-by: sqlbuild skills -->

# sqb plan

> Preview what SQLBuild will do before executing.

Online: https://sqlbuild.com/docs/cli/plan/

## Contents

- Usage
- Flags
- Output
- Timings
- Exit codes
- Missing upstream dependencies
- Missing source tables

Shows the execution plan without making any changes. Useful for inspecting change detection, backfill policies, and selector scope before building.

## Usage

```bash
sqb plan [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined. `--project-dir <path>`, `--no-color`, and `--debug` can go before or after the command name.

## Flags

| Flag | Description |
|------|-------------|
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias) |
| `--no-cache` | Bypass the reusable compile-analysis cache for this invocation |
| `--no-python` | Exclude read-side Python tasks and assets from the plan |
| `--defer-to` | Resolve unselected model references against another target |
| `--plan-from` | Read warehouse state `live`, or from the target's `synced_state` checked against the warehouse first; overrides the target's `plan_from`. See [Planning from synced state](../concepts/project-configuration.md#planning-from-synced-state) |
| `--as` | Compile for another target and show its plan, including [model migration](../concepts/models/migrations.md) decisions, using the current connection. Inspection only. |
| `--json` | Output the plan as JSON |
| `--timings` | After the plan, print how long each planning step took; see [Timings](#timings) |
| `--ignore-audit-failures`, `--no-ignore-audit-failures` | Plan the build `sqb build` would run with the same flag; a plan whose audit failures are ignored starts with `Audit failures won't stop this build`. See [Ignoring audit failures](../concepts/audits.md#ignoring-audit-failures) |
| `--selection-diagnostics` | Warn when a selected model will build on changed upstream models that are not selected; see [Selection and staleness](../concepts/planning/selection-and-staleness.md) |
| `--full-refresh` | Plan a full rebuild for selected models unless a model sets `full_refresh false`; `full_refresh true` forces a model without this flag |
| `--changed` | Show what `sqb build --changed` would run, which nodes it holds back and why; takes `--changed=definition,source_data` and `--reach` like [`sqb build`](build.md#building-only-what-changed). In `--json`, the `changed` object lists `runs`, `held_back` and `unchanged` |
| `--start-cursor` | Override the start cursor, typed from the selected incremental models' cursor type; fails with `S303` if the selection mixes timestamp and integer cursors. With no cursor model selected, an integer value is read as an integer cursor and anything else as an ISO timestamp |
| `--end-cursor` | Override the end cursor, typed the same way as `--start-cursor` |
| `--start-cursor-ts` | Override start cursor for timestamp incremental models (ISO format) |
| `--end-cursor-ts` | Override end cursor for timestamp incremental models (ISO format) |
| `--start-cursor-int` | Override start cursor for integer incremental models |
| `--end-cursor-int` | Override end cursor for integer incremental models |
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Select specific models |
| `--exclude` | Exclude specific models |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `build` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Output

First run:

```
Plan ready  15 selected, 2 sources to load

Sources to load (2)
├── raw__customers                     table
└── raw__orders                        delete_insert (cursor: ordered_at)

Changed functions (2)
├── table_fn__customer_orders          table function
│   └── reason  first run
└── udf__is_completed_order            sql udf
    └── reason  first run

First run (12)
├── stg_customers                      view
├── stg_payments                       view
├── stg_orders                         view
├── daily_order_partitioned            partition_tracked (custom)
├── daily_revenue                      table
├── dim_customers                      table
├── fact_orders                        table
├── customer_status_snapshot           merge (timestamp)
│   ├── cursor  last_ordered_at (timestamp)
│   └── bounds  computed at run time, once model fact_orders is up to date
├── hourly_order_activity              delete_insert (timestamp, microbatch)
│   ├── cursor  activity_hour (timestamp)
│   ├── mode  microbatch
    ...
└── order_status_index                 delete_insert (integer)
    ├── cursor  order_id (integer)
    └── bounds  computed at run time, once model fact_orders is up to date

Seeds (1)
└── waffle_types  (first_run)

✓ Plan complete  15 selected, 2 sources to load
```

Steady state:

```
Plan ready  15 selected, 2 sources to load

Sources to load (2)
├── raw__customers                     table
└── raw__orders                        delete_insert (cursor: ordered_at)

Models (12)
├── stg_customers                      view
├── stg_payments                       view
...
└── order_status_index                 delete_insert (integer)
    ├── cursor  order_id (integer)
    └── bounds  computed at run time, once model fact_orders is up to date

Functions (2)
├── table_fn__customer_orders          table function
└── udf__is_completed_order            sql udf

Seeds (1)
└── waffle_types  (current)

✓ Plan complete  15 selected, 2 sources to load
```

When query, function, or schema changes are detected, the plan lists each affected model under its own change with its backfill action. A model that calls a changed function directly appears under `Function changed` with `cause: function <name> changed`; models further downstream continue forward. See [Replay decisions](../concepts/planning/replay-decisions.md).

In `sqb plan --json`, each model's `reason` is its own change. A direct caller of a changed function has `"reason": "function_changed"` and a `changed_functions` list, and a renamed model whose query also changed has `"reason": "renamed"` and `"query_changed": true`. A changed model also carries the detail the text plan prints under it: `query_diff` when its query changed and the previous query is known, and `config_diff` when its config changed, each as unified diff lines, and `schema_findings` for added, removed, or retyped columns.

Warnings and maintenance notices, such as `migrate_from can be removed`, are listed in a `Warnings` section before the final `Plan complete` line. `sqb plan --json` output is unchanged.

Work that runs before models build is listed first. [Model migrations](../concepts/models/migrations.md) appear under `Migrations`, and in-place [column renames](../concepts/models/column-migrations.md) under `Column migrations`:

```
Column migrations (1)
└── fact_orders  migrate columns
    └── amount -> revenue  rename in place
```

When a removed column and an added column look related but are not an exact rename, the schema diff shows a hint such as `similar to amount; if this is a rename, add revenue (migrate_from amount)` until the change is built.

## Timings

`--timings` prints how long each planning step took, in pipeline order, after the plan:

```
Plan timings                   seconds  share
  Discovering the project          1.2    10%
  Compiling the project            2.1    17%
  Evaluating rules                 0.6     5%
  Connecting to the warehouse      0.3     2%
  Reading warehouse state          3.3    26%
  Detecting changes                2.4    19%
  Checking source freshness        0.2     2%
  Building plan entries            1.6    13%
  Preparing test fixtures          0.1     1%
  Assembling the plan              0.4     3%
  Writing the plan                 0.3     2%
  Other                            0.1     1%
  Total                           12.6
```

`Reading build history` appears with `--changed`. A plan a location makes for the web UI from its synced warehouse state uses the project it already discovered, compiled and checked against the rules, and opens no connection, so those steps do not appear there. `Preparing test fixtures` counts only the time the plan waits for them, because they are prepared alongside the other steps. `Other` is the time no step covers. A plan a location makes for the web UI records the same steps in its plan document.

## Exit codes

`sqb plan` exits `0` when the plan is ready, including plans that only have warnings. It exits `1`
when the plan has an error-severity finding that `sqb build` would refuse, such as an `M114` name
claim; the final line then reads `✗ Plan failed with errors`. `sqb plan --json` uses the same exit
codes, so CI can gate on either form.

When `sqb plan --json` fails before it can produce a plan, for example on an unknown selector, it
still exits `1` and prints its coded error on stderr, and prints the error as JSON on stdout:
`{"error": {"code": "S007", "message": "...", "help": null}}`.

Release note: earlier versions printed `Plan complete with errors` and exited `0` for such plans.

## Missing upstream dependencies

Planning a scoped selection whose upstream inputs were never built in the target fails with `S301` rather than producing a plan that would break at build time:

```
error[S301]: 3 upstream models and seeds this build reads don't exist in target 'dev'
  stg_orders
  stg_payments
  waffle_types

  If they exist in prod:
    read them from it for this run:  --defer-to prod
    copy them into this target:      sqb clone --from prod -s stg_orders stg_payments waffle_types
  Or build them too:                 add + before your selection
```

The error names the first five missing resources; `sqb plan --json` lists all of them in
`error.missing`. Each fix fits a different case:

| Fix | When to use it |
| --- | --- |
| `--defer-to <origin>` | Read the missing resources from another target for this run only. |
| `sqb clone --from <origin> -s ...` | Copy them into this target once. The command lists every missing name, so you can run it as printed. |
| `+` before the selection, such as `sqb build --select +fact_orders` | Build the upstream chain along with the model. |

The error offers an origin only for targets with `clone.allow_as_clone_origin = true`, and only
when the current target has `clone.allow_as_clone_destination = true`. It does not check that the
resources exist in the origin.

The check also covers resources that an attached audit on a selected resource reads, even when the
model SQL does not read them. Selecting `+fact_orders` follows data lineage only, so it does not
pull those resources in. When a missing resource is needed only by an audit, the error names the
audit, its target, and the resource:

```
error[S301]: cannot build selected scope: audit 'relationships' on 'stg_orders' reads 'waffle_types', which is not selected and does not exist in the warehouse; select it or build it first
```

Load or build the resource first (for a seed, `sqb seed`), or add it to the selection. When both
kinds are missing, the error lists the missing upstream resources and then the audit reads.

## Missing source tables

When a selected model reads an external source whose table does not exist in the warehouse,
planning fails with `S405` before anything runs. The error names each missing source, its table,
the selected models that read it, and the source declaration file and line. See
[Source freshness: Missing source tables](../concepts/planning/source-freshness.md#missing-source-tables).
