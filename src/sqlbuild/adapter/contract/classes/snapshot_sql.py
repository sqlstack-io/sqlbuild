"""Shared SQL for current-state snapshots and fragments common to every snapshot."""

from __future__ import annotations

from sqlbuild._native import InitialValidFrom, SnapshotStrategy
from sqlbuild.adapter.contract.models import SnapshotChangeTarget, SnapshotSqlDialect
from sqlbuild.adapter.contract.types import SnapshotUpdateStyle


class SnapshotSql:
    """Render current-state snapshot statements and shared snapshot fragments."""

    def __init__(self, *, dialect: SnapshotSqlDialect) -> None:
        self.dialect: SnapshotSqlDialect = dialect

    @staticmethod
    def key_condition(*, left_alias: str, right_alias: str, unique_key: tuple[str, ...]) -> str:
        """Render the equality join between two aliases over the snapshot unique key."""

        return " AND ".join(
            f"{left_alias}.{column} = {right_alias}.{column}" for column in unique_key
        )

    @staticmethod
    def initial_valid_from_expr(
        *,
        snapshot_strategy: str | None,
        updated_at_column: str | None,
        observed_at_column: str | None,
        initial_valid_from: str | None,
        source_alias: str | None,
        current_timestamp: str,
    ) -> str:
        """Render the valid_from expression for a key's first snapshot version."""

        prefix: str = f"{source_alias}." if source_alias is not None else ""
        if initial_valid_from == InitialValidFrom.EXECUTION_TIME:
            return current_timestamp
        if initial_valid_from == InitialValidFrom.OBSERVED_AT and observed_at_column is not None:
            return f"{prefix}{observed_at_column}"
        if initial_valid_from == InitialValidFrom.UPDATED_AT and updated_at_column is not None:
            return f"{prefix}{updated_at_column}"
        if snapshot_strategy == SnapshotStrategy.TIMESTAMP and updated_at_column is not None:
            return f"{prefix}{updated_at_column}"
        return current_timestamp

    def initial_select_sql(
        self,
        *,
        origin: str,
        snapshot_strategy: str | None,
        updated_at_column: str | None,
        observed_at_column: str | None,
        valid_from_column: str,
        valid_to_column: str,
        initial_valid_from: str | None,
        current_timestamp: str,
    ) -> str:
        """Render the initial-build query for a current-state snapshot."""

        typed_by_updated_at: bool = (
            snapshot_strategy == SnapshotStrategy.TIMESTAMP and updated_at_column is not None
        )
        valid_from_expr: str = self.initial_valid_from_expr(
            snapshot_strategy=snapshot_strategy,
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            initial_valid_from=initial_valid_from,
            source_alias=None,
            current_timestamp=current_timestamp
            if typed_by_updated_at
            else self.run_timestamp(current_timestamp),
        )
        valid_to_expr: str = f"CAST(NULL AS {self.dialect.timestamp_type})"
        if typed_by_updated_at and self.dialect.null_keeps_column_type:
            # A timestamp snapshot's validity columns take updated_at's type, such as DATE.
            valid_to_expr = f"CASE WHEN 1 = 0 THEN {updated_at_column} END"
        return (
            f"SELECT *, {valid_from_expr} AS {valid_from_column}, "
            f"{valid_to_expr} AS {valid_to_column} FROM {origin}"
        )

    def run_timestamp(self, current_timestamp: str) -> str:
        """The run's time as the dialect's plain timestamp type, as a check snapshot stamps it."""

        return f"CAST({current_timestamp} AS {self.dialect.timestamp_type})"

    def timestamp_changes_sql(
        self,
        *,
        target: SnapshotChangeTarget,
        updated_at_column: str,
        observed_at_column: str | None,
        initial_valid_from: str | None,
        invalidate_hard_deletes: bool,
        current_timestamp: str,
    ) -> tuple[str, ...]:
        """Render changes, inserting before closing when hard deletes need active-row evidence."""

        initial_valid_from_expr: str = self.initial_valid_from_expr(
            snapshot_strategy="timestamp",
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            initial_valid_from=initial_valid_from,
            source_alias="__source",
            current_timestamp=current_timestamp,
        )
        first_key: str = target.unique_key[0]
        close_sql: str = self._keyed_close_sql(
            target=target,
            valid_to_sql=f"__source.{updated_at_column}",
            changed_sql=f"__source.{updated_at_column} > __target.{updated_at_column}",
        )
        version_valid_from_expr: str = (
            f"CASE WHEN __active.{first_key} IS NULL THEN {initial_valid_from_expr} "
            f"ELSE __source.{updated_at_column} END"
        )
        history_join_sql: str = ""
        insert_guard_sql: str | None = None
        if invalidate_hard_deletes:
            key_sql: str = ", ".join(target.unique_key)
            history_condition: str = self.key_condition(
                left_alias="__history", right_alias="__source", unique_key=target.unique_key
            )
            # A returning key opens at updated_at only after its close, so versions never overlap.
            version_valid_from_expr = (
                f"CASE WHEN __active.{first_key} IS NULL AND __history.__closed_at IS NOT NULL "
                f"AND __source.{updated_at_column} > __history.__closed_at "
                f"THEN __source.{updated_at_column} "
                f"WHEN __active.{first_key} IS NULL AND __history.__closed_at IS NOT NULL "
                f"THEN {current_timestamp} "
                f"WHEN __active.{first_key} IS NULL THEN {initial_valid_from_expr} "
                f"ELSE __source.{updated_at_column} END"
            )
            history_join_sql = (
                f"LEFT JOIN (SELECT {key_sql}, MAX({target.valid_to_column}) AS __closed_at "
                f"FROM {target.destination} GROUP BY {key_sql}) AS __history "
                f"ON {history_condition} "
            )
            inserted_key_condition: str = self.key_condition(
                left_alias="__inserted", right_alias="__source", unique_key=target.unique_key
            )
            insert_guard_sql = (
                f"NOT EXISTS (SELECT 1 FROM {target.destination} AS __inserted "
                f"WHERE {inserted_key_condition} "
                f"AND __inserted.{target.valid_to_column} IS NULL "
                f"AND __inserted.{updated_at_column} = __source.{updated_at_column})"
            )
        insert_sql: str = self._insert_sql(
            target=target,
            valid_from_sql=version_valid_from_expr,
            joins_sql=history_join_sql,
            changed_sql=f"__source.{updated_at_column} > __active.{updated_at_column}",
            insert_guard_sql=insert_guard_sql,
        )
        return self._with_hard_delete_close(
            statements=(insert_sql, close_sql)
            if invalidate_hard_deletes
            else (close_sql, insert_sql),
            target=target,
            invalidate_hard_deletes=invalidate_hard_deletes,
            current_timestamp=current_timestamp,
        )

    def check_changes_sql(
        self,
        *,
        target: SnapshotChangeTarget,
        check_columns: tuple[str, ...],
        updated_at_column: str | None,
        observed_at_column: str | None,
        initial_valid_from: str | None,
        invalidate_hard_deletes: bool,
        current_timestamp: str,
    ) -> tuple[str, ...]:
        """Render the incremental statements for a current-state check snapshot."""

        current_timestamp = self.run_timestamp(current_timestamp)
        initial_valid_from_expr: str = self.initial_valid_from_expr(
            snapshot_strategy="check",
            updated_at_column=updated_at_column,
            observed_at_column=observed_at_column,
            initial_valid_from=initial_valid_from,
            source_alias="__source",
            current_timestamp=current_timestamp,
        )
        change_condition: str = " OR ".join(
            self.dialect.distinct_condition(left=f"__source.{column}", right=f"__target.{column}")
            for column in check_columns
        )
        close_sql: str = self._keyed_close_sql(
            target=target, valid_to_sql=current_timestamp, changed_sql=f"({change_condition})"
        )
        active_change_condition: str = " OR ".join(
            self.dialect.distinct_condition(left=f"__source.{column}", right=f"__active.{column}")
            for column in check_columns
        )
        insert_sql: str = self._insert_sql(
            target=target,
            valid_from_sql=(
                f"CASE WHEN __active.{target.unique_key[0]} IS NULL "
                f"THEN {initial_valid_from_expr} ELSE {current_timestamp} END"
            ),
            joins_sql="",
            changed_sql=f"({active_change_condition})",
        )
        return self._with_hard_delete_close(
            statements=(close_sql, insert_sql),
            target=target,
            invalidate_hard_deletes=invalidate_hard_deletes,
            current_timestamp=current_timestamp,
        )

    def _keyed_close_sql(
        self, *, target: SnapshotChangeTarget, valid_to_sql: str, changed_sql: str
    ) -> str:
        key_condition: str = self.key_condition(
            left_alias="__target", right_alias="__source", unique_key=target.unique_key
        )
        valid_to_column: str = target.valid_to_column
        if self.dialect.update_style == SnapshotUpdateStyle.MERGE:
            return (
                f"MERGE INTO {target.destination} AS __target "
                f"USING {target.origin} AS __source "
                f"ON {key_condition} "
                f"AND __target.{valid_to_column} IS NULL "
                f"AND {changed_sql} "
                f"WHEN MATCHED THEN UPDATE SET {valid_to_column} = {valid_to_sql}"
            )
        if self.dialect.update_style == SnapshotUpdateStyle.TSQL:
            return (
                f"UPDATE __target SET {valid_to_column} = {valid_to_sql} "
                f"FROM {target.destination} AS __target "
                f"JOIN {target.origin} AS __source ON {key_condition} "
                f"WHERE __target.{valid_to_column} IS NULL "
                f"AND {changed_sql}"
            )
        return (
            f"UPDATE {target.destination} AS __target "
            f"SET {valid_to_column} = {valid_to_sql} "
            f"FROM {target.origin} AS __source "
            f"WHERE {key_condition} "
            f"AND __target.{valid_to_column} IS NULL "
            f"AND {changed_sql}"
        )

    def _insert_sql(
        self,
        *,
        target: SnapshotChangeTarget,
        valid_from_sql: str,
        joins_sql: str,
        changed_sql: str,
        insert_guard_sql: str | None = None,
    ) -> str:
        insert_column_sql: str = ", ".join(
            (*target.output_columns, target.valid_from_column, target.valid_to_column)
        )
        output_select_sql: str = ", ".join(f"__source.{column}" for column in target.output_columns)
        active_join_condition: str = self.key_condition(
            left_alias="__active", right_alias="__source", unique_key=target.unique_key
        )
        insert_condition: str = f"__active.{target.unique_key[0]} IS NULL OR {changed_sql}"
        if insert_guard_sql is not None:
            insert_condition = f"({insert_condition}) AND {insert_guard_sql}"
        return (
            f"INSERT INTO {target.destination} ({insert_column_sql}) "
            f"SELECT {output_select_sql}, {valid_from_sql}, "
            f"CAST(NULL AS {self.dialect.timestamp_type}) "
            f"FROM {target.origin} AS __source "
            f"LEFT JOIN {target.destination} AS __active "
            f"ON {active_join_condition} AND __active.{target.valid_to_column} IS NULL "
            f"{joins_sql}"
            f"WHERE {insert_condition}"
        )

    def _with_hard_delete_close(
        self,
        *,
        statements: tuple[str, ...],
        target: SnapshotChangeTarget,
        invalidate_hard_deletes: bool,
        current_timestamp: str,
    ) -> tuple[str, ...]:
        if not invalidate_hard_deletes:
            return statements
        missing_key_condition: str = self.key_condition(
            left_alias="__source", right_alias="__target", unique_key=target.unique_key
        )
        update_sql: str = (
            f"UPDATE {target.destination} AS __target "
            f"SET {target.valid_to_column} = {current_timestamp} "
        )
        if self.dialect.update_style == SnapshotUpdateStyle.TSQL:
            update_sql = (
                f"UPDATE __target SET {target.valid_to_column} = {current_timestamp} "
                f"FROM {target.destination} AS __target "
            )
        hard_delete_sql: str = (
            f"{update_sql}WHERE __target.{target.valid_to_column} IS NULL "
            "AND NOT EXISTS ("
            f"SELECT 1 FROM {target.origin} AS __source "
            f"WHERE {missing_key_condition} AND __source.{target.unique_key[0]} IS NOT NULL"
            ")"
        )
        return (*statements, hard_delete_sql)
