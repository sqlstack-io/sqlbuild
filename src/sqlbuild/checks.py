"""Public decorator API for SQLBuild checks."""

from __future__ import annotations

from collections.abc import Callable, Sequence

from sqlbuild.executor.python_nodes.models import CheckContext, PythonCheckResult
from sqlbuild.python_nodes.main.apply_check import apply_check
from sqlbuild.python_nodes.main.read_check_definition import read_check_definition
from sqlbuild.python_nodes.models import CheckDefinition, NodeSetRef, SqlResourceRef
from sqlbuild.python_nodes.types import PythonCheckSeverity

__all__ = ("CheckContext", "PythonCheckResult", "check", "get_check_definition")


def check(
    *,
    depends_on: Callable[..., object]
    | SqlResourceRef
    | NodeSetRef
    | tuple[Callable[..., object] | SqlResourceRef | NodeSetRef, ...]
    | list[Callable[..., object] | SqlResourceRef | NodeSetRef],
    name: str | None = None,
    severity: str | PythonCheckSeverity = PythonCheckSeverity.ERROR,
    tags: Sequence[str] = (),
    group: str | None = None,
    description: str | None = None,
    meta: dict[str, object] | None = None,
) -> Callable[[Callable[..., object]], Callable[..., object]]:
    """Mark a Python function as a SQLBuild check."""

    return apply_check(
        depends_on=depends_on,
        name=name,
        severity=severity,
        tags=tags,
        group=group,
        description=description,
        meta=meta,
    )


def get_check_definition(function: Callable[..., object]) -> CheckDefinition | None:
    """Return SQLBuild check metadata from a decorated function, if present."""

    return read_check_definition(function)
