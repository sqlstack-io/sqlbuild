"""Runtime validation for enforced model contracts."""

from __future__ import annotations

from sqlbuild._native import ModelPlanEntry, SchemaDynamicColumnFamily, matching_dynamic_families
from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import TypeDialect
from sqlbuild.adapter.type_system.main.types_equal import types_equal
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run.constants import (
    RUNTIME_CONTRACT_EXTRA_COLUMN_CODE,
    RUNTIME_CONTRACT_MISSING_COLUMN_CODE,
    RUNTIME_CONTRACT_MISSING_DECLARATIONS_CODE,
    RUNTIME_CONTRACT_TYPE_MISMATCH_CODE,
)


def validate_runtime_contract(
    *,
    entry: ModelPlanEntry,
    actual_columns: tuple[ColumnInfo, ...],
    dialect: TypeDialect | str | None = None,
) -> None:
    """Validate a staged relation's actual columns against an enforced contract."""

    if not entry.contract_enforced:
        return
    if not entry.contract_columns and not entry.contract_dynamic_columns:
        raise ExecutorInputError(
            f"model '{entry.name}' has contract enforced but declares no columns",
            code=RUNTIME_CONTRACT_MISSING_DECLARATIONS_CODE,
        )

    declared_by_name: dict[str, ColumnInfo] = {
        column_name_key(column.name): column for column in entry.contract_columns
    }
    actual_by_name: dict[str, ColumnInfo] = {
        column_name_key(column.name): column for column in actual_columns
    }

    missing_names: tuple[str, ...] = tuple(
        column.name
        for column in entry.contract_columns
        if column_name_key(column.name) not in actual_by_name
    )
    if missing_names:
        raise ExecutorInputError(
            f"model '{entry.name}' runtime contract missing columns: {', '.join(missing_names)}",
            code=RUNTIME_CONTRACT_MISSING_COLUMN_CODE,
        )

    extra_columns: tuple[ColumnInfo, ...] = tuple(
        column for column in actual_columns if column_name_key(column.name) not in declared_by_name
    )
    extra_names: tuple[str, ...] = tuple(
        column.name
        for column in extra_columns
        if len(
            matching_dynamic_families(
                families=entry.contract_dynamic_columns,
                column_name=column.name,
            )
        )
        != 1
    )
    if extra_names:
        raise ExecutorInputError(
            f"model '{entry.name}' runtime contract has extra columns: {', '.join(extra_names)}",
            code=RUNTIME_CONTRACT_EXTRA_COLUMN_CODE,
        )

    for column in extra_columns:
        matching: tuple[SchemaDynamicColumnFamily, ...] = matching_dynamic_families(
            families=entry.contract_dynamic_columns,
            column_name=column.name,
        )
        if len(matching) != 1:
            continue
        family: SchemaDynamicColumnFamily = matching[0]
        if not types_equal(left=family.type, right=column.type, dialect=dialect):
            raise ExecutorInputError(
                f"model '{entry.name}' runtime dynamic column '{column.name}' has type "
                f"{column.type} but family '{family.name}' declares {family.type}",
                code=RUNTIME_CONTRACT_TYPE_MISMATCH_CODE,
            )

    actual_column: ColumnInfo
    for actual_column in actual_columns:
        declared_column: ColumnInfo | None = declared_by_name.get(
            column_name_key(actual_column.name)
        )
        if declared_column is None:
            continue
        if not declared_column.type or not actual_column.type:
            continue
        if types_equal(left=actual_column.type, right=declared_column.type, dialect=dialect):
            continue
        raise ExecutorInputError(
            f"model '{entry.name}' runtime contract column '{declared_column.name}' "
            f"has type {actual_column.type} but contract declares {declared_column.type}",
            code=RUNTIME_CONTRACT_TYPE_MISMATCH_CODE,
        )
