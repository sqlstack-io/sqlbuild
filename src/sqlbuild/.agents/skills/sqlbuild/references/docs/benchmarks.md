<!-- generated-by: sqlbuild skills -->

# Benchmarks

> SQLBuild and dbt v2 compiling the same 10,000-model project: cold, unchanged, one-model and 1% edits.

Online: https://sqlbuild.com/docs/benchmarks/

## Contents

- SQLBuild vs dbt v2
- The project
- All variants, 10,000 models
- What is compared
- Why SQLBuild can reuse work by default
- Reproduce
- Compiler guards in CI

SQLBuild 0.164.3 and dbt 2.0.9 compiling the same generated [10,000-model DuckDB project](#the-project), on a
GitHub-hosted runner with 4 CPUs and 15 GiB RAM. Each cell is the median of three rounds from
[run 39543189999](https://github.com/sqlstack-io/sqlbuild/actions/runs/39543189999) (October 21, 2026).

## SQLBuild vs dbt v2

Both tools bind every column and type in every model. SQLBuild runs `sqb compile` with its defaults
(configurable rules off). dbt runs `dbt compile --static-analysis strict` (strict: with column and
type checks) with its opt-in cache (`--partial-parse --partial-load`).

| Scenario | SQLBuild | dbt v2 | dbt / SQLBuild |
|---|---:|---:|---:|
| Cold (no `target/`, no caches) | **15.7 s** | 64.7 s | 4.1× |
| Unchanged | **0.46 s** (reused) | 43.8 s | 95× |
| One model edited | **2.4 s** | 45.9 s | 19× |
| 100 models edited (1%) | **2.4 s** | 72.9 s | 30× |

Cold, SQLBuild used 30 s of CPU and 3.1 GiB at peak; dbt used 167 s and 4.7 GiB.

- **All rules on.** With every built-in rule and a custom Python rule, SQLBuild compiles cold in
  19.5 s, 3.3× faster than dbt strict.
- **dbt's changed-models-only mode wins a one-model edit.** `--dirty` compiles only changed models
  and their descendants: 1.6 s for the edited model and 47 descendants, against SQLBuild's 2.4 s for
  a report on the whole project. With 100 edited files it falls back to a full parse and takes 60.7 s.

## The project

The project comes from the dense generator SQLBuild's compile CI uses. The dbt project has the same
names, folders, SQL and DAG.

| What | Count | In dbt |
|---|---:|---|
| Models with typed columns (8–472 columns, about 31 on average) | 10,000 | models with enforced contracts |
| Sources | 2,400 | sources |
| Seeds | 500 | seeds |
| SQL functions | 250 | functions |
| Macros | 120 | Jinja macros producing the same SQL |
| `not_null` audits | 17,000 | `not_null` data tests |
| SQL tests with fixture rows | 10,000 | unit tests |

Each edit changes one constant in a model's SQL; the 1% edit changes 100 models spread across the
project. Rounds alternate the variant order.

## All variants, 10,000 models

Wall time in seconds on the same 10,000-model project. CPU and memory for every cell are in the run's `summary.md`.

| Variant | Cold | Unchanged | 1 edit | 100 edits |
|---|---:|---:|---:|---:|
| SQLBuild | 15.7 | 0.46 | 2.4 | 2.4 |
| SQLBuild, all rules on | 19.5 | 0.46 | 2.9 | 3.1 |
| dbt v2 strict, with cache | 64.7 | 43.8 | 45.9 | 72.9 |
| dbt v2 strict, changed models only (`--dirty`) | 63.3 | 0.86 | 1.6 | 60.7 |
| dbt v2 strict, no cache | 61.5 | 67.2 | 67.4 | 65.7 |
| dbt v2 strict, no cache, sources read from the warehouse | 67.4 | 74.3 | 73.9 | 73.5 |
| dbt v2, with cache | 39.5 | 21.8 | 24.0 | 47.9 |
| dbt v2, changed models only (`--dirty`) | 40.1 | 0.80 | 1.5 | 48.6 |
| dbt v2, no cache | 36.9 | 45.9 | 44.8 | 43.4 |

dbt v2 without `--static-analysis strict` doesn't report unknown columns.

## What is compared

- Both tools bind columns and types; dbt only with `--static-analysis strict`. The measured dbt is
  the `dbt` distribution on PyPI; strict analysis isn't in its Apache-2.0 build.
- SQLBuild also plans and binds its 10,000 SQL tests, reports contract, type and nullability
  problems as errors, and computes column lineage. dbt `compile` skips unit tests, warns on contract
  type mismatches and checks enforced contracts at run time.
- Sources are declared locally in both. The warehouse holds every relation, empty, as after
  `dbt build`; dbt reads it and `sqb compile` doesn't.
- dbt's cache and changed-models-only mode are opt-in. SQLBuild reuses unchanged work by default.

Before timing anything, the workflow checks both tools on 50-model projects, and fails if strict
analysis doesn't report the missing column in every dbt mode:

| Project | `sqb compile` | `dbt compile` | `dbt compile` strict |
|---|:-:|:-:|:-:|
| Clean | ✓ passes | ✓ passes | ✓ passes |
| Reference to a missing model | ✓ reported | ✓ reported | ✓ reported |
| Column the upstream model doesn't have | ✓ reported | ✗ not reported | ✓ reported |

## Why SQLBuild can reuse work by default

SQLBuild records what each compile depended on and checks all of it before reusing anything:

- **Every input is recorded during the compile itself:** project files, the command line, `--vars`,
  every environment variable a template read, provider settings files, the Python interpreter and
  loaded modules, and the SQLBuild build.
- **Files are checked by type, size, modification time, change time and inode**, and by content
  digest when a recently touched file's stamps move. If anything doesn't match, the stored result
  isn't reused.
- **Edits recompile only what they affect.** A changed model is compiled again, with every model
  whose input columns, types or nullability changed. Edits outside model and SQL test files, such as
  a macro, config or a new file, run a full compile.
- **The result matches a full compile.** Recompiling only the edited models produces byte-identical
  output to `sqb compile --no-cache`, and a reused compile prints the previous report again. CI compiles a copy in full
  after every step of a seeded edit series and compares the exit code and everything printed.

Reuse assumes macros, hooks, providers, adapters and custom rules are deterministic. It has
[documented limits](cli/compile.md#reusing-an-unchanged-compile), and `--no-cache` always
compiles in full.

## Reproduce

The harness and the exact command for every variant are in
[`benchmarks/dbt-compare/`](https://github.com/sqlstack-io/sqlbuild/tree/main/benchmarks/dbt-compare).
To rerun it on GitHub Actions:

```bash
gh workflow run benchmark-dbt.yml -f sqlbuild_version=0.164.3 -f dbt_version=2.0.9 -f rounds=3
```

The README also covers running it locally. Use an otherwise idle machine: load changes wall times
far more than CPU times.

## Compiler guards in CI

SQLBuild's CI also runs compile time and memory guards on dense projects and a custom Rules
benchmark; see `make test-e2e-dense-compile-performance` and `scripts/benchmark_rules.py`.
