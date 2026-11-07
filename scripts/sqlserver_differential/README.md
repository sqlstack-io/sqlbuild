# SQL Server differential

Compares Polyglot's SQL Server parse of the SQL SQLBuild produces for its SQL Server adapter with
SQLQuack's SQL Server parser, on the same text. Each body is normalized as SQLBuild's syntax gate
normalizes it (`normalize_analysis_sql`), checked with SQLBuild's own gate in the body's mode
(`validate` for model queries and hooks, `parse` for function bodies), parsed with
`Dialect::get(SQL Server)`, and parsed strictly with `sqlquack_syntax_sqlserver::parse_strict`.

Bodies come from:

- compile captures (`--work-dir LABEL=PATH`): model queries and their pre- and post-hooks, SQL hook
  files, audits with their measure and evidence SQL, SQL tests, scenarios and SQL function bodies,
  and the compiled tree the compile wrote (models, audits, SQL tests and function DDL). Only
  projects whose binding catalog is SQL Server yield bodies;
- SQL lists (`--listed LABEL=PATH`): JSON lines with `id`, `sql` and optionally `kind` and `mode`,
  for SQL gathered another way, such as the adapter's statements rendered with representative
  arguments, the Rust fixtures that parse SQL Server SQL, or a formatter corpus.

SQLBuild's corpus has no SQL Server seed projects, so seeds are generated with the SQL Server
adapter and dummy connection settings and compiled offline; the e2e projects under
`tests/e2e/src/sqlbuild/cli/commands/main/databricks` are snapshotted at their first `sqb` call and
compiled the same way. Nothing reaches a warehouse.

A scalar function body is an expression, which SQL Server only ever receives inside the adapter's
`CREATE OR REPLACE FUNCTION f(args) RETURNS type RETURN ...`: Polyglot parses the bare body, as
SQLBuild's gate does, and SQLQuack parses that statement.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus --project /tmp/dbx-projects/* \
  --stage-captures --work-dir /tmp/captures
uv run python -m scripts.run_sqlserver_differential --work-dir compile=/tmp/captures \
  --listed templates=/tmp/templates.jsonl --sqlquack ../sqlquack --out /tmp/sqlserver-differential
```

The output directory holds `results.jsonl` (one comparison per body) and `summary.json`.

## Verdicts

Each body is `both`, `polyglot_only`, `sqlquack_only` or `neither`. Polyglot parses when it returns
statements; SQLQuack parses only when it returns a tree with no gap, since a gap is a statement it
declines to decide. Counts are over unique bodies, grouped by statement form, body kind and source.
`bar_misses` lists every body Polyglot parses and SQLQuack does not, and `gap_constructs` ranks
SQLQuack's gaps by how many bodies hit each.

## Semantic differential

The `sqlserver-semantic` binary compares SQLBuild's Polyglot analysis of each compiled SQL Server
model with SQLQuack's, as the ClickHouse semantic differential does: lineage, binding findings
(`E2xx`) and output types and nullability. Each model also yields variants with one name broken: an
unknown column, the column in upper case in brackets (which SQL Server resolves under the default
case-insensitive collation SQLBuild's adapter keeps), an unknown qualifier, and an unqualified column
two joined relations share. SQLQuack's side runs under `sqlquack-semantics`'s SQL Server profile, so
build against a SQLQuack checkout that has it (`wip/sqlserver-semantics` until it reaches `main`), with
the same `--config patch.crates-io...` flags as the parse harness plus `sqlquack-semantics`.

SQL Server referees both sides. `semantic_oracle.py` connects to a local SQL Server as `sa`, with
the password in `MSSQL_SA_PASSWORD`, gives each project a fresh database with a table per relation and
a `__sqlbuild.udf_<name>` function per scalar UDF stub, and describes each query with
`sys.dm_exec_describe_first_result_set` for the output types or SQL Server's error.
`semantic_types.py` canonicalizes every type spelling with SQL Server itself before comparing.

```sh
python scripts/sqlserver_differential/semantic_cases.py /tmp/ss/cases.jsonl seeds=/tmp/ss/seeds e2e=/tmp/ss/e2e
python scripts/sqlserver_differential/semantic_run.py <harness>/sqlserver-semantic /tmp/ss/cases.jsonl \
  /tmp/ss/results.jsonl
python scripts/sqlserver_differential/semantic_oracle.py /tmp/ss/cases.jsonl /tmp/ss/results.jsonl \
  /tmp/ss/oracle.jsonl
python scripts/sqlserver_differential/semantic_types.py /tmp/ss/cases.chained.jsonl /tmp/ss/results.jsonl \
  /tmp/ss/oracle.jsonl /tmp/ss/types.json
python scripts/sqlserver_differential/semantic_findings.py /tmp/ss/results.jsonl /tmp/ss/oracle.jsonl
```
