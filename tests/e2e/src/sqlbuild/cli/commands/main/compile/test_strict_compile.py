"""E2E tests for `settings.strict`: report-only lists every unproven check, ranked."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import StrictCompileTestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT: str = 'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n'
_MARGIN_FUNCTION: str = (
    "FUNCTION (description 'Margin of a price.',\n"
    "  arguments (price DOUBLE),\n"
    "  returns DOUBLE,\n"
    ");\n\n"
    "price * 0.2\n"
)
_ORDERS: str = (
    "MODEL (description 'Orders.');\n\n"
    "SELECT order_id, shop_margin(price) AS margin\n"
    "FROM (SELECT 1 AS order_id, 2.5 AS price) AS o\n"
)
_REFUNDS: str = (
    "MODEL (description 'Refunds.');\n\n"
    "SELECT refund_id FROM (SELECT 1 AS refund_id, 2.5 AS price) AS r\n"
    "WHERE shop_margin(price) > 0\n"
)
_DECLARED_ORDERS: str = (
    "MODEL (description 'Orders.');\n\n"
    'SELECT order_id, __udf("margin")(price) AS margin\n'
    "FROM (SELECT 1 AS order_id, CAST(2.5 AS DOUBLE) AS price) AS o\n"
)


@pytest.mark.parametrize(
    "test_case",
    (
        StrictCompileTestCase(
            description="without strict the partial-checks notice is unchanged",
            settings="",
            files={"models/orders.sql": _ORDERS, "models/refunds.sql": _REFUNDS},
            expected_exit_code=0,
            expected_strict="off",
            expected_fragments=(
                "Semantic checks were partial for 2 models:",
                "orders, refunds: calls unknown function shop_margin;",
            ),
            unexpected_fragments=("Strict report",),
        ),
        StrictCompileTestCase(
            description="report-only ranks each unproven check by the models it leaves partial",
            settings='\n[settings]\nstrict = "report"\n',
            files={"models/orders.sql": _ORDERS, "models/refunds.sql": _REFUNDS},
            expected_exit_code=0,
            expected_strict="report",
            expected_fragments=(
                "Strict report: 2 checks unproven across 2 models:",
                "  1. calls unknown function shop_margin (2 models): orders, refunds;",
                "  2. some output types are unknown (1 model): orders;",
                'Report only (settings.strict = "report"): the exit code is unchanged',
            ),
            unexpected_fragments=("Semantic checks were partial",),
        ),
        StrictCompileTestCase(
            description="report-only with every check proven prints no report",
            settings='\n[settings]\nstrict = "report"\n',
            files={"functions/sql/margin.sql": _MARGIN_FUNCTION, "models/orders.sql": _DECLARED_ORDERS},
            expected_exit_code=0,
            expected_strict="report",
            expected_fragments=("0 errors",),
            unexpected_fragments=("Strict report", "Semantic checks were partial"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_strict_setting_when_compiling_then_unproven_checks_are_reported(
    test_case: StrictCompileTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={"sqlbuild_project.toml": _PROJECT + test_case.settings, **test_case.files},
    )

    text: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile", "--no-cache")
    )
    report: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("compile", "--json", "--no-cache")
    )

    output: str = text.stdout + text.stderr
    payload: dict[str, object] = json.loads(report.stdout)
    assert text.returncode == test_case.expected_exit_code, output
    assert report.returncode == test_case.expected_exit_code, report.stdout + report.stderr
    assert payload["strict"] == test_case.expected_strict
    for fragment in test_case.expected_fragments:
        assert fragment in output, output
    for fragment in test_case.unexpected_fragments:
        assert fragment not in output, output


def test_given_an_invalid_strict_value_when_compiling_then_the_setting_is_rejected(
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT + '\n[settings]\nstrict = "always"\n',
            "models/orders.sql": _ORDERS,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )

    assert result.returncode != 0
    assert 'settings.strict must be true, false or "report"' in result.stdout + result.stderr
