from __future__ import annotations

import pickle
from dataclasses import replace

import pytest

from sqlbuild._native import (
    PlannerPolicies,
    PlannerScope,
    PlannerScopeResolution,
    PlannerSelection,
    resolve_planner_scopes,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import build_test_project
from tests.unit.src.sqlbuild.compiler.planner._helpers.planning._test_types import (
    PlannerScopesTestCase,
)

_MODEL_DEPS: dict[str, tuple[str, ...]] = {
    "stg_orders": (),
    "stg_customers": (),
    "orders_enriched": ("stg_orders", "stg_customers"),
    "orders_summary": ("orders_enriched",),
    "orders_report": ("orders_summary",),
    "inventory": (),
}


@pytest.mark.parametrize(
    "test_case",
    [
        PlannerScopesTestCase(
            description="stale warning scope is the selection plus all upstreams",
            model_deps=_MODEL_DEPS,
            select=("orders_summary",),
            expected_selected_names=frozenset({"orders_summary"}),
            expected_stale_warning_names=(
                "orders_enriched",
                "orders_summary",
                "stg_customers",
                "stg_orders",
            ),
        ),
        PlannerScopesTestCase(
            description="stale warning scope unions upstreams of every selected model",
            model_deps=_MODEL_DEPS,
            select=("orders_enriched", "inventory"),
            expected_selected_names=frozenset({"orders_enriched", "inventory"}),
            expected_stale_warning_names=(
                "inventory",
                "orders_enriched",
                "stg_customers",
                "stg_orders",
            ),
        ),
        PlannerScopesTestCase(
            description="stale warning scope of a root model is the model itself",
            model_deps=_MODEL_DEPS,
            select=("stg_orders",),
            expected_selected_names=frozenset({"stg_orders"}),
            expected_stale_warning_names=("stg_orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selection_when_resolving_scopes_then_stale_warning_scope_is_upstream_closure(
    test_case: PlannerScopesTestCase,
) -> None:
    project: CompiledProjectView = build_test_project(model_deps=test_case.model_deps)

    resolution: PlannerScopeResolution = resolve_planner_scopes(
        project=project,
        selection=PlannerSelection(select=test_case.select),
        policies=PlannerPolicies(),
    )

    selected_names: frozenset[str] = frozenset(
        key.name for key in resolution.selected_scope.selected_keys
    )
    stale_scope_names: tuple[str, ...] = tuple(sorted(resolution.stale_warning_scope.all_keys))
    stale_order_names: tuple[str, ...] = tuple(
        key.name for key in resolution.stale_warning_scope.execution_order
    )
    assert selected_names == test_case.expected_selected_names
    assert resolution.stale_warning_scope.selected_keys == resolution.selected_scope.selected_keys
    assert stale_scope_names == test_case.expected_stale_warning_names
    assert tuple(sorted(stale_order_names)) == test_case.expected_stale_warning_names
    assert resolution.inspection_scope is resolution.selected_scope
    assert isinstance(resolution.selected_scope, PlannerScope)
    assert tuple(resolution.selected_scope.models_by_name) == tuple(test_case.model_deps)
    assert set(resolution.stale_warning_scope.models_by_name) == set(
        test_case.expected_stale_warning_names
    )
    assert replace(resolution.selected_scope, load_sources=True).load_sources
    assert pickle.loads(pickle.dumps(resolution.selected_scope)) == resolution.selected_scope
