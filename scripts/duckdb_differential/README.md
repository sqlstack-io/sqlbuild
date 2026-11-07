# DuckDB differential

Compares Polyglot's DuckDB parse of the SQL SQLBuild produces for its DuckDB adapter with
SQLQuack's DuckDB parser, on the same text. Each body is normalized as SQLBuild's syntax gate
normalizes it (`normalize_analysis_sql`), checked with SQLBuild's own gate in the body's mode
(`validate` for model queries and hooks, `parse` for function bodies), parsed with
`Dialect::get(DuckDB)`, and parsed strictly with `sqlquack_syntax_duckdb::parse_strict`.

Bodies come from:

- the compiler goldens (`tests/goldens/compiler`): compiled models, audits, SQL tests and the
  adapter's function DDL of every DuckDB seed, example, fixture and failure project;
- compile captures (`--work-dir LABEL=PATH`): model queries and their pre- and post-hooks, SQL hook
  files, audits with their measure and evidence SQL, SQL tests, scenarios and SQL function bodies.
  Only projects whose binding catalog is DuckDB yield bodies;
- recorded statements (`--recorded LABEL=PATH`): every statement SQLBuild sends to a DuckDB or
  MotherDuck connection, recorded at `StatementLifecycle` while a test suite or project runs. This
  covers the adapter's run-time templates: materializations, merges, snapshots, seeds, state tables,
  attachments and transactions.

A scalar function body is an expression, which DuckDB only ever receives inside the adapter's
`CREATE OR REPLACE MACRO f(args) AS (...)`: Polyglot parses the bare body, as SQLBuild's gate does,
and SQLQuack parses that macro.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus seeds fixtures examples failures \
  --seeds 12 --stage-captures --work-dir /tmp/captures
uv run python -m scripts.record_duckdb_statements /tmp/recorded \
  tests/integration/src/sqlbuild/adapters/duckdb
uv run python -m scripts.run_duckdb_differential --work-dir compile=/tmp/captures \
  --recorded integration=/tmp/recorded --sqlquack ../sqlquack --out /tmp/duckdb-differential
```

`record_duckdb_statements` runs pytest with a `sitecustomize` on `PYTHONPATH`, so every Python
process the suite starts records into its own file; a project run with the same
`PYTHONPATH` and `SQLBUILD_DUCKDB_DIFFERENTIAL_RECORD_DIR` records the same way.

The output directory holds `results.jsonl` (one comparison per body) and `summary.json`.

## Verdicts

Each body is `both`, `polyglot_only`, `sqlquack_only` or `neither`. Polyglot parses when it returns
statements; SQLQuack parses only when it returns a tree with no gap, since a gap is a statement it
declines to decide. Counts are over unique bodies, grouped by statement form, body kind and source.
`bar_misses` lists every body Polyglot parses and SQLQuack does not, and `gap_constructs` ranks
SQLQuack's gaps by how many bodies hit each.
