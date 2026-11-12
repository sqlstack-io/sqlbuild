"""Additive evolution of the audit result table's nullable meta column."""

from __future__ import annotations

import logging
from collections.abc import Callable
from typing import Any

from sqlbuild._native import AUDIT_RESULTS_TABLE_NAME
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import AdapterExecute, FrameworkType
from sqlbuild.executor.audit_results._helpers.sql import build_qualified_table_name
from sqlbuild.executor.audit_results.constants import (
    COLUMN_AUDIT_META_JSON,
)
from sqlbuild.executor.audit_results.models import AuditResultColumnEvolution

_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")


def ensure_meta_column(
    *,
    connection: Any,
    execute: AdapterExecute[Any, Any],
    database: str | None,
    schema: str,
    evolution: AuditResultColumnEvolution,
    render_qualified_name: Callable[..., str | None],
    render_framework_type: Callable[[FrameworkType], str],
) -> bool:
    """Add the meta column to a table created before it existed; report whether it exists."""

    try:
        present: bool = _meta_column_present(
            connection=connection, database=database, schema=schema, evolution=evolution
        )
    except Exception as error:
        _LOGGER.warning(
            "Audit result meta was not stored because the columns of the result table could "
            "not be read (%s)",
            error,
        )
        return False
    if present:
        return True
    statements: tuple[str, ...] = evolution.render_add_columns(
        destination=build_qualified_table_name(
            database=database,
            schema=schema,
            render_qualified_name=render_qualified_name,
        ),
        columns=(
            ColumnInfo(
                name=COLUMN_AUDIT_META_JSON,
                type=render_framework_type(FrameworkType.STRING),
            ),
        ),
    )
    try:
        for statement in statements:
            _ = execute(connection=connection, sql=statement)
    except Exception as error:
        if _meta_column_added_concurrently(
            connection=connection, database=database, schema=schema, evolution=evolution
        ):
            return True
        _LOGGER.warning(
            "Audit result meta was not stored because column '%s' could not be added (%s)",
            COLUMN_AUDIT_META_JSON,
            error,
        )
        return False
    return True


def _meta_column_added_concurrently(
    *,
    connection: Any,
    database: str | None,
    schema: str,
    evolution: AuditResultColumnEvolution,
) -> bool:
    try:
        return _meta_column_present(
            connection=connection, database=database, schema=schema, evolution=evolution
        )
    except Exception as error:
        _LOGGER.debug("Audit result columns could not be re-read after adding meta: %s", error)
        return False


def _meta_column_present(
    *,
    connection: Any,
    database: str | None,
    schema: str,
    evolution: AuditResultColumnEvolution,
) -> bool:
    columns: tuple[ColumnInfo, ...] = evolution.read_columns(
        connection=connection, database=database, schema=schema, name=AUDIT_RESULTS_TABLE_NAME
    )
    return any(column.name.lower() == COLUMN_AUDIT_META_JSON for column in columns)
