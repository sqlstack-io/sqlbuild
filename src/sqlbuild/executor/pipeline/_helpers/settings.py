"""Executor pipeline settings resolution."""

from __future__ import annotations

from dataclasses import replace

from sqlbuild._native import PlannerInputError, PlanOutput, PlanWarning, SettingsConfig
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.types import TablePromotionMode
from sqlbuild.executor.build.models import (
    BuildCallbacks,
    BuildCustomizations,
    BuildInitialState,
    BuildRuntimeParams,
)
from sqlbuild.executor.pipeline.models import ResolvedBuildInputs


def resolve_promotion_mode(
    *,
    settings: SettingsConfig,
    adapter: BaseAdapter,
) -> TablePromotionMode:
    """Resolve the effective table promotion mode from project settings or adapter default."""

    if settings.table_promotion_mode is not None:
        return TablePromotionMode(settings.table_promotion_mode)
    return adapter.default_table_promotion_mode()


def resolve_build_inputs(
    *,
    settings: SettingsConfig,
    adapter: BaseAdapter,
    runtime: BuildRuntimeParams,
    callbacks: BuildCallbacks | None,
    customizations: BuildCustomizations | None,
    initial_state: BuildInitialState | None,
) -> ResolvedBuildInputs:
    """Resolve optional bundles and settings-derived runtime fields for one build run."""

    return ResolvedBuildInputs(
        runtime=replace(
            runtime,
            promotion_mode=resolve_promotion_mode(settings=settings, adapter=adapter),
            query_change_tracking=(
                settings.query_change_tracking
                if runtime.query_change_tracking is None
                else runtime.query_change_tracking
            ),
            microbatch_concurrency=settings.microbatch_concurrency,
            microbatch_unaccounted_partition_policy=(
                settings.microbatch_unaccounted_partition_policy
            ),
        ),
        callbacks=callbacks if callbacks is not None else BuildCallbacks(),
        customizations=customizations if customizations is not None else BuildCustomizations(),
        initial_state=initial_state if initial_state is not None else BuildInitialState(),
    )


def plannable_plan(plan: PlanOutput) -> PlanOutput:
    """The plan, or the first model's planning error, so no run silently skips that model."""

    for entry in plan.model_entries:
        error: PlanWarning | None = entry.planning_error
        if error is not None:
            raise PlannerInputError(error.message, code=error.code, help=error.help)
    return plan
