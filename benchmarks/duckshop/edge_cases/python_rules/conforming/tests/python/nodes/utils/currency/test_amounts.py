"""Tests for carrier money formatting."""

import pytest
from python.nodes.utils.currency import amounts
from tests.python.utils.currency._test_types import CentsTextTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        CentsTextTestCase(
            description="whole amounts keep two decimals", cents=1200, expected_text="12.00"
        ),
        CentsTextTestCase(description="single cents are padded", cents=1205, expected_text="12.05"),
    ],
    ids=lambda case: case.description,
)
def test_given_cents_when_formatting_then_returns_carrier_text(
    test_case: CentsTextTestCase,
) -> None:
    """Format cents as carrier text."""
    assert amounts.cents_text(test_case.cents) == test_case.expected_text
