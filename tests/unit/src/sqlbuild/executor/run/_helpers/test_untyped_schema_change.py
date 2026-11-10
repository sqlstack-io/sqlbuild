from __future__ import annotations

import pytest

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from tests.unit.src.sqlbuild.executor.run._helpers._test_types import (
    UntypedSchemaChangeTestCase,
)
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import applied_untyped_schema_change


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
    assert applied_untyped_schema_change(test_case) == test_case.expected_statements


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
        applied_untyped_schema_change(test_case)
