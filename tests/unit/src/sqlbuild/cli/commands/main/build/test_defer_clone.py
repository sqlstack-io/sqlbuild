from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    PlannerScope,
    resolve_defer_clone_selection,
)
from sqlbuild.compiler.compile.types import CompiledModelView
from tests.unit.src.sqlbuild.cli.commands.main.build._test_types import (
    DeferCloneBoundaryTestCase,
    DeferCloneModelCountTestCase,
    FunctionDeferCloneBoundaryTestCase,
)
from tests.unit.src.sqlbuild.cli.commands.main.build.helpers import (
    build_compiled_object_key,
)

MODEL_A: CompiledObjectKey = build_compiled_object_key(CompiledResourceType.MODEL, "a")
MODEL_B: CompiledObjectKey = build_compiled_object_key(CompiledResourceType.MODEL, "b")
MODEL_C: CompiledObjectKey = build_compiled_object_key(CompiledResourceType.MODEL, "c")
SEED_COUNTRIES: CompiledObjectKey = build_compiled_object_key(
    CompiledResourceType.SEED, "countries"
)
SOURCE_RAW: CompiledObjectKey = build_compiled_object_key(CompiledResourceType.SOURCE, "raw_orders")
FUNCTION_ADD_COUNTRY: CompiledObjectKey = build_compiled_object_key(
    CompiledResourceType.UDF, "add_country"
)


@pytest.mark.parametrize(
    "test_case",
    [
        DeferCloneBoundaryTestCase(
            description="clones the first non-view ancestor and stops",
            selected_keys=frozenset({MODEL_C}),
            upstream_deps={
                MODEL_C: (MODEL_B, SOURCE_RAW),
                MODEL_B: (MODEL_A, SEED_COUNTRIES),
                MODEL_A: (),
                SEED_COUNTRIES: (),
                SOURCE_RAW: (),
            },
            expected_selectors=("b",),
        ),
        DeferCloneBoundaryTestCase(
            description="excludes selected upstreams but includes their own boundary",
            selected_keys=frozenset({MODEL_B, MODEL_C}),
            upstream_deps={
                MODEL_C: (MODEL_B,),
                MODEL_B: (MODEL_A,),
                MODEL_A: (),
            },
            expected_selectors=("a",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scope_when_resolving_defer_clone_boundary_then_returns_expected_selectors(
    test_case: DeferCloneBoundaryTestCase,
) -> None:
    scope: PlannerScope = PlannerScope(
        selected_keys=test_case.selected_keys,
        upstream_deps=test_case.upstream_deps,
        downstream_deps={},
        all_keys={},
        models_by_name={},
        execution_order=(),
    )

    boundary, _, _ = resolve_defer_clone_selection(scope=scope)

    assert tuple(boundary) == test_case.expected_selectors


@pytest.mark.parametrize(
    "test_case",
    (
        FunctionDeferCloneBoundaryTestCase(
            description="clones the seed boundary and recreates the intervening function",
            expected_boundary_selectors=("countries",),
            expected_view_chain_selectors=("add_country",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_function_between_model_and_seed_when_resolving_then_recreates_function_and_copies_seed(
    test_case: FunctionDeferCloneBoundaryTestCase,
) -> None:
    scope: PlannerScope = PlannerScope(
        selected_keys=frozenset({MODEL_C}),
        upstream_deps={
            MODEL_C: (FUNCTION_ADD_COUNTRY,),
            FUNCTION_ADD_COUNTRY: (SEED_COUNTRIES,),
            SEED_COUNTRIES: (),
        },
        downstream_deps={},
        all_keys={},
        models_by_name={},
        execution_order=(),
    )

    boundary, view_chain, _ = resolve_defer_clone_selection(scope=scope)

    assert tuple(boundary) == test_case.expected_boundary_selectors
    assert tuple(view_chain) == test_case.expected_view_chain_selectors


@pytest.mark.parametrize(
    "test_case",
    [
        DeferCloneModelCountTestCase(
            description="disabled selected model is excluded from preflight count",
            enabled_by_name={"a": True, "b": False},
            expected_model_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_disabled_model_in_defer_clone_scope_when_counting_then_only_enabled_models_count(
    test_case: DeferCloneModelCountTestCase,
) -> None:
    keys_by_name: dict[str, CompiledObjectKey] = {
        "a": MODEL_A,
        "b": MODEL_B,
    }
    models_by_name: dict[str, CompiledModelView] = {
        name: CompiledModel(
            key=keys_by_name[name],
            deps=(),
            name=name,
            relative_path=Path(f"sql/models/{name}.sql"),
            query_sql="SELECT 1 AS id",
            config=CompileModelConfig(values={"enabled": enabled}),
            destination=CompiledRelationLocation(
                database=None,
                schema="main",
                name=name,
                qualified_name=f"main.{name}",
            ),
        )
        for name, enabled in test_case.enabled_by_name.items()
    }
    scope: PlannerScope = PlannerScope(
        selected_keys=frozenset(keys_by_name.values()),
        upstream_deps={},
        downstream_deps={},
        all_keys=keys_by_name,
        models_by_name=models_by_name,
        execution_order=tuple(keys_by_name.values()),
    )

    _, _, result = resolve_defer_clone_selection(scope=scope)

    assert result == test_case.expected_model_count
