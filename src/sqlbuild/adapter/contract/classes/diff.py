"""Diff mixin for adapter implementations."""

from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Any

from sqlbuild.adapter.contract.models import (
    CursorValue,
    RelationRowCountEstimate,
    RowDiffCoverage,
    RowDiffResult,
    RowDiffSampleRow,
    RowDiffSampling,
    RowDiffTolerances,
    SchemaDiffResult,
)


class DiffMixin(ABC):
    """Compares relation data across targets."""

    @abstractmethod
    def diff_schema(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
    ) -> SchemaDiffResult:
        """Compare the schema of two relations."""
        ...

    @abstractmethod
    def diff_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        excluded_columns: tuple[str, ...] = (),
        tolerances: RowDiffTolerances | None = None,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        """Compare row-level data between two relations."""
        ...

    @abstractmethod
    def diff_unkeyed_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        excluded_columns: tuple[str, ...] = (),
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffResult:
        """Compare exact full-row multiplicities without a unique key."""
        ...

    @abstractmethod
    def inspect_row_diff_coverage(
        self,
        *,
        connection: Any,
        relation: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
    ) -> RowDiffCoverage:
        """Return exact bounded row count and cursor extent for one relation."""
        ...

    @abstractmethod
    def estimate_relation_row_count(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
        metadata_only: bool = False,
    ) -> RelationRowCountEstimate:
        """Estimate counts, forbidding data scans when metadata_only is requested."""
        ...

    @abstractmethod
    def sample_unequal_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        excluded_columns: tuple[str, ...] = (),
        tolerances: RowDiffTolerances | None = None,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
        limit: int = 20,
    ) -> tuple[RowDiffSampleRow, ...]:
        """Return sampled unequal rows for verbose diff output."""
        ...

    @abstractmethod
    def sample_side_only_rows(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        unique_key: str | tuple[str, ...],
        side: str,
        cursor_column: str | None = None,
        start_cursor: CursorValue | None = None,
        end_cursor: CursorValue | None = None,
        limit: int = 20,
        sampling: RowDiffSampling | None = None,
    ) -> tuple[tuple[tuple[str, object], ...], ...]:
        """Return sampled side-only keys for verbose diff output."""
        ...
