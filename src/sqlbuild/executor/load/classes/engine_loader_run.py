"""One `@loader` on SQLLoad's engine: what it loads, how the engine calls it, and its skip."""

from __future__ import annotations

import json
from collections.abc import Callable, Iterable, Iterator, Mapping
from typing import Any, cast

from sqlload_api import Context, Job, _host

from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.load.exceptions import LoaderSkippedError
from sqlbuild.executor.load.models import EngineLoaderInputs, LoaderResult, LoaderSkipResult


class EngineLoaderRun:
    """The run `run_sqlload_loader` loads, and the table function its Python host calls."""

    def __init__(
        self,
        *,
        inputs: EngineLoaderInputs,
        start: Callable[[Mapping[str, Any]], tuple[Context, object]],
    ) -> None:
        self.name: str = inputs.name
        self.project: str = inputs.project
        self.source: str = inputs.source
        self.reload: bool = inputs.reload
        self.start_cursor_ts: str | None = inputs.start_cursor_ts
        self.end_cursor_ts: str | None = inputs.end_cursor_ts
        self.start_cursor_int: str | None = inputs.start_cursor_int
        self.end_cursor_int: str | None = inputs.end_cursor_int
        self._start: Callable[[Mapping[str, Any]], tuple[Context, object]] = start
        self.context: Context | None = None
        self.skipped: LoaderSkipResult | None = None

    def table_items(self, request: str) -> Iterator[Any]:
        """The items the loader returns or yields for the engine's read `request`."""

        context, returned = self._start(json.loads(request))
        self.context = context
        if isinstance(returned, LoaderSkipResult):
            self.skipped = returned
            raise LoaderSkippedError(returned.reason)
        if isinstance(returned, LoaderResult):
            raise ExecutorInputError(
                f"Loader '{self.name}' returned ctx.result(...); a loader with a write strategy "
                "returns or yields rows, or returns ctx.skip(...)"
            )
        if returned is None:
            raise ExecutorInputError(
                f"Loader '{self.name}' defines write_strategy but returned no rows; "
                "return an empty list to load none"
            )
        if isinstance(returned, Job):
            return iter([returned])
        return iter(cast(Iterable[Any], returned))

    def current_state(self) -> str | None:
        """The logged position the rows crossing now commit with."""

        return _host.current_state(self.context)

    def final_state(self) -> str | None:
        """The position the loader left in `ctx.state`, once it finished."""

        return _host.final_state(self.context)
