# DuckShop

DuckShop is the online shop of Mallard & Co., a fictional company that sells duck-themed home goods:
rubber ducks (collectible series and limited editions), duck-print textiles, decoys, pond and garden
supplies, and bath goods. It is written as a dense SQLBuild project in Snowflake SQL, and is a
realistic corpus for exercising the SQLBuild compiler, Rules, unit tests and scenarios, and for
testing translation from Snowflake SQL to other dialects.

The theme lives only in the data and descriptions: product lines named after duck species
(Mallard, Pintail, Teal, Eider, Merganser), warehouses such as The Pond and The Nest, and loyalty
tiers from Duckling to Golden Duck. Model, column and source names stay conventional, and the
business mechanics are those of any multi-region online shop.

Everything in it is synthetic: order references use a `DS-O-` prefix, email addresses use
`example.com`-style domains, and every brand, carrier and product name is invented.

## Layout

```text
benchmarks/duckshop/
├── project/      the SQLBuild project (Snowflake adapter), with its data loaders in python/loaders/
├── scripts/      tooling that is not part of the project: the construct census, cleanup SQL, the
│                 core-only copy and edge-case expectation capture
├── tests/        offline checks for the synthetic data loaders
└── RUNBOOK.md    loading, building, testing and capturing on Snowflake, and cleaning up
```

## Domain

- Storefront regions: EU (GBP and EUR), US (USD), APAC (JPY and AUD) and MENA (KWD), so currencies
  with 0, 2 and 3 minor-unit digits all occur. Warehouses: The Pond (`Europe/London`), The Nest
  (`America/New_York`) and The Marsh (`Australia/Sydney`).
- Raw tables are append-only change logs with `_op`, `_seq` and `_loaded_at`. Staging keeps the
  latest surviving version of each key with `QUALIFY ROW_NUMBER()`, through the `cdc_latest`
  CTE-list macro.
- Payloads arrive as `VARIANT`, `OBJECT` and `ARRAY` columns, plus raw JSON text that needs
  `TRY_PARSE_JSON`. Money is in integer minor units with an ISO currency. Timestamps arrive as
  `TIMESTAMP_TZ` with offsets, epoch milliseconds in JSON, warehouse-local `TIMESTAMP_NTZ` with an
  IANA zone, and one deliberately session-dependent `TIMESTAMP_LTZ`.

## Project shape

