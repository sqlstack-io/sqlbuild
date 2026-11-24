"""A location load is a full compile: its rules run once, at load, and never again on its snapshot."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    RulesAtLoadTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    failed_load,
    plans_from_copy,
    run_sqb,
    scheduled_worker,
    worker_result,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_PROJECT: str = (
    'name = "orders"\nadapter = "duckdb"\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
    '[targets.development]\nschema = "main"\n'
)
_SELECT_STAR_RULE: str = '[rules]\nselect = ["SQBRMODEL102"]\n'
_NO_RULES: str = "[rules]\nselect = []\n"
_MODEL: str = "MODEL (description 'Orders.', materialized table);\n"
_EVALUATING: str = "Evaluating configured rules..."


@pytest.mark.parametrize(
    "test_case",
    [
        RulesAtLoadTestCase(
            description="a SELECT * model fails the load as a compile failure with its finding",
            model_sql="SELECT * FROM (SELECT 1 AS amount) AS amounts",
            expected_health={
                "kind": "compile_failed",
                "generation": 1,
                "rules": {
                    "finding_count": 1,
                    "findings": [
                        {
                            "code": "SQBRMODEL102",
                            "message": "SELECT * is allowed only inside dependency import CTEs",
                            "path": "sql/models/orders.sql",
                            "line": 2,
                        }
                    ],
                },
            },
            expected_terminal="Compile failed: 1 rule finding",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_rule_finding_when_a_location_loads_then_nothing_is_published_and_health_lists_it(
    test_case: RulesAtLoadTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": _PROJECT + _SELECT_STAR_RULE,
            "sql/models/orders.sql": _MODEL + test_case.model_sql + "\n",
        },
    )

    health, snapshot_status, log = failed_load(tmp_path, project)

    assert health == test_case.expected_health, log
    assert snapshot_status == 404, log
    assert test_case.expected_terminal in log, log


@pytest.mark.parametrize(
    "test_case",
    [
        RulesAtLoadTestCase(
            description="plan previews on the published snapshot leave the rules to the load",
            model_sql="SELECT 1 AS amount",
            expected_rule_evaluations=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_published_snapshot_when_plans_are_previewed_then_rules_ran_only_at_load(
    test_case: RulesAtLoadTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": _PROJECT + _SELECT_STAR_RULE,
            "sql/models/orders.sql": _MODEL + test_case.model_sql + "\n",
        },
    )
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr

    plans: list[dict[str, Any]] = plans_from_copy(
        tmp_path, project, [(["model:orders"], {}), (["model:orders"], {"changed": True})]
    )

    log: str = (tmp_path / "location.log").read_text()
    assert len(plans) == 2, log
    assert log.count(_EVALUATING) == test_case.expected_rule_evaluations, log
    assert "Configured rules passed." in log, log


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="a scheduled run builds what was published though its rules now fail",
            model_sql="SELECT * FROM (SELECT 1 AS amount) AS amounts",
            warehouse_query="SELECT amount FROM main.orders",
            expected_status="success",
            expected_rows=[(1,)],
            files={"sqlbuild_project.toml": _PROJECT + _NO_RULES},
            files_after_publication={"sqlbuild_project.toml": _PROJECT + _SELECT_STAR_RULE},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_published_snapshot_when_a_scheduled_run_executes_then_it_does_not_evaluate_rules(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        run: dict[str, Any] = worker_result(worker, test_case)

    assert run["status"] == test_case.expected_status, worker.log.read_text()
