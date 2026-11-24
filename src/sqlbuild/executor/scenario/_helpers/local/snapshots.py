"""Load durable local scenario snapshots into DuckDB."""

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path
from typing import Any

from sqlbuild._native import ScenarioArtifactKind, error_code, error_message
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    is_scenario_snapshot_fresh,
    read_scenario_snapshot_jsonl,
    read_scenario_snapshot_manifest,
    scenario_snapshot_manifest_path,
    scenario_snapshot_root,
)
from sqlbuild.executor.scenario.constants import (
    JSON_FROM_TEXT_PLACEHOLDER,
    LOCAL_CAPTURE_ADAPTER,
    LOCAL_TYPE_JSON,
    LOCAL_TYPE_VARIANT,
    SCENARIO_LOCAL_JSONL_INVALID,
    SCENARIO_LOCAL_LOAD_FAILED,
    SCENARIO_LOCAL_MANIFEST_INVALID,
    SCENARIO_LOCAL_SNAPSHOT_MISSING,
    SCENARIO_LOCAL_SNAPSHOT_STALE,
    SCENARIO_LOCAL_TYPE_INVALID,
)
from sqlbuild.executor.scenario.models import (
    ScenarioLocalSnapshotLoadedRelation,
    ScenarioLocalSnapshotLoadResult,
    ScenarioLocalSnapshotTypeValidationIssue,
    ScenarioSnapshotColumn,
    ScenarioSnapshotManifest,
    ScenarioSnapshotRelation,
)


def load_scenario_snapshot_into_duckdb(
    *,
    project_dir: Path,
    scenario_name: str,
    current_input_fingerprint: str,
    connection: Any,
) -> ScenarioLocalSnapshotLoadResult:
    """Load one fresh local scenario snapshot into a DuckDB connection."""

    snapshot_root: Path = scenario_snapshot_root(
        project_dir=project_dir,
        scenario_name=scenario_name,
    )
    manifest_path: Path = scenario_snapshot_manifest_path(
        project_dir=project_dir,
        scenario_name=scenario_name,
    )
    if not manifest_path.exists():
        raise ExecutorInputError(
            f"Scenario '{scenario_name}' is missing local snapshot manifest "
            f"'{manifest_path.as_posix()}'.",
            code=SCENARIO_LOCAL_SNAPSHOT_MISSING,
            help=f"Run `sqb scenario capture {scenario_name}` to create the snapshot.",
        )

    try:
        manifest: ScenarioSnapshotManifest = read_scenario_snapshot_manifest(
            manifest_path=manifest_path,
        )
    except ValueError as exc:
        raise ExecutorInputError(
            f"Scenario '{scenario_name}' has invalid local snapshot manifest "
            f"'{manifest_path.as_posix()}': {error_message(exc)}",
            code=error_code(error=exc, fallback_code=SCENARIO_LOCAL_MANIFEST_INVALID),
            help="Fix scenario.json or regenerate it with `sqb scenario capture`.",
        ) from exc

    if manifest.scenario_name != scenario_name:
        raise ExecutorInputError(
            f"Scenario '{scenario_name}' snapshot manifest '{manifest_path.as_posix()}' "
            f"belongs to scenario '{manifest.scenario_name}'.",
            code=SCENARIO_LOCAL_MANIFEST_INVALID,
            help="Move the snapshot to the matching scenario folder or recapture this scenario.",
        )
    if not is_scenario_snapshot_fresh(
        manifest=manifest,
        current_input_fingerprint=current_input_fingerprint,
    ):
        raise ExecutorInputError(
            f"Scenario '{scenario_name}' local snapshot '{manifest_path.as_posix()}' is stale.",
            code=SCENARIO_LOCAL_SNAPSHOT_STALE,
            help=f"Run `sqb scenario capture {scenario_name}` to refresh the snapshot.",
        )

    _validate_snapshot_relation_files(
        scenario_name=scenario_name,
        snapshot_root=snapshot_root,
        manifest_path=manifest_path,
        relations=manifest.relations,
    )
    type_errors: tuple[ScenarioLocalSnapshotTypeValidationIssue, ...] = _collect_local_type_errors(
        connection=connection,
        scenario_name=scenario_name,
        manifest_path=manifest_path,
        relations=manifest.relations,
    )
    if type_errors:
        raise _build_local_type_validation_error(type_errors)

    loaded_relations: list[ScenarioLocalSnapshotLoadedRelation] = []
    relation: ScenarioSnapshotRelation
    for relation in manifest.relations:
        loaded_relations.append(
            _load_snapshot_relation(
                connection=connection,
                scenario_name=scenario_name,
                snapshot_root=snapshot_root,
                manifest_path=manifest_path,
                relation=relation,
                capture_adapter=manifest.capture_adapter,
            )
        )

    return ScenarioLocalSnapshotLoadResult(
        scenario_name=scenario_name,
        manifest=manifest,
        relations=tuple(loaded_relations),
    )


