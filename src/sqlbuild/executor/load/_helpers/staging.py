"""Staging-table write helpers for source loaders."""

from __future__ import annotations

from typing import Any

from sqlbuild._native import ContractPolicy, SourceEntry, fit_auxiliary_relation_name
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import LoaderLogicalType
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.executor.load._helpers.rows import (
    build_rows_sql,
    iter_loader_row_batches,
    loader_column_sql_types,
    update_loader_rows_schema,
)
from sqlbuild.executor.load.models import DecimalShape, LoaderRowsSchema
from sqlbuild.executor.run.constants import STAGING_RELATION_SUFFIX


def write_loader_rows_to_staging(
    *,
    loader_return_value: object,
    source_entry: SourceEntry,
    adapter: BaseAdapter,
    connection: Any,
    staging: str,
    statement_recorder: StatementRecorder,
) -> int:
    """Write framework-managed loader rows into one staging table."""

    default_load_batch_size: int = 10000
    batch_size: int = source_entry.load_batch_size or default_load_batch_size
    rows_loaded: int = 0
    staging_created: bool = False
    column_names: tuple[str, ...] = tuple(column.name for column in source_entry.columns)
    inferred_types: dict[str, LoaderLogicalType] = {}
    decimal_shapes: dict[str, DecimalShape] = {}
    adapter.drop(
        connection=connection,
        destination=staging,
        if_exists=True,
        statement_recorder=statement_recorder,
    )
    batch: tuple[dict[str, object], ...]
    for batch in iter_loader_row_batches(value=loader_return_value, batch_size=batch_size):
        schema: LoaderRowsSchema = update_loader_rows_schema(
            adapter=adapter,
            rows=batch,
            columns=source_entry.columns,
            column_names=column_names,
            inferred_types=inferred_types,
            decimal_shapes=decimal_shapes,
            contract_enforced=source_entry.contract == ContractPolicy.ENFORCED,
        )
        column_names = schema.column_names
        inferred_types = schema.inferred_types
        decimal_shapes = schema.decimal_shapes
        if staging_created and schema.added_columns:
            adapter.add_columns(
                connection=connection,
                destination=staging,
                columns=schema.added_columns,
                statement_recorder=statement_recorder,
            )
        sql: str = build_rows_sql(
            adapter=adapter,
            rows=batch,
            columns=source_entry.columns,
            column_names=column_names,
            inferred_types=inferred_types,
            decimal_shapes=decimal_shapes,
        )
        if staging_created:
            staging_adapter: BaseAdapter = adapter
            staging_adapter.append(
                connection=connection,
                destination=staging,
                sql=sql,
                columns=column_names,
                statement_recorder=statement_recorder,
            )
        else:
            adapter.create_table_as(
                connection=connection,
                destination=staging,
                sql=sql,
                config=_staging_config(
                    adapter=adapter,
                    source_entry=source_entry,
                    column_names=column_names,
                    inferred_types=inferred_types,
                    decimal_shapes=decimal_shapes,
                ),
                statement_recorder=statement_recorder,
            )
            staging_created = True
        rows_loaded += len(batch)
    if not staging_created:
        sql = build_rows_sql(
            adapter=adapter,
            rows=(),
            columns=source_entry.columns,
            column_names=column_names,
            inferred_types=inferred_types,
            decimal_shapes=decimal_shapes,
        )
        adapter.create_table_as(
            connection=connection,
            destination=staging,
            sql=sql,
            config=_staging_config(
                adapter=adapter,
                source_entry=source_entry,
                column_names=column_names,
                inferred_types=inferred_types,
                decimal_shapes=decimal_shapes,
            ),
            statement_recorder=statement_recorder,
        )
    return rows_loaded


def _staging_config(
    *,
    adapter: BaseAdapter,
    source_entry: SourceEntry,
    column_names: tuple[str, ...],
    inferred_types: dict[str, LoaderLogicalType],
    decimal_shapes: dict[str, DecimalShape],
) -> dict[str, Any]:
    """Hand loader column types to adapters whose CTAS cannot keep them."""

    return {
        "inferred_column_types": loader_column_sql_types(
            adapter=adapter,
            columns=source_entry.columns,
            column_names=column_names,
            inferred_types=inferred_types,
            decimal_shapes=decimal_shapes,
        )
    }


def resolve_loader_relations(
    *, adapter: BaseAdapter, source_entry: SourceEntry, destination_name: str
) -> tuple[str, str]:
    """Return the qualified loader destination and its fitted staging relation."""

    return (
        resolve_qualified_name_parts(
            adapter=adapter,
            database=source_entry.database,
            schema=source_entry.schema,
            name=destination_name,
        ),
        resolve_qualified_name_parts(
            adapter=adapter,
            database=source_entry.database,
            schema=source_entry.schema,
            name=fit_auxiliary_relation_name(
                base_name=destination_name,
                suffix=STAGING_RELATION_SUFFIX,
                identifier_limit=adapter.maximum_identifier_length(),
            ),
        ),
    )
