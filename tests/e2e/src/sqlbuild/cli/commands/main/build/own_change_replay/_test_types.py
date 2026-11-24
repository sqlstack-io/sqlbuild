"""Test case types for own-change replay e2e tests."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass(frozen=True)
class OwnChangePlanBuildTestCase:
    description: str
    expected_plan_fragments: tuple[str, ...]
    unexpected_plan_fragments: tuple[str, ...]
    expected_reasons: dict[str, str]
    expected_rows: dict[str, list[tuple[Any, ...]]]


@dataclass(frozen=True)
class RenamePlanBuildTestCase:
    description: str
    extra_sql: str
    expected_plan_fragments: tuple[str, ...]
    unexpected_plan_fragments: tuple[str, ...]
    expected_query_changed: bool
    expected_rows: dict[str, list[tuple[Any, ...]]]
    expected_relation_types: dict[str, list[str]] = field(default_factory=dict)


@dataclass(frozen=True)
class FunctionCallerReplayTestCase:
    description: str
    caller_config: str
    expected_plan_fragments: tuple[str, ...]
    expected_caller_action: str
    expected_rows: dict[str, list[tuple[Any, ...]]]


@dataclass(frozen=True)
class RefusedPlanTestCase:
    description: str
    expected_fragments: tuple[str, ...]


@dataclass(frozen=True)
class FullRefreshIntrinsicTestCase:
    description: str
    expected_plan_fragments: tuple[str, ...]


@dataclass(frozen=True)
class UpstreamContractFailureTestCase:
    description: str
    expected_fragments: tuple[str, ...]
    unexpected_fragments: tuple[str, ...]
