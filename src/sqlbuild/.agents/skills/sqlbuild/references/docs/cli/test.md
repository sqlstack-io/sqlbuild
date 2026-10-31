<!-- generated-by: sqlbuild skills -->

# sqb test

> Run SQL unit tests and multi-model tests in isolation.

Online: https://sqlbuild.com/docs/cli/test/

## Contents

- Usage
- Flags
- Run only affected tests
- Examples

Runs SQL unit tests, independently reported parameterized cases, and multi-model tests without
building models. Useful for validating test logic independently.

## Usage

```bash
sqb test [SELECTOR ...] [flags]
```

Positional `SELECTOR` arguments work like `--select`, and both forms can be combined. `--project-dir <path>`, `--no-color`, and `--debug` can go before or after the command name.

## Flags

| Flag | Description |
|------|-------------|
| `--no-sql-analysis` | Disable compile-time SQL analysis (`--no-sql-validation` is an alias) |
| `SELECTOR ...` | Positional selectors, the same as `--select` |
| `--select`, `-s` | Select tests by name (`order_status_rules` or `test:order_status_rules`), or the tests targeting specific models |
| `--exclude` | Exclude tests by name, or the tests targeting specific models |
| `--warehouse <name>` | Snowflake warehouse for this invocation; overrides the target's `query` [warehouse group](../concepts/project-configuration.md#command-group-warehouses) and the connection warehouse |
| `--test-timeout <duration>` | Time limit for each test in this run, such as `30s` or `2m`, or `disabled`; overrides `TEST(timeout ...)`, the target and `[settings] test_timeout` |
| `--since <ref>` | Run only the tests affected by changes since the merge base of `<ref>` and `HEAD` (see [Run only affected tests](#run-only-affected-tests)); `@base` reads `[git] base_ref` |
| `--explain` | With `--since`, print why each selected test was selected |

Test names are globally unique, so a bare test name selects exactly that test. `test:<name>` is the
explicit form and accepts the same glob patterns as names, such as `test:orders_*`. A bare pattern
selects tests only when it matches no other resource. Test selectors are unioned with model
selectors, so `sqb test test:order_status_rules stg_orders` runs that test and every test targeting
`stg_orders`. Test selectors cannot use `+` graph expansion or `,` intersection. Only `sqb test` and
`sqb build` accept test selectors; other commands reject them with `S013`.

Parameterized cases use parent-level selection. Selecting a test or a target model includes every
case of each matching `TEST` template, and `--case` narrows the run to one case name.

Text output identifies each case as `<parent> [<case>]` and includes its source path and safe typed
parameters. Structured JSON emits one check per case with stable source/block/case identity,
declared parameter types and nullability, typed values, and a content fingerprint. Exact decimals
are strings in JSON so their scale is preserved. Each test check in JSON output reports its
`duration_ms`.

Each test row shows how many real (unmocked) models the test executes, such as `3 real models`. A
test that runs past its [time limit](../concepts/testing.md#time-limits) is reported as `TIMEOUT`,
counted under `TIMEOUT=` and excluded from `FAIL=` in the summary line
(`PASS=4  FAIL=0  TIMEOUT=1  TOTAL=5`), and the run continues with the remaining tests. In `--json`
output, `summary.fail_count` still includes timed-out tests, and `summary.timeout_count` reports how
many of them timed out. A timeout still fails the run: `sqb test` exits non-zero.

## Run only affected tests

`--since <ref>` runs only the tests a change can affect, which keeps CI on branch pushes from
running the whole suite. SQLBuild compiles the project as it was at the merge base of `<ref>` and
`HEAD`, straight from git objects without a checkout, and compares it with the working tree,
uncommitted edits included. A test is selected when:

- its own file changed or is new, or
- any node it actually executes changed: the models in its chain, the seeds, sources and
  functions they read, and the macros, enums, constants and audits they and the test file use
  (every case of a parameterized test counts), or
- a model, source or seed it mocks changed its declared columns: names, types, nullability or
  contract, which shape the mock's columns, or
- a project-wide input changed: see [Project-wide inputs](#project-wide-inputs).

The resolution is the one `--inspect` shows, so a test that mocks a model, source, seed or macro
is not selected when only that relation's SQL, data or upstream changes, while a downstream test
that really runs it is.

A node changed when its definition fingerprint differs. The fingerprint covers the model SQL with
formatting and comments ignored, the resolved configuration (including what `[defaults]` and
`[path_defaults]` contribute), columns and contract, attached audits, the values of the project
variables the model reads with `@@name`, and every macro, enum, constant, audit, function and seed
the node uses, resolved through [scopes](../concepts/declaration-scopes.md). A change to the
`[settings]` that shape compiled models counts as a change to every model. Descriptions do not
count; `meta` does.

A macro counts as changed when the Python file that defines it changes, or any project module it
imports, such as a helper under `python/`; when that code reads `ctx.vars`, a change to any project
variable counts too. A [SQL macro](../concepts/macros/sql-macros.md) counts as changed when its
arguments, result type or body change, or a project variable it reads; formatting, comments and its
description do not count. A node that calls a SQL macro also uses every SQL macro that macro calls. An enum, constant or audit counts as changed when its declaration file
changes other than in formatting and comments. A seed counts as changed with its data or column
types, a source with its declaration or inline expression, and a function with its definition.
Loaders, tasks, assets and checks are compared by their Python modules and the project modules
they import; they appear in the change summary but no unit test runs them.

### Project-wide inputs

Some inputs reach every test, so a change to any of them selects every test:

- the `[settings]` that shape compiled models;
- the adapter, `[connection]` and `[connections]` (including connection settings and adapter
  extensions), `[targets]` and the target in use, and project-local adapter modules under
  `adapters/`. Their values are compared by digest and never printed;
- the Python environment as the project records it: the git-tracked `pyproject.toml`, `uv.lock`,
  `poetry.lock`, `pdm.lock`, `Pipfile.lock` and `requirements*.txt` in the project directory or at
  the repository root, compared with line endings normalised. Untracked and ignored copies do not
  count. Any edit to `pyproject.toml` counts, even one unrelated to dependencies.

Environment variables a project reads with `${ENV:NAME}` in configuration or `@@ENV:NAME` in SQL
are part of the cache key of the base compile and of the fingerprints of the SQL that reads them;
their values are never printed.

`--since` cannot see code outside the project: modules imported from outside the project
directory, installed package versions that the files above do not record, and environment
variables read directly from Python with `os.environ`. Run the full suite when those change.

Added nodes count as changed. A model renamed with `migrate_from` is compared with its old
definition, so a pure rename selects nothing, while a rename that also edits the model counts as a
change. Deleted nodes select nothing and are listed in the summary.

```bash
# Tests affected by this branch
sqb test --since origin/main

# Read the ref from [git] base_ref in sqlbuild_project.toml
sqb test --since @base

# Only affected tests that target stg_orders, and say why each runs
sqb test stg_orders --since origin/main --explain
```

Selectors still apply: `--select` and positional selectors intersect with the affected tests, and
`--exclude` removes tests. `--explain` prints the reasons for each selected test, and `--inspect`
adds them to the resolved plan as `selected:` lines:

```text
Comparing with origin/main (merge base 1a2b3c4d5e)
Compiling base revision 1a2b3c4d5e...
Compiled base revision 1a2b3c4d5e (2.31s)
Changed since origin/main: 1 model, 1 macro changed; deleted: model legacy_orders
Selected 2 of 9 tests
  orders_rollup
    executes stg_orders (model SQL changed)
  order_timestamps
    executes macro to_utc (changed)
```

The base compile is cached under `target/since/` per merge-base commit, target, `--vars`, local
configuration and referenced environment variables, and the newest few are kept. `sqb test --since`
and `sqb scenario test --since` in the same CI job compile the base once when they use the same
target and `--vars`; `sqb scenario test` uses the default target, so a `sqb test --target` run
compiles the base separately. When no test is affected, the command says so and exits
successfully.

With `--json`, the progress and selection lines go to stderr, and the JSON document on stdout gains a
`since` section: the ref and merge base, the `changed`, `renamed` and `deleted` nodes, the number
of candidate tests, and each selected test with its reasons:

```json
"since": {
  "ref": "origin/main",
  "merge_base": "1a2b3c4d5e...",
  "changed": [
    {"kind": "model", "name": "stg_orders", "added": false, "aspects": ["sql"], "renamed_from": null}
  ],
  "renamed": [],
  "deleted": [{"kind": "model", "name": "legacy_orders"}],
  "candidate_count": 9,
  "selected": [
    {"name": "orders_rollup", "reasons": ["executes stg_orders (model SQL changed)"]}
  ]
}
```

The base revision must compile. When it does not, `--since` fails with the compile error instead
of guessing; compare with a later ref whose project compiles, or run without `--since`. A
`--target` that the base revision does not define, such as one the branch adds, compiles the base
with its default target instead; the run says so, and because the target changed, every test is
selected.

SQL comparisons ignore layout and comments using a generic SQL tokenizer. In dialects where `--`
starts a comment only when followed by whitespace, such as MySQL, `1--1` is read as `1` followed by
a comment, so an edit inside such an expression can go unnoticed. The base tree is written from git
objects with the repository's filters. Git LFS content is restored for files that an unanchored
`.gitattributes` pattern marks, such as `*.csv filter=lfs`, but a pattern anchored to a path, such
as `/data/*.csv` at the repository root for a project in a subdirectory, does not match the
exported paths. Such a file is an LFS pointer at the base and real content in the checkout, so it
always counts as changed: at most extra tests run.

`<ref>` is any git ref or commit, such as the merge request's target branch from a CI variable.
`--since` always takes a value. A ref that does not exist, or a shallow clone that does not reach
the merge base, fails with an error that says what to fetch, such as `git fetch --unshallow` or a
larger clone depth; it never silently selects nothing.

## Examples

```bash
# Run all tests
sqb test

# Run tests for a specific model
sqb test stg_orders

# Run one test by name
sqb test order_status_rules

# Run one case of a parameterized test
sqb test test:order_status_rules --case open_case

# Allow each test two minutes in this run
sqb test --test-timeout 2m
```
