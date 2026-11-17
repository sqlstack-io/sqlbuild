"""Tests for resolving relation references written in model SQL hooks."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    Dialect,
    ModelPlanContext,
    PythonHookEntry,
    SourceEntry,
    SqlHookEntry,
    resolve_model_hook_entries,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve._test_types import (
    HookSqlResolutionTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.resolve.helpers import (
    build_cursor_intrinsic_model,
    build_empty_model_plan_context,
    build_target,
)

_PYTHON_HOOK: PythonHookEntry = PythonHookEntry(name="refresh_lookup", kwargs={})
_CONTEXT: ModelPlanContext = build_empty_model_plan_context(
    model_locations={"orders": build_target("staging.orders", "orders")},
    seed_locations={"country_codes": build_target("seeds.country_codes", "country_codes")},
    source_map={"raw_orders": SourceEntry(name="raw_orders", schema="raw_prod", table="orders")},
)


@pytest.mark.parametrize(
    "test_case",
    [
        HookSqlResolutionTestCase(
            description="inline hook model reference resolves to the planned location",
            pre_hooks=[SqlHookEntry(statement='DELETE FROM __ref("orders")'), _PYTHON_HOOK],
            post_hooks=None,
            expected_pre_hooks=[SqlHookEntry(statement="DELETE FROM staging.orders"), _PYTHON_HOOK],
            expected_post_hooks=None,
        ),
        HookSqlResolutionTestCase(
            description="named hook source and seed references use the planned read entries",
            pre_hooks=None,
            post_hooks=(
                SqlHookEntry(
                    statement='INSERT INTO t SELECT * FROM __source("raw_orders"), '
                    '__seed("country_codes")',
                    name="record_rows",
                ),
            ),
            expected_pre_hooks=None,
            expected_post_hooks=(
                SqlHookEntry(
                    statement="INSERT INTO t SELECT * FROM raw_prod.orders, seeds.country_codes",
                    name="record_rows",
                ),
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_hook_references_when_planning_then_they_resolve_like_model_sql(
    test_case: HookSqlResolutionTestCase,
) -> None:
    model: CompiledModelView = build_cursor_intrinsic_model(
        config_values={"pre_hooks": test_case.pre_hooks, "post_hooks": test_case.post_hooks}
    )

    pre_hooks, post_hooks = resolve_model_hook_entries(
        model=model,
        dialect=Dialect(DuckDbAdapter()),
        context=_CONTEXT,
        external_sql_reference_resolver=None,
    )

    assert (pre_hooks, post_hooks) == (
        test_case.expected_pre_hooks,
        test_case.expected_post_hooks,
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
