"""Real CLI namespace isolation on a shared DuckDB schema."""

from __future__ import annotations

import contextvars
import json
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import compute_scenario_hash_prefix
from sqlbuild.cli.entry.main.entry import main
from sqlbuild.executor.scenario._helpers.execution import run as scenario_execution
from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioNamespaceE2ETestCase,
    ScenarioNamespaceSourceE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    change_namespace_fixture,
    list_scenario_relation_names,
    prepare_namespace_project,
    run_namespaced_scenario,
    scenario_relation_name_by_suffix,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import query_duckdb, run_sqb


@pytest.mark.parametrize(
    "test_case",
    (ScenarioNamespaceE2ETestCase("different inputs and isolated success and failure teardown"),),
    ids=lambda case: case.description,
)
def test_given_two_namespaces_when_running_and_tearing_down_then_results_are_independent(
    test_case: ScenarioNamespaceE2ETestCase, tmp_path: Path
) -> None:
    project: Path = prepare_namespace_project(tmp_path)
    database: Path = project / "scenario_demo.duckdb"
    run_namespaced_scenario(project, test_case.namespace_a, retain=True)
    names_a: set[str] = set(list_scenario_relation_names(db_path=database))
    assert len(names_a) == test_case.expected_relation_count
    change_namespace_fixture(project)
    run_namespaced_scenario(project, test_case.namespace_b, retain=True)
    names_b: set[str] = set(list_scenario_relation_names(db_path=database)) - names_a
    assert len(names_b) == test_case.expected_relation_count
    for namespace, expected in (
        (test_case.namespace_a, test_case.expected_total),
        (test_case.namespace_b, test_case.expected_other_total),
    ):
        prefix: str = compute_scenario_hash_prefix(
            project_name="scenario_demo", scenario_name="order_totals_pass", run_namespace=namespace
        )
        assert query_duckdb(
            db_path=database, sql=f'SELECT total_amount FROM "__sqb_{prefix}__model__order_totals"'
        ) == [(expected,)]
    run_namespaced_scenario(project, test_case.namespace_b)
    assert set(list_scenario_relation_names(db_path=database)) == names_a
    scenario: Path = project / "tests/scenarios/order_totals_pass.sql"
    scenario.write_text(scenario.read_text().replace("27 AS total_amount", "28 AS total_amount"))
    failed: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project,
        command=(
            "scenario",
            "test",
            "order_totals_pass",
            "--scenario-namespace",
            test_case.namespace_b,
        ),
    )
    assert failed.returncode == 1
    assert set(list_scenario_relation_names(db_path=database)) == names_a


