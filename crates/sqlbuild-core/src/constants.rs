pub const SQL_WILDCARD: &str = "*";
pub const TIMESTAMP_TYPE: &str = "TIMESTAMP";
pub const OPEN_PAREN: &str = "(";
pub const CLOSE_PAREN: &str = ")";
/// The error text every caught native panic carries across the boundary.
pub const PANIC_MESSAGE: &str = "NativeCompilerError: native SQL compilation panicked";
/// The prefix every native internal failure carries, so the boundary raises `NativeCompilerError`.
pub const NATIVE_FAILURE_PREFIX: &str = "NativeCompilerError: ";
/// File name of the project configuration at the project root.
pub const PROJECT_CONFIG_FILENAME: &str = "sqlbuild_project.toml";
/// Project configuration section holding settings.
pub const SETTINGS_SECTION: &str = "settings";
/// Settings key selecting how table builds are promoted.
pub const TABLE_PROMOTION_MODE_SETTING_KEY: &str = "table_promotion_mode";
/// Table promotion mode that builds into a staging relation before swapping.
pub const STAGED_PROMOTION_MODE: &str = "staged";
/// Project folder holding everything the warehouse runs.
pub const SQL_DIRECTORY: &str = "sql";
/// Project folder holding everything SQLBuild imports as Python, imported as the `python` package.
pub const PYTHON_DIRECTORY: &str = "python";
/// Project folder of SQL models.
pub const MODELS_PATH: &str = "sql/models";
/// Project folder of seeds.
pub const SEEDS_PATH: &str = "sql/seeds";
/// Project folder of sources.
pub const SOURCES_PATH: &str = "sql/sources";
/// Project folder of project-wide macros.
pub const MACROS_PATH: &str = "sql/macros";
/// Project folder of functions.
pub const FUNCTIONS_PATH: &str = "sql/functions";
/// Project folder of project-wide enums.
pub const ENUMS_PATH: &str = "sql/enums";
/// Project folder of project-wide constants.
pub const CONSTANTS_PATH: &str = "sql/constants";
/// Project folder of project-wide audits.
pub const AUDITS_PATH: &str = "sql/audits";
/// Project folder of project-wide hooks.
pub const HOOKS_PATH: &str = "sql/hooks";
/// Project folder of project-wide schemas.
pub const SCHEMAS_PATH: &str = "sql/schemas";
/// The folders `sql/` may hold.
pub const SQL_CHILDREN: [&str; 10] = [
    "models",
    "seeds",
    "sources",
    "macros",
    "functions",
    "enums",
    "constants",
    "audits",
    "hooks",
    "schemas",
];
/// Project folder holding Python node modules.
pub const PYTHON_NODE_ROOT: &str = "python/nodes";
/// Project folder of providers.
pub const PROVIDERS_PATH: &str = "python/providers";
/// Project folder of lifecycle event and command output event sinks.
pub const EVENT_SINKS_PATH: &str = "python/event_sinks";
/// Project folder of custom materializations.
pub const MATERIALIZATIONS_PATH: &str = "python/materializations";
/// Project folder of custom adapters.
pub const ADAPTERS_PATH: &str = "python/adapters";
/// Project file of a single custom adapter.
pub const ADAPTER_FILE_PATH: &str = "python/adapter.py";
/// The entries `python/` may hold.
pub const PYTHON_CHILDREN: [&str; 7] = [
    "nodes",
    "providers",
    "event_sinks",
    "materializations",
    "adapters",
    "adapter.py",
    "__init__.py",
];
/// Project folder of jobs, job groups, calendars and sensors.
pub const ORCHESTRATION_PATH: &str = "orchestration";
/// Project folder of custom rules.
pub const RULES_PATH: &str = "rules";
/// Project folder of SQL tests, scenarios and Python tests.
pub const TESTS_PATH: &str = "tests";
/// Project folder of SQL unit tests.
pub const UNIT_TESTS_PATH: &str = "tests/unit";
/// Project folder of SQL scenarios.
pub const SCENARIOS_PATH: &str = "tests/scenarios";
/// The folders a project's top level may hold.
pub const TOP_LEVEL_DIRECTORIES: [&str; 5] = ["sql", "python", "orchestration", "rules", "tests"];
/// The files a project's top level may hold.
pub const TOP_LEVEL_FILES: [&str; 3] =
    ["sqlbuild_project.toml", "sqlbuild_local.toml", "README.md"];
