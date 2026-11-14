"""Materialization mixin for adapter implementations."""

from __future__ import annotations

from abc import ABC, abstractmethod
from pathlib import Path
from typing import Any

from sqlbuild._native import SeedCsvSettings
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.models import ColumnInfo, FunctionDefinition
from sqlbuild.spec.contracts.constants import DEFAULT_SEED_CSV_SETTINGS


class MaterializationMixin(ABC):
    """Executes SQL materialization operations against the warehouse."""

    @abstractmethod
    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Create or replace a table from a SELECT statement."""
        ...

    @abstractmethod
    def create_view_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Create or replace a view from a SELECT statement."""
        ...

    @abstractmethod
    def create_function(
        self,
        *,
        connection: Any,
        definition: FunctionDefinition,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Create or replace a function."""
        ...

    @abstractmethod
    def drop(
        self,
        *,
        connection: Any,
        destination: str,
        if_exists: bool = True,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Drop a relation from the warehouse."""
        ...

    @abstractmethod
    def rename(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename a relation."""
        ...

    @abstractmethod
    def swap(
        self,
        *,
        connection: Any,
        left: str,
        right: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Swap two relations atomically where supported."""
        ...

    @abstractmethod
    def clone(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        hard_copy: bool = False,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Clone a relation using zero-copy where supported."""
        ...

    @abstractmethod
    def durable_clone(
        self,
        *,
        connection: Any,
        origin: str,
        destination: str,
        origin_is_transient: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Clone/copy a relation into a durable independent destination."""
        ...

    @abstractmethod
    def load_seed(
        self,
        *,
        connection: Any,
        destination: str,
        file_path: Path,
        columns: tuple[ColumnInfo, ...],
        csv_settings: SeedCsvSettings = DEFAULT_SEED_CSV_SETTINGS,
        replace: bool = True,
        infer_types: bool = False,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Load a seed CSV file into a warehouse table."""
        ...

    @abstractmethod
    def append(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> int | None:
        """Insert rows from a SELECT statement into an existing table."""
        ...

    @abstractmethod
    def delete_insert(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
    ) -> int | None:
        """Delete matching rows then insert from a SELECT statement."""
        ...

    @abstractmethod
    def delete_insert_cursor(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: StatementRecorder,
        cursor_type: str | None = None,
        unique_key: tuple[str, ...] = (),
    ) -> int | None:
        """Delete rows by cursor range then insert from a SELECT statement."""
        ...

    @abstractmethod
    def merge(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        unique_key: str | tuple[str, ...],
        statement_recorder: StatementRecorder,
        exclude_columns: tuple[str, ...] = (),
    ) -> int | None:
        """Upsert rows from a SELECT statement using MERGE or equivalent."""
        ...

    @abstractmethod
    def add_columns(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        """Add columns to an existing table."""
        ...

    @abstractmethod
    def drop_columns(
        self,
        *,
        connection: Any,
        destination: str,
        column_names: tuple[str, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        """Drop columns from an existing table."""
        ...

    @abstractmethod
    def rename_column(
        self,
        *,
        connection: Any,
        destination: str,
        old_name: str,
        new_name: str,
        statement_recorder: StatementRecorder,
    ) -> None:
        """Rename one column of an existing table in place, keeping its data."""
        ...

    @abstractmethod
    def column_rename_unavailable_reason(self, *, connection: Any, destination: str) -> str | None:
        """Return why this table cannot rename a column in place, or None when it can."""
        ...

    @abstractmethod
    def alter_column_types(
        self,
        *,
        connection: Any,
        destination: str,
        columns: tuple[ColumnInfo, ...],
        statement_recorder: StatementRecorder,
    ) -> None:
        """Alter column types on an existing table."""
        ...
