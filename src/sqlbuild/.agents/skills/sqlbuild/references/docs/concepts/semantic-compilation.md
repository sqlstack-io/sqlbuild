<!-- generated-by: sqlbuild skills -->

# What sqb compile checks

> Offline reference, semantic and type checks that sqb compile runs before any warehouse work.

Online: https://sqlbuild.com/docs/concepts/semantic-compilation/

## Contents

- Closed and open inputs
- What gets checked
- What isn't checked
- Reading a diagnostic
- Turning analysis off
- Diagnostic codes

`sqb compile` checks your SQL before anything runs in the warehouse: unknown columns, wrong types,
misspelled functions, invalid grouping, and more. It works offline, from your project alone.

```text
error[B002]: Unknown column 'qty' in stg_orders (as o)
  model: fact_orders
  --> models/marts/fact_orders.sql:16:5
     |
  16 |   o.qty,
     |     ^^^
  = note: stg_orders has: quantity, status, waffle_type_id, customer_id, order_id, ordered_at
  = note: 2 downstream uses of fact_orders.quantity were not checked because of this error
  = help: did you mean 'quantity'?
```

## Closed and open inputs

A check can only reject a column when SQLBuild knows the input's complete set of columns. Such an
input is **closed**; otherwise it is **open**.

| Input | Shape |
|---|---|
| Model or source with `contract: enforced`, seed, declared table-function output | Closed |
| Expression source | Closed: names come from its expression |
| Model with an explicit `SELECT` list | Closed names, even over open inputs |
| `SELECT *` over closed inputs | Closed |
| `SELECT *` over an open input, or a table source without an enforced contract | Open |

Each input is checked separately, so joining an open source doesn't switch off checks on a closed
one. An unqualified column that could come from an open input can't be rejected offline.

Declaring some columns on a source doesn't close it. To close a source, adopt its physical schema:

```bash
sqb contract generate --from <target> --select source:raw_orders --write
```

`sqb plan` and `sqb build` also recheck source readers against the warehouse's real columns before
running models.

## What gets checked

Checks run on every SQL body a project owns: models (including CTEs, subqueries and expanded
macros), audits, SQL unit tests and SQL hooks. A problem in an audit or test is reported against it,
for example `audit 'recent_rows' on model 'orders': ...`, and fails compile instead of the
warehouse.

| Check | Example | Code |
|---|---|---|
| Unknown or ambiguous columns | `o.qty` when the input has `quantity` | B002, B003 |
| Columns named in configuration | `unique_key`, `cursor`, audit columns, SQL-test fixture columns | B300, B302 |
| Types of configured columns | A timestamp `cursor` on a VARCHAR column | B301 |
| Types that the warehouse rejects | `ordered_at > 5` on a TIMESTAMP in DuckDB | B210–B219 |
| Implicit conversions that can fail at runtime | `order_id = customer_code` (INTEGER and VARCHAR) in Snowflake | B211–B218 |
| Function names and spellings | `STARTS_WITH` in Snowflake, which only accepts `STARTSWITH` | B101 |
| Function arity | Too many arguments to a built-in or project UDF | B102 |
| Truth-value tests the warehouse rejects | `x IS NOT TRUE` in Snowflake or SQL Server | B103 |
| Grouping, aggregates and windows | A non-grouped column next to an aggregate | B230–B232 |
| Set operations | A VARCHAR branch combined with a numeric branch | B215, B216 |
| Unknown types | `CAST(x AS MYTYPE)` | B218 |

Derived-table column aliases, `UNPIVOT` and Snowflake `GROUPING SETS` are checked too.

Every check except B104 is a blocking error. The fix is always explicit SQL. For example, write
`order_id = CAST(customer_code AS INTEGER)`, or use the spelling the error suggests. There is no
per-model suppression.

### Type checks by dialect

| Dialect | Reference and semantic checks | Expression type checks | Function spellings |
|---|---|---|---|
| DuckDB (including MotherDuck) | Yes | Yes | Yes |
| Snowflake | Yes | Yes | Yes |
| PostgreSQL | Yes | Yes | No |
| BigQuery | Yes | Yes | No |
| Other adapters | Yes | No | No |

Coercion rules differ between dialects: DuckDB accepts a number as a predicate, while PostgreSQL,
Snowflake and BigQuery reject it. Typed string literals such as `ordered_at > '2026-01-01'` remain
valid.

### Snowflake details

- Unknown function names are not rejected, because Snowflake's built-in catalogue is incomplete. An
  unmodelled function's result type is unknown rather than guessed.
- `QUOTED_IDENTIFIERS_IGNORE_CASE = true` in the target's session parameters is honoured:

  ```toml
  [targets.analytics.connection]
  session_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }
  ```

- Set operations are checked in operator order (`INTERSECT` before `UNION` and `EXCEPT`).

## What isn't checked

Compilation doesn't replace running your SQL unit tests. These remain partly unchecked:

- **Expressions that use a project function's result.** Registering a function prevents
  unknown-function errors but doesn't declare its return type.
- **Raw relations SQLBuild doesn't manage,** such as tables written by hooks.
- **Function overloads.** Signature coverage is partial for every dialect.

Inputs with unknown types never cause type errors. Instead, `sqb compile` prints one notice listing
the affected models by reason, with the command to fix or rerun each. `sqb compile --json` gives
the full list in `semantic_checks_partial`.

## Reading a diagnostic

Each diagnostic shows, in order:

1. the code and headline;
2. the model, test, audit or hook it belongs to, with the file and position;
3. the source line, with the problem underlined;
4. notes, such as the input's available columns, closest first;
5. help: a suggested fix, such as the column you probably meant or the explicit cast to write.

When one error makes downstream columns uncertain, SQLBuild doesn't repeat it for each use. A note
counts the downstream uses that weren't checked. Independent errors elsewhere are still reported.

`sqb compile --json` carries the same fields, with exclusive `end_line` and `end_column`.

## Turning analysis off

To skip SQL analysis for one model, use `MODEL (sql_analysis false)`, a path default, or
`--no-sql-analysis` for a run. This also turns off type inference and lineage for that model.
To allow those opt-outs only for SQL the parser can't read, set
`[settings] require_sql_analysis = true`; see
[Requiring SQL analysis](models/configuration.md#requiring-sql-analysis).

## Diagnostic codes

| Code | Meaning |
|---|---|
| B002 | Unknown column |
| B003 | Ambiguous column |
| B004–B005 | Reference and scope errors |
| B101 | Unknown function, or a spelling the dialect doesn't accept |
| B102 | Wrong number of function arguments |
| B103 | Truth-value test the dialect doesn't accept |
| B104 | Warning: a SQLite `CAST` to a date or time type, which keeps only the leading number |
| B210–B219 | Type and shape errors the warehouse would raise |
| B211 | Implicit conversion in a predicate |
| B212 | Implicit conversion in arithmetic |
| B213 | Implicit conversion in a function argument |
| B214 | Implicit conversion in an assignment |
| B215 | Incompatible set-operation branches |
| B216 | Set-operation, subquery or row arity |
| B217 | Implicit conversion in a comparison |
| B218 | Implicit conversion in a cast or literal, or an unknown type |
| B230–B232 | Grouping, aggregate and window errors |
| B233 | Duplicate names in one scope |
| B234 | Invalid `LIMIT` or `OFFSET` |
| B300 | Unknown column in metadata or an audit |
| B301 | Incompatible declared metadata, audit or UDF argument type |
| B302 | Unknown SQL-test fixture or expected column |
| P001 | SQL the parser can't read, including in audits and tests |
