from __future__ import annotations

from unittest.mock import Mock

import pytest

from sqlbuild._native import OnSchemaChange
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.run._helpers.materializations.incremental import _apply_schema_change
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    UntypedSchemaChangeTestCase,
)


def _applied_change(test_case: UntypedSchemaChangeTestCase) -> tuple[str, ...]:
    adapter: Mock = Mock()
    adapter.sql_analysis_dialect_name = "sqlite"
    adapter.untyped_column_type = test_case.untyped_column_type
    statements: tuple[str, ...] = _apply_schema_change(
        adapter=adapter,
        connection=object(),
        target_qualified="analytics.daily_revenue",
        target_columns=(ColumnInfo(name="order_day", type=test_case.target_type),),
        delta_columns=(ColumnInfo(name="order_day", type=test_case.delta_type),),
        on_schema_change=OnSchemaChange.APPEND_NEW_COLUMNS,
        statement_recorder=StatementRecorder(),
    )
    adapter.add_columns.assert_not_called()
    return statements


@pytest.mark.parametrize(
    "test_case",
    (
        UntypedSchemaChangeTestCase(
            description="an empty delta column with no type evidence is not a type change",
            untyped_column_type="ANY",
            target_type="TEXT",
            delta_type="ANY",
            expected_error=None,
        ),
        UntypedSchemaChangeTestCase(
            description="a target column with no type evidence is not a type change",
            untyped_column_type="ANY",
            target_type="ANY",
            delta_type="INTEGER",
            expected_error=None,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_column_without_type_evidence_when_appending_new_columns_then_nothing_changes(
    test_case: UntypedSchemaChangeTestCase,
) -> None:
    assert _applied_change(test_case) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        UntypedSchemaChangeTestCase(
            description="two stored types that differ are still a type change",
            untyped_column_type="ANY",
            target_type="TEXT",
            delta_type="INTEGER",
            expected_error="append_new_columns does not support type changes: order_day",
        ),
        UntypedSchemaChangeTestCase(
            description="an adapter without a no-evidence marker compares the types as written",
            untyped_column_type=None,
            target_type="TEXT",
            delta_type="ANY",
            expected_error="append_new_columns does not support type changes: order_day",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_a_real_type_change_when_appending_new_columns_then_it_fails(
    test_case: UntypedSchemaChangeTestCase,
) -> None:
    with pytest.raises(ExecutorInputError, match=test_case.expected_error):
        _applied_change(test_case)
