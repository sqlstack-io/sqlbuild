from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import SchemaAuditInstance, parse_audit_instances
from tests.unit.src.sqlbuild.compiler.auditing.main._test_types import (
    ParseAuditInstancesErrorTestCase,
    ParseAuditInstancesTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstancesTestCase(
            description="parses string and mapping audits in order",
            raw_audits=["not_null", {"accepted_values": {"values": ["open", "closed"]}}],
            null_as_empty=False,
            expected_definition_names=("not_null", "accepted_values"),
        ),
        ParseAuditInstancesTestCase(
            description="treats null as empty when requested",
            raw_audits=None,
            null_as_empty=True,
            expected_definition_names=(),
        ),
        ParseAuditInstancesTestCase(
            description="accepts an empty list",
            raw_audits=[],
            null_as_empty=False,
            expected_definition_names=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_list_when_parsing_then_returns_audit_instances(
    test_case: ParseAuditInstancesTestCase,
) -> None:
    result: tuple[SchemaAuditInstance, ...] = parse_audit_instances(
        raw_audits=test_case.raw_audits,
        file_path=Path("sql/models/orders.yml"),
        label="model orders",
        error_class=ValueError,
        null_as_empty=test_case.null_as_empty,
    )

    assert tuple(audit.definition_name for audit in result) == (test_case.expected_definition_names)


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstancesErrorTestCase(
            description="rejects null when null is not empty",
            raw_audits=None,
            null_as_empty=False,
            expected_message="sql/models/orders.yml model orders audits must be a list",
        ),
        ParseAuditInstancesErrorTestCase(
            description="rejects a mapping",
            raw_audits={"not_null": {}},
            null_as_empty=True,
            expected_message="sql/models/orders.yml model orders audits must be a list",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_list_when_parsing_then_raises_injected_error(
    test_case: ParseAuditInstancesErrorTestCase,
) -> None:
    with pytest.raises(ValueError) as error:
        parse_audit_instances(
            raw_audits=test_case.raw_audits,
            file_path=Path("sql/models/orders.yml"),
            label="model orders",
            error_class=ValueError,
            null_as_empty=test_case.null_as_empty,
        )

    assert str(error.value) == test_case.expected_message
