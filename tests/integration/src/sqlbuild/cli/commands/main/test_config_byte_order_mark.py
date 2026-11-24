"""Config files saved with a leading byte order mark work across commands."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import ExpectedCountTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        ExpectedCountTestCase(
            description="compile and format accept a byte order mark before the project config",
            expected_count=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_config_with_byte_order_mark_when_running_commands_then_they_succeed(
    test_case: ExpectedCountTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        '\ufeffname = "orders"\nadapter = "duckdb"\n', encoding="utf-8"
    )
    (tmp_path / "sqlbuild_local.toml").write_text(
        "\ufeff[settings]\nsql_analysis = true\n", encoding="utf-8"
    )
    (tmp_path / "sql" / "models").mkdir(parents=True)
    (tmp_path / "sql" / "models" / "orders.sql").write_text(
        "MODEL (description 'One row per order.', materialized table);\n\nSELECT 1 AS order_id\n"
    )

    exit_codes: list[int] = [
        main(["--project-dir", str(tmp_path), "compile", "--no-cache"]),
        main(["--project-dir", str(tmp_path), "format", "--check"]),
    ]
    _ = capsys.readouterr()

    assert sum(exit_codes) == test_case.expected_count
