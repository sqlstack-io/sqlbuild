"""Column names match by the compiler's key: case-folded and without identifier quotes."""

import pytest

from sqlbuild.adapter.contract.main.column_name_key import column_name_key
from tests.unit.src.sqlbuild.adapter.contract.main._test_types import ColumnNameKeyTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnNameKeyTestCase(
            description="a quoted camelCase listing", name='"listingId"', expected_key="listingid"
        ),
        ColumnNameKeyTestCase(
            description="an unquoted declaration", name="listingId", expected_key="listingid"
        ),
        ColumnNameKeyTestCase(
            description="an upper-case listing", name="ORDER_ID", expected_key="order_id"
        ),
        ColumnNameKeyTestCase(
            description="a name holding a quote is not unwrapped",
            name='"a""b"',
            expected_key='"a""b"',
        ),
        ColumnNameKeyTestCase(description="empty quotes are kept", name='""', expected_key='""'),
    ],
    ids=lambda case: case.description,
)
def test_given_column_name_when_keying_then_matches_compiler_key(
    test_case: ColumnNameKeyTestCase,
) -> None:
    assert column_name_key(test_case.name) == test_case.expected_key


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
