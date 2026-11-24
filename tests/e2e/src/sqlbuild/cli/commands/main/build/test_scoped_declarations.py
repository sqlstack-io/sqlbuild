"""Real-CLI coverage of scoped audits, schemas, hooks, and seed audits on DuckDB."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ScopedDeclarationBuildE2ETestCase,
    ScopedDeclarationScopeE2ETestCase,
    SeedAuditBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    query_duckdb,
    run_sqb,
    table_exists,
)

_MART_MODEL: str = (
    "MODEL (\n"
    "  materialized table,\n"
    "  model_schema order_shape,\n"
    "  contract enforced,\n"
    "  audits [order_check (severity error)],\n"
    '  pre_hooks [sql("record_start")],\n'
    '  post_hooks [python("record_finish")],\n'
    ");\n\n"
    "SELECT CAST(1 AS INTEGER) AS order_id\n"
)
_SCOPED_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "scoped_shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "scoped_shop.duckdb"\n'
    ),
    "sql/models/marts/daily/orders.sql": _MART_MODEL,
    "sql/models/marts/weekly/orders_weekly.sql": _MART_MODEL,
    "sql/models/marts/_sqlbuild/schemas/order_shape.sql": (
        "SCHEMA (description 'Test schema.', name order_shape, columns (order_id (type INTEGER, nullable false)));\n"
    ),
    "sql/models/marts/_sqlbuild/audits/generic/order_check.sql": (
        'AUDIT ();\n\nSELECT order_id FROM __ref("@model") WHERE order_id IS NULL\n'
    ),
    "sql/models/marts/_sqlbuild/audits/singular/orders_match_weekly.sql": (
        'AUDIT (name "orders_match_weekly", severity error);\n\n'
        'SELECT d.order_id FROM __ref("orders") d\n'
        'LEFT JOIN __ref("orders_weekly") w USING (order_id)\n'
        "WHERE w.order_id IS NULL\n"
    ),
    "sql/models/marts/_sqlbuild/hooks/sql/record_start.sql": (
        "HOOK (description 'Test hook record_start.');\n\nINSERT INTO main.hook_log VALUES ('sql')\n"
    ),
    "sql/models/marts/_sqlbuild/hooks/python/lifecycle.py": (
        "from sqlbuild.hooks import hook\n\n\n"
        "@hook\n"
        "def record_finish(ctx):\n"
        "    '''Test hook record_finish.'''\n    ctx.execute_sql(\"INSERT INTO main.hook_log VALUES ('python')\")\n"
    ),
}
_SEED_YAML: str = (
    "seeds:\n"
    "  - name: order_codes\n    description: Test seed order_codes.\n"
    "    columns:\n"
    "      - name: order_id\n"
    "        type: INTEGER\n"
    "        audits:\n"
    "          - not_null:\n"
    "              severity: error\n"
    "      - name: label\n"
    "        type: VARCHAR\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ScopedDeclarationBuildE2ETestCase(
            description="scoped schema, hooks, generic and singular audits run in a build",
            expected_exit_code=0,
            expected_checks={
                ("order_check", "orders"): "pass",
                ("order_check", "orders_weekly"): "pass",
                ("orders_match_weekly", None): "pass",
            },
            expected_hook_rows=(("python", 2), ("sql", 2)),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_declarations_when_building_then_schema_hooks_and_audits_apply(
    test_case: ScopedDeclarationBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scoped_shop", repo_files=_SCOPED_PROJECT_FILES
    )
    execute_duckdb(
        db_path=project_dir / "scoped_shop.duckdb",
        sql="CREATE TABLE main.hook_log (kind VARCHAR)",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    checks: list[dict[str, Any]] = json.loads(result.stdout)["checks"]
    statuses: dict[tuple[object, object], object] = {
        (check["name"], check.get("asset_name")): check["status"] for check in checks
    }
    assert {key: statuses.get(key) for key in test_case.expected_checks} == (
        test_case.expected_checks
    )
    assert tuple(
        query_duckdb(
            db_path=project_dir / "scoped_shop.duckdb",
            sql="SELECT kind, COUNT(*) FROM main.hook_log GROUP BY kind ORDER BY kind",
        )
    ) == (test_case.expected_hook_rows)


@pytest.mark.parametrize(
    "test_case",
    [
        ScopedDeclarationScopeE2ETestCase(
            description="scope report lists scoped schema, hooks, and generic audit as used",
            target="sql/models/marts/daily/orders.sql",
            expected_fragments=(
                "audit:order_check  sql/models/marts/_sqlbuild/audits/generic/order_check.sql",
                "python_hook:record_finish  sql/models/marts/_sqlbuild/hooks/python/lifecycle.py",
                "schema:order_shape  sql/models/marts/_sqlbuild/schemas/order_shape.sql",
                "sql_hook:record_start  sql/models/marts/_sqlbuild/hooks/sql/record_start.sql",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scoped_declarations_when_running_scope_then_used_declarations_are_listed(
    test_case: ScopedDeclarationScopeE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="scoped_shop", repo_files=_SCOPED_PROJECT_FILES
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scope", test_case.target), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    used_section: str = result.stdout.split("Used", maxsplit=1)[1].split("Scope chain")[0]
    for fragment in test_case.expected_fragments:
        assert fragment in used_section


@pytest.mark.parametrize(
    "test_case",
    [
        SeedAuditBuildE2ETestCase(
            description="passing seed audit lets the dependent model build",
            seed_csv="order_id,label\n1,first\n2,second\n",
            expected_exit_code=0,
            expected_status="pass",
            expected_dependent_table=True,
        ),
        SeedAuditBuildE2ETestCase(
            description="failing error seed audit blocks the dependent model",
            seed_csv="order_id,label\n1,first\n,missing\n",
            expected_exit_code=1,
            expected_status="error",
            expected_dependent_table=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_seed_audit_when_building_then_it_runs_after_load_and_gates_dependants(
    test_case: SeedAuditBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="seed_audit_shop",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "seed_audit_shop"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "seed_audit_shop.duckdb"\n'
            ),
            "sql/seeds/order_codes.yml": _SEED_YAML,
            "sql/seeds/order_codes.csv": test_case.seed_csv,
            "sql/models/coded_orders.sql": (
                'MODEL (description "Test model coded_orders.", materialized table);\n\nSELECT order_id, label FROM __seed("order_codes")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--json"), project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    checks: list[dict[str, Any]] = json.loads(result.stdout)["checks"]
    outcomes: dict[tuple[object, object], tuple[object, object]] = {
        (check["name"], check.get("asset_name")): (check["attachment_kind"], check["status"])
        for check in checks
    }
    assert outcomes.get(("not_null", "order_codes")) == ("seed", test_case.expected_status)
    assert (
        table_exists(db_path=project_dir / "seed_audit_shop.duckdb", table_name="coded_orders")
        is test_case.expected_dependent_table
    )
