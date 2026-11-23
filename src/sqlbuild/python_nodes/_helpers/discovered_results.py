"""A discover function's items checked against the assets that run once per item, as JSON data."""

from __future__ import annotations

import inspect
from collections.abc import Callable, Mapping, Sequence

from sqlbuild._native import PythonNodeDiscoveryError
from sqlbuild.python_nodes.constants import CONTEXT_PARAMETER_NAMES, ITEM_DEPENDENCY_KIND
from sqlbuild.python_nodes.models import (
    Discovered,
    DiscoveredItemRef,
    Item,
    NodeSetRef,
    SqlResourceRef,
)
from sqlbuild.python_nodes.types import NodeSetKind, SqlResourceRefKind


def checked_items(
    *,
    returned: object,
    functions: tuple[Callable[..., object], ...],
    provider_names: frozenset[str],
) -> dict[str, object]:
    """`returned` as the snapshot's JSON object; anything but a valid `Discovered` raises."""

    if not isinstance(returned, Discovered):
        raise PythonNodeDiscoveryError(
            f"a @discover function must return Discovered(...), not {type(returned).__name__}"
        )
    items: list[dict[str, object]] = [
        _item(item=item, functions=functions, provider_names=provider_names)
        for item in returned.items
    ]
    payload: dict[str, object] = {"items": items, "tags": dict(returned.tags or {})}
    if returned.max_parallel is not None:
        payload["max_parallel"] = returned.max_parallel
    return payload


def _item(
    *,
    item: object,
    functions: tuple[Callable[..., object], ...],
    provider_names: frozenset[str],
) -> dict[str, object]:
    if not isinstance(item, Item):
        raise PythonNodeDiscoveryError(
            f"Discovered items must be Item(...), not {type(item).__name__}"
        )
    args: dict[str, object] = dict(item.args)
    for function in functions:
        problem: str | None = _args_problem(
            function=function, args=args, provider_names=provider_names
        )
        if problem is not None:
            raise PythonNodeDiscoveryError(f"item '{item.id}': {problem}")
    return {
        "id": item.id,
        "depends_on": [_dependency(dependency) for dependency in item.depends_on],
        "args": args,
    }


def _dependency(dependency: object) -> object:
    if isinstance(dependency, str):
        return {ITEM_DEPENDENCY_KIND: dependency}
    if isinstance(dependency, SqlResourceRef):
        return {dependency.kind.value: dependency.name}
    if isinstance(dependency, NodeSetRef):
        return {dependency.kind.value: {"name": dependency.name, "exclude": dependency.exclude}}
    raise PythonNodeDiscoveryError(
        "an item depends on item IDs, model(), source(), seed(), job() or tag(), "
        f"not {type(dependency).__name__}"
    )


def _args_problem(
    *, function: Callable[..., object], args: dict[str, object], provider_names: frozenset[str]
) -> str | None:
    """Why `args` cannot fill `function`'s parameters other than its context and providers."""

    parameters: list[inspect.Parameter] = [
        parameter
        for parameter in inspect.signature(function).parameters.values()
        if parameter.kind not in {inspect.Parameter.VAR_POSITIONAL, inspect.Parameter.VAR_KEYWORD}
    ]
    named_context: bool = any(parameter.name in CONTEXT_PARAMETER_NAMES for parameter in parameters)
    if not named_context and parameters and parameters[0].name not in args:
        parameters = parameters[1:]
    fillable: list[inspect.Parameter] = [
        parameter
        for parameter in parameters
        if parameter.name not in CONTEXT_PARAMETER_NAMES and parameter.name not in provider_names
    ]
    names: set[str] = {parameter.name for parameter in fillable}
    unknown: list[str] = sorted(set(args) - names)
    function_name: str = str(getattr(function, "__name__", function))
    if unknown:
        return f"{function_name} has no parameter {', '.join(unknown)}"
    missing: list[str] = [
        parameter.name
        for parameter in fillable
        if parameter.default is inspect.Parameter.empty and parameter.name not in args
    ]
    if missing:
        return f"args leave {function_name}'s {', '.join(missing)} unset"
    return None


def item_dependency_refs(
    *, depends_on: Sequence[Mapping[str, object]]
) -> tuple[DiscoveredItemRef | SqlResourceRef | NodeSetRef, ...]:
    """Each saved `{"item": node}`, `{"model": name}` or `{"job": {...}}` as a node reference."""

    return tuple(_reference(dependency) for dependency in depends_on)


def _reference(
    dependency: Mapping[str, object],
) -> DiscoveredItemRef | SqlResourceRef | NodeSetRef:
    ((kind, value),) = dependency.items()
    sql_kinds: dict[str, SqlResourceRefKind] = {
        member.value: member for member in SqlResourceRefKind
    }
    set_kinds: dict[str, NodeSetKind] = {member.value: member for member in NodeSetKind}
    if kind == ITEM_DEPENDENCY_KIND:
        return DiscoveredItemRef(name=str(value))
    if kind in sql_kinds:
        return SqlResourceRef(kind=sql_kinds[kind], name=str(value))
    if kind in set_kinds and isinstance(value, Mapping):
        fields: dict[str, object] = {str(key): item for key, item in value.items()}
        exclude: object = fields.get("exclude")
        return NodeSetRef(
            kind=set_kinds[kind],
            name=str(fields["name"]),
            exclude=None if exclude is None else str(exclude),
        )
    raise PythonNodeDiscoveryError(f"a discovered item cannot depend on {kind!r}")
