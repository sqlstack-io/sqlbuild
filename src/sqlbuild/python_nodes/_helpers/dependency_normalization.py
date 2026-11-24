"""Dependency declaration normalization for Python-node decorators."""

from collections.abc import Callable

from sqlbuild.python_nodes.models import NodeSetRef, SqlResourceRef


def normalize_python_node_dependencies(
    value: Callable[..., object]
    | SqlResourceRef
    | NodeSetRef
    | tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]
    | list[Callable[..., object] | SqlResourceRef | NodeSetRef],
) -> tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]:
    if callable(value) or isinstance(value, (SqlResourceRef, NodeSetRef)):
        return (value,)
    return tuple(value)


def normalize_loader_dependencies(
    value: tuple[Callable[..., object] | NodeSetRef, ...]
    | list[Callable[..., object] | NodeSetRef],
) -> tuple[Callable[..., object] | NodeSetRef, ...]:
    return tuple(value)
