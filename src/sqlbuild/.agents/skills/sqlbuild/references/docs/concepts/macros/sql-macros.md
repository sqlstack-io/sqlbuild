<!-- generated-by: sqlbuild skills -->

# SQL Macros

> Write typed, logic-free SQL fragments that the compiler checks and sees inside.

Online: https://sqlbuild.com/docs/concepts/macros/sql-macros/

## Contents

- Create a SQL macro
- Call a SQL macro
- Parameter types
- Parameters in the body
- Relation slots
- Compose macros
- Tools
- Reference

A SQL macro is a reusable piece of SQL with typed parameters. Unlike a
[Python macro](python-macros.md), it holds no logic: its body is SQL, so the compiler checks
it on its own, expands it in place, and formatting, rules and column lineage see through every
call.

A SQL macro is one of three forms, decided by how its body starts:

| Body starts with | Form | Called |
|---|---|---|
| anything else | expression | where an expression belongs, such as a select item or a `WHERE` clause |
| `WITH` | CTE list | between the CTEs of a `WITH` list |
| `SELECT` | column list | as an item of a select list |

Bodies are read by SQLBuild's native parser for the adapter's SQL analysis dialect. On an adapter
whose dialect has no native parser, including a custom adapter without
`sql_analysis_dialect_name`, every call fails with `P015`, which says SQL macros are unavailable for
that dialect.

## Create a SQL macro

Put each SQL macro in its own `.sql` file under `macros/sql/`. The file name is the macro name.
Python macros live next to it in `macros/python/`:

```text
my_project/
├── macros/
│   ├── python/
│   │   └── labels.py
│   └── sql/
│       ├── cents_to_dollars.sql
│       ├── clean_orders.sql
│       └── audit_columns.sql
├── models/
└── sqlbuild_project.toml
```

A file directly under a `macros/` folder fails with `D013`, which names the subfolder to move it
to. Folder scopes use the same layout, such as `models/marts/_sqlbuild/_macros/sql/`.

Rules that bound declaration folders, such as `SQBRDECLARATION304` (files per folder) and
`SQBRDECLARATION306` (shared filename prefixes), count `python/` and `sql/` as separate folders. To
split a full `sql/` folder, move files into subfolders such as `macros/sql/text/`: a SQL macro is
named by its file, so it keeps its name and its callers stay unchanged.

A SQL macro file starts with a `MACRO` header, then the body:

```sql
-- macros/sql/cents_to_dollars.sql
MACRO (
  description "Cents to dollars",
  arguments (amount_cents BIGINT, places INT CONST DEFAULT 2),
  returns DECIMAL(18, 2),
);

CAST(ROUND(@amount_cents / 100, @places) AS DECIMAL(18, 2))
```

