"""The in-memory DuckDB adapter that runs a project's local unit tests."""

from __future__ import annotations

from typing import Any

from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.executor.local._helpers.schema import project_schema
from sqlbuild.executor.local._helpers.session import session_time_zone
from sqlbuild.executor.local.classes.local_test_adapter import LocalTestAdapter
from sqlbuild.executor.local.constants import CONVERTED_DIALECT


def local_test_adapter(*, project: Any, adapter: BaseAdapter) -> LocalTestAdapter:
    """The in-memory DuckDB adapter that runs the project's local unit tests."""

    dialect: str | None = adapter.sql_analysis_dialect()
    return LocalTestAdapter(
        project_adapter=adapter,
        schema=project_schema(project) if dialect == CONVERTED_DIALECT else None,
        time_zone=session_time_zone(project),
    )
