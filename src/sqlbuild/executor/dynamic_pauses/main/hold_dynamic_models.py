"""The hold a build keeps on the dynamic models it touches, or None when it touches none."""

from __future__ import annotations

from sqlbuild._native import (
    DYNAMIC_RESUME_SECONDS,
    CompiledRelationLocation,
    PlanOutput,
    TargetSettings,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.executor.build.models import BuildRuntimeParams
from sqlbuild.executor.dynamic_pauses._helpers.scope import held_names, refreshed_for_readers
from sqlbuild.executor.dynamic_pauses.classes.dynamic_heartbeat import (
    DynamicHeartbeat,
)
from sqlbuild.executor.dynamic_pauses.classes.dynamic_hold import DynamicHold
from sqlbuild.executor.dynamic_pauses.constants import (
    CLAIM_POLL_SECONDS,
    HEARTBEAT_SECONDS,
    IGNORE_GRACE_REFUSED_CODE,
)
from sqlbuild.executor.dynamic_pauses.types import StateLocation


def hold_dynamic_models(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    runtime: BuildRuntimeParams,
    connection_config: dict[str, object],
    heartbeat_seconds: float = HEARTBEAT_SECONDS,
    claim_wait: tuple[float, float] = (float(DYNAMIC_RESUME_SECONDS), CLAIM_POLL_SECONDS),
) -> DynamicHold | None:
    """The build's hold, unacquired; `claim_wait` is a sync claim's age limit and poll interval."""

    if runtime.ignore_dynamic_grace and not runtime.allow_ignore_dynamic_grace:
        raise AdapterUserError(
            "--ignore-dynamic-grace needs [dynamic] allow_ignore_grace = true",
            code=IGNORE_GRACE_REFUSED_CODE,
            help="set allow_ignore_grace = true under [dynamic] in sqlbuild_project.toml",
        )
    names: tuple[str, ...] = held_names(plan=plan, pause_dynamic=runtime.pause_dynamic)
    refreshed: frozenset[str] = refreshed_for_readers(plan=plan)
    if not names and not refreshed:
        return None
    held: dict[str, CompiledRelationLocation] = {name: plan.model_locations[name] for name in names}
    return DynamicHold(
        adapter=adapter,
        held=held,
        refreshed=refreshed,
        state_location=_state_location(runtime=runtime, held=held) if held else None,
        settings=(
            runtime.run_id,
            runtime.pause_dynamic,
            float(runtime.dynamic_heartbeat_grace_seconds),
            runtime.ignore_dynamic_grace,
        ),
        heartbeat=lambda write: DynamicHeartbeat(
            adapter=adapter,
            connection_config=connection_config,
            write=write,
            interval_seconds=heartbeat_seconds,
        ),
        claim_wait=claim_wait,
    )


def _state_location(
    *, runtime: BuildRuntimeParams, held: dict[str, CompiledRelationLocation]
) -> StateLocation:
    """The target's schema, next to the runs table; the first held model's for a preserve schema."""

    target: TargetSettings | None = runtime.target_config
    if target is not None and target.schema is not None and not target.schema_preserved:
        return (target.database, target.schema)
    first: CompiledRelationLocation = next(iter(held.values()))
    return (first.database, str(first.schema))
