"""Partition-tracked custom materialization for the waffle shop fixture.

Tracks which date partitions have been materialized in a state table.
Generates the expected date range, checks the tracking table for untracked
partitions, then builds each one: CTAS into staging, audit, delete-insert
into target, record in tracking table. The target is never dropped.
"""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.types import FrameworkType
from sqlbuild.executor.auditing.models import AuditExecutionResult
from sqlbuild.executor.custom.models import MaterializationContext, MaterializationResult

_AUDIT_ERROR: str = "ERROR"


def materialize(ctx: MaterializationContext) -> MaterializationResult:
    tracking_table: str = ctx.qualify_in_destination_schema(str(ctx.config["tracking_table"]))
    partition_col: str = str(ctx.config["partition_column"])
    date_start: str = str(ctx.config["date_range_start"])
    date_end: str = str(ctx.config["date_range_end"])
    staging: str = ctx.qualify_in_destination_schema(f"{ctx.destination_name}__staging")
    string_type: str = ctx.adapter.render_framework_type(FrameworkType.STRING)
    timestamp_type: str = ctx.adapter.render_framework_type(FrameworkType.TIMESTAMP)
    built_at_default: str = ""
    if ctx.adapter.sql_analysis_dialect() != "databricks":
        built_at_default = " DEFAULT CURRENT_TIMESTAMP"

    ctx.execute_sql(
        f"CREATE TABLE IF NOT EXISTS {tracking_table} "
        f"(partition_value {string_type}, run_id {string_type}, "
        f"built_at {timestamp_type}{built_at_default})"
    )

    if ctx.is_full_refresh:
        ctx.log.info("clearing partition tracking state")
        ctx.execute_sql(f"DELETE FROM {tracking_table}")

    all_partitions: list[str] = _generate_date_range(date_start, date_end)
    ctx.log.info("checking for stale partitions")
    stale: list[str] = _find_stale_partitions(ctx, tracking_table, all_partitions)

    if not stale:
        if ctx.on_progress is not None:
            ctx.on_progress("no stale partitions")
        return MaterializationResult(relation=ctx.destination, audit_results=())

    if ctx.on_progress is not None:
        ctx.on_progress(f"{len(stale)} partitions to build")

    all_audit_results: list[AuditExecutionResult] = []
    target_exists: bool = ctx.adapter.relation_exists(
        connection=ctx.connection,
        database=ctx.destination_database,
        schema=ctx.destination_schema,
        name=ctx.destination_name,
    )

    partition_value: str
    for i, partition_value in enumerate(stale):
        next_day: str = _next_date(partition_value)
        partition_sql: str = ctx.sql.replace("@@@partition_start", f"'{partition_value}'").replace(
            "@@@partition_end", f"'{next_day}'"
        )

        if ctx.on_progress is not None:
            ctx.on_progress(f"partition {i + 1}/{len(stale)}: {partition_value}")
        ctx.log.info(f"building partition {partition_value}")

        ctx.adapter.drop(
            connection=ctx.connection,
            destination=staging,
            if_exists=True,
            statement_recorder=ctx.statement_recorder,
        )
        ctx.adapter.create_table_as(
            connection=ctx.connection,
            destination=staging,
            sql=partition_sql,
            statement_recorder=ctx.statement_recorder,
        )

        audit_results: tuple[AuditExecutionResult, ...] = ctx.run_audits(staging)
        all_audit_results.extend(audit_results)
        if any(r.outcome.value == _AUDIT_ERROR for r in audit_results):
            return MaterializationResult(
                relation=ctx.destination,
                failed=True,
                error=f"audit failed for partition {partition_value}",
                cleanup_relations=(staging,),
                audit_results=tuple(all_audit_results),
            )

        if not target_exists:
            ctx.log.info("promoting first partition into target")
            ctx.adapter.rename(
                connection=ctx.connection,
                origin=staging,
                destination=ctx.destination,
                statement_recorder=ctx.statement_recorder,
            )
            target_exists = True
        else:
            ctx.log.info("promoting partition into target")
            ctx.execute_sql(
                f"DELETE FROM {ctx.destination} "
                f"WHERE CAST({partition_col} AS DATE) >= CAST('{partition_value}' AS DATE) "
                f"AND CAST({partition_col} AS DATE) < CAST('{next_day}' AS DATE)"
            )
            ctx.execute_sql(f"INSERT INTO {ctx.destination} SELECT * FROM {staging}")

        ctx.adapter.merge(
            connection=ctx.connection,
            destination=tracking_table,
            sql=(
                f"SELECT '{partition_value}' AS partition_value, "
                f"'{ctx.run_id}' AS run_id, CURRENT_TIMESTAMP AS built_at"
            ),
            unique_key="partition_value",
            statement_recorder=ctx.statement_recorder,
        )

    ctx.adapter.drop(
        connection=ctx.connection,
        destination=staging,
        if_exists=True,
        statement_recorder=ctx.statement_recorder,
    )
    return MaterializationResult(
        relation=ctx.destination,
        cleanup_relations=(staging,),
        audit_results=tuple(all_audit_results),
    )


def _find_stale_partitions(
    ctx: MaterializationContext,
    tracking_table: str,
    all_partitions: list[str],
) -> list[str]:
    if not all_partitions:
        return []

    cursor: Any = ctx.execute_sql(f"SELECT partition_value FROM {tracking_table}")
    tracked: set[str] = {str(row[0]) for row in cursor.fetchall()}
    stale: list[str] = [p for p in all_partitions if p not in tracked]
    latest_partition: str = all_partitions[-1]
    if latest_partition not in stale:
        stale.append(latest_partition)
    return stale


def _generate_date_range(start: str, end: str) -> list[str]:
    result: list[str] = []
    current: str = start
    while current < end:
        result.append(current)
        current = _next_date(current)
    return result


def _next_date(date_str: str) -> str:
    parts: list[str] = date_str.split("-")
    year: int = int(parts[0])
    month: int = int(parts[1])
    day: int = int(parts[2]) + 1
    if day > 28:
        day = 1
        month += 1
    if month > 12:
        month = 1
        year += 1
    return f"{year:04d}-{month:02d}-{day:02d}"
