<!-- generated-by: sqlbuild skills -->

# Unit tests

> Check a model's logic on rows you write, by mocking its inputs and comparing its output.

Online: https://sqlbuild.com/docs/concepts/testing/

## Contents

- How tests work
- Test files
- CTE conventions
- Multi-model tests
- Mocking refs and seeds
- Fixture columns
- Mocking table functions in model tests
- Multiple expected models
- Macros, enums and constants in tests
- Macro mocking
- Assertions
- Running tests
- More test features

A unit test checks a model's logic on rows you write. You mock the model's inputs with CTEs,
SQLBuild runs the model's real SQL over them, and the test passes when the output matches the rows
you expect:

```sql
-- tests/unit/test_stg_orders.sql
TEST();

WITH
__source__raw__orders AS (
  SELECT
    1 AS id,
    100 AS customer_id,
    2 AS waffle_type_id,
    3 AS quantity,
    '2026-04-01 10:00:00' AS ordered_at,
    'completed' AS status
),
__expected__stg_orders AS (
  SELECT
    1 AS order_id,
    100 AS customer_id,
    2 AS waffle_type_id,
    3 AS quantity,
    '2026-04-01 10:00:00' AS ordered_at,
    'completed' AS status
)
```

Tests can chain several models, mock models, seeds, macros and table functions, assert properties
that must hold for every row, and test macros, UDFs and table functions directly. For end-to-end
tests over physical warehouse relations, see [Scenarios](scenarios.md).

## How tests work

The test above:

1. mocks the `raw__orders` source with the `__source__raw__orders` CTE;
2. runs the real `stg_orders` SQL with the mock in place of the source;
3. compares the output with `__expected__stg_orders` in both directions;
4. passes when the row counts match and no row differs.

### The `TEST` header

