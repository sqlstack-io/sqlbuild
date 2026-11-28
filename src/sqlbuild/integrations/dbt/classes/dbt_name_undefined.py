"""The Jinja undefined value of a dbt macro template."""

from __future__ import annotations

from collections.abc import Iterator

from jinja2 import Undefined
from jinja2.utils import missing


class DbtNameUndefined(Undefined):
    """A name nothing passed in fails the render; a missing attribute or item renders empty."""

    __slots__ = ()

    def __str__(self) -> str:
        self._fail_for_name()
        return super().__str__()

    def __iter__(self) -> Iterator[object]:
        self._fail_for_name()
        return super().__iter__()

    def __bool__(self) -> bool:
        self._fail_for_name()
        return super().__bool__()

    def __len__(self) -> int:
        self._fail_for_name()
        return super().__len__()

    def _fail_for_name(self) -> None:
        if self._undefined_obj is missing:
            self._fail_with_undefined_error()