`returns` is optional for expression macros and not allowed for the other forms. See
[Result types](sql-macro-reference.md#result-types) for what it does.

The description follows the same rules as every header string: write text that contains double
quotes as `"""..."""`, and a wrapped description is stored folded into one flowing text (see
[Header strings](../models/configuration.md#header-strings)). Quotes in the macro body
never change how the header is read.

## Call a SQL macro

Every macro call says its kind: `@sql.name(...)` calls a SQL macro and `@py.name(...)` a Python
macro. A bare `@name(...)` fails with `P014`, which names both spellings.

Arguments are SQL, positional or named with `name := value`:

```sql
SELECT
  o.order_id,
  @sql.cents_to_dollars(o.fee_cents + o.tax_cents, places := 0) AS fees
FROM __ref("stg_orders") AS o
```

compiles to:

```sql
SELECT
  o.order_id,
  CAST(ROUND((o.fee_cents + o.tax_cents) / 100, 0) AS DECIMAL(18, 2)) AS fees
FROM analytics.stg_orders AS o
```

An argument is parenthesised where the body needs it, and so is an expression macro's result where
the call stands, such as before `AT TIME ZONE` or `FILTER`. Arguments resolve in the caller's
scope, so a model can pass its own private constant; the macro receives only the value.

### CTE lists

A CTE-list macro inserts its CTEs, with its own names, into the caller's `WITH` list at the call:

```sql
-- macros/sql/clean_orders.sql
MACRO (
  description "Deduplicated, non-deleted orders",
  arguments (src RELATION (order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN)),
);

WITH live AS (SELECT * FROM @src WHERE NOT is_deleted),
ranked AS (
  SELECT *, ROW_NUMBER() OVER (PARTITION BY order_id ORDER BY updated_at DESC) AS row_num
  FROM live
),
latest AS (SELECT * EXCLUDE (row_num) FROM ranked WHERE row_num = 1)
```

```sql
WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),
@sql.clean_orders(orders_raw)
final AS (SELECT order_id, amount FROM latest)
SELECT * FROM final
```

The compiled model holds `orders_raw`, `live`, `ranked`, `latest` and `final`, flat and in that
order, and later CTEs may read any of the macro's CTEs. Commas around the call are optional;
`sqb format` normalises them.

The last CTE of a CTE-list macro is its result. To call one macro more than once in a model, name
each call's CTEs with the reserved `__cte` argument:

```sql
WITH eu_raw AS (SELECT * FROM __ref("stg_orders_eu")),
us_raw AS (SELECT * FROM __ref("stg_orders_us")),
@sql.clean_orders(eu_raw, __cte := eu_orders)
@sql.clean_orders(us_raw, __cte := us_orders)
final AS (SELECT * FROM eu_orders UNION ALL SELECT * FROM us_orders)
SELECT * FROM final
```

The macro's last CTE takes exactly the `__cte` name, and its other CTEs become `<name>__<cte>`, so
this model holds `eu_raw`, `us_raw`, `eu_orders__live`, `eu_orders__ranked`, `eu_orders`,
`us_orders__live`, `us_orders__ranked`, `us_orders` and `final`. References inside the body are
renamed with them, wherever a name reads the macro's CTE: a table alias such as `FROM live AS l`,
or a nested `WITH` that defines its own `live`, keeps its own meaning. Arguments are never renamed,
even one named like a macro CTE. Later CTEs may read the prefixed ones too.

`__cte` takes a bare name, is only for CTE-list calls (`P017` otherwise), and is not declared in
`arguments`. A call without it keeps the macro's own names.

### Column lists

A column-list macro inserts its columns into the caller's select list. Every column needs a name:

```sql
-- macros/sql/audit_columns.sql
MACRO (
  description "Standard audit columns",
  arguments (src RELATION (created_at TIMESTAMP, updated_at TIMESTAMP)),
);

SELECT @src.created_at, @src.updated_at, @src.updated_at > @src.created_at AS was_updated
```

`SELECT o.order_id, @sql.audit_columns(o) FROM orders AS o` compiles to
`SELECT o.order_id, o.created_at, o.updated_at, o.updated_at > o.created_at AS was_updated FROM orders AS o`.

Use a column list when the same ordered set of columns repeats, for example in every branch of a
`UNION ALL`. The call takes no alias (`P021`): each inserted column is named in the macro.

A CTE or column name a macro inserts that the caller already defines fails with `P018`, which names
both. Calling the same CTE-list macro twice without `__cte` fails the same way, and the error
suggests `__cte`.

## Parameter types

Every parameter has a type, and may have a `DEFAULT`:

| Type | Accepts |
|---|---|
| `BIGINT`, `VARCHAR`, `TIMESTAMP`, ... | Any SQL expression of a compatible type, evaluated per row: a column, arithmetic, a function call, a literal or a constant. |
| `ANY` | Any scalar SQL expression, whatever its type: a column, a literal, an expression or a scalar constant. Its type is not checked. Lists and relations fail with `P017`. |
| `ANY NUMERIC`, `ANY TEXT`, `ANY TEMPORAL` | Like `ANY`, but only values of one family: integer, decimal and float types; string types; or date, time and timestamp types. |
| `<type> CONST` | Only a value known at compile time: a literal, a scalar constant, an enum value, or a `CONST` parameter of a calling macro. A column or expression fails with `P017`. Scalar types only. |
| `LIST <type>`, `SET <type>` | An inline list or set, or a list or set constant. Always compile-time, so they take no `CONST`. They render like constants. |
| `ENUM <name>` | A member of that enum, such as `@enum("order_status").OPEN`. |
| `RELATION (<slots>)` | A CTE name, a table alias, or `__ref()`, `__source()` or `__seed()`, with the columns its [slots](#relation-slots) read. |

SQL types are written as in [schema declarations](../models/schemas.md) and checked against the
adapter's SQL dialect the same way, including the dialect's array types such as `VARCHAR[]`. A
runtime array value uses the array type; `LIST VARCHAR` is a list known at compile time.

Use `ANY` when the body works for values of any type, typically because it casts the argument
itself, as in `CAST(@order_ref AS VARCHAR)`, so callers can pass a number or a string. The body is
still checked, and the argument's type counts as unknown there. `ANY` is not `VARIANT`: a
`VARIANT` parameter says the macro takes semi-structured data, while `ANY` says it takes a plain
value of any type. `ANY` takes a `DEFAULT` but not `CONST`, which needs a type to check values
against, and it cannot be a `LIST` or `SET` element type.

### Choosing an argument type

- A fixed type, such as `BIGINT`, when the body needs that type. Callers may pass any compatible
  value, and the body is checked with that type.
- A family, such as `ANY NUMERIC`, when the body works for every type of the family, such as
  `ROUND(@value, @places)`. A call whose argument has a known type outside the family fails with
  `P017` at the argument. The body is checked as if the argument had the family's widest type
  (`DOUBLE`, `VARCHAR` or `TIMESTAMP`), so `UPPER(@value)` on `ANY NUMERIC` fails with `P025` at the
  macro file. Pair a family with `returns TYPE OF value` to return the caller's own type.
- `ANY` when the body works for values of any type, typically because it casts the argument itself.

Families take a `DEFAULT` but not `CONST`, and are not `LIST` or `SET` element types.

Leave `CONST` off unless the body needs a literal. `CONST` is part of the macro's contract: callers
can no longer pass a column.

## Parameters in the body

Write every parameter as `@name` in the body, as [generic audits](../audits.md) do. A bare
identifier is always plain SQL, a column, CTE or table, never the parameter:

```sql
-- macros/sql/is_status.sql
MACRO (
  description "Whether the status column matches",
  arguments (status VARCHAR),
  returns BOOLEAN,
);

status = @status
```

Here `status` is the caller's column and `@status` the argument. A relation parameter is qualified
directly, as `@src.created_at` or `@src.*`, or aliased, as `FROM @src AS s` and then `s.created_at`.

These fail with `P022`:

- an `@name` that names no parameter; the error lists the parameters
- a relation parameter written bare where it would read a table, such as `FROM src` or
  `src.created_at`; the error says `src is a parameter of macro clean_orders(); write @src`

Parameters may not be named `sql`, `py`, `const`, `enum`, `param` or `__cte`.

## Relation slots

Everything a macro reads from a relation is declared in its `RELATION` type as a slot. A slot is
either one typed column or a `COLUMNS` group of columns the caller chooses, so one macro works at
any grain:

```sql
-- macros/sql/weekly_rates.sql
MACRO (
  description "Weekly refund rate per grain",
  arguments (weekly RELATION (keys COLUMNS, order_count BIGINT, refund_count BIGINT)),
);

WITH counted AS (
  SELECT
    s.@weekly.keys,
    s.@weekly.order_count AS orders,
    s.@weekly.refund_count AS refunds
  FROM @weekly AS s
),

rated AS (
  SELECT
    @weekly.keys,
    SUM(refunds) / NULLIF(SUM(orders), 0) AS refund_rate
  FROM counted
  GROUP BY @weekly.keys
)
```

A `COLUMNS` slot is untyped: each column passed to it keeps its own type, so the expanded SQL is
fully typed.

## Compose macros

A SQL macro may call other SQL macros, and pass its own parameters on as `@name`:

```sql
-- macros/sql/net_amount.sql
MACRO (
  description "Net amount in dollars",
  arguments (gross_cents BIGINT, refund_cents BIGINT),
  returns DECIMAL(18, 2),
);

@sql.cents_to_dollars(@gross_cents - COALESCE(@refund_cents, 0))
```

Macros that call each other in a cycle fail with `P019`. SQL and Python macros never call each
other (`P020`); compose them in the model by passing results, as in
`@py.bracket(@sql.cents_to_dollars(o.amount_cents))`. A SQL macro and a Python macro may not share
a visible name.

A body may not call `__ref()`, `__source()` or `__seed()` (`P016`): pass relations in as `RELATION`
arguments. `@const(...)` and `@enum(...)` in a body resolve from the macro file's folder, not from
the calling model, so a macro means the same thing wherever it's called. A constant or enum that
isn't visible from there fails with `S026`; to use a caller's value, make it an argument and pass
`@const(...)` at the call site. SQL macros a body calls resolve the same way, so a macro can call a
folder-local macro beside it that its callers cannot see.

## Tools

- `sqb format` formats SQL macro files, bodies of every form and the `MACRO` header, and
  normalises the commas around CTE-list and column-list calls.
- [Rules](../rules.md) and [column lineage](../column-lineage.md) read the expanded
  SQL, so they see through every call. A Rule finding inside an expansion is reported at the call,
  marked `(in SQL generated by @sql.name(...))`. The model's expanded SQL is in
  `target/compiled/models/`, or in `target/compiled-failed/models/` when the Rules fail; see
  [When Rules fail](../../cli/compile.md#when-rules-fail).
- Editing a macro recompiles its callers.

Test SQL macros with [macro-mode unit tests](../testing-reference.md#macro-tests), and replace one
in a model test with a [`__macro__` mock](../testing.md#macro-mocking).

See [Macros](../macros.md#choose-the-right-tool) for when to use a SQL macro, a Python
macro, a SQL function or a generic audit.

## Reference

[SQL macro reference](sql-macro-reference.md) covers positions that need a
literal, reading and mapping relation slots, the checks at each call and result types.
