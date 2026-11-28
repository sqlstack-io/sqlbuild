"""Render a dbt macro's Jinja template with exactly the values and macros passed to it."""

from __future__ import annotations

from collections.abc import Callable
from functools import lru_cache
from typing import NoReturn

from sqlbuild.integrations.dbt.exceptions import DbtMacroReturn


def jinja(template: str, /, **values: object) -> object:
    """Render `template` with only `values` and dbt's `return`; return the result."""

    try:
        return _template_render(template)({**values, "return": _return})
    except DbtMacroReturn as returned:
        return returned.value


@lru_cache(maxsize=1024)
def _template_render(template: str) -> Callable[..., str]:
    """Compile `template` as dbt does: `{% do return(x) %}`, `{% break %}`, names passed in."""

    try:
        from jinja2 import Template

        from sqlbuild.integrations.dbt.classes.dbt_name_undefined import DbtNameUndefined
    except ImportError as error:
        raise ImportError(
            "sqlbuild.dbt.jinja requires Jinja. Install SQLBuild's dbt extra with: "
            'pip install "sqlbuild[dbt]"'
        ) from error
    return Template(
        template,
        autoescape=False,
        extensions=["jinja2.ext.do", "jinja2.ext.loopcontrols"],
        keep_trailing_newline=True,
        undefined=DbtNameUndefined,
    ).render


def _return(value: object) -> NoReturn:
    raise DbtMacroReturn(value)
