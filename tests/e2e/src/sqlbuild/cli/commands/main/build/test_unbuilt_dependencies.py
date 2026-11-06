"""E2E coverage for readers of resources a build failed to build."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    HookSkippedAuditBuildE2ETestCase,
    UnbuiltDependencyBuildE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "unbuilt_dependencies"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n\n[defaults]\nmaterialized = "table"\n'
    ),
    "models/broken_orders.sql": (
        "MODEL (description 'Test model broken_orders.');\n"
        "SELECT CAST('not a number' AS INTEGER) AS order_id\n"
    ),
    "models/customers.sql": (
        "MODEL (description 'Test model customers.');\nSELECT 1 AS customer_id\n"
    ),
}

_HOOK_FILE: str = (
    "from sqlbuild.hooks import hook\n\n\n"
    "@hook\ndef skip_model(ctx):\n"
    "    '''Test hook skip_model.'''\n"
    "    return ctx.skip(reason='paused')\n"
)
_CUSTOMERS_SQL: str = "SELECT 1 AS customer_id\n"


@pytest.mark.parametrize(
    "test_case",
    [
        UnbuiltDependencyBuildE2ETestCase(
            description="end-of-build audit over a failed model is skipped",
            repo_files={
                **_BASE_FILES,
                "audits/singular/orders_have_customers.sql": (
                    "AUDIT ();\n"
                    'SELECT o.order_id FROM __ref("broken_orders") AS o\n'
                    'LEFT JOIN __ref("customers") AS c ON c.customer_id = o.order_id\n'
                    "WHERE c.customer_id IS NULL\n"
                ),
            },
            expected_stdout_fragments=(
                "broken_orders",
                "Could not convert string",
                "audit     orders_have_customers SKIP",
                "Skipped (1): orders_have_customers (audit, reads broken_orders, which was not "
                "built)",
                "SKIP=1",
            ),
            unexpected_stdout_fragments=("does not exist",),
        ),
        UnbuiltDependencyBuildE2ETestCase(
            description="full refresh on a fresh warehouse skips the audit over a failed model",
            repo_files={
                **_BASE_FILES,
                "audits/singular/orders_have_customers.sql": (
                    "AUDIT ();\n"
                    'SELECT o.order_id FROM __ref("broken_orders") AS o\n'
                    'LEFT JOIN __ref("customers") AS c ON c.customer_id = o.order_id\n'
                    "WHERE c.customer_id IS NULL\n"
                ),
            },
            expected_stdout_fragments=(
                "audit     orders_have_customers SKIP",
                "Skipped (1): orders_have_customers (audit, reads broken_orders, which was not "
                "built)",
            ),
            unexpected_stdout_fragments=("does not exist",),
            build_arguments=("--full-refresh",),
        ),
        UnbuiltDependencyBuildE2ETestCase(
            description="test calling a skipped table function is skipped",
            repo_files={
                **_BASE_FILES,
                "functions/sql/customer_orders.sql": (
                    'FUNCTION (description "Test function customer_orders.",\n'
                    "  arguments (p_customer_id INTEGER), returns table (order_id INTEGER));\n"
                    'SELECT o.order_id FROM __ref("broken_orders") AS o\n'
                    "WHERE o.order_id = p_customer_id\n"
                ),
                "models/summary.sql": (
                    "MODEL (description 'Test model summary.');\n"
                    "SELECT c.customer_id, f.order_id\n"
                    'FROM __ref("customers") AS c\n'
                    '  CROSS JOIN __table_fn("customer_orders")(c.customer_id) AS f\n'
                ),
                "tests/unit/test_summary.sql": (
                    "TEST();\nWITH\n__ref__customers AS (SELECT 1 AS customer_id),\n"
                    "__expected__summary AS (SELECT 1 AS customer_id, 1 AS order_id WHERE FALSE)\n"
                    "SELECT 1\n"
                ),
            },
            expected_stdout_fragments=("Skipped (2): summary, customer_orders",),
            unexpected_stdout_fragments=("test_summary", "does not exist"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failed_model_when_building_then_skips_readers_of_unbuilt_resources(
    test_case: UnbuiltDependencyBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="unbuilt_dependencies", repo_files=test_case.repo_files
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", *test_case.build_arguments), project_dir=project_dir
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_stdout_fragments)
    assert not any(fragment in result.stdout for fragment in test_case.unexpected_stdout_fragments)


@pytest.mark.parametrize(
    "test_case",
    [
        HookSkippedAuditBuildE2ETestCase(
            description="end-of-build audit over a model a hook skipped still runs",
            hooked_model=(
                "MODEL (description 'Test model customers.', "
                'pre_hooks [python("skip_model")]);\n' + _CUSTOMERS_SQL
            ),
            expected_exit_code=0,
            expected_stdout_fragments=("audit     customers_exist PASS",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_skipped_existing_model_when_rebuilding_then_its_end_audits_run(
    test_case: HookSkippedAuditBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="unbuilt_dependencies",
        repo_files={
            "sqlbuild_project.toml": _BASE_FILES["sqlbuild_project.toml"],
            "models/customers.sql": "MODEL (description 'Test model customers.');\n"
            + _CUSTOMERS_SQL,
            "models/orders.sql": "MODEL (description 'Test model orders.');\nSELECT 1 AS order_id\n",
            "audits/singular/customers_exist.sql": (
                "AUDIT ();\n"
                'SELECT o.order_id FROM __ref("orders") AS o\n'
                'LEFT JOIN __ref("customers") AS c ON c.customer_id = o.order_id\n'
                "WHERE c.customer_id IS NULL\n"
            ),
        },
    )
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )
    assert first.returncode == 0, first.stdout + first.stderr
    (project_dir / "hooks/python").mkdir(parents=True)
    _ = (project_dir / "hooks/python/skips.py").write_text(_HOOK_FILE)
    _ = (project_dir / "models/customers.sql").write_text(test_case.hooked_model)

    rebuilt: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project_dir
    )

    assert rebuilt.returncode == test_case.expected_exit_code, rebuilt.stdout + rebuilt.stderr
    assert all(fragment in rebuilt.stdout for fragment in test_case.expected_stdout_fragments)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
