# Native crates

SQLBuild's native code is a Cargo workspace that builds one Python extension module,
`sqlbuild._native`, and the `sqb`/`sqlbuild` executables in the `sqlbuild-cli` wheel. Only `sqlbuild-python` knows about Python; every other crate is plain Rust
that can be tested and benchmarked with `cargo test` alone.

| Crate | Responsibility |
|---|---|
| `sqlbuild-build-features` | No code. Enables, in the `sqb` build, the dependency features the extension build resolves, so Cargo compiles the crates both builds share (polyglot-sql, `sqlbuild-analysis`, `sqlbuild-rules` and the rest) once. `make check-build-features` (part of `make check-ci`) fails when the two builds resolve a shared crate differently and names the features to add. |
| `sqlbuild-core` | Constants shared by several crates, the panic boundary that turns an unwinding native failure into a named compiler error, passive identity, diagnostic and SQL value types, text positions, a JSON emitter whose output matches Python's `json.dumps` and `orjson` byte for byte, and run-cost allocation with exact Python `decimal` default-context arithmetic and Snowflake query-history SQL. |
| `sqlbuild-cursor` | Microbatch cursor algebra: typed cursor values, grain flooring and stepping, half-open interval clamping, merging and batch splitting, timestamp text normalization and calendar durations, with Python `datetime` results and errors. Zones other than fixed offsets are resolved through a caller-supplied lookup. |
| `sqlbuild-state` | Append-only warehouse state rows: content-addressed migration event IDs, typed event SQL literals and statements, and source freshness data-version normalization, hashing, equivalence, age policy and latest-state read SQL, byte-compatible with the rows Python wrote. It also owns execution history: lifecycle event batch digests and conflicts, run projection, event and run cursors, filter and page validation, and the SQLite and PostgreSQL statements the native history backends run. |
| `sqlbuild-pysource` | Static reading of project Python source with the ruff parser: macro module structure, literal SQL arguments, definition locations and string-literal edits. It never imports or runs user code; CPython stays the authority on whether a file is valid Python. |
| `sqlbuild-cache` | The shared native store every native cache uses (one atomically replaced, checksummed file per cache kind, keyed by content digests) and the one fingerprinting scheme for cache keys: the content digest and the project file fingerprint. |
| `sqlbuild-sqltext` | Lexical SQL text without polyglot: comment, quote and parenthesis scanning, the rules quote policy, model header tokenization and matching, static variable substitution, static reference extraction, and the general reference scan with table-function call arguments under each adapter's lexical rules. |
| `sqlbuild-config` | Configuration file reading without Python: TOML, YAML 1.2 with the core schema and merge keys, and typed project and local config readers for the fields discovery needs. It also writes the workspace files `sqb playground` and `sqb skills` create from packaged templates and skills. Errors carry their kind and position; YAML outside the supported subset is an `Unsupported` error. |
| `sqlbuild-host` | The contract of the permanent Python edges as plain Rust: the catalogue of `sqlbuild._native_host` entries and the trait that resolves them, the per-thread compile timing collector with its process clock, the compile diagnostic sink, and the executor result types the Python result dataclasses and enums mirror. `sqlbuild-python` implements the traits. |
| `sqlbuild-discovery` | Native project discovery and the only implementation of the model, SQL test, scenario, source and schema file collections and the declaration layout: the shared directory walk with Python's glob and sort semantics, file reading with Python's newline handling and read errors, and parsing of authored files with Python's exact discovery messages. Results are plain data the Python discovery facade materialises. |
| `sqlbuild-scopes` | Declaration scopes, visibility, relationship grants, placement, report queries and the offline scope cache, including dialect-aware SQL-test and scenario scans. |
| `sqlbuild-model-config` | MODEL configuration, columns and audits, template expansion over caller values, and dynamic-column name patterns using Rust regex syntax. |
| `sqlbuild-render` | Macro call scanning, argument evaluation, expansion, splicing, memoisation and persistent call-store keys. Python executes user macro bodies at the extension boundary. |
| `sqlbuild-attachments` | Native attachment of tests, audits, sources, functions, scenarios and seeds, including argument merging, validation and audit policy resolution. Adapter value rendering remains an extension boundary. |
| `sqlbuild-analysis` | SQL analysis over polyglot: query analysis, binding, semantic validation and usage, column references, type normalization, FAST/RICH lineage, SQL-test/scenario extraction and compiler-side assembly, and Snowflake cost statement classification and query-tag parsing. |
| `sqlbuild-rules` | Built-in rules and the rules engine, the custom-rule host, SQL lint, quality checks and formatting, rules configuration and the request models. It also owns the build identity script. |
| `sqlbuild-refactor` | Native rename/move plans, contract-adoption edits, MODEL/SCHEMA/YAML text edits and atomic staging/commit with rollback. Shared scope placement delegates to the native scope implementation; user Python source inspection remains at the Python AST boundary. |
| `sqlbuild-project` | The compiled project as plain Rust: `CompiledProject` with the producers' facts and each resource's records (model inputs and schemas, compiled models and their analysis, sources, functions, audits and seeds), and what those records answer without Python, such as lineage rows, resolved namespaces and dependency keys. Values the host owns, such as user config and discovered files, and the views it builds lazily live in one slot per record, whose types `sqlbuild-python` supplies through the `ProjectHost` trait; reads that need Python stay in the bindings as extension traits. |
| `sqlbuild-cli` | The `sqb` and `sqlbuild` executables: `--version`, unchanged compiles replayed from the shared reuse store with Python's exact streams, exit code and compute log capture, and a handoff of every other invocation to the installed `sqlbuild-python` script. Built as its own wheel (`crates/sqlbuild-cli/pyproject.toml`). |
| `sqlbuild-python` | The only PyO3 crate: the `_native` module, its Python classes and functions, conversions from Python objects, and the process allocator. |

