from __future__ import annotations

import os
import time
from dataclasses import replace
from pathlib import Path

import pytest

import sqlbuild._native as _native
import sqlbuild._native as plan_artifacts_module
from sqlbuild._native import (
    ChainStep,
    CompiledResourceType,
    CompiledSqlTest,
    DiagnosticPhase,
    PlanOutput,
    SqlTestPlanEntry,
    WrittenTarget,
    build_manifest,
    compiled_model_record,
    project_with,
    render_sql_test_comparison_sql,
    write_compile_target,
    write_static_compile_target,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)
from tests.unit.src.sqlbuild.cli.commands.main.compile._test_types import (
    ExpectedMessageTestCase,
    TargetWriterCacheTestCase,
    TargetWriterPlanningErrorTestCase,
    TargetWriterTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.compile.helpers import (
    BlockedModelStaging,
    FunctionRenderFailingAdapter,
    FunctionRenderInterruptedAdapter,
    build_cached_target_writer_project,
    build_missing_mock_target_writer_project,
    build_static_target_writer_project,
    build_target_writer_plan_output,
    read_target_files,
)
from tests.unit.src.sqlbuild.compiler.manifest.helpers import build_test_project

SQL_TEST_ARTIFACT_PLANS: str = "plan_compiled_sql_test_artifacts"
INTERRUPT_RETURN_SECONDS: float = 10.0
SQL_TEST_MODEL_CHAINS: str = "sql_test_model_chain_names_by_key"


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWriterTestCase(
            description="writes compiled SQL, manifest, audits, and chain tests",
            expected_files={
                "compiled/sql/models/staging/orders.sql": "SELECT 1 AS order_id\n",
                "compiled/sql/functions/sql/is_completed_order.sql": (
                    "CREATE OR REPLACE MACRO analytics.is_completed_order(order_status) AS (\n"
                    "order_status = 'completed'\n"
                    ")\n"
                ),
                "compiled/sql/functions/python/is_completed_order_py.sql": (
                    "REGISTER PYTHON FUNCTION analytics.is_completed_order_py"
                    "(VARCHAR) RETURNS BOOLEAN\n"
                ),
                "compiled/audits/generic/orders/not_null__order_id.sql": (
                    "SELECT order_id FROM analytics.orders WHERE order_id IS NULL\n"
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_plan_output_when_writing_target_then_expected_files_are_written(
    test_case: TargetWriterTestCase,
    tmp_path: Path,
) -> None:
    plan_output: PlanOutput = build_target_writer_plan_output()
    expected_test_sql: str = (
        render_sql_test_comparison_sql((plan_output.test_entries[0],))[0] + "\n"
    )
    manifest: _native.NativeManifest = build_manifest(
        project=build_test_project(),
        project_name="demo",
        adapter_type="duckdb",
        upstream_deps={},
        downstream_deps={},
    )
    manifest_contents: bytes = manifest.contents()

    expected_files: dict[str, str] = dict(test_case.expected_files)
    expected_files["compiled/tests/_chain_/orders__stg_orders/orders_chain.sql"] = expected_test_sql

    write_compile_target(
        target_dir=tmp_path / "target",
        adapter=DuckDbAdapter(),
        plan_output=plan_output,
        manifest=manifest,
    )
    manifest_path: Path = tmp_path / "target" / "manifest.json"
    unchanged_mtime_ns: int = 1_000_000_000
    os.utime(manifest_path, ns=(unchanged_mtime_ns, unchanged_mtime_ns))
    _ = write_compile_target(
        target_dir=tmp_path / "target",
        adapter=DuckDbAdapter(),
        plan_output=plan_output,
        manifest=manifest,
    )

    assert read_target_files(tmp_path / "target", expected_files) == expected_files
    assert manifest_path.read_bytes() == manifest_contents
    assert manifest_path.stat().st_mtime_ns == unchanged_mtime_ns
    assert not (tmp_path / "target" / "run").exists()


@pytest.mark.parametrize(
    "test_case",
    (
        TargetWriterTestCase(
            description="long chain uses a bounded deterministic artifact directory",
            model_count=20,
            expected_max_component_bytes=200,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_long_test_chain_when_writing_target_then_artifact_component_is_bounded(
    test_case: TargetWriterTestCase,
    tmp_path: Path,
) -> None:
    plan_output: PlanOutput = build_target_writer_plan_output()
    template: SqlTestPlanEntry = plan_output.test_entries[0]
    long_entry: SqlTestPlanEntry = replace(
        template,
        chain=tuple(
            ChainStep(
                model_name=f"model_{index:02d}_{'x' * 40}",
                resolved_sql="SELECT 1 AS value",
            )
            for index in range(test_case.model_count)
        ),
    )

    _ = write_compile_target(
        target_dir=tmp_path / "target",
        adapter=DuckDbAdapter(),
        plan_output=replace(plan_output, test_entries=(long_entry,)),
    )

    artifact: Path = next((tmp_path / "target" / "compiled" / "tests").rglob("*.sql"))
    assert len(artifact.parent.name.encode()) <= test_case.expected_max_component_bytes
    assert artifact.parent.name.startswith("model_00_")
    assert f"model_{test_case.model_count - 1:02d}_" in artifact.parent.name


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWriterTestCase(
            description="writes SQL test cases to source and block scoped paths",
            expected_files={
                "compiled/tests/tests/unit/orders/block_1__small.sql": "",
                "compiled/tests/tests/unit/orders/block_1__large.sql": "",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_parameterized_test_entries_when_writing_target_then_case_artifacts_do_not_collide(
    test_case: TargetWriterTestCase,
    tmp_path: Path,
) -> None:
    plan_output: PlanOutput = build_target_writer_plan_output()
    template: SqlTestPlanEntry = plan_output.test_entries[0]
    case_entries: tuple[SqlTestPlanEntry, ...] = tuple(
        replace(
            template,
            key=replace(template.key, name=f"orders [{case_name}]"),
            name=f"orders [{case_name}]",
            source_path=Path("tests/unit/orders.sql"),
            block_index=1,
            parent_name="orders",
            case_name=case_name,
        )
        for case_name in ("small", "large")
    )
    parameterized_plan: PlanOutput = replace(plan_output, test_entries=case_entries)

    write_compile_target(
        target_dir=tmp_path / "target",
        adapter=DuckDbAdapter(),
        plan_output=parameterized_plan,
    )

    expected_path: str
    for expected_path in test_case.expected_files:
        assert (tmp_path / "target" / expected_path).is_file()


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWriterTestCase(
            description="writes offline compiled SQL without manifest by default",
            expected_files={
                "compiled/sql/models/staging/orders.sql": "SELECT 2 AS order_id\n",
                "compiled/sql/functions/sql/is_completed_order.sql": (
                    "CREATE OR REPLACE MACRO analytics.is_completed_order(order_status) AS (\n"
                    "order_status = 'completed'\n"
                    ")\n"
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_writing_static_target_then_expected_files_are_written(
    test_case: TargetWriterTestCase,
    tmp_path: Path,
) -> None:
    project: CompiledProjectView = build_static_target_writer_project()
    target_dir: Path = tmp_path / "target"
    stale_path: Path = target_dir / "compiled" / "sql" / "models" / "deleted.sql"
    stale_path.parent.mkdir(parents=True)
    stale_path.write_text("SELECT 'stale'\n", encoding="utf-8")

    write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    model_path: Path = target_dir / "compiled" / "sql" / "models" / "staging" / "orders.sql"
    unchanged_mtime_ns: int = 1_000_000_000
    os.utime(model_path, ns=(unchanged_mtime_ns, unchanged_mtime_ns))
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert read_target_files(target_dir, test_case.expected_files) == test_case.expected_files
    assert model_path.stat().st_mtime_ns == unchanged_mtime_ns
    assert not stale_path.exists()
    assert not (target_dir / "manifest.json").exists()

    changed_project: CompiledProjectView = project_with(
        project=project,
        models=(
            replace(compiled_model_record(project.models[0]), query_sql="SELECT 3 AS order_id"),
        ),
    )
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=changed_project,
    )

    assert model_path.read_text(encoding="utf-8") == "SELECT 3 AS order_id\n"
    assert model_path.stat().st_mtime_ns != unchanged_mtime_ns


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="unicode SQL bytes remain unchanged",
            expected_message="SELECT 'café' AS product_name\n",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unicode_sql_when_writing_static_target_twice_then_bytes_remain_unchanged(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    project: CompiledProjectView = build_static_target_writer_project()
    project = project_with(
        project=project,
        models=(
            replace(
                compiled_model_record(project.models[0]),
                query_sql="SELECT 'café' AS product_name  \n\n",
            ),
        ),
    )
    target_dir: Path = tmp_path / "target"

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    model_path: Path = target_dir / "compiled" / "sql" / "models" / "staging" / "orders.sql"
    unchanged_mtime_ns: int = 1_000_000_000
    os.utime(model_path, ns=(unchanged_mtime_ns, unchanged_mtime_ns))
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert model_path.read_text(encoding="utf-8") == test_case.expected_message
    assert model_path.stat().st_mtime_ns == unchanged_mtime_ns


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="invalid UTF-8 remains an error", expected_message="utf-8"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_utf8_artifact_when_writing_again_then_existing_decode_error_is_preserved(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    project: CompiledProjectView = build_static_target_writer_project()
    target_dir: Path = tmp_path / "target"
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    model_path: Path = target_dir / "compiled" / "sql" / "models" / "staging" / "orders.sql"
    model_path.write_bytes(b"\xff")

    with pytest.raises(UnicodeDecodeError, match=test_case.expected_message):
        write_static_compile_target(
            target_dir=target_dir,
            adapter=DuckDbAdapter(),
            project=project,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="blocked model directory stops later groups", expected_message="staging"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unwritable_model_artifact_when_writing_static_target_then_later_groups_are_not_written(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    project: CompiledProjectView = build_static_target_writer_project()
    target_dir: Path = tmp_path / "target"
    blocker: Path = target_dir / "compiled" / "sql" / "models" / "staging"
    blocker.parent.mkdir(parents=True)
    blocker.write_text("not a directory\n", encoding="utf-8")

    with pytest.raises(OSError, match=test_case.expected_message):
        write_static_compile_target(
            target_dir=target_dir,
            adapter=DuckDbAdapter(),
            project=project,
        )

    assert blocker.read_text(encoding="utf-8") == "not a directory\n"
    assert not (target_dir / "compiled" / "sql" / "functions").exists()
    assert not (target_dir / "compiled" / "sql" / "audits").exists()
    assert not (target_dir / "compiled" / "tests").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="the model write error wins over a later rendering error",
            expected_message="staging",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_write_failure_and_later_error_when_writing_static_target_then_os_error_wins(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    blocker: Path = target_dir / "compiled" / "sql" / "models" / "staging"
    blocker.parent.mkdir(parents=True)
    blocker.write_text("not a directory\n", encoding="utf-8")

    with pytest.raises(OSError, match=test_case.expected_message):
        write_static_compile_target(
            target_dir=target_dir,
            adapter=FunctionRenderFailingAdapter(),
            project=build_static_target_writer_project(),
        )

    assert not (target_dir / "compiled" / "sql" / "functions").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="interrupt after the model group", expected_message="compiled/sql/functions"
        )
    ],
    ids=lambda case: case.description,
)
def test_given_interrupt_after_model_group_when_writing_static_target_then_later_groups_are_not_written(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    started: float = time.monotonic()

    with pytest.raises(KeyboardInterrupt):
        write_static_compile_target(
            target_dir=target_dir,
            adapter=FunctionRenderInterruptedAdapter(),
            project=build_static_target_writer_project(),
        )

    assert time.monotonic() - started < INTERRUPT_RETURN_SECONDS
    assert not (target_dir / test_case.expected_message).exists()
    assert not (target_dir / "compiled" / "sql" / "audits").exists()
    assert not (target_dir / "compiled" / "tests").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedMessageTestCase(
            description="staged model write failure falls back despite a later error",
            expected_message="compiled/sql/models/staging/orders.sql",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_staged_model_write_failure_and_later_error_when_publishing_then_staging_falls_back(
    tmp_path: Path,
    test_case: ExpectedMessageTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"

    with BlockedModelStaging(enabled=True) as artifacts:
        artifacts.start(
            project=build_static_target_writer_project(),
            adapter=FunctionRenderFailingAdapter(),
            target_dir=target_dir,
            stage_artifacts=True,
        )
        written: WrittenTarget | None = artifacts.publish(target_dir=target_dir, manifest=None)

    assert written is None
    assert not (target_dir / test_case.expected_message).exists()


@pytest.mark.parametrize(
    "test_case",
    (TargetWriterCacheTestCase(description="unchanged artifact reuse", expected_builder_calls=0),),
    ids=lambda case: case.description,
)
def test_given_unchanged_test_artifact_when_writing_again_then_skips_test_plan_reconstruction(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    artifact_path: Path = next((target_dir / "compiled" / "tests").rglob("*.sql"))
    original_mtime_ns: int = artifact_path.stat().st_mtime_ns
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert artifact_path.stat().st_mtime_ns == original_mtime_ns
    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )


@pytest.mark.parametrize(
    "test_case",
    (
        TargetWriterCacheTestCase(
            description="identical bytes with a moved mtime", expected_builder_calls=0
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_artifact_with_moved_mtime_and_same_bytes_when_writing_then_reuses_it(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    artifact_path: Path = next((target_dir / "compiled" / "tests").rglob("*.sql"))
    moved_mtime_ns: int = artifact_path.stat().st_mtime_ns + 1_000_000_000
    os.utime(artifact_path, ns=(moved_mtime_ns, moved_mtime_ns))
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )
    assert artifact_path.stat().st_mtime_ns == moved_mtime_ns


@pytest.mark.parametrize(
    "test_case",
    (
        TargetWriterPlanningErrorTestCase(
            description="missing mock is reported on every write",
            expected_builder_calls=1,
            expected_message=(
                "model 'orders' references __source(\"raw_orders\") which has no mock"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_test_with_planning_error_when_writing_twice_then_error_is_not_cached(
    tmp_path: Path,
    test_case: TargetWriterPlanningErrorTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_missing_mock_target_writer_project(target_dir=target_dir)
    first: WrittenTarget = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    second: WrittenTarget = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )
    assert first.diagnostics == second.diagnostics
    (diagnostic,) = second.diagnostics
    assert diagnostic.is_error
    assert diagnostic.phase is DiagnosticPhase.TEST
    assert diagnostic.code == "S000"
    assert diagnostic.resource_type is CompiledResourceType.SQL_TEST
    assert diagnostic.resource_name == project.sql_tests[0].name
    assert test_case.expected_message in diagnostic.message


@pytest.mark.parametrize(
    "test_case",
    (
        TargetWriterCacheTestCase(
            description="model closure invalidation", expected_builder_calls=1
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_changed_model_in_test_closure_when_writing_then_rebuilds_test_artifact(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)
    changed_project: CompiledProjectView = project_with(
        project=project,
        models=(
            replace(compiled_model_record(project.models[0]), query_sql="SELECT 3 AS order_id"),
        ),
    )

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=changed_project,
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )


@pytest.mark.parametrize(
    "test_case",
    (TargetWriterCacheTestCase(description="unrelated model reuse", expected_builder_calls=0),),
    ids=lambda case: case.description,
)
def test_given_changed_unrelated_model_when_writing_then_reuses_test_artifact(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    unrelated_model: CompiledModelView = replace(
        compiled_model_record(project.models[0]),
        key=replace(project.models[0].key, name="unrelated"),
        name="unrelated",
        relative_path=Path("sql/models/unrelated.sql"),
        query_sql="SELECT 9 AS unrelated_id",
    )
    changed_project: CompiledProjectView = project_with(
        project=project,
        models=(*project.models, unrelated_model),
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=changed_project,
    )
    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )


@pytest.mark.parametrize(
    "test_case",
    (TargetWriterCacheTestCase(description="test SQL invalidation", expected_builder_calls=1),),
    ids=lambda case: case.description,
)
def test_given_changed_test_sql_when_writing_then_rebuilds_test_artifact(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)
    changed_test: CompiledSqlTest = replace(
        project.sql_tests[0], sql_body=project.sql_tests[0].sql_body + "\n-- edit"
    )

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project_with(project=project, sql_tests=(changed_test,)),
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )


@pytest.mark.parametrize(
    "test_case",
    (
        TargetWriterCacheTestCase(
            description="compile target invalidation", expected_builder_calls=1
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_changed_compile_target_when_writing_then_rebuilds_test_artifact(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project_with(project=project, effective_target_schema="changed_schema"),
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )


@pytest.mark.parametrize(
    "test_case",
    (TargetWriterCacheTestCase(description="disabled cache rebuild", expected_builder_calls=2),),
    ids=lambda case: case.description,
)
def test_given_compile_cache_disabled_when_writing_twice_then_rebuilds_test_artifact(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = project_with(
        project=build_cached_target_writer_project(target_dir=target_dir),
        compile_cache_dir=None,
    )
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    for _ in range(2):
        _ = write_static_compile_target(
            target_dir=target_dir,
            adapter=DuckDbAdapter(),
            project=project,
        )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )
    assert plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_MODEL_CHAINS, 0) == 0
    assert not (target_dir / "cache" / "compiler" / "sql-test-artifacts.json").exists()


@pytest.mark.parametrize(
    "test_case",
    (TargetWriterCacheTestCase(description="tampered artifact repair", expected_builder_calls=1),),
    ids=lambda case: case.description,
)
def test_given_modified_test_artifact_when_writing_then_rebuilds_expected_sql(
    tmp_path: Path,
    test_case: TargetWriterCacheTestCase,
) -> None:
    target_dir: Path = tmp_path / "target"
    project: CompiledProjectView = build_cached_target_writer_project(target_dir=target_dir)
    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )
    artifact_path: Path = next((target_dir / "compiled" / "tests").rglob("*.sql"))
    expected_sql: str = artifact_path.read_text(encoding="utf-8")
    artifact_path.write_text("SELECT 'tampered'\n", encoding="utf-8")
    _ = plan_artifacts_module.compile_stage_work_info(reset=True)

    _ = write_static_compile_target(
        target_dir=target_dir,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert (
        plan_artifacts_module.compile_stage_work_info().get(SQL_TEST_ARTIFACT_PLANS, 0)
        == test_case.expected_builder_calls
    )
    assert artifact_path.read_text(encoding="utf-8") == expected_sql
