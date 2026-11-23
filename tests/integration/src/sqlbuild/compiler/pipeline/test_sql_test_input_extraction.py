from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledDirectLogicSqlTestPayload,
    CompiledModelSqlTestPayload,
    build_compiled_project,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import ExpectedCountTestCase


@pytest.mark.parametrize(
    "test_case",
    [ExpectedCountTestCase(description="mixed test modes are preserved", expected_count=2)],
    ids=lambda case: case.description,
)
def test_given_expanded_mixed_mode_tests_when_compiling_project_then_complete_inputs_are_preserved(
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    test_case: ExpectedCountTestCase,
) -> None:
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS order_id\n",
            "sql/models/_constants/order_id.sql": "CONSTANT (name order_id, value 7);\n",
            "sql/functions/sql/increment.sql": (
                "FUNCTION (description 'Test function increment.', arguments (value INTEGER), returns INTEGER);\n\nvalue + 1\n"
            ),
            "tests/unit/a_orders.sql": (
                "TEST (name orders_match);\n\n"
                'WITH helper AS (SELECT @const("order_id") AS order_id),\n'
                "__ref__orders AS (SELECT order_id FROM helper),\n"
                "__expected__orders AS (SELECT order_id FROM helper),\n"
                "__assert__positive AS (SELECT order_id FROM helper WHERE order_id < 0)\n"
                "SELECT 1\n"
            ),
            "tests/unit/b_increment.sql": (
                "TEST (name increment_works, mode udf);\n\n"
                "WITH input AS (SELECT 7 AS value),\n"
                '__udf_actual__ AS (SELECT __udf("increment")(value) AS value FROM input),\n'
                "__udf_expected__ AS (SELECT 8 AS value)\n"
                "SELECT 1\n"
            ),
        },
    )

    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
    )

    assert len(project.sql_tests) == test_case.expected_count
    assert tuple(test.name for test in project.sql_tests) == ("orders_match", "increment_works")
    model_payload: CompiledModelSqlTestPayload | CompiledDirectLogicSqlTestPayload = (
        project.sql_tests[0].payload
    )
    direct_payload: CompiledModelSqlTestPayload | CompiledDirectLogicSqlTestPayload = (
        project.sql_tests[1].payload
    )
    assert isinstance(model_payload, CompiledModelSqlTestPayload)
    assert tuple(cte.name for cte in model_payload.authored_ctes) == ("helper", "__ref__orders")
    assert model_payload.expected_model_names == ("orders",)
    assert model_payload.assertion_names == ("positive",)
    assert "SELECT 7 AS order_id" in project.sql_tests[0].sql_body
    assert isinstance(direct_payload, CompiledDirectLogicSqlTestPayload)
    assert tuple(cte.name for cte in direct_payload.helper_ctes) == ("input",)
    assert direct_payload.actual_cte.name == "__udf_actual__"
    assert direct_payload.expected_cte.name == "__udf_expected__"
