"""Convert a SQL project function's body for `sqb test --local`."""

from __future__ import annotations

from sqlbuild.executor.testing._helpers.local_converter import convert_function
from sqlbuild.executor.testing.constants import SQLQUACK_CONVERT_TIMEOUT_SECONDS
from sqlbuild.executor.testing.models import LocalTestEngine, SqlQuackFunction


def convert_project_function(
    *, engine: LocalTestEngine, name: str, schema: str, body: str
) -> SqlQuackFunction:
    """A SQL project function's body as a DuckDB macro body, or why SQLQuack refused it."""

    return convert_function(
        engine=engine,
        name=name,
        schema=schema,
        body=body,
        timeout_seconds=SQLQUACK_CONVERT_TIMEOUT_SECONDS,
    )
