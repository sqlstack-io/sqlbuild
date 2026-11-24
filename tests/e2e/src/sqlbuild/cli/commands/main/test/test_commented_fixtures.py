"""E2E coverage for comments around set-operation branches in SQL-test fixtures."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import (
    CommentedFixtureE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
    run_sqb,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "commented_fixtures"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "commented_fixtures.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n'
    ),
    "sql/macros/python/status.py": (
        'def normalize_status(value: str) -> str:\n    return f"LOWER(TRIM({value}))"\n'
    ),
    "sql/sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: status\n        type: VARCHAR\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Test model orders.');\n\n"
        'SELECT order_id, @py.normalize_status("status") AS status FROM __source("raw_orders")\n'
    ),
    "sql/models/order_counts.sql": (
        "MODEL (description 'Test model order_counts.');\n\n"
        'SELECT status, COUNT(*) AS order_count FROM __ref("orders") GROUP BY status\n'
    ),
}
_LINE_COMMENTED_SOURCE: str = (
    "__source__raw_orders AS (\n"
    "  -- first order\n"
    "  SELECT 1 AS order_id, ' PAID ' AS status -- trailing note\n"
    "  -- before the union\n"
    "  UNION ALL\n"
    "  -- second order\n"
    "  SELECT 2 AS order_id, 'void' AS status\n"
    "  -- after the last branch\n"
    "),\n"
)
_LINE_COMMENTED_EXPECTED: str = (
    "__expected__orders AS (\n"
    "  SELECT 1 AS order_id, 'paid' AS status -- paid order\n"
    "  -- the voided order follows\n"
    "  UNION ALL\n"
    "  SELECT 2 AS order_id, 'void' AS status -- '--' inside a comment\n"
    ")\nSELECT 1\n"
)
_BLOCK_COMMENTED_SOURCE: str = (
    "__source__raw_orders AS (\n"
    "  /* first order */ SELECT 1 AS order_id, ' PAID ' AS status /* note */\n"
    "  UNION ALL /* second order */\n"
    "  SELECT 2 AS order_id, 'void' AS status\n"
    "),\n"
)
_BLOCK_COMMENTED_EXPECTED: str = (
    "__expected__orders AS (\n"
    "  SELECT 1 AS order_id, 'paid' AS status\n"
    "  /* nested /* UNION ALL SELECT 9 AS other_id */ still a comment */\n"
    "  UNION ALL\n"
    "  /* voided */ SELECT 2 AS order_id, 'void' AS status /* last */\n"
    ")\nSELECT 1\n"
)
_MISMATCH_FRAGMENT: str = (
    "must use the same __expected__orders projection names and order in every "
    "set-operation branch; branch 2 does not match branch 1"
)


@pytest.mark.parametrize(
    "test_case",
    [
        CommentedFixtureE2ETestCase(
            description="bounded CTE casts joins and unions execute with mocked sources",
            test_files={
                "sql/macros/python/status.py": "",
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders through projected CTEs.');\n"
                    'WITH imported AS (SELECT * FROM __source("raw_orders")),\n'
                    "projected AS (SELECT CAST(COALESCE(b.order_id, 0) + 0 AS INTEGER) AS order_id,\n"
                    "b.status AS status FROM imported AS b\n"
                    "LEFT JOIN imported AS c ON c.order_id = b.order_id),\n"
                    "combined AS (SELECT order_id, status FROM projected\n"
                    "UNION ALL BY NAME SELECT order_id, status FROM projected WHERE order_id = 0),\n"
                    "final AS (SELECT CAST(order_id AS INTEGER) AS order_id, status FROM combined)\n"
                    "SELECT order_id, status FROM final\n"
                ),
                "tests/unit/test_orders.sql": (
                    'TEST (name "projected_orders");\nWITH\n'
                    "__source__raw_orders AS (\n"
                    "SELECT 1 AS order_id, 'paid' AS status UNION ALL SELECT 2, 'void'\n"
                    "),\n" + _LINE_COMMENTED_EXPECTED
                ),
            },
            expected_exit_code=0,
            expected_output_fragments=("PASS=1", "FAIL=0"),
        ),
        CommentedFixtureE2ETestCase(
            description="nested model CTEs and marker lookalikes preserve mocked source results",
            test_files={
                "sql/macros/python/status.py": "",
                "sql/models/_macros/python/status.py": _PROJECT_FILES[
                    "sql/macros/python/status.py"
                ],
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders through nested CTEs.');\n"
                    'WITH "recent" AS (\n'
                    "  WITH nested AS (SELECT order_id, status, '__ref(\"ignored\")''s' AS note\n"
                    '    FROM __source("raw_orders"))\n'
                    "  SELECT order_id, status FROM nested\n"
                    ")\n"
                    'SELECT order_id, @py.normalize_status("status") AS status\n'
                    'FROM "recent" /* __seed("ignored") */\n'
                ),
                "tests/unit/test_orders.sql": (
                    'TEST (name "nested_orders");\nWITH\n'
                    + _LINE_COMMENTED_SOURCE
                    + _LINE_COMMENTED_EXPECTED
                ),
                "tests/unit/test_order_counts.sql": (
                    'TEST (name "nested_order_counts");\nWITH\n'
                    + _LINE_COMMENTED_SOURCE
                    + "__expected__order_counts AS (\n"
                    "  SELECT 'paid' AS status, CAST(1 AS BIGINT) AS order_count\n"
                    "  UNION ALL SELECT 'void' AS status, CAST(1 AS BIGINT) AS order_count\n"
                    ") SELECT 1\n"
                ),
            },
            expected_exit_code=0,
            expected_output_fragments=("PASS=2", "FAIL=0"),
        ),
        CommentedFixtureE2ETestCase(
            description="line and block comments around fixture branches pass",
            test_files={
                "tests/unit/test_orders_line_comments.sql": (
                    'TEST (name "orders_line_comments");\n\nWITH\n'
                    + _LINE_COMMENTED_SOURCE
                    + _LINE_COMMENTED_EXPECTED
                ),
                "tests/unit/test_orders_block_comments.sql": (
                    'TEST (name "orders_block_comments");\n\nWITH\n'
                    + _BLOCK_COMMENTED_SOURCE
                    + _BLOCK_COMMENTED_EXPECTED
                ),
                "tests/unit/test_order_counts_commented_ref.sql": (
                    'TEST (name "order_counts_commented_ref");\n\nWITH\n'
                    "__ref__orders AS (\n"
                    "  SELECT 1 AS order_id, 'paid' AS status -- first\n"
                    "  /* between */ UNION ALL -- second\n"
                    "  SELECT 2 AS order_id, 'paid' AS status\n"
                    "),\n"
                    "__expected__order_counts AS (\n"
                    "  SELECT 'paid' AS status, CAST(2 AS BIGINT) AS order_count -- only row\n"
                    ")\nSELECT 1\n"
                ),
                "tests/unit/test_normalize_status_commented.sql": (
                    'TEST (mode macro, name "normalizes_status_commented");\n\nWITH\n'
                    "input_values AS (SELECT '  PAID  ' AS raw_status),\n"
                    "__macro_actual__ AS (\n"
                    '  SELECT @py.normalize_status("raw_status") AS status FROM input_values\n'
                    "),\n"
                    "__macro_expected__ AS (\n"
                    "  SELECT 'paid' AS status -- normalized\n"
                    "  -- duplicates collapse\n"
                    "  INTERSECT DISTINCT SELECT 'paid' AS status\n"
                    ")\nSELECT 1\n"
                ),
            },
            expected_exit_code=0,
            expected_output_fragments=("PASS=4", "FAIL=0"),
        ),
        CommentedFixtureE2ETestCase(
            description="line-commented branch mismatch still fails",
            test_files={
                "tests/unit/test_orders_mismatch.sql": (
                    'TEST (name "orders_mismatch");\n\nWITH\n'
                    + _LINE_COMMENTED_SOURCE
                    + "__expected__orders AS (\n"
                    "  SELECT 1 AS order_id, 'paid' AS status -- paid order\n"
                    "  UNION ALL\n"
                    "  SELECT 2 AS order_id, 'void' AS order_status -- renamed\n"
                    ")\nSELECT 1\n"
                ),
            },
            expected_exit_code=1,
            expected_output_fragments=("P001", _MISMATCH_FRAGMENT),
        ),
        CommentedFixtureE2ETestCase(
            description="block-commented branch mismatch still fails",
            test_files={
                "tests/unit/test_orders_mismatch.sql": (
                    'TEST (name "orders_mismatch");\n\nWITH\n'
                    + _BLOCK_COMMENTED_SOURCE
                    + "__expected__orders AS (\n"
                    "  SELECT 1 AS order_id, 'paid' AS status /* paid */\n"
                    "  UNION ALL /* voided */\n"
                    "  SELECT 'void' AS status, 2 AS order_id\n"
                    ")\nSELECT 1\n"
                ),
            },
            expected_exit_code=1,
            expected_output_fragments=("P001", _MISMATCH_FRAGMENT),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_commented_fixture_branches_when_testing_then_comments_are_not_projections(
    test_case: CommentedFixtureE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="commented_fixtures",
        repo_files={**_PROJECT_FILES, **test_case.test_files},
    )
    execute_duckdb(
        db_path=project_dir / "commented_fixtures.duckdb",
        sql="CREATE TABLE raw_orders AS SELECT 1 AS order_id, 'paid' AS status",
    )

    tested: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "test"), project_dir=project_dir
    )

    output: str = tested.stdout + tested.stderr
    assert tested.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_output_fragments:
        assert fragment in output, output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