Every test starts with a `TEST()` header. All of its fields are optional: `TEST();` is a complete
header, and a test without a `name` is named after its file. Fields name the test, set its
[mode](testing-reference.md#test-modes) or [cursor window](testing-reference.md#cursor-windows-in-tests), declare
[parameters](testing-reference.md#native-independent-cases), set a [time limit](testing-reference.md#time-limits), or
[skip SQL analysis](testing-reference.md#skipping-sql-analysis).

A test ends after its last CTE. A trailing `SELECT 1` is accepted; nothing else may follow the last
CTE.

### Failure output

When the output doesn't match, SQLBuild reports how many rows were unexpected and how many were
missing, with samples of each. Samples are limited to three rows, 12 columns and 120 characters per
value, and are redacted like other diagnostics. JSON output has them as `unexpected_samples` and
`missing_samples`. If sampling fails, the test still fails with its counts.

When exactly one row differs in each direction and both samples have the same columns, SQLBuild
lines the two rows up and shows only the changed columns, with their actual and expected values.
With several differing rows it doesn't guess which rows belong together.

`target/run/tests/` keeps the complete comparison query of each test, which you can run by hand.

### Fixture checks

When SQL analysis is on, SQLBuild checks fixture shapes before it connects to the warehouse:

- Missing columns are reported together, with the test path, the fixture and the models that read
  them.
- A collection value where a scalar is expected, or the reverse, names the column and suggests an
  explicit `CAST`, `ARRAY_CONSTRUCT` or `PARSE_JSON`.

SQLBuild never invents missing values, except the typed nulls described in
[Fixture columns](#fixture-columns).

## Test files

Put unit tests under `tests/unit/`. SQLBuild finds every `.sql` file there, recursively.

```
tests/
  unit/
    test_stg_orders.sql              # model test
    test_fact_orders.sql              # model test with assertion
    test_daily_revenue_chain.sql      # multi-model test
    test_line_total_cents_macro.sql   # macro test
    test_is_completed_order_udf.sql   # UDF test
    test_customer_orders_table_fn.sql # table function test
  scenarios/
    ...
```

### Multiple tests per file

A file can hold several `TEST()` blocks. Each block in such a file needs its own `name`; a file with
one test can leave it out.

```sql
TEST (name "completed_orders_only");

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 'completed' AS status
),
__expected__stg_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 'completed' AS status
)

TEST (name "cancelled_orders_excluded");

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 'cancelled' AS status
),
__expected__stg_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 'cancelled' AS status
)
```

Test names are unique across the project: a test can't share a name with another test, a scenario,
a model, a source, a seed, a function or a Python node. Name a test after the behaviour it checks,
such as `orders_status_rules`, rather than after the model. The cases of one
[parameterized test](testing-reference.md#native-independent-cases) share its name.

## CTE conventions

A CTE's prefix says what it does:

| Prefix | Purpose |
|--------|---------|
| `__source__<name>` | Mock a source. Replaces `__source("<name>")` in the model SQL |
| `__ref__<name>` | Mock a model. Replaces `__ref("<name>")` in the model SQL |
| `__seed__<name>` | Mock a seed. Replaces `__seed("<name>")` in the model SQL |
| `__table_fn__<name>` | Mock a table function. Replaces the whole `__table_fn("<name>")(...)` call |
| `__expected__<name>` | Expected output of a model, compared in both directions on the listed columns |
| `__assert__<name>` | Assertion. Passes when the query returns no rows; returned rows are reported |
| `__macro__<name>` | Mock a macro. Replaces every `@<name>(...)` call with the mock value |
| any other name | Helper CTE, shared by the test's mocks, helpers, `__expected__` and `__assert__` CTEs |

An `__expected__<model>` CTE compares only the columns it lists, matched by name. Column order
doesn't matter and unlisted model columns are ignored, but row counts are always compared. Listing
a column the model doesn't output is an error.
[Scenario](scenarios.md#scenario-format) expected output is compared the same way.

### Helper CTEs

Models never see helpers, so a helper can share a name with a model's CTE or a table. Helpers are
emitted as `__helper__<name>`, so `__helper__` names are reserved. A CTE inside another test CTE
must not redefine a helper's name, and a model CTE must not use a name SQLBuild generates for the
test query, such as `__ref__orders` or `__helper__expected_rows`.

Helpers can read other helpers and mocks by CTE name, such as `__ref__stg_orders`. Shared expected
rows can therefore live in one helper that an `__expected__` CTE and an assertion both read:

```sql
TEST();

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 3 AS quantity UNION ALL SELECT 2 AS id, 5 AS quantity
),
expected_orders AS (
  SELECT 1 AS order_id, 3 AS quantity UNION ALL SELECT 2 AS order_id, 5 AS quantity
),
__expected__stg_orders AS (
  SELECT order_id, quantity FROM expected_orders
),
__assert__no_unexpected_order_ids AS (
  SELECT order_id FROM __ref("stg_orders")
  EXCEPT
  SELECT order_id FROM expected_orders
)
```

SQLBuild emits each helper and mock once, in dependency order, after the mocks and models it reads.
A helper that no `__expected__` or `__assert__` CTE reaches, directly or through other helpers and
mocks, is left out of the test query, and its references are neither checked nor run.

### References in test CTEs

`__expected__` and `__assert__` CTEs, and the helpers they read, can call `__ref()`, `__source()`,
`__seed()` and `__dbt_ref()`. They resolve the same way everywhere in a test:

- **A mocked name means the mock.** `__ref("stg_orders")` reads `__ref__stg_orders` when the test
  defines it. A mocked model isn't run, and `sqb test --select stg_orders` doesn't select the test
  because of it.
- **An unmocked `__ref("<model>")` runs the model's real SQL,** and `sqb test --select <model>`
  selects the test, even when only a helper reads it.
- **`__source()`, `__seed()` and `__dbt_ref()` must be mocked,** for example with
  `__source__raw__orders AS (SELECT ...)`.

Mocks are defined before the models a test runs, so a mock reads another mock by CTE name, such as
`FROM __ref__stg_orders`, not through `__ref("stg_orders")`:

```sql
TEST();

WITH
__ref__stg_orders AS (
  SELECT 1 AS order_id, 3 AS quantity
),
order_totals AS (
  SELECT order_id, quantity FROM __ref("orders")
),
__assert__quantities_are_positive AS (
  SELECT order_id FROM order_totals WHERE quantity <= 0
)
```

Compile rejects references the test query can't resolve with `P013`, at the CTE or call that causes
it:

- a `__ref()` to a model that doesn't exist;
- an unmocked `__source()`, `__seed()` or `__dbt_ref()`;
- helpers or mocks that read each other in a cycle;
- a mock or `__table_fn__` fixture the test uses that reads a helper calling a reference.

A malformed reference call in a helper or expected CTE, such as `__source("raw", "orders")`, is a
`P012` error at the call. Reference calls are found outside comments and quoted text, following the
project adapter's quoting and comment rules.

### CTE syntax

SQLBuild reads test CTEs strictly. Other forms are compile errors that name the CTE:

| Rule | Error example |
|------|---------------|
| A CTE name is an unquoted ASCII identifier: letters, digits and underscores, not starting with a digit | `"recent orders"`, `` `recent orders` ``, names with `$` or non-ASCII characters |
| The CTE body follows `AS` directly | `AS MATERIALIZED (...)`, `AS NOT MATERIALIZED (...)` |
| Every column of an expected CTE has a name | `amount IS NULL` or `CASE ... END` without an alias |
| An expected CTE is a plain `SELECT`, optionally combined with `UNION`, `INTERSECT` or `EXCEPT` | Parenthesized branches, a `WITH` inside the CTE |
| An expected CTE lists its columns | `SELECT *`, `SELECT DISTINCT *`, `t.*` |

Keywords such as `WITH`, `AS` and `SELECT` match in any case. Expected CTEs are
`__expected__<model>`, `__macro_expected__`, `__udf_expected__` and `__table_fn_expected__`. A
column reference such as `order_id` or `o.order_id` names itself, and an alias without `AS`, such as
`1 order_id`, names the column too. For shared rows, use a helper CTE instead of a nested `WITH`.
The one `SELECT *` allowed is `SELECT * FROM __empty_fixture()` in a contracted `__expected__` CTE.

## Multi-model tests

A test can span several models. Mock the sources, give the expected output of the model you care
about, and SQLBuild runs every model in between from its real SQL:

```sql
-- Mock two sources, assert on the final mart.
-- stg_orders and stg_payments resolve automatically from their real SQL.
TEST();

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 2 AS waffle_type_id, 3 AS quantity,
         '2026-04-01 10:00:00' AS ordered_at, 'completed' AS status
),
__source__raw__payments AS (
  SELECT 1 AS payment_id, 1 AS order_id, 1500 AS amount_cents,
         'credit_card' AS method, '2026-04-01 10:01:00' AS paid_at, 'success' AS status
),
__expected__fact_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 1500 AS payment_amount_cents,
         'credit_card' AS payment_method
)
```

Inspect what a test will run without connecting to the warehouse:

```bash
sqb test --select fact_orders --inspect
```

The plan lists the mocked sources, refs and seeds, the real models in execution order, the expected
models, and any unsatisfied leaf dependencies. A mocked ref is called out, because its real SQL
won't run. Inspection exits non-zero when the plan has unresolved dependencies.

Each real model adds to the cost of the test; see [Cost guards](testing-reference.md#cost-guards).

## Mocking refs and seeds

Mock a model with `__ref__<name>` or a seed with `__seed__<name>`. The model's SQL, or the seed's
CSV data, is then replaced by your rows:

```sql
TEST();

WITH
__ref__stg_orders AS (
  SELECT
    1 AS order_id,
    100 AS customer_id,
    2 AS waffle_type_id,
    3 AS quantity,
    CAST('2026-04-01 10:00:00' AS TIMESTAMP) AS ordered_at,
    'completed' AS status
),
__ref__stg_payments AS (
  SELECT
    10 AS payment_id,
    1 AS order_id,
    2850 AS amount_cents,
    'card' AS payment_method,
    CAST('2026-04-01 10:05:00' AS TIMESTAMP) AS paid_at,
    'success' AS payment_status
),
__seed__waffle_types AS (
  SELECT
    2 AS waffle_type_id,
    'Liege' AS waffle_name,
    'sweet' AS category,
    950 AS price_cents
),
__expected__fact_orders AS (
  SELECT
    1 AS order_id,
    100 AS customer_id,
    2 AS waffle_type_id,
    'Liege' AS waffle_name,
    'sweet' AS waffle_category,
    3 AS quantity,
    2850 AS line_total_cents,
    CAST('2026-04-01 10:00:00' AS TIMESTAMP) AS ordered_at,
    'completed' AS order_status,
    TRUE AS is_completed_order,
    'card' AS payment_method,
    'success' AS payment_status,
    2850 AS payment_amount_cents
)
```

`__source__`, `__ref__` and `__seed__` mocks can be mixed. The test resolves as long as every leaf
dependency is mocked or is in the expected chain.

## Fixture columns

A fixture can leave out a column the test needs when SQLBuild knows the column's type and the column
isn't declared `nullable false`. SQLBuild fills it with a typed null, such as
`CAST(NULL AS VARCHAR)`, in the test only; it never changes model SQL or warehouse defaults.
Supply these columns yourself:

- columns declared `nullable false`;
- columns whose type is unknown.

When a relation's full column list is known, a misspelled or unknown fixture column is rejected.

A `NULL AS column_name` projection gets the relation's declared type in the compiled test SQL,
including in expected CTEs for contracted models. To mock a contracted relation with no rows, write
`SELECT * FROM __empty_fixture()` as the whole body of a `__ref__`, `__source__` or `__seed__`
fixture, or of a contracted `__expected__` CTE. SQLBuild expands it to the relation's typed columns
with a false filter.

## Mocking table functions in model tests

Use `__table_fn__<name>` when the model under test reads a managed table function and the test
should supply the rows. SQLBuild replaces the whole call, arguments included, with the fixture. The
mocked function isn't deployed for the test.

```sql
-- models/customer_order_totals.sql reads:
-- __table_fn("customer_orders")(42)
TEST();

WITH
fixture_orders AS (
  SELECT 101 AS order_id, 42 AS customer_id, 2500 AS amount_cents
),
__table_fn__customer_orders AS (
  SELECT order_id, customer_id, amount_cents FROM fixture_orders
),
__expected__customer_order_totals AS (
  SELECT 42 AS customer_id, 2500 AS total_cents
)
```

The fixture name must match a declared table function. Fixtures can read helper CTEs, and an
assertion can read a table-function fixture directly. To test the table function itself, use a
[table function test](testing-reference.md#table-function-tests).

## Multiple expected models

One test can check several models. Each is compared on its own:

```sql
TEST();

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 2 AS waffle_type_id, 3 AS quantity,
         '2026-04-01 10:00:00' AS ordered_at, 'completed' AS status
),
__source__raw__payments AS (
  SELECT 1 AS payment_id, 1 AS order_id, 1500 AS amount_cents,
         'credit_card' AS method, '2026-04-01 10:01:00' AS paid_at, 'success' AS status
),
__source__raw__customers AS (
  SELECT 100 AS id, 'Leslie' AS first_name, 'Knope' AS last_name,
         'leslie@pawnee.gov' AS email, '2026-01-15 09:00:00' AS created_at
),
__expected__fact_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 1500 AS payment_amount_cents,
         'credit_card' AS payment_method
),
__expected__dim_customers AS (
  SELECT 100 AS customer_id, 1 AS total_orders, 1500 AS lifetime_spend_cents
)
```

When the expected models form a chain, such as `stg_orders` to `fact_orders` to `dim_customers`,
they run in dependency order, each reading the output of the one before.

## Macros, enums and constants in tests

Test SQL can call macros, so mock data can come from a reusable generator:

```sql
TEST();

WITH
__source__raw__orders AS (
  @py.mock_orders()
),
__source__raw__payments AS (
  SELECT 1 AS payment_id, 1 AS order_id, 1500 AS amount_cents, 'credit_card' AS method
),
__expected__fact_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 1500 AS total_cents,
         'credit_card' AS payment_method
)
```

`@py.mock_orders()` expands at compile time to the SQL the Python macro returns.

A test can use:

- the macros, enums and constants visible from its directory under `tests/unit/`;
- the public enums and constants visible to each model it defines `__expected__` output for.

Only an `__expected__` CTE grants that access. A matching file name, a `__ref__` mock or a nearby
model path doesn't. Model-private values and macros are never available to tests. See
[How Visibility Works](declaration-scopes/visibility.md#tests-and-expected-output).

A macro used only by tests must be private to them. Put it in a `_sqlbuild/_macros/` folder among
the tests, such as `tests/unit/orders/_sqlbuild/_macros/`, or in the `_sqlbuild/_macros/` of a model
the tests define `__expected__` output for. The project `macros/` directory rejects a test-only
macro with `S024`, which names the folder to use, and `tests/unit/_sqlbuild/` is rejected with
`D013`.

## Macro mocking

To control a macro's output in a test, such as target-specific logic or generated SQL, mock it with
a `__macro__<name>` CTE:

```sql
TEST();

WITH
__macro__country_filter AS (
  SELECT 'country_code = ''US'''
),
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 'US' AS country_code, 'completed' AS status
),
__expected__stg_orders AS (
  SELECT 1 AS order_id, 100 AS customer_id, 'completed' AS status
)
```

Every `@py.country_filter(...)` call in the model SQL the test runs is replaced with
`country_code = 'US'`. The macro isn't called, and its arguments are ignored.

- The mock is a single `SELECT` of one string literal, optionally named with `AS`. Escape quotes by
  doubling them.
- [SQL macros](macros/sql-macros.md) are mocked the same way: `__macro__net_amount`
  replaces each `@sql.net_amount(...)` call written in model SQL. A call inside another SQL macro's
  body is expanded as usual.
- Macro names are unique across both kinds, so the mock name has no `sql.` or `py.` prefix.

## Assertions

An `__assert__<name>` CTE checks a property. It passes when it returns no rows; each returned row is
a failing example.

```sql
TEST();

WITH
__source__raw__orders AS (
  SELECT 1 AS id, 100 AS customer_id, 2 AS waffle_type_id, 3 AS quantity,
         '2026-04-01 10:00:00' AS ordered_at, 'completed' AS status
),
__assert__order_ids_are_not_null AS (
  SELECT * FROM __ref("stg_orders") WHERE order_id IS NULL
)
```

Use assertions when the natural check is "no row breaks this rule", such as duplicates, negative
values or conditional business rules. They can be mixed with `__expected__` CTEs or used alone.
`sqb test` and `sqb build` show each assertion as a nested check row.

An assertion must read a model the test runs. A test whose checks read only mocks, such as
`__ref("stg_orders")` in a test that also defines `__ref__stg_orders`, has nothing to run, and
compile rejects it with `P013`. Mock the model's inputs instead, as above, or remove the test.

## Running tests

Tests run during `sqb build` in DAG order, before the model they test is built. Run them on their
own with `sqb test`:

```bash
sqb test
sqb test --select stg_orders
```

## More test features

[Testing reference](testing-reference.md) covers macro, UDF and table function tests,
parameterized cases, cursor windows, empty-input tests, cost guards and time limits.
