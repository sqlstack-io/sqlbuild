"""E2E coverage that the layered production benchmark project compiles cleanly."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    LayeredProductionCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    write_layered_production_compile_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        LayeredProductionCompileTestCase(
            description="small layered project with seed joins, macros, functions and contract",
            model_count=350,
            source_count=83,
            seed_count=16,
            function_count=8,
            macro_count=4,
            test_count=47,
            audit_count=251,
            expected_diagnostic_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_layered_production_project_when_compiling_then_reports_no_diagnostics(
    test_case: LayeredProductionCompileTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = tmp_path / "layered_production"
    write_layered_production_compile_project(
        project_dir=project_dir,
        model_count=test_case.model_count,
        source_count=test_case.source_count,
        seed_count=test_case.seed_count,
        function_count=test_case.function_count,
        macro_count=test_case.macro_count,
        test_count=test_case.test_count,
        audit_count=test_case.audit_count,
    )
    model_sql: str = "".join(
        path.read_text(encoding="utf-8") for path in (project_dir / "sql" / "models").rglob("*.sql")
    )
    for fragment in ("__seed(", "__udf(", "@py.macro_", "model_schema benchmark_row"):
        assert fragment in model_sql

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir,
        command=("compile", "--no-cache", "--json"),
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, Any] = json.loads(result.stdout)
    diagnostic_codes: tuple[str, ...] = tuple(
        str(diagnostic["code"]) for diagnostic in payload["diagnostics"]
    )
    assert diagnostic_codes == test_case.expected_diagnostic_codes, payload["diagnostics"]
    assert payload["summary"]["errors"] == 0
    assert payload["summary"]["warnings"] == 0
    assert payload["summary"]["models"] == test_case.model_count
    assert payload["summary"]["tests"] == test_case.test_count


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
