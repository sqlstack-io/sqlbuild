"""Test cases for carrier money formatting."""

from dataclasses import dataclass


@dataclass(frozen=True)
class CentsTextTestCase:
    """One amount in cents and its carrier text."""

    description: str
    cents: int
    expected_text: str
