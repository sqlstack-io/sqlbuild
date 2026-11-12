<!-- generated-by: sqlbuild skills -->

# Naming rules

> Names of models, tests, macros and other SQLBuild resources are lowercase snake_case.

Online: https://sqlbuild.com/docs/concepts/resource-identities/

Names of models, tests, macros and every other SQLBuild resource are lowercase snake_case, such as
`daily_orders`. SQLBuild never renames anything for you; it suggests the right spelling. Renaming a
file by hand creates a new resource with no history, so use [`sqb rename`](../cli/rename.md) to
keep it.

```text
error[D016]: Invalid model identity 'DailyOrders' in models/DailyOrders.sql;
use snake_case 'daily_orders'
```

A name starts with a lowercase letter, ends with a lowercase letter or digit, and contains only
lowercase letters, digits and underscores. Double underscores are fine, as in
`commerce__mart_v_order`.

A resource named after its file takes the file name without the extension:
`models/daily_orders.sql` is the model `daily_orders`. Folders organise files but are not part of
the name.

## Renaming a resource

[`sqb rename`](../cli/rename.md) and `sqb mv` rename a model or column, update every reference, and
keep the warehouse table's history. See [Refactoring](refactoring.md).

A rename you make by hand creates a new resource. Its build history, fingerprints and audit results
start fresh, and selectors and integrations that use the old name, such as Dagster asset keys, need
updating too. Compile before you build.

## Details

- A private scoped declaration starts with exactly one underscore, such as `_country_codes`. Public
  declarations don't start with an underscore. See [Declaration scopes](declaration-scopes.md).
- A provider class without a `provider_name` gets a snake_case name from its class name:
  `AnalyticsApiProvider` is `analytics_api_provider`. An explicit `provider_name` must already be
  snake_case.
- Warehouse names are not affected: databases, schemas, tables, aliases, columns, tags and groups
  follow the warehouse's own rules.
- Paths SQLBuild reads, such as models, tests, scenarios and source or schema YAML files, must be
  valid UTF-8, including the folders above them. Otherwise compilation fails with `D016`, showing
  each invalid byte as `\xNN`. `sqb scope` reports such a file and continues.
