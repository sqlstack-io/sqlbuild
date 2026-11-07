# SQLQuack dialect differential

Compares SQLBuild's Polyglot analysis of Snowflake, DuckDB (and MotherDuck) and SQLite models with
SQLQuack's parser and analysis, on the same inputs. It measures, per compiled model:

- **Parse coverage:** whether SQLQuack's parser reads the analysis SQL whole (`ok`), refuses it
  (`parse_error`) or keeps a construct opaque (`opaque`).
- **Lineage (B):** each output's upstream resource columns and transform kind, against the
  compile's `fast_lineage_columns`.
- **Binding findings (C):** `E2xx` findings from Polyglot's `validate_with_schema` with the options
  SQLBuild's binding catalog uses for the dialect, against SQLQuack's statement findings under the
  dialect's profile. Differences are grouped per code: `only_polyglot:<code>`,
  `only_sqlquack:<code>` and `message:<code>`.
- **Types (D):** each output's inferred type and nullability, against the compile's
  `inferred_columns`.

Inputs are compile captures from compiler differential work dirs (`--stage-captures`). Only
projects whose binding catalog is `snowflake`, `duckdb` or `sqlite` yield cases. Each model also
yields variants with one name broken, which measure only findings.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. The runner
takes the SQLQuack crates from a local checkout with `--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus seeds --seeds 30 \
  --stage-captures --work-dir /tmp/seeds
uv run python -m scripts.run_sqlquack_differential --work-dir seeds=/tmp/seeds \
  --sqlquack ../sqlquack --out /tmp/sqlquack-differential
```

The output directory holds `cases.jsonl`, `results.jsonl` (one comparison per case, with the
analysis SQL and SQLQuack's outputs) and `summary.json` (counts per measure and grouped differences
with examples, overall and per `LABEL:dialect`).

To parse DuckDB cases with SQLQuack's native DuckDB parser, build the harness with the
`native-duckdb` feature and the `sqlquack-syntax-duckdb` dependency from a checkout that has the
crate, then point `SQLQUACK_DIFFERENTIAL_BINARY` at the built binary.

## Verdicts

Each measure is `agree`, `differ`, `refines` or `not_measured`. `refines` means SQLQuack only adds
what Polyglot left out: an output Polyglot leaves unnamed, a type Polyglot leaves unknown, or
nullability Polyglot leaves unknown. A difference is not a verdict on which side is right: check
the engine before calling either one wrong.
