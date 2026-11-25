"""SQLQuack's DuckDB conversion of Snowflake SQL, for scenarios replayed locally."""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from sqlbuild.executor.local._helpers.conversion import converted
from sqlbuild.executor.local._helpers.positions import (
    file_placed_refusal as file_placed_refusal,
)
from sqlbuild.executor.local._helpers.session import session_time_zone as session_time_zone
from sqlbuild.executor.local.models import ConvertedSql


def convert_local_sql(
    *, sql: str, schema: Mapping[str, Any] | None, time_zone: str | None, path: str
) -> ConvertedSql:
    """SQLQuack's conversion of `sql` in the target's time zone: DuckDB SQL, or its refusals."""

    return converted(sql=sql, schema=schema, time_zone=time_zone, path=path)
