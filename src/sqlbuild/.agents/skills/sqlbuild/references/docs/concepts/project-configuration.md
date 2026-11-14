<!-- generated-by: sqlbuild skills -->

# Configuration reference

> Every key in sqlbuild_project.toml and sqlbuild_local.toml.

Online: https://sqlbuild.com/docs/concepts/project-configuration/

## Contents

- sqlbuild_project.toml
- Targets
- Defaults
- Constants
- Path defaults
- Settings
- Rules
- Scopes
- References
- Project variables
- Janitor
- Migrations
- Scenario
- Git
- Review
- dbt
- Skills
- Project Python
- sqlbuild_local.toml

Every key in `sqlbuild_project.toml`, which you commit, and `sqlbuild_local.toml`, which each
developer keeps out of Git. For a walkthrough, see [Set up a project](../project-setup.md).

Where macros, constants and enums are visible is set by folders, not in these files; see
[Declaration Scopes](declaration-scopes.md).

## sqlbuild_project.toml

A complete example:

```toml
name = "waffle_shop"
adapter = "duckdb"
default_target = "dev"

[connections.local]
database = "waffle_shop_control.duckdb"

[settings]
default_audit_severity = "warn"

[defaults]
materialized = "table"

[constants]
collection_rendering = "value_list"

[targets.prod]
connection = "local"
schema = "prod"

[targets.dev]
connection = "local"
schema = "dev"

[path_defaults.staging]
materialized = "view"
```

### Required fields

| Field | Description |
|-------|-------------|
| `name` | Project name |
| `adapter` | Database adapter: `duckdb`, `motherduck`, `snowflake`, `bigquery`, `databricks`, `postgres`, `sqlite`, `sqlserver`, or `clickhouse`. See [Adapters](adapters.md). |

### Optional fields

