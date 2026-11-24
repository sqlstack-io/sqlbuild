"""Test case types for resolve helper tests."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import date, datetime

from sqlbuild._native import CursorBounds, CursorOverrides, ModelCursorSnapshot, SourceEntry
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo


@dataclass(frozen=True)
class CursorBoundsTestCase:
    description: str
    cursor_snapshot: ModelCursorSnapshot
    lookback: str | None
    backfill_duration: str | None
    start_cursor_override: str | None
    end_cursor_override: str | None
    is_microbatch: bool
    expected_bounds: CursorBounds | None
    cursor_type: str | None = None
    cursor_start: str | None = None
    cursor_grain: str | None = None


@dataclass(frozen=True)
class DiscoveredCursorPartitionTestCase:
    """Expected canonical partition for one observed physical cursor value."""

    description: str
    value: str | date | datetime
    cursor_grain: str | None
    expected_start: str | date | datetime
    expected_end: str | date | datetime


@dataclass(frozen=True, kw_only=True)
class IntegerWatermarkModeTestCase:
    """Integer watermark aggregation case with non-lexicographic values."""

    description: str
    mode: str
    minimums: tuple[str, ...]
    maximums: tuple[str, ...]
    expected_bounds: CursorBounds


@dataclass(frozen=True, kw_only=True)
class ReplayFloorRenderingTestCase:
    """Expected temporal representation after applying a configured cursor floor."""

    description: str
    current_start: str
    cursor_start: str | None
    expected_start: str


@dataclass(frozen=True)
class CursorIntrinsicResolutionTestCase:
    description: str
    model_config: dict[str, object]
    full_refresh: bool
    cursor_snapshots: dict[str, ModelCursorSnapshot]
    expected_sql: str | None = None
    expected_error_fragment: str | None = None


@dataclass(frozen=True)
class FullBuildIntrinsicTestCase:
    description: str
    adapter: Callable[[], BaseAdapter]
    model_config: dict[str, object]
    full_refresh: bool
    full_rebuild_cause: str | None
    expected_sql: str


@dataclass(frozen=True)
class SourceResolutionTestCase:
    description: str
    query_sql: str
    star_exclude_keyword: str
    source_map: dict[str, SourceEntry]
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]]
    expected_sql: str
    cursor_bounds: CursorBounds | None = None
    cursor_inputs: dict[str, str] = field(default_factory=dict)
    cursor_type: str | None = None
    lower_bound_inclusive: bool = True


@dataclass(frozen=True)
class SourceResolutionErrorTestCase:
    description: str
    query_sql: str
    star_exclude_keyword: str
    source_map: dict[str, SourceEntry]
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]]
    expected_error_fragment: str


@dataclass(frozen=True)
class AdapterSourceResolutionTestCase:
    description: str
    query_sql: str
    source_map: dict[str, SourceEntry]
    source_warehouse_columns: dict[str, tuple[ColumnInfo, ...]]
    expected_sql_fragment: str
    forbidden_sql_fragment: str


@dataclass(frozen=True)
class ReservedSourceColumnTestCase:
    description: str
    expected_sql: str


@dataclass(frozen=True)
class RefResolutionTestCase:
    description: str
    query_sql: str
    expected_sql: str
    cursor_type: str | None = None
    cursor_bounds: CursorBounds | None = None


@dataclass(frozen=True)
class ApplyDeferredTargetsTestCase:
    description: str
    model_target_qualified: dict[str, str | None]
    seed_target_qualified: dict[str, str | None]
    deferred_qualified: dict[str, str | None]
    selected_names: tuple[str, ...]
    expected_model_qualified: dict[str, str | None]
    expected_seed_qualified: dict[str, str | None] = field(default_factory=dict)


@dataclass(frozen=True)
class HookSqlResolutionTestCase:
    description: str
    pre_hooks: object
    post_hooks: object
    expected_pre_hooks: object
    expected_post_hooks: object


@dataclass(frozen=True)
class UntypedCursorOverrideTestCase:
    """Untyped --start-cursor/--end-cursor values and the selected models' cursor types."""

    description: str
    model_cursor_types: tuple[tuple[str, str | None], ...]
    overrides: CursorOverrides
    expected_overrides: CursorOverrides


@dataclass(frozen=True)
class UntypedCursorOverrideErrorTestCase:
    """Untyped cursor values that cannot be typed from the selection."""

    description: str
    model_cursor_types: tuple[tuple[str, str | None], ...]
    overrides: CursorOverrides
    expected_message_prefix: str
    expected_code: str
