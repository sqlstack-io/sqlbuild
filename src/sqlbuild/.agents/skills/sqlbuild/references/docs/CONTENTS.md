<!-- generated-by: sqlbuild skills -->

# SQLBuild documentation index

Bundled copies of every page on the SQLBuild documentation site, matching the installed SQLBuild version. Open only the page you need.

## Getting Started

- [Introduction](index.md) (`index`) - The refactorable warehouse. Verify early, test properly, and refactor safely.
- [Quickstart](quickstart.md) (`quickstart`) - Get a SQLBuild project running locally with DuckDB in under a minute.
- [Set up a project](project-setup.md) (`project-setup`) - Connect a SQLBuild project to your warehouse with dev and prod targets and a personal local file.
- [Using SQLBuild with dbt](concepts/dbt-compatibility/overview.md) (`concepts/dbt-compatibility/overview`) - Run dbt and SQLBuild side by side with coordinated selection and SQLBuild models downstream.
- [Selection](concepts/dbt-compatibility/selection.md) (`concepts/dbt-compatibility/selection`) - How select and exclude flags route work across the dbt and SQLBuild graphs.
- [Adding SQLBuild models](concepts/dbt-compatibility/adding-sqlbuild-models.md) (`concepts/dbt-compatibility/adding-sqlbuild-models`) - Grow into SQLBuild's own models, tests, audits, and scenarios downstream of your dbt project.
- [Feature Comparison](feature-comparison.md) (`feature-comparison`) - Feature comparison between SQLBuild, dbt, and SQLMesh.
- [Roadmap](roadmap.md) (`roadmap`) - What's coming next in SQLBuild.
- [Benchmarks](benchmarks.md) (`benchmarks`) - SQLBuild and dbt v2 compiling the same 10,000-model project: cold, unchanged, one-model and 1% edits.

## Build models

- [Overview](concepts/models.md) (`concepts/models`) - SQL model anatomy, references, dependencies, and the model documentation guide.
- [Materializations](concepts/models/materializations.md) (`concepts/models/materializations`) - Choose how SQLBuild persists model output.
- [Sources](concepts/sources.md) (`concepts/sources`) - Declare external data inputs for your pipeline.
- [Seeds](concepts/seeds.md) (`concepts/seeds`) - Load static CSV data into your pipeline as tables.
- [Incremental Models](concepts/incremental.md) (`concepts/incremental`) - Cursor-based incremental strategies, microbatch execution, and backfill policies.
- [Snapshots (SCD Type 2)](concepts/snapshots.md) (`concepts/snapshots`) - Preserve row history over time using SCD Type 2 semantics with timestamp or check-based change detection.
- [Functions](concepts/functions.md) (`concepts/functions`) - User-defined functions and table functions managed as part of your project.
- [Macros](concepts/macros.md) (`concepts/macros`) - Reuse SQL with typed SQL macros, or generate it with Python macros when it needs logic.
- [SQL Macros](concepts/macros/sql-macros.md) (`concepts/macros/sql-macros`) - Write typed, logic-free SQL fragments that the compiler checks and sees inside.
- [SQL macro reference](concepts/macros/sql-macro-reference.md) (`concepts/macros/sql-macro-reference`) - Literal positions, relation slot reads and mappings, call checks and result types for SQL macros.
- [Python Macros](concepts/macros/python-macros.md) (`concepts/macros/python-macros`) - Write Python functions that generate SQL at compile time when the SQL needs logic.
- [Python Macro Composition and Context](concepts/macros/composition-and-context.md) (`concepts/macros/composition-and-context`) - Compose Python macros through imports, and use adapter, target, constants and enums.
- [Selectors](concepts/selectors.md) (`concepts/selectors`) - Target specific models, paths, tags, or DAG subsets with select and exclude flags.

## Testing

- [Unit tests](concepts/testing.md) (`concepts/testing`) - Check a model's logic on rows you write, by mocking its inputs and comparing its output.
- [Scenarios](concepts/scenarios.md) (`concepts/scenarios`) - End-to-end tests that build real project graphs against coherent fixture data.
- [Audits](concepts/audits.md) (`concepts/audits`) - Violation and measurement checks that gate data and record quality outcomes.
- [Data Diffs](concepts/diff.md) (`concepts/diff`) - Compare schemas and data between targets to validate changes before they reach production.

## Correctness

