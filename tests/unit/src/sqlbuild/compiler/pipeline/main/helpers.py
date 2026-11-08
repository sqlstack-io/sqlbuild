from __future__ import annotations

from pathlib import Path
from typing import Any, ClassVar

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSource,
    DiscoveredSourceFile,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    SourceEntry,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView


class RelationTargetTestAdapter(BaseAdapter):
    adapter_name: ClassVar[str] = "relation-target-test"

    def connect(self, config: dict[str, object]) -> object:
        del config
        return object()

    def close(self, connection: object) -> None:
        del connection

    def _execute(self, connection: Any, sql: str) -> object:
        del connection
        return sql


def relation_target_python_node() -> None:
    """No-op Python declaration used by relation target graph tests."""


def build_relation_target_project() -> CompiledProjectView:
    source_entry: SourceEntry = SourceEntry(name="orders", schema="load_raw", table="orders")
    source_file: DiscoveredSourceFile = DiscoveredSourceFile(
        file_path=Path("sources/raw.yml"),
        relative_path=Path("sources/raw.yml"),
        contents="",
        source_entries=(source_entry,),
    )
    return CompiledProject(
        run_id="test_run",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        sources=(
            CompiledSource(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.SOURCE,
                    name="orders",
                ),
                deps=(),
                name="orders",
                source_entry=source_entry,
                source_file=source_file,
            ),
        ),
    )


def build_plan_output_with_model(name: str = "orders") -> PlanOutput:
    entry: ModelPlanEntry = ModelPlanEntry(
        key=CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name=name),
        name=name,
        relative_path=Path(f"models/{name}.sql"),
        materialization_type=MaterializationType.TABLE,
        action=PlanAction.CREATE_TABLE,
        reason=PlanReason.FIRST_RUN,
        destination=CompiledRelationLocation(
            database=None, schema="main", name=name, qualified_name=f"main.{name}"
        ),
        fingerprint_query_sql="SELECT 1",
        resolved_sql="SELECT 1",
        logical_ddl="CREATE TABLE main.orders AS SELECT 1",
    )
    return PlanOutput(
        execution_order=(entry.key,),
        model_entries=(entry,),
        selected_keys=frozenset((entry.key,)),
    )


class ManualClock:
    """A monotonic clock that only moves when a test advances it."""

    def __init__(self) -> None:
        self.now: float = 0.0

    def __call__(self) -> float:
        return self.now

    def advance(self, seconds: float) -> None:
        self.now += seconds


class SlowConnectionDuckDbAdapter(DuckDbAdapter):
    """An in-memory DuckDB adapter whose connections, compile profile and rules dialect take
    `connection_seconds`, `compile_seconds` and `rules_seconds` on a manual clock.

    The compile profile takes its time on its first call; the dialect on its third, which the
    rules gate makes after the compile's two dialect reads (its adapter context and its profile)."""

    def __init__(
        self,
        *,
        clock: ManualClock,
        connection_seconds: float,
        compile_seconds: float = 0.0,
        rules_seconds: float = 0.0,
    ) -> None:
        super().__init__()
        self._clock: ManualClock = clock
        self._connection_seconds: float = connection_seconds
        self._compile_seconds: float = compile_seconds
        self._dialect_seconds: list[float] = [0.0, 0.0, rules_seconds]

    def expression_inference_profile(self) -> ExpressionInferenceProfile:
        self._clock.advance(self._compile_seconds)
        self._compile_seconds = 0.0
        return super().expression_inference_profile()

    def sql_analysis_dialect(self) -> str | None:
        self._clock.advance(sum(self._dialect_seconds[:1]))
        del self._dialect_seconds[:1]
        return super().sql_analysis_dialect()

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        self._clock.advance(self._connection_seconds)
        return super().connect({})
