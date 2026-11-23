from __future__ import annotations

import io
from collections.abc import Callable
from pathlib import Path
from typing import Any

from sqlbuild._native import (
    AuditPlanEntry,
    ChainStep,
    CompiledFunction,
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    FunctionArgument,
    FunctionPlanEntry,
    MaterializationType,
    ModelPlanEntry,
    PlanAction,
    PlanOutput,
    PlanReason,
    PreparedCompileArtifacts,
    ProjectGraph,
    SeedCsvSettings,
    SeedPlanEntry,
    SqlTestPlanEntry,
    WrittenTarget,
    project_with,
)
from sqlbuild.adapter.contract.models import LifeCycleEvent, QueryResult
from sqlbuild.adapter.contract.types import LifeCycleEventKind
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.auditing.types import (
    AuditAttachmentKind,
    AuditRunScope,
    AuditSeverity,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
    FunctionLanguage,
)
from sqlbuild.executor.build.models import (
    BuildExecutionResult,
    FunctionExecutionResult,
)
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.run.models import ModelExecutionResult
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.unit.src.sqlbuild.cli.commands.main.dag.helpers import (
    prepare_python_dag_project,
)
from tests.unit.src.sqlbuild.compiler.graph.helpers import project_graph_from_indexes
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly._test_types import (
    PlanTestChainTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly.helpers import (
    build_test_and_project,
)


class TtyStdout(io.StringIO):
    """A captured stdout that reports a terminal."""

    def isatty(self) -> bool:
        return True


class NoConnectDuckDbAdapter(DuckDbAdapter):
    """DuckDB adapter test double that fails if compile opens a connection."""

    def connect(self, config: dict[str, Any]) -> Any:
        del config
        raise AssertionError("compile should not connect")

    def _execute(self, connection: Any, sql: str) -> Any:
        del connection, sql
        raise AssertionError("compile should not execute SQL")

    def query(self, connection: Any, sql: str, *, limit: int | None) -> QueryResult:
        del connection, sql, limit
        raise AssertionError("compile should not query")

    def close(self, connection: Any) -> None:
        del connection
        raise AssertionError("compile should not close a connection")


def prepare_static_compile_project(root: Path) -> Path:
    """Create a minimal local project for offline compile command tests."""

    project_dir: Path = root / "project"
    models_dir: Path = project_dir / "sql" / "models"
    models_dir.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "offline_compile"',
                'adapter = "duckdb"',
                'default_target = "dev"',
                "",
                "[targets.dev]",
                'schema = "main"',
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (models_dir / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view);\n\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    return project_dir


def prepare_python_compile_project(root: Path) -> Path:
    """Create a local project with Python nodes for compile DAG tests.

    Its loader depends on a task that reads SQL, so the project allows warehouse round trips.
    """

    project_dir: Path = prepare_python_dag_project(root)
    (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "offline_compile"',
                'adapter = "duckdb"',
                'default_target = "dev"',
                "",
                "[targets.dev]",
                'schema = "main"',
                "",
                "[settings]",
                "allow_warehouse_round_trips = true",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return project_dir


def build_target_writer_plan_output() -> PlanOutput:
    """Build a plan output covering every target writer artifact type."""

    model_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    seed_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SEED,
        name="country_codes",
    )
    audit_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.AUDIT,
        name="not_null",
    )
    test_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.SQL_TEST,
        name="orders_chain",
    )
    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF,
        name="is_completed_order",
    )
    python_function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF,
        name="is_completed_order_py",
    )
    target: CompiledRelationLocation = CompiledRelationLocation(
        database=None,
        schema="analytics",
        name="orders",
        qualified_name="analytics.orders",
    )
    return PlanOutput(
        model_entries=(
            ModelPlanEntry(
                key=model_key,
                name="orders",
                relative_path=Path("staging/orders.sql"),
                materialization_type=MaterializationType.TABLE,
                action=PlanAction.CREATE_TABLE,
                reason=PlanReason.FIRST_RUN,
                destination=target,
                fingerprint_query_sql="SELECT 1 AS order_id",
                resolved_sql="SELECT 1 AS order_id",
                logical_ddl="CREATE TABLE analytics.orders AS SELECT 1 AS order_id",
            ),
        ),
        seed_entries=(
            SeedPlanEntry(
                key=seed_key,
                name="country_codes",
                destination=target,
                file_path=Path("sql/seeds/country_codes.csv"),
                columns=(),
                csv_settings=SeedCsvSettings(),
            ),
        ),
        function_entries=(
            FunctionPlanEntry(
                key=function_key,
                name="is_completed_order",
                relative_path=Path("sql/functions/sql/is_completed_order.sql"),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order",
                    qualified_name="analytics.is_completed_order",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order",
                    qualified_name="analytics.is_completed_order",
                ),
                arguments=(FunctionArgument(name="order_status", type="VARCHAR"),),
                returns="BOOLEAN",
                body_sql="order_status = 'completed'",
                fingerprint_query_sql="order_status = 'completed'",
            ),
            FunctionPlanEntry(
                key=python_function_key,
                name="is_completed_order_py",
                relative_path=Path("sql/functions/python/is_completed_order_py.py"),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order_py",
                    qualified_name="analytics.is_completed_order_py",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order_py",
                    qualified_name="analytics.is_completed_order_py",
                ),
                arguments=(FunctionArgument(name="order_status", type="VARCHAR"),),
                returns="BOOLEAN",
                body_sql=(
                    "def main(order_status: str | None) -> bool:\n"
                    "    return order_status == 'completed'"
                ),
                fingerprint_query_sql=(
                    "def main(order_status: str | None) -> bool:\n"
                    "    return order_status == 'completed'"
                ),
                language=FunctionLanguage.PYTHON,
                entry_point="main",
            ),
        ),
        audit_entries=(
            AuditPlanEntry(
                key=audit_key,
                name="not_null",
                definition_name="test_audit",
                resolved_sql="SELECT order_id FROM analytics.orders WHERE order_id IS NULL",
                unresolved_sql='SELECT order_id FROM __ref("orders") WHERE order_id IS NULL',
                attachment_kind=AuditAttachmentKind.MODEL,
                severity=AuditSeverity.WARN,
                requested_run_scope=AuditRunScope.FINAL,
                effective_run_scope=AuditRunScope.FINAL,
                attached_target_name="orders",
                attached_column_name="order_id",
            ),
        ),
        test_entries=(
            SqlTestPlanEntry(
                key=test_key,
                name="orders_chain",
                chain=(
                    ChainStep(
                        model_name="stg_orders",
                        resolved_sql="SELECT 1 AS order_id",
                        expected_cte_sql="SELECT 1 AS order_id",
                    ),
                    ChainStep(
                        model_name="orders",
                        resolved_sql="SELECT order_id FROM stg_orders",
                        expected_cte_sql="SELECT 1 AS order_id",
                    ),
                ),
            ),
        ),
    )


