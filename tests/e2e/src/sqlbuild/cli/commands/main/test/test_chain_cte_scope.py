"""Chained model CTEs retain their lexical scope through the real DuckDB CLI."""

import re
from pathlib import Path
from subprocess import CompletedProcess

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.test._test_types import ChainCteScopeTestCase
from tests.e2e.src.sqlbuild.cli.commands.main.test.helpers import build_cte_scope_project_files
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_RELATION_MARKER: re.Pattern[str] = re.compile(r'__(ref|source)\("(\w+)"\)')

_FIRST: str = (
    'WITH final AS (SELECT order_id FROM __source("raw_orders")) SELECT final.order_id FROM final'
)
_SECOND: str = (
    'WITH final AS (SELECT order_id + 1 AS order_id FROM __ref("orders_0")) '
    "SELECT final.order_id FROM final"
)


@pytest.mark.parametrize(
    "test_case",
    (
        ChainCteScopeTestCase("two final CTEs", (_FIRST, _SECOND), "SELECT 2 AS order_id"),
        ChainCteScopeTestCase(
            "three final CTEs",
            (
                _FIRST,
                _SECOND,
                'WITH final AS (SELECT order_id + 1 AS order_id FROM __ref("orders_1")) '
                "SELECT final.order_id FROM final",
            ),
            "SELECT 3 AS order_id",
        ),
        ChainCteScopeTestCase(
            "shared non-final CTEs with different shapes",
            (
                'WITH staged AS (SELECT order_id FROM __source("raw_orders")), '
                "final AS (SELECT order_id FROM staged) SELECT order_id FROM final",
                'WITH staged AS (SELECT order_id + 10 AS customer_id FROM __ref("orders_0")), '
                "final AS (SELECT customer_id FROM staged) SELECT customer_id FROM final",
            ),
            "SELECT 11 AS customer_id",
        ),
        ChainCteScopeTestCase(
            "nested shadowing and qualified stars",
            (
                _FIRST,
                'WITH "Final" AS (SELECT order_id + 1 AS order_id FROM __ref("orders_0")), '
                'nested AS (WITH "Final" AS (SELECT 7 AS extra) SELECT "Final".* FROM "Final") '
                'SELECT f.order_id + n.extra AS order_id FROM "Final" f CROSS JOIN nested n',
            ),
            "SELECT 9 AS order_id",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_overlapping_ctes_when_testing_chain_then_each_model_uses_its_own_rows(
    tmp_path: Path, test_case: ChainCteScopeTestCase
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="cte_scope",
        repo_files=build_cte_scope_project_files(
            queries=test_case.queries, expected=test_case.expected_sql
        ),
    )
    result: CompletedProcess[str] = run_sqb(command=("--no-color", "test"), project_dir=project)
    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    compiled: CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    sql: str = next(
        (project / "target" / "compiled" / "tests").rglob("order_chain.sql")
    ).read_text()
    assert "__sqb_cte_" not in sql
    upstream_queries: tuple[str, ...] = tuple(
        _RELATION_MARKER.sub(r"__\1__\2", query) for query in test_case.queries[:-1]
    )
    assert all(f"AS ({query})" in sql for query in upstream_queries), sql


@pytest.mark.parametrize(
    "test_case",
    (
        ChainCteScopeTestCase(
            "upstream rows must not pass downstream expectation",
            (_FIRST, _SECOND),
            "SELECT 1 AS order_id",
            1,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_same_shape_wrong_upstream_rows_when_testing_chain_then_reports_authored_difference(
    tmp_path: Path,
    test_case: ChainCteScopeTestCase,
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="wrong_scope",
        repo_files=build_cte_scope_project_files(
            queries=test_case.queries, expected=test_case.expected_sql
        ),
    )
    result: CompletedProcess[str] = run_sqb(command=("--no-color", "test"), project_dir=project)
    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    assert "FAIL=1" in output
    assert "orders_1" in output
    assert (
        "expected rows of orders_1 differ: 1 missing, 1 extra (1 expected, 1 actual)" in output
    ), output
    assert "order_id: expected 1, actual 2" not in output, output
    assert "__sqb_cte_" not in output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
