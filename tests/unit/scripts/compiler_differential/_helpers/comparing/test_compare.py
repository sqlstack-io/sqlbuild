"""First-difference reports name the exact JSON pointer or line that diverged."""

from __future__ import annotations

import pytest

from scripts.compiler_differential._helpers.comparing.compare import first_json_difference
from scripts.compiler_differential.models import Divergence
from tests.unit.scripts.compiler_differential._helpers.comparing._test_types import (
    JsonDifferenceTestCase,
    PreviewTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        JsonDifferenceTestCase(
            description="identical",
            left={"orders": [1, 2]},
            right={"orders": [1, 2]},
            expected_location=None,
        ),
        JsonDifferenceTestCase(
            description="nested_value",
            left={"resources": {"models": [{"name": "orders", "sql": "SELECT 1"}]}},
            right={"resources": {"models": [{"name": "orders", "sql": "SELECT 2"}]}},
            expected_location="/resources/models/0/sql",
        ),
        JsonDifferenceTestCase(
            description="first_of_several",
            left={"a": 1, "b": [1, 2, 3]},
            right={"a": 1, "b": [1, 9, 8]},
            expected_location="/b/1",
        ),
        JsonDifferenceTestCase(
            description="extra_list_item",
            left={"diagnostics": [{"code": "P010"}]},
            right={"diagnostics": [{"code": "P010"}, {"code": "S024"}]},
            expected_location="/diagnostics/1",
        ),
        JsonDifferenceTestCase(
            description="missing_key",
            left={"summary": {"models": 2, "seeds": 1}},
            right={"summary": {"models": 2}},
            expected_location="/summary/seeds",
        ),
        JsonDifferenceTestCase(
            description="key_order",
            left={"name": "orders", "path": "sql/models/orders.sql"},
            right={"path": "sql/models/orders.sql", "name": "orders"},
            expected_location="/ (key order)",
        ),
        JsonDifferenceTestCase(
            description="integer_versus_float",
            left={"ratio": 1},
            right={"ratio": 1.0},
            expected_location="/ratio",
        ),
        JsonDifferenceTestCase(
            description="escaped_pointer_token",
            left={"sql/models/orders.sql": {"a~b": 1}},
            right={"sql/models/orders.sql": {"a~b": 2}},
            expected_location="/models~1orders.sql/a~0b",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_documents_when_comparing_then_first_difference_pointer_is_reported(
    test_case: JsonDifferenceTestCase,
) -> None:
    found: Divergence | None = first_json_difference(left=test_case.left, right=test_case.right)

    assert getattr(found, "location", None) == test_case.expected_location


@pytest.mark.parametrize(
    "test_case",
    [
        PreviewTestCase(
            description="long_sql",
            left={"sql": "SELECT " + "a, " * 200},
            right={"sql": "SELECT " + "b, " * 200},
            expected_max_length=160,
            expected_suffix="...",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_long_differing_values_when_reporting_then_previews_are_bounded(
    test_case: PreviewTestCase,
) -> None:
    found: Divergence | None = first_json_difference(left=test_case.left, right=test_case.right)

    assert found is not None
    assert len(found.left) <= test_case.expected_max_length
    assert found.left.endswith(test_case.expected_suffix)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
