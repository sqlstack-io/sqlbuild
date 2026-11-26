<!-- generated-by: sqlbuild skills -->

# sqb scenario

> Run end-to-end scenario tests locally with DuckDB or against the warehouse.

Online: https://sqlbuild.com/docs/cli/scenario/

## Contents

- sqb scenario test
- sqb scenario capture
- Runtime artifacts

Run end-to-end scenario tests. Scenarios materialize fixture inputs as physical relations, build the real project graph against them, and evaluate expected outputs and assertions. See [Scenarios](../concepts/scenarios.md) for concepts and authoring details.

## sqb scenario test

Run scenario tests locally in DuckDB from captured snapshots, or against the warehouse.

```bash
sqb scenario test [flags]
```

### Flags

| Flag | Description |
|------|-------------|
| `--select`, `-s` | Select scenarios to run |
| `--exclude` | Exclude scenarios from the selection |
| `--retain` | Keep scenario-owned warehouse artifacts for inspection |
| `--scenario-namespace NAMESPACE` | Isolate warehouse artifacts for this run; overrides environment and config |
| `--run-on auto\|local\|warehouse` | Where scenarios run; overrides the target's and `[settings]` `scenario_run_on` (see [Where scenarios run](#where-scenarios-run)) |
| `--verbose` | Print every reason that keeps a scenario off DuckDB in full, instead of the census |
| `--concurrency N` | Run up to `N` scenarios at once (default: the project `concurrency` setting) |
| `--scenario-timeout <duration>` | Time limit for each scenario run on the warehouse, such as `10m`, or `disabled`; overrides `SCENARIO(timeout ...)`, the target and `[settings] scenario_timeout`. Not applied to local runs |
| `--sync-snapshots` | Capture missing or stale snapshots before the run, so those scenarios can run locally; not with `--run-on warehouse` |
| `--refresh` | Recapture all selected snapshots before the run; not with `--run-on warehouse` |
| `--force` | Bypass snapshot capture safety limits |
| `--max-snapshot-rows` | Override per-relation row limit for capture |
| `--max-snapshot-total-rows` | Override total row limit for capture |
| `--max-snapshot-bytes` | Override per-relation byte limit for capture |
| `--max-snapshot-total-bytes` | Override total byte limit for capture |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |
| `--since <ref>` | Run only the scenarios affected by changes since the merge base of `<ref>` and `HEAD`; `@base` reads `[git] base_ref` |
| `--explain` | Print where each scenario runs and why; with `--since`, also why it was selected |

### Affected scenarios

