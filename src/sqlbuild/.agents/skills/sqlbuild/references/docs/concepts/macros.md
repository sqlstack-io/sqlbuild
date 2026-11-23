<!-- generated-by: sqlbuild skills -->

# Macros

> Reuse SQL with typed SQL macros, or generate it with Python macros when it needs logic.

Online: https://sqlbuild.com/docs/concepts/macros/

A macro is a reusable piece of SQL that a model, test, hook or audit calls by name. SQLBuild
expands each call at compile time, so the warehouse only ever sees plain SQL.

There are two kinds:

| | SQL macro | Python macro |
|---|---|---|
| Written as | A `.sql` file: a `MACRO` header and a SQL body | A Python function that returns SQL |
| Called as | `@sql.name(...)` | `@py.name(...)` |
| Lives in | `sql/macros/sql/` | `sql/macros/python/` |
| Arguments | Typed SQL expressions, checked against the caller's column types | Python values: strings, numbers, lists, dictionaries, references |
| Logic | None: the body is SQL | Any Python: loops, conditions, adapter and target context |
| Checks and tools | Each macro is checked on its own; `sqb format`, Rules and lineage see inside every call | Rules and lineage see the SQL each call returns |

Prefer a SQL macro whenever no logic is needed: its calls stay readable, typed and checked, and
every tool sees the SQL it produces.

    Typed, logic-free expressions, CTE lists and column lists.
    Python functions that generate SQL when it needs logic.

## Where macros live

Each kind has its own folder:

```text
my_project/
├── sql/
│   ├── macros/
│   │   ├── python/
│   │   │   └── labels.py
│   │   └── sql/
│   │       └── cents_to_dollars.sql
│   └── models/
└── sqlbuild_project.toml
```

A file directly under a `sql/macros/` folder fails with `D013`, which names the subfolder to move it
to. A macro used by only part of the project can live in that folder instead, as
`_sqlbuild/macros/sql/` or `_sqlbuild/_macros/python/`, and is then visible only there. See
[Declarations and Scopes](declaration-scopes.md).

## Call a macro

Every call names the kind of macro it calls:

```sql
SELECT
  order_id,
  @sql.cents_to_dollars(amount_cents) AS amount_dollars,
  @py.status_label("status") AS status_label
FROM __ref("stg_orders")
```

A bare `@name(...)` fails with `P014`, which names both spellings. Calling a macro with the other
kind's prefix fails with `P020`, and a SQL macro and a Python macro may not share a visible name.

## Use the two kinds together

A SQL macro calls only SQL macros, and a Python macro composes other Python macros through Python
imports. To combine the two, pass one call's result to the other in the calling SQL:

```sql
SELECT @py.bracket(@sql.cents_to_dollars(o.amount_cents)) AS amount_band
FROM __ref("stg_orders") AS o
```

The SQL macro expands first, and the Python macro receives its SQL as a string.

## Where macros work

Macros are supported in:

- Model query SQL
- Inline and named SQL hooks
- Unit tests and scenarios
- Standalone audit SQL
- SQL functions and supported inline source expressions

Macros are not accepted in ordinary `MODEL()` configuration fields. SQL hook entries are the
exception because their contents are SQL.

### Macros used only by tests

A macro that only tests use follows the usual [placement](declaration-scopes/placement.md)
rule: it must be private to the narrowest folder that contains its uses, so the project-wide
`sql/macros/` directory rejects it with `S024`. Put it in `_sqlbuild/_macros/python/` or
`_sqlbuild/_macros/sql/` in the folder of the tests that call it, such as
`tests/unit/orders/_sqlbuild/_macros/python/`, or, when those tests define `__expected__` output
for a model, in that model's `_sqlbuild/_macros/`. The `S024` error names the folder to use.
`tests/unit/_sqlbuild/` itself is rejected with `D013`.

## Choose the right tool

| Use | When |
|---|---|
| [SQL macro](macros/sql-macros.md) | The fragment is plain SQL with typed inputs: a calculation, a filter, a set of CTEs or columns you repeat. The compiler sees inside it. |
| [Python macro](macros/python-macros.md) | Generating the SQL needs logic: loops, conditions on arguments, the adapter or target, or reading project context. |
| [SQL function](functions.md) | The logic belongs in the warehouse, to be called by other tools or evaluated per row there, or it is too large to inline everywhere. |
| [Generic audit](audits.md) | The SQL checks data quality on a model rather than producing part of its query. |