def read_target_files(target_dir: Path, expected_files: dict[str, str]) -> dict[str, str]:
    """Read expected target files into a comparable mapping."""

    return {
        relative_path: (target_dir / relative_path).read_text(encoding="utf-8")
        for relative_path in expected_files
    }


def build_static_target_writer_project() -> CompiledProjectView:
    """Build compiled project state for offline compile target writer tests."""

    model_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.MODEL,
        name="orders",
    )
    function_key: CompiledObjectKey = CompiledObjectKey(
        resource_type=CompiledResourceType.UDF,
        name="is_completed_order",
    )
    return CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        models=(
            CompiledModel(
                key=model_key,
                deps=(),
                name="orders",
                relative_path=Path("staging/orders.sql"),
                query_sql="SELECT 2 AS order_id",
                config=CompileModelConfig(values={}),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="orders",
                    qualified_name="analytics.orders",
                ),
            ),
        ),
        functions=(
            CompiledFunction(
                key=function_key,
                deps=(),
                name="is_completed_order",
                relative_path=Path("sql/functions/sql/is_completed_order.sql"),
                arguments=(FunctionArgument(name="order_status", type="VARCHAR"),),
                returns="BOOLEAN",
                body_sql="order_status = 'completed'",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order",
                    qualified_name="analytics.is_completed_order",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name="is_completed_order",
                    qualified_name="analytics.is_completed_order",
                ),
            ),
        ),
    )


def build_cached_target_writer_project(*, target_dir: Path) -> CompiledProjectView:
    """Build a static project containing one cacheable model SQL test."""

    compiled_test, project = build_test_and_project(
        PlanTestChainTestCase(
            description="cached target writer project",
            model_queries={"orders": "SELECT 2 AS order_id"},
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_cte_bodies={"orders": "SELECT 2 AS order_id"},
        )
    )
    return project_with(
        project=project,
        compile_cache_dir=target_dir / "cache" / "compiler",
        sql_tests=(compiled_test,),
    )