- [What sqb compile checks](concepts/semantic-compilation.md) (`concepts/semantic-compilation`) - Offline reference, semantic and type checks that sqb compile runs before any warehouse work.
- [Contracts](concepts/models/contracts.md) (`concepts/models/contracts`) - Validate required or exact model output schemas.
- [Type Enforcement](concepts/models/type-enforcement.md) (`concepts/models/type-enforcement`) - Understand declared model types, static checks, and runtime casting.
- [Schemas](concepts/models/schemas.md) (`concepts/models/schemas`) - Declare model columns inline or reuse canonical inherited schemas.
- [Overview](concepts/rules.md) (`concepts/rules`) - Turn repeated SQL and project review decisions into compiler-enforced requirements.
- [Configuration and selection](concepts/rules/configuration-and-selection.md) (`concepts/rules/configuration-and-selection`) - Select built-in and custom Rules by exact code or family prefix.
- [Findings and exceptions](concepts/rules/findings-and-exceptions.md) (`concepts/rules/findings-and-exceptions`) - Understand Rule diagnostics and record intentional exceptions safely.
- [Execution and caching](concepts/rules/execution-and-caching.md) (`concepts/rules/execution-and-caching`) - Understand Rule enforcement order, focused runs, and dependency-aware reuse.

## Ship

- [Planning and change detection](concepts/planning.md) (`concepts/planning`) - How SQLBuild explains build work, detects changes, and chooses safe model actions.
- [Replay decisions](concepts/planning/replay-decisions.md) (`concepts/planning/replay-decisions`) - How much history each model reprocesses: decided by its own change and its own replay_on_change, never inherited from upstream.
- [Source freshness](concepts/planning/source-freshness.md) (`concepts/planning/source-freshness`) - Observe external source changes and propagate them through planning.
- [Selection and staleness](concepts/planning/selection-and-staleness.md) (`concepts/planning/selection-and-staleness`) - How selection interacts with change detection, and the stale warnings that stop silent partial rebuilds.
- [Column Lineage](concepts/column-lineage.md) (`concepts/column-lineage`) - Trace individual columns through your SQL pipeline - understand where data comes from and where it goes.

## Refactor

- [Refactoring](concepts/refactoring.md) (`concepts/refactoring`) - Rename and move models and columns, keep their warehouse history, and keep old names working.
- [Model migrations](concepts/models/migrations.md) (`concepts/models/migrations`) - Keep a renamed model's history, and keep its old name working for a while.
- [Column migrations](concepts/models/column-migrations.md) (`concepts/models/column-migrations`) - Keep a column's history when you rename it in an incremental or snapshot model.

## Investigate

- [Recall](concepts/observability/recall.md) (`concepts/observability/recall`) - The local history of every command: what ran, what it executed, and what it printed.
- [Sharing Recall in ClickHouse](concepts/observability/recall-clickhouse.md) (`concepts/observability/recall-clickhouse`) - Keep one Recall history for a team or CI in ClickHouse: users, grants, setup, retention and reading.
- [Observability](concepts/observability.md) (`concepts/observability`) - Find out what a run did, or send its events to your own systems.
- [Typed Sinks](concepts/observability/sinks.md) (`concepts/observability/sinks`) - Export lifecycle facts and command output through project-owned providers.

## Advanced

