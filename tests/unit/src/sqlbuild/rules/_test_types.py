"""Test case types for the public custom-rule SQL document."""

from dataclasses import dataclass


@dataclass(frozen=True)
class DeepTreeTestCase:
    """A deeply nested query whose syntax tree a rule reads and then frees."""

    description: str
    dialect: str
    terms: int
    expected_node_count: int
