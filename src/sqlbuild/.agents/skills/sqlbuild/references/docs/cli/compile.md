<!-- generated-by: sqlbuild skills -->

# sqb compile

> Compile models into resolved SQL, validate contracts, and write target artifacts - fully offline.

Online: https://sqlbuild.com/docs/cli/compile/

## Contents

- Usage
- Flags
- What compile does
- Compiling a selection
- Compiled artifacts
- Contract diagnostics
- Output
- Column lineage modes
- Reusing an unchanged compile
- Compiling after an edit
- Forcing a fresh compile
- Examples

`sqb compile` turns your project into the SQL it will run. It resolves references, expands
macros, checks SQL and column contracts, computes column lineage, runs your Rules, and writes the
compiled SQL to `target/compiled/`. It works offline and never connects to the warehouse.

```bash
sqb compile
```

```
Compile ready  12 models

├── daily_revenue                      OK   6 columns
├── dim_customers                      OK   7 columns
├── fact_orders                        OK   13 columns
├── stg_customers                      OK   5 columns
├── stg_orders                         OK   6 columns
└── stg_payments                       OK   6 columns
...

✓ Project compiled  12 models, 1 seed, 2 functions, 0 errors, 0 warnings
  Wrote: target/compiled/
```

Each model shows its status (`OK` or `FAIL`) and column count. Compile exits with `1` when it
finds any error, so it can gate CI.

## Usage

```bash
sqb compile [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined.
`--project-dir <path>`, `--no-color` and `--debug` can go before or after the command name.

## Flags

| Flag | Description |
|------|-------------|
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Analyze and report selected resources |
| `--select-file` | Read selectors from a file, one selector per line |
| `--exclude` | Remove resources from the selected scope |
| `--no-sql-analysis` | Turn off SQL syntax, binding, type inference and semantic checks (`--no-sql-validation` is an alias) |
| `--no-cache` | Compile in full, without reusing earlier compiles or the Rules cache; see [Forcing a fresh compile](#forcing-a-fresh-compile) |
| `--defer-to` | Resolve unselected model references against another target |
| `--json` | Print the full compile report as JSON |
| `--manifest` | Also write `target/manifest.json` with project metadata |
| `--lineage-mode` | Column lineage mode: `fast` (default), `rich` or `none` |

## What compile does

1. **Discovery.** Finds `sqlbuild_project.toml` and the project's models, sources, seeds,
   functions, audits, tests and macros.
2. **Graph.** Resolves `__ref()`, `__source()` and `__seed()`, expands macros, and orders models
   by dependency.
3. **SQL checks.** Parses and checks SQL, when SQL analysis is on. See
   [Semantic Compilation](../concepts/semantic-compilation.md).
4. **Column lineage.** Traces column-level dependencies across models.
5. **Contracts.** Checks declared columns against the inferred query output.
6. **Rules.** Runs the selected built-in Rules, then the selected custom Rules.
7. **Write.** Writes compiled SQL to `target/compiled/` when the Rules pass. When they fail, it
   writes the models' SQL to `target/compiled-failed/` instead, so you can read what the Rules
   checked; see [When Rules fail](#when-rules-fail).

Rules that need inferred types, such as `SQBRCONTRACT105`, are skipped when SQL analysis is off,
and compile prints one note naming them.

`sqb compile` is authoritative for correctness, contracts, lineage and configured Rules. Rule
findings block the artifacts. To check one Rule code or family without changing project
configuration, use [`sqb rules run`](rules.md).

### When Rules fail

A Rule finding exits `1` and blocks the manifest, the DAG file and the other artifacts. A
`target/manifest.json` from an earlier compile is removed, since it no longer matches.
`target/compiled/` keeps exactly what the last passing compile wrote.

Each model's expanded SQL goes to `target/compiled-failed/`, laid out like `target/compiled/`, so
you can read the SQL that macros generated. The report names the file of each model with Rule
errors:

```
✗ Project compiled  2 models, 0 seeds, 0 functions, 1 error, 0 warnings
  Failed SQL: target/compiled-failed/models/open_orders.sql
  Wrote: target/compiled-failed/
