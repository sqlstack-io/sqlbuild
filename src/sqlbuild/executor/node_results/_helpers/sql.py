"""SQL helpers for runtime node result storage."""

from __future__ import annotations

from collections.abc import Callable

from sqlbuild._native import (
    NODE_RESULTS_TABLE_NAME,
    render_state_sql_literal,
    render_state_table_create_sql,
)
from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.node_results.constants import (
    COLUMN_ERROR_MESSAGE,
    COLUMN_MATERIALIZED,
    COLUMN_METADATA_JSON_B64,
    COLUMN_NODE_NAME,
    COLUMN_NODE_TYPE,
    COLUMN_PAYLOAD_JSON_B64,
    COLUMN_RUN_ID,
    COLUMN_STATUS,
    COLUMN_TARGET_DATABASE,
    COLUMN_TARGET_NAME,
    COLUMN_TARGET_SCHEMA,
    COLUMN_TIMESTAMP,
    NODE_RESULT_COLUMN_TYPES,
    NODE_RESULT_COLUMNS,
)
from sqlbuild.executor.node_results.models import NodeResultQuery, NodeResultRecord

_REQUIRED_NODE_RESULT_COLUMNS: frozenset[str] = frozenset(
    {
        COLUMN_NODE_TYPE,
        COLUMN_NODE_NAME,
        COLUMN_RUN_ID,
        COLUMN_STATUS,
        COLUMN_PAYLOAD_JSON_B64,
        COLUMN_METADATA_JSON_B64,
        COLUMN_TIMESTAMP,
    }
)


def build_qualified_table_name(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
) -> str:
    qualified_name: str | None = render_qualified_name(
        database=database,
        schema=schema,
        name=NODE_RESULTS_TABLE_NAME,
    )
    if qualified_name is None:
        raise ExecutorInputError("node result table requires a target schema")
    return qualified_name


def build_create_table_sql(
    *,
    database: str | None,
    schema: str,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> str:
    return render_state_table_create_sql(
        qualified_name=build_qualified_table_name(
            database=database, schema=schema, render_qualified_name=render_qualified_name
        ),
        columns=NODE_RESULT_COLUMNS,
        column_types=NODE_RESULT_COLUMN_TYPES,
        required_columns=_REQUIRED_NODE_RESULT_COLUMNS,
        render_framework_type=render_framework_type,
    )


def build_insert_sql(
    *,
    database: str | None,
    schema: str,
    record: NodeResultRecord,
    payload_json_b64: str,
    metadata_json_b64: str,
    render_qualified_name: Callable[..., str | None],
) -> str:
    qualified_name: str = build_qualified_table_name(
        database=database,
        schema=schema,
        render_qualified_name=render_qualified_name,
    )
    materialized: str | None = _materialized_storage(record.materialized)
    return (
        f"INSERT INTO {qualified_name} ("
        f"{COLUMN_NODE_TYPE}, "
        f"{COLUMN_NODE_NAME}, "
        f"{COLUMN_TARGET_DATABASE}, "
        f"{COLUMN_TARGET_SCHEMA}, "
        f"{COLUMN_TARGET_NAME}, "
        f"{COLUMN_RUN_ID}, "
        f"{COLUMN_STATUS}, "
        f"{COLUMN_PAYLOAD_JSON_B64}, "
        f"{COLUMN_METADATA_JSON_B64}, "
        f"{COLUMN_ERROR_MESSAGE}, "
        f"{COLUMN_MATERIALIZED}, "
        f"{COLUMN_TIMESTAMP}"
        f") VALUES ("
        f"{_column_literal(column=COLUMN_NODE_TYPE, value=record.node_type)}, "
        f"{_column_literal(column=COLUMN_NODE_NAME, value=record.node_name)}, "
        f"{_column_literal(column=COLUMN_TARGET_DATABASE, value=record.target_database)}, "
        f"{_column_literal(column=COLUMN_TARGET_SCHEMA, value=record.target_schema)}, "
        f"{_column_literal(column=COLUMN_TARGET_NAME, value=record.target_name)}, "
        f"{_column_literal(column=COLUMN_RUN_ID, value=record.run_id)}, "
        f"{_column_literal(column=COLUMN_STATUS, value=record.status)}, "
        f"{_column_literal(column=COLUMN_PAYLOAD_JSON_B64, value=payload_json_b64)}, "
        f"{_column_literal(column=COLUMN_METADATA_JSON_B64, value=metadata_json_b64)}, "
        f"{_column_literal(column=COLUMN_ERROR_MESSAGE, value=record.error_message)}, "
        f"{_column_literal(column=COLUMN_MATERIALIZED, value=materialized)}, "
        f"{_column_literal(column=COLUMN_TIMESTAMP, value=record.ts)}"
        f")"
    )


def build_read_history_sql(
    *,
    database: str | None,
    schema: str,
    query: NodeResultQuery,
    render_qualified_name: Callable[..., str | None],
) -> str:
    qualified_name: str = build_qualified_table_name(
        database=database,
        schema=schema,
        render_qualified_name=render_qualified_name,
    )
    predicates: list[str] = [
        f"{COLUMN_NODE_TYPE} = {_column_literal(column=COLUMN_NODE_TYPE, value=query.node_type)}",
        f"{COLUMN_NODE_NAME} = {_column_literal(column=COLUMN_NODE_NAME, value=query.node_name)}",
        _optional_equality(column=COLUMN_TARGET_DATABASE, value=query.target_database),
        _optional_equality(column=COLUMN_TARGET_SCHEMA, value=query.target_schema),
        _optional_equality(column=COLUMN_TARGET_NAME, value=query.target_name),
    ]
    if query.statuses is not None:
        status_literals: str = ", ".join(
            _column_literal(column=COLUMN_STATUS, value=status) for status in query.statuses
        )
        predicates.append(f"{COLUMN_STATUS} IN ({status_literals})")
    if query.run_id is not None:
        predicates.append(
            f"{COLUMN_RUN_ID} = {_column_literal(column=COLUMN_RUN_ID, value=query.run_id)}"
        )
    return (
        f"SELECT {_select_columns()} "
        f"FROM {qualified_name} "
        f"WHERE {' AND '.join(predicates)} "
        f"ORDER BY {COLUMN_TIMESTAMP} DESC, {COLUMN_RUN_ID} DESC"
    )


def _select_columns() -> str:
    return (
        f"{COLUMN_NODE_TYPE}, "
        f"{COLUMN_NODE_NAME}, "
        f"{COLUMN_TARGET_DATABASE}, "
        f"{COLUMN_TARGET_SCHEMA}, "
        f"{COLUMN_TARGET_NAME}, "
        f"{COLUMN_RUN_ID}, "
        f"{COLUMN_STATUS}, "
        f"{COLUMN_PAYLOAD_JSON_B64}, "
        f"{COLUMN_METADATA_JSON_B64}, "
        f"{COLUMN_ERROR_MESSAGE}, "
        f"{COLUMN_MATERIALIZED}, "
        f"{COLUMN_TIMESTAMP}"
    )


def _optional_equality(*, column: str, value: str | None) -> str:
    if value is None:
        return f"{column} IS NULL"
    return f"{column} = {_column_literal(column=column, value=value)}"


def _materialized_storage(value: bool | None) -> str | None:
    if value is None:
        return None
    return "true" if value else "false"


def _column_literal(*, column: str, value: object | None) -> str:
    return render_state_sql_literal(value=value, declared_type=NODE_RESULT_COLUMN_TYPES[column])
