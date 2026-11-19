"""The hold a build keeps on the maintained models it touches, or None when it touches none."""

from __future__ import annotations

from sqlbuild._native import CompiledRelationLocation, PlanOutput, TargetSettings
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.exceptions import AdapterUserError
from sqlbuild.executor.build.models import BuildRuntimeParams
from sqlbuild.executor.maintained_pauses._helpers.scope import held_names, refreshed_for_readers
from sqlbuild.executor.maintained_pauses.classes.maintained_heartbeat import (
    MaintainedHeartbeat,
)
from sqlbuild.executor.maintained_pauses.classes.maintained_hold import MaintainedHold
from sqlbuild.executor.maintained_pauses.constants import (
    HEARTBEAT_SECONDS,
    IGNORE_GRACE_REFUSED_CODE,
)
from sqlbuild.executor.maintained_pauses.types import StateLocation


def hold_maintained_models(
    *,
    plan: PlanOutput,
    adapter: BaseAdapter,
    runtime: BuildRuntimeParams,
    connection_config: dict[str, object],
    heartbeat_seconds: float = HEARTBEAT_SECONDS,
) -> MaintainedHold | None:
    """The build's hold, unacquired; `--ignore-maintained-grace` needs `allow_ignore_grace`."""

    if runtime.ignore_maintained_grace and not runtime.allow_ignore_maintained_grace:
        raise AdapterUserError(
            "--ignore-maintained-grace needs [maintained] allow_ignore_grace = true",
            code=IGNORE_GRACE_REFUSED_CODE,
            help="set allow_ignore_grace = true under [maintained] in sqlbuild_project.toml",
        )
    names: tuple[str, ...] = held_names(plan=plan, pause_maintained=runtime.pause_maintained)
    refreshed: frozenset[str] = refreshed_for_readers(plan=plan)
    if not names and not refreshed:
        return None
    held: dict[str, CompiledRelationLocation] = {name: plan.model_locations[name] for name in names}
    return MaintainedHold(
        adapter=adapter,
        held=held,
        refreshed=refreshed,
        state_location=_state_location(runtime=runtime, held=held) if held else None,
        settings=(
            runtime.run_id,
            runtime.pause_maintained,
            float(runtime.maintained_heartbeat_grace_seconds),
            runtime.ignore_maintained_grace,
        ),
        heartbeat=lambda write: MaintainedHeartbeat(
            adapter=adapter,
            connection_config=connection_config,
            write=write,
            interval_seconds=heartbeat_seconds,
        ),
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
