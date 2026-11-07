# BigQuery parse differential

Compares Polyglot's BigQuery parse of SQLBuild's SQL with SQLQuack's native BigQuery parser
(`sqlquack-syntax-bigquery`), statement by statement, and checks both against GoogleSQL's own
parser. It measures parsing only: whether each parser accepts the text and whether SQLQuack keeps
any construct as a gap (an opaque fallback). Lineage, types and findings are not compared.

Each body goes through SQLBuild's `normalize_analysis_sql` for BigQuery, with the model's `@@@`
placeholder values, before both parsers see the same text, as SQLBuild does before handing a body
to Polyglot. Hooks, SQLClean cases, Rust fixtures and adapter statements are parsed as written.

## Inputs

Nothing here connects to BigQuery; connection values are placeholders.

- **Generated seeds:** `capture/seeds.py` runs the compiler differential's seed corpus with every
  seed generated for the BigQuery adapter, with `--stage-captures`.
- **e2e projects:** `capture/snapshot_e2e.py` is a pytest plugin. Run the BigQuery e2e tests with
  it and `BQ_SNAPSHOT_OUT=<dir>`: it copies each project at its first `sqb` call and skips the
  test. Compile the copies with `run_compiler_differential --corpus --project <dir>/* \
  --stage-captures --work-dir <work-dir>` with `SQB_TEST_BIGQUERY_PROJECT=example-project`. Give
  each copy's `[targets.prod]` a schema distinct from dev, or the compile refuses the shared
  loader schema.
- **Failure corpus:** `capture/failures.py <dir>` writes the analysis failure corpus with each
  project rewired to BigQuery; compile it the same way.
- **Adapter templates:** `capture/adapter_sql.py OUT` calls every `render_*` method that takes
  plain arguments; `capture/adapter_structured.py OUT` then appends the ones that take structured
  arguments (snapshots, grants, typed values, loader rows, function DDL, retention, column
  changes), every boolean switch combination.
- **SQLClean's format corpus** (`crates/sqlclean/src/sql_lint/tests/fixtures/format_corpus.json`,
  its BigQuery cases) and the SQL literals in SQLBuild's Rust tests that mention BigQuery.

## Running

```sh
uv run python scripts/bq_differential/capture/seeds.py --corpus seeds --seeds 30 \
  --stage-captures --work-dir /tmp/bq/seeds --jobs 4
uv run python scripts/bq_differential/capture/adapter_sql.py /tmp/bq/adapter.jsonl
uv run python scripts/bq_differential/capture/adapter_structured.py /tmp/bq/adapter.jsonl
python scripts/bq_differential/extract.py /tmp/bq/bodies.jsonl \
  --capture seeds=/tmp/bq/seeds --capture e2e=/tmp/bq/e2e --capture failures=/tmp/bq/failures \
  --sqlclean ../sqlclean/crates/sqlclean/src/sql_lint/tests/fixtures/format_corpus.json \
  --rust-fixtures crates --adapter /tmp/bq/adapter.jsonl
Q=../sqlquack/crates
CARGO_TARGET_DIR=target cargo build --manifest-path scripts/bq_differential/harness/Cargo.toml \
  --config "patch.crates-io.sqlquack-syntax-bigquery.path=\"$Q/sqlquack-syntax-bigquery\"" \
  --config "patch.crates-io.sqlquack-syntax.path=\"$Q/sqlquack-syntax\""
target/debug/bq-differential < /tmp/bq/bodies.jsonl > /tmp/bq/results.jsonl
python scripts/bq_differential/oracle.py /tmp/bq/results.jsonl /tmp/bq/oracle.json <execute_query>
python scripts/bq_differential/tally.py /tmp/bq/bodies.jsonl /tmp/bq/results.jsonl \
  /tmp/bq/detail.json /tmp/bq/oracle.json
```

`<execute_query>` is GoogleSQL's `execute_query` tool; the oracle runs it with `--mode=parse
--sql_mode=script --enabled_language_features=MAXIMUM` and caches each verdict by text. The tally
prints, per source, how many bodies both parsers, only Polyglot, only SQLQuack or neither parse,
the gapped ones by construct, and each parser's false accepts and false rejects against GoogleSQL.
`detail.json` lists the body ids in each cell.

## Semantic differential

The `bq-semantic` binary compares SQLBuild's Polyglot analysis of each compiled BigQuery model
with SQLQuack's, as the Postgres differential does: lineage (each output's upstream resource
columns and transform kind), binding findings (`E2xx`, from `validate_with_schema` with the
binding catalog's BigQuery options) and output types and nullability. Each model also yields
variants with one name broken (an unknown column, the column in upper case and backquotes, an
unknown qualifier, an unqualified column two joined relations share), which measure findings only.

SQLQuack has no BigQuery semantics profile yet, so `src/semantic/profile.rs` stands in for one: it
keeps names as written and compares them without regard to case, and its name is one
`sqlquack-semantics` does not know, so typing follows the generic rules.

GoogleSQL's analyzer referees both sides. `semantic_oracle.py` types every query with
`execute_query --mode=analyze --product_mode=external`, each relation given as a CTE of typed
`NULL` columns and each project UDF as a temporary function, and records the analyzer's output
types or its first error. A model that casts to `VARCHAR` or `DOUBLE`, which BigQuery does not
have, is analysed with `STRING` and `FLOAT64` and marked lenient.

Upstream models are typed by each side itself: `semantic_run.py` reruns the harness, giving each
undeclared column of a model relation SQLQuack's type for that model, until nothing changes, and
the oracle uses its own types for those columns. Polyglot's side is what the compile recorded.

```sh
python scripts/bq_differential/semantic_cases.py /tmp/bq/cases.jsonl \
  seeds=/tmp/bq/seeds e2e=/tmp/bq/e2e failures=/tmp/bq/failures
CARGO_TARGET_DIR=target cargo build --manifest-path scripts/bq_differential/harness/Cargo.toml \
  --bin bq-semantic \
  --config "patch.crates-io.sqlquack-syntax-bigquery.path=\"$Q/sqlquack-syntax-bigquery\"" \
  --config "patch.crates-io.sqlquack-syntax.path=\"$Q/sqlquack-syntax\"" \
  --config "patch.crates-io.sqlquack-semantics.path=\"$Q/sqlquack-semantics\""
python scripts/bq_differential/semantic_run.py target/debug/bq-semantic /tmp/bq/cases.jsonl \
  /tmp/bq/results.jsonl
python scripts/bq_differential/semantic_oracle.py /tmp/bq/cases.jsonl /tmp/bq/results.jsonl \
  /tmp/bq/oracle.jsonl <execute_query>
python scripts/bq_differential/semantic_tally.py /tmp/bq/cases.jsonl /tmp/bq/results.jsonl \
  /tmp/bq/summary.json
python scripts/bq_differential/semantic_types.py /tmp/bq/cases.jsonl /tmp/bq/results.jsonl \
  /tmp/bq/oracle.jsonl /tmp/bq/types.json
python scripts/bq_differential/semantic_findings.py /tmp/bq/results.jsonl /tmp/bq/oracle.jsonl
```

`semantic_tally.py` prints each measure's verdicts per source and the differences ranked;
`semantic_types.py` classes every output's type against the analyzer (`agree`, `polyglot_wrong`,
`sqlquack_wrong`, untyped on either side); `semantic_findings.py` crosses each case's analyzer
verdict with whether each side reports a finding.
