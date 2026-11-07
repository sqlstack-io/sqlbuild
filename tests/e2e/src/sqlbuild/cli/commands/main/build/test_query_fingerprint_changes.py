"""E2E coverage for token-stream query fingerprints in change detection."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import pytest

from sqlbuild._native import compute_query_hash
from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    QueryFingerprintEditE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "fingerprint_orders"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "fingerprint.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n'
    ),
    "sources/raw.yml": (
        "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n    schema: main\n    table: raw_orders\n"
    ),
    "functions/sql/is_large_order.sql": (
        "FUNCTION (description 'Test function is_large_order.', arguments (amount_cents INTEGER), returns BOOLEAN);\n\namount_cents > 100\n"
    ),
    "models/stg_orders.sql": (
        "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
        "SELECT id AS order_id, amount_cents, 'web  order' AS channel\n"
        'FROM __source("raw_orders")\n'
    ),
    "models/large_orders.sql": (
        "MODEL (description 'Test model large_orders.', materialized table);\n\n"
        "SELECT order_id, is_large_order(amount_cents) AS is_large\n"
        'FROM __ref("stg_orders")\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        QueryFingerprintEditE2ETestCase(
            description="sqb format edits are layout only",
            edits=(),
            format_flags=(),
            expected_format_summary="3 files checked, 2 changed",
            expected_reasons={"stg_orders": "no_change", "large_orders": "no_change"},
        ),
        QueryFingerprintEditE2ETestCase(
            description="an unedited project plans nothing after a build",
            edits=(),
            format_flags=("--check",),
            expected_format_summary="3 files checked, 2 changed",
            expected_reasons={"stg_orders": "no_change", "large_orders": "no_change"},
        ),
        QueryFingerprintEditE2ETestCase(
            description="comment and keyword case edits are unchanged",
            edits=(
                ("models/stg_orders.sql", "SELECT id", "-- Staged orders.\nselect id"),
                ("functions/sql/is_large_order.sql", "amount_cents > 100", "amount_cents  >  100"),
            ),
            format_flags=("--check",),
            expected_format_summary="3 files checked, 3 changed",
            expected_reasons={"stg_orders": "no_change", "large_orders": "no_change"},
        ),
        QueryFingerprintEditE2ETestCase(
            description="whitespace inside a string literal is a query change",
            edits=(("models/stg_orders.sql", "'web  order'", "'web order'"),),
            format_flags=("--check",),
            expected_format_summary="3 files checked, 2 changed",
            expected_reasons={"stg_orders": "query_changed"},
        ),
        QueryFingerprintEditE2ETestCase(
            description="identifier case is a query change",
            edits=(("models/large_orders.sql", "AS is_large", "AS Is_Large"),),
            format_flags=("--check",),
            expected_format_summary="3 files checked, 2 changed",
            expected_reasons={"stg_orders": "no_change", "large_orders": "query_changed"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_project_when_sql_is_edited_then_plan_follows_token_fingerprints(
    test_case: QueryFingerprintEditE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="fingerprint_orders", repo_files=_PROJECT_FILES
    )
    db_path: Path = project_dir / "fingerprint.duckdb"
    execute_duckdb(
        db_path=db_path,
        sql="CREATE TABLE raw_orders (id INTEGER, amount_cents INTEGER); "
        "INSERT INTO raw_orders VALUES (1, 50), (2, 500)",
    )
    built: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    stored: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql="SELECT node_name, from_base64(definition_b64), definition_hash FROM main._sqlbuild_fingerprints "
        "WHERE node_type = 'model' ORDER BY node_name",
    )
    for relative_path, before, after in test_case.edits:
        path: Path = project_dir / relative_path
        path.write_text(path.read_text(encoding="utf-8").replace(before, after), encoding="utf-8")
    formatted: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "format", *test_case.format_flags),
        project_dir=project_dir,
    )
    planned: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"), project_dir=project_dir
    )

    assert built.returncode == 0, built.stdout + built.stderr
    assert "format-unsafe" not in formatted.stdout + formatted.stderr
    assert test_case.expected_format_summary in formatted.stderr
    assert [str(row[0]) for row in stored] == ["large_orders", "stg_orders"]
    assert [row[2] for row in stored] == [
        compute_query_hash(query_sql=bytes(row[1]).decode(), dialect="duckdb") for row in stored
    ]
    assert planned.returncode == 0, planned.stdout + planned.stderr
    reasons: dict[str, str] = {
        str(entry["name"]): str(entry["reason"]) for entry in json.loads(planned.stdout)["models"]
    }
    assert {name: reasons[name] for name in test_case.expected_reasons} == (
        test_case.expected_reasons
    )
