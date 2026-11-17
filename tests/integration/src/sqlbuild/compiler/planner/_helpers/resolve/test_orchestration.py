"""Execute compiled model, function and hook SQL with custom adapter overrides."""

from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    BackfillAction,
    BackfillResult,
    CursorOverridePair,
    Dialect,
    ModelPlanContext,
    SqlHookEntry,
    WarehouseSnapshot,
    build_function_locations,
    build_model_locations,
    build_seed_locations,
    resolve_function_sql,
    resolve_model_hook_entries,
    resolve_model_sql,
)
from sqlbuild.compiler.compile.types import CompiledModelView, CompiledProjectView
from tests.integration.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    OrchestrationTestCase,
)
from tests.integration.src.sqlbuild.compiler.planner._helpers.resolve.helpers import (
    OrchestrationOverrideAdapter,
    compile_resolution_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        OrchestrationTestCase(
            description="compiled aliases and function dependencies honor adapter rendering",
            expected_rows=[(9,)],
            expected_model_calls=[
                "relation:staging.orders_alias",
                "function:staging.order_lookup(9)",
            ],
            expected_relation_calls=["relation:staging.orders_alias"],
        )
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_dependencies_when_resolving_then_adapter_overrides_execute_in_order(
    test_case: OrchestrationTestCase, tmp_path: Path, connection: Any
) -> None:
    project: CompiledProjectView = compile_resolution_project(tmp_path)
    model: CompiledModelView = {model.name: model for model in project.models}["report"]
    adapter: OrchestrationOverrideAdapter = OrchestrationOverrideAdapter()
    context: ModelPlanContext = ModelPlanContext(
        model_locations=build_model_locations(project.models),
        models_by_name={model.name: model for model in project.models},
        seed_locations=build_seed_locations(project.seeds),
        function_locations=build_function_locations(project.functions),
        source_map={},
        source_warehouse_columns={},
        star_exclude_keyword="EXCLUDE",
    )
    assert not project.diagnostics
    assert context.model_locations["orders"].name == "orders_alias"
    context.function_locations["order_lookup"] = replace(
        context.function_locations["order_lookup"], qualified_name="staging.order_lookup"
    )
    connection.execute("CREATE TABLE staging.actual_orders AS SELECT 9 AS order_id")
    sql: str = resolve_model_sql(
        dialect=Dialect(adapter),
        model=model,
        snapshot=WarehouseSnapshot(existing_relations={}),
        context=context,
        backfill=BackfillResult(action=BackfillAction.FORWARD_ONLY),
        full_refresh=False,
        cursor_overrides=CursorOverridePair(),
    )
    assert connection.execute(sql).fetchall() == test_case.expected_rows
    assert adapter.calls == test_case.expected_model_calls
    adapter.calls.clear()
    function_sql: str = resolve_function_sql(
        dialect=Dialect(adapter),
        function=project.functions[0],
        model_locations=context.model_locations,
        seed_locations=context.seed_locations,
        function_locations=context.function_locations,
        source_map={},
        source_warehouse_columns={},
        star_exclude_keyword="EXCLUDE",
    )
    connection.execute(f"CREATE MACRO order_lookup(id) AS TABLE {function_sql}")
    assert connection.execute("SELECT * FROM order_lookup(9)").fetchall() == test_case.expected_rows
    assert adapter.calls == test_case.expected_relation_calls
    adapter.calls.clear()
    pre_hooks, post_hooks = resolve_model_hook_entries(
        model=model, dialect=Dialect(adapter), context=context, external_sql_reference_resolver=None
    )
    assert isinstance(pre_hooks, tuple | list)
    assert len(pre_hooks) == 1
    assert isinstance(pre_hooks[0], SqlHookEntry)
    assert connection.execute(pre_hooks[0].statement).fetchall() == test_case.expected_rows
    assert adapter.calls == test_case.expected_relation_calls
    assert not post_hooks


if __name__ == "__main__":
    pytest.main([__file__])
