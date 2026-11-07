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

SQLBuild's corpus has no ClickHouse seed projects, so seed projects are generated with the
ClickHouse adapter and dummy connection settings, and the e2e projects under
`tests/e2e/src/sqlbuild/cli/commands/main/clickhouse` are copied as each test first calls `sqb`.
Both are made outside this harness and compiled offline with `run_compiler_differential`, so
nothing reaches a warehouse.

A scalar function body is an expression, which ClickHouse only ever receives inside the adapter's
`CREATE OR REPLACE FUNCTION f(args) RETURNS type RETURN ...`: Polyglot parses the bare body, as
SQLBuild's gate does, and SQLQuack parses that statement.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus --project /tmp/clickhouse-projects/* \
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

## Semantic differential

The `clickhouse-semantic` binary compares SQLBuild's Polyglot analysis of each compiled ClickHouse
model with SQLQuack's, as the BigQuery semantic differential does: lineage, binding findings
(`E2xx`) and output types and nullability. Each model also yields variants with one name broken (an
unknown column, the column in upper case and backquotes, which ClickHouse's case-sensitive names do
not resolve, an unknown qualifier, and an unqualified column two joined relations share).
SQLQuack's side runs under `sqlquack-semantics`'s ClickHouse profile, so build against a SQLQuack
checkout that has it (`wip/clickhouse-semantics-stack` until it reaches `main`).

ClickHouse referees both sides. `semantic_oracle.py` gives each case a fresh `clickhouse local`
session with a Memory table per relation and a SQL function per project UDF stub, then runs
`DESCRIBE (<query>)` for the output types or ClickHouse's error. `semantic_types.py` canonicalizes
every type spelling with ClickHouse itself before comparing.

```sh
python scripts/clickhouse_differential/semantic_cases.py /tmp/ch/cases.jsonl seeds=/tmp/ch/seeds e2e=/tmp/ch/e2e
python scripts/clickhouse_differential/semantic_run.py target/debug/clickhouse-semantic /tmp/ch/cases.jsonl \
  /tmp/ch/results.jsonl
python scripts/clickhouse_differential/semantic_oracle.py /tmp/ch/cases.chained.jsonl /tmp/ch/results.jsonl \
  /tmp/ch/oracle.jsonl <clickhouse>
python scripts/clickhouse_differential/semantic_types.py /tmp/ch/cases.chained.jsonl /tmp/ch/results.jsonl \
  /tmp/ch/oracle.jsonl /tmp/ch/types.json <clickhouse>
python scripts/clickhouse_differential/semantic_findings.py /tmp/ch/results.jsonl /tmp/ch/oracle.jsonl
```

Build `clickhouse-semantic` with the same `--config patch.crates-io...` flags as the parse harness,
adding `sqlquack-semantics`.