@pytest.mark.parametrize(
    "test_case",
    (ScenarioNamespaceE2ETestCase("A fixtures survive B complete execution"),),
    ids=lambda case: case.description,
)
def test_given_interleaved_cli_runs_when_b_tears_down_then_a_finishes_correctly(
    test_case: ScenarioNamespaceE2ETestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = prepare_namespace_project(tmp_path)
    original: Callable[..., Any] = scenario_execution.execute_scenario_models
    interleaved = False

    def execute_models(**kwargs: Any) -> Any:
        nonlocal interleaved
        interleaved = True
        monkeypatch.setattr(scenario_execution, "execute_scenario_models", original)
        change_namespace_fixture(project)
        # DuckDB allows simultaneous connections in one process. A fresh context keeps
        # CLI observability separate, while both runs use the real shared warehouse.
        result: int = contextvars.Context().run(
            main,
            [
                "--project-dir",
                str(project),
                "--no-color",
                "scenario",
                "test",
                "order_totals_pass",
                "--scenario-namespace",
                test_case.namespace_b,
            ],
        )
        assert result == test_case.expected_exit_code
        relations: list[tuple[str]] = (
            kwargs["connection"]
            .execute(
                "SELECT table_name FROM information_schema.tables WHERE table_name LIKE '__sqb_%'"
            )
            .fetchall()
        )
        assert len(relations) == 1  # A's fixture survives B's prepare and teardown.
        return original(**kwargs)

    monkeypatch.setattr(scenario_execution, "execute_scenario_models", execute_models)
    result: int = main(
        [
            "--project-dir",
            str(project),
            "--no-color",
            "scenario",
            "test",
            "order_totals_pass",
            "--scenario-namespace",
            test_case.namespace_a,
            "--retain",
        ]
    )
    assert interleaved
    assert result == test_case.expected_exit_code
    database: Path = project / "scenario_demo.duckdb"
    total: str = scenario_relation_name_by_suffix(db_path=database, suffix="__model__order_totals")
    assert query_duckdb(db_path=database, sql=f'SELECT total_amount FROM "{total}"') == [
        (test_case.expected_total,)
    ]


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioNamespaceE2ETestCase(
            "invalid namespace fails before connecting",
            namespace_a="invalid/value",
            expected_exit_code=1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_namespace_when_running_cli_then_no_warehouse_is_created(
    test_case: ScenarioNamespaceE2ETestCase, tmp_path: Path
) -> None:
    project: Path = prepare_namespace_project(tmp_path)
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project,
        command=("scenario", "test", "--scenario-namespace", test_case.namespace_a),
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_error in result.stderr
    assert not (project / "scenario_demo.duckdb").exists()


@pytest.mark.parametrize(
    "test_case",
    (ScenarioNamespaceE2ETestCase("janitor recognizes all namespace hashes"),),
    ids=lambda case: case.description,
)
def test_given_namespaced_leftovers_when_running_janitor_then_all_namespaces_are_cleaned(
    test_case: ScenarioNamespaceE2ETestCase, tmp_path: Path
) -> None:
    project: Path = prepare_namespace_project(tmp_path)
    source: Path = project / "sql/sources/raw.yml"
    source.write_text(source.read_text().replace("schema: main", "schema: raw"))
    config: Path = project / "sqlbuild_project.toml"
    config.write_text(config.read_text() + "\n[janitor]\nenabled = true\nretention_days = 0\n")
    run_namespaced_scenario(project, test_case.namespace_a, retain=True)
    run_namespaced_scenario(project, test_case.namespace_b, retain=True)
    database: Path = project / "scenario_demo.duckdb"
    assert (
        len(list_scenario_relation_names(db_path=database)) == 2 * test_case.expected_relation_count
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project, command=("--no-color", "janitor", "--auto-approve")
    )
    assert result.returncode == test_case.expected_exit_code
    assert not list_scenario_relation_names(db_path=database)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioNamespaceSourceE2ETestCase("unset", None, "unset"),
        ScenarioNamespaceSourceE2ETestCase(
            "project",
            "project",
            "project config",
            project_config='[scenario]\nrun_namespace = "project"\n',
        ),
        ScenarioNamespaceSourceE2ETestCase(
            "local overrides project",
            "local",
            "local config",
            project_config='[scenario]\nrun_namespace = "project"\n',
            local_config='[scenario]\nrun_namespace = "local"\n',
        ),
        ScenarioNamespaceSourceE2ETestCase(
            "environment overrides config",
            "env-job",
            "env",
            project_config='[scenario]\nrun_namespace = "project"\n',
            local_config='[scenario]\nrun_namespace = "local"\n',
            environment=(("SQLBUILD_SCENARIO_NAMESPACE", "env-job"),),
        ),
        ScenarioNamespaceSourceE2ETestCase(
            "CLI overrides environment",
            "cli-job",
            "cli",
            project_config='[scenario]\nrun_namespace = "project"\n',
            environment=(("SQLBUILD_SCENARIO_NAMESPACE", "env-job"),),
            cli_args=("--scenario-namespace", "cli-job"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_namespace_sources_when_running_cli_then_reports_effective_value(
    test_case: ScenarioNamespaceSourceE2ETestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SQLBUILD_SCENARIO_NAMESPACE", raising=False)
    project: Path = prepare_namespace_project(tmp_path)
    config: Path = project / "sqlbuild_project.toml"
    config.write_text(config.read_text() + "\n" + test_case.project_config)
    (project / "sqlbuild_local.toml").write_text(test_case.local_config)
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project,
        command=(
            "--no-color",
            "scenario",
            "test",
            "order_totals_pass",
            "--json",
            *test_case.cli_args,
        ),
        env=dict(test_case.environment),
    )
    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    assert payload["execution"]["scenario_namespace"] == test_case.expected_namespace
    assert payload["execution"]["scenario_namespace_source"] == test_case.expected_source


@pytest.mark.parametrize(
    "test_case",
    (ScenarioNamespaceE2ETestCase("capture cleanup leaves another run intact"),),
    ids=lambda case: case.description,
)
def test_given_retained_run_when_capturing_another_namespace_then_cleanup_is_isolated(
    test_case: ScenarioNamespaceE2ETestCase, tmp_path: Path
) -> None:
    project: Path = prepare_namespace_project(tmp_path)
    database: Path = project / "scenario_demo.duckdb"
    run_namespaced_scenario(project, test_case.namespace_a, retain=True)
    names_a: tuple[str, ...] = list_scenario_relation_names(db_path=database)
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project,
        command=(
            "--no-color",
            "scenario",
            "capture",
            "order_totals_pass",
            "--scenario-namespace",
            test_case.namespace_b,
        ),
    )
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    assert f"Scenario namespace: {test_case.namespace_b} (source: cli)" in result.stdout
    assert list_scenario_relation_names(db_path=database) == names_a


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
