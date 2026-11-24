"""A DuckDB adapter that builds dynamic models as tables, for executor lifecycle tests."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanReason,
)
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.types import DynamicListing
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.run.models import ModelMaterializationContext

DAILY_TOTALS_SQL: str = "SELECT 3 AS orders"


class DynamicDuckDbAdapter(DuckDbAdapter):
    """Builds a dynamic model as a table that, like a dynamic table, replaces nothing else."""

    def __init__(
        self,
        *,
        session_warehouse: str | None,
        listed_settings: DynamicListing | None = None,
        failing_query: str | None = None,
        failing_renames: dict[str, str] | None = None,
    ) -> None:
        super().__init__()
        self.warehouse: str | None = session_warehouse
        self.listed_settings: DynamicListing | None = listed_settings
        self.failing_query: str | None = failing_query
        self.failing_renames: dict[str, str] = dict(failing_renames or {})
        self.created: list[tuple[str, str, str]] = []
        self.altered: list[tuple[str, str | None]] = []

    def session_warehouse(self, *, connection: Any) -> str | None:
        del connection
        return self.warehouse

    def dynamic_listing(
        self, *, connection: Any, database: str | None, schema: str | None, name: str
    ) -> DynamicListing | None:
        del connection, database, schema, name
        return self.listed_settings

    def render_create_dynamic(
        self, *, destination: str, sql: str, target_lag: str, refresh_mode: str, warehouse: str
    ) -> tuple[str, ...]:
        self.created.append((target_lag, refresh_mode, warehouse))
        return (f"CREATE TABLE {destination} AS {self.failing_query or sql}",)

    def render_rename(self, *, origin: str, destination: str) -> tuple[str, ...]:
        renamed: str = f"ALTER TABLE {origin} RENAME TO {destination.split('.')[-1]}"
        return (self.failing_renames.get(origin, renamed),)

    def render_rename_view(self, *, origin: str, destination: str) -> tuple[str, ...]:
        return (f"ALTER VIEW {origin} RENAME TO {destination.split('.')[-1]}",)

    def render_alter_dynamic(
        self, *, destination: str, target_lag: str, warehouse: str | None
    ) -> tuple[str, ...]:
        self.altered.append((target_lag, warehouse))
        lag: str = target_lag.replace("'", "''")
        return (f"COMMENT ON TABLE {destination} IS 'refreshed within {lag}'",)


def dynamic_context(
    *,
    adapter: DynamicDuckDbAdapter,
    connection: Any,
    action: PlanAction,
    refresh_warehouse: str | None,
) -> ModelMaterializationContext:
    """Daily totals, dynamic within an hour, planned with `action`."""

    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="daily_totals"),
        name="daily_totals",
        relative_path=Path("sql/models/daily_totals.sql"),
        materialization_type=MaterializationType.DYNAMIC,
        action=action,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None,
            schema="staging",
            name="daily_totals",
            qualified_name="staging.daily_totals",
        ),
        fingerprint_query_sql=DAILY_TOTALS_SQL,
        resolved_sql=DAILY_TOTALS_SQL,
        logical_ddl="",
        target_lag="'1 hour'",
        refresh_mode="incremental",
        refresh_warehouse=refresh_warehouse,
    )
    return ModelMaterializationContext(
        entry=entry,
        adapter=adapter,
        connection=connection,
        model_locations={},
        seed_locations={},
        source_map={},
        python_source_read_map={},
        model_audits=(),
        run_id="dynamic_run",
        query_change_tracking=True,
    )


def listed_types(*, connection: Any) -> list[tuple[Any, ...]]:
    """The relation types daily_totals is listed as; empty when nothing holds the name."""

    return connection.execute(
        "SELECT table_type FROM information_schema.tables "
        "WHERE table_schema = 'staging' AND table_name = 'daily_totals'"
    ).fetchall()


class TrackingDuckDbAdapter(DuckDbAdapter):
    """Records whether each table it creates was asked to track changes."""

    def __init__(self) -> None:
        super().__init__()
        self.tracking: list[bool] = []

    def create_table_as(
        self,
        *,
        connection: Any,
        destination: str,
        sql: str,
        config: dict[str, Any] | None = None,
        statement_recorder: StatementRecorder,
    ) -> None:
        self.tracking.append(bool((config or {}).get("change_tracking")))
        super().create_table_as(
            connection=connection,
            destination=destination,
            sql=sql,
            config=config,
            statement_recorder=statement_recorder,
        )


class TrackedSnapshotDuckDbAdapter(DuckDbAdapter):
    """Records each table it is asked to track changes on after creating it."""

    def __init__(self) -> None:
        super().__init__()
        self.tracked: list[str] = []

    def render_track_changes(self, *, destination: str) -> tuple[str, ...]:
        self.tracked.append(destination)
        return ()
