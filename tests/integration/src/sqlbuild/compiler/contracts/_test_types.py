"""Test case types for native contract validation and promotion conflicts."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class GeneratedContractTestCase:
    """Generated projects whose models are perturbed into every contract family per dialect."""

    description: str
    seed: int
    variants: int
    dialects: tuple[str | None, ...]
    expected_minimum_native: int
    expected_minimum_typed_comparisons: int
    expected_minimum_diagnostics: int
    expected_codes: frozenset[str]


@dataclass(frozen=True)
class FormerlyDeferredContractTestCase:
    """A contract input Python used to answer, and the native `(code, model, column, message)`s."""

    description: str
    declared_type: str
    dialect: str | None
    expected_diagnostics: tuple[tuple[str, str | None, str | None, str], ...]
