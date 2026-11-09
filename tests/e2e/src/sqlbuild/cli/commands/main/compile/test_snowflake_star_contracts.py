"""E2E coverage of Snowflake star outputs, named as the upstream query writes them, against lowercase contracts."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    SnowflakeStarContractE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "shop"\nadapter = "snowflake"\n\n[connection]\naccount = "example"\n'
    'user = "builder"\n\n[defaults]\ndatabase = "ANALYTICS"\nschema = "MARTS"\n\n'
    "[scopes]\nenforce_placement = false\n"
)
_CONTRACT: str = "contract enforced, columns (order_id (type INT), status (type VARCHAR))"


@pytest.mark.parametrize(
    "test_case",
    [
        SnowflakeStarContractE2ETestCase(
            description="a star over the declared columns satisfies the lowercase contract",
            upstream_columns="1 AS order_id, CAST('a' AS VARCHAR) AS status",
            expected_exit_code=0,
            expected_k005_columns=(),
        ),
        SnowflakeStarContractE2ETestCase(
            description="an undeclared column a star adds is still reported",
            upstream_columns="1 AS order_id, CAST('a' AS VARCHAR) AS status, 2 AS extra",
            expected_exit_code=1,
            expected_k005_columns=("extra",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_star_when_compiling_lowercase_contract_then_folded_names_match(
    test_case: SnowflakeStarContractE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "models/orders.sql": (
                f"MODEL (description 'Test model orders.');\nSELECT {test_case.upstream_columns}\n"
            ),
            "models/order_rows.sql": (
                f"MODEL (description 'Test model order_rows.', {_CONTRACT});\n"
                'SELECT o.* FROM __ref("orders") AS o\n'
            ),
        },
    )

    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", "--json"), project_dir=project_dir
    )

    report: dict[str, Any] = json.loads(compiled.stdout)
    codes: list[str] = [item["code"] for item in report["diagnostics"]]
    messages: list[str] = [item["message"] for item in report["diagnostics"]]
    assert compiled.returncode == test_case.expected_exit_code, compiled.stdout + compiled.stderr
    assert codes.count("K005") == len(test_case.expected_k005_columns), messages
    assert set(messages) >= {
        f"column '{name}' is not declared in enforced contract for model 'order_rows'"
        for name in test_case.expected_k005_columns
    }, test_case.description
