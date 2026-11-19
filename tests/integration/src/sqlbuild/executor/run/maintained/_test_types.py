from __future__ import annotations

from dataclasses import dataclass

from sqlbuild._native import PlanAction
from sqlbuild.adapter.contract.types import MaintainedListing, TablePromotionMode


@dataclass(frozen=True)
class MaintainedSuccessTestCase:
    description: str
    action: PlanAction
    setup_sql: tuple[str, ...]
    refresh_warehouse: str | None
    expected_created: tuple[tuple[str, str, str], ...]
    expected_altered: tuple[tuple[str, str | None], ...]
    expected_rows: tuple[tuple[int, ...], ...]


@dataclass(frozen=True)
class MaintainedFailureTestCase:
    description: str
    setup_sql: tuple[str, ...]
    session_warehouse: str | None
    listed_settings: MaintainedListing | None
    failing_query: str | None
    expected_error_code: str
    expected_listed_types: tuple[tuple[str], ...]


@dataclass(frozen=True)
class FailedRestoreTestCase:
    description: str
    setup_sql: tuple[str, ...]
    failing_query: str
    failing_renames: dict[str, str]
    expected_error_code: str
    expected_fragments: tuple[str, ...]
    expected_relations: tuple[tuple[str, str], ...]


@dataclass(frozen=True)
class ChangeTrackingTableTestCase:
    description: str
    maintained_readers: tuple[str, ...]
    promotion_mode: TablePromotionMode
    declared_columns: tuple[tuple[str, str], ...]
    expected_tracking: tuple[bool, ...]


@dataclass(frozen=True)
class ChangeTrackingSnapshotTestCase:
    description: str
    maintained_readers: tuple[str, ...]
    expected_tracked: tuple[str, ...]
