# DuckShop runbook

How to load DuckShop's synthetic data into Snowflake, build and test the project, capture the
scenario snapshots, run the edge-case area, and clean up afterwards. Run every command from
`benchmarks/duckshop/project` unless it says otherwise.

Steps 3 to 6 run the core project only: they exclude the edge-case area (models tagged
`edge_case`, their `edge__*` sources and the `edge_cases` scenario folder), which step 7 runs on its
own target.

## 1. Configure the connection

Install SQLBuild with Snowflake support (`pip install "sqlbuild[snowflake]"`), then create the
untracked local configuration:

```bash
cd benchmarks/duckshop/project
cp sqlbuild_local.toml.example sqlbuild_local.toml
```

Fill in the account, user, authenticator, role and warehouse, and the `database` and `schema` of
`[targets.dev]`. For the edge-case area (step 7), fill in `[connections.edge]` and `[targets.edge]`
the same way, with a different schema from `[targets.dev]`: two targets that write managed sources
to the same schema fail with `S102`. `sqlbuild_local.toml` adds to the connection tables in
`sqlbuild_project.toml` rather than replacing them, so the edge connection keeps the project's
pinned `session_parameters`; leave `session_parameters` out of the local file.
`sqlbuild_local.toml` is gitignored; never commit it, and never copy its values into tracked
files, commit messages or pull requests.

Check the connection, then confirm the project compiles:

```bash
sqb debug
sqb compile
```

`sqb compile` must report `0 errors, 0 warnings`.

## 2. Check the schema before the first build

A build creates or replaces about 340 objects in the target schema: 252 models, 5 seeds, 76 raw
source tables and 3 functions. If the schema holds other work, make sure none of it has a DuckShop
name:

```bash
sqb compile --json > target/duckshop-compile.json
python ../scripts/relations.py check target/duckshop-compile.json > target/check.sql
sqb query --file target/check.sql --no-limit
```

The query lists every existing table, view or function with a DuckShop name, which is everything
step 8 drops, and any SQLBuild state tables (`_SQLBUILD_*`). Before the first build it should list
no DuckShop names; if it does, stop and pick another schema. Note which state tables already existed, for the cleanup in step 8.
The list includes the edge-case area's names; run the check with `--target edge` too before its
first build.

The project no longer sets `QUOTED_IDENTIFIERS_IGNORE_CASE = true` on its connections. A schema
built while it did may hold objects and SQLBuild state that no longer resolve under Snowflake's
default case rules, so rebuild into a clean schema: drop everything as in step 8, or pick a new
schema, before the first build.

## 3. Load the synthetic data

```bash
sqb load --concurrency 4 --exclude "source:edge__*"
```

All 76 managed sources load, about 60,000 rows in total. Each loader replaces its table, so loading
again gives identical data. The volume multiplier is the `data_scale` project variable; for a larger
run, set it in `sqlbuild_local.toml`:

```toml
[vars]
data_scale = "10"
```

## 4. Build

```bash
sqb build --concurrency 8 --exclude tag:edge_case "source:edge__*"
```

The build reloads the managed sources first. SQLBuild 0.175.0 does this even with `--no-load`
when no `--select` is given, because the default selection includes the sources themselves; the
reload writes the same rows as step 3, so it only costs time. The first build replays the
clickstream microbatch models from the start of their history, about 90 six-hour batches, so it
takes longer than later builds.

Audits run during the build. Error-severity failures stop the dependent models and fail the
command. A few warn-severity audits, such as `orders_balance_to_payments` (orders part-paid with
gift cards) and the buyer match rate on partner orders, may report findings without failing.

### Rebuilding after a model change

Incremental models are forward-only by default: when a model's SQL changes, rows already built keep
their old results and only new cursor ranges use the new query, and the plan prints a warning that
says so. Each model's `replay_on_change` setting decides how much history its own change
reprocesses. Upstream changes never replay a downstream model.

DuckShop's history is small and fixed (January to March 2026 at every `data_scale`), so its
incremental models replay all of it when they change:

