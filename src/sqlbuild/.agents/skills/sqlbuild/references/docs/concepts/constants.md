<!-- generated-by: sqlbuild skills -->

# Constants

> Define reusable compiler-validated values and reference them safely from SQL.

Online: https://sqlbuild.com/docs/concepts/constants/

Constants give a name to a value used in SQL. SQLBuild validates the value during compilation and
asks the active adapter to render it safely for its SQL dialect. Constant values are data, never raw
SQL snippets.

## Create a constant

Put project-wide constants under the top-level `sql/constants/` directory. Files are discovered
recursively, and one file may contain more than one declaration.

```text
my_project/
├── sql/
│   ├── constants/
│   │   ├── commerce/
│   │   │   └── thresholds.sql
│   │   └── reporting_day.sql
│   └── models/
└── sqlbuild_project.toml
```

```sql
-- sql/constants/commerce/thresholds.sql
CONSTANT (name min_items, value 7);
CONSTANT (name fallback_source, value "web");
CONSTANT (name enabled, value true);
CONSTANT (name ratio, value 0.75);
CONSTANT (name missing_value, value null);
```

Folders below `sql/constants/` are organizational. They do not change where project-wide constants are
available.

## Use a constant

Reference a constant with `@const("name")`:

```sql
SELECT *
FROM order_batches
WHERE item_count >= @const("min_items")
  AND source = @const("fallback_source")
```

References work in model queries, SQL hooks, SQL functions, audits, unit tests, scenarios, and
inline source expressions. An unknown constant fails compilation.

## Scalar values

Constants support strings, signed integers, booleans, finite floating-point numbers, exact
decimals, and `null`. Integers use a portable signed 64-bit range. `NaN` and positive or negative
infinity are rejected.

Use `type decimal` with a quoted value when decimal precision must be exact:

```sql
CONSTANT (
  name usd_rate,
  type decimal,
  value "2.4700",
);
```

SQLBuild parses the quoted value directly as a decimal rather than first converting it to a binary
float. An incompatible `type` and `value` fails compilation.

## Naming rules

Public constant names must be unique among public constants and cannot begin with `_`. Enum,
constant, and macro names use separate namespaces, so an enum and a constant may share a name.

## More constant features

    Define lists, sets, and objects, then choose value-list or native-array rendering.
    Keep a constant inside one model when no other resource should use it.

To limit a constant to one folder, or to that folder and its child folders, see
[Declarations and Scopes](declaration-scopes.md).
