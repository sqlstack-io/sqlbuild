"""Render a dbt macro's Jinja template with exactly the values and macros passed to it."""

from __future__ import annotations

try:
    from jinja2 import Environment, Template
except ImportError as error:
    raise ImportError(
        "sqlbuild.dbt requires Jinja. Install SQLBuild's dbt extra with: "
        'pip install "sqlbuild[dbt]"'
    ) from error

from collections.abc import Callable
from functools import lru_cache
from typing import NoReturn

from sqlbuild.integrations.dbt.classes.dbt_name_undefined import DbtNameUndefined
from sqlbuild.integrations.dbt.exceptions import DbtMacroReturn

# dbt's templates rely on `{% do %}`, as in `{% do return(x) %}`, and on `{% break %}`.
_ENVIRONMENT: Environment = Environment(
    autoescape=False,
    extensions=["jinja2.ext.do", "jinja2.ext.loopcontrols"],
    keep_trailing_newline=True,
    undefined=DbtNameUndefined,
)
_COMPILED_TEMPLATE: Callable[[str], Template] = lru_cache(maxsize=1024)(_ENVIRONMENT.from_string)


def jinja(template: str, /, **values: object) -> object:
    """Render `template` seeing only `values` and dbt's `return`; return what it renders or returns.

    Pass every macro the template calls by name, as in `concat=concat`, so SQLBuild records the
    call as a dependency. A name not passed in fails the render; a missing attribute or item
    renders empty, as in dbt.
    """

    try:
        return _COMPILED_TEMPLATE(template).render({**values, "return": _return})
    except DbtMacroReturn as returned:
        return returned.value


def _return(value: object) -> NoReturn:
    raise DbtMacroReturn(value)