The core project, without the [edge-case area](#edge-cases):

- 252 models: 99 staging, 81 intermediate, 66 marts and 6 snapshots. Marts are grouped by area:
  core, orders, customers, catalogue, inventory, fulfilment, support, clickstream, growth,
  finance, partners and executive.
- 5 seeds, 2 SQL functions (a scalar UDF and a table function) and a Python UDF.
- SQL and Python macros, folder-scoped constants and enums, generic and singular audits, model
  schemas, hooks, incremental models with cursor windows, and snapshots with both strategies.
- Rules select `SQLCLEAN`, `SQBRGRAPH`, `SQBRCONTRACT`, `SQBRTEST` and the project's own
  `XSQBRDUCK` rules, with reviewed exceptions in `sqlbuild_project.toml`. Every model must have at least one unit test (`SQBRTEST202`).

## Tests and scenarios

`tests/unit/` mirrors `models/` and holds 266 core unit tests:

- 260 model tests, including cursor-window tests on incremental models, a table-function mock, a
  macro mock, a fixture built by a test-only Python macro, an empty fixture for a contracted
  source, and a file with two `TEST` blocks;
- 3 macro tests, 2 UDF tests and 1 table-function test, most driven by `VALUES` case tables;
- assertions in 98 tests, including a key check on every mart.

`tests/scenarios/` holds 8 core scenarios: one each for catalogue, clickstream, orders, fulfilment,
support and inventory, and two that cross areas (a regional scorecard, and campaign return on ad
spend). Two use `expected_columns "all"`. Expected values are derived by hand from the fixtures,
and every test and scenario passes on Snowflake (see [Status](#status)); the scenarios' captured
snapshots are in `tests/_scenario_snapshots/`. Unit tests and scenarios read only their own fixtures, so they do not depend on the
synthetic data, and capturing a scenario snapshots its fixtures rather than loaded tables.

```bash
cd benchmarks/duckshop/project
sqb test --inspect
```

## Synthetic data

Every raw source is a managed source with a Python loader in `project/python/loaders/`, run by
`sqb load` (or automatically by `sqb build`). The loaders generate one coherent world: about 300
customers, 120 SKUs and 1,200 orders between January and March 2026, with the payments, refunds,
shipments, returns, stock movements, clickstream sessions, support tickets, campaigns and partner
messages that follow from them, about 60,000 rows in all.

- Generation is deterministic: every table draws from its own stream seeded from a fixed seed, so
  the same code version always produces the same rows. The `data_scale` project variable
  multiplies volume (`"1"` by default).
- Change logs carry inserts, updates and deletes with `_op`, `_seq` and `_loaded_at`. Some versions
  arrive days late or out of order, and some are delivered twice.
- JSON payloads have missing keys, JSON nulls next to SQL NULLs, nested arrays, numbers sent as
  strings and odd precision. Partner order messages come in two payload versions, some with a
  JSON-null order id and the id nested elsewhere.
- Timestamps carry offsets from nine time zones, with orders, stock movements and delivery slots on
  the 2026 daylight-saving changes, including wall-clock times that do not exist. Web sessions run
  past midnight, and a few events arrive a day or two late.
- Support tickets and comments contain order references, phone numbers, email addresses and
  contact reasons for the regular-expression models to find.

`tests/test_synthetic_data.py` checks the loaders offline: determinism, the declared column shape
of every managed source, staging keys and audited columns, references between tables, and the edge
cases above. It needs only `pytest` and `pyyaml`:

```bash
uv run --isolated --no-project --with pytest --with pyyaml pytest benchmarks/duckshop/tests
```

See [RUNBOOK.md](RUNBOOK.md) for loading, building and testing on Snowflake.

## Edge cases

The edge-case area holds small, hand-picked cases for SQL whose results are easy to get subtly
wrong when translating Snowflake SQL or replaying it on another engine. It lives beside the core
project and does not change it:

- models in `models/staging/edge_cases/`, `models/intermediate/edge_cases/` and
  `models/marts/edge_cases/`, each tagged `edge_case`. The `XSQBRDUCK002` Rule fails a model in an
  `edge_cases` folder without the tag, or a model with the tag outside one;
- unit tests in the matching `tests/unit/<layer>/edge_cases/` folders and scenarios in
  `tests/scenarios/edge_cases/`;
- raw `edge__*` sources in `sources/edge_cases.yml`, loaded by `python/loaders/edge_cases.py` from
  fixed rows that `data_scale` does not change, so core data and core expectations stay the same.

It has 39 models (7 staging, 22 intermediate, 10 marts), 63 unit tests and 4 scenarios, in four
groups:

| Group | Covers |
|-------|--------|
| Semi-structured | JSON null against SQL NULL and missing keys; document text with sorted, case-differing and escaped keys; JSON number fidelity and casts out of `VARIANT`; path-like keys (`''`, `*`, `$`-prefixed, dotted) with `GET` and `GET_IGNORE_CASE`; `OBJECT_CONSTRUCT_KEEP_NULL`, `OBJECT_INSERT`, `OBJECT_DELETE`, `OBJECT_PICK`; array builders, `ARRAY_EXCEPT` and number spellings in `ARRAY_POSITION`; `FLATTEN` outer; lenient and malformed `TRY_PARSE_JSON`; typed lambdas |
| Dates and times | `TRY_TO_TIMESTAMP_NTZ` and its siblings over free text; epoch milliseconds as numbers and text; daylight-saving gaps and overlaps; fractional offsets; nanosecond truncation and epoch parts after 2242; pre-1970 text with sub-microsecond digits; date-part normalisation; month ends and leap days; week numbering across a year; sentinel timestamps |
| Ordering, numbers and text | NULL ordering under `DESC`; ties in ordered aggregates, `MAX_BY`, `MODE` and `ANY_VALUE`; default window frames; `UNION ALL BY NAME`; `ROUND` half-way values, decimal division, `MOD` and 38-digit amounts; integers past 2^53 against `FLOAT`; regular expressions and backslash escapes; `LIKE ... ESCAPE` and `ILIKE ANY`; `SPLIT_PART` and `SUBSTR` before the start; `NOT IN` with a NULL; names in many scripts |
| Scenarios and lifecycle | typed values across model boundaries; `VARIANT` expected output; a star under a lower-case contract and a quoted camelCase export; a snapshot keyed and compared on quoted camelCase columns; a cursor-window incremental model over far-apart fixture loads; a change log merged across two loads, checked against a full rebuild |

Unit-test expectations in the edge-case area were captured on Snowflake under the `edge` target's
pinned session, rather than derived by hand; scenario expectations, and the assertions in tests
about ties, are written by hand from those captured values. `fct_edge_delivery_events` is disabled: it is the
acceptance case for cursor `delete_insert` models that emit rows outside the window they replace.
See [RUNBOOK.md](RUNBOOK.md), step 7.

Benchmarks and core measurements leave the area out:

- `python benchmarks/duckshop/scripts/core_project.py <destination>` copies the project without
  it. The core-copy compile guard compares the copy's compiled files with a recorded checksum
  (`core_project.py --checksum`), so a change to the core project's compiled SQL fails the guard
  until the checksum is updated on purpose and noted for the benchmark;
- warehouse runs exclude it with `--exclude tag:edge_case "source:edge__*"`, and scenario runs
  with `--exclude edge_cases`;
- the construct census counts the core project unless given `--area edge` or `--area all`.

Changes to the core project's compiled SQL, after which benchmark results are not comparable with
earlier ones:

- `int_cart_funnel`, `int_delivery_slot_utilisation` and `rpt_delivery_exceptions_monthly` give the
  same results on every run: added SKUs are sorted, the weekday average utilisation is an exact
  decimal sum over a count, the busiest slot of equally busy ones is the earliest, and the top
  exception code of equally frequent ones is the first in alphabetical order.

## Status

Last verified on Snowflake with SQLBuild 0.175.0 at `data_scale = "1"`:

| Step | Result |
|------|--------|
| `sqb load` | All 76 managed sources load |
| `sqb build` | Passes; one warn-severity audit (`orders_balance_to_payments`) reports findings |
| `sqb test` | 266 of 266 pass |
| `sqb scenario test` | 8 of 8 pass |
| `sqb scenario capture` | Captures all 8; snapshots are in `project/tests/_scenario_snapshots/` |
| `sqb scenario test --local --strict --exclude edge_cases` | 0 of 8 pass: 5 fail in translation, 3 report a stale snapshot |

Local replay translates each scenario's Snowflake SQL to DuckDB on a best-effort basis. These five
failures are the baseline that better translation should remove:

| Scenario | Model | Construct that fails |
|----------|-------|----------------------|
| `category_tree__rolls_sales_up_built_paths` | `stg_categories` | `OBJECT_KEYS` is passed through unchanged |
| `cart_funnel__assigns_furthest_stage_per_session` | `int_cart_funnel` | `::ARRAY` cast of a lambda result becomes an untyped `CAST(... AS ARRAY)` |
| `region_scorecard__combines_sales_and_recognised_revenue` | `rpt_daily_sales_by_region` | `TO_VARCHAR` is passed through unchanged |
| `support_sla__weekly_breaches_and_monthly_reasons` | `rpt_contact_reasons_monthly` | `TO_VARCHAR` is passed through unchanged |
| `stock_outs__finds_longest_run_per_week` | `rpt_stock_outs_weekly` | `DATEADD('day', <window function>, ...)` becomes an invalid `(...) DAY` interval |

The other three scenarios (`campaign_roas__attributes_orders_to_channel_spend`,
`carrier_performance__weekly_rates_from_shipment_facts` and
`payments__fully_refunded_orders_collect_nothing`) never reach translation: their snapshots are
reported stale straight after capture. The snapshot fingerprint now hashes the authored fixture
SQL, so these results predate that fix; recapture the snapshots to replay them.

The project no longer sets `QUOTED_IDENTIFIERS_IGNORE_CASE = true` on its connection. With SQLBuild
0.184.0, a core build without it passes with the same results as with it: every core model holds
the same rows, apart from the snapshots, which record build times, and three models whose results
vary from one run to the next (`int_cart_funnel`, `int_delivery_slot_utilisation` and
`rpt_delivery_exceptions_monthly`).

The edge-case area, verified on Snowflake with SQLBuild built after 0.185.0 on the `edge` target:

| Step | Result |
|------|--------|
| `sqb build --select tag:edge_case` | Passes; the disabled model is skipped |
| `sqb test --select tag:edge_case` | 62 of 62 pass |
| `sqb scenario test --select edge_cases` | 4 of 4 pass |
| Two-load replay of the delivery change log | The merged status equals the full rebuild after each load |

## Expected divergences

Nine models use constructs whose results depend on the engine or the session. They are tagged
`known_divergence` and say so in their descriptions, so translation and replay tools can report
them as expected differences or refusals instead of errors:

| Construct | Models |
|-----------|--------|
| `HASH` | `int_customer_identity`, `dim_customers`, `fct_web_sessions` |
| `TIMESTAMP_LTZ` | `stg_tickets`, `int_ticket_first_response`, `rpt_customer_order_lookup` |
| `CURRENT_TIMESTAMP()` | `int_promotion_performance`, `int_ticket_sla_breaches`, `rpt_customer_order_lookup` |

## Known limitations

These SQLBuild limitations shape parts of the project; each workaround is a reviewed exception or
noted here:

- Snowflake does not support concurrent microbatch `delete_insert` batches (`P001`), so
  `int_web_sessions` runs its batches one at a time.
- Self-managed loaders always report `rows=0`, since SQLBuild does not see the rows they insert.
- Compilation accepts SQL that Snowflake rejects: `CONNECT_BY_ISLEAF`, which Snowflake does not
  have, and a CTE whose body is a bare `VALUES` list. `int_category_paths` finds leaf categories
  with a join instead, and test case tables use `SELECT ... FROM VALUES ... AS v(...)`.
- On Snowflake, a model that names a column in quoted mixed case, such as `"unitPrice"`, loses the
  static types of the columns it reads from a CTE through a table alias (`FROM listings AS l`), so
  `rpt_edge_partner_listing_export` declares only the name and audits of `"listingId"`, without
  types.
- `SQLCLEAN018` can only take proof from single-column keys. Append-only feeds redeliver rows, so
  their id is unique only together with `_loaded_at` (the `unique_per_delivery` audit), and the
  staging models that keep the latest delivery carry reviewed exceptions.

## Compile

Compiling needs no warehouse connection:

```bash
cd benchmarks/duckshop/project
sqb compile
sqb format --check
```

CI compiles a copy of the project with the SQLBuild built from the same pull request and fails on
any error or warning (`tests/e2e/src/sqlbuild/cli/commands/main/compile/test_duckshop_benchmark_compile.py`).

`sqlbuild_project.toml` holds placeholder connection values only. To run against a warehouse, copy
`sqlbuild_local.toml.example` to `sqlbuild_local.toml` (gitignored) and fill in your own details.

## Construct census

After compiling, measure how often each construct family occurs, per layer:

```bash
python benchmarks/duckshop/scripts/census.py
```