def local_snapshot_table_name(*, kind: ScenarioArtifactKind, logical_name: str) -> str:
    """Return the DuckDB table name used for one loaded local snapshot relation."""

    return f"__sqb_local__{kind.value}__{logical_name}"


def _validate_snapshot_relation_files(
    *,
    scenario_name: str,
    snapshot_root: Path,
    manifest_path: Path,
    relations: tuple[ScenarioSnapshotRelation, ...],
) -> None:
    relation: ScenarioSnapshotRelation
    for relation in relations:
        file_path: Path = snapshot_root / relation.file_path
        if not file_path.exists():
            raise ExecutorInputError(
                f"Scenario '{scenario_name}' is missing local snapshot JSONL file "
                f"'{file_path.as_posix()}' for relation "
                f"'{relation.kind.value} {relation.logical_name}'.",
                code=SCENARIO_LOCAL_SNAPSHOT_MISSING,
                help=f"Run `sqb scenario capture {scenario_name}` to recreate snapshot files.",
            )
        if not relation.columns:
            raise ExecutorInputError(
                f"Scenario '{scenario_name}' snapshot manifest '{manifest_path.as_posix()}' "
                f"relation '{relation.kind.value} {relation.logical_name}' has no column "
                "metadata.",
                code=SCENARIO_LOCAL_MANIFEST_INVALID,
                help="Regenerate the snapshot so scenario.json includes relation columns.",
            )


def _collect_local_type_errors(
    *,
    connection: Any,
    scenario_name: str,
    manifest_path: Path,
    relations: tuple[ScenarioSnapshotRelation, ...],
) -> tuple[ScenarioLocalSnapshotTypeValidationIssue, ...]:
    errors: list[ScenarioLocalSnapshotTypeValidationIssue] = []
    relation: ScenarioSnapshotRelation
    for relation in relations:
        column: ScenarioSnapshotColumn
        for column in relation.columns:
            error_message: str | None = _local_type_error_message(
                connection=connection,
                column=column,
            )
            if error_message is None:
                continue
            errors.append(
                ScenarioLocalSnapshotTypeValidationIssue(
                    scenario_name=scenario_name,
                    manifest_path=manifest_path,
                    kind=relation.kind,
                    logical_name=relation.logical_name,
                    column_name=column.name,
                    local_type=column.local_type,
                    error_message=error_message,
                )
            )
    return tuple(errors)


def _build_local_type_validation_error(
    type_errors: tuple[ScenarioLocalSnapshotTypeValidationIssue, ...],
) -> ExecutorInputError:
    first_error: ScenarioLocalSnapshotTypeValidationIssue = type_errors[0]
    details: str = "; ".join(
        "relation "
        f"'{error.kind.value} {error.logical_name}', column '{error.column_name}', "
        f"local_type '{error.local_type}' rejected by DuckDB: {error.error_message}"
        for error in type_errors
    )
    error_label: str = "error" if len(type_errors) == 1 else "errors"
    return ExecutorInputError(
        f"DuckDB rejected {len(type_errors)} local_type {error_label} for scenario "
        f"'{first_error.scenario_name}' in '{first_error.manifest_path.as_posix()}': "
        f"{details}",
        code=SCENARIO_LOCAL_TYPE_INVALID,
        help="Edit scenario.json or add scenario local type overrides and recapture.",
    )


def _load_snapshot_relation(
    *,
    connection: Any,
    scenario_name: str,
    snapshot_root: Path,
    manifest_path: Path,
    relation: ScenarioSnapshotRelation,
    capture_adapter: str,
) -> ScenarioLocalSnapshotLoadedRelation:
    del manifest_path
    file_path: Path = snapshot_root / relation.file_path
    table_name: str = local_snapshot_table_name(
        kind=relation.kind,
        logical_name=relation.logical_name,
    )
    connection.execute(f"DROP TABLE IF EXISTS {_quote_identifier(table_name)}")
    native_values: bool = capture_adapter == LOCAL_CAPTURE_ADAPTER
    connection.execute(
        _build_create_table_sql(
            table_name=table_name,
            columns=tuple(
                _storage_column(column=column, native_values=native_values)
                for column in relation.columns
            ),
        )
    )

    try:
        rows: tuple[dict[str, object], ...] = read_scenario_snapshot_jsonl(file_path=file_path)
    except ValueError as exc:
        raise ExecutorInputError(
            f"Scenario '{scenario_name}' has invalid local snapshot JSONL "
            f"'{file_path.as_posix()}' for relation "
            f"'{relation.kind.value} {relation.logical_name}': {error_message(exc)}",
            code=error_code(error=exc, fallback_code=SCENARIO_LOCAL_JSONL_INVALID),
            help="Fix the JSONL row data or regenerate the snapshot with `sqb scenario capture`.",
        ) from exc

    _insert_rows(
        connection=connection,
        scenario_name=scenario_name,
        file_path=file_path,
        relation=relation,
        table_name=table_name,
        rows=rows,
        native_values=native_values,
    )
    return ScenarioLocalSnapshotLoadedRelation(
        kind=relation.kind,
        logical_name=relation.logical_name,
        file_path=relation.file_path,
        table_name=table_name,
        row_count=len(rows),
    )


