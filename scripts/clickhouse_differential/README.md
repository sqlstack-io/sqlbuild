# ClickHouse differential

Compares Polyglot's ClickHouse parse of the SQL SQLBuild produces for its ClickHouse adapter with
SQLQuack's ClickHouse parser, on the same text. Each body is normalized as SQLBuild's syntax gate
normalizes it (`normalize_analysis_sql`), checked with SQLBuild's own gate in the body's mode
(`validate` for model queries and hooks, `parse` for function bodies), parsed with
`Dialect::get(ClickHouse)`, and parsed strictly with `sqlquack_syntax_clickhouse::parse_strict`.

Bodies come from:

- compile captures (`--work-dir LABEL=PATH`): model queries and their pre- and post-hooks, SQL hook
  files, audits with their measure and evidence SQL, SQL tests, scenarios and SQL function bodies,
  and the compiled tree the compile wrote (models, audits, SQL tests and function DDL). Only
  projects whose binding catalog is ClickHouse yield bodies;
- SQL lists (`--listed LABEL=PATH`): JSON lines with `id`, `sql` and optionally `kind` and `mode`,
  for SQL gathered another way, such as the adapter's statements rendered with representative
  arguments, the Rust fixtures that parse ClickHouse SQL, or a formatter corpus.

SQLBuild's corpus has no ClickHouse seed projects, so seeds are generated with the ClickHouse
adapter and dummy connection settings and compiled offline; the e2e projects under
`tests/e2e/src/sqlbuild/cli/commands/main/clickhouse` are snapshotted at their first `sqb` call and
compiled the same way. Nothing reaches a warehouse.

A scalar function body is an expression, which ClickHouse only ever receives inside the adapter's
`CREATE OR REPLACE FUNCTION f(args) RETURNS type RETURN ...`: Polyglot parses the bare body, as
SQLBuild's gate does, and SQLQuack parses that statement.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus --project /tmp/dbx-projects/* \
  --stage-captures --work-dir /tmp/captures
uv run python -m scripts.run_clickhouse_differential --work-dir compile=/tmp/captures \
  --listed templates=/tmp/templates.jsonl --sqlquack ../sqlquack --out /tmp/clickhouse-differential
```

The output directory holds `results.jsonl` (one comparison per body) and `summary.json`.

## Verdicts

Each body is `both`, `polyglot_only`, `sqlquack_only` or `neither`. Polyglot parses when it returns
statements; SQLQuack parses only when it returns a tree with no gap, since a gap is a statement it
declines to decide. Counts are over unique bodies, grouped by statement form, body kind and source.
`bar_misses` lists every body Polyglot parses and SQLQuack does not, and `gap_constructs` ranks
SQLQuack's gaps by how many bodies hit each.
