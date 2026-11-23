from pathlib import Path

import sqlbuild._native as _native
from sqlbuild._native import (
    DiscoveredCommandOutputSink,
    DiscoveredEventExporter,
    DiscoveredRuntimeExtensions,
    DiscoveredSqlModelFile,
    DiscoveredSqlScenarioFile,
    DiscoveredSqlTestBlock,
    SchemaModelEntry,
    SchemaSeedEntry,
    SourceEntry,
    SourceLocation,
)

REMOVED_SQL_MODEL_HEADER_KEYS: frozenset[str] = frozenset({"run_despite_unchanged"})
SQL_MODEL_HEADER_KEYS: frozenset[str] = frozenset(
    {
        "alias",
        "append_cursor_inclusive",
        "audit_factories",
        "audits",
        "batch_concurrency",
        "batch_size",
        "check_columns",
        "columns",
        "config",
        "constants",
        "contract",
        "cursor",
        "cursor_end",
        "cursor_future_action",
        "cursor_future_max_distance",
        "cursor_grain",
        "cursor_inputs",
        "cursor_start",
        "cursor_start_max_action",
        "cursor_start_max_ahead",
        "cursor_type",
        "cursor_watermark_mode",
        "database",
        "description",
        "dynamic_columns",
        "enabled",
        "enums",
        "full_refresh",
        "historical_input",
        "incremental_mode",
        "incremental_strategy",
        "initial_valid_from",
        "invalidate_hard_deletes",
        "lookback",
        "materialized",
        "max_microbatches",
        "merge_exclude_columns",
        "microbatch_limit",
        "microbatch_strategy",
        "migrate_force",
        "migrate_from",
        "model_schema",
        "observed_at",
        "old_name_view",
        "on_schema_change",
        "placeholders",
        "post_hooks",
        "pre_hooks",
        "replay_on_change",
        "row_diff_exclude_columns",
        "row_diff_sample_rows",
        "row_diff_sample_seed",
        "row_diff_tolerances",
        "schema",
        "snapshot_full_refresh",
        "snapshot_schema_change",
        "snapshot_strategy",
        "sql_analysis",
        "sql_validation",
        "table_type",
        "tags",
        "time_travel_retention",
        "unaccounted_partition_policy",
        "unique_key",
        "updated_at",
        "valid_from_column",
        "valid_to_column",
    }
)


def write_lifecycle_event_sink_project(*, project_dir: Path, exporter_config: str) -> None:
    (project_dir / "python" / "event_sinks").mkdir()
    (project_dir / "sqlbuild_project.toml").write_text(
        'name = "filters"\nadapter = "duckdb"\n' + exporter_config,
        encoding="utf-8",
    )
    (project_dir / "python" / "event_sinks" / "publish.py").write_text(
        "from sqlbuild.event_sinks import lifecycle_event_sink\n"
        '@lifecycle_event_sink(event_kinds={"run", "statement"}, min_severity="info")\n'
        "def publish(event):\n    del event\n",
        encoding="utf-8",
    )


def discover_all_model_files(*, project_dir: Path) -> tuple[DiscoveredSqlModelFile, ...]:
    """Discover every model file with the column extraction full compilation uses."""

    return _native.discover_model_files(
        project_dir=project_dir,
        extract_implicit_alias_columns=True,
        extract_output_column_locations=True,
    )


def parse_model_sql(*, contents: str, file_path: Path) -> tuple[dict[str, object], str]:
    """Parse model contents as model discovery does, returning header values and query SQL."""

    header_values, query_sql, _ = _parsed_model(contents=contents, file_path=file_path)
    return header_values, query_sql


def model_header_column_locations(
    *, contents: str, relative_path: Path
) -> dict[str, SourceLocation]:
    """Return the authored MODEL(columns) locations model discovery records."""

    return _parsed_model(contents=contents, file_path=relative_path)[2]


def _parsed_model(
    *, contents: str, file_path: Path
) -> tuple[dict[str, object], str, dict[str, SourceLocation]]:
    return _native.parse_model_contents(
        {
            "file_path": str(file_path),
            "supported_keys": sorted(SQL_MODEL_HEADER_KEYS),
            "removed_keys": sorted(REMOVED_SQL_MODEL_HEADER_KEYS),
            "extract_implicit_alias_columns": True,
            "extract_output_column_locations": False,
        },
        contents,
    )


def parse_sql_test_file(*, contents: str, file_path: Path) -> tuple[DiscoveredSqlTestBlock, ...]:
    """Parse SQL test contents as test discovery does."""

    return _native.parse_sql_test_file(contents=contents, file_path=file_path)


def parse_sql_scenario_file(
    *, contents: str, file_path: Path, relative_path: Path
) -> DiscoveredSqlScenarioFile:
    """Parse scenario contents as scenario discovery does."""

    return _native.parse_sql_scenario_file(
        contents=contents, file_path=file_path, relative_path=relative_path
    )


def parse_sources_yml(*, contents: str, file_path: Path) -> tuple[SourceEntry, ...]:
    """Load and parse sources YAML contents as source discovery does."""

    return _native.parse_sources_document(
        _native.load_yaml_document(str(file_path), contents, "source"),
        str(file_path),
    )


def parse_schema_yml(
    *, contents: str, file_path: Path
) -> tuple[tuple[SchemaModelEntry, ...], tuple[SchemaSeedEntry, ...]]:
    """Load and parse schema or seed YAML contents as schema discovery does."""

    return (), _native.parse_schema_document(
        _native.load_yaml_document(str(file_path), contents, "schema"),
        str(file_path),
    )


RECALL_SINK_IMPORTS: str = (
    "from sqlbuild.recall import RecallWriter\n"
    "from sqlbuild.event_sinks import (\n"
    "    CommandOutputRecord, LifecycleEvent, command_output_event_sink, lifecycle_event_sink,\n"
    ")\n\n"
)


def write_recall_sink_project(*, project_dir: Path, files: dict[str, str]) -> None:
    """Write a project named `orders` with `files`, each relative to the project directory."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    for relative, contents in files.items():
        path: Path = project_dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")


def recall_sink_names(
    extensions: DiscoveredRuntimeExtensions,
) -> tuple[tuple[tuple[tuple[str, str], ...], ...], tuple[tuple[str, ...], ...]]:
    """Each stream's replacement as `(name, writer_parameter)`, then the ordinary sinks' names."""

    lifecycle: tuple[tuple[DiscoveredEventExporter, str], ...] = tuple(
        filter(None, (extensions.recall_lifecycle_sink,))
    )
    output: tuple[tuple[DiscoveredCommandOutputSink, str], ...] = tuple(
        filter(None, (extensions.recall_output_sink,))
    )
    replacements: tuple[tuple[tuple[str, str], ...], ...] = (
        tuple((sink.name, writer) for sink, writer in lifecycle),
        tuple((sink.name, writer) for sink, writer in output),
    )
    ordinary: tuple[tuple[str, ...], ...] = (
        tuple(sink.name for sink in extensions.event_exporters),
        tuple(sink.name for sink in extensions.command_output_sinks),
    )
    return replacements, ordinary
