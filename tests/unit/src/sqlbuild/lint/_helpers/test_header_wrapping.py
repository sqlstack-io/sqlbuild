"""Unit tests for wrapping long MODEL header audit lists."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import LintConfig, format_native_headers
from tests.unit.src.sqlbuild.lint._helpers._test_types import HeaderAuditWrapTestCase

_BODY: str = "\nSELECT 1 AS status\n"


@pytest.mark.parametrize(
    "test_case",
    [
        HeaderAuditWrapTestCase(
            "audits then their arguments go one per line",
            50,
            'MODEL (\n  description "Test model.",\n  audits [expression_is_true (name "positive", expression "total > 0")],\n);\n',
            'MODEL (\n  description "Test model.",\n  audits [\n    expression_is_true (\n'
            '      name "positive",\n      expression "total > 0",\n    ),\n  ],\n);\n',
        ),
        HeaderAuditWrapTestCase(
            "quoted values are never split",
            30,
            'MODEL (\n  description "Test model.",\n  audits [expression_is_true (expression "a, b, c, d > 0")],\n);\n',
            'MODEL (\n  description "Test model.",\n  audits [\n    expression_is_true (\n'
            '      expression "a, b, c, d > 0",\n    ),\n  ],\n);\n',
        ),
        HeaderAuditWrapTestCase(
            "a line with a comment is left alone",
            30,
            'MODEL (\n  description "Test model.",\n  audits [not_null, unique, accepted_values (values [1, 2])], -- keep\n);\n',
            'MODEL (\n  description "Test model.",\n  audits [not_null, unique, accepted_values (values [1, 2])], -- keep\n);\n',
        ),
        HeaderAuditWrapTestCase(
            "lists outside audits are left alone",
            30,
            'MODEL (\n  description "Test model.",\n  tags [orders, customers, products, inventory],\n);\n',
            'MODEL (\n  description "Test model.",\n  tags [orders, customers, products, inventory],\n);\n',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_long_header_audits_when_formatting_then_lists_wrap_idempotently(
    test_case: HeaderAuditWrapTestCase,
) -> None:
    config: LintConfig = LintConfig(line_width=test_case.line_width)

    formatted, faults = format_native_headers(
        contents=test_case.header + _BODY, file_path=Path("sql/models/orders.sql"), config=config
    )
    again, _ = format_native_headers(
        contents=formatted, file_path=Path("sql/models/orders.sql"), config=config
    )

    assert faults == ()
    assert formatted == test_case.expected_header + _BODY
    assert again == formatted