/// Stem of a Python package's initializer module.
pub const PYTHON_INIT_MODULE_STEM: &str = "__init__";
/// State table of model, seed and hook fingerprints.
pub const FINGERPRINT_TABLE_NAME: &str = "_sqlbuild_fingerprints";
/// State table of observed source freshness.
pub const SOURCE_FRESHNESS_TABLE_NAME: &str = "_sqlbuild_source_freshness";
/// State table of the inputs each model and seed execution consumed.
pub const CONSUMPTION_TABLE_NAME: &str = "_sqlbuild_consumption";
/// State table of model migration events.
pub const MIGRATION_TABLE_NAME: &str = "_sqlbuild_migrations";
/// State table of column migration events.
pub const COLUMN_MIGRATION_TABLE_NAME: &str = "_sqlbuild_column_migrations";
/// State table of old-name view events.
pub const OLD_NAME_VIEW_TABLE_NAME: &str = "_sqlbuild_old_name_views";
/// State table of each node's outcome in each run.
pub const NODE_RESULTS_TABLE_NAME: &str = "_sqlbuild_node_results";
/// State table of microbatch requirements and completions.
pub const MICROBATCH_TABLE_NAME: &str = "_sqlbuild_microbatches";
/// State table of audit outcomes.
pub const AUDIT_RESULTS_TABLE_NAME: &str = "_sqlbuild_audit_results";
/// State table of janitor archive and drop events.
pub const JANITOR_EVENTS_TABLE_NAME: &str = "_sqlbuild_janitor_events";
/// State table of display-only observations, which plan previews read and planning never does.
pub const OBSERVATIONS_TABLE_NAME: &str = "_sqlbuild_observations";
/// State table of estimated query cost per run, resource and warehouse; display only.
pub const RESOURCE_COSTS_TABLE_NAME: &str = "_sqlbuild_resource_costs";
/// State table of each build's start and end, which shows runs the control plane did not start.
pub const RUNS_TABLE_NAME: &str = "_sqlbuild_runs";
/// State table of the dynamic models a build paused, and its heartbeats while it holds them.
pub const DYNAMIC_PAUSES_TABLE_NAME: &str = "_sqlbuild_dynamic_pauses";
/// How long a location may spend resuming dynamic holds before it is killed; older claims are dead.
pub const DYNAMIC_RESUME_SECONDS: u64 = 120;
/// Every state table SQLBuild writes in a target's state schema.
pub const STATE_TABLE_NAMES: [&str; 14] = [
    FINGERPRINT_TABLE_NAME,
    MICROBATCH_TABLE_NAME,
    SOURCE_FRESHNESS_TABLE_NAME,
    CONSUMPTION_TABLE_NAME,
    NODE_RESULTS_TABLE_NAME,
    AUDIT_RESULTS_TABLE_NAME,
    JANITOR_EVENTS_TABLE_NAME,
    MIGRATION_TABLE_NAME,
    COLUMN_MIGRATION_TABLE_NAME,
    OLD_NAME_VIEW_TABLE_NAME,
    OBSERVATIONS_TABLE_NAME,
    RESOURCE_COSTS_TABLE_NAME,
    RUNS_TABLE_NAME,
    DYNAMIC_PAUSES_TABLE_NAME,
];
/// A zero-length cursor duration.
pub const ZERO_DAY_DURATION: &str = "0d";
/// Node key of a common table expression in the polyglot SQL AST.
pub const CTE_AST_KIND: &str = "cte";
/// How `sqb dev` hands its location the SQLite control store file it writes.
pub const DEVELOPMENT_CONTROL_FILE_ENV: &str = "SQLBUILD_DEVELOPMENT_CONTROL_FILE";
/// How `sqb dev` hands its location the folder it writes run output to.
pub const DEVELOPMENT_OUTPUT_FOLDER_ENV: &str = "SQLBUILD_DEVELOPMENT_OUTPUT_FOLDER";
