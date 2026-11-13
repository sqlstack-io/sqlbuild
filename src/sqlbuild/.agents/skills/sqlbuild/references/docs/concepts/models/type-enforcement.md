<!-- generated-by: sqlbuild skills -->

# Type Enforcement

> Understand declared model types, static checks, and runtime casting.

Online: https://sqlbuild.com/docs/concepts/models/type-enforcement/

Declare a column's type and SQLBuild checks it at compile time and, on staged tables and incremental models, casts the column to it at runtime. You don't switch it on: any declared type enables it.

## Enabling it

For a SQL model, declaring a type on any inline or reusable-schema column enables type enforcement automatically:

```sql
MODEL (
  description "One row per order",
  materialized table,
  columns (
    order_id (type INTEGER),
    ordered_at (type TIMESTAMP),
    source_system (),
  ),
);
```

There is no `MODEL(type_enforcement ...)` switch. In this example, `order_id` and `ordered_at` are typed; `source_system` remains untyped and passes through unchanged.

Sources have related behavior but different configuration. A source can explicitly set `type_enforcement: false`; see [Sources](../sources.md#type-enforcement).

## Compile-time checks

When SQL analysis can infer an output type, SQLBuild compares it with the declared type using adapter-aware normalization. A proven mismatch is a compile error when type enforcement is active. If SQLBuild identifies an output column but cannot prove its expression type, it reports an unproven-type warning rather than guessing. If the entire output shape cannot be inferred, static type checks and warnings cannot run.

Static analysis does not rewrite the authored query. It determines whether the inferred output is compatible before warehouse execution.

## Runtime casting

Runtime enforcement inspects a staged relation. If a typed output column does not already have its declared type, SQLBuild rebuilds the staging or delta relation with an explicit cast:

```sql
CAST(order_id AS INTEGER) AS order_id
```

Only columns with declared types are cast. Untyped output columns are preserved. If a cast fails, the model fails during the type-enforcement phase before the staged table is promoted or the incremental delta is applied.

Runtime cast support currently depends on materialization:

| Materialization path | Framework runtime casts |
|----------------------|-------------------------|
| Full table with staged promotion | Yes, on the staging table |
| Non-microbatch incremental | Yes, on the staged delta |
| Microbatch incremental | Yes, on each staged batch |
| View | No; the view query defines the warehouse output type |
| Snapshot | No framework cast reconstruction |
| Custom materialization | Owned by the custom materialization |
| Full table with immediate promotion | Rejected when type enforcement is required |

Staged promotion is the default table mode. If a project selects immediate table promotion, a typed model fails with guidance to use staged promotion because SQLBuild cannot inspect and reconstruct output before mutating the destination.

## Type enforcement and contracts

Type enforcement works whatever the model's `contract` setting, including `contract none`. A
contract decides which columns must exist; see [Contracts](contracts.md).

## Schema changes

Declared types are used in schema-change detection. With type enforcement active, the declared type is authoritative for typed columns when SQLBuild compares a planned model with the warehouse relation.
