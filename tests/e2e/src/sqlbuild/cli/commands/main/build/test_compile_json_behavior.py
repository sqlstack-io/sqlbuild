"""E2E tests for compile --json behavior."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CompileJsonBuildE2ETestCase,
    CompileJsonRaisedErrorE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileJsonBuildE2ETestCase(
            description="compile json reports offline query sql",
            command=("compile", "--json"),
            expected_exit_code=0,
            expected_model_names=(
                "customer_status_snapshot",
                "hourly_order_activity",
                "daily_activity_rollup",
            ),
            expected_sql_fragments=('__ref("fact_orders")', "DATE_TRUNC"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_compile_json_then_it_reports_offline_query_sql(
    test_case: CompileJsonBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert payload["offline"] is True
    assert payload["command"] == "compile"
    assert payload["has_errors"] is False
    summary: dict[str, object] = payload["summary"]
    assert summary["models"] >= len(test_case.expected_model_names)
    assert summary["errors"] == 0
    assert summary["warnings"] == test_case.expected_warning_count
    diagnostics: list[dict[str, object]] = payload["diagnostics"]
    assert tuple(diagnostic["code"] for diagnostic in diagnostics) == (
        test_case.expected_diagnostic_codes
    )
    timings: dict[str, object] = payload["compile_timings"]
    assert set(timings) == {
        "discover_ms",
        "graph_ms",
        "lineage_ms",
        "contracts_ms",
        "contracts_cpu_ms",
        "write_ms",
        "attachment_ms",
        "built_in_rules_ms",
        "custom_rules_ms",
        "custom_rules_cpu_ms",
        "custom_rules_invocations",
        "custom_rules_monitor_cpu_ms",
        "custom_rules_guard_cpu_ms",
        "early_lint_wait_ms",
        "model_analysis_ms",
        "model_analysis_cpu_ms",
        "analysis_preparation_ms",
        "analysis_native_ms",
        "analysis_projection_ms",
        "binding_validation_ms",
        "rule_cache_hits",
        "rule_cache_misses",
        "test_input_compile_ms",
        "test_planning_ms",
        "comparison_render_ms",
        "cache_publication_ms",
        "physical_write_ms",
        "stale_traversal_ms",
        "early_rules_build_ms",
        "early_rules_wait_ms",
        "analysis_batch_cache_hits",
        "analysis_entry_cache_hits",
        "analysis_cache_misses",
        "analysis_cache_bypasses",
        "sql_test_scan_cache_hits",
        "sql_test_scan_cache_misses",
        "early_rules_hits",
        "early_rules_misses",
        "native_analysis_shared",
        "native_analysis_unshared",
        "project_reuse_hits",
        "project_reuse_misses",
        "project_reuse_bypasses",
        "project_reuse_check_ms",
        "total_ms",
    }
    assert all(isinstance(value, int) and value >= 0 for value in timings.values())
    artifacts: dict[str, object] = payload["artifacts"]
    assert artifacts["compiled_sql_dir"] == "target/compiled/"
    assert artifacts["manifest"] is None
    resources: dict[str, object] = payload["resources"]
    models: list[dict[str, object]] = resources["models"]
    models_by_name: dict[str, tuple[dict[str, object], ...]] = {
        str(model["name"]): (model,) for model in models
    }
    assert len(models_by_name) == len(models)
    selected_models: list[dict[str, object]] = [
        next(iter(models_by_name.get(name, ()))) for name in test_case.expected_model_names
    ]
    assert len(selected_models) == len(test_case.expected_model_names)
    fragment: str
    for fragment in test_case.expected_sql_fragments:
        assert any(fragment in str(model["query_sql"]) for model in selected_models)
    fact_orders: dict[str, object] = next(iter(models_by_name.get("fact_orders", ())))
    assert fact_orders["materialized"] == "table"
    assert fact_orders["column_count"] == 14
    assert fact_orders["lineage"] == {
        "available": True,
        "column_count": 14,
        "edge_count": 15,
        "has_star": False,
    }
    depends_on: list[dict[str, object]] = cast(list[dict[str, object]], fact_orders["depends_on"])
    assert {dep["name"] for dep in depends_on} >= {
        "stg_orders",
        "stg_payments",
        "waffle_types",
    }
    assert resources["seeds"]
    assert resources["functions"]
    assert resources["audits"]
    assert resources["tests"]
    assert all("logical_ddl" not in model for model in models)
    assert all("action" not in model for model in models)


@pytest.mark.parametrize(
    "test_case",
    [
        CompileJsonBuildE2ETestCase(
            description="compile json reports normalized lifecycle hook metadata",
            command=("compile", "--json"),
            expected_exit_code=0,
            expected_model_names=("orders",),
            expected_sql_fragments=("SELECT 1 AS id",),
            expected_pre_hooks=(
                {
                    "type": "sql",
                    "statement": "SELECT 1",
                    "name": "select_value",
                    "relative_path": "hooks/sql/select_value.sql",
                    "definition_sql": "SELECT @value",
                    "kwargs": {"value": 1},
                    "description": "Test hook select_value.",
                },
                {"type": "python", "name": "notify", "kwargs": {"message": "starting"}},
            ),
            expected_post_hooks=(
                {"type": "python", "name": "notify", "kwargs": {"message": "done"}},
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_model_with_hooks_when_running_compile_json_then_it_reports_hook_metadata(
    test_case: CompileJsonBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="compile_json_hooks_project",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "compile_json_hooks_project"\n'
                'adapter = "duckdb"\n\n'
                "[connection]\n"
                'database = "compile_json_hooks_project.duckdb"\n'
            ),
            "hooks/python/notify.py": (
                "from sqlbuild.hooks import hook\n\n"
                "@hook\n"
                "def notify(ctx, message):\n"
                "    '''Test hook notify.'''\n    ctx.log(message)\n"
            ),
            "hooks/sql/select_value.sql": "HOOK (description 'Test hook select_value.');\n\nSELECT @value\n",
            "models/orders.sql": (
                "MODEL (description 'Test model orders.',\n"
                "  materialized table,\n"
                '  pre_hooks [sql("select_value", value: 1), '
                'python("notify", message: "starting")],\n'
                '  post_hooks [python("notify", message: "done")]\n'
                ");\n\n"
                "SELECT 1 AS id\n"
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    resources: dict[str, object] = payload["resources"]
    models: list[dict[str, object]] = resources["models"]
    models_by_name: dict[str, tuple[dict[str, object], ...]] = {
        str(model["name"]): (model,) for model in models
    }
    assert len(models_by_name) == len(models)
    orders: dict[str, object] = next(iter(models_by_name.get("orders", ())))
    fragment: str
    for fragment in test_case.expected_sql_fragments:
        assert fragment in str(orders["query_sql"])
    assert orders["pre_hooks"] == list(test_case.expected_pre_hooks)
    assert orders["post_hooks"] == list(test_case.expected_post_hooks)
    hooks: list[dict[str, object]] = resources["hooks"]
    hooks_by_name: dict[str, dict[str, object]] = {str(hook["name"]): hook for hook in hooks}
    assert hooks_by_name["select_value"] == {
        "type": "sql",
        "name": "select_value",
        "relative_path": "hooks/sql/select_value.sql",
        "description": "Test hook select_value.",
        "sql": "SELECT @value",
    }
    assert hooks_by_name["notify"]["relative_path"] == "hooks/python/notify.py"
    assert hooks_by_name["notify"]["definition_hash"]
    assert hooks_by_name["notify"]["version_hash"]


@pytest.mark.parametrize(
    "test_case",
    [
        CompileJsonRaisedErrorE2ETestCase(
            description="unknown materialization stops compile",
            repo_files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "models/orders.sql": "MODEL (description 'Test model orders.', materialized tablex);\n\nSELECT 1 AS order_id\n",
            },
            expected_code="P001",
            expected_message_fragment="unknown materialization 'tablex'",
            expected_help_fragment=None,
        ),
        CompileJsonRaisedErrorE2ETestCase(
            description="unknown on_schema_change stops compile with help",
            repo_files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "models/orders.sql": (
                    "MODEL (description 'Test model orders.',\n  materialized incremental,\n  incremental_strategy append,\n"
                    "  on_schema_change append_columns,\n);\n\nSELECT 1 AS order_id\n"
                ),
            },
            expected_code="P001",
            expected_message_fragment="unknown on_schema_change 'append_columns'",
            expected_help_fragment="on_schema_change append_new_columns,",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_raised_compile_error_when_running_compile_json_then_stdout_holds_one_diagnostic_report(
    test_case: CompileJsonRaisedErrorE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("compile", "--json"), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert payload["command"] == "compile"
    assert payload["has_errors"] is True
    assert payload["stopped"] is True
    diagnostics: list[dict[str, object]] = cast(list[dict[str, object]], payload["diagnostics"])
    assert len(diagnostics) == 1
    diagnostic: dict[str, object] = diagnostics[0]
    assert diagnostic["severity"] == "error"
    assert diagnostic["code"] == test_case.expected_code
    message: str = cast(str, diagnostic["message"])
    assert test_case.expected_message_fragment in message
    assert f"error[{test_case.expected_code}]: {message}" in result.stderr
    help_text: str = cast(str, diagnostic.get("help", ""))
    assert ("help" in diagnostic) is (test_case.expected_help_fragment is not None)
    assert (test_case.expected_help_fragment or "") in help_text
    assert all(line.strip() in result.stderr for line in help_text.splitlines())
