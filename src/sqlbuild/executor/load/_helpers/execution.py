"""Shared helpers for source loader execution paths."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry, SourceWriteStrategy
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.load.models import LoadExecutionIndexes, LoadExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.runtime.contracts.types import ExecutionResourceKind


def build_load_execution_indexes(
    *,
    sources: tuple[SourceEntry, ...],
    loader_functions: tuple[DiscoveredLoaderFunction, ...],
) -> LoadExecutionIndexes:
    """Build reusable indexes for load execution and dependency handling."""

    loader_by_name: dict[str, DiscoveredLoaderFunction] = {
        loader.name: loader for loader in loader_functions
    }
    source_by_name: dict[str, SourceEntry] = {source.name: source for source in sources}
    source_by_loader_name: dict[str, SourceEntry] = {
        source.loader: source for source in sources if source.loader is not None
    }
    loader_name_by_function: dict[Callable[..., object], str] = {
        loader.function: loader.name for loader in loader_functions
    }

    loader_ref_entries: dict[Callable[..., object], SourceEntry] = {}
    loader_name: str
    source_entry: SourceEntry
    for loader_name, source_entry in source_by_loader_name.items():
        loader: DiscoveredLoaderFunction | None = loader_by_name.get(loader_name)
        if loader is None:
            continue
        loader_ref_entries[loader.function] = source_entry

    return LoadExecutionIndexes(
        loader_by_name=loader_by_name,
        source_by_name=source_by_name,
        source_by_loader_name=source_by_loader_name,
        loader_ref_entries=loader_ref_entries,
        loader_name_by_function=loader_name_by_function,
        has_loader_dependencies=has_loader_dependencies(
            sources=sources,
            loader_by_name=loader_by_name,
        ),
    )


def has_loader_dependencies(
    *,
    sources: tuple[SourceEntry, ...],
    loader_by_name: dict[str, DiscoveredLoaderFunction],
) -> bool:
    """Return whether any selected load node depends on another loader."""

    source: SourceEntry
    for source in sources:
        if source.loader is None or source.loader not in loader_by_name:
            continue
        if loader_by_name[source.loader].depends_on:
            return True
    return False


def dependency_node_names(*, source: SourceEntry, indexes: LoadExecutionIndexes) -> tuple[str, ...]:
    """Return dependency node names used for skip propagation."""

    if source.loader is None:
        return ()
    loader: DiscoveredLoaderFunction = indexes.loader_by_name[source.loader]
    dependency: Callable[..., object] | SqlResourceRef
    names: list[str] = []
    for dependency in loader.depends_on:
        if isinstance(dependency, SqlResourceRef):
            continue
        if dependency not in indexes.loader_name_by_function:
            continue
        loader_name: str = indexes.loader_name_by_function[dependency]
        dependency_source: SourceEntry | None = indexes.source_by_loader_name.get(loader_name)
        names.append(dependency_source.name if dependency_source is not None else loader_name)
    return tuple(names)


def build_source_upstream_names(
    *, sources: tuple[SourceEntry, ...], indexes: LoadExecutionIndexes
) -> dict[str, tuple[str, ...]]:
    """Return source-node upstream dependency names for selected load nodes."""

    source_names: frozenset[str] = frozenset(source.name for source in sources)
    upstream_names: dict[str, tuple[str, ...]] = {}
    source: SourceEntry
    for source in sources:
        upstream_names[source.name] = tuple(
            dependency_name
            for dependency_name in dependency_node_names(source=source, indexes=indexes)
            if dependency_name in source_names
        )
    return upstream_names


def build_source_downstream_names(
    *, upstream_names: dict[str, tuple[str, ...]]
) -> dict[str, tuple[str, ...]]:
    """Return source-node downstream dependency names for selected load nodes."""

    downstream_names: dict[str, list[str]] = {name: [] for name in upstream_names}
    source_name: str
    dependencies: tuple[str, ...]
    for source_name, dependencies in upstream_names.items():
        dependency_name: str
        for dependency_name in dependencies:
            downstream_names.setdefault(dependency_name, []).append(source_name)
    return {name: tuple(dependents) for name, dependents in downstream_names.items()}


def should_skip_due_to_hard_dependency(
    *,
    source: SourceEntry,
    failed_or_hard_skipped: set[str],
    indexes: LoadExecutionIndexes,
) -> bool:
    """Return whether a source should skip because an upstream dependency is blocking."""

    return any(
        dependency in failed_or_hard_skipped
        for dependency in dependency_node_names(source=source, indexes=indexes)
    )


def should_soft_skip_due_to_all_skipped_dependencies(
    *,
    source: SourceEntry,
    results_by_name: dict[str, LoadExecutionResult],
    indexes: LoadExecutionIndexes,
) -> bool:
    """Return whether every upstream dependency completed as a non-blocking skip."""

    dependencies: tuple[str, ...] = dependency_node_names(source=source, indexes=indexes)
    if not dependencies:
        return False
    return all(
        (result := results_by_name.get(dependency)) is not None
        and result.status == ExecutionStatus.SKIPPED
        and result.skip_mode == SkipMode.SOFT
        for dependency in dependencies
    )


def load_resource_kind(source: SourceEntry) -> ExecutionResourceKind:
    """Return the display/execution kind for one load node."""

    return (
        ExecutionResourceKind.LOADER
        if source.meta.get("sqlbuild_loader_node") is True
        else ExecutionResourceKind.SOURCE
    )


def skipped_load_result(
    *,
    source: SourceEntry,
    reason: str | None = None,
    mode: SkipMode = SkipMode.HARD,
) -> LoadExecutionResult:
    """Build a skipped result for a loader/source node."""

    return LoadExecutionResult(
        source_name=source.name,
        loader_name=source.loader or "",
        status=ExecutionStatus.SKIPPED,
        target=source.table or source.name,
        resource_kind=load_resource_kind(source),
        skip_mode=mode,
        skip_reason=reason,
    )


def is_untargeted_self_managed_intermediate(
    *, source_entry: SourceEntry, loader_function: DiscoveredLoaderFunction
) -> bool:
    """Return whether a self-managed intermediate lacks a declared target."""

    return (
        source_entry.meta.get("sqlbuild_loader_node") is True
        and source_entry.write_strategy is None
        and loader_function.destination is None
    )


def row_write_entry(source_entry: SourceEntry) -> SourceEntry:
    """Return the entry rows are written with: an intermediate loader defaults to `table`."""

    if (
        source_entry.meta.get("sqlbuild_loader_node") is True
        and source_entry.write_strategy is None
    ):
        return replace(source_entry, write_strategy=SourceWriteStrategy.TABLE)
    return source_entry