| Field | Description |
|-------|-------------|
| `default_target` | Name of the target to build against when none is selected (see [Selecting a target](#selecting-a-target)). |

### Named connections

Define reusable connections under `[connections.<name>]`, then reference one by name from
each target. Connections own endpoint, authentication, and compute settings. Targets own the
authoritative `database`, `schema`, variables, and operational policy.

```toml
[connections.local]
database = "my_project.duckdb"

[targets.dev]
connection = "local"
schema = "dev"
```

Multiple targets can reuse one connection while keeping separate namespaces and policies:

```toml
[connections.warehouse]
account = "my_org-my_account"
user = "${ENV:SNOWFLAKE_USER}"
password = "${ENV:SNOWFLAKE_PASSWORD}"
warehouse = "TRANSFORM_WH"

[targets.prod]
connection = "warehouse"
database = "ANALYTICS"
schema = "PROD"

[targets.dev]
connection = "warehouse"
database = "ANALYTICS"
schema = "DEV_ALICE"
```

When the project defines exactly one named connection, across `sqlbuild_project.toml` and
`sqlbuild_local.toml`, a target that doesn't set `connection` uses it. With several named
connections, every target must choose one; SQLBuild rejects a target without `connection` while
loading configuration.

```toml
[connections.local]
database = "my_project.duckdb"

[targets.dev]
schema = "dev"    # uses connections.local, the only named connection
```

A command that needs the warehouse, such as `build`, `plan`, `query`, `diff`, or `debug`, fails
with `D001` before connecting when no connection is configured. Commands and modes that don't
connect, such as `compile`, `format`, `lineage`, `scope`, `rename`, `mv`, `test --inspect`, and
`scenario test --local`, keep working without one.
SQLBuild never falls back to an implicit in-memory DuckDB database; to use one, set it
explicitly:

```toml
[connections.memory]
database = ":memory:"
```

Where models build depends on how the project names its connection:

| Project shape | Models, intermediate loaders and the janitor use |
|---------------|--------------------------------------------------|
| One top-level `[connection]`, no `[connections.*]` and no `[targets]` | `[defaults]`, then the connection's `database` and `schema` |
| Named `[connections.*]` or any `[targets]` | The target's `database` and `schema`, then `[defaults]`; never the connection's |

A project with a single top-level `[connection]` treats it as its default target, so this is
enough on every adapter:

```toml
name = "orders"
adapter = "snowflake"

[connection]
account = "my_org-my_account"
user = "${ENV:SNOWFLAKE_USER}"
password = "${ENV:SNOWFLAKE_PASSWORD}"
database = "ANALYTICS"
schema = "ORDERS"
```

Once the project names connections or targets, a `database` or `schema` on a connection is
ignored, and the `S101`, `S103` or `S104` error for a missing location names the ignored value and
the setting to add. Managed sources load into their own `database` and `schema`, then the
target's, as described in [Targets](#targets).

A target that names an unknown connection fails when the configuration loads, before anything
connects.

## Targets

A target is a named build context - the database and schema you build into, plus execution policy (for example `dev` and `prod`). Each target references a named connection (or uses the only one defined; see [Named connections](#named-connections)) and can configure:

| Field | Description |
|-------|-------------|
| `schema` | Schema for all models in this target; required for named targets |
| `loader_schema` | Default write schema for managed source loaders; falls back to `schema` |
| `database` | Database for all models in this target |
| `connection` | Name from `[connections.<name>]` used for endpoint, authentication, and compute; optional when exactly one named connection is defined |
| `vars` | Target-specific project variables |
| `defer_sources_to` | Target name to read managed source data from (see [Loaders](python-nodes/loaders.md#source-deferral)) |
| `clone` | Clone policy (see below) |
| `missing_migration_origin` | What a build does when a declared `migrate_from` origin does not exist in this target: `allow` (default), `require_confirmation`, or `deny`. See [Missing migration origins](models/migrations.md#missing-migration-origins) |
| `warehouses` | Snowflake warehouse per command group, `build` and `query`. See [Command-group warehouses](adapters/snowflake.md#command-group-warehouses) |
| `concurrency` | Worker limit for commands run against this target, replacing `[settings] concurrency` (see below) |

```toml
[connections.warehouse]
account = "my_org-my_account"
warehouse = "TRANSFORM_WH"

[targets.prod]
connection = "warehouse"
database = "analytics"
schema = "analytics_prod"
loader_schema = "raw_prod"
defer_sources_to = "prod"

[targets.dev]
connection = "warehouse"
database = "analytics"
schema = "analytics_dev"
loader_schema = "raw_dev"
defer_sources_to = "prod"

[targets.staging]
connection = "warehouse"
database = "analytics"
schema = "staging"

```

Managed loader writes use the active target's `loader_schema`, falling back to its model
`schema`. Managed source reads use the target named by `defer_sources_to`, or the active
target itself when deferral is omitted. In the example, `load --target dev` writes to
`raw_dev`, while models built in dev read from `raw_prod`.

SQLBuild rejects targets on the same warehouse/database when their managed loader writes
resolve to the same schema. Two targets may read the same schema through deferral, but they
cannot both own loader writes there.

### Selecting a target

The active target is determined by (in order of precedence):

1. `--target` on the command line (highest priority)
2. `sqlbuild_local.toml` `target` field
3. `default_target` in `sqlbuild_project.toml`
4. No target. Named connections are chosen through targets, so warehouse commands fail with
   `D001` unless a target is selected.

A typical developer keeps shared target definitions in `sqlbuild_project.toml` and selects
their normal target once in the optional local file:

```toml
# sqlbuild_local.toml
target = "dev"

[targets.dev]
connection = "warehouse"
schema = "dev_alice"
loader_schema = "raw_alice"

[connections.warehouse]
user = "alice"
password = "${ENV:SNOWFLAKE_PASSWORD}"
```

Commands then use `dev` automatically. An explicit command such as `sqb build --target prod`
still takes precedence for that invocation.

### Target concurrency

Targets often run on very different warehouses. A target can set its own worker limit, so local
and CI runs use many workers while production stays within what its warehouse runs without
queuing:

```toml
[settings]
concurrency = 16

[targets.prod]
concurrency = 8
```

`sqb build`, `sqb test`, `sqb audit`, `sqb seed`, `sqb load`, and `sqb scenario test` use the first
value set in this order:

1. `--concurrency` on the command line, for that invocation only
2. The selected target's `concurrency`, from `[targets.<name>]` in either file
3. `[settings] concurrency` in `sqlbuild_local.toml`
4. `[settings] concurrency` in `sqlbuild_project.toml` (default `1`)

The value must be an integer of at least `1`; anything else fails with `D001`. `sqb build
--verbose` shows the effective limit and the setting it came from, for example
`8 configured limit (targets.prod.concurrency)`. `sqb scenario test` has no `--target` option and
uses the active target's value.

### Clone policies

Targets can declare whether they allow cloning to or from:

```toml
[targets.prod]
schema = "prod"

[targets.prod.clone]
allow_as_clone_origin = true
allow_as_clone_destination = false

[targets.dev]
schema = "dev"

[targets.dev.clone]
allow_as_clone_origin = false
allow_as_clone_destination = true
```

Both policies default to `false`. `sqb clone --from prod --to dev` requires `allow_as_clone_origin = true` on `prod` and `allow_as_clone_destination = true` on `dev`.

### Execution limits

A target can cap what one `sqb build` runs, so a broad selector on a development target stops
instead of running up warehouse cost. A target without this section has no limits.

```toml
[targets.dev]
connection = "warehouse"
database = "analytics"
schema = "dev_alice"

[targets.dev.execution_limits]
max_models = 125
max_duration = "45m"
test_timeout = "60s"
scenario_timeout = "10m"
remediation = """
Review the expanded plan and confirm that the selected scope is intentional.
Automated tools must request approval before changing this policy.
"""
```

| Field | Description |
|-------|-------------|
| `max_models` | Most models one build may run. A larger selection stops before anything runs; `sqb plan` has no limit |
| `max_duration` | Longest one build may run, such as `"45m"`. At the deadline SQLBuild cancels the statements still running |
| `test_timeout` | Time limit for each unit test on this target, or `"disabled"`; replaces `[settings] test_timeout` |
| `scenario_timeout` | Time limit for each scenario on this target, or `"disabled"`; replaces `[settings] scenario_timeout` |
| `remediation` | Text added to limit errors, such as how your team approves a larger build |

See [Limits](../cli/build.md#limits) for how each warehouse enforces them. `sqlbuild_local.toml`
accepts the same section: the fields it sets replace the project's, and the rest are inherited.

### Diff limits

`sqb diff` without `--full`, `--bounded` or `--schema-only` runs a full comparison only when every
selected table is small enough. Each target sets the largest table it will compare in full this
way:

```toml
[targets.prod.diff]
max_full_rows = 50_000_000

[targets.dev.diff]
max_full_rows = "unlimited"
```

`max_full_rows` is a positive integer or `"unlimited"`. When it is not set, the limit is
10,000,000 rows. Each side of a comparison uses its own target's limit. See
[`sqb diff`](../cli/diff.md#size-guard) for how sizes are read and what happens when a table is too
large.

`sqlbuild_local.toml` accepts the same section, and a local value replaces the project value for
that target:

```toml
[targets.dev.diff]
max_full_rows = 100_000_000
```

## Defaults

Project-wide defaults for models, seeds, and functions:

```toml
[defaults]
materialized = "table"
incremental_strategy = "delete_insert"
replay_on_change = "full"
tags = ["managed"]
```

These apply to all models unless overridden by path defaults or the model's own `MODEL()` header.

`[defaults]` accepts only the following keys. Any other key, including other `MODEL()` header
fields, fails compilation with `D001`; set those in `[path_defaults]` or the `MODEL()` header
instead.

- Placement: `database`, `schema`, `seed_database`, `seed_schema`, `function_database`,
  `function_schema`
- Materialization: `materialized`, `contract`, `full_refresh`, `replay_on_change`, `tags`
- Incremental: `incremental_strategy`, `incremental_mode`, `merge_exclude_columns`,
  `append_cursor_inclusive`, `lookback`, `unaccounted_partition_policy`
- Cursors: `cursor_start`, `cursor_end`, `cursor_watermark_mode`, `cursor_future_action`,
  `cursor_future_max_distance`, `cursor_start_max_action`, `cursor_start_max_ahead`
- Microbatch: `microbatch_strategy`, `batch_size`, `batch_concurrency`, `max_microbatches`
- Row diff: `row_diff_exclude_columns`, `row_diff_tolerances`, `row_diff_sample_rows`,
  `row_diff_sample_seed`
- Execution: `max_duration` (see [Execution time limit](models/configuration.md#execution-time-limit))

## Constants

Collection constants default to parenthesized SQL value lists. Set a project-wide default when lists and sets should instead compile to first-class adapter-native arrays:

```toml
[constants]
collection_rendering = "array"
```

`collection_rendering` accepts `value_list` (the SQLBuild default) or `array`. A public constant's `render_as` field or a model-local `constant(...)` wrapper overrides the project setting. The complete precedence order is declaration override, project setting, then `value_list`.

This setting does not make unsupported adapter features portable. In particular, SQL Server rejects native arrays, and BigQuery rejects nested arrays. See [Collections and Rendering](constants/collections-and-rendering.md#project-default) for syntax, adapter output, and value-list usage constraints.

## Path defaults

Per-directory model defaults. Useful for applying different config to different parts of your project. Each path default accepts any `MODEL()` header field; an unknown key fails compilation with `D001`:

```toml
[path_defaults."models/staging"]
materialized = "view"
tags = ["staging"]

[path_defaults.marts]
materialized = "table"
tags = ["marts"]
replay_on_change = "full"
```

Path matching uses the path below `models/`. A model at `models/staging/stg_orders.sql` matches the
`staging` path default. A key matches its folder and every folder below it. In a key, `*` matches one
folder name and `**` matches any number of folders, so `"**"` matches every model.

### Which path default applies

Each model uses at most one path default: the nearest matching key. Path defaults are not merged, so
a model gets nothing from a broader key when a more specific key also matches it.

1. A key without wildcards beats every wildcard key. Among those, the deepest key wins, so
   `staging/orders` beats `staging`.
2. When only wildcard keys match, the most specific wins: more literal folder names first, then more
   `*` segments, then fewer `**` segments. Two equally specific matches fail compilation.

A broad `"**"` default therefore applies only to models that no other key matches. With the
configuration below, `models/orders.sql` gets the `"**"` description, but
`models/staging/stg_customers.sql` uses only the `staging` entry. It has no description and fails
with `P010`:

```toml
[path_defaults."**"]
description = "Order analytics model."

[path_defaults.staging]
materialized = "view"
```

Descriptions belong to individual models, so put each one in the `MODEL()` header, or in the
`description` of the model's `model_schema`. `[defaults]` rejects `description` with `D001`. If a
folder of models really shares one description, every key those models can select must set it:

```toml
[path_defaults."**"]
description = "Order analytics model."

[path_defaults.staging]
materialized = "view"
description = "Staging model."
```

### Config layering order

Configuration is layered in this order, with later layers overriding earlier ones:

1. **Project defaults** (`defaults`)
2. **Path defaults** (`path_defaults`) - the one nearest matching entry, if any (see
   [Which path default applies](#which-path-default-applies))
3. **MODEL() header** - the model's own config

Most keys are overridden by the more specific layer, but three merge instead:

- `tags` are *unioned* across layers. A model with `tags [marts]` in its header that matches a path default with `tags [managed]` will have both tags.
- `row_diff_exclude_columns` lists are unioned across layers.
- `row_diff_tolerances` mappings are deep-merged across layers, so a header tolerance for one column adds to (rather than replaces) tolerances declared in defaults or path defaults.

Diff sampling values use ordinary replacement precedence. `row_diff_sample_rows = 0` explicitly
disables an inherited sample for a path or model. CLI `--sample-rows`, `--sample-seed`, and
`--exhaustive` override the compiled configuration for one invocation.

## Settings

Global feature toggles:

```toml
[settings]
sql_analysis = true
require_sql_analysis = false
query_change_tracking = true
check_declared_columns = true
concurrency = 1
microbatch_concurrency = false
auto_load_sources = true
table_promotion_mode = "staged"
default_audit_severity = "error"
default_audit_run_scope = "delta_and_final"
unit_test_max_real_models = 10
test_timeout = "30s"
scenario_timeout = "15m"
delta_compile = true
```

| Field | Default | Description |
|-------|---------|-------------|
| `sql_analysis` | `true` | Enable SQL syntax analysis, column binding, type inference, output inference, and semantic validation at compile time |
| `require_sql_analysis` | `false` | Allow `sql_analysis false` (in a MODEL header or a `[path_defaults]` entry) only on models whose SQL the parser cannot read, and reject `[settings] sql_analysis = false` in `sqlbuild_local.toml`. Project file only. See [SQL analysis](models/configuration.md#sql-analysis). |
| `query_change_tracking` | `true` | Rebuild a model when its compiled query changes. Whitespace, comments, and the case of reserved keywords and built-in function names don't count as changes |
| `check_declared_columns` | `true` | Check that models without a `contract` setting output their declared columns, and that `nullable false` columns can't be null. `false` makes those columns documentation and audit targets only. See [Contracts](models/contracts.md) |
| `concurrency` | `1` | Default number of worker connections for commands that run resources in parallel, such as `sqb build`, `sqb test`, `sqb audit`, and `sqb seed`, and for `sqb janitor` actions on warehouses that run DDL concurrently; a target's `concurrency` and `--concurrency` override it (see [Target concurrency](#target-concurrency)) |
| `microbatch_concurrency` | `false` | Allow models with `batch_concurrency > 1` |
| `auto_load_sources` | `true` | Automatically run source loaders before building dependent models during `sqb build`. See [Loaders](python-nodes/loaders.md). |
| `table_promotion_mode` | `staged` | `staged` builds a table under a staging name and replaces the old one only after audits pass; `immediate` builds in place and audits afterwards. See [Table](models/materializations.md#table) |
| `default_audit_severity` | `error` | Default severity for audits: `warn` or `error` |
| `default_audit_run_scope` | `delta_and_final` | Default run scope for audits: `final` or `delta_and_final` |
| `unit_test_max_real_models` | `10` | Most real (unmocked) models one SQL unit test may execute; a larger test fails compilation. See [Unit test and scenario limits](#unit-test-and-scenario-limits). |
| `test_timeout` | `30s` | Time limit for each SQL unit test, or `"disabled"` |
| `scenario_timeout` | none | Time limit for each scenario, or `"disabled"` |
| `delta_compile` | `true` | After an edit to model SQL or SQL test files only, compile just the changed files against the previous compile. See [Compiling after an edit](../cli/compile.md#compiling-after-an-edit). |

### Unit test and scenario limits

| Setting | Limits | Details |
|---------|--------|---------|
| `unit_test_max_real_models` | Real (unmocked) models one SQL unit test may run; a larger test fails compilation and names each model, the path that pulls it in, and the inputs to mock | [Real-model limit](testing-reference.md#real-model-limit) |
| `test_timeout` | Each unit test's warehouse time, from its first statement | [Time limits](testing-reference.md#time-limits), including precedence |
| `scenario_timeout` | Each scenario's warehouse time; no default | Below |

The scenario limit that applies is the first one set of `--scenario-timeout`,
`SCENARIO(timeout "...")`, `[targets.<target>.execution_limits] scenario_timeout`, and
`[settings] scenario_timeout`. At the limit, SQLBuild cancels the scenario's running statement,
reports it as `TIMEOUT`, and continues with independent items, as for
[unit tests](testing-reference.md#time-limits). Every value is a positive fixed duration such as
`"45s"` or `"10m"`, or `"disabled"`. `sqlbuild_local.toml` accepts all three settings.

## Rules

Rules configuration belongs in the shared `sqlbuild_project.toml` so local and CI compilation use
the same checks:

```toml
[rules]
select = ["SQLCLEAN", "SQBRGRAPH", "XSQBRARCH"]
```

Without `select`, the SQL Rules enabled by default run; `select = []` runs no configurable Rules.
Exact codes activate individual checks and prefixes activate a family. SQL Rule codes begin with
`SQLCLEAN`, other built-in codes with `SQBR`, and repository-defined codes with `XSQBR`. See
[Compiler-integrated Rules](rules.md) for configuration, authoring, and exceptions.

| Key | Description |
| --- | --- |
| `ignore` | Rule codes or families turned off for the whole project. |
| `[[rules.rule_exceptions]]` | One reviewed exception: `rule`, `path` and `reason`, plus `relation` and optional `cte` for relation-level Rules such as `SQLCLEAN038`. Stale entries fail compilation. See [Findings and exceptions](rules/findings-and-exceptions.md). |
| `[[rules.rule_ignores]]` | Rules turned off for `paths` or `selectors`, with a `reason`. |

Rule exceptions live only in this file. Inline `-- sqb: ignore` comments are `SQBRSUPPRESSION000` errors
that print the entry to add.

## Scopes

Declaration placement findings are errors by default. Set `enforce_placement = false` to report
them as warnings for the whole project while you move declarations into place:

```toml
[scopes]
enforce_placement = false
```

Visibility errors are unaffected. See
[Where to Put Declarations](declaration-scopes/placement.md).

## References

Every reference to a model, source, or seed must be explicit and typed. By default:

- A macro whose expanded output contains `__ref()`, `__source()`, or `__seed()` fails to compile
  with `P006`. Pass the reference to the macro as an argument instead; see
  [Typed reference arguments](macros/python-macros.md#typed-reference-arguments).
- Literal SQL passed to `ctx.query()` or `ctx.execute_sql()` in a task, asset, check, loader, or
  Python hook that names a project relation fails to compile with `P008`.
- The same check runs on SQL sent at run time and reports a `P008` warning; the query still runs.
  See [Hard-coded relation names](python-nodes/sql-references.md#hard-coded-relation-names).

One compile reports every `P006`, `P007`, and `P008` violation in the project, in the terminal and in
`sqb compile --json`.

While migrating a project from another tool, turn these checks off for the whole project:

```toml
[references]
enforce_explicit = false   # default true
```

The switch has no per-model or per-folder form. Undeclared `ctx.relation(...)` calls still fail,
and declared hook reads still order the build.

## Project variables

Variables are simple string substitutions available in model SQL via the `@@name` syntax:

```toml
[vars]
schema_prefix = "analytics"
retention_days = "90"
```

Target-specific variables override project-level ones:

```toml
[vars]
schema_prefix = "analytics"

[targets.prod.vars]
schema_prefix = "prod_analytics"
```

See [Macros](macros.md) for details on variable substitution and how variables interact with macros.

## Janitor

Configuration for the `sqb janitor` command, which archives and then deletes stale warehouse relations:

```toml
[janitor]
enabled = false
retention_days = 14
archive_retention_days = 14
delete_tracked_only = true
exclude_patterns = ["audit_*", "tmp_*"]
in_use_window = "7d"
keep_in_use = false
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Whether janitor is active |
| `retention_days` | `14` | How many days a relation must be stale before it is archived |
| `archive_retention_days` | `14` | How many days an archive is kept before it is deleted. `0` deletes it in the same run. |
| `delete_tracked_only` | `true` | Only clean relations that appear in fingerprint tracking |
| `exclude_patterns` | `[]` | Glob patterns for relations to skip |
| `direct_state_history_versions` | `20` | How many state-history rows to keep per identity in `_sqlbuild_fingerprints` and `_sqlbuild_source_freshness`, and how many builds of each model and seed to keep in `_sqlbuild_consumption` |
| `in_use_window` | `"7d"` | How far back the janitor looks for reads of each relation it would archive. A fixed duration such as `"7d"` or `"36h"`. See [Relations in use](../cli/janitor.md#relations-in-use) |
| `keep_in_use` | `false` | Keep relations read within `in_use_window` instead of archiving them. `--keep-in-use` turns it on for one run. Stops with `C503` when usage can't be read in full |

See [janitor](../cli/janitor.md) for the archive and delete lifecycle.

## Migrations

Defaults for [model migrations](models/migrations.md):

```toml
[migrations]
old_name_views = "30d"
```

| Field | Default | Description |
|-------|---------|-------------|
| `old_name_views` | `"30d"` | How long a renamed model's old name keeps working through a [compatibility view](models/migrations.md#old-names) before janitor drops it. `false` keeps the old relation in place instead. A model's `old_name_view` header overrides it. |

## Scenario

Configuration for scenario run isolation, snapshot capture safety limits, and local type overrides:

```toml
[scenario]
run_namespace = "orders-job-123" # Optional

[scenario.snapshot_limits]
max_rows_per_relation = 10000
max_total_rows = 50000
max_bytes_per_relation = 10485760
max_total_bytes = 52428800
```

| Field | Default | Description |
|-------|---------|-------------|
| `max_rows_per_relation` | none | Maximum rows per captured relation |
| `scenario.run_namespace` | unset | Optional 1–128-character namespace using ASCII letters, digits, `-`, `_`, or `.`. CLI `--scenario-namespace` overrides `SQLBUILD_SCENARIO_NAMESPACE`, then local config, then project config. |
| `max_total_rows` | none | Maximum total rows across all relations in one scenario |
| `max_bytes_per_relation` | none | Maximum JSONL bytes per relation file |
| `max_total_bytes` | none | Maximum total JSONL bytes per scenario |

Local type overrides for DuckDB replay are configured per adapter dialect:

```toml
[scenario.local_type_overrides.snowflake]
"OBJECT" = "JSON"
"ARRAY" = "JSON"
```

See [Scenarios](scenarios.md) for details on local type overrides and capture limits.

## Git

The branch that `--since @base` compares against:

```toml
[git]
base_ref = "origin/main"
```

| Field | Default | Description |
|-------|---------|-------------|
| `base_ref` | unset | Git ref that [`sqb test --since @base`](../cli/test.md#run-only-affected-tests), `sqb scenario test --since @base` and [`sqb review`](../cli/review.md) read. Use a ref every checkout has, such as the remote default branch. |

## Review

Defaults for [`sqb review`](../cli/review.md):

```toml
[review]
against = "prod"
downstream = "critical"
schema_prefix = "review"
critical_tags = ["critical"]
```

| Field | Default | Description |
|-------|---------|-------------|
| `against` | unset | Target to compare with and to read unchanged upstream models from. `--against` overrides it; one of them is required |
| `downstream` | `"critical"` | Which unchanged downstream models to build and compare: `critical`, `all` or `none`. `--downstream` overrides it |
| `schema_prefix` | `"review"` | Prefix of the schema each review creates and drops; the schema name is lowercase |
| `critical_tags` | `["critical"]` | Model tags that make a model critical, in addition to `critical true` in its `MODEL` header |

## dbt

`[dbt]` points SQLBuild at an existing dbt project. See
[Using SQLBuild with dbt](dbt-compatibility/overview.md#configuration) for its fields.

## Skills

Configuration for AI agent skill file installation:

```toml
[skills]
targets = ["agents", "claude"]
auto_update = false
```

| Field | Default | Description |
|-------|---------|-------------|
| `targets` | `["agents", "claude"]` | Agent targets to install. OpenCode consumes `.agents`; use `opencode` only as an explicit override. |
| `auto_update` | `false` | Refresh stale SQLBuild-owned generated files from the installed package during normal commands; custom collisions are never overwritten. |

See [skills CLI reference](../cli/skills.md) for usage details.

## Project Python

Project-owned Python lives in a small set of roots. Anything that runs as part of the pipeline graph
goes in `python/`: loaders, tasks, assets, checks, factories, and the helper modules they import.
Modules under `python/` are normal Python: constants, classes, undecorated helper functions, and
helper-only modules such as `python/_helpers.py` are allowed, while decorators determine which
functions become graph nodes. See [Python nodes](python-nodes/overview.md#the-python-folder).

Code that extends SQLBuild itself keeps its own root:

| Location | Holds |
|----------|-------|
| `rules/` | Custom project rules |
| `materializations/` | Custom materializations |
| `adapters/`, `adapter.py` | Custom adapters |
| `providers/` | Runtime providers |
| `sinks/` | Lifecycle event and command output sinks |
| `hooks/python/` | Python model lifecycle hooks |
| `functions/python/` | Python UDFs |
| `macros/python/`, `enums/`, `constants/` and scoped `_macros/python/`, `_enums/`, `_constants/` | Python macros and declarations |
| `orchestration/` | Jobs and job groups as TOML in `jobs/` and `job_groups/`; calendars and sensors as Python in `calendars/` and `sensors/`. See [Orchestration](orchestration.md) |

Compilation rejects Python anywhere else with `D016`, so indirectly importable modules cannot create
an unofficial project structure.

Keep repository pytest tests outside the SQLBuild project's `tests/` directory, which is reserved for
SQLBuild SQL tests and scenarios. Orchestrator code such as Dagster or Rivers definitions lives outside
the SQLBuild project directory and points at it; see [Dagster](../integrations/dagster.md) and
[Rivers](../integrations/rivers.md).

## sqlbuild_local.toml

Local developer overrides. This optional file is loaded automatically and should be
gitignored. Only put values that differ from the shared project configuration here.

```toml
target = "dev"

[targets.dev]
schema = "dev_alice"
loader_schema = "raw_alice"

[connections.local]
database = "my_local.duckdb"

[settings]
sql_analysis = false
concurrency = 4

[vars]
debug_mode = "true"
```

| Field | Description |
|-------|-------------|
| `target` | Override which target is active for this developer |
| `adapter` | Override the database adapter (e.g. use DuckDB locally while prod uses Snowflake) |
| `connections` | Override named connection fields; entries merge by connection name |
| `settings` | Override global settings (only explicitly set fields take effect) |
| `vars` | Developer-specific variable overrides (merged on top of project + target vars) |

Project and local configuration merge named connections by name and merge their explicitly
configured fields. Target blocks merge the same way and may override the connection reference,
`database`, `schema`, `loader_schema`, variables, source deferral, and policy fields. Unspecified
values continue to come from `sqlbuild_project.toml`; a local reference to an unknown merged
connection still fails offline during configuration loading.
