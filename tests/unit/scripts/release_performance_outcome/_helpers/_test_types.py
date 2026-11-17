"""Test-case models for classifying main comparison runs."""

from dataclasses import dataclass


@dataclass(frozen=True)
class UnfinishedComparisonTestCase:
    description: str
    result: str
    regressed: str
    annotations: tuple[tuple[str, str], ...]
    expected_output: str
