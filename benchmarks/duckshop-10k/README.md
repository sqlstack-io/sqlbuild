# DuckShop 10K

A deterministic generator for a ten-thousand-model SQLBuild project in Snowflake SQL. It describes
Mallard & Co., the fictional duck-themed home goods business of the
[DuckShop](../duckshop/README.md) project, after a decade of growth: 40 storefronts in 10 regions,
marketplace partners, carriers, warehouses, retail stores, suppliers, payment gateways, ad
networks, messaging, affiliates, support tools, ledgers, wholesale desks, returns portals and a
product catalogue. Every name and value is invented.

Only the generators, their tests and `checksum.txt` are committed. The generated project is written
to a directory you choose.

## Usage

```bash
python benchmarks/duckshop-10k/generate.py --out /path/to/duckshop-10k   # write the project
python benchmarks/duckshop-10k/generate.py --verify /path/to/duckshop-10k  # check a written copy
python benchmarks/duckshop-10k/generate.py --check            # check an in-memory build
cd /path/to/duckshop-10k && sqb compile                        # no warehouse connection needed
```

`--profile smoke` writes a small project of about 1,100 models with the same templates, for fast
iteration. `checksum.txt` records the full profile only.

## Determinism and versioning

The generator uses no randomness, clocks or environment: the same generator version always writes
byte-identical files. `checksum.txt` holds the generator version, the file count and the SHA-256 of
`MANIFEST.sha256`, a sorted list of the SHA-256 of every generated file.

When a change alters any generated file, bump `GENERATOR_VERSION` in `duckshop10k/__init__.py` and
run `python benchmarks/duckshop-10k/generate.py --update-checksum`.

## Shape

| | Count |
|---|---|
| Models | 10,840 |
| Staging, intermediate, marts and snapshots | 6,149 (56.7%), 2,519 (23.2%), 2,162 (19.9%), 10 |
| Incremental models | 583, 112 of them with a capped automatic start and 112 renamed |
| Python export assets | 433 in 241 modules sharing one helper module |
| Sources | 2,466 table sources and one expression source |
| Unit tests and scenarios | 288 and 2 |
| Dependency depth | 11 at most; 170 models at depth 7 or more |

Most models read one to four upstreams; a few dozen wide marts, unions and scorecards read ten or
more. Staging deduplicates change logs with a change-log macro, types three platform layouts
(legacy columns with local times, typed columns with offset times, and JSON documents with epoch
times) and flattens arrays into child models. The intermediate layer converts every instance to a
shared shape (UTC times, major-unit amounts, mapped statuses), enriches headers with child totals,
unions instances by region and company, converts currencies with `ASOF JOIN`, sessionises
clickstream events in microbatches and builds customer identity, value and segment models and a
budget variance chain. Marts give every instance, team and region its reports, and publish facts and
dimensions with enforced contracts.

A large project also carries planning workload the layers above barely touch, so the generator adds
it in the proportions such projects have: an incrementally kept history of every third timestamped
intermediate table, one in five of them capping how far ahead its automatic start may sit and one in
five renamed from an older name with `migrate_from`, and a Python export asset for every fifth
published mart, a few per module, all sharing one helper module. A live plan reads a cursor bound for
each incremental model, plans each rename and discovers each Python node.

The models that use `HASH`, `TIMESTAMP_LTZ` or `CURRENT_TIMESTAMP()` are tagged `known_divergence`,
because their results depend on the engine or the session.

## Checks

- `benchmarks/duckshop-10k/tests/` checks the shape, the determinism of two builds, the recorded
  checksum and the generated custom Rule's tests, offline.
- `tests/e2e/.../compile/test_duckshop_10k_compile.py` generates the project, verifies it against
  `checksum.txt` and compiles it cold with the checkout's build, requiring zero errors and zero
  warnings.

CI runs both in the 10,000-model cell of the dense compile job, under its 4 GiB memory limit, after
the dense guards have finished, so the DuckShop compile never shares the limit with them.

## Synced warehouse state

`synced_state.py` writes the warehouse state a location syncs after building the project: about 13K
relations, a fingerprint and two `--changed` executions per built model, an execution per seed and
cursor observations, as the metadata pages a location sends the server. No warehouse runs the
project, so the hashes come from the planner itself: a `SQLBUILD_PLANNER_CAPTURE_DIR` capture of
one plan of the published project gives each model's identity metadata and query, and an unedited
model then plans as unchanged. `--changed-share` of the built models record a definition one line
older, and the models outside `--built-share` were never built.

```bash
python benchmarks/duckshop-10k/synced_state.py control.sqlite3 001-plan_output.json state.json \
  --built-share 0.95
```

`tests/e2e/.../plan/test_duckshop_10k_preview_performance.py` serves the project on a location,
mirrors that state into the server, and times a warm `--changed` plan preview of every node, with
every model built and with one in twenty unbuilt. It fails when the warm preview takes more than 0.45
of the same location's publish, a linear 10K compile on the same machine, so a lookup over every
model or relation shows however fast the runner is; it also checks that only the edited and unbuilt
models report their own change, and times the ad-hoc command check. It runs in the 10,000-model
cell after the DuckShop compile guard.
