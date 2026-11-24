"""Real-CLI coverage of clone phase timings in --debug output and the JSON document."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.clone._test_types import CloneTimingsE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "clone_timings"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "clone.duckdb"\n\n'
        '[targets.prod]\nschema = "prod"\n\n'
        "[targets.prod.clone]\nallow_as_clone_origin = true\n\n"
        '[targets.dev]\nschema = "dev"\n\n'
        "[targets.dev.clone]\nallow_as_clone_destination = true\n"
    ),
    "sql/models/fact_orders.sql": (
        'MODEL (description "Test model fact_orders.", materialized table);\n\n'
        "SELECT 1 AS order_id\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        CloneTimingsE2ETestCase(
            description="debug and JSON name every clone phase",
            expected_phases=(
                "compile_prod",
                "compile_dev",
                "source_inspection",
                "planning",
                "execution",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_clone_when_debugging_then_reports_each_phase_duration(
    test_case: CloneTimingsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="clone_timings", repo_files=_PROJECT_FILES
    )
    execute_duckdb(
        db_path=project_dir / "clone.duckdb",
        sql="CREATE SCHEMA prod; CREATE SCHEMA dev; CREATE TABLE prod.fact_orders AS SELECT 1 AS order_id;",
    )
    document_path: Path = tmp_path / "clone.json"

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "--debug",
            "clone",
            "--from",
            "prod",
            "--json-output",
            str(document_path),
        ),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, output
    document: dict[str, Any] = json.loads(document_path.read_text(encoding="utf-8"))
    timings: dict[str, float] = document["phase_timings"]
    assert tuple(timings) == test_case.expected_phases
    assert all(seconds >= 0 for seconds in timings.values())
    for phase in test_case.expected_phases:
        assert f"Clone timing  {phase}  " in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
