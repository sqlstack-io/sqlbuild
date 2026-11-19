"""Dynamic model lifecycle: the warehouse refreshes the model; a build creates or alters it."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from sqlbuild._native import (
    CompiledRelationLocation,
    ModelPlanEntry,
    PlanAction,
    error_code,
    error_message,
    fit_auxiliary_relation_name,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.classes.statement_recorder import StatementRecorder
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.adapter.contract.types import DynamicListing, DynamicOperation, RelationType
from sqlbuild.adapter.relations.main.cached_relation_type import cached_relation_type
from sqlbuild.adapter.relations.main.resolve_qualified_name_parts import (
    resolve_qualified_name_parts,
)
from sqlbuild.executor.dynamic_pauses.classes.dynamic_hold import DynamicHold
from sqlbuild.executor.run._helpers.materializations.view import execute_in_place_entry
from sqlbuild.executor.run.constants import REPLACED_RELATION_SUFFIX, RUN_STAGING_FAILED_CODE
from sqlbuild.executor.run.models import ModelExecutionResult, ModelMaterializationContext

_NO_WAREHOUSE_CODE: str = "A717"


def execute_dynamic_entry(*, context: ModelMaterializationContext) -> ModelExecutionResult:
    """Create, alter or keep one dynamic model, refresh a kept one when asked, then audit it."""

    def materialize(*, target_qualified: str, statement_recorder: StatementRecorder) -> None:
        entry: ModelPlanEntry = context.entry
        hold: DynamicHold | None = context.dynamic_hold
        if entry.action == PlanAction.CREATE_DYNAMIC:
            _create(context=context, destination=target_qualified, recorder=statement_recorder)
            if hold is not None:
                hold.after_create(connection=context.connection, name=entry.name)
            return
        if entry.action == PlanAction.ALTER_DYNAMIC:
            _run(
                adapter=context.adapter,
                connection=context.connection,
                statements=context.adapter.render_alter_dynamic(
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
                statements=context.adapter.render_dynamic_operation(
                    destination=target_qualified, operation=DynamicOperation.REFRESH
                ),
                recorder=statement_recorder,
            )

    return execute_in_place_entry(
        context=context,
        materialize=materialize,
        action_name=str(context.entry.action),
        built="the dynamic model's refresh",
    )


def _create(
    *, context: ModelMaterializationContext, destination: str, recorder: StatementRecorder
) -> None:
    """Create or replace; a table or view the model was before is swapped out, never a DT."""

    entry: ModelPlanEntry = context.entry
    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    warehouse: str | None = entry.refresh_warehouse or adapter.session_warehouse(
        connection=connection
    )
    if warehouse is None:
        raise AdapterUserError(
            f"dynamic model '{entry.name}' has no warehouse to refresh it",
            code=_NO_WAREHOUSE_CODE,
            help="set refresh_warehouse on the model, or a warehouse on the target's connection",
        )
    statements: tuple[str, ...] = adapter.render_create_dynamic(
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
        _replace_former(
            context=context,
            destination=destination,
            statements=statements,
            former=former,
            recorder=recorder,
        )


def _replace_former(
    *,
    context: ModelMaterializationContext,
    destination: str,
    statements: tuple[str, ...],
    former: RelationType,
    recorder: StatementRecorder,
) -> None:
    """Move the former table or view aside, create, then drop it; a failed create restores it."""

    adapter: BaseAdapter = context.adapter
    connection: Any = context.connection
    location: CompiledRelationLocation = context.entry.destination
    aside: str = resolve_qualified_name_parts(
        adapter=adapter,
        database=location.database,
        schema=location.schema,
        name=fit_auxiliary_relation_name(
            base_name=location.name,
            suffix=REPLACED_RELATION_SUFFIX,
            identifier_limit=adapter.maximum_identifier_length(),
        ),
    )
    view: bool = former == RelationType.VIEW
    rename: Callable[..., None] = adapter.rename_view if view else adapter.rename
    drop: Callable[..., None] = adapter.drop_view if view else adapter.drop
    rename(
        connection=connection, origin=destination, destination=aside, statement_recorder=recorder
    )
    try:
        _run(adapter=adapter, connection=connection, statements=statements, recorder=recorder)
    except Exception as create_error:
        try:
            rename(
                connection=connection,
                origin=aside,
                destination=destination,
                statement_recorder=recorder,
            )
        except Exception as restore_error:
            raise AdapterUserError(
                f"{error_message(create_error)}; restoring the former {former.value} "
                f"also failed, so it is left at {aside}: {error_message(restore_error)}",
                code=error_code(error=create_error, fallback_code=RUN_STAGING_FAILED_CODE),
                help=f"rename {aside} back to {destination}, then build the model again",
            ) from create_error
        raise
    drop(connection=connection, destination=aside, if_exists=True, statement_recorder=recorder)


def _former_relation(
    *, adapter: BaseAdapter, connection: Any, entry: ModelPlanEntry
) -> RelationType | None:
    """The type of a relation at the destination the warehouse does not list as dynamic."""

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
    listed: DynamicListing | None = adapter.dynamic_listing(
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