| Model | Policy |
|-------|--------|
| `stg_web_events`, `int_web_sessions` (microbatch) | `replay_on_change full` |
| `fct_web_sessions`, `fct_support_tickets`, `fct_inventory_daily` | `replay_on_change bounded-120d`, which covers all synthetic history |
| `stg_app_events` (append) | forward-only; replaying an append model would duplicate rows |

`full` is not used for the three non-microbatch models because a full rebuild of a model that
filters on `__cursor_start()` needs an explicit interval. To reprocess a model by hand, for example
after an upstream change or for `stg_app_events`:

```bash
# Microbatch models
sqb build --select stg_web_events --full-refresh
# Models that filter on cursor intrinsics without microbatches: give the interval explicitly
sqb build --select fct_web_sessions --full-refresh \
  --start-cursor-ts 2026-03-01T00:00:00 --end-cursor-ts 2026-04-02T00:00:00
```

## 5. Run the unit tests and scenarios on Snowflake

```bash
sqb test --concurrency 8 --exclude tag:edge_case
sqb scenario test --concurrency 4 --exclude edge_cases
```

Unit tests and scenarios read only their own fixtures, not the loaded data, so they can run before
or after the build. Their expected values were derived by hand; all 266 tests and 8 scenarios pass
on Snowflake. Report any failure rather than editing the expectations to match.

## 6. Capture the scenario snapshots and commit them

```bash
sqb scenario capture --exclude edge_cases
sqb scenario test --local --strict --exclude edge_cases
```

Capture materialises each scenario's fixtures in Snowflake, downloads them as JSONL into
`tests/_scenario_snapshots/`, and drops what it created. The local run then replays all 8
scenarios on your machine from those files, with no warehouse connection; `--strict` fails on a
missing or stale snapshot instead of skipping it.

With SQLBuild 0.175.0 the local run is expected to end with `ERROR=8`: five scenarios fail while
translating Snowflake SQL to DuckDB, and three report `X602` (stale snapshot) because of a
SQLBuild fingerprint issue. Both are described in the README's status section. Capturing again does
not clear the `X602` errors.

The snapshots hold only the synthetic fixture rows and column types, but check them before
committing. From the repository root:

```bash
grep -ril "<your schema>\|<your account>" benchmarks/duckshop/project/tests/_scenario_snapshots
git add benchmarks/duckshop/project/tests/_scenario_snapshots
git commit -m "test(benchmarks): capture DuckShop scenario snapshots"
```

The `grep` must print nothing. The repository's publication checks run on commit and push as usual.

## 7. Edge cases

The edge-case area runs on the `edge` target, whose connection pins the session its expected
values were captured under: UTC, and Snowflake's defaults for every parameter that changes a
result or how text is parsed, including `QUOTED_IDENTIFIERS_IGNORE_CASE = false`. Run it on that
target only; under another session some expected values differ by design.

`DEFAULT_DDL_COLLATION` also changes results (comparisons, ordering and `LIKE` on text columns),
but it is an account, database or schema parameter, not a session one, so the connection cannot
pin it. The edge target's schema, and its database and account, must leave it at the default (no
collation). Check with `SHOW PARAMETERS LIKE 'DEFAULT_DDL_COLLATION' IN SCHEMA <database>.<schema>`.

### Load, build and test

```bash
sqb build --target edge --select tag:edge_case --concurrency 8
sqb test --target edge --select tag:edge_case --concurrency 8
sqb scenario test --target edge --select edge_cases
```

