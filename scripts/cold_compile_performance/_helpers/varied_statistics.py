"""Measure workload structure independently of identifiers and SQL literal values."""

import re
from typing import Any, cast

from scripts.cold_compile_performance.constants import (
    VARIED_ARRAY_NODE_KEYS,
    VARIED_WINDOW_NODE_FRAGMENT,
)
from scripts.cold_compile_performance.models import VariedProjectStatistics
from sqlbuild._native import CompiledResourceType, SqlDocument

_UDF_REFERENCE: re.Pattern[str] = re.compile(r'__udf\("([a-zA-Z0-9_]+)"\)')


def varied_project_statistics(payload: dict[str, object]) -> VariedProjectStatistics:
    resources: dict[str, object] = cast(dict[str, object], payload["resources"])
    models: list[dict[str, Any]] = cast(list[dict[str, Any]], resources["models"])
    names: set[str] = {model["name"] for model in models}
    dependencies: dict[str, set[str]] = {}
    for model in models:
        dependencies[model["name"]] = {
            reference["name"]
            for reference in model["depends_on"]
            if reference["resource_type"] == CompiledResourceType.MODEL
        }
    shapes: set[tuple[str, ...]] = set()
    array_models: int = 0
    window_models: int = 0
    for model in models:
        query_sql: str = _UDF_REFERENCE.sub(r"\1", model["query_sql"])
        document: SqlDocument = SqlDocument(source=query_sql, dialect="snowflake")
        shape: tuple[str, ...] = tuple(node.kind for node in document.ast().walk())
        shapes.add(shape)
        array_models += int(any(key in VARIED_ARRAY_NODE_KEYS for key in shape))
        window_models += int(any(VARIED_WINDOW_NODE_FRAGMENT in key for key in shape))
    depths: dict[str, int] = {}
    for name in names:
        _depth(name=name, dependencies=dependencies, depths=depths)
    consumed: set[str] = set().union(*dependencies.values())
    return VariedProjectStatistics(
        model_count=len(models),
        distinct_operator_shapes=len(shapes),
        model_edges=sum(len(parents) for parents in dependencies.values()),
        largest_component=_largest_component(dependencies),
        maximum_depth=max(depths.values(), default=0),
        leaf_models=len(names - consumed),
        array_models=array_models,
        window_models=window_models,
    )


def _depth(*, name: str, dependencies: dict[str, set[str]], depths: dict[str, int]) -> int:
    if name not in depths:
        depths[name] = 1 + max(
            (
                _depth(name=parent, dependencies=dependencies, depths=depths)
                for parent in dependencies[name]
            ),
            default=0,
        )
    return depths[name]


def _largest_component(dependencies: dict[str, set[str]]) -> int:
    neighbors: dict[str, set[str]] = {name: set(parents) for name, parents in dependencies.items()}
    for name, parents in dependencies.items():
        for parent in parents:
            neighbors[parent].add(name)
    unseen: set[str] = set(neighbors)
    largest: int = 0
    while unseen:
        pending: list[str] = [unseen.pop()]
        size: int = 0
        while pending:
            current: str = pending.pop()
            size += 1
            discovered: set[str] = neighbors[current] & unseen
            unseen.difference_update(discovered)
            pending.extend(discovered)
        largest = max(largest, size)
    return largest
