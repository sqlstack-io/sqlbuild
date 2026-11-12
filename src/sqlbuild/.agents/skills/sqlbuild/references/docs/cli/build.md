<!-- generated-by: sqlbuild skills -->

# sqb build

> Compile, plan, and execute the selected build lifecycle.

Online: https://sqlbuild.com/docs/cli/build/

## Contents

- Usage
- Global options
- Flags
- Fast iteration
- Building only what changed
- Execution order
- Output
- Deferred builds
- Failure behavior
- Limits
- Fingerprints
- Runtime artifacts

Compiles, plans, and executes the selected build lifecycle, running the full selected scope. Use
`--no-tests` and `--no-audits` to skip validation for fast iteration.

## Usage

```bash
sqb build [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined.

## Global options

These options apply to every `sqb` command. `--project-dir`, `--no-color`, and `--debug` go
before or after the command name, as in `sqb --debug build` or `sqb build --debug`. Under
`sqb dbt`, put `--project-dir` before `dbt`: after it, `--project-dir` is passed to dbt as dbt's
own option. `--version` works only directly after `sqb`, as in `sqb --version`:

| Option | Description |
|--------|-------------|
| `--project-dir <path>` | Run against the project in `<path>` instead of the current directory |
| `--no-color` | Print without colour |
| `--debug` | Print detailed progress, the SQL each step runs, and internal diagnostics to stderr; implies `--verbose` |
| `--version`, `-V` | Print the SQLBuild version |

## Flags

| Flag | Description |
|------|-------------|
| `--target` | Build against a configured target instead of the active/default target |
| `--no-tests` | Skip SQL unit tests |
| `--no-audits` | Skip audits |
| `--no-python` | Skip read-side Python tasks and assets (loader-side Python still runs for selected sources) |
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias) |
| `--no-cache` | Bypass the reusable compile-analysis cache for this invocation |
| `--full-refresh` | Drop and rebuild selected models unless a model sets `full_refresh false`; `full_refresh true` forces a model even without this flag |
| `--changed` | Build only the selected models and seeds whose definition or inputs changed since they last succeeded; see [Building only what changed](#building-only-what-changed) |
| `--defer-to` | Resolve unselected model references against another target |
| `--defer-sources-to` | Read managed source data from another target |
| `--fail-fast` | Stop on first failure and skip remaining nodes |
| `--concurrency` | Number of worker connections (default: 1) |
| `--test-timeout <duration>` | Time limit for each unit test the build runs, such as `30s`, or `disabled`; see [Time limits](../concepts/testing-reference.md#time-limits) |
| `--verbose`, `-v` | Show lifecycle SQL inline after each model |
| `--start-cursor` | Override the start cursor, typed from the selected incremental models' cursor type; fails with `S303` if the selection mixes timestamp and integer cursors. With no cursor model selected, an integer value is read as an integer cursor and anything else as an ISO timestamp |
| `--end-cursor` | Override the end cursor, typed the same way as `--start-cursor` |
| `--start-cursor-ts` | Override start cursor for timestamp incremental models (ISO format) |
| `--end-cursor-ts` | Override end cursor for timestamp incremental models (ISO format) |
| `--start-cursor-int` | Override start cursor for integer incremental models |
| `--end-cursor-int` | Override end cursor for integer incremental models |
| `--load` | Explicitly load managed sources before building |
| `--no-load` | Run no source loaders; selected managed sources are still audited |
| `--reload` | Reload managed sources (passes `is_reload=True` to loaders) |
| `--selection-diagnostics` | Warn when a selected model will build on changed upstream models that are not selected; see [Selection and staleness](../concepts/planning/selection-and-staleness.md) |
| `--manifest` | Generate `target/manifest.json` with plan-aware project metadata |
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Select specific models, or unit tests by name (`test:<name>`); selecting only unit tests runs them without building models |
| `--exclude` | Exclude specific models |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `build` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

## Fast iteration

Use `--no-tests` and `--no-audits` to skip validation when you only want to materialize models:

```bash
sqb build --no-tests --no-audits
```

This replaces the former `sqb run` command. The full lifecycle (tests + audits) is always the default; skip flags opt out of specific phases when you need speed.

## Building only what changed

`--changed` skips each selected model or seed that would read exactly what it read the last time
it built successfully:

```bash
sqb build --changed
```

```
Changed: 2 of 3 selected models and seeds run; 1 unchanged are skipped.
```

Add `--verbose` to print why each one runs, such as `input raw_orders changed`.

A selected model or seed runs when any of these is true:

| Reason | Example |
|--------|---------|
| It has no successful build on record | A new model |
| Its last build did not finish | The build was killed or failed after the model started writing |
| Its relation is missing, or `--full-refresh` or a cursor override applies | `sqb build --changed --full-refresh` |
| Its definition changed | Edited SQL, config, hooks, contract or a function it calls |
| A source it reads has a different [freshness](../concepts/sources.md#source-freshness) value | New rows raise `MAX(loaded_at)` |
| An upstream model or seed was rebuilt since, or runs in this build | `stg_orders` ran, so `customer_orders` runs |
| An upstream model or seed needs to run, even when it is not selected | `sqb build customer_orders --changed` runs `customer_orders` on every build until a changed `stg_orders` is built |
| What it reads can't be listed | SQL that reads relations not declared as sources or models, or a Python task or asset upstream |

A model that runs under `--changed` plans exactly as in a plain build: its
[`replay_on_change`](../concepts/planning/replay-decisions.md) policy still decides how much
history an incremental model reprocesses. A model that runs only because an upstream was rebuilt
takes its normal incremental step.

In a scheduled job, set `settings={'changed': True}`. Skipped models count as successful nodes of
the run.

Every build, with or without `--changed`, records what each model and seed read in
`_sqlbuild_consumption` in its target schema, so the build needs permission to create and insert
into that table. [`sqb janitor`](janitor.md) keeps the newest
`direct_state_history_versions` builds of each relation.

Limitations:

- A source is compared only through its freshness value. A change that doesn't move it, such as an
  update that keeps the old `loaded_at`, isn't seen. A source without freshness configuration counts
  as changed on every build.
- SQL that depends on the time, such as `current_date`, or on random values never counts as changed.
  Build those models without `--changed`.
- Two builds of one model less than 5 seconds apart can't be ordered, so its downstream models run
  on the next `--changed` build.

## Execution order

1. Managed sources are loaded (unless `--no-load`, which audits their existing data instead)
2. Selected seeds are loaded
3. Source audits run before their dependent models (unless `--no-audits`); with `--no-load`, or for a source selected by name, a source no model reads is audited at the end
4. SQL unit tests run before their target model (unless `--no-tests`)
5. Models are materialized in DAG topological order
6. Error-severity audits run against the staging table before promotion to the target (unless `--no-audits`)

A resource with an attached audit also waits for every other model, seed, or table function that
audit reads, so the audit can gate the resource like its other audits. See
[Audits that read other resources](../concepts/audits.md#audits-that-read-other-resources).

## Output

```
Execution  sqb build  (concurrency: 1)

   1/17  source    raw__customers                                        OK     0.05s  rows=5
   2/17  source    raw__orders                                           OK     0.03s  rows=10

   3/17  seed      waffle_types                                          OK     0.03s
   7/17  view      stg_orders                                            OK     0.04s
           test      test_stg_orders                                     PASS
             expect  expected stg_orders                                 PASS
             expect  assertion order_ids_are_not_null                    PASS
           audit     not_null (order_id)                                 PASS
           audit     unique (order_id)                                   PASS
           audit     not_null (customer_id)                              PASS
           audit     relationships (waffle_type_id)                      PASS
           audit     accepted_values (status)                            PASS
  13/17  table     hourly_order_activity  (delete_insert)                OK     0.24s
         5 batches (1d)    range 2026-03-31T09:00:00 → 2026-04-04T14:00:00    8 rows
           audit (d) orders_placed_is_non_negative                       PASS  5/5
           audit (d) not_null (activity_hour)                            PASS  5/5
           audit (f) orders_placed_is_non_negative                       PASS
           audit (f) not_null (activity_hour)                            PASS

