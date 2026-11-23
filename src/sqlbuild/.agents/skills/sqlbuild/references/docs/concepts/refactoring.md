<!-- generated-by: sqlbuild skills -->

# Refactoring

> Rename and move models and columns, keep their warehouse history, and keep old names working.

Online: https://sqlbuild.com/docs/concepts/refactoring/

## Contents

- Rename a model
- Rename a column
- Old names keep working
- Check the impact first
- When a rename is refused

SQLBuild lets you rename a model, move it to another folder, or rename one of its columns as a single
verified change. The compiler finds every reference, the edited project is compiled before anything is
written, and the next build keeps the existing table's history instead of rebuilding it.

## Rename a model

```bash
sqb rename hourly_order_activity order_activity_by_hour
```

```text
Rename model  hourly_order_activity -> order_activity_by_hour
├── sql/models/marts/daily_activity_rollup.sql
│   ├── 10:5    header    hourly_order_activity  ->  order_activity_by_hour
│   └── 34:13   reference __ref("hourly_order_activity")  ->  __ref("order_activity_by_hour")
├── sql/models/marts/hourly_activity_with_daily_context.sql
│   ├── 10:5    header    hourly_order_activity  ->  order_activity_by_hour
│   └── 36:15   reference __ref("hourly_order_activity")  ->  __ref("order_activity_by_hour")
└── sql/models/marts/order_activity_by_hour.sql  (moved from sql/models/marts/hourly_order_activity.sql)
    └── 1:8     migration + migrate_from hourly_order_activity
Compiled: ok, 3 files changed
```

The plan then shows a migration instead of a first run:

```bash
sqb plan
```

```text
Migrations (1)
└── order_activity_by_hour  migrate  dev.hourly_order_activity -> dev.order_activity_by_hour
    ├── compatibility  compatible
    ├── transfer  physical copy, promote by transactional rename
    └── old name  dev.hourly_order_activity
        ├── view  until 2026-11-06
        └── old table  archived
```

On the next `sqb build`, the incremental model's existing data moves to the new name and the model
carries on from where it was. Nothing is backfilled.

`sqb mv` does the same for a move to another folder, optionally with a new name:

```bash
sqb mv fact_orders sql/models/reporting/
sqb mv fact_orders sql/models/reporting/order_facts.sql
```

A move also takes the model's private macros, enums, constants and other scoped declarations with it,
following the same placement rules as `sqb compile`. See [Declarations and scopes](declaration-scopes.md).

## Rename a column

```bash
sqb rename stg_customers.email email_address --dry-run
```

```text
Rename column  stg_customers.email -> email_address
├── sql/models/marts/dim_customers.sql
│   ├── 16:5    column    email  ->  email_address
│   ├── 16:10   column    + AS email
│   └── 23:54   column    email  ->  email_address
└── sql/models/staging/stg_customers.sql
    ├── 7:5     header    email  ->  email_address
    └── 15:8    column    + AS email_address
Dry run: compiles, 2 files would change; nothing written
```

Edits follow the columns each query actually binds, so a same-named column of another table is left
alone. Downstream models keep their own output names unless you pass `--cascade`. An incremental or
snapshot model renames the column in place on the next build. See
[Column migrations](models/column-migrations.md).

## Old names keep working

After a model moves to a new name, the old name becomes a view over the new relation for 30 days by
default, with the old relation's grants copied to it. Queries, dashboards and other tools that read
the old name keep working while you update them. After that, [`sqb janitor`](../cli/janitor.md) drops
the view. See [Model migrations](models/migrations.md#old-names) for the settings.

## Check the impact first

- [`sqb lineage`](../cli/lineage.md) shows which models and columns depend on what you are changing.
- [`sqb scope`](../cli/scope.md) shows which declarations a model uses and what a move would break.
- `--dry-run` on `sqb rename` and `sqb mv` prints every edit and checks the edited project compiles,
  without writing files.

## When a rename is refused

If any location cannot be rewritten safely, the command lists it and writes nothing:

```text
Rename column  stg_payments.amount_cents -> amount_minor
Cannot rename column:stg_payments.amount_cents automatically
└── sql/models/marts/daily_revenue.sql:21:3  column amount_cents is referenced in SQL a macro generates; update the macro call by hand
Edit the listed locations, then run the command again.
Refused; no files changed
```

The full list of refusals and every flag is in the [`sqb rename` and `sqb mv` reference](../cli/rename.md).