```

A model with Rule errors starts with a header that marks it not buildable and lists its findings:

```sql
-- sqb: compile failed for this model; not buildable
-- SQLCLEAN028 at models/open_orders.sql:6:20: Single-relation query mixes qualified and unqualified columns
SELECT o.order_id, status FROM __ref("orders") AS o
```

`target/compiled-failed/` holds only the latest failing compile: each failing compile replaces it
as a whole, and the next passing compile deletes it.

## Compiling a selection

Selectors limit analysis and reporting. Discovery still covers the whole project, so every
reference is still checked.

```bash
sqb compile fact_orders daily_revenue
sqb compile --select-file selected-models.txt
sqb compile --exclude deprecated_model
```

SQLBuild analyses the selected resources and the upstream models needed to understand them. The
text report shows only the selection. JSON also has the project totals and the selected model, seed
and function counts. The manifest and the DAG always cover the whole project.

## Compiled artifacts

`target/compiled/` holds what each resource will run, with every macro, variable and audit
argument rendered:

| Path | Contents |
|------|----------|
| `models/<path>.sql` | Each model's query |
| `functions/<language>/<path>.sql` | Each function's `CREATE FUNCTION` statements |
| `audits/generic/<resource>/<audit>__<column>.sql` | Each attached generic audit, under the model, source or seed it checks; column audits end in `__<column>`, and an audit's `name` replaces the audit name |
| `audits/singular/<audit>.sql` | Each singular audit |
| `tests/...` | Each SQL unit test's comparison query |

Read an audit's file to see how its arguments were spliced into the SQL, without running it.
References such as `__ref("orders")` stay as written. Files are rewritten on every compile, and
files for resources that no longer exist are removed.

## Contract diagnostics

A model's declared columns are checked against the columns its SQL produces:

```
error[K001]: declared column 'total_cents' was not found in statically inferred output for model 'fact_orders'
  model: fact_orders
  --> models/marts/fact_orders.sql:6:5
  6 |     total_cents (),
    |     ^^^^^^^^^^^
  = help: add total_cents to the SELECT list or correct/remove MODEL(columns (...)); MODEL(columns (...)) is validated using static SQL analysis because settings.column_contract_mode is "implicit" (the default). If this project intentionally uses columns only for metadata and audits, set [settings] column_contract_mode = "explicit"; models with contract enforced remain validated