- [Overview](concepts/declaration-scopes.md) (`concepts/declaration-scopes`) - Limit enums, constants, macros, audits, schemas, and named hooks to the parts of a project that use them.
- [How Visibility Works](concepts/declaration-scopes/visibility.md) (`concepts/declaration-scopes/visibility`) - See which enums, constants, and macros are available to each SQL file.
- [Where to Put Declarations](concepts/declaration-scopes/placement.md) (`concepts/declaration-scopes/placement`) - Choose the narrowest folder that contains every real use.
- [Scope Explorer](concepts/declaration-scopes/explorer.md) (`concepts/declaration-scopes/explorer`) - Inspect visibility, explain resolution, browse declarations, and preview moves offline.
- [Enums](concepts/enums.md) (`concepts/enums`) - Define a fixed set of named string or integer values and use them safely in SQL.
- [Constants](concepts/constants.md) (`concepts/constants`) - Define reusable compiler-validated values and reference them safely from SQL.
- [Collections and Rendering](concepts/constants/collections-and-rendering.md) (`concepts/constants/collections-and-rendering`) - Define list, set, and object constants and control their adapter-specific SQL rendering.
- [Enum Model Contracts](concepts/enums/model-contracts.md) (`concepts/enums/model-contracts`) - Use an enum as a portable model-column domain with generated accepted-value validation.
- [Model-Private Values](concepts/model-private-values.md) (`concepts/model-private-values`) - Keep enums and constants inside one model when no other resource should use them.
- [Variables and placeholders](concepts/interpolation.md) (`concepts/interpolation`) - How SQLBuild processes variables, context, and dynamic content in SQL and config.
- [Hooks](concepts/models/hooks.md) (`concepts/models/hooks`) - Run SQL or Python lifecycle hooks around model materialization.
- [SQL Hooks](concepts/models/hooks/sql.md) (`concepts/models/hooks/sql`) - Define, parameterize, compile, and invoke reusable or inline SQL lifecycle hooks.
- [Python Hooks](concepts/models/hooks/python.md) (`concepts/models/hooks/python`) - Define Python lifecycle hooks with runtime context, providers, SQL access, and skips.
- [Overview](concepts/rules/custom-rules/overview.md) (`concepts/rules/custom-rules/overview`) - Define repository-owned checks with one typed Python API.
- [RuleContext and compiler facts](concepts/rules/custom-rules/rule-context.md) (`concepts/rules/custom-rules/rule-context`) - Inspect stable compiler-owned facts and opt into lazy SQL AST access.
- [Testing and deterministic Rules](concepts/rules/custom-rules/testing-and-determinism.md) (`concepts/rules/custom-rules/testing-and-determinism`) - Test custom findings and keep implementations safe for dependency-aware caching.
- [Published-layer Rules](concepts/rules/published-layer-rules.md) (`concepts/rules/published-layer-rules`) - Keep each domain's published layer free of consumer-specific variants.
- [Overview](concepts/python-nodes/overview.md) (`concepts/python-nodes/overview`) - Tasks, assets, loaders, and checks as first-class nodes in the SQLBuild graph.
- [Loaders](concepts/python-nodes/loaders.md) (`concepts/python-nodes/loaders`) - Load external data into source tables with Python functions.
- [Tasks](concepts/python-nodes/tasks.md) (`concepts/python-nodes/tasks`) - Run Python computation and side effects as nodes in the SQLBuild graph.
- [Assets](concepts/python-nodes/assets.md) (`concepts/python-nodes/assets`) - Produce or observe external artifacts as nodes in the SQLBuild graph.
- [Checks](concepts/python-nodes/checks.md) (`concepts/python-nodes/checks`) - Validate tasks, assets, loaders, and SQL relations with Python checks.
- [Factories](concepts/python-nodes/factories.md) (`concepts/python-nodes/factories`) - Generate Python nodes programmatically with @factory.
- [Providers](concepts/python-nodes/providers.md) (`concepts/python-nodes/providers`) - Shared runtime services for Python nodes and hooks.
- [SQL References](concepts/python-nodes/sql-references.md) (`concepts/python-nodes/sql-references`) - Read SQL models, sources, and seeds from Python nodes without creating SQL dependencies.
- [Custom adapters](concepts/adapters/custom.md) (`concepts/adapters/custom`) - Extend a built-in adapter or write one for another database engine in Python.
- [Execution history](concepts/observability/execution-history.md) (`concepts/observability/execution-history`) - Store and query lifecycle events and run history from Python, with SQLite locally or PostgreSQL when deployed.

## Adapters

- [Overview](concepts/adapters.md) (`concepts/adapters`) - Supported database engines and their connection configuration.
- [Snowflake](concepts/adapters/snowflake.md) (`concepts/adapters/snowflake`) - Snowflake adapter configuration for SQLBuild.
- [DuckDB](concepts/adapters/duckdb.md) (`concepts/adapters/duckdb`) - DuckDB adapter configuration for SQLBuild.
- [MotherDuck](concepts/adapters/motherduck.md) (`concepts/adapters/motherduck`) - MotherDuck adapter configuration for SQLBuild.
- [PostgreSQL](concepts/adapters/postgres.md) (`concepts/adapters/postgres`) - PostgreSQL adapter configuration for SQLBuild.
- [SQLite](concepts/adapters/sqlite.md) (`concepts/adapters/sqlite`) - SQLite adapter configuration, capabilities and limits for SQLBuild.
- [BigQuery](concepts/adapters/bigquery.md) (`concepts/adapters/bigquery`) - BigQuery adapter configuration for SQLBuild.
- [Databricks](concepts/adapters/databricks.md) (`concepts/adapters/databricks`) - Databricks adapter configuration for SQLBuild.
- [SQL Server](concepts/adapters/sqlserver.md) (`concepts/adapters/sqlserver`) - Microsoft SQL Server adapter configuration for SQLBuild.
- [ClickHouse](concepts/adapters/clickhouse.md) (`concepts/adapters/clickhouse`) - ClickHouse adapter configuration for SQLBuild.

## Integrations

