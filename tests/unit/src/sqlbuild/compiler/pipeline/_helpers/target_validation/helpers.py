"""Helpers for target validation unit tests."""

from __future__ import annotations

from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView


class ConservativeCustomAdapter(BaseAdapter):
    """Custom adapter retaining the conservative schema capability default."""

    adapter_name: ClassVar[str] = "custom"

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        return None

    def close(self, connection: Any) -> None:
        del connection

    def _execute(self, connection: Any, sql: str) -> Any:
        del connection, sql
        return None


def build_project(
    *,
    target: CompiledRelationLocation,
    effective_connection: dict[str, object],
    effective_target_schema: str | None = None,
) -> CompiledProjectView:
    """Build a minimal compiled project for target validation tests."""

    return CompiledProject(
        run_id="run_123",
        effective_target_name=None,
        effective_connection=effective_connection,
        effective_target_schema=effective_target_schema,
        effective_vars={},
        models=(
            CompiledModel(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.MODEL, name="stg_customers"
                ),
                deps=(),
                name="stg_customers",
                relative_path=Path("models/staging/stg_customers.sql"),
                query_sql="SELECT 1",
                config=CompileModelConfig(values={}),
                destination=target,
            ),
        ),
    )
