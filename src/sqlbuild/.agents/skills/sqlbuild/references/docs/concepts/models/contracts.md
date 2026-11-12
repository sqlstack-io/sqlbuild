<!-- generated-by: sqlbuild skills -->

# Contracts

> Validate required or exact model output schemas.

Online: https://sqlbuild.com/docs/concepts/models/contracts/

## Contents

- Validation
- Type enforcement
- Reusable schemas
- Enum columns
- Adopting an existing schema
- Related policies

A model's declared columns provide output metadata, type enforcement, and column audit attachment. The project `check_declared_columns` setting and model `contract` policy determine whether those declarations also define a statically validated output shape.

| Model declaration | Behavior |
|-------------------|----------|
| Unspecified | Follow `settings.check_declared_columns`: `true` validates declared columns as an open shape; `false` treats them as metadata and audit attachment only. |
| `none` | Do not activate declared-shape or nullability contract validation for this model. Explicit type enforcement remains active. |
| `enforced` | Treat declared columns as the complete authoritative output shape and enable runtime exact-schema checks on supported materializations. |

The project default is `check_declared_columns = true`. Projects that use column declarations primarily for metadata and audits, including migrations from dbt, can turn the check off:

```toml
[settings]
check_declared_columns = false
```

With `check_declared_columns = false`, a model must declare `contract enforced` to activate shape validation, the same as every other model declaring `contract none`. A model-level `contract enforced` or `contract none` always overrides the project setting.

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

## Validation

### Compile time

When declared-shape validation is active and SQL analysis can infer the model's output:

- A missing declared column fails.
- A proven type mismatch fails when type enforcement or an exact contract is active.
- A declared non-null column fails when its expression is proven nullable.
- Additional inferred columns fail only for `contract enforced`.

If SQLBuild cannot infer the output shape, these static shape checks cannot run. An enforced contract additionally requires a non-empty column declaration and adds runtime requirements on supported materializations.

Type enforcement remains independent. Typed columns continue to receive static type checks and supported runtime casts with `check_declared_columns = false` and `contract none`.

Configuration fields that reference output columns, including `unique_key`, `cursor`, `updated_at`, and `check_columns`, are checked against an enforced declaration.

### Runtime

For supported materialization paths, `contract enforced` inspects the staged relation and rejects:

- Missing declared columns.
- Additional undeclared columns.
- Warehouse types that differ from declared types.

Runtime contract validation does not scan data for nulls and does not inspect warehouse nullability metadata. `nullable false` participates in static nullability analysis; add a `not_null` audit when null values must be checked at runtime.

Runtime exact-schema validation currently runs for:

- Staged full-table builds.
- Non-microbatch incremental deltas.
- Snapshot deltas.

Views and custom materializations rely on compile-time contract analysis. Microbatch incrementals currently apply type enforcement and audits but do not perform the framework runtime exact-schema validation step.

Staged table validation happens before promotion, so a failure leaves the existing destination untouched. Immediate table promotion is incompatible with `contract enforced` because SQLBuild cannot validate output before replacing the destination.

## Type enforcement

Declaring a model column type enables type enforcement automatically, independently of the contract policy. Type enforcement controls static type compatibility and runtime casts on supported table and incremental paths; contracts control output shape. See [Type Enforcement](type-enforcement.md) for the materialization matrix.

## Reusable schemas

Contracts apply to the effective declaration, not only the reusable base:

```sql
MODEL (
  model_schema order,
  columns (
    ingestion_batch_id (type VARCHAR, nullable false),
  ),
  contract enforced,
);
```

With `contract enforced`, this requires exactly the resolved `order` columns plus `ingestion_batch_id`. With `contract none`, the resolved columns remain metadata, audit attachment points, and inputs to explicit type enforcement without activating shape validation. See [Schemas](schemas.md).

## Enum columns

A column may use a declared enum as a portable logical domain type:

```sql
MODEL (
  description "Example model",
  contract enforced,
  columns (
    fulfillment_method (type fulfillment_method),
  ),
);
```

This does not require, create, or reference a warehouse-native enum type. SQLBuild resolves string-valued enums to `VARCHAR` and integer-valued enums to `INTEGER`. Under `contract enforced`, it also generates an `accepted_values` audit for the declared members. Audit severity and timing follow normal audit configuration and materialization behavior. With the default error severity, it gates staged-table promotion and pre-DML delta paths; views and custom materializations may already have changed their relation when the audit runs.

Enum member references such as `@enum("fulfillment_method").DELIVERY` are a separate feature that render one validated SQL literal. See [Enum Model Contracts](../enums/model-contracts.md) for the complete distinction and lowering behavior.

## Adopting an existing schema

Ordinary `sqb compile` validates authored contracts offline. To compare declarations with existing
warehouse relations, use the explicitly online, read-only contract commands:

```bash
sqb contract diff --from prod --select tag:commerce
sqb contract generate --from prod --select tag:commerce
```

The target named by `--from` supplies the database and schema namespace to inspect. SQLBuild uses
the active project connection and never resolves a second set of origin credentials.

### Preview and write

`contract diff` returns exit code `1` when declarations and physical relations disagree.
`contract generate` prints proposed declaration changes without changing files:

```bash
# Fill missing types and append physical columns missing from code
sqb contract generate --from prod --select tag:commerce --write

# Replace conflicting types/names and remove declarations absent physically
sqb contract generate --from prod --select tag:commerce --write --overwrite
```

Contract commands never mutate warehouse relations, lifecycle state, or fingerprints.

### Safety rules

- Additive generation preserves existing types on conflict, repository-only columns, comments,
  descriptions, audits, nullability, enums, tags, freshness, and loader configuration.
- `--overwrite` is an explicit repository replacement policy, not a warehouse write.
- Generation does not add `contract enforced`; activation remains an authored decision.
- A shared `SCHEMA()` is not changed from one model's evidence. Ownership conflicts are reported.
- Writes are atomic and SQLBuild recompiles the project, restoring prior contents if validation
  fails.
- Sources are first-class selectable resources, for example `--select source:raw_orders`.

Enforced upstream contracts are authoritative compile-time interfaces. When input evidence is
complete, missing or ambiguous columns fail in projections, joins, filters, grouping, windows, and
ordering. Partial or opaque schemas remain open; SQLBuild does not turn missing metadata into a
false error. Use `sqb lineage <model> --include-uses --format json` to inspect direct
non-projection uses such as `join_on`, `where`, `group_by`, and `window_order_by`.

## Related policies

The project default checks declared columns as an open shape for models that omit `contract`. A repository can require an explicit `contract enforced` instead with `settings.check_declared_columns = false` or configure a custom [Rule](../rules/custom-rules/overview.md) that requires enforced contracts for selected model families.

Contracts also constrain schema-change behavior. For example, `snapshot_schema_change append_new_columns` is incompatible with `contract enforced` because an unannounced appended column would violate the exact declaration.
