"""Type enforcement for staged table materialization."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import fit_auxiliary_relation_name
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.executor.run._helpers.execution.schema import inspect_runtime_relation_schema
from sqlbuild.executor.run.constants import TYPE_ENFORCEMENT_RELATION_SUFFIX


def enforce_types_staged(
    *,
    adapter: BaseAdapter,
    connection: Any,
    staging_qualified: str,
    staging_database: str | None,
    staging_schema: str | None,
    staging_table: str,
    declared_columns: tuple[ColumnInfo, ...],
    table_type: str,
    statement_recorder: StatementRecorder,
    physical_storage: dict[str, object] | None = None,
) -> None:
    """Inspect staging columns and rebuild with casts for declared types."""

    produced_columns: tuple[ColumnInfo, ...] = inspect_runtime_relation_schema(
        adapter=adapter,
        connection=connection,
        database=staging_database,
        schema=staging_schema,
        name=staging_table,
    )
    declared_map: dict[str, str] = {column_name_key(col.name): col.type for col in declared_columns}

    needs_enforcement: bool = False
    col: ColumnInfo
    for col in declared_columns:
        produced_type: str | None = None
        produced_col: ColumnInfo
        for produced_col in produced_columns:
            if column_name_key(produced_col.name) == column_name_key(col.name):
                produced_type = produced_col.type
                break
        if produced_type is not None and not types_equal(
            left=produced_type,
            right=col.type,
            dialect=adapter.sql_analysis_dialect_name,
        ):
            needs_enforcement = True
            break

    if not needs_enforcement:
        return

    projection_sql: str = _build_type_enforcement_projection(
        adapter=adapter,
        produced_columns=produced_columns,
        declared_map=declared_map,
    )
    enforced_qualified: str = resolve_qualified_name_parts(
        adapter=adapter,
        database=staging_database,
        schema=staging_schema,
        name=type_enforcement_relation_name(adapter=adapter, staging_table=staging_table),
    )
    adapter.create_table_as(
        connection=connection,
        destination=enforced_qualified,
        sql=f"SELECT {projection_sql} FROM {staging_qualified}",
        config={
            "table_type": table_type,
            "column_types": declared_map,
            "physical_storage": physical_storage or {},
        },
        statement_recorder=statement_recorder,
    )
    adapter.drop_table(
        connection=connection, destination=staging_qualified, statement_recorder=statement_recorder
    )
    adapter.rename(
        connection=connection,
        origin=enforced_qualified,
        destination=staging_qualified,
        statement_recorder=statement_recorder,
    )


def _build_type_enforcement_projection(
    *,
    adapter: BaseAdapter,
    produced_columns: tuple[ColumnInfo, ...],
    declared_map: dict[str, str],
) -> str:
    """Render adapter-aware projections that preserve names while coercing selected columns."""

    projection_parts: list[str] = []
    produced_column: ColumnInfo
    for produced_column in produced_columns:
        declared_type: str | None = declared_map.get(column_name_key(produced_column.name))
        quoted_column: str = adapter.render_identifier(produced_column.name)
        if declared_type is None:
            projection_parts.append(quoted_column)
        else:
            projection_parts.append(
                adapter.render_source_expression_cast(
                    expression=quoted_column,
                    target_type=declared_type,
                    alias=quoted_column,
                )
            )
    return ", ".join(projection_parts)


def type_enforcement_relation_name(*, adapter: BaseAdapter, staging_table: str) -> str:
    """Return the fitted name of the relation type enforcement rebuilds staging into."""

    return fit_auxiliary_relation_name(
        base_name=staging_table,
        suffix=TYPE_ENFORCEMENT_RELATION_SUFFIX,
        identifier_limit=adapter.maximum_identifier_length(),
    )
