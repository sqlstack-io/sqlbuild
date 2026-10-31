"""SQL unit test executor domain types."""

from __future__ import annotations

from enum import StrEnum


class SqlTestOutcome(StrEnum):
    PASS = "pass"
    FAIL = "fail"
    ERROR = "error"
    TIMEOUT = "timeout"
    REFUSED = "refused"


class SqlTestDifferenceDirection(StrEnum):
    """Direction of one expected-output set difference."""

    UNEXPECTED = "unexpected"
    MISSING = "missing"
