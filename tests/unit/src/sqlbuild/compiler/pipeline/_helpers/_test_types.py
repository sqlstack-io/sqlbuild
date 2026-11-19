"""Test case types for pipeline helper tests."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild._native import PythonIdentityStatus


@dataclass(frozen=True)
class DeferredTargetTestCase:
    description: str
    render_qualified_name: Callable[..., str | None]
    logical_schema: str | None
    logical_database: str | None
    env_schema: str | None
    env_database: str | None
    effective_vars: dict[str, object]
    default_schema: str | None
    default_database: str | None
    expected_schema: str | None
    expected_database: str | None
    expected_qualified_name: str | None


@dataclass(frozen=True)
class PythonPlanIdentityStatusTestCase:
    description: str
    previous_version_hash: str | None
    expected_status: PythonIdentityStatus


@dataclass(frozen=True)
class CloneTargetSchemaTestCase:
    description: str
    origin_schema: str | None
    destination_schema: str | None
    expected_target_name: str


@dataclass(frozen=True)
class CloneConnectionInjectionTestCase:
    description: str
    expected_connection: dict[str, object]


@dataclass(frozen=True)
class DeferredListingTestCase:
    """Deferred locations across databases, and what the deferred target is asked to list."""

    description: str
    locations: tuple[tuple[str | None, str, str], ...]
    expected_listings: list[tuple[str | None, tuple[str, ...]]]
    expected_names: list[str]
