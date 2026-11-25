"""E2E coverage for orchestration selectors: compile resolves them as publication does, with each site."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    OrchestrationSelectorCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    orchestration_job,
    orchestration_sensor,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import GROUP_CYCLE_NODES

_ROOT_ERROR: str = "path selectors require an explicit root: use 'sql/models/' or 'python/nodes/'"
_ORDERS_JOB: str = "orchestration/jobs/orders_job.toml"
_REPORT_JOB: str = "orchestration/jobs/report_job.toml"
_SENSOR: str = "orchestration/sensors/regions_sensor.py"


@pytest.mark.parametrize(
    "test_case",
    [
        OrchestrationSelectorCompileTestCase(
            description="root-less path selectors in jobs fail compile at their select lines",
            files={
                _ORDERS_JOB: orchestration_job("orders_job", "path:models"),
                _REPORT_JOB: orchestration_job(
                    "report_job", "region_report", exclude="path:python/loaders"
                ),
            },
            expected_messages=(
                f"{_ORDERS_JOB}:3:10: job orders_job: {_ROOT_ERROR}",
                f"{_REPORT_JOB}:3:10: job report_job: {_ROOT_ERROR}",
            ),
            fixed_files={
                _ORDERS_JOB: orchestration_job("orders_job", "path:sql/models"),
                _REPORT_JOB: orchestration_job("report_job", "region_report", exclude="orders"),
            },
        ),
        OrchestrationSelectorCompileTestCase(
            description="a job selecting a node the project lacks fails compile",
            files={_ORDERS_JOB: orchestration_job("orders_job", "missing_orders")},
            expected_messages=(f"{_ORDERS_JOB}:3:10: job orders_job:", "missing_orders"),
            fixed_files={_ORDERS_JOB: orchestration_job("orders_job", "orders")},
        ),
        OrchestrationSelectorCompileTestCase(
            description="a sensor condition with a root-less path selector fails compile at its decorator",
            files={
                _ORDERS_JOB: orchestration_job("orders_job", "orders"),
                "sql/sources/raw.yml": (
                    "sources:\n  - name: raw__regions\n    description: Raw regions\n"
                    "    columns:\n      - name: id\n        type: INTEGER\n"
                ),
                _SENSOR: orchestration_sensor("path:raw"),
            },
            expected_messages=(
                f"{_SENSOR}:4: sensor regions_sensor: source_new_data: {_ROOT_ERROR}",
            ),
            fixed_files={_SENSOR: orchestration_sensor("raw__regions")},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_orchestration_selectors_publication_refuses_when_compiling_then_each_fails_until_fixed(
    tmp_path: Path,
    test_case: OrchestrationSelectorCompileTestCase,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="orchestration_selectors",
        repo_files={
            "sqlbuild_project.toml": 'name = "orchestration_selectors"\nadapter = "duckdb"\n',
            **GROUP_CYCLE_NODES,
            **test_case.files,
        },
    )

    failed: subprocess.CompletedProcess[str] = run_sqb(
        command=("compile",), project_dir=project_dir
    )

    assert failed.returncode == 1, failed.stdout + failed.stderr
    assert "error[D017]" in failed.stderr
    for message in test_case.expected_messages:
        assert message in failed.stderr, failed.stderr

    for path, text in test_case.fixed_files.items():
        (project_dir / path).write_text(text, encoding="utf-8")
    fixed: subprocess.CompletedProcess[str] = run_sqb(command=("compile",), project_dir=project_dir)

    assert fixed.returncode == 0, fixed.stdout + fixed.stderr
    assert "D017" not in fixed.stderr
