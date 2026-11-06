from __future__ import annotations

import pytest

from tests.unit.src.sqlbuild.executor.run._helpers._test_types import QuotedSpellingTestCase
from tests.unit.src.sqlbuild.executor.run._helpers.helpers import quoted_spelling_rejection


@pytest.mark.parametrize(
    "test_case",
    [
        QuotedSpellingTestCase(
            description="a quoted name matches its quoted column",
            configured='"orderId"',
            listed_names=('"orderId"', "amount"),
        ),
        QuotedSpellingTestCase(
            description="an unquoted name matches a case-folded column",
            configured="ORDER_ID",
            listed_names=("order_id",),
        ),
        QuotedSpellingTestCase(
            description="an unquoted name matches a case-folded column beside a quoted one",
            configured="orderId",
            listed_names=("orderid", '"orderId"'),
        ),
        QuotedSpellingTestCase(
            description="a name no column matches is left to the other checks",
            configured="missing",
            listed_names=('"orderId"',),
        ),
        QuotedSpellingTestCase(
            description="an unquoted name for a case-sensitive column is rejected",
            configured="orderId",
            listed_names=('"orderId"',),
            expected_error=(
                "model 'orders' unique_key 'orderId' names the case-sensitive column "
                '"orderId"; write it with its quotes, as \'"orderId"\', so the generated SQL '
                "keeps its case"
            ),
        ),
        QuotedSpellingTestCase(
            description="an unquoted lower-case name for a quoted lower-case column is rejected",
            configured="orderid",
            listed_names=('"orderid"',),
            expected_error=(
                "model 'orders' unique_key 'orderid' names the case-sensitive column "
                '"orderid"; write it with its quotes, as \'"orderid"\', so the generated SQL '
                "keeps its case"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_configured_column_when_checking_quoted_spelling_then_rejects_only_case_sensitive(
    test_case: QuotedSpellingTestCase,
) -> None:
    rejection: str | None = quoted_spelling_rejection(
        configured=test_case.configured, listed_names=test_case.listed_names
    )

    assert rejection == test_case.expected_error


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
