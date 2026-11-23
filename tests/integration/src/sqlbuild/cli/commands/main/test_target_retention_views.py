"""Integration coverage for target-level retention defaults with views."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    TargetRetentionViewsTestCase,
)

_PROJECT_TOML: str = (
    'name = "demo"\nadapter = "duckdb"\ndefault_target = "prod"\n\n'
    '[connection]\ndatabase = "demo.duckdb"\n\n'
    '[targets.prod]\ntime_travel_retention = "90d"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        TargetRetentionViewsTestCase(
            description="view skips inherited target retention",
            view_header="MODEL (description 'Test model.', materialized view);",
            expected_exit_code=0,
            expected_fragment="Project compiled",
        ),
        TargetRetentionViewsTestCase(
            description="explicit view retention is still rejected",
            view_header="MODEL (description 'Test model.', materialized view, time_travel_retention 7d);",
            expected_exit_code=1,
            expected_fragment="managed time_travel_retention is not valid for views",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_retention_default_when_compiling_views_then_only_explicit_retention_fails(
    test_case: TargetRetentionViewsTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    (tmp_path / "sql" / "models").mkdir(parents=True)
    (tmp_path / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
    (tmp_path / "sql" / "models" / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    (tmp_path / "sql" / "models" / "order_view.sql").write_text(
        f'{test_case.view_header}\n\nSELECT order_id FROM __ref("orders")\n', encoding="utf-8"
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", "compile"])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert test_case.expected_fragment in captured


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