## Layering

Dependencies point one way, from the top of this graph to the bottom:

```text
sqlbuild-python
  -> sqlbuild-project -> sqlbuild-refactor -> sqlbuild-rules -> sqlbuild-analysis -> sqlbuild-attachments
  -> sqlbuild-model-config -> sqlbuild-scopes -> sqlbuild-render -> sqlbuild-discovery -> sqlbuild-host
  -> sqlbuild-config -> sqlbuild-sqltext -> sqlbuild-cache -> sqlbuild-pysource -> sqlbuild-state -> sqlbuild-cursor -> sqlbuild-core
sqlbuild-cli -> sqlbuild-cache, sqlbuild-core, sqlbuild-build-features
```

`sqlbuild-scopes` reuses the macro scanner in `sqlbuild-render`; render itself depends only on
`sqlbuild-cache` and `sqlbuild-core`. Their place in the order fixes which crates may
use them. The JSON emitter
lives in `sqlbuild-core` so that any layer can produce text that must equal Python's
`json.dumps` output.

Each crate may depend only on crates below it, and may also skip layers. The order is declared
in `[workspace.metadata.native-layers]` in the workspace `Cargo.toml`, and
`make check-native-layering` (part of `make check-ci`) fails when:

- a crate depends on a crate that is not in an earlier layer, or a workspace crate is missing
  from the order;
- any crate other than `sqlbuild-python` reaches `pyo3`, directly or through another package;
- a crate below `sqlbuild-analysis` reaches polyglot.

The same check fails when the workspace version, the `pyproject.toml` version and
`.release-please-manifest.json` disagree, or when a workspace crate's `Cargo.lock` entry carries a
version other than its manifest's, because the version is part of the build identity.

`fensu check` applies the Rust structure rules inside each crate: other crates use items through
`main/` entry points, `models.rs`, `types.rs` and `constants.rs`, not through `_helpers/`.

Inside `sqlbuild-python`, the custom rule XSB075 orders the `bindings/_helpers` modules in layers
that follow the planned crate split: shared Python plumbing (`infra`), then the glue of the lower
crates (`front`), the compiled project (`project`), planning and compilation as siblings (`plan`,
`compile`), command output (`output`), command handlers (`commands`) and registration
(`registry`). A module may import only from lower layers; `plan` and `compile` may not import each
other. `module_layers` in `fensu.toml` assigns every helper module a layer. `pending_budgets` is
empty: every import points down, and a new upward import fails the check.

## Python module paths

`sqlbuild-python` and `sqlbuild-cli` import Python by module path strings. The repository test
`tests/unit/scripts/native_module_paths` (`scripts/native_module_paths`) keeps them honest:

- every `"sqlbuild...."` literal must be a complete dotted path that imports as a module or
  resolves as a module attribute; constants whose name contains `LOGGER` hold `logging`
  logger names and are not imported;
- attributes are checked where a literal module, or a module constant defined in the same file,
  is passed with a literal name to an import helper (`imported(py, MODULE, "name")`), to
  `import(MODULE)?.getattr("name")`, or written as a `("sqlbuild.module", "name")` pair;
- module paths are never built at runtime: a `format!` or `concat!` template may not start with
  `sqlbuild.` or with a `{..PACKAGE..}.`/`{..MODULE..}.` constant. Write the complete module
  path as a constant instead;
- every row of the host table (`bindings/_helpers/native_host/`) must resolve as an attribute of
  `sqlbuild._native_host`.

Staying-Python entry points (adapter, user code, executor and runtime, third-party and public
facade) are reached through the host table and `native_host::host::entry`, which imports only
`sqlbuild._native_host`. Classes, enums, errors and functions `sqlbuild._native` registers are
reached through the typed registry (`records::registry` with `RecordId`, `ErrorId` and
`NativeAttr`), never by module path.

## Compiler

The compiler runs discovery, rendering and analysis natively. Generic compiler stages do not
rescue native failures through a Python implementation; user macros, custom rules and adapter
callbacks keep their explicit Python boundaries. A failing render raises its first error directly
and runs each macro call at most once; a divergence the bridge detects is reported as a native
stage mismatch. Because the macro-call store is always on, the
[macro determinism contract](../website/src/content/docs/docs/concepts/macros.mdx) applies to
every compile.

`make compiler-differential` runs the full per-PR corpus once and checks every output against
the goldens in `tests/goldens/compiler`, with stage captures and full discovery, render and
analysis coverage required. CI also runs it on Python 3.13 and 3.14, and compiles and builds a
playground project on Windows.

## Working on the crates

- Tests live next to the module they cover; run them for one crate with
  `cargo test -p <crate>` or for all crates with `cargo test --workspace`.
- The build identity (`sqlbuild._native.BUILD_IDENTITY`) hashes the sources of every crate in
  this directory, so editing any crate invalidates the native caches keyed on it.
- maturin builds `crates/sqlbuild-python`; the release version lives in
  `[workspace.package]` and is shared by every crate.
