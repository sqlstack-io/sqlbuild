<!-- generated-by: sqlbuild skills -->

# Audits

> Violation and measurement checks that gate data and record quality outcomes.

Online: https://sqlbuild.com/docs/concepts/audits/

## Contents

- How audits work
- Measurement audits
- Evidence
- Audit factories
- Audit metadata
- Result history
- Built-in audits
- Custom generic audits
- Singular audits
- Source audits
- Seed audits
- Severity
- Run scope
- Skipping SQL analysis
- Running audits standalone

Audits are SQL queries that verify data quality. Violation audits return invalid rows. Measurement
audits return a value that SQLBuild evaluates against authored thresholds and sample policy.
SQLBuild can run audits before table promotion or incremental DML so error-severity failures do not
reach the target.

## How audits work

Violation audits pass when their query returns zero rows. Measurement audits produce one value and
optionally a sample count; their outcome is `pass`, `warn`, `fail`, or `insufficient`.

For `error` severity audits:
- **Full table builds:** SQLBuild materializes into a staging table, runs audits against it, and only promotes to the target if all audits pass. If any fail, the staging table is kept for inspection and the production table is untouched.
- **Incremental models:** Delta-phase audits validate each batch before DML is applied. If an audit fails, the batch is not applied.

For `warn` severity audits, the build continues and the failure is reported in the output.

## Measurement audits

A reusable measurement audit separates the aggregate query from optional bounded evidence:

```sql
-- audits/generic/valid_order_rate.sql
AUDIT (
  evaluation measurement,
  value valid_rate,
  sample_count total_rows,
  sample_unit rows
);

MEASURE (
  SELECT
    COUNT(*) AS total_rows,
    100.0 * AVG(CASE WHEN @condition THEN 1 ELSE 0 END) AS valid_rate
  FROM @relation
);

EVIDENCE (
  SELECT * FROM @relation WHERE NOT (@condition)
);
```

Attach threshold and sample policy where the audit is used:

```sql
MODEL (
  description "Valid order rate",
  audits [
    valid_order_rate (
      condition "order_id IS NOT NULL",
      minimum_samples 100,
      evidence_limit 20,
      thresholds (warn (below 99.9), error (below 99))
    )
  ]
);
```

`minimum_samples` keeps low-volume measurements distinct as `insufficient` rather than inventing a
pass or failure. The measurement and threshold determine the outcome.

## Evidence

When an audit warns or fails, SQLBuild keeps some of the rows behind the failure as evidence: a
violation audit keeps the rows its query returned, and a measurement audit runs its `EVIDENCE`
query. `evidence_limit` caps the rows kept (default 20), and at most 64 KB of them are kept. Set it
on any attachment, for example `not_null (evidence_limit 5)`.