- [Overview](integrations/dagster.md) (`integrations/dagster`) - Orchestrate SQLBuild pipelines with Dagster scheduling, retries, and asset UI.
- [API Reference](integrations/dagster-reference.md) (`integrations/dagster-reference`) - Dagster integration classes, decorators, and translator hooks.
- [Rivers](integrations/rivers.md) (`integrations/rivers`) - Orchestrate SQLBuild pipelines with Rivers scheduling, jobs, and asset tracking.
- [dlt](integrations/dlt.md) (`integrations/dlt`) - Declarative dlt sources in YAML, or full dlt pipelines inside Python source loaders.
- [ingestr](integrations/ingestr.md) (`integrations/ingestr`) - Declarative data ingestion from 50+ sources using ingestr.

## Reference

- [Project Configuration](concepts/project-configuration.md) (`concepts/project-configuration`) - Configure your SQLBuild project with sqlbuild_project.toml and sqlbuild_local.toml.
- [Configuration](concepts/models/configuration.md) (`concepts/models/configuration`) - MODEL() header fields and SQL-analysis controls.
- [Testing reference](concepts/testing-reference.md) (`concepts/testing-reference`) - Test modes, parameterized cases, cursor windows, cost guards and time limits for SQL unit tests.
- [Naming rules](concepts/resource-identities.md) (`concepts/resource-identities`) - Names of models, tests, macros and other SQLBuild resources are lowercase snake_case.

## CLI Reference

- [sqb init](cli/init.md) (`cli/init`) - Scaffold a new SQLBuild project.
- [sqb playground](cli/playground.md) (`cli/playground`) - Create a self-contained SQLBuild project to explore locally.
- [sqb skills](cli/skills.md) (`cli/skills`) - Install SQLBuild skill files for AI coding agents.
- [sqb debug](cli/debug.md) (`cli/debug`) - Validate project configuration and test the warehouse connection.
- [sqb compile](cli/compile.md) (`cli/compile`) - Compile models into resolved SQL, validate contracts, and write target artifacts - fully offline.
- [sqb format](cli/format.md) (`cli/format`) - Apply canonical SQLBuild SQL formatting.
- [sqb test](cli/test.md) (`cli/test`) - Run SQL unit tests and multi-model tests in isolation.
- [sqb scenario](cli/scenario.md) (`cli/scenario`) - Run end-to-end scenario tests against the warehouse or locally with DuckDB.
- [sqb lineage](cli/lineage.md) (`cli/lineage`) - Explore model and column-level dependency graphs from the command line.
- [sqb scope](cli/scope.md) (`cli/scope`) - Inspect declaration visibility, usage, placement, and move impact offline.
- [sqb rename and sqb mv](cli/rename.md) (`cli/rename`) - Rename or move a model, or rename a column, and update every reference in one verified step.
- [sqb rules](cli/rules.md) (`cli/rules`) - List, inspect, or run compiler-integrated Rules and generate project guidance.
- [sqb contract](cli/contract.md) (`cli/contract`) - Compare or generate repository contracts from physical warehouse schemas.
- [sqb plan](cli/plan.md) (`cli/plan`) - Preview what SQLBuild will do before executing.
- [sqb build](cli/build.md) (`cli/build`) - Compile, plan, and execute the selected build lifecycle.
- [sqb load](cli/load.md) (`cli/load`) - Load managed sources into the warehouse.
- [sqb seed](cli/seed.md) (`cli/seed`) - Load seed CSV files into the warehouse.
- [sqb audit](cli/audit.md) (`cli/audit`) - Run data quality audits in isolation.
- [sqb check](cli/check.md) (`cli/check`) - Run Python checks against tasks, assets, and loaders.
- [sqb freshness](cli/freshness.md) (`cli/freshness`) - Observe source freshness without writing state.
- [sqb diff](cli/diff.md) (`cli/diff`) - Compare schemas and data between targets.
- [sqb review](cli/review.md) (`cli/review`) - Build what a branch changed in a temporary schema, test it, compare it with a target and report.
- [sqb clone](cli/clone.md) (`cli/clone`) - Copy model relations between configured targets.
- [sqb janitor](cli/janitor.md) (`cli/janitor`) - Archive and then delete stale warehouse relations.
- [sqb recall](cli/recall.md) (`cli/recall`) - Query the recorded history of past invocations.
- [sqb query](cli/query.md) (`cli/query`) - Run ad hoc SQL queries against the project database.
- [sqb clean](cli/clean.md) (`cli/clean`) - Remove compiled artifacts from the target directory.
- [sqb dag](cli/dag.md) (`cli/dag`) - Generate the static DAG artifact for Dagster and other integrations.
- [sqb dbt](cli/dbt.md) (`cli/dbt`) - Coordinate dbt and SQLBuild projects.
- [sqb server](cli/server.md) (`cli/server`) - Run the control service and scheduler.
- [sqb login and logout](cli/login.md) (`cli/login`) - Authorize a bounded CLI session through your server's browser sign-in.
