"""Render a Jinja template from a Python macro that can call the macro's sibling macros."""

from __future__ import annotations

import inspect
from collections.abc import Callable
from functools import lru_cache
from types import FrameType

try:
    from jinja2 import Environment, StrictUndefined, Template
except ImportError as error:
    raise ImportError(
        'sqlbuild.dbt requires Jinja. Install SQLBuild\'s dbt extra with: pip install "sqlbuild[dbt]"'
    ) from error

_ENVIRONMENT: Environment = Environment(
    autoescape=False, keep_trailing_newline=True, undefined=StrictUndefined
)
_COMPILED_TEMPLATE: Callable[[str], Template] = lru_cache(maxsize=1024)(_ENVIRONMENT.from_string)


def jinja(template: str, /, **values: object) -> str:
    """Render `template` with `values`; it calls the caller's other macros by plain name."""

    frame: FrameType | None = inspect.currentframe()
    caller: FrameType | None = frame.f_back if frame is not None else None
    try:
        scope: dict[str, object] = (
            {
                name: value
                for name, value in caller.f_globals.items()
                if not name.startswith("_") and inspect.isfunction(value)
            }
            if caller is not None
            else {}
        )
    finally:
        del frame, caller
    return _COMPILED_TEMPLATE(template).render({**scope, **values})
