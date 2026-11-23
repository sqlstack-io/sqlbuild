from __future__ import annotations

import subprocess
from collections.abc import Mapping
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.dbt._test_types import (
    DbtExecutionCliTestCase,
    DbtExecutionFailureCliTestCase,
    DbtExecutionQueryAssertion,
    DbtMissingOriginE2ETestCase,
    DbtTargetConcurrencyE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.dbt.helpers import (
    break_dbt_interop_fact_orders_model,
    declare_missing_origin_requiring_confirmation,
    declare_sqlbuild_main_target,
    load_json_stdout,
    prepare_dbt_interop_project,
    skip_unless_dbt_is_runnable,
    write_dbt_interop_manifest,
    write_sqlbuild_defer_target_models,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    query_duckdb,
    row_count,
    run_sqb,
    table_exists,
)

pytestmark: pytest.MarkDecorator = pytest.mark.dbt


@pytest.mark.parametrize(
    "test_case",
    [
        DbtExecutionCliTestCase(
            description="run executes mixed dbt and sqlbuild work",
            command=("dbt", "run", "--select", "+fact_orders+"),
            expected_row_counts=(
                ("stg_orders", 1),
                ("fact_orders", 1),
                ("downstream_orders", 1),
                ("event_time_orders", 1),
                ("mart_orders", 1),
            ),
            expected_stdout_fragments=(
                "Plan ready",
                "dbt execution",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="mixed run materializes downstream orders row",
                    sql="SELECT order_id FROM main.downstream_orders ORDER BY order_id",
                    expected_rows=((1,),),
                ),
                DbtExecutionQueryAssertion(
                    description="mixed run materializes mart orders row",
                    sql="SELECT order_id FROM main.mart_orders ORDER BY order_id",
                    expected_rows=((1,),),
                ),
            ),
        ),
        DbtExecutionCliTestCase(
            description="run executes dbt only work",
            command=("dbt", "run", "--select", "dbt_only"),
            expected_row_counts=(("dbt_only", 1),),
            unexpected_relations=("local_only",),
        ),
        DbtExecutionCliTestCase(
            description="run executes sqlbuild only work",
            command=("dbt", "run", "--select", "local_only"),
            expected_row_counts=(("local_only", 1),),
            unexpected_relations=("dbt_only",),
            expected_stdout_fragments=("Skipping dbt: no dbt work selected.",),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="sqlbuild only run materializes local row",
                    sql="SELECT order_id FROM main.local_only",
                    expected_rows=((10,),),
                ),
            ),
        ),
        DbtExecutionCliTestCase(
            description="build executes mixed dbt and sqlbuild work",
            command=("dbt", "build", "--select", "+fact_orders+"),
            expected_row_counts=(
                ("stg_orders", 1),
                ("fact_orders", 1),
                ("downstream_orders", 1),
                ("event_time_orders", 1),
                ("mart_orders", 1),
            ),
            expected_stdout_fragments=(
                "Plan ready",
                "dbt execution",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="mixed build materializes downstream orders row",
                    sql="SELECT order_id FROM main.downstream_orders ORDER BY order_id",
                    expected_rows=((1,),),
                ),
                DbtExecutionQueryAssertion(
                    description="mixed build materializes mart orders row",
                    sql="SELECT order_id FROM main.mart_orders ORDER BY order_id",
                    expected_rows=((1,),),
                ),
            ),
            expected_planned_sqlbuild_models=(
                "downstream_orders",
                "+event_time_orders",
                "mart_orders",
            ),
        ),
        DbtExecutionCliTestCase(
            description="build executes explicit upstream mixed tag selector",
            command=("dbt", "build", "--select", "+tag:nightly"),
            expected_row_counts=(
                ("stg_orders", 1),
                ("fact_orders", 1),
                ("downstream_orders", 1),
            ),
            unexpected_relations=("mart_orders",),
            expected_stdout_fragments=(
                "dbt execution",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="direct mixed tag materializes fact orders upstream row",
                    sql="SELECT order_id FROM main.fact_orders",
                    expected_rows=((1,),),
                ),
                DbtExecutionQueryAssertion(
                    description="direct mixed tag materializes downstream row",
                    sql="SELECT order_id FROM main.downstream_orders",
                    expected_rows=((1,),),
                ),
            ),
            expected_planned_sqlbuild_models=("downstream_orders", "local_only"),
        ),
        DbtExecutionCliTestCase(
            description="build executes dbt only work",
            command=("dbt", "build", "--select", "dbt_only"),
            expected_row_counts=(("dbt_only", 1),),
            unexpected_relations=("local_only",),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="dbt only build materializes dbt row",
                    sql="SELECT order_id FROM main.dbt_only",
                    expected_rows=((2,),),
                ),
            ),
        ),
        DbtExecutionCliTestCase(
            description="build executes sqlbuild only work",
            command=("dbt", "build", "--select", "local_only"),
            expected_row_counts=(("local_only", 1),),
            unexpected_relations=("dbt_only",),
            expected_stdout_fragments=("Skipping dbt: no dbt work selected.",),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="sqlbuild only build materializes local row",
                    sql="SELECT order_id FROM main.local_only",
                    expected_rows=((10,),),
                ),
            ),
            rerun_count=2,
        ),
        DbtExecutionCliTestCase(
            description="run executes event time incremental sqlbuild work",
            command=(
                "dbt",
                "run",
                "--select",
                "+event_time_orders",
                "--event-time-start",
                "2025-12-31",
                "--event-time-end",
                "2026-01-02",
            ),
            expected_row_counts=(
                ("stg_orders", 1),
                ("fact_orders", 1),
                ("event_time_orders", 1),
            ),
            expected_stdout_fragments=(
                "event_time_orders",
                "--event-time-start 2025-12-31",
                "--event-time-end 2026-01-02",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="event time row preserves timestamp value",
                    sql="SELECT order_id, CAST(ordered_at AS VARCHAR) FROM main.event_time_orders",
                    expected_rows=((1, "2026-01-01 00:00:00"),),
                ),
            ),
            expected_planned_sqlbuild_models=("event_time_orders",),
        ),
        DbtExecutionCliTestCase(
            description="run executes full refresh sqlbuild work",
            command=("dbt", "run", "--full-refresh", "--select", "+event_time_orders"),
            expected_row_counts=(
                ("stg_orders", 1),
                ("fact_orders", 1),
                ("event_time_orders", 1),
            ),
            expected_stdout_fragments=(
                "--full-refresh",
                "event_time_orders",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="full refresh materializes event time row",
                    sql="SELECT order_id, CAST(ordered_at AS VARCHAR) FROM main.event_time_orders",
                    expected_rows=((1, "2026-01-01 00:00:00"),),
                ),
            ),
        ),
        DbtExecutionCliTestCase(
            description="run executes explicit upstream sqlbuild model with package qualified dbt ref",
            command=("dbt", "run", "--select", "+downstream_orders"),
            expected_row_counts=(
                ("fact_orders", 1),
                ("downstream_orders", 1),
            ),
            unexpected_relations=("mart_orders",),
            expected_stdout_fragments=(
                "dbt execution",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="package qualified dbt ref materializes downstream row",
                    sql="SELECT order_id FROM main.downstream_orders",
                    expected_rows=((1,),),
                ),
            ),
            expected_planned_sqlbuild_models=("downstream_orders", "local_only"),
        ),
        DbtExecutionCliTestCase(
            description="build executes path translation scope",
            command=(
                "dbt",
                "build",
                "--select",
                "+path:sql/models/marts",
                "--exclude",
                "deprecated_orders",
                "mart_orders",
                "event_time_orders",
            ),
            expected_row_counts=(
                ("fact_orders", 1),
                ("downstream_orders", 1),
            ),
            unexpected_relations=("deprecated_orders", "mart_orders", "event_time_orders"),
            expected_stdout_fragments=(
                "downstream_orders",
                "downstream_orders",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="path translation materializes downstream row",
                    sql="SELECT order_id FROM main.downstream_orders",
                    expected_rows=((1,),),
                ),
            ),
            expected_planned_sqlbuild_models=("downstream_orders", "local_only"),
        ),
        DbtExecutionCliTestCase(
            description="build executes exclude scope correctly",
            command=("dbt", "build", "--select", "tag:sqb_only", "--exclude", "tag:deprecated"),
            expected_row_counts=(("local_only", 1),),
            unexpected_relations=("deprecated_orders",),
            expected_stdout_fragments=("local_only", "\u2713 Completed successfully"),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="exclude leaves local only row",
                    sql="SELECT order_id FROM main.local_only",
                    expected_rows=((10,),),
                ),
            ),
            expected_planned_sqlbuild_models=("local_only",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_interop_project_when_running_execution_command_then_outputs_expected_relations(
    test_case: DbtExecutionCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    db_path: Path = project_dir / "dbt_interop.duckdb"
    assert result.returncode == 0, result.stderr or result.stdout
    for _ in range(test_case.rerun_count - 1):
        result = run_sqb(command=test_case.command, project_dir=project_dir)
        assert result.returncode == 0, result.stderr or result.stdout
    relation_name: str
    expected_row_count: int
    for relation_name, expected_row_count in test_case.expected_row_counts:
        assert table_exists(db_path=db_path, table_name=relation_name), relation_name
        assert row_count(db_path=db_path, table_name=relation_name) == expected_row_count
    for relation_name in test_case.unexpected_relations:
        assert not table_exists(db_path=db_path, table_name=relation_name), relation_name
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    absent_fragment: str
    for absent_fragment in test_case.expected_absent_stdout_fragments:
        assert absent_fragment not in result.stdout
    query_assertion: DbtExecutionQueryAssertion
    for query_assertion in test_case.expected_query_assertions:
        assert query_duckdb(db_path=db_path, sql=query_assertion.sql) == list(
            query_assertion.expected_rows
        ), query_assertion.description


@pytest.mark.parametrize(
    "test_case",
    [
        DbtExecutionCliTestCase(
            description="dbt interop run preserves native SQLBuild target deferral",
            command=(
                "dbt",
                "run",
                "--select",
                "deferred_consumer",
                "--defer-to",
                "prod",
            ),
            expected_row_counts=(("deferred_consumer", 1),),
            expected_stdout_fragments=(
                "Skipping dbt: no dbt work selected.",
                "SQLBuild execution",
                "\u2713 Completed successfully",
            ),
            expected_query_assertions=(
                DbtExecutionQueryAssertion(
                    description="consumer reads the deferred production upstream",
                    sql="SELECT order_id FROM dev.deferred_consumer",
                    expected_rows=((42,),),
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_native_defer_to_when_running_dbt_interop_then_reads_deferred_target(
    test_case: DbtExecutionCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    write_sqlbuild_defer_target_models(project_dir=project_dir)
    write_dbt_interop_manifest(project_dir=project_dir)
    local_config_path: Path = project_dir / "sqlbuild_local.toml"
    local_config_path.write_text('target = "prod"\n', encoding="utf-8")
    setup_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("build", "--select", "deferred_upstream"),
        project_dir=project_dir,
    )
    assert setup_result.returncode == 0, setup_result.stderr or setup_result.stdout
    local_config_path.write_text('target = "dev"\n', encoding="utf-8")

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    db_path: Path = project_dir / "dbt_interop.duckdb"
    assert result.returncode == 0, result.stderr or result.stdout
    assert table_exists(db_path=db_path, table_name="deferred_consumer", schema="dev")
    assert not table_exists(db_path=db_path, table_name="deferred_upstream", schema="dev")
    for expected_fragment in test_case.expected_stdout_fragments:
        assert expected_fragment in result.stdout
    for query_assertion in test_case.expected_query_assertions:
        assert query_duckdb(db_path=db_path, sql=query_assertion.sql) == list(
            query_assertion.expected_rows
        )


@pytest.mark.parametrize(
    "test_case",
    [
        DbtExecutionCliTestCase(
            description="plan consistency for mixed build case",
            command=("dbt", "build", "--select", "+fact_orders+"),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=(
                "downstream_orders",
                "event_time_orders",
                "mart_orders",
            ),
        ),
        DbtExecutionCliTestCase(
            description="plan consistency for explicit upstream mixed tag case",
            command=("dbt", "build", "--select", "+tag:nightly"),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=("downstream_orders",),
        ),
        DbtExecutionCliTestCase(
            description="plan consistency for event time case",
            command=(
                "dbt",
                "run",
                "--select",
                "event_time_orders",
                "--event-time-start",
                "2025-12-31",
                "--event-time-end",
                "2026-01-02",
            ),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=("event_time_orders",),
        ),
        DbtExecutionCliTestCase(
            description="plan consistency for explicit upstream package qualified ref case",
            command=("dbt", "run", "--select", "+downstream_orders"),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=("downstream_orders",),
        ),
        DbtExecutionCliTestCase(
            description="plan consistency for path translation case",
            command=(
                "dbt",
                "build",
                "--select",
                "+path:sql/models/marts",
                "--exclude",
                "deprecated_orders",
                "mart_orders",
                "event_time_orders",
            ),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=("downstream_orders", "local_only"),
        ),
        DbtExecutionCliTestCase(
            description="plan consistency for exclude case",
            command=("dbt", "build", "--select", "tag:sqb_only", "--exclude", "tag:deprecated"),
            expected_row_counts=(),
            expected_planned_sqlbuild_models=("local_only",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_interop_execution_case_when_planning_then_selected_models_match_expectation(
    test_case: DbtExecutionCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)

    planned_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("dbt", "plan", "--json", *test_case.command[2:]),
        project_dir=project_dir,
    )

    assert planned_result.returncode == 0, planned_result.stderr or planned_result.stdout
    planned_payload: dict[str, object] = load_json_stdout(planned_result.stdout)
    planned_sqlbuild_payload: object = planned_payload["sqlbuild"]
    assert isinstance(planned_sqlbuild_payload, dict)
    typed_sqlbuild_payload: Mapping[str, object] = cast(
        Mapping[str, object], planned_sqlbuild_payload
    )
    assert typed_sqlbuild_payload["selected_models"] == list(
        test_case.expected_planned_sqlbuild_models or ()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DbtExecutionFailureCliTestCase(
            description="run skips dependent sqlbuild work when dbt model fails",
            command=("dbt", "run", "--select", "fact_orders+"),
            expected_stdout_fragments=(
                "dbt execution",
                "fact_orders",
                "skip: external_upstream_failed",
            ),
            expected_absent_stdout_fragments=(),
            expected_returncode=1,
            expected_absent_relations=("downstream_orders", "mart_orders"),
            setup=break_dbt_interop_fact_orders_model,
        ),
        DbtExecutionFailureCliTestCase(
            description="build skips dependent sqlbuild work when dbt model fails",
            command=("dbt", "build", "--select", "fact_orders+"),
            expected_stdout_fragments=(
                "dbt execution",
                "fact_orders",
                "skip: external_upstream_failed",
            ),
            expected_absent_stdout_fragments=(),
            expected_returncode=1,
            expected_absent_relations=("downstream_orders", "mart_orders"),
            setup=break_dbt_interop_fact_orders_model,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_dbt_model_when_running_command_then_dependent_sqlbuild_is_skipped(
    test_case: DbtExecutionFailureCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    test_case.setup(project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_returncode, result.stderr or result.stdout
    db_path: Path = project_dir / "dbt_interop.duckdb"
    expected_stdout_fragment: str
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout
    absent_fragment: str
    for absent_fragment in test_case.expected_absent_stdout_fragments:
        assert absent_fragment not in result.stdout
    relation_name: str
    for relation_name in test_case.expected_absent_relations:
        assert not table_exists(db_path=db_path, table_name=relation_name), relation_name


@pytest.mark.parametrize(
    "test_case",
    [
        DbtMissingOriginE2ETestCase(
            description="dbt build refuses a missing origin that needs confirmation",
            command=("dbt", "build", "--select", "tag:sqb_only"),
            expected_output="build the affected models with sqb build --allow-missing-migration-origin",
        ),
        DbtMissingOriginE2ETestCase(
            description="dbt run refuses a missing origin that needs confirmation",
            command=("dbt", "run", "--select", "local_only"),
            expected_output="build the affected models with sqb build --allow-missing-migration-origin",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_origin_requiring_confirmation_when_running_dbt_then_refuses(
    test_case: DbtMissingOriginE2ETestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    declare_missing_origin_requiring_confirmation(project_dir=project_dir)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir, input_text=""
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert test_case.expected_output in result.stdout + result.stderr
    assert "Pass --allow-missing-migration-origin" not in result.stdout + result.stderr
    assert not table_exists(db_path=project_dir / "dbt_interop.duckdb", table_name="local_only")


@pytest.mark.parametrize(
    "test_case",
    [
        DbtTargetConcurrencyE2ETestCase(
            description="SQLBuild work in dbt interop uses the target's concurrency",
            target_settings="concurrency = 3\n",
            command=("--no-color", "dbt", "run", "--select", "local_only"),
            expected_fragment="concurrency: 3",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_concurrency_when_running_dbt_interop_then_sqlbuild_work_uses_it(
    test_case: DbtTargetConcurrencyE2ETestCase, tmp_path: Path
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    declare_sqlbuild_main_target(project_dir=project_dir, target_settings=test_case.target_settings)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    assert test_case.expected_fragment in output
