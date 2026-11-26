"""Test case types for source loader execution models."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass
from datetime import date, datetime
from typing import Any

from sqlbuild._native import SourceColumnEntry
from sqlbuild.adapter.contract.types import CursorKind
from sqlbuild.compiler.python_nodes.types import SkipMode
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


@dataclass(frozen=True)
class LoaderRowsSqlTestCase:
    """One loader row SQL rendering case."""

    description: str
    rows: tuple[dict[str, object], ...]
    expected_sql_fragments: tuple[str, ...]
    columns: tuple[SourceColumnEntry, ...] = ()


@dataclass(frozen=True)
class LoaderRowsExecutableSqlTestCase:
    """One executable loader row SQL rendering case."""

    description: str
    rows: tuple[dict[str, object], ...]
    expected_rows: tuple[tuple[object, ...], ...]
    expected_sql_fragments: tuple[str, ...]


@dataclass(frozen=True)
class LoaderRowsNormalizeTestCase:
    """One loader row normalization success case."""

    description: str
    value: Any
    expected_rows: tuple[dict[str, object], ...]


@dataclass(frozen=True)
class LoaderRowsNormalizeErrorTestCase:
    """One loader row normalization error case."""

    description: str
    value: Any
    expected_error_fragment: str


@dataclass(frozen=True)
class LoaderRowsBatchTestCase:
    """One loader row batching case."""

    description: str
    value: Any
    batch_size: int
    expected_batches: tuple[tuple[dict[str, object], ...], ...]


@dataclass(frozen=True)
class LoaderRowsSchemaTestCase:
    """One loader row schema tracking case."""

    description: str
    rows: tuple[dict[str, object], ...]
    columns: tuple[SourceColumnEntry, ...]
    column_names: tuple[str, ...]
    expected_column_names: tuple[str, ...]
    expected_added_columns: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class LoaderRowsErrorTestCase:
    """One loader row SQL rendering error case."""

    description: str
    rows: tuple[dict[str, object], ...]
    expected_error_fragment: str


@dataclass(frozen=True)
class LoaderCursorKindTestCase:
    """One native loader cursor kind inference case."""

    description: str
    value: int | bool | datetime | date | str
    expected_cursor_kind: CursorKind | None


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
class LoadDagWorkerFailureTestCase:
    """One load DAG worker failure handling case."""

    description: str
    source_name: str
    loader_name: str
    expected_status: ExecutionStatus
    expected_error_fragment: str


@dataclass(frozen=True)
class LoadDagStateSchedulingTestCase:
    """One loader DAG state scheduling behavior case."""

    description: str
    source_names: tuple[str, ...]
    upstream_names: dict[str, tuple[str, ...]]
    downstream_names: dict[str, tuple[str, ...]]
    completed_source_name: str
    expected_initial_ready: tuple[str, ...]
    expected_final_ready: tuple[str, ...]
    expected_callback_sources: tuple[str, ...]


@dataclass(frozen=True)
class LoadPipelineSkipFanInTestCase:
    """One loader skip fan-in behavior case."""

    description: str
    skip_mode: SkipMode
    expected_statuses: tuple[ExecutionStatus, ...]
    expected_skip_modes: tuple[str | None, ...]
    expected_skip_reasons: tuple[str | None, ...]


@dataclass(frozen=True)
class LoadStartOrderingTestCase:
    description: str
    expected_connection_count: int


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
    start_cursor_ts: datetime | None
    end_cursor_ts: datetime | None
    start_cursor_int: int | None
    end_cursor_int: int | None
    expected_target: str
    expected_current_cursor_value: object | None
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
