"""Which maintained models a build holds, upstream first, and which ones it refreshes by hand."""

from __future__ import annotations

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
)

_UNWRITTEN: frozenset[PlanAction] = frozenset(
    {PlanAction.SKIP, PlanAction.KEEP_MAINTAINED, PlanAction.ALTER_MAINTAINED}
)


def held_names(*, plan: PlanOutput, pause_maintained: bool) -> tuple[str, ...]:
    """Maintained models built, or reading what is written (every planned one when paused)."""

    names: set[str] = set()
    entry: ModelPlanEntry
    for entry in plan.model_entries:
        maintained: bool = entry.materialization_type == MaterializationType.MAINTAINED
        if maintained and (pause_maintained or entry.action not in _UNWRITTEN):
            names.add(entry.name)
        if entry.action not in _UNWRITTEN:
            names.update(entry.maintained_readers)
    return _upstream_first(names=frozenset(names), plan=plan)


def refreshed_for_readers(*, plan: PlanOutput) -> frozenset[str]:
    """Maintained models that a model built later in this build reads, directly or not."""

    entries: dict[str, ModelPlanEntry] = {entry.name: entry for entry in plan.model_entries}
    readers: dict[str, tuple[str, ...]] = _model_edges(edges=plan.downstream_deps)
    found: set[str] = set()
    name: str
    for name, entry in entries.items():
        if entry.materialization_type != MaterializationType.MAINTAINED:
            continue
        reached: set[str] = _reachable(start=name, edges=readers)
        if any(_built_table(entry=entries.get(reader)) for reader in reached):
            found.add(name)
    return frozenset(found)


def _built_table(*, entry: ModelPlanEntry | None) -> bool:
    return (
        entry is not None
        and entry.materialization_type != MaterializationType.MAINTAINED
        and entry.action != PlanAction.SKIP
    )


def _upstream_first(*, names: frozenset[str], plan: PlanOutput) -> tuple[str, ...]:
    """A model upstream of another has strictly fewer held ancestors, so it sorts first."""

    upstream: dict[str, tuple[str, ...]] = _model_edges(edges=plan.upstream_deps)
    held_ancestors: dict[str, int] = {
        name: len(_reachable(start=name, edges=upstream) & names) for name in names
    }
    return tuple(sorted(names, key=lambda name: (held_ancestors[name], name)))


def _model_edges(
    *, edges: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]]
) -> dict[str, tuple[str, ...]]:
    models: dict[str, tuple[str, ...]] = {}
    key: CompiledObjectKey
    others: tuple[CompiledObjectKey, ...]
    for key, others in edges.items():
        if key.resource_type == CompiledResourceType.MODEL:
            models[key.name] = _model_names(keys=others)
    return models


def _model_names(*, keys: tuple[CompiledObjectKey, ...]) -> tuple[str, ...]:
    return tuple(key.name for key in keys if key.resource_type == CompiledResourceType.MODEL)


def _reachable(*, start: str, edges: dict[str, tuple[str, ...]]) -> set[str]:
    seen: set[str] = set()
    pending: list[str] = list(edges.get(start, ()))
    while pending:
        name: str = pending.pop()
        if name in seen:
            continue
        seen.add(name)
        pending.extend(edges.get(name, ()))
    return seen
