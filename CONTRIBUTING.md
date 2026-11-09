# Contributing

Thanks for helping improve SQLBuild. Keep changes focused, tested, and easy to review.

## Setup

SQLBuild supports Python 3.12, 3.13 and 3.14.

Install dependencies with:

```bash
uv sync --python 3.12 --all-extras
```

## Development Workflow

- Create a branch for your change.
- Keep pull requests focused on one behavior change or cleanup.
- Add or update tests for behavior changes.
- Prefer small, direct changes over broad rewrites.
- Do not commit secrets, credentials, machine-local config, or generated scratch files.

## Checks

### Native development loop

After installing dependencies, rebuild the Python extension after Rust edits with:

```bash
make dev-native
uv run --no-sync pytest tests/integration/src/sqlbuild/compiler/compiled_project/test_native_resource_diagnostics.py -n auto --dist loadfile
```

Choose the test file covering your change. `dev-native` builds the extension in place with
Maturin's development profile and skips wheel installation and dependency resolution. Use
`uv run --no-sync` for subsequent focused checks so the package manager does not replace that
extension with a release build. Python-only source edits need no native rebuild.

For a change confined to one Rust crate, run its focused tests before rebuilding the extension:

```bash
cargo test -p sqlbuild-sqltext given_model_files_when_matching_headers_then_offsets_follow_the_python_pattern
```

Control-plane integration tests require Docker and run both SQLite and PostgreSQL:

```bash
make test-native NATIVE_TEST_ARGS='-p sqlbuild-orchestration --lib'
```

The same Testcontainers harness runs under `make rust-check` in CI. Each invocation starts one
digest-pinned PostgreSQL container, waits for database readiness, shares it across the suite with
an isolated schema per fixture, and removes it after success or failure. No host PostgreSQL setup
is needed. Missing Docker, startup failures or missing database configuration fail the gate;
PostgreSQL cases never silently skip. `NATIVE_TEST_ARGS` accepts Cargo test arguments; its default
runs the workspace with all features.

Keep the toolchain, `CARGO_TARGET_DIR`, profile and `RUSTFLAGS` consistent between iterations.
Changing them can invalidate compiled artifacts. Coordinate builds that share a target directory;
do not delete another task's artifacts or change global compiler-cache settings. The first build
for a new profile is a setup cost, not an incremental-build measurement.

The development profile keeps workspace crates unoptimised with line-table debug information;
dependencies use light optimisation without debug information. This trades development runtime
speed for faster changes to our code while avoiding fully unoptimised dependency execution.

Development builds are for correctness iteration. Before measuring compiler performance, restore
the production profile and run measurements without concurrent builds:

```bash
uv run --no-sync maturin develop --release --skip-install
```

Do not compare development-profile timings with release benchmarks. Dependency or packaging
changes still require `uv sync --all-extras`; `--skip-install` does not refresh installed metadata,
dependencies or console scripts.

Always pass `--skip-install` to `maturin develop`, or use `make dev-native` or `make fast-native`.
Without it, maturin installs the `sqlbuild-cli` version that `pyproject.toml` pins from the package
index, not this checkout's `crates/sqlbuild-cli`: the `sqb` executable is then a released build,
and between a release commit and its publication the install fails because that version does not
exist yet. `uv sync` and `uv run` install the workspace `sqlbuild-cli` and rebuild the extension
when Rust sources change.

Before opening a pull request, run:

```bash
make check
```

For broader validation, run:

```bash
make verify
```

In CI, every push to a pull request runs a fast Linux core: static checks, unit and integration
tests and native workspace checks, plus the UI checks for UI changes. A change confined to the web
UI and documentation runs only the UI checks. The full matrix (wheels, Compose, Windows, macOS,
DuckDB end-to-end, warehouse, compiler-golden and timing jobs) runs once a maintainer labels a
pull request that is not a draft `full-verify`, normally when it is next to merge. The required
`Verify` check only reports after every suite the change needs has passed, so a pull request that
needs the full matrix cannot merge until it has run.

For focused iteration, run the narrowest relevant pytest target, for example:

```bash
pytest tests/unit/src/sqlbuild/compiler/compile/test_main.py
```

To check whether a branch introduces copied or drifting code in Python or Rust, run:

```bash
uv run fensu dupes --since origin/main
```

`make dupes` reports the whole repository. The report is advisory and never needs to reach zero;
`[dupes]` in `fensu.toml` configures exclusions and contract exemptions. See `fensu dupes --help`.

## Tests

- Mirror source layout under `tests/<scope>/src/sqlbuild/...`.
- Prefer dataclass-backed test cases for non-trivial scenarios.
- Use clear Given-When-Then test names.
- Prefer real parsing, filesystem, compile, and planner behavior over mocks when practical.
- For substantive behavior changes, validate test strength with a small semantic mutation and confirm the relevant test fails before reverting it.

## Project Conventions

- Keep `sqb compile` offline and static unless intentionally changing that contract.
- Keep examples reproducible and avoid environment-specific assumptions.
- Use project-local config files for local overrides, and do not commit credentials or personal settings.
- Follow existing code style and module organization before introducing new patterns.

### Native ownership and Python interfaces

Compiler and planner algorithms belong in the Rust crates. Keep parsed header metadata, config
nodes, dependency graphs and selections native between their producers and consumers. Constructing
a Python container or public record only to extract it again is not an interface requirement.

The supported Python interfaces have specific owners:

| Interface | Why Python remains | Ownership rule |
| --- | --- | --- |
| Adapters and warehouse execution | Projects subclass adapters; the executor runs warehouse operations | Call through the adapter/executor boundary, retaining their public inputs and results |
| User functions, factories, hooks, rules and custom constructors | These execute project-authored Python | Preserve invocation timing, errors and opaque return values; do not execute them speculatively |
| Public records and caller-authored protocol objects | Python callers access, replace, compare and serialize documented records | Project native values on public access; import caller-authored values at entry, not repeatedly inside native algorithms |
| Config and sample values | Python can supply values without a lossless native representation | Retain opaque values and their Python semantics; keep ordinary builtin containers native |
| Streams, progress, logging and interpreter shutdown | These integrate with Python's runtime and caller-owned resources | Keep resource lifetime and error propagation at the owning boundary |

`python-boundary.toml` lists the Python source that intentionally stays. The native crossing,
storage and layer rules in `fensu.toml` prevent new accidental dependencies; pending budgets are
an inventory to reduce, not permission to add callbacks. `native_host` owns genuine host edges,
not a destination for movable compiler or planner algorithms. Remove an obsolete private
interface with its last caller, while retaining supported public facades and stubs.

## Commits And Pull Requests

- Use concise conventional commit messages, such as `fix: ...`, `feat: ...`, `test: ...`, or `docs: ...`.
- In pull requests, summarize the behavior change and list the checks you ran.
- Call out breaking changes, migrations, or compatibility decisions explicitly.

Before creating or editing a pull request, write its description to a file and validate the
current branch, proposed title, and description locally:

```bash
make check-pr-metadata PR_TITLE='type: concise summary' PR_BODY_FILE=/path/to/pr-body.md
```
