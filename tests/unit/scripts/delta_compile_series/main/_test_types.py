"""Comparison evidence controls."""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class ComparisonEvidenceTestCase:
    description: str
    stderr: str = "SQL test planning  START\npassword=synthetic-secret\n"
    stdout: str = '{"models": ["orders"]}\n'
    steps: int = 1
    expected_mismatches: int = 1
    cache_relative_path: str = "orders/check.sql"
