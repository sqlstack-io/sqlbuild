"""Shared helpers for source loader execution paths."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import replace

from sqlbuild._native import DiscoveredLoaderFunction, SourceEntry, SourceWriteStrategy
from sqlbuild.compiler.python_nodes.types import SkipMode
from sqlbuild.executor.load.models import LoadExecutionIndexes, LoadExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
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
