from __future__ import annotations

from datetime import date
from pathlib import Path

import pytest

from sqlbuild._native import SchemaAuditInstance
from sqlbuild.compiler.auditing.types import AuditSeverity, ThresholdOperator
from tests.unit.src.sqlbuild.compiler.auditing.main._test_types import (
    MeasurementPolicyParsingTestCase,
    ParseAuditInstanceErrorTestCase,
    ParseAuditInstanceTestCase,
)
from tests.unit.src.sqlbuild.compiler.auditing.main.helpers import parse_audit_instance


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstanceTestCase(
            description="parses always_run as reserved audit option",
            raw_audit={
                "not_null": {
                    "always_run": True,
                    "severity": "error",
                    "column": "order_id",
                }
            },
            expected_definition_name="not_null",
            expected_always_run=True,
            expected_argument_keys=("column",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_mapping_when_parsing_then_always_run_is_reserved_option(
    test_case: ParseAuditInstanceTestCase,
) -> None:
    result: SchemaAuditInstance = parse_audit_instance(
        raw_audit=test_case.raw_audit,
        file_path=Path("sql/models/orders.sql"),
        label="model orders",
        error_class=ValueError,
    )

    assert result.definition_name == test_case.expected_definition_name
    assert result.always_run is test_case.expected_always_run
    assert result.severity is AuditSeverity.ERROR
    assert tuple(sorted(result.arguments)) == test_case.expected_argument_keys


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstanceErrorTestCase(
            description="rejects unknown severity",
            raw_audit={"not_null": {"severity": "critical"}},
            expected_error_fragment="'severity' must be one of: warn, error",
        ),
        ParseAuditInstanceErrorTestCase(
            description="rejects non-boolean always_run",
            raw_audit={"not_null": {"always_run": "true"}},
            expected_error_fragment="'always_run' must be a boolean",
        ),
        ParseAuditInstanceErrorTestCase(
            description="rejects noncanonical definition identity",
            raw_audit={"AcceptedValues": {"values": ["open"]}},
            expected_error_fragment=(
                "Invalid model orders audit definition identity 'AcceptedValues'"
            ),
        ),
        ParseAuditInstanceErrorTestCase(
            description="rejects noncanonical attachment identity",
            raw_audit={"accepted_values": {"name": "OrderStatus", "values": ["open"]}},
            expected_error_fragment=("Invalid model orders audit instance identity 'OrderStatus'"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_option_when_parsing_then_raises_clear_error(
    test_case: ParseAuditInstanceErrorTestCase,
) -> None:
    with pytest.raises(ValueError) as error_info:
        parse_audit_instance(
            raw_audit=test_case.raw_audit,
            file_path=Path("sql/models/orders.sql"),
            label="model orders",
            error_class=ValueError,
        )

    assert test_case.expected_error_fragment in str(error_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        MeasurementPolicyParsingTestCase(
            description="all threshold operators", expected_minimum_samples=0
        )
    ],
    ids=lambda case: case.description,
)
def test_given_all_directional_thresholds_when_parsing_then_typed_policies_are_returned(
    test_case: MeasurementPolicyParsingTestCase,
) -> None:
    below: SchemaAuditInstance = parse_audit_instance(
        raw_audit={
            "rate": {
                "thresholds": {"warn": {"below": 100}},
                "minimum_samples": 0,
                "evidence_limit": 7,
            }
        },
        file_path=Path("sql/models/orders.sql"),
        label="model orders",
        error_class=ValueError,
    )
    above: SchemaAuditInstance = parse_audit_instance(
        raw_audit={"rate": {"thresholds": {"error": {"above": 10.5}}}},
        file_path=Path("sql/models/orders.sql"),
        label="model orders",
        error_class=ValueError,
    )
    outside: SchemaAuditInstance = parse_audit_instance(
        raw_audit={"rate": {"thresholds": {"warn": {"outside": (90, 110)}}}},
        file_path=Path("sql/models/orders.sql"),
        label="model orders",
        error_class=ValueError,
    )

    assert below.thresholds is not None and below.thresholds.warn is not None
    assert below.thresholds.warn.operator == ThresholdOperator.BELOW
    assert below.thresholds.warn.limit == 100.0
    assert below.minimum_samples == test_case.expected_minimum_samples
    assert below.evidence_limit == 7
    assert "evidence_limit" not in below.arguments
    assert above.thresholds is not None and above.thresholds.error is not None
    assert above.thresholds.error.operator == ThresholdOperator.ABOVE
    assert outside.thresholds is not None and outside.thresholds.warn is not None
    assert outside.thresholds.warn.operator == ThresholdOperator.OUTSIDE
    assert (outside.thresholds.warn.lower, outside.thresholds.warn.upper) == (90.0, 110.0)


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstanceErrorTestCase(
            description="invalid measurement policy values",
            raw_audit={"rate": {"minimum_samples": -1}},
            expected_error_fragment="non-negative integer",
        ),
        ParseAuditInstanceErrorTestCase(
            description="invalid evidence limit",
            raw_audit={"rate": {"evidence_limit": True}},
            expected_error_fragment="'evidence_limit' must be a non-negative integer",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_measurement_policy_options_when_parsing_then_clear_errors_are_raised(
    test_case: ParseAuditInstanceErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        parse_audit_instance(
            raw_audit=test_case.raw_audit,
            file_path=Path("sql/models/orders.sql"),
            label="model orders",
            error_class=ValueError,
        )
    with pytest.raises(ValueError, match="outside threshold requires two numeric values"):
        parse_audit_instance(
            raw_audit={"rate": {"thresholds": {"warn": {"outside": (90,)}}}},
            file_path=Path("sql/models/orders.sql"),
            label="model orders",
            error_class=ValueError,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstanceTestCase(
            description="meta is a reserved option kept unchanged",
            raw_audit={
                "expression_is_true": {
                    "expression": "amount > 0",
                    "meta": {"region": "emea", "tiers": ["gold", 2], "weight": 1.5},
                }
            },
            expected_definition_name="expression_is_true",
            expected_always_run=False,
            expected_argument_keys=("expression",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_audit_meta_when_parsing_then_meta_is_not_an_argument(
    test_case: ParseAuditInstanceTestCase,
) -> None:
    result: SchemaAuditInstance = parse_audit_instance(
        raw_audit=test_case.raw_audit,
        file_path=Path("sql/models/orders.sql"),
        label="model orders",
        error_class=ValueError,
    )

    assert result.definition_name == test_case.expected_definition_name
    assert tuple(sorted(result.arguments)) == test_case.expected_argument_keys
    assert result.meta == {"region": "emea", "tiers": ["gold", 2], "weight": 1.5}


@pytest.mark.parametrize(
    "test_case",
    [
        ParseAuditInstanceErrorTestCase(
            description="meta must be a mapping",
            raw_audit={"not_null": {"meta": "emea"}},
            expected_error_fragment="audit 'not_null' 'meta' must be a mapping of JSON values",
        ),
        ParseAuditInstanceErrorTestCase(
            description="meta values must be JSON",
            raw_audit={"not_null": {"meta": {"since": date(2020, 1, 1)}}},
            expected_error_fragment=r"'meta' must map string keys to JSON values; meta\['since'\]",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_meta_when_parsing_then_compile_error_names_the_value(
    test_case: ParseAuditInstanceErrorTestCase,
) -> None:
    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        parse_audit_instance(
            raw_audit=test_case.raw_audit,
            file_path=Path("sql/models/orders.sql"),
            label="model orders",
            error_class=ValueError,
        )
