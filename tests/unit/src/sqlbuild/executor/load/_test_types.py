"""Test case types for source loader execution models."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass

from sqlbuild.cursor_algebra.types import CursorScalar
from sqlbuild.executor.scheduling.types import ExecutionStatus


@dataclass(frozen=True)
class LoaderContextHelperTestCase:
    """One loader context helper behavior case."""

    description: str
    raw_name: str
    database: str | None
    schema: str | None
    expected_qualified_name: str
    expected_target_schema_name: str
    expected_execute_result: object
    expected_query_result: object
    expected_recorded_events: tuple[str, ...]
    expected_logger_name: str


@dataclass(frozen=True, kw_only=True)
class LoaderCursorParseTestCase:
    description: str
    value: object
    expected_value: CursorScalar
    expected_rendered: str


@dataclass(frozen=True, kw_only=True)
class LoaderCursorParseErrorTestCase:
    description: str
    value: object
    expected_error_type: type[Exception]
    expected_error_fragment: str


@dataclass(frozen=True)
class SourceLoadExecutionContextTestCase:
    """One source load execution context behavior case."""

    description: str
    source_name: str
    loader_name: str
    target_table: str
    database: str | None
    schema: str | None
    run_id: str
    target: str | None
    vars: dict[str, object]
    is_reload: bool
    expected_target: str
    expected_status: ExecutionStatus
    expected_rows_loaded: int


@dataclass(frozen=True)
class SourceLoadNoneReturnTestCase:
    """One source load None-return behavior case."""

    description: str
    source_name: str
    loader_name: str
    loader_target: str | None
    expected_status: ExecutionStatus
    expected_rows_loaded: int
    expected_error_fragment: str = ""


@dataclass(frozen=True)
class LoaderOperationLifecycleTestCase:
    """One framework-owned loader iterable lifecycle case."""

    description: str
    expected_status: ExecutionStatus
    expected_event_types: tuple[str, ...]


@dataclass(frozen=True)
class LoaderReferenceRejectionTestCase:
    """A function passed to `ctx.loader()` that is not a loader, and the error it raises."""

    description: str
    reference: Callable[..., object]
    expected_message: str