def build_missing_mock_target_writer_project(*, target_dir: Path) -> CompiledProjectView:
    """Build a static cached project whose SQL test omits a required source mock."""

    compiled_test, project = build_test_and_project(
        PlanTestChainTestCase(
            description="missing mock target writer project",
            model_queries={"orders": 'SELECT order_id FROM __source("raw_orders")'},
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_cte_bodies={"orders": "SELECT 2 AS order_id"},
        )
    )
    return project_with(
        project=project,
        compile_cache_dir=target_dir / "cache" / "compiler",
        sql_tests=(compiled_test,),
    )


def build_compile_output_graph(*, model_names: tuple[str, ...]) -> ProjectGraph:
    """Build a static project graph for compile output formatter tests."""

    models: list[CompiledModelView] = []
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {}
    all_keys: dict[str, CompiledObjectKey] = {}
    name: str
    for name in model_names:
        key: CompiledObjectKey = CompiledObjectKey(
            resource_type=CompiledResourceType.MODEL,
            name=name,
        )
        models.append(
            CompiledModel(
                key=key,
                deps=(),
                name=name,
                relative_path=Path(f"sql/models/{name}.sql"),
                query_sql="SELECT 1 AS id",
                config=CompileModelConfig(values={}),
                destination=CompiledRelationLocation(
                    database=None,
                    schema="analytics",
                    name=name,
                    qualified_name=f"analytics.{name}",
                ),
                inferred_columns=(),
            )
        )
        upstream_deps[key] = ()
        all_keys[name] = key

    project: CompiledProjectView = CompiledProject(
        run_id="run-1",
        effective_target_name="dev",
        effective_connection={},
        effective_vars={},
        models=tuple(models),
    )
    return project_graph_from_indexes(
        project=project,
        upstream_deps=upstream_deps,
        downstream_deps={key: () for key in upstream_deps},
        tag_index={},
        path_index={key: "" for key in upstream_deps},
        all_keys=all_keys,
    )


def build_linear_compile_output_graph(*, model_count: int) -> ProjectGraph:
    """Build a static project graph with one model depending on the previous model."""

    graph: ProjectGraph = build_compile_output_graph(
        model_names=tuple(f"model_{index:05d}" for index in range(model_count))
    )
    upstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {}
    downstream_deps: dict[CompiledObjectKey, tuple[CompiledObjectKey, ...]] = {
        model.key: () for model in graph.project.models
    }
    first_model: CompiledModelView = graph.project.models[0]
    upstream_deps[first_model.key] = ()
    for previous_model, model in zip(graph.project.models, graph.project.models[1:], strict=False):
        upstream_deps[model.key] = (previous_model.key,)
        downstream_deps[previous_model.key] = (
            *downstream_deps[previous_model.key],
            model.key,
        )
    return project_graph_from_indexes(
        project=graph.project,
        upstream_deps=upstream_deps,
        downstream_deps=downstream_deps,
        tag_index=graph.tag_index,
        path_index=graph.path_index,
        all_keys=graph.all_keys,
    )


def build_compile_output_model_names(model_count: int) -> tuple[str, ...]:
    """Build model names for compile output formatter cases."""

    special_names: tuple[str, ...] = (
        "short",
        "hourly_activity_with_daily_context",
        "extremely_long_model_name_that_should_be_truncated_in_human_output",
    )
    generated_names: tuple[str, ...] = tuple(f"model_{index:03d}" for index in range(model_count))
    return {3: special_names}.get(model_count, generated_names)


def build_runtime_target_execution_result() -> BuildExecutionResult:
    return BuildExecutionResult(
        status=BuildStatus.SUCCESS,
        model_results=(
            ModelExecutionResult(
                model_name="orders",
                status=ExecutionStatus.SUCCESS,
                lifecycle_events=(
                    LifeCycleEvent(
                        kind=LifeCycleEventKind.SQL,
                        content="DROP TABLE IF EXISTS analytics.orders__staging",
                    ),
                    LifeCycleEvent(
                        kind=LifeCycleEventKind.LOG,
                        content="building partition 2024-01-01",
                    ),
                    LifeCycleEvent(
                        kind=LifeCycleEventKind.SQL,
                        content="CREATE OR REPLACE TABLE analytics.orders__staging "
                        "AS SELECT 1 AS order_id",
                    ),
                ),
            ),
        ),
        function_results=(
            FunctionExecutionResult(
                function_name="is_completed_order",
                status=ExecutionStatus.SUCCESS,
                function_kind="udf",
                lifecycle_events=(
                    LifeCycleEvent(
                        kind=LifeCycleEventKind.SQL,
                        content=(
                            "CREATE OR REPLACE FUNCTION analytics.is_completed_order"
                            "(order_status VARCHAR) RETURNS BOOLEAN"
                        ),
                    ),
                ),
            ),
            FunctionExecutionResult(
                function_name="is_completed_order_py",
                status=ExecutionStatus.SUCCESS,
                function_kind="udf",
                lifecycle_events=(
                    LifeCycleEvent(
                        kind=LifeCycleEventKind.SQL,
                        content=(
                            "REGISTER PYTHON FUNCTION analytics.is_completed_order_py"
                            "(VARCHAR) RETURNS BOOLEAN"
                        ),
                    ),
                ),
            ),
        ),
        success_count=1,
    )


