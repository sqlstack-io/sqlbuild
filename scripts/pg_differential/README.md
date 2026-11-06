# Postgres differential

Compares SQLBuild's Polyglot analysis of Postgres models with SQLQuack's Postgres parser and
analysis, on the same inputs. It measures three things per compiled model:

- **Lineage (B):** each output's upstream resource columns and transform kind, against the
  compile's `fast_lineage_columns`.
- **Binding findings (C):** `E2xx` findings from Polyglot's `validate_with_schema` with the options
  SQLBuild's binding catalog uses for Postgres (strict, type, reference and semantic checks),
  against SQLQuack's statement findings under the Postgres profile.
- **Types (D):** each output's inferred type and nullability, against the compile's
  `inferred_columns`.

Inputs are compile captures from compiler differential work dirs: the generated seed projects, and
any project compiled with `--project ... --stage-captures`, such as the Postgres e2e projects, the
waffle-shop templates wired to Postgres, or the analysis failure corpus recompiled for Postgres.
Only projects whose binding catalog is Postgres yield cases. Each model also yields variants with one name broken (an unknown column, an upper-case quoted column,
an unknown qualifier, an unqualified column two joined relations share), so binding findings have
something to find; variants measure only findings.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus seeds --seeds 30 \
  --stage-captures --work-dir /tmp/seeds
uv run python -m scripts.run_pg_differential --work-dir seeds=/tmp/seeds \
  --work-dir e2e=/tmp/e2e-captures --sqlquack ../sqlquack --out /tmp/pg-differential
```

The output directory holds `cases.jsonl`, `results.jsonl` (one comparison per case) and
`summary.json` (counts per measure and grouped differences with examples, overall and per
`LABEL=` source).

Relations follow SQLBuild's binding catalog: closed relations carry their declared types and
inferred nullability; referenced relations with no closed shape are open inputs, left out of both
schemas and kept only to map lineage to their resource. Project functions are declared to both
engines as SQLBuild declares them to Polyglot, with each scalar UDF typed by its return type.

## Verdicts

Each measure is `agree`, `differ`, `refines` or `not_measured`. `refines` means SQLQuack only adds
what Polyglot left out: an output Polyglot leaves unnamed, a type Polyglot leaves unknown, or
nullability Polyglot leaves unknown. Lineage is not measured for star queries or models the compile
did not analyse; nothing is measured when SQLQuack's parser rejects the query or keeps a construct
opaque.