✓ Completed successfully  PASS=99  WARN=0  FAIL=0  INSUFFICIENT=0  SKIP=0  TOTAL=99  (2.13s)
```

Each resource prints one row when it finishes, followed by its tests and audits. Rows for
individual warehouse statements and for steps inside a resource, such as `Create staging relation`,
`Promote relation`, `loader callable`, or audit evaluation, appear only with `sqb --debug build`.
Without `--debug`, a statement that is still running after 30 seconds prints a `RUNNING` row every
30 seconds, with its elapsed time and, when the warehouse reports one, its query ID. A failed
statement or step still prints its `FAIL` row.

## Deferred builds

Use `--defer-to` to resolve unselected model references against another target. This lets you build a subset of models in dev while referencing production tables for everything else:

```bash
sqb build fact_orders --defer-to prod
```

No `manifest.json` is required. `--defer-to` selects only the namespace used for unselected
references. SQLBuild resolves those relations through the active target's sole physical
connection and never opens the deferred target's connection. The deferred namespace must be
visible and readable through the active connection; deferral is not a cross-account,
cross-server, or cross-file transfer mechanism.

## Failure behavior

When a model fails:
- Downstream models are automatically blocked and skipped
- Staging/delta tables are retained for inspection
- Failure details show the model name, failed phase, and error message

A model that exceeds its [`max_duration`](../concepts/models/configuration.md#execution-time-limit)
fails with `C417`. The failure states the elapsed time, the limit and where it was set, whether
cancelling its running statement was confirmed, and the query IDs SQLBuild saw; downstream models are
skipped as for any other failure. Its row shows `TIMEOUT`. In the summary line, timed-out models and unit tests are counted under
`TIMEOUT=` and excluded from `FAIL=`
(`PASS=1  WARN=0  FAIL=0  TIMEOUT=1  INSUFFICIENT=0  SKIP=1  TOTAL=3`). In `--json` output,
`summary.failure_count` still includes them, and `summary.timeout_count` reports how many of those
failures were timeouts. A timeout is a failure for the exit code: the build exits non-zero and its
JSON `status` is `failed`.

## Limits

A target's [`execution_limits`](../concepts/project-configuration.md#execution-limits) cap a whole
build, and a model's [`max_duration`](../concepts/models/configuration.md#execution-time-limit) caps
one model.

### Model limit

When the build would run more models than `max_models`, the build stops before any clone, load or
model runs, and says that nothing in the warehouse changed. `sqb plan` has no limit, so use it to
check a broad selection.

### Duration limit

`max_duration` covers planning and the build. At the deadline SQLBuild starts no more work and
cancels the statements still running. Where the warehouse has a statement time limit, each
statement also gets only the time that remains.

| Adapter | Warehouse statement limit | Cancellation at the deadline | Longest accepted duration |
|---------|---------------------------|------------------------------|---------------------------|
| Snowflake | `STATEMENT_TIMEOUT_IN_SECONDS` per statement | Driver cancel request for the running query | 7 days |
| DuckDB, MotherDuck | None | Connection interrupt | About 24 days |
| PostgreSQL | `SET LOCAL statement_timeout` inside transactions; statements outside a transaction rely on cancellation | Driver cancel (`cancel_safe`) | About 24 days |
| BigQuery | Job timeout (`job_timeout_ms`) on each query job | `jobs.cancel`; reported as requested, not confirmed | About 24 days |
| Databricks | `STATEMENT_TIMEOUT` session setting, capped at 48 hours | Cursor cancel; reported as requested, not confirmed | About 24 days |
| SQL Server | None; the driver's query timeout is process-wide | Driver cancel | About 24 days |
| SQLite | None | Connection interrupt | About 24 days |

A duration timeout can come after earlier models finished, so its error says that warehouse changes
may already have been made. A custom adapter that can't cancel a running statement can't be used
with `max_duration`.

### Per-item time limits

Unit test, scenario and model limits (`test_timeout`, `scenario_timeout` and a model's
`max_duration`) count running time only, from the item's first warehouse statement. Rollback and
cleanup always run and don't count. Where a warehouse reports queueing separately, queued
time doesn't count:

| Adapter | How the item limit is enforced | Queued time |
|---------|--------------------------------|-------------|
| Snowflake | `STATEMENT_TIMEOUT_IN_SECONDS` per statement, with a client-side backstop after a grace margin of half the limit (at least 30 seconds) | Excluded by the warehouse limit; the backstop counts it |
| DuckDB, MotherDuck | Connection interrupt at the limit | Not applicable |
| PostgreSQL | `SET LOCAL statement_timeout` inside transactions; cancellation at the limit | Lock waits count |
| BigQuery | `jobs.cancel` at the limit; no job timeout, because a job timeout counts from job creation | Excluded: the clock stops while a job is pending |
| Databricks | `STATEMENT_TIMEOUT` session setting and cursor cancel | May count |
| SQL Server | Driver cancel at the limit | Lock waits count |
| SQLite | Connection interrupt at the limit | Waits for the write lock count |

When limits nest, such as a model's unit tests inside its own limit, the earliest deadline applies.
After a timeout, SQLBuild checks the connection and replaces it if it is no longer usable.

## Fingerprints

After a successful build, SQLBuild writes version identities to `_sqlbuild_fingerprints` in the target schema. These are used on subsequent runs to detect changes and skip unchanged work. A build writes a row only for a resource whose identity differs from its latest row, so a build that changes no definitions writes none. In a project with model migration history, every build writes a row for each resource it builds. See [Planning and Change Detection](../concepts/planning.md) for details.

## Runtime artifacts

Build writes executed lifecycle SQL to `target/run/models/`. These files contain the actual SQL that was executed, including resolved cursor bounds and runtime substitutions.
