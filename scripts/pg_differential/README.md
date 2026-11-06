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

Inputs are the compile captures of the compiler differential's generated Postgres projects. Each
model also yields variants with one name broken (an unknown column, an upper-case quoted column,
an unknown qualifier, an unqualified column two joined relations share), so binding findings have
something to find; variants measure only findings.

## Running

The harness is a standalone Rust crate under `harness/`, outside the SQLBuild workspace. SQLQuack's
crates are not published, so the runner takes them from a local SQLQuack checkout with
`--config patch.crates-io...` flags.

```sh
uv run python -m scripts.run_compiler_differential --corpus seeds --seeds 30 \
  --stage-captures --work-dir /tmp/seeds
uv run python -m scripts.run_pg_differential --work-dir /tmp/seeds \
  --sqlquack ../sqlquack --out /tmp/pg-differential
```

The output directory holds `cases.jsonl`, `results.jsonl` (one comparison per case) and
`summary.json` (counts per measure and grouped differences with examples).

## Verdicts

Each measure is `agree`, `differ`, `refines` or `not_measured`. `refines` means SQLQuack only adds
what Polyglot left out: an output Polyglot leaves unnamed, a type Polyglot leaves unknown, or
nullability Polyglot leaves unknown. Lineage is not measured for star queries or models the compile
did not analyse; nothing is measured when SQLQuack's parser rejects the query or keeps a construct
opaque.