```

| Code | Meaning |
|------|---------|
| `K001` | A declared column is missing from the query output |
| `K002` | A column's inferred type doesn't match the declared type |
| `K003` | A column's type can't be proven (with `type_enforcement` on) |
| `K013` | A string literal fills an enum-typed column of an enforced contract but isn't a member of the enum (not checked on SQL Server) |

Which models are checked depends on `[settings] column_contract_mode`:

- `implicit` (default): a model with declared columns and no `contract` setting must output every
  declared column.
- `explicit`: only models with `contract enforced` are shape-checked; declared columns elsewhere
  are metadata and audit targets.

Explicit type enforcement works in both modes. Switching to `explicit` is a project policy, not a
way to silence an error: if a model is meant to have a contract, fix the query or the
declaration. Errors on `contract enforced` models don't suggest changing the mode. See
[Contracts](../concepts/models/contracts.md).

## Output

### Text output

The default output is the tree shown at the top of this page, followed by any diagnostics. Models
with contract errors are marked `FAIL`.

### JSON output

```bash
sqb compile --json
```

| Field | Contents |
|-------|----------|
| `summary` | Model, seed, function, audit, test, error and warning counts |
| `resources` | Per model: column count, dependencies, lineage summary and compiled SQL |
| `diagnostics` | Every diagnostic, with its source location |
| `compile_timings` | Timings and cache counters; see below |
| `lineage_mode` | The lineage mode used |
| `artifacts` | Paths of the files written. When the Rules fail, `compiled_sql_dir` is `target/compiled-failed/` and `failed_model_sql` lists each failing model's file |

If compile stops before it can build the report, for example on a model header that doesn't
parse, `--json` still prints one document with `command`, `has_errors: true`, `stopped: true` and
a `diagnostics` list holding the error's `code`, `message` and `help`. The exit code is `1`, and
progress stays on stderr.

`compile_timings` is one level deep, and every value is a non-negative integer. Use it to see
where a slow compile spends its time:

| Key | Meaning |
|-----|---------|
| `discover_ms`, `graph_ms`, `lineage_ms`, `contracts_ms`, `write_ms` | Wall time of each phase |
| `built_in_rules_ms` | Wall time of the built-in Rules |
| `custom_rules_ms`, `custom_rules_cpu_ms` | Wall and CPU time of the custom Rules, including their host processes |
| `custom_rules_invocations` | Custom Rule invocations, one per Rule and subject |
| `custom_rules.<code>.invocations` | Invocations of one custom Rule |
| `custom_rules.<code>.cpu_ms` | CPU time of those invocations |
| `custom_rules.<code>.cache_hits` | Subjects whose stored result was reused |
| `custom_rules.<code>.cache_misses` | Subjects evaluated again; every subject when the Rules cache is off |
| `rule_cache_hits`, `rule_cache_misses` | Rule results reused or evaluated again |
| `project_reuse_hits`, `project_reuse_misses`, `project_reuse_bypasses` | Whether the compile [reused an unchanged compile](#reusing-an-unchanged-compile), compiled because something changed, or was not eligible |
| `project_reuse_check_ms` | Time spent checking whether the previous compile can be reused |
| `total_ms` | Wall time of the whole compile |

Other keys are finer-grained phase timings, cache counters and the custom Rules host's own
overhead (`custom_rules_monitor_cpu_ms`, `custom_rules_guard_cpu_ms`). Each selected custom Rule has all
four `custom_rules.<code>.*` keys; read one with
`jq '.compile_timings["custom_rules.XORDERS001.cpu_ms"]'`. A Rule answered fully from the cache
reports `0` invocations. Invocations and CPU can exceed a Rule's subject count, because a Rule
whose module state changed is evaluated again.

## Column lineage modes

| Mode | Description |
|------|-------------|
| `fast` | Default. Lightweight column extraction from model metadata |
| `rich` | Full SQL analysis with transform classification and deeper tracing; slower on large projects |
| `none` | No column lineage |

The JSON report holds a lineage summary per model, not the full column graph. Use
`sqb lineage <model>[.<column>]` to inspect lineage. See
[Column Lineage](../concepts/column-lineage.md).

## Reusing an unchanged compile

When nothing that affects the result has changed, `sqb compile` replays the previous result. It
prints one line to stderr, then the same report and exit code as before, including failed compiles
and their diagnostics:

```
Inputs unchanged; reused the previous compile (0.1 s)
```

A compile is reused only when all of these match the previous one:

- every file and directory in the project, apart from the paths listed below;
- the command line: project and working directory, flags, selectors, `--vars`, target, and
  whether color output is on;
- every `SQLBUILD_*` and `SQB_*` environment variable, every variable a template or `@@ENV` read,
  and every variable a provider's settings can read, including `env_prefix`, aliases and
  nested-delimiter variables (values are stored only as hashes);
- the contents of each provider settings `env_file` and `secrets_dir`;
- the Python interpreter, the SQLBuild version, `sys.path`, and every loaded Python module file
  outside the project;
- the files the previous compile wrote under `target/compiled/` and the `--dag` file. If one is
  missing or edited, compile runs in full and rewrites it.

These paths are not inputs: `target/`, `logs/`, `venv/`, `.git/` and `__pycache__/` anywhere, and
`.cache`, `.fensu`, `.hg`, `.idea`, `.mypy_cache`, `.nox`, `.pytest_cache`, `.ruff_cache`,
`.sqlbuild`, `.svn`, `.tox`, `.venv` and `.vscode` at the project root. Other hidden folders are
inputs, because custom Rules can read them. DuckDB database files count only by whether they exist.

Reuse assumes that macros, hooks, providers, adapters and custom Rules are deterministic: the same
inputs give the same output. See
[Macros must be deterministic](../concepts/macros/python-macros.md#macros-must-be-deterministic). A
compile whose templates read `run.id` is never reused.

A replay differs from a full compile in three ways:

- In JSON, only `compile_timings` differs. It holds `project_reuse_hits`, `project_reuse_misses`,
  `project_reuse_bypasses`, `project_reuse_check_ms` and `total_ms`.
- In text mode, the per-phase progress lines are not printed.
- Lifecycle sinks receive only the invocation events, because no compile phase runs.

When storing a result for reuse takes noticeable time, for example in a project with very many
files, compile prints `Recording compile for reuse...` and a completion line on stderr before the
report.

## Compiling after an edit

When only model SQL files and SQL test files changed since the previous compile, `sqb compile`
compiles just those files. It checks the changed models and tests, and every model whose inputs'
columns, types or nullability the edit changed, and takes everything else from the previous
compile. Project-wide Rules still run over the whole project. It prints one line to stderr, then
the report a full compile would give:

```
Compiled 1 changed file(s) against the previous compile (1 model(s), 2.1s)
```

Compile runs in full instead when:

- a file was added, removed or renamed, or any other file changed, such as
  `sqlbuild_project.toml`, a source or schema YAML file, a macro, a seed or a function;
- an edited model changes what it references, or changes a header setting other than
  `description`, `columns`, `dynamic_columns`, `model_schema`, `audits`, `contract`, or a switch
  between `view` and `table`;
- a custom Rule needs to see the edit: a project-subject Rule reads something the edit changed, or
  a model-subject Rule must run again for a model that was not edited (for example because it reads
  an edited upstream file), or reads more for an edited model than that model's SQL, columns,
  contract and grain, project files and the source list;
- the previous compile failed its Rules, since that compile writes no SQL tests, functions or
  audits to `target/compiled/`, or it reported other errors or warnings in files the edit didn't
  change;
- the previous compile used the other output format (`--json` or text), or ran with delta compiles
  off;
- the edit changes which models or sources a macro reads through its arguments, changes a model's
  `dynamic_columns` families, or changes the shape of so many models that a full compile is cheaper.

A full compile after an edit still reuses work for unchanged inputs: the column analysis of models
whose query, settings and upstream columns are unchanged, the scan of unchanged SQL test files,
Rule results, and macro results.

Either way, the output is identical to `sqb compile --no-cache`. To turn delta compiles off, set
`delta_compile = false` under `[settings]` or set `SQLBUILD_DISABLE_DELTA_COMPILE=1`.

## Forcing a fresh compile

Reuse notices edits from file metadata, such as size and timestamps, without reading unchanged
files. An edit can go unnoticed in these cases:

| Situation | Why |
|-----------|-----|
| A Python package outside the project gains a submodule or data file that compile reads without importing first | Packages are tracked by the module files compile loaded |
| A network filesystem that caches file attributes, or clocks that differ between machines | Timestamps may not show a recent edit |
| Windows, or a FUSE, SMB or exFAT mount, and a rewrite that keeps the size and restores the modification time (`touch -r`, `rsync -t`, `robocopy /COPY:T`) | These filesystems have no real change time |

In these cases, or whenever a result looks stale, compile in full:

| To | Use |
|----|-----|
| Compile in full once | `sqb compile --no-cache` |
| Always compile in full | `SQLBUILD_DISABLE_COMPILE_REUSE=1`, `SQLBUILD_DISABLE_COMPILE_CACHE=1`, or `compile_cache = false` on the target |
| Never compile only the changed files | `[settings] delta_compile = false` or `SQLBUILD_DISABLE_DELTA_COMPILE=1` |

`--no-cache` also turns the Rules cache off for that compile, as `[rules.cache] enabled = false`
does: every Rule evaluates every subject again, and `rule_cache_hits` is `0`. The next compile
without the flag reuses what the cache held before.

Compile also always runs in full with `--manifest`, `--debug` or a profiling flag, in a project
without `sqlbuild_project.toml` or `sqlbuild_project.yml`, and when a provider's settings use
sources SQLBuild can't list exactly, such as a custom `settings_customise_sources` or
command-line parsing. The reason is logged at debug level.

## Examples

```bash
# Compile the project
sqb compile

# Print the full report as JSON
sqb compile --json

# Compile with rich column lineage, or none
sqb compile --lineage-mode rich
sqb compile --lineage-mode none

# Also write target/manifest.json
sqb compile --manifest

# Compile only selected models
sqb compile fact_orders daily_revenue

# Turn off SQL analysis
sqb compile --no-sql-analysis

# Ignore earlier compiles and caches
sqb compile --no-cache
```