def _local_type_error_message(*, connection: Any, column: ScenarioSnapshotColumn) -> str | None:
    try:
        connection.execute(
            "CREATE TEMP TABLE __sqb_local_type_probe "
            f"({_quote_identifier(column.name)} {column.local_type})"
        )
        connection.execute("DROP TABLE __sqb_local_type_probe")
        return None
    except Exception as exc:
        try:
            connection.execute("DROP TABLE IF EXISTS __sqb_local_type_probe")
        except Exception:
            pass
        return str(exc)


def _insert_rows(
    *,
    connection: Any,
    scenario_name: str,
    file_path: Path,
    relation: ScenarioSnapshotRelation,
    table_name: str,
    rows: tuple[dict[str, object], ...],
    native_values: bool,
) -> None:
    if not rows:
        return
    column_names: tuple[str, ...] = tuple(column.name for column in relation.columns)
    variant_columns: tuple[bool, ...] = tuple(
        not native_values and column.local_type.strip().upper() == LOCAL_TYPE_VARIANT
        for column in relation.columns
    )
    insert_sql: str = _build_insert_sql(
        table_name=table_name, column_names=column_names, variant_columns=variant_columns
    )
    row_values: list[tuple[object, ...]] = []
    for row in rows:
        values: list[object] = []
        for column_name, variant in zip(column_names, variant_columns, strict=True):
            value: object = _row_value(row=row, column_name=column_name)
            values.append(_variant_json_text(value=value) if variant else value)
        row_values.append(tuple(values))
    try:
        connection.executemany(insert_sql, row_values)
    except Exception as exc:
        column_context: str = _load_failure_column_context(
            connection=connection,
            relation=relation,
            rows=rows,
        )
        raise ExecutorInputError(
            f"DuckDB could not load local snapshot JSONL '{file_path.as_posix()}' for "
            f"scenario '{scenario_name}', relation "
            f"'{relation.kind.value} {relation.logical_name}'{column_context}: {exc}",
            code=SCENARIO_LOCAL_LOAD_FAILED,
            help="Check scenario.json local_type values and the JSONL values for this relation.",
        ) from exc


def _row_value(*, row: dict[str, object], column_name: str) -> object:
    if column_name in row:
        return row[column_name]
    normalized_column_name: str = column_name.lower()
    row_key: str
    for row_key in row:
        if row_key.lower() == normalized_column_name:
            return row[row_key]
    return None


def _load_failure_column_context(
    *, connection: Any, relation: ScenarioSnapshotRelation, rows: tuple[dict[str, object], ...]
) -> str:
    row: dict[str, object]
    for row_index, row in enumerate(rows, start=1):
        column: ScenarioSnapshotColumn
        for column in relation.columns:
            try:
                connection.execute(
                    f"SELECT CAST(? AS {column.local_type})", [row.get(column.name)]
                ).fetchone()
            except Exception:
                return (
                    f", column '{column.name}', local_type '{column.local_type}', row {row_index}"
                )
    return ""


def _build_create_table_sql(*, table_name: str, columns: tuple[ScenarioSnapshotColumn, ...]) -> str:
    column_defs: str = ", ".join(
        f"{_quote_identifier(column.name)} {column.local_type}" for column in columns
    )
    return f"CREATE TABLE {_quote_identifier(table_name)} ({column_defs})"


def _storage_column(
    *, column: ScenarioSnapshotColumn, native_values: bool
) -> ScenarioSnapshotColumn:
    """The column as stored locally: SQLQuack reads a captured VARIANT as DuckDB JSON text."""

    if native_values or column.local_type.strip().upper() != LOCAL_TYPE_VARIANT:
        return column
    return replace(column, local_type=LOCAL_TYPE_JSON)


def _variant_json_text(*, value: object) -> str | None:
    """Another adapter's VARIANT value as JSON text; a string that parses is already JSON text."""

    if value is None:
        return None
    if isinstance(value, str):
        try:
            json.loads(value)
        except ValueError:
            return json.dumps(value)
        return value
    return json.dumps(value)


def _build_insert_sql(
    *, table_name: str, column_names: tuple[str, ...], variant_columns: tuple[bool, ...]
) -> str:
    columns: str = ", ".join(_quote_identifier(column_name) for column_name in column_names)
    placeholders: str = ", ".join(
        JSON_FROM_TEXT_PLACEHOLDER if variant else "?" for variant in variant_columns
    )
    return f"INSERT INTO {_quote_identifier(table_name)} ({columns}) VALUES ({placeholders})"


def _quote_identifier(identifier: str) -> str:
    return f'"{identifier.replace(chr(34), chr(34) + chr(34))}"'
