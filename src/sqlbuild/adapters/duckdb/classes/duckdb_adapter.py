"""DuckDB adapter implementation."""

from __future__ import annotations

import base64
from typing import Any, ClassVar
from uuid import uuid4

from sqlbuild.adapter.contract.classes.base_adapter import _quote_sql_string
from sqlbuild.adapter.contract.classes.consumption import ConsumptionMixin
from sqlbuild.adapter.contract.classes.duckdb_backed_adapter import DuckDbBackedAdapter
from sqlbuild.adapter.contract.classes.microbatch import MicrobatchMixin
from sqlbuild.adapter.contract.types import BuiltinAdapter
from sqlbuild.microbatches.constants import MICROBATCH_GENERATION_COMMENT_PREFIX


class DuckDbAdapter(MicrobatchMixin, ConsumptionMixin, DuckDbBackedAdapter):
    """DuckDB adapter allowing implicit main for targetless and named local targets."""

    adapter_name: ClassVar[str] = BuiltinAdapter.DUCKDB.value
    allows_implicit_managed_write_schema: ClassVar[bool] = True

    def supports_concurrent_microbatch_dml(self) -> bool:
        return True

    def physical_relation_generation(
        self,
        *,
        connection: Any,
        database: str | None,
        schema: str | None,
        name: str,
    ) -> str | None:
        del database
        row: tuple[object, ...] | None = connection.execute(
            "SELECT comment FROM duckdb_tables() "
            "WHERE (? IS NULL OR schema_name = ?) AND table_name = ?",
            [schema, schema, name],
        ).fetchone()
        if row is None:
            return None
        comment: str = "" if row[0] is None else str(row[0])
        if comment.startswith(MICROBATCH_GENERATION_COMMENT_PREFIX):
            return comment.removeprefix(MICROBATCH_GENERATION_COMMENT_PREFIX).partition(":")[0]
        generation: str = uuid4().hex
        encoded_comment: str = base64.urlsafe_b64encode(comment.encode()).decode()
        qualified_name: str = (
            self.render_qualified_name(
                database=None,
                schema=schema,
                name=name,
            )
            or name
        )
        connection.execute(
            f"COMMENT ON TABLE {qualified_name} IS "
            + _quote_sql_string(
                f"{MICROBATCH_GENERATION_COMMENT_PREFIX}{generation}:{encoded_comment}"
            )
        )
        return generation

    def render_create_microbatch_state_index_sqls(
        self, *, database: str | None, schema: str
    ) -> tuple[str, ...]:
        from sqlbuild._native import build_microbatch_create_index_sqls

        return build_microbatch_create_index_sqls(
            database=database,
            schema=schema,
            render_identifier=self.render_identifier,
            render_qualified_name=self.render_qualified_name,
        )
