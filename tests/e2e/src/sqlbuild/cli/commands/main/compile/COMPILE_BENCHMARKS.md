# Compile performance benchmarks

SQLBuild maintains complementary generated projects because model count alone does not predict
compile cost. Every fixture uses neutral orders-style names and synthetic SQL.

## Layered scale guard

The layered guard combines 976 models, 232 sources, 46 seeds, 23 SQL functions, 700 attached
audits, 130 native tests, two hooks, and 54 execution layers. Its model files follow a bounded SQL
size distribution up to roughly 520 KB. The graph contains a 54-layer spine and independent
eight-model chains.

This guard measures cold, unchanged, leaf-model edit, central-model edit, test edit, macro edit,
and project-config edit paths. It protects mixed-resource scaling and cache invalidation while the
separate 3,000/10,000-model and test-heavy guards retain their narrower contracts.

Budgets:

- cold compile: under 7 seconds;
- unchanged warm compile: under 3 seconds;
- one-model, one-test, and one-macro edits: under 4 seconds;
- project-config edit: under 8 seconds.

## Semantic density guard

The semantic guard exercises costs that SQL-size padding cannot represent:

| Characteristic | Generated guard |
| --- | ---: |
| Models | 976 |
| Sources | 232 |
| Seeds | 46 |
| SQL functions | 23 |
| Attached audits | 1,645 |
| Native test cases | 958 |
| Declared model columns | about 33,000 |
| Model SQL | about 5.8 MB |
| Compiled test SQL | about 13 MB |
| Hooks | 2 |
| Execution layers | 54 |

Declared column widths range from three to 538 columns. The SQL uses real expression trees rather
than comment-only padding: casts, conditional expressions, CTEs, derived tables, macros, functions,
seed joins, assertions, and a 267-branch set operation feeding a 270-column aggregate. The large set
operation specifically protects schema validation and CTE type inference from nested-tree
materialization regressions.

The guard records phase timings for cold and unchanged warm compiles. It enforces a 60-second cold
budget and a 12-second warm budget. Resource counts, declared-column count, model SQL bytes, and
compiled-test bytes are asserted so simplifying the fixture cannot make the performance check pass.

Both guards initialize the process-local SQL runtime with an unrelated 32-model project before
measurement. “Cold” means the measured project has no compiler cache or target artifacts; it does
not include one-time lazy initialization noise from the Python test worker. Phase timings are
attribution spans rather than additive buckets.

## Inspection command guard

The semantic benchmark's model graph is a spine plus independent chains, so it has no shared
dependencies. `test_inspection_command_performance.py` adds an opt-in lattice to the 3,000-model
semantic project: `shared_orders_hub` reads the end of the spine and fans out to eight models,
sixteen layers each join two neighbouring slots of the previous layer, and `shared_orders_rollup`
fans the last layer back in. Every lattice model is reachable along exponentially many paths. The
compile guards do not enable the lattice, so their fingerprints and budgets are unchanged.

After one compile and warm lineage and scope caches, the guard runs `sqb lineage` downstream from
the hub and upstream from the rollup as text and JSON, rich column traces upstream from
`shared_orders_rollup.amount` and downstream from `shared_orders_hub.amount`, `sqb dag --json` and
`sqb scope --json`, each in a fresh process. Every command
has a wall-time ceiling, a peak-RSS ceiling and an output-line bound linear in the model count, so
a renderer or graph walk that re-expands shared nodes per path fails on size before it can hide
behind a loose timing budget. It runs in the 3,000-model fresh-process compile job.

## Plan and build guards

The same inspection project also bounds `sqb plan --json` against an empty DuckDB warehouse. The
plan budget is a ratio to a warm `sqb compile --json` measured in the same test on the same runner,
plus a peak-RSS cap. The CPU-time ratio is the tight limit; a looser wall-time ratio backstops lost
parallelism and lock or I/O waits. A second case plans a 300-model copy of the benchmark and fails
when the planning CPU of the large project (plan CPU minus compile CPU) exceeds twice its linear
projection from the small one, so super-linear planning work fails on any runner.

`test_build_performance.py` builds a 1,000-model DuckDB benchmark without tests or audits from an
empty warehouse once per module, with wall and peak-RSS limits. Its existing-state cases copy the
built project, change the queries of a few models (and rename one table in the second case so
rename discovery matches it against stored fingerprints), and bound the next `sqb plan --json`.
All cases carry `models_3000` identifiers so they run only in the 3,000-model fresh-process job.

