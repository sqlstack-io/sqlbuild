"""Maintained model lifecycle: the warehouse refreshes the model; a build creates or alters it."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from sqlbuild._native import CompiledRelationLocation, ModelPlanEntry, PlanAction
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.types import MaintainedOperation, RelationType
from sqlbuild.adapter.relations.main.cached_relation_type import cached_relation_type
from sqlbuild.executor.maintained_pauses.classes.maintained_hold import MaintainedHold
from sqlbuild.executor.run._helpers.materializations.view import execute_in_place_entry
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext

_NO_WAREHOUSE_CODE: str = "A717"


def execute_maintained_entry(*, context: ModelMaterializationContext) -> ModelExecutionResult:
    """Create, alter or keep one maintained model, refresh a kept one when asked, then audit it."""

    def materialize(*, target_qualified: str, statement_recorder: StatementRecorder) -> None:
        entry: ModelPlanEntry = context.entry
        hold: MaintainedHold | None = context.maintained_hold
        if entry.action == PlanAction.CREATE_MAINTAINED:
            _create(context=context, destination=target_qualified, recorder=statement_recorder)
            if hold is not None:
                hold.after_create(connection=context.connection, name=entry.name)
            return
        if entry.action == PlanAction.ALTER_MAINTAINED:
            _run(
                adapter=context.adapter,
                connection=context.connection,
                statements=context.adapter.render_alter_maintained(
                    destination=target_qualified,
                    target_lag=str(entry.target_lag),
                    warehouse=entry.refresh_warehouse,
                ),
                recorder=statement_recorder,
            )
        if entry.refresh_on_build or (hold is not None and hold.refreshes(name=entry.name)):
            _run(
                adapter=context.adapter,
                connection=context.connection,
                statements=context.adapter.render_maintained_operation(
                    destination=target_qualified, operation=MaintainedOperation.REFRESH
                ),
                recorder=statement_recorder,
            )

    return execute_in_place_entry(
        context=context,
        materialize=materialize,
        action_name=str(context.entry.action),
        built="the maintained model's refresh",
    )


def _create(
    *, context: ModelMaterializationContext, destination: str, recorder: StatementRecorder
) -> None:
    """Create or replace; a table or view the model was before is dropped, never a dynamic table."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    warehouse: str | None = entry.refresh_warehouse or adapter.session_warehouse(
        connection=connection
    )
    if warehouse is None:
        raise AdapterUserError(
            f"maintained model '{entry.name}' has no warehouse to refresh it",
            code=_NO_WAREHOUSE_CODE,
            help="set refresh_warehouse on the model, or a warehouse on the target's connection",
        )
    statements: tuple[str, ...] = adapter.render_create_maintained(
        destination=destination,
        sql=entry.resolved_sql,
        target_lag=str(entry.target_lag),
        refresh_mode=str(entry.refresh_mode),
        warehouse=warehouse,
    )
    try:
        _run(adapter=adapter, connection=connection, statements=statements, recorder=recorder)
    except Exception as error:
        former: RelationType | None = _former_relation(
            adapter=adapter, connection=connection, entry=entry
        )
        if former is None:
            raise error
        drop: Callable[..., None] = (
            adapter.drop_view if former == RelationType.VIEW else adapter.drop
        )
        drop(
            connection=connection,
            destination=destination,
            if_exists=True,
            statement_recorder=recorder,
        )
        _run(adapter=adapter, connection=connection, statements=statements, recorder=recorder)


def _former_relation(
    *, adapter: BaseAdapter, connection: Any, entry: ModelPlanEntry
) -> RelationType | None:
    """The type of a relation at the destination the warehouse does not list as maintained."""

    location: CompiledRelationLocation = entry.destination
    existing: RelationType | None = cached_relation_type(
        adapter=adapter,
        connection=connection,
        database=location.database,
        schema=location.schema,
        name=location.name,
    )
    if existing is None:
        return None
    listed: tuple[str, str] | None = adapter.maintained_refresh_settings(
        connection=connection,
        database=location.database,
        schema=location.schema,
        name=location.name,
    )
    return existing if listed is None else None


def _run(
    *,
    adapter: BaseAdapter,
    connection: Any,
    statements: tuple[str, ...],
    recorder: StatementRecorder,
) -> None:
    recorder.record_many(statements)
    for statement in statements:
        adapter.execute(connection=connection, sql=statement)