`--since <ref>` applies the rules of [`sqb test --since`](test.md#run-only-affected-tests)
to scenarios. A scenario is selected when its own file (its definition and fixtures) or its
captured local snapshots changed, or when any node it actually executes changed: the models it
builds, the seeds and sources it reads, the functions they call, the audits attached to its models,
and the macros, enums and constants they and the scenario use. A model the scenario replaces with a
`__ref__` fixture, a source it replaces with a `__source__` fixture, or a seed it replaces with a
`__seed__` fixture, is not executed, so only changes to its declared columns, which shape the
fixture, select the scenario. [Project-wide inputs](test.md#project-wide-inputs) such as
the connection, targets and Python dependency files select every scenario.

`--select` and `--exclude` narrow the affected scenarios further. With `--json`, the document gains
the same `since` section as `sqb test`, listing each selected scenario with its reasons. Both
commands share the cached base compile when `sqb test` uses the default target and no `--vars`, so
`sqb test --since origin/main` and `sqb scenario test --since origin/main` in one CI job compile
the base once and resolve the same changed set:

```bash
sqb test --since "origin/$TARGET_BRANCH"
sqb scenario test --since "origin/$TARGET_BRANCH" --explain
```

### Selectors

Scenarios are selected with `--select`. `--exclude` removes scenarios from the selection. Without any selectors, all discovered scenarios run.

| Selector | Example |
|----------|---------|
| Scenario name | `sqb scenario test --select daily_revenue_minimal` |
| Multiple names | `sqb scenario test --select daily_revenue_minimal --select daily_revenue_multi_order` |
| `.sql` file path | `sqb scenario test --select tests/scenarios/revenue/daily_revenue_minimal.sql` |
| Folder | `sqb scenario test --select tests/scenarios/revenue` |
| Scenario-root-relative folder | `sqb scenario test --select revenue` |
| Exclude | `sqb scenario test --select revenue --exclude daily_revenue_multi_order` |

Mixed selector types are supported and the result is de-duplicated by scenario name.

### Remote examples

```bash
# Run all scenarios
sqb scenario test

# Run one scenario
sqb scenario test --select daily_revenue_minimal

# Run and retain warehouse artifacts
sqb scenario test --select daily_revenue_minimal --retain

# Run all scenarios in a folder
sqb scenario test --select revenue
```

### Where scenarios run

| `--run-on` | Where each scenario runs |
|------|-------------|
| `auto` (default) | Locally when its snapshot is fresh and SQLQuack converts all its SQL; otherwise on the warehouse |
| `local` | Locally. A scenario with a missing or stale snapshot, or SQL SQLQuack refuses, is not run and fails the run; nothing goes to the warehouse |
| `warehouse` | On the warehouse |

The setting comes from, in order: `--run-on`, the default target's `scenario_run_on`,
`[settings] scenario_run_on` in `sqlbuild_local.toml`, then in `sqlbuild_project.toml`, then
`auto`. `--retain` keeps warehouse artifacts only, so it is rejected with `--run-on local`.

```bash
# Run locally (requires prior capture)
sqb scenario test --run-on local

# Capture missing/stale snapshots first, then run locally
sqb scenario test --run-on local --sync-snapshots

# Recapture everything first
sqb scenario test --run-on local --refresh
```

### Output

`sqb scenario test` prints its output the way [`sqb test`](test.md#output) does: a header,
one row per scenario, then failures, the reasons scenarios could not run locally, and the summary
line last:

```
Scenarios ready  3 selected, 2 local, 1 on the warehouse
  note: 1 scenario runs on the warehouse: snapshot stale 1 (--explain for where)

Execution  sqb scenario test  (concurrency: 1)

  1/3  scenario  daily_revenue_minimal       PASS   local        0.21s
  2/3  scenario  daily_revenue_multi_order   FAIL   local        0.34s
         └── expected  daily_revenue         FAIL   2 rows differ
  3/3  scenario  order_prices_retained       PASS   warehouse    0.84s

Failures:

  daily_revenue_multi_order  (scenario, local)
    error     expected rows of daily_revenue differ: 1 missing, 1 extra (2 expected, 2 actual)
    duckdb    target/run/scenarios/daily_revenue_multi_order/local.duckdb

Retained (2):
  daily_revenue_minimal
    duckdb    target/run/scenarios/daily_revenue_minimal/local.duckdb
  daily_revenue_multi_order
    duckdb    target/run/scenarios/daily_revenue_multi_order/local.duckdb

Failed (1):
  ✗ daily_revenue_multi_order  2 rows differ

  2 local, 1 on the warehouse
  Statements: 9 on the warehouse (CREATE 3, SELECT 3, DROP 2, ALTER 1); compile time not reported by duckdb
✗ Completed with errors  PASS=2  FAIL=1  TOTAL=3  (1.43s)
```

A passing scenario is one row; a failed one also lists its expected outputs and assertions and
its error. `--verbose` lists them for passing scenarios too. `Retained (N):` lists the local DuckDB
file each local run keeps, and with `--retain` the warehouse relations each scenario kept. A
namespaced run prints `namespace <value> (from <source>)` under the header and in the detail
lines. Each scenario row shows its wall time and the summary shows the total. `--json` reports
the same values as `scenarios[].duration_ms` and `summary.duration_ms`. The `Statements:` line
and the JSON `statements` section are described in
[Statement summary](test.md#statement-summary).

Under `--run-on local`, a scenario that cannot run locally, such as one without a fresh snapshot,
gets no row and is counted under `NOT_RUN=`, and the run fails:

```
  Can't run locally (1): snapshot stale 1; see below

Can't run locally (1 scenario):

error: snapshot stale                                                         1 scenario · 1 model
   └── order_prices_retained  1 scenario
   = note: tests/_scenario_snapshots/order_prices_retained/scenario.json is not a fresh snapshot
   = help: run these scenarios on the warehouse with --run-on auto

  2 local, 1 can't run locally
✗ Completed with errors  PASS=2  FAIL=0  NOT_RUN=1  TOTAL=3  (1.20s)
```

The header and the grouped causes work as for [`sqb test`](test.md#local-and-warehouse-runs):
under `--run-on local` a `Run on` tree and a `Can't run locally (N scenarios):` section after the
failures; in `auto` a `note:` line in the header, `--explain` for where each cause is, and
`--verbose` for the full notes after the run. A scenario whose converted SQL hits a SQLQuack guard
or a DuckDB internal error while it replays runs again on the warehouse in `auto`, with the
reason `SQLQuack guard: <name>` or `DuckDB internal error: <message>`, and its local attempt
prints nothing; with `--run-on local` it is not run. With `--json`, each
scenario has `run_on` and `run_on_reason`, `summary` has `local_count` and `warehouse_count`, and
the document has `refusals`, as for `sqb test`.

Failed scenarios suggest rerunning with `--retain` for inspection. Local runs always keep the DuckDB file at `target/run/scenarios/<scenario_name>/local.duckdb`.

## sqb scenario capture

Capture scenario input fixtures from the warehouse as JSONL snapshots for local replay.

```bash
sqb scenario capture [flags]
```

### Flags

| Flag | Description |
|------|-------------|
| `--select`, `-s` | Select scenarios to capture |
| `--exclude` | Exclude scenarios from the selection |
| `--retain` | Keep warehouse fixture artifacts after capture |
| `--scenario-namespace NAMESPACE` | Isolate warehouse artifacts for this capture; overrides environment and config |
| `--force` | Bypass snapshot capture safety limits |
| `--max-snapshot-rows` | Override per-relation row limit |
| `--max-snapshot-total-rows` | Override total row limit |
| `--max-snapshot-bytes` | Override per-relation byte limit |
| `--max-snapshot-total-bytes` | Override total byte limit |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/adapters/snowflake.md#command-group-warehouses) and the connection warehouse |

### Examples

```bash
# Capture all scenarios
sqb scenario capture

# Capture one scenario
sqb scenario capture --select daily_revenue_minimal

# Capture and retain warehouse artifacts
sqb scenario capture --select daily_revenue_minimal --retain
```

Snapshots are written to `tests/_scenario_snapshots/<scenario_name>/` with a `scenario.json` manifest and JSONL files for each fixture relation. These files can be committed to version control.

## Runtime artifacts

Both subcommands report the effective namespace and its source. `scenario test --json` includes
`execution.scenario_namespace` (null when unset) and `execution.scenario_namespace_source`
(`cli`, `env`, `local config`, `project config`, or `unset`). Progress goes to stderr in JSON mode.
See [parallel CI runs](../concepts/scenarios.md#parallel-ci-runs) for namespace configuration and cleanup.

Both remote and local scenario runs write runtime artifacts to `target/run/scenarios/<scenario_name>/`:

```
target/run/scenarios/daily_revenue_minimal/
  cleanup/
    prepare.sql
    final.sql
  fixtures/
    ref__stg_orders.sql
    ref__stg_payments.sql
  sql/models/
    marts/daily_revenue.sql
  expectations/
    expected__daily_revenue.sql
    assertion__no_negative_revenue.sql
```

Local runs additionally write to a `local/` subdirectory and create `local.duckdb`.