def prepare_rule_gated_compile_project(root: Path, *, test_header: str) -> Path:
    """Create a project with a rule error and a SQL test that has a planning error."""

    project_dir: Path = prepare_static_compile_project(root)
    with (project_dir / "sqlbuild_project.toml").open("a", encoding="utf-8") as config:
        config.write('\n[rules]\nselect = ["SQBRMODEL102"]\n')
    (project_dir / "sql" / "sources").mkdir()
    (project_dir / "sql" / "sources" / "raw.yml").write_text(
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n    table: raw_orders\n"
        "  - name: raw_refunds\n    description: Test source raw_refunds.\n"
        "    schema: main\n    table: raw_refunds\n",
        encoding="utf-8",
    )
    (project_dir / "sql" / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view);\n\n"
        'SELECT o.order_id, r.refund_id FROM __source("raw_orders") AS o\n'
        'LEFT JOIN __source("raw_refunds") AS r ON r.order_id = o.order_id\n',
        encoding="utf-8",
    )
    (project_dir / "sql" / "models" / "bad_star.sql").write_text(
        'MODEL (description "Test model bad_star.", '
        'materialized view);\n\nSELECT * FROM __source("raw_orders")\n',
        encoding="utf-8",
    )
    (project_dir / "tests" / "unit").mkdir(parents=True)
    (project_dir / "tests" / "unit" / "orders_case.sql").write_text(
        f"TEST ({test_header});\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT 1 AS order_id)\n"
        "SELECT 1\n",
        encoding="utf-8",
    )
    return project_dir


def write_relative_files(*, root: Path, files: dict[str, str]) -> None:
    """Write each relative path under root with its text contents."""

    for relative, contents in files.items():
        path: Path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


class StagingEveryProject(PreparedCompileArtifacts):
    """Stage artifacts for a project of any size."""

    min_prepared_models: int = 1


def staged_then_published(
    *, target: Path, before_publish: Callable[[Path], None] = lambda _: None
) -> WrittenTarget | None:
    """Stage the project's artifacts beside target, run before_publish, then publish them."""

    with StagingEveryProject(enabled=True) as artifacts:
        artifacts.start(
            project=build_static_target_writer_project(),
            adapter=DuckDbAdapter(),
            target_dir=target,
            stage_artifacts=True,
        )
        _ = artifacts.planning_diagnostics()
        before_publish(target)
        return artifacts.publish(target_dir=target, manifest=None)


class FunctionRenderFailingAdapter(DuckDbAdapter):
    """A DuckDB adapter whose function rendering fails after the model group is handed off."""

    def render_create_function(
        self,
        *,
        destination: str,
        arguments: tuple[Any, ...],
        returns: str,
        body_sql: str,
        return_columns: tuple[Any, ...] = (),
        language: FunctionLanguage = FunctionLanguage.SQL,
        runtime_version: str | None = None,
        entry_point: str | None = None,
        packages: tuple[str, ...] = (),
        dataframe: str | None = None,
    ) -> tuple[str, ...]:
        raise ValueError(f"function rendering failed for {destination}")


class FunctionRenderInterruptedAdapter(DuckDbAdapter):
    """A DuckDB adapter interrupted while rendering a function after the model group."""

    def render_create_function(
        self,
        *,
        destination: str,
        arguments: tuple[Any, ...],
        returns: str,
        body_sql: str,
        return_columns: tuple[Any, ...] = (),
        language: FunctionLanguage = FunctionLanguage.SQL,
        runtime_version: str | None = None,
        entry_point: str | None = None,
        packages: tuple[str, ...] = (),
        dataframe: str | None = None,
    ) -> tuple[str, ...]:
        raise KeyboardInterrupt(destination)


class BlockedModelStaging(StagingEveryProject):
    """Stage artifacts into a staging tree whose model directory is a file."""

    def create_staging_directory(self, staging_dir: Path) -> None:
        super().create_staging_directory(staging_dir)
        blocker: Path = staging_dir / "compiled" / "sql" / "models" / "staging"
        blocker.parent.mkdir(parents=True)
        _ = blocker.write_text("not a directory\n", encoding="utf-8")
