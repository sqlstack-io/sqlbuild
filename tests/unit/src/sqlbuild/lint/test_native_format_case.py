"""Native formatting and fingerprints fold only reserved keywords and built-in calls."""

from __future__ import annotations

import json

import pytest

import sqlbuild._native as native_module
from tests.unit.src.sqlbuild.lint._test_types import (
    KeywordNamedIdentifierFormatTestCase,
    ReservedKeywordFoldTestCase,
)
from tests.unit.src.sqlbuild.lint.helpers import keyword_name_positions, reserved_keyword_pairs


@pytest.mark.parametrize(
    "test_case",
    [
        KeywordNamedIdentifierFormatTestCase(
            description=f"{dialect} {name} as {position}",
            dialect=dialect,
            sql=template.format(name=name),
            name=name,
            expected_name_occurrences=1,
        )
        for dialect, name, position, template in keyword_name_positions()
    ],
    ids=lambda case: case.description,
)
def test_given_keyword_named_identifier_when_formatting_then_case_is_kept_and_fingerprinted(
    test_case: KeywordNamedIdentifierFormatTestCase,
) -> None:
    response: dict[str, object] = json.loads(
        native_module.format_sql_json(
            json.dumps({"version": 1, "sql": test_case.sql, "dialect": test_case.dialect})
        )
    )
    printed: str = str(response.get("sql") or "")
    swapped: str = test_case.sql.replace(test_case.name, test_case.name.swapcase())

    assert response["formatted"] is False or (
        printed.count(test_case.name) == test_case.expected_name_occurrences
    ), test_case.description
    assert native_module.query_fingerprint(
        test_case.sql, test_case.dialect
    ) != native_module.query_fingerprint(swapped, test_case.dialect), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        ReservedKeywordFoldTestCase(
            description=f"{dialect} {keyword}",
            dialect=dialect,
            keyword=keyword,
            expected_same_fingerprint=True,
        )
        for dialect, keyword in reserved_keyword_pairs()
    ],
    ids=lambda case: case.description,
)
def test_given_reserved_keyword_when_fingerprinting_then_its_case_is_folded(
    test_case: ReservedKeywordFoldTestCase,
) -> None:
    lower: str = f"select order_id {test_case.keyword.lower()} x from orders"
    upper: str = f"select order_id {test_case.keyword.upper()} x from orders"

    assert (
        native_module.query_fingerprint(lower, test_case.dialect)
        == native_module.query_fingerprint(upper, test_case.dialect)
    ) == test_case.expected_same_fingerprint, test_case.description