## Same-runner comparison with the base branch

Absolute budgets must absorb the spread between hosted runners, so every pull request that touches
the compiler also runs `scripts/compare_compile_performance.py` on the dense and the fresh
3,000-model benchmark. Base and head each compile a project written by their own generator, on the
same runner, three alternating runs per mode, and the medians of wall and CPU time must stay within
1.10x of base for each mode:

- cold: `sqb compile --json --no-cache` after removing `target`, after one untimed warm-up;
- warm: an unchanged `sqb compile --json` after one untimed run that writes the cache;
- edit: `sqb compile --json` after the same one-model edit as the release comparison below.

The base branch predates the default SQL Rules, so the fresh comparison runs twice. The
compiler-only job writes the head project with `select = []` and keeps the 1.10x limit for every
mode. The default Rules job allows 1.25x cold wall and 1.41x cold CPU: the 1.10x limit times the
measured cold cost of the default Rules over `select = []` on the same machine (1.14x wall, 1.28x
CPU, five paired runs). Its warm and edit modes keep 1.10x.

The fresh-process 3,000- and 5,000-model compile guards and the 3,000-model cache guard follow the
same split (approved 2026-11-01): `compiler_only` cases keep the previous wall budgets, and the
default Rules cases scale them by the measured cold cost (1.14x and 1.16x fresh, 1.19x for the cache
guard's cold and project-config runs). Larger cases already fit their budgets and are unchanged.

The warm and edit modes reuse the cold project. A warm run that misses the analysis cache, or an
edit run that does not, stops the comparison because it would measure the wrong path.

## Release comparison with the previous release

Fixed ceilings miss gradual creep and slowdowns that stay under a loose limit, so every release
pull request also compares the candidate with the previous published release on the same runner.
`scripts/compare_release_performance.py` installs the candidate wheel and the baseline from PyPI
into separate Python 3.12 environments, generates the inspection, build and dense benchmarks
with each version's own generator, and gives each version its own copy with compile, lineage and scope caches warmed by that version.
It then runs each command alternately for baseline and candidate, five times each, and compares
the medians of wall time and CPU time (user+sys) and the worst-run peak RSS. Peak RSS depends on
whether concurrent phases overlap, so it lands on one of a few levels from run to run; a median
flips between those levels, while the worst run is stable:

- uncached, warm and one-model edit `sqb compile --json`, `sqb plan --json`, `sqb dag --json`
  and `sqb scope --json` on the 3,000-model inspection benchmark. The edit run adds a
  revision-specific comment before the query of the middle model, so every run recompiles one
  changed model against a warm cache; it runs after the other inspection commands;
- `sqb lineage` downstream from the hub, upstream from the rollup, and the upstream column trace of
  `shared_orders_rollup.amount`;
- uncached, warm and one-model edit `sqb compile --json` on the 3,000-model dense all-rules
  benchmark, which each version generates with its own `write_dense_compile_project`. The
  uncached runs remove `target` first, so built-in and custom Rules run as in the dense guard;
- `sqb build` of the 1,000-model build benchmark from an empty warehouse, restored before each run.

A command fails when a candidate value exceeds the baseline by more than 25% and by more than
0.5 s (wall and CPU) or 32 MiB (peak RSS). The six compile commands use a fixed 15% wall and CPU
limit instead, so small per-pull-request compile regressions cannot accumulate across a release
unnoticed. The limits live in `scripts/release_performance/constants.py`. The baseline defaults to the highest version below
the candidate that is installable from PyPI, or release-tagged but not yet on PyPI; yanked releases
never count. A freshly tagged baseline is awaited until its wheels
are on PyPI, installing from the uncompressed simple index's wheel URL when the CDN still lags. A
command that needs a feature newer than a compared version declares `minimum_version` and appears
as skipped, with the reason, in the job summary; any other failure of either version fails the
check.

`.github/workflows/release-performance.yml` runs on every push to `main` and does not gate release
pull requests. A commit that bumps the version is compared with the release before it; any other
commit is compared with the release whose version it still carries. A newer push cancels the
comparison of an older commit. A regression fails the run and opens an issue labelled
`performance-regression`, or comments on the open one, with the commit and the comparison summary.
Run the comparison manually with an optional `baseline` input, or locally:

```bash
uv run python -m scripts.compare_release_performance --candidate 0.121.0 --baseline 0.119.1
```