The build loads the `edge__*` sources from fixed rows that `data_scale` does not change, creates
the area's Python functions (`functions/python/edge_*.py`, tagged `edge_case`), then builds the edge
models and runs their tests and audits. `fct_edge_delivery_events` is disabled and reports
`skip: disabled` (see [Pending acceptance cases](#pending-acceptance-cases)).

`int_edge_basket_running_peaks` calls `edge_running_peaks`, a JavaScript function its pre-hook
creates in the target schema. SQLBuild does not declare it, so the model's unit test needs a build
of that model first, and the model calls the function without a schema: set `database` and
`schema` on the edge connection as well as on the target, so that the session's current schema is
the target schema. `relations.py drop` drops the function too.

`int_edge_ticket_tokens` calls volatile Python functions, so its values change on every run; its
test compares only the deterministic columns and asserts facts that hold for any values, such as
every token being distinct.

### Replay the delivery change log as two loads

`int_edge_delivery_status` merges a shipment delivery change log incrementally, and the
`edge_delivery_status_matches_rebuild` audit fails if it differs from a rebuild from the whole log.
To exercise the merge across loads, start without the table, load the first load only, build, then
load everything and build again:

```bash
sqb query --target edge "DROP TABLE IF EXISTS <database>.<schema>.int_edge_delivery_status"
sqb load --target edge --select edge__delivery_updates --vars '{"edge_delivery_batch": "1"}'
sqb build --target edge --no-load \
  --select int_edge_delivery_update_versions int_edge_delivery_status int_edge_delivery_status_rebuilt
sqb load --target edge --select edge__delivery_updates
sqb build --target edge --no-load \
  --select int_edge_delivery_update_versions int_edge_delivery_status int_edge_delivery_status_rebuilt
```

The second build plans `int_edge_delivery_status` as a `merge` over the second load, which carries
a late older version, a redelivered version, a delete and a new shipment, and the audit passes.
Load with `sqb load` and build with `--no-load`, so that each build reads exactly the load you
chose.

### Capture expected values

Unit-test expectations in the edge-case area are Snowflake's own results on the fixtures, not
values derived by hand. To capture them for a new or changed edge test, write its `__expected__<model>` CTE with the
columns to compare (any values, such as `NULL AS column_name`), then run:

```bash
python ../scripts/capture_edge_expectations.py --write <test name> ...
sqb format tests/unit/staging/edge_cases tests/unit/intermediate/edge_cases tests/unit/marts/edge_cases
```

The script runs the edge tests on the `edge` target, reads each selected test's executed SQL, and
replaces each expected CTE with the actual rows as typed literals. Without a test name it
recaptures every edge test, which must leave the files unchanged. Check every captured value
against the case the test describes before committing: a capture records what Snowflake returned,
which is the point, but a fixture mistake is captured as faithfully as a real edge. Scenario
expectations, and the assertions in tests about ties (where Snowflake may return any of the tied
rows), are written by hand from the captured values.

Some edge models read values whose order Snowflake does not define, such as an `ARRAY_AGG`
without `WITHIN GROUP` or a `MAX_BY` whose keys tie. They output only order-free facts, and a
comment in the model or test says so; leave any column that can still differ between runs out of
the expected CTE and check it with an assertion instead. Run the capture twice on such tests: the
second run must leave the files unchanged.

Capture scenario snapshots for the edge scenarios with
`sqb scenario capture --target edge --select edge_cases`.

### Pending acceptance cases

`fct_edge_delivery_events` filters its input by order date but is rebuilt by delivery date, so a
rerun of a window deletes only that window's deliveries and inserts deliveries outside it again.
It stays disabled (`enabled false`) until cursor `delete_insert` models guard rows outside the
window; its unit test records the rows outside the window that the model emits today.

## 8. Clean up

Drop everything the project created, by name:

```bash
sqb compile --json > target/duckshop-compile.json
python ../scripts/relations.py drop target/duckshop-compile.json > target/drop.sql
sqb query --file target/drop.sql
sqb clean
```

`drop.sql` is one Snowflake Scripting block of `DROP ... IF EXISTS` statements for every model,
snapshot, seed, raw source table and function, edge-case area included, so it is safe to run
twice. Run the same three commands with `--target edge` (on `sqb compile` and `sqb query`) to clean
up the edge target. Scenario runs and
captures already drop their own relations.

SQLBuild also keeps state tables (`_SQLBUILD_*`) in the schema. If step 2 showed none before the
first build, they belong to DuckShop alone and you can drop them too. List them, then drop each
one by name:

```bash
sqb query "SHOW TABLES LIKE '\\_SQLBUILD\\_%'"
sqb query "DROP TABLE IF EXISTS <name>"
```

`sqb janitor` is not used here. It archives relations the current project no longer produces and
keeps the ones it does, so it would leave DuckShop's own tables in place. In a schema that other
SQLBuild projects also write to, it could archive their relations instead.
