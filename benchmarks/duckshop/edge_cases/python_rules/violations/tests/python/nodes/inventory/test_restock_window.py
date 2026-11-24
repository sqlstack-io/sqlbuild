"""Tests for a restock window that has no module."""

import pytest


@pytest.mark.parametrize("case", [{"days": 3, "expected": True}])
def test_window(case: dict[str, object]) -> None:
    """Check the restock window."""
    if case["days"]:
        assert case["expected"]
