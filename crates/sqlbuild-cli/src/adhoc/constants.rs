//! Which commands and flags an ad-hoc run accepts; anything not listed is refused.

/// Verbs a person may plan; workers launch only those the control plane lists.
pub(crate) const ADHOC_VERBS: [&str; 5] = ["build", "test", "seed", "audit", "freshness"];

/// Parser destinations that carry selectors, resolved to exact node keys on the server.
pub(crate) const SELECTOR_DESTS: [&str; 3] = ["select", "exclude", "positional_select"];

/// Destinations an ad-hoc run may set. Rewriting history and choosing a warehouse are gated by the
/// control plane from the frozen settings (`run.rewrite_history`, `run.warehouse`), not here.
pub(crate) const ALLOWED_DESTS: [&str; 28] = [
    "max_microbatches",
    "changed",
    "run_tests",
    "run_audits",
    "include_python",
    "fail_fast",
    "concurrency",
    "test_timeout",
    "selection_diagnostics",
    "verbose",
    "case",
    "inspect",
    "state",
    "fail_on_error",
    "fail_on_stale",
    "full_refresh",
    "start_cursor",
    "end_cursor",
    "start_cursor_ts",
    "end_cursor_ts",
    "start_cursor_int",
    "end_cursor_int",
    "allow_snapshot_full_refresh",
    "allow_snapshot_schema_change",
    "allow_table_type_downgrade",
    "allow_retention_decrease",
    "warehouse",
    "load_sources",
];

/// Parser converters that take whole numbers; the table names them as Python does.
pub(crate) const INTEGER_CONVERTERS: [&str; 2] = ["int", POSITIVE_INTEGER_CONVERTER];

/// The converter that also refuses zero and negative numbers.
pub(crate) const POSITIVE_INTEGER_CONVERTER: &str = "_positive_integer";

/// Refused destinations: the flag people typed and why an ad-hoc run cannot honour it.
pub(crate) const REFUSED_DESTS: [(&str, &str, &str); 25] = [
    (
        "select_file",
        "--select-file",
        "it reads a file on the worker",
    ),
    (
        "reload",
        "--reload",
        "source loads are not part of ad-hoc runs yet",
    ),
    (
        "allow_missing_migration_origin",
        "--allow-missing-migration-origin",
        "it can lose data",
    ),
    ("vars", "--vars", "variables change the published SQL"),
    ("defer_to", "--defer-to", "it reads another target"),
    (
        "defer_sources_to",
        "--defer-sources-to",
        "it reads another target",
    ),
    (
        "defer_clone_from",
        "--defer-clone-from",
        "it clones from another target",
    ),
    (
        "no_sql_validation",
        "--no-sql-analysis",
        "workers always verify the published SQL",
    ),
    ("no_cache", "--no-cache", "workers manage their own cache"),
    (
        "target",
        "--target",
        "the run's target is the one you are viewing",
    ),
    (
        "project_dir",
        "--project-dir",
        "workers run the published project",
    ),
    (
        "dbt_project_dir",
        "--dbt-project-dir",
        "workers run the published project",
    ),
    (
        "dbt_profiles_dir",
        "--dbt-profiles-dir",
        "workers run the published project",
    ),
    (
        "dbt_target",
        "--dbt-target",
        "workers run the published project",
    ),
    (
        "dbt_target_path",
        "--dbt-target-path",
        "workers run the published project",
    ),
    ("json", "--json", "workers own the run's output"),
    (
        "json_output",
        "--json-output",
        "workers own the run's output",
    ),
    (
        "event_output",
        "--event-output",
        "workers own the run's output",
    ),
    ("manifest", "--manifest", "workers own the run's output"),
    ("no_color", "--no-color", "workers own the run's output"),
    ("debug", "--debug", "it prints executed SQL"),
    ("help", "--help", "help is not a run"),
    ("version", "--version", "it is not a run"),
    ("since", "--since", "it needs the worker's Git checkout"),
    ("explain", "--explain", "it needs the worker's Git checkout"),
];
