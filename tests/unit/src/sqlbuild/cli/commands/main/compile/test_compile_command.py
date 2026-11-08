from __future__ import annotations

import gc
import json
import sys
import threading
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import sqlbuild._native as discovery_entry
import sqlbuild._native as plan_artifacts_module
from sqlbuild._native import (
    ColumnLineageMode,
    CompileCommandRequest,
    CompileLineageMode,
    run_compile,
)
from sqlbuild.adapter.discovery.main import resolve_adapter as runtime_adapters
from tests.unit.src.sqlbuild.cli.commands.main.compile._test_types import (
    CompileColumnContractModeTestCase,
    CompileCommandTestCase,
    CompileCyclicCollectionTestCase,
    CompileDagArtifactTestCase,
    CompileJsonDiagnosticsTestCase,
    CompileLineageModeTestCase,
    CompilePythonDagArtifactTestCase,
    RuleGatedTestPlanningTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.compile.helpers import (
    NoConnectDuckDbAdapter,
    TtyStdout,
    prepare_python_compile_project,
    prepare_rule_gated_compile_project,
    prepare_static_compile_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileCommandTestCase(
            description="compiles local project without connecting",
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Compile ready  1 model",
                "orders                   OK   1 columns",
                "\u2713 Project compiled  1 model, 0 seeds, 0 functions, 0 errors, 0 warnings",
                "  Wrote: target/compiled/",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_local_project_when_running_compile_then_it_does_not_connect(
    test_case: CompileCommandTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(CompileCommandRequest(project_dir=project_dir))
    rendered_stdout: str = capsys.readouterr().out

    assert exit_code == test_case.expected_exit_code
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in rendered_stdout
    assert (project_dir / "target" / "compiled" / "models" / "orders.sql").exists()
    assert not (project_dir / "target" / "manifest.json").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        CompileCommandTestCase(
            description="persists phase timings for tty stdout",
            expected_exit_code=0,
            expected_stdout_fragments=(
                "Discovered project.",
                "Compiled project graph.",
                "Analyzed column lineage.",
                "Validated model contracts.",
                "Evaluated rules.",
                "Wrote compiled artifacts.",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_tty_stdout_when_running_compile_then_it_persists_phase_timings(
    test_case: CompileCommandTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    stdout: TtyStdout = TtyStdout()
    monkeypatch.setenv("SQLBUILD_NO_PROGRESS", "1")
    monkeypatch.setattr(sys, "stdout", stdout)
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(
        CompileCommandRequest(project_dir=project_dir, no_sql_validation=True, no_color=True)
    )
    rendered_stdout: str = stdout.getvalue()

    assert exit_code == test_case.expected_exit_code
    for message in (
        "Discovering project...",
        "Compiling project graph...",
        "Analyzing column lineage...",
        "Validating model contracts...",
        "Evaluating built-in and custom rules...",
        "Writing compiled artifacts...",
    ):
        assert f"{message}\n" in rendered_stdout
    for fragment in test_case.expected_stdout_fragments:
        assert f"\n{fragment} (" in rendered_stdout


@pytest.mark.parametrize(
    "test_case",
    [
        CompileDagArtifactTestCase(
            description="writes dag artifact to default target path",
            dag_path="",
            expected_project_name="offline_compile",
            expected_node_ids=("model:orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dag_flag_when_running_compile_then_writes_dag_artifact(
    test_case: CompileDagArtifactTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(
        CompileCommandRequest(
            project_dir=project_dir,
            no_sql_validation=True,
            dag_path=test_case.dag_path,
        )
    )
    dag_payload: dict[str, object] = json.loads(
        (project_dir / "target" / "sqlbuild_dag.json").read_text(encoding="utf-8")
    )

    assert exit_code == 0
    assert dag_payload["project_name"] == test_case.expected_project_name
    assert tuple(node["id"] for node in dag_payload["nodes"]) == test_case.expected_node_ids


@pytest.mark.parametrize(
    "test_case",
    [
        CompilePythonDagArtifactTestCase(
            description="writes Python nodes to default dag artifact path",
            dag_path="",
            expected_project_name="offline_compile",
            expected_node_ids={
                "model:orders",
                "task:prepare_orders",
                "loader:warehouse_export",
                "asset:orders_export",
                "check:check_orders_export",
                "check:check_loader_export",
            },
            expected_edges={
                ("model:orders", "task:prepare_orders"),
                ("task:prepare_orders", "loader:warehouse_export"),
                ("loader:warehouse_export", "asset:orders_export"),
                ("loader:warehouse_export", "check:check_loader_export"),
            },
            expected_check_ids={
                "check:check_orders_export",
                "check:check_loader_export",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_project_dag_flag_when_running_compile_then_writes_python_dag_artifact(
    test_case: CompilePythonDagArtifactTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_python_compile_project(tmp_path)
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(
        CompileCommandRequest(
            project_dir=project_dir,
            no_sql_validation=True,
            dag_path=test_case.dag_path,
        )
    )
    dag_payload: dict[str, object] = json.loads(
        (project_dir / "target" / "sqlbuild_dag.json").read_text(encoding="utf-8")
    )
    node_ids: set[str] = {str(node["id"]) for node in dag_payload["nodes"]}
    edges: set[tuple[str, str]] = {
        (str(edge["from_id"]), str(edge["to_id"])) for edge in dag_payload["edges"]
    }
    check_ids: set[str] = {str(check["id"]) for check in dag_payload["checks"]}

    assert exit_code == 0
    assert dag_payload["project_name"] == test_case.expected_project_name
    assert test_case.expected_node_ids.issubset(node_ids)
    assert test_case.expected_edges.issubset(edges)
    assert test_case.expected_check_ids.issubset(check_ids)


@pytest.mark.parametrize(
    "test_case",
    (
        CompileCommandTestCase(
            description="returns one when contract diagnostics have errors",
            expected_exit_code=1,
            expected_stdout_fragments=(
                "Compile ready  1 model",
                "orders                   FAIL 1 columns",
                "error[K001]: declared column 'customer_id' was not found in statically inferred output",
                "  model: orders",
                "  --> models/orders.sql:5:5",
                "  5 |     customer_id (),",
                "    |     ^^^^^^^^^^^",
                'settings.column_contract_mode is "implicit" (the default)',
                'column_contract_mode = "explicit"',
                "models with contract enforced remain validated",
                "\u2717 Project compiled  1 model, 0 seeds, 0 functions, 1 error, 0 warnings",
                "  Wrote: target/compiled/",
            ),
            model_sql=(
                "MODEL (description 'Test model.',\n"
                "  materialized view,\n"
                "  columns (\n"
                "    order_id (),\n"
                "    customer_id (),\n"
                "  ),\n"
                ");\n\n"
                "SELECT 1 AS order_id\n"
            ),
        ),
        CompileCommandTestCase(
            description="reports contract and output locations for type diagnostics",
            expected_exit_code=1,
            expected_stdout_fragments=(
                "error[K002]: column 'amount_cents' inferred as VARCHAR but declared type is INTEGER",
                "  --> models/orders.sql:4:5",
                "  4 |     amount_cents (type INTEGER),",
                "    |     ^^^^^^^^^^^^",
                "  output:",
                "  --> models/orders.sql:9:3",
                "  9 |   CAST('1' AS VARCHAR) AS amount_cents",
                "inferred VARCHAR",
                "  = help: change the declared type or cast the expression explicitly",
            ),
            model_sql=(
                "MODEL (description 'Test model.',\n"
                "  materialized view,\n"
                "  columns (\n"
                "    amount_cents (type INTEGER),\n"
                "  ),\n"
                ");\n\n"
                "SELECT\n"
                "  CAST('1' AS VARCHAR) AS amount_cents\n"
            ),
        ),
        CompileCommandTestCase(
            description="reports output location for undeclared contract columns",
            expected_exit_code=1,
            expected_stdout_fragments=(
                "error[K005]: column 'customer_id' is not declared in enforced contract",
                "  --> models/orders.sql:11:3",
                " 11 |   2 AS customer_id",
                "    |   ^^^^^^^^^^^^^^^^",
                "  = help: add the column to MODEL(columns) or remove it from the SELECT list",
            ),
            model_sql=(
                "MODEL (description 'Test model.',\n"
                "  materialized view,\n"
                "  contract enforced,\n"
                "  columns (\n"
                "    order_id (type INTEGER),\n"
                "  ),\n"
                ");\n\n"
                "SELECT\n"
                "  1 AS order_id,\n"
                "  2 AS customer_id\n"
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_contract_errors_when_running_compile_then_reports_diagnostics(
    test_case: CompileCommandTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    assert test_case.model_sql is not None
    (project_dir / "models" / "orders.sql").write_text(
        test_case.model_sql,
        encoding="utf-8",
    )
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(CompileCommandRequest(project_dir=project_dir))
    rendered_stdout: str = capsys.readouterr().out

    assert exit_code == test_case.expected_exit_code
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in rendered_stdout
    assert (project_dir / "target" / "compiled" / "models" / "orders.sql").exists()
    assert not (project_dir / "target" / "manifest.json").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        CompileJsonDiagnosticsTestCase(
            description="serializes contract diagnostics in compile json",
            expected_exit_code=1,
            expected_code="K001",
            expected_severity="error",
            expected_message=(
                "declared column 'customer_id' was not found in statically inferred output "
                "for model 'orders'"
            ),
            expected_line=5,
            expected_column=5,
            model_sql=(
                "MODEL (description 'Test model.',\n"
                "  materialized view,\n"
                "  columns (\n"
                "    order_id (),\n"
                "    customer_id (),\n"
                "  ),\n"
                ");\n\n"
                "SELECT 1 AS order_id\n"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_contract_errors_when_running_compile_json_then_serializes_diagnostics(
    test_case: CompileJsonDiagnosticsTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    (project_dir / "models" / "orders.sql").write_text(
        test_case.model_sql,
        encoding="utf-8",
    )
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(
        CompileCommandRequest(
            project_dir=project_dir,
            json_output=True,
        )
    )
    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    diagnostics: list[dict[str, object]] = payload["diagnostics"]

    assert exit_code == test_case.expected_exit_code
    assert payload["has_errors"] is True
    assert payload["summary"]["errors"] == 1
    assert diagnostics[0]["code"] == test_case.expected_code
    assert diagnostics[0]["severity"] == test_case.expected_severity
    assert diagnostics[0]["message"] == test_case.expected_message
    assert diagnostics[0]["path"] == "models/orders.sql"
    assert diagnostics[0]["line"] == test_case.expected_line
    assert diagnostics[0]["column"] == test_case.expected_column
    assert diagnostics[0]["location"] == {
        "path": "models/orders.sql",
        "line": test_case.expected_line,
        "column": test_case.expected_column,
        "end_line": test_case.expected_line,
        "end_column": 16,
    }
    assert diagnostics[0]["phase"] == "contract"


@pytest.mark.parametrize(
    "test_case",
    [
        CompileColumnContractModeTestCase(
            description="explicit mode retains column audit",
            mode="explicit",
            expected_audit_name="not_null",
            expected_column_name="customer_id",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_column_contract_mode_when_compiling_column_audit_then_audit_is_retained_without_contract_error(
    test_case: CompileColumnContractModeTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    project_config_path: Path = project_dir / "sqlbuild_project.toml"
    project_config_path.write_text(
        project_config_path.read_text(encoding="utf-8")
        + f'\n[settings]\ncolumn_contract_mode = "{test_case.mode}"\n',
        encoding="utf-8",
    )
    (project_dir / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.',\n"
        "  materialized view,\n"
        "  columns (\n"
        "    customer_id (audits [not_null]),\n"
        "  ),\n"
        ");\n\n"
        "SELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    monkeypatch.setattr(
        runtime_adapters,
        "resolve_adapter",
        lambda *args, **kwargs: NoConnectDuckDbAdapter(),
    )

    exit_code: int = run_compile(
        CompileCommandRequest(project_dir=project_dir, no_sql_validation=True, dag_path="")
    )
    dag_payload: dict[str, object] = json.loads(
        (project_dir / "target" / "sqlbuild_dag.json").read_text(encoding="utf-8")
    )

    assert exit_code == 0
    assert len(dag_payload["checks"]) == 1
    assert dag_payload["checks"][0]["name"] == test_case.expected_audit_name
    assert dag_payload["checks"][0]["attached_column_name"] == test_case.expected_column_name


@pytest.mark.parametrize(
    "test_case",
    (
        CompileLineageModeTestCase(
            description="uses fast lineage by default",
            lineage_mode=CompileLineageMode.FAST,
            expected_lineage_mode_values=(ColumnLineageMode.FAST.value,),
        ),
        CompileLineageModeTestCase(
            description="uses rich lineage when requested",
            lineage_mode=CompileLineageMode.RICH,
            expected_lineage_mode_values=(ColumnLineageMode.RICH.value,),
        ),
        CompileLineageModeTestCase(
            description="skips lineage when disabled",
            lineage_mode=CompileLineageMode.NONE,
            expected_lineage_mode_values=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_lineage_mode_when_compiling_then_lineage_runs_in_that_mode(
    test_case: CompileLineageModeTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    _ = discovery_entry.compile_stage_work_info(reset=True)

    exit_code: int = run_compile(
        CompileCommandRequest(
            project_dir=project_dir,
            json_output=True,
            no_cache=True,
            lineage_mode=test_case.lineage_mode,
        )
    )
    _ = capsys.readouterr()

    assert exit_code == 0
    assert discovery_entry.compile_stage_work_info().get("build_project_column_lineage", 0) == len(
        test_case.expected_lineage_mode_values
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RuleGatedTestPlanningTestCase(
            description="background test planning is reused for withheld artifacts",
            min_prepared_models=128,
            test_header='name "orders_case"',
            expected_codes=("SQBRMODEL102", "S000"),
            expected_message_fragment='references __source("raw_refunds") which has no mock',
            expected_inline_planning_calls=0,
        ),
        RuleGatedTestPlanningTestCase(
            description="background preparation reuses staged planning",
            min_prepared_models=1,
            test_header='name "orders_case"',
            expected_codes=("SQBRMODEL102", "S000"),
            expected_message_fragment='references __source("raw_refunds") which has no mock',
            expected_inline_planning_calls=0,
        ),
        RuleGatedTestPlanningTestCase(
            description="background test planning input error becomes a diagnostic",
            min_prepared_models=128,
            test_header='name "orders_case", cursor_start "2026-02-01"',
            expected_codes=("SQBRMODEL102", "P001"),
            expected_message_fragment="declares cursor_start or cursor_end",
            expected_inline_planning_calls=0,
        ),
        RuleGatedTestPlanningTestCase(
            description="background planning input error becomes a diagnostic",
            min_prepared_models=1,
            test_header='name "orders_case", cursor_start "2026-02-01"',
            expected_codes=("SQBRMODEL102", "P001"),
            expected_message_fragment="declares cursor_start or cursor_end",
            expected_inline_planning_calls=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rule_error_when_compiling_json_then_withheld_test_planning_errors_are_reported(
    test_case: RuleGatedTestPlanningTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = prepare_rule_gated_compile_project(
        tmp_path, test_header=test_case.test_header
    )
    planning_on_main_thread: list[bool] = []
    plan_artifacts: Callable[..., object] = plan_artifacts_module.plan_compiled_sql_test_artifacts

    def counted_inline_planning(**kwargs: Any) -> object:
        planning_on_main_thread.append(threading.current_thread() is threading.main_thread())
        return plan_artifacts(**kwargs)

    class Prepared(discovery_entry.PreparedCompileArtifacts):
        min_prepared_models: int = test_case.min_prepared_models

    monkeypatch.setattr(
        runtime_adapters, "resolve_adapter", lambda *args, **kwargs: NoConnectDuckDbAdapter()
    )
    monkeypatch.setattr(
        plan_artifacts_module, "plan_compiled_sql_test_artifacts", counted_inline_planning
    )
    monkeypatch.setattr(discovery_entry, "PreparedCompileArtifacts", Prepared)

    exit_code: int = run_compile(
        CompileCommandRequest(project_dir=project_dir, json_output=True, no_cache=True)
    )
    payload: dict[str, Any] = json.loads(capsys.readouterr().out)

    assert exit_code == 1
    assert tuple(diagnostic["code"] for diagnostic in payload["diagnostics"]) == (
        test_case.expected_codes
    )
    assert test_case.expected_message_fragment in payload["diagnostics"][-1]["message"]
    assert payload["summary"]["errors"] == len(test_case.expected_codes)
    assert planning_on_main_thread.count(True) == test_case.expected_inline_planning_calls
    assert not (project_dir / "target" / "compiled" / "tests").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        CompileCyclicCollectionTestCase(
            description="enabled collection is paused and restored",
            collection_enabled_before=True,
            expected_enabled_during=False,
            expected_enabled_after=True,
        ),
        CompileCyclicCollectionTestCase(
            description="disabled collection stays disabled",
            collection_enabled_before=False,
            expected_enabled_during=False,
            expected_enabled_after=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_collection_state_when_compiling_then_cycles_are_paused_and_state_restored(
    test_case: CompileCyclicCollectionTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project_dir: Path = prepare_static_compile_project(tmp_path)
    enabled_during: list[bool] = []

    def observed_adapter(*args: Any, **kwargs: Any) -> NoConnectDuckDbAdapter:
        enabled_during.append(gc.isenabled())
        return NoConnectDuckDbAdapter()

    monkeypatch.setattr(runtime_adapters, "resolve_adapter", observed_adapter)
    set_collection: dict[bool, Callable[[], None]] = {True: gc.enable, False: gc.disable}
    was_enabled: bool = gc.isenabled()
    set_collection[test_case.collection_enabled_before]()
    try:
        exit_code: int = run_compile(CompileCommandRequest(project_dir=project_dir))
        enabled_after: bool = gc.isenabled()
    finally:
        set_collection[was_enabled]()

    assert exit_code == 0
    assert enabled_during == [test_case.expected_enabled_during]
    assert enabled_after is test_case.expected_enabled_after
    assert (project_dir / "target" / "compiled" / "models" / "orders.sql").exists()
