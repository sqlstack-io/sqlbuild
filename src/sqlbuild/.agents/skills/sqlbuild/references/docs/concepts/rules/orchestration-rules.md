<!-- generated-by: sqlbuild skills -->

# Orchestration Rules

> Keep job and sensor selections from listing nodes one by one.

Online: https://sqlbuild.com/docs/concepts/rules/orchestration-rules/

`SQBRORCHESTRATION101` reports a job or sensor whose `select` names more than three nodes one by one.
Select a pipeline by its folder or a tag instead, so nodes added to it later are built too.

```toml
# orchestration/jobs/nightly_orders_job.toml
name = "nightly_orders_job"
description = "Orders and their daily totals"
select = "stg_orders stg_payments int_orders order_totals"
```

```text
error[SQBRORCHESTRATION101]: job 'nightly_orders_job' names 4 nodes in its selection (limit 3)
  --> orchestration/jobs/nightly_orders_job.toml:3:10
  = help: If these nodes are one pipeline, select its folder or a tag. If the job deliberately targets exactly these nodes, add an exact rule exception for this file with a reason.
```

```toml
select = "sql/models/orders"
```

The Rule is selected with `SQBR` or `SQBRORCHESTRATION`, and with its exact code.

## What counts as a named node

The Rule reads the `select` of every [job](../orchestration.md#declare-a-job) and the
`select` of each `node_event`, `check_event`, `source_new_data` and `source_stale` condition of a
[sensor](../orchestration.md#react-with-a-sensor). It counts the distinct
[selector](../selectors.md) atoms that each name one node.

| Atom | Counts |
| --- | --- |
| A bare name, such as `order_totals` | Yes |
| A typed name, such as `source:raw_orders` or `asset:refund_review` | Yes |
| Each name in an intersection, such as `order_totals,tag:finance` | The name only |
| `tag:`, a folder path, `job:`, a job or job group name | No |
| A name with `+` before or after it | No |
| A glob, such as `fct_*` | No |

A name written twice counts once. A job's `exclude` is not checked.

## Change the limit

Set `max_named_selectors` to the most nodes one selection may name. The default is 3, and 0 rejects
every named node.

```toml
[rules.rule_options.SQBRORCHESTRATION101]
max_named_selectors = 5
```

## Keep a deliberate list

A repair job that targets exactly a few nodes is a deliberate exception. Record it for the job's
file with a reason:

```toml
[[rules.rule_exceptions]]
rule = "SQBRORCHESTRATION101"
path = "orchestration/jobs/repair_orders_job.toml"
reason = "Rebuilds the four order models after a late payments load."
```

See [Findings and exceptions](findings-and-exceptions.md#exact-exceptions).

## Sensor conditions the Rule cannot count

The Rule reads a sensor's `when` without running the module. It follows a name bound at module
level, in the sensor's file or in a project module the sensor imports it from, to the condition
call that name holds.

| `when` value | Result |
| --- | --- |
| A selecting condition whose `select` is a quoted string | Counted |
| A `platform_event` or `run_event` condition, written inline or held in a name | Not checked |
| A selecting condition whose `select` is a variable or an expression | Reported |
| A call to any other function, such as a helper that builds the condition | Reported |
| A name imported from outside the project, or not bound at module level | Reported |
| Any other expression, such as `module.NAME` | Reported |

A reported condition names what the Rule could not resolve:

```text
error[SQBRORCHESTRATION101]: sensor 'on_orders_built_sensor' has a condition the rule cannot resolve: `landed_after(...)` is a function call, so the nodes it selects cannot be counted
```

To keep a condition the Rule cannot count, add an exact rule exception for the sensor's file with a
reason.
