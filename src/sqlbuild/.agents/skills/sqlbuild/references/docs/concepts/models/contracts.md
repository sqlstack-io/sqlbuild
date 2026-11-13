<!-- generated-by: sqlbuild skills -->

# Contracts

> Say which columns a model promises to output, and when SQLBuild checks them.

Online: https://sqlbuild.com/docs/concepts/models/contracts/

## Contents

- What fails and when
- Starting from an existing warehouse
- Models that read an enforced contract
- Details

A contract is the list of columns a model promises to output. You write it in the model's
`columns (...)`, and SQLBuild checks the model's SQL against it.

```sql
MODEL (
  description "One row per order",
  materialized table,
  contract enforced,
  columns (
    order_id (type INTEGER, nullable false),
    customer_id (type INTEGER, nullable false),
    amount_cents (type INTEGER),
    status (type VARCHAR),
  ),
);
```

How strictly the list is checked depends on the model's `contract` setting, which can also come
from `[defaults]` or `[path_defaults]`:

| `contract` setting | Listed columns | Extra columns | Checked |
|--------------------|----------------|---------------|---------|
| not set (default) | must be in the output | allowed | at compile time |
| `enforced` | must be in the output, with their declared types | not allowed | at compile time, and at runtime where the [table below](#runtime) says so |
| `none` | documentation and audit targets only | allowed | declared types only |

To make models without a `contract` setting treat their columns as documentation and audit targets
only, set `[settings] check_declared_columns = false`.

Do this when column lists may not match the SQL, such as lists carried over from dbt YAML during a
migration. Models that need a contract then opt in with `contract enforced`.

Declared types are checked and cast whatever the contract; see
[Type Enforcement](type-enforcement.md).

## What fails and when

### Compile time

When SQLBuild can work out the model's output columns, these fail unless the model's columns are
documentation only:

- A listed column is missing from the output (`K001`).
- A `nullable false` column can be null (`K004`).
- With `contract enforced` only: an extra column (`K005`), or a `unique_key`, `cursor`, `updated_at`
  or `check_columns` that names an unlisted column.

A column whose type doesn't match its declared type fails too (`K002`), whatever the contract. If
SQLBuild can't work out the output, these checks are skipped. `contract enforced` needs at least one
listed column (`K006`).

### Runtime

With `contract enforced`, SQLBuild also checks the built output's column names and types before it
replaces anything:

| Materialization | Runtime check |
|-----------------|---------------|
| Table (staged, the default) | Before the new table replaces the old one |
| Table or non-microbatch incremental with `table_promotion_mode = "immediate"` | Not allowed: a `K011` compile error. Use staged promotion |
| Incremental | On each delta, before it is written |
| Microbatch incremental | None; compile time only |
| Snapshot | On each delta |
| View, custom | None; compile time only |

Runtime checks don't look at nulls. Add a `not_null` audit to check for nulls at runtime.

## Starting from an existing warehouse

To compare your declarations with tables that already exist, or write declarations from them, use
[`sqb contract`](../../cli/contract.md). It reads the warehouse and never changes it:

```bash
sqb contract diff --from prod --select tag:commerce
sqb contract generate --from prod --select tag:commerce --write
```

`generate` fills in missing types and columns but never adds `contract enforced`; that stays your
decision. See [`sqb contract`](../../cli/contract.md) for `--overwrite` and what generation keeps.

## Models that read an enforced contract

When a model reads a model or source with `contract enforced`, SQLBuild knows its exact columns. A
reference to a column it doesn't have, or to an ambiguous one, fails at compile time, wherever it
appears: the select list, a join, a filter, grouping, a window or `ORDER BY`. When an upstream's columns aren't fully known,
nothing is reported. To see where a model uses each upstream column, run
`sqb lineage <model> --include-uses --format json`.

## Details

- **Reusable schemas.** A contract covers the model's full column list: the columns of its
  [reusable schema](schemas.md) plus any it adds. With `contract enforced`, the
  output must have exactly those.
- **Column order.** Contracts match columns by name; output order isn't checked.
- **Enum columns.** A column typed with an enum, such as `fulfillment_method (type fulfillment_method)`,
  is a `VARCHAR` or `INTEGER`. With `contract enforced`, SQLBuild adds an `accepted_values` audit for
  its members. See [Enum Model Contracts](../enums/model-contracts.md).
- **Schema changes.** `snapshot_schema_change append_new_columns` can't be combined with
  `contract enforced`, because an appended column would break the exact list.
- **Requiring contracts.** A custom [Rule](../rules/custom-rules/overview.md) can require
  `contract enforced` for chosen models.
