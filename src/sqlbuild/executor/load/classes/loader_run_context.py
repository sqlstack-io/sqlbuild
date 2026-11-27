"""A `@loader`'s `ctx`: SQLLoad's table context with SQLBuild's warehouse, references, results."""

from __future__ import annotations

from typing import Any

from sqlload_api import Connection, Context, Http, http

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.constants import SQLBUILD_CONTEXT_MEMBERS
from sqlbuild.executor.load.models import LoaderContext


class LoaderRunContext(Context):
    """SQLLoad's `Context` with SQLBuild's members; `ctx.connection` stays the warehouse's."""

    def __init__(
        self, *, sqlbuild: LoaderContext, connection_name: str | None = None, **context: Any
    ) -> None:
        self._sqlbuild: LoaderContext = sqlbuild
        self._connection_name: str | None = connection_name
        super().__init__(**context)

    def http(self, *, headers: dict[str, str] | None = None) -> Http:
        """`sl.http` requests to the loader's connection; SQLLoad runs the jobs it returns."""

        if self._connection_name is None:
            raise ExecutorInputError(
                "ctx.http() needs @loader(connection=...) naming an http connection, and a "
                "write_strategy, so that SQLLoad runs the job the loader returns"
            )
        return http(connection=self._connection_name, headers=headers)

    @property
    def source(self) -> Any:
        """`ctx.source(name)`: a reference to the project source `name`."""

        return self._sqlbuild.source

    @source.setter
    def source(self, value: str) -> None:
        self.source_name: str = value

    @property
    def connection(self) -> Any:
        """The live warehouse connection."""

        return self._sqlbuild.connection

    @property
    def loader_connection(self) -> Connection:
        """The source connection the loader names (empty without one); call it for another."""

        return self._connection

    def __getattr__(self, name: str) -> Any:
        if name in SQLBUILD_CONTEXT_MEMBERS:
            return getattr(self._sqlbuild, name)
        raise AttributeError(f"'{type(self).__name__}' object has no attribute '{name}'")
