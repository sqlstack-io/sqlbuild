from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    CompiledSqlScenario,
    CompiledSqlTest,
    DiscoveredSqlScenarioFile,
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledProjectView, SqlTestMode
from tests.unit.src.sqlbuild.rule_engine._helpers.engine._test_types import (
    NativeFactPayloadTestCase,
)
from tests.unit.src.sqlbuild.rule_engine._helpers.engine.helpers import (
    captured_native_request,
    direct_sql_test,
    model_sql_test,
    scenario_summary,
    sql_test_summary,
)
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    [
        NativeFactPayloadTestCase(
            description="complete deterministic safe SQL test and scenario facts",
            expected_test_count=4,
            expected_scenario_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_sql_facts_when_evaluating_native_then_exact_safe_rows_are_sent(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    test_case: NativeFactPayloadTestCase,
) -> None:
    project: CompiledProjectView = build_project(
        name="orders",
        relative_path="sql/models/orders.sql",
        sql="SELECT 1",
        config_values={},
    )
    direct_tests: tuple[CompiledSqlTest, ...] = tuple(
        direct_sql_test(mode=mode, name=name, block_index=index)
        for index, (mode, name) in enumerate(
            (
                (SqlTestMode.MACRO, "normalize"),
                (SqlTestMode.UDF, "tax"),
                (SqlTestMode.TABLE_FN, "items"),
            ),
            start=1,
        )
    )
    sql_tests: tuple[CompiledSqlTest, ...] = (*direct_tests, model_sql_test())
    scenario_file: DiscoveredSqlScenarioFile = DiscoveredSqlScenarioFile(
        file_path=Path("/private/project/tests/scenarios/orders.sql"),
        relative_path=Path("tests/scenarios/orders.sql"),
        contents="secret scenario fixture",
        header_values={"description": "orders remain valid", "tags": ["safe"]},
        sql_body="SELECT 'secret scenario fixture'",
        name="orders",
    )
    scenario: CompiledSqlScenario = CompiledSqlScenario(
        key=CompiledObjectKey(CompiledResourceType.SQL_SCENARIO, "orders"),
        name="orders",
        scenario_file=scenario_file,
        sql_body=scenario_file.sql_body,
        expected_model_names=("orders",),
        assertion_names=("positive",),
        assertion_target_model_names=("payments",),
        target_model_names=("orders", "payments"),
    )

    request: dict[str, Any] = captured_native_request(
        project=project_with(project=project, sql_tests=sql_tests, sql_scenarios=(scenario,)),
        project_dir=tmp_path,
    )

    assert len(request["sql_tests"]) == test_case.expected_test_count
    assert [sql_test_summary(fact) for fact in request["sql_tests"][:3]] == [
        (
            f"tests/unit/{name}.sql",
            "tests/unit",
            index,
            name,
            name,
            mode,
            ([], [], [], []),
            [(mode, name)],
            ([], [], [], False, False),
            None,
        )
        for index, (mode, name) in enumerate(
            (("macro", "normalize"), ("udf", "tax"), ("table_fn", "items")), start=1
        )
    ]
    assert sql_test_summary(request["sql_tests"][3]) == (
        "tests/unit/test_orders__keeps_paid.sql",
        "tests/unit",
        1,
        "orders: keeps paid orders",
        "orders: keeps paid orders",
        "model",
        (["orders"], ["paid"], ["orders"], ["orders"]),
        [],
        ([], [], [], False, False),
        None,
    )
    assert len(request["sql_scenarios"]) == test_case.expected_scenario_count
    assert [scenario_summary(fact) for fact in request["sql_scenarios"]] == [
        (
            "tests/scenarios/orders.sql",
            "tests/scenarios",
            "orders",
            "orders remain valid",
            ["orders"],
            ["positive"],
            ["payments"],
            ["orders", "payments"],
        )
    ]
    assert "/private/project" not in str(request["sql_tests"] + request["sql_scenarios"])
    assert "secret fixture value" not in str(request["sql_tests"] + request["sql_scenarios"])


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
