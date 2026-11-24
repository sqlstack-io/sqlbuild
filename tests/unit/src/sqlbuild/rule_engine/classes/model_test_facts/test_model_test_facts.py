from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledSqlTest,
    Model,
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledProjectView, SqlTestMode
from sqlbuild.rule_engine.classes.model_test_facts import TestFacts as ModelTestFacts
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import (
    direct_sql_test,
    model_sql_test,
)
from tests.unit.src.sqlbuild.rule_engine.classes.model_test_facts._test_types import (
    ModelTestFactsTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    (
        ModelTestFactsTestCase(
            description="matching and unrelated SQL tests",
            model_name="orders",
            other_model_name="customers",
            direct_test_name="order_status",
            expected_matching_names=("orders: keeps paid orders",),
            expected_all_names=("orders: keeps paid orders", "customers", "order_status"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_and_non_model_tests_when_querying_model_then_returns_each_matching_test_once(
    test_case: ModelTestFactsTestCase,
) -> None:
    matching: CompiledSqlTest = replace(
        model_sql_test(), target_model_names=(test_case.model_name, test_case.model_name)
    )
    other_model: CompiledSqlTest = replace(
        model_sql_test(),
        name=test_case.other_model_name,
        target_model_names=(test_case.other_model_name,),
    )
    direct: CompiledSqlTest = direct_sql_test(
        mode=SqlTestMode.UDF, name=test_case.direct_test_name, block_index=1
    )
    project: CompiledProjectView = project_with(
        project=build_project(
            name=test_case.model_name,
            relative_path="sql/models/orders.sql",
            sql="SELECT 1 AS order_id",
            config_values={},
        ),
        sql_tests=(matching, other_model, direct),
    )

    facts: ModelTestFacts = ModelTestFacts(project=project)

    matching_tests: tuple[CompiledSqlTest, ...] = facts.for_model(
        Model(name=test_case.model_name, path=Path("sql/models/orders.sql"), materialization="view")
    )
    assert tuple(test.name for test in matching_tests) == test_case.expected_matching_names
    assert tuple(test.name for test in facts.all()) == test_case.expected_all_names


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