Evidence is diagnostic and never changes the outcome. It holds data values from the audited
relation, and is recorded in the `evidence_json` column of `_sqlbuild_audit_results`, as
`evidence` in the `audit_completed` lifecycle event, and in
[Recall](observability/recall.md#audit-evidence), which `audit_evidence` controls.

## Audit factories

Use a Python audit factory when many related audit instances should be generated from one reviewed
declaration:

```python
from sqlbuild.audits import AuditCase, AuditSeverity, audit_factory

@audit_factory
def order_quality():
    return [
        AuditCase(
            name="positive_amount",
            definition="expression_is_true",
            arguments={"expression": "amount > 0"},
            severity=AuditSeverity.ERROR,
        )
    ]
```

Attach it with `MODEL (audit_factories [order_quality])`. Generated cases compile to the same audit
contract as directly authored instances.

## Audit metadata

Use `meta` to attach structured context, such as the segment a case covers or why its condition
excludes some rows, so dashboards can group and filter results without parsing descriptions:

```python
AuditCase(
    name="valid_order_rate__emea",
    definition="valid_order_rate",
    arguments={"condition": "order_id IS NOT NULL AND region = 'emea'"},
    thresholds=MeasurementThresholds(error=below(99)),
    meta={
        "region": "emea",
        "exclusion": "orders before 2020",
        "exclusion_reason": "imported without order IDs",
    },
)
```

SQL attachments accept the same option, for example
`valid_order_rate (condition "order_id IS NOT NULL", meta (region "emea"), ...)`.

`meta` must be a mapping with string keys and JSON values; anything else is a compile error that
names the offending value. It is never passed to the audit SQL and never affects evaluation,
thresholds, or audit fingerprints, so changing only `meta` reruns nothing that a `description`
change would not. It is recorded with each result in the `audit_meta_json` column of
`_sqlbuild_audit_results`, as `audit_meta` in the `audit_completed` lifecycle event, and as
`audit_meta` on audit checks in JSON output.

## Result history

Native warehouse adapters best-effort append confirmed audit outcomes to
`_sqlbuild_audit_results`. Rows are immutable and use deterministic IDs, so retrying the same result
is idempotent. Projection failure is reported separately and does not change the audit outcome or
command exit code. A results table created before `audit_meta_json` existed gains that nullable
column the first time a result with `meta` is recorded. Lifecycle sinks can also consume the corresponding `audit_completed` fact, which
is published as each audit finishes.

## Built-in audits

SQLBuild includes four generic audits out of the box. You do not need to define these in `audits/generic/` - they are available automatically:

| Audit | Description | Parameters |
|-------|-------------|------------|
| `not_null` | Fails if any row has a NULL value in the column | Column-level only |
| `unique` | Fails if any non-NULL value appears more than once | Column-level only |
| `accepted_values` | Fails if any non-NULL value is not in the allowed list | `values` - list of allowed values |
| `relationships` | Fails if any non-NULL value does not exist in the referenced column | `to` - target relation as `__ref(...)`, `__source(...)` or `__seed(...)`, `field` - target column |

### Using built-in audits

Attach them in the `MODEL()` header like any generic audit:

```sql
MODEL (
  description "One row per order",
  materialized view,
  tags [staging],
  columns (
    order_id (audits [not_null, unique]),
    customer_id (audits [not_null]),
    status (
      audits [
        accepted_values (values ["placed", "preparing", "ready", "completed", "cancelled"]),
      ],
    ),
    payment_method (
      audits [
        relationships (to __ref("stg_payments"), field "method"),
      ],
    ),
  ),
);
```

### Overriding built-in audits

If you define a generic audit with the same name as a built-in (e.g. `audits/generic/not_null.sql`), your definition takes precedence. SQLBuild emits a warning so you're aware of the override:

```
warning[P003]: project audit 'not_null' overrides built-in audit 'not_null'
```

## Custom generic audits

Beyond the built-ins, you can define reusable SQL templates in a generic audit role. They use `@parameter` placeholders that are resolved by the audit engine at compile time.

| Location | Who can attach it |
|----------|-------------------|
| `audits/generic/` | Resources anywhere in the project |
| `<folder>/_sqlbuild/audits/generic/` | Resources in `<folder>` and below |
| `<folder>/_sqlbuild/_audits/generic/` | Resources directly in `<folder>` only |

A generic audit must live under the nearest folder that contains every model, source, seed, or
audit factory that attaches it. A top-level generic audit used only by models under
`models/marts/` fails to compile and names the folder it belongs in:

```
error[S024]: Declaration 'audit:non_negative' is currently project at 'audits/generic' (audits/generic/non_negative.sql); required exact-owner-private at 'models/marts'. Consumers: model:daily_revenue. Move it to 'models/marts/_sqlbuild/_audits/generic/'
  audit: non_negative
  --> audits/generic/non_negative.sql:1:1
```

Attaching a scoped generic audit from a folder that cannot see it fails with `S006`. Built-in
audits are available everywhere and are unaffected. See
[Where to Put Declarations](declaration-scopes/placement.md).

```sql
-- audits/generic/expression_is_true.sql
AUDIT ();

SELECT *
FROM @relation
WHERE NOT (@expression)
```

### Audit parameters

Generic audit SQL uses `@name` for parameter placeholders. These are resolved by the audit engine, not the general SQL interpolation system:

| Parameter | Description |
|-----------|-------------|
| `@column` | The column name (auto-populated for column-level audits) |
| `@relation` | The target relation (auto-populated from the attached model, source or seed) |
| `@model`, `@source`, `@seed` | The attached resource's name. Each is set only for its own kind, so an audit that attaches to more than one kind reads `@relation` |
| `@'name'` | A quoted parameter passed from the audit declaration (e.g. `@'values'`) |
| `@name` | An unquoted parameter passed from the audit declaration (e.g. `@expression`) |

Generic and singular audit SQL uses macros, constants, and enums available from the folder that owns
the audit, not from a model or source that uses the audit. An audit in
`models/marts/_sqlbuild/_audits/generic/` sees the declarations a model in `models/marts/` sees; an
audit in the top-level `audits/` roles sees only project-wide declarations. Using a declaration
that is not visible from there is a compile error. See
[How Visibility Works](declaration-scopes/visibility.md).

### Attaching custom generic audits

```sql
MODEL (
  description "One row per order",
  materialized table,
  audits [
    expression_is_true (
      name "revenue_is_non_negative",
      expression "total_revenue_cents >= 0",
    ),
  ],
);
```

Every argument an attachment passes must be read by the audit's SQL or evidence SQL as `@name` or
`@'name'`. An argument the audit never reads, including a misspelt one, is a compile error that names
the argument to remove, so a check is never silently broader or narrower than its attachment says:

```text
models/marts/fct_orders.sql audit 'expression_is_true' passes argument 'where_condition', which the audit's SQL in audits/generic/expression_is_true.sql never uses; remove 'where_condition' from the attachment
```

An argument that repeats one the attached resource supplies, such as `column`, is not checked.

### Audits that read other resources

An attached audit can read other models, sources, seeds, and table functions besides its target,
for example to check that every value appears in a lookup table:

```sql
-- audits/generic/known_status.sql
AUDIT ();

SELECT o.*
FROM @relation o
LEFT JOIN __seed("order_statuses") s USING (status)
WHERE s.status IS NULL
```

An attached audit always gates its target. SQLBuild waits for everything the audit reads, then
runs the audit exactly where any other audit on that target runs:

- On a model, it follows the model's table promotion mode and the audit's run scope. With `staged`
  promotion, a `final` audit checks the staged table before promotion; with `immediate` promotion,
  or for final-phase audits on incremental and snapshot models, it runs after the write, like any
  other audit there. See [Run scope](#run-scope) and
  [Table promotion mode](models/materializations.md#table).
- On a source or seed, it runs before any model that depends on it is built.

An `error` failure blocks everything downstream of the target, and where the target's other audits
would keep it unpublished, this one does too. A `warn` failure is reported and the build continues.
The audit is reported under its target. If a resource the audit reads fails to build, or a source
it reads fails its own `error` audit, the target is blocked as well.

In a partial build such as `sqb build -s orders`, resources the audit reads but that are not
selected are not built; the audit reads their existing tables. Selectors such as `+orders` follow
data lineage only, so they do not add the audit's reads to the selection. If an unselected read
does not exist in the warehouse yet, planning fails with `S301` and names the audit and the read;
see [Missing upstream dependencies](../cli/plan.md#missing-upstream-dependencies).

An attached audit cannot read a resource built from its own target, because the target would have
to wait for its own dependants. That fails to compile with `P005`:

```
error[P005]: Audit 'known_status' on model 'orders' reads model 'order_summary', which depends on
'orders'; an attached audit gates its target, so it cannot wait for a resource built from that
target
  = help: write a singular audit under audits/singular/ that references both 'orders' and
  'order_summary'; it runs once both are built
```

## Singular audits

Singular audits are standalone SQL files that check a relationship between resources. They must be
cross-resource: a singular audit references two or more models, or at least one model plus a
source, seed, or table function, through `__ref()`, `__source()`, `__seed()`, or `__table_fn()`.

```sql
-- models/marts/_sqlbuild/audits/singular/orders_have_payments.sql
AUDIT (
  name "completed_orders_have_payments",
  severity error
);

SELECT o.order_id
FROM __ref("fact_orders") o
LEFT JOIN __ref("stg_payments") p ON o.order_id = p.order_id
WHERE p.payment_id IS NULL
  AND o.order_status = 'completed'
```

Other shapes fail with `P004`:

| Singular audit references | Use instead |
|---------------------------|-------------|
| One model only, including a self-join of that model | A generic audit attached to the model that selects `FROM @relation` |
| Sources or seeds only | YAML audits on the source or seed |
| No SQLBuild resource, such as hard-coded relation names | Reference resources with `__ref()`, `__source()`, or `__seed()` |

Singular audits live only in a `singular/` role:

| Location | What it may reference |
|----------|-----------------------|
| `audits/singular/` | Resources whose nearest shared folder is the project root, such as models in sibling folders or a model and a source |
| `<folder>/_sqlbuild/audits/singular/` | Resources in `<folder>` and below |

There is no `_audits/singular/` role, because singular audits are never attached by name. A
singular audit must live under the nearest folder containing everything it references. Audit files
directly under `audits/` or in any other `audits/` child are compile errors.

SQLBuild infers where a singular audit runs from its references. When one referenced model is
downstream of every other referenced model, the audit attaches to that model. Otherwise it runs at
the end of the build, after the resources it references are published. Only singular audits run at
the end of the build; attached audits always gate their target.

## Source audits

Sources support the same audit system as models. Audits attached to sources run *before* any dependent model is built:

```yaml
sources:
  - name: raw__orders
    description: Orders
    columns:
      - name: id
        audits:
          - not_null
          - unique
    audits:
      - expression_is_true:
          name: no_future_orders
          expression: "ordered_at <= CURRENT_TIMESTAMP"
```

If a source audit with `error` severity fails, all downstream models that depend on that source are blocked. This lets you catch data quality issues at the source before any transformations run.

A source's audits run before the first model in the build that reads it. A source no selected model
reads is audited at the end of the build only when loading is off (`--no-load` or
`auto_load_sources = false`) or when the source is selected by name, such as
`sqb build --select raw_orders`; otherwise its audits do not run.

A source audit that reads other resources waits for them before any model that depends on the
source is built. It may read a model that is not built from the source; reading one that is fails
with `P005`. See [Audits that read other resources](#audits-that-read-other-resources).

## Seed audits

Seeds use the same YAML audit syntax as sources, at table and column level:

```yaml
seeds:
  - name: waffle_types
    description: Waffle types
    columns:
      - name: waffle_type_id
        type: INTEGER
        audits:
          - not_null
          - unique
    audits:
      - expression_is_true:
          expression: "price_cents >= 0"
          severity: error
```

Seed audits run in `sqb build` after the seed loads and after everything the audit reads is built.
An `error` failure blocks every model that depends on the seed; a `warn` failure is reported and
the build continues. A seed audit cannot read a model built from that seed (`P005`).

## Severity

| Severity | Behavior |
|----------|----------|
| `error` | Blocks the build. Staging table is not promoted, DML is not applied. |
| `warn` | Reports a warning but allows the build to continue. |

An audit whose query raises a warehouse error, for example because a relation it reads is missing,
fails whatever its severity: the build reports the error message and fails, and everything
downstream of the audit's target is blocked, the same as when a model audit raises.

Audits are `error` by default. Set a different default severity in `sqlbuild_project.toml`:

```toml
[settings]
default_audit_severity = "warn"
```

Override per audit instance in the `MODEL()` header:

```sql
columns (
  order_id (audits [not_null (severity error)]),
),
```

## Run scope

Audits on incremental models can run at different lifecycle phases:

| Scope | Behavior |
|-------|----------|
| `final` | Run once against the staged table before promotion. |
| `delta_and_final` | Run against each delta batch before DML, then again against the target after all batches complete (default). |

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  ...
  columns (
    activity_hour (audits [not_null (run_scope delta_and_final)]),
  ),
  audits [
    expression_is_true (
      name "orders_placed_is_non_negative",
      expression "orders_placed >= 0",
      run_scope delta_and_final,
    ),
  ],
);
```

Delta-phase audits with `error` severity block DML before the target is updated. This is visible in the build output as `audit (d)` for delta-phase and `audit (f)` for final-phase:

```
  10/13  table     hourly_order_activity  (delete_insert)                OK     0.16s
           audit (d) expression_is_true                                  PASS  4/4
           audit (d) not_null (activity_hour)                            PASS  4/4
           audit (f) expression_is_true                                  PASS
           audit (f) not_null (activity_hour)                            PASS
```

The `4/4` indicates the audit passed for all 4 microbatch batches.

If a model is not incremental, `delta_and_final` degrades to `final` automatically.

## Skipping SQL analysis

Compile checks audit SQL like model SQL: unknown columns and functions, type mismatches and syntax
errors are compile errors. For SQL the parser cannot read, add `sql_analysis false` to the `AUDIT`
header of a singular or generic audit; the audit still runs. Audits attached to a model with
`sql_analysis false`, and every audit in a `--no-sql-analysis` run, are not analysed. With
`[settings] require_sql_analysis = true`, the opt-out is accepted only on audit SQL that cannot be
parsed (see [Requiring SQL analysis](models/configuration.md#requiring-sql-analysis)).

## Running audits standalone

```bash
sqb audit
```

This runs all audits without rebuilding any models.

Standalone audits run serially unless concurrency is configured explicitly, through
`SQLBUILD_CONCURRENCY`, or in project settings. For example, `sqb audit --concurrency 8` runs up
to eight selected audits at once, using one warehouse connection per active worker. Increase this
limit deliberately because parallel queries can increase warehouse load and cost. See
[`sqb audit`](../cli/audit.md) for precedence, ordering, and cancellation details.
