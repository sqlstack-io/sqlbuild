"""Tests for source-load node planning helpers."""

from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    CompiledModel,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompileModelConfig,
    DiscoveredLoaderFunction,
    SourceEntry,
    SourceLoadPlanEntry,
    SourceWriteStrategy,
    build_intermediate_source_map,
    build_source_load_entries,
    build_source_load_map,
    expand_selected_loader_dependencies,
    planner_metadata_name_filter,
    project_with,
    upstream_loader_dependency_names,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)
from sqlbuild.runtime.contracts.types import ExecutionResourceKind
from tests.unit.src.sqlbuild.compiler.planner._helpers._test_types import (
    LoaderDagExpansionTestCase,
    LoaderDependencyNamesTestCase,
    LoaderDestinationPlanningTestCase,
    MetadataNameFilterTestCase,
    SourceLoadNodesTestCase,
    SourceMetadataClosureTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import (
    build_source_load_nodes_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceLoadNodesTestCase(
            description="builds terminal and intermediate source-load entries",
            expected_map_names=("fetch_orders", "raw_orders"),
            expected_entries=(
                SourceLoadPlanEntry(
                    key=CompiledObjectKey(CompiledResourceType.SOURCE, "fetch_orders"),
                    name="fetch_orders",
                    loader="fetch_orders",
                    destination="staging_fetch_orders",
                    resource_kind=ExecutionResourceKind.LOADER,
                    write_strategy=SourceWriteStrategy.TABLE,
                    cursor_column=None,
                    unique_key=(),
                    is_reload=True,
                ),
                SourceLoadPlanEntry(
                    key=CompiledObjectKey(CompiledResourceType.SOURCE, "raw_orders"),
                    name="raw_orders",
                    loader="load_orders",
                    destination="orders",
                    resource_kind=ExecutionResourceKind.SOURCE,
                    write_strategy=SourceWriteStrategy.MERGE,
                    cursor_column="updated_at",
                    unique_key=("order_id",),
                    is_reload=True,
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_selected_source_load_keys_when_building_entries_then_returns_ordered_load_nodes(
    test_case: SourceLoadNodesTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    selected_keys: frozenset[CompiledObjectKey] = frozenset(
        entry.key for entry in test_case.expected_entries
    )
    execution_order: tuple[CompiledObjectKey, ...] = tuple(
        entry.key for entry in test_case.expected_entries
    )

    source_map: dict[str, SourceEntry] = build_source_load_map(
        project=project,
        selected_keys=selected_keys,
    )
    entries: tuple[SourceLoadPlanEntry, ...] = build_source_load_entries(
        execution_order=execution_order,
        selected_keys=selected_keys,
        source_map=source_map,
        is_reload=True,
    )

    assert tuple(sorted(source_map)) == test_case.expected_map_names
    assert entries == test_case.expected_entries


@pytest.mark.parametrize(
    "test_case",
    [
        SourceMetadataClosureTestCase(
            description="unrelated selected seed excludes all project sources",
            expected_source_names=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_selected_seed_when_building_source_map_then_sources_are_excluded(
    test_case: SourceMetadataClosureTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    seed_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SEED, "country_codes")

    source_map: dict[str, SourceEntry] = build_source_load_map(
        project=project,
        selected_keys=frozenset({seed_key}),
        upstream_deps={seed_key: ()},
    )

    assert tuple(sorted(source_map)) == test_case.expected_source_names


@pytest.mark.parametrize(
    "test_case",
    [
        SourceMetadataClosureTestCase(
            description="reachable source through unselected upstream model is retained",
            expected_source_names=("raw_orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_transitive_unselected_upstream_when_building_source_map_then_source_is_retained(
    test_case: SourceMetadataClosureTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    selected_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "fact_orders")
    upstream_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "stg_orders")
    source_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.SOURCE, "raw_orders")

    source_map: dict[str, SourceEntry] = build_source_load_map(
        project=project,
        selected_keys=frozenset({selected_key}),
        upstream_deps={
            selected_key: (upstream_key,),
            upstream_key: (source_key,),
            source_key: (),
        },
    )

    assert tuple(sorted(source_map)) == test_case.expected_source_names


@pytest.mark.parametrize(
    "test_case",
    [
        MetadataNameFilterTestCase(
            description="selected closure inventories logical aliases by physical destination",
            expected_physical_names=frozenset({"FACT_ORDERS_PHYSICAL", "STG_ORDERS_PHYSICAL"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unselected_upstream_in_closure_when_filtering_metadata_then_uses_physical_names(
    test_case: MetadataNameFilterTestCase,
) -> None:
    selected_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "fact_orders")
    upstream_key: CompiledObjectKey = CompiledObjectKey(CompiledResourceType.MODEL, "stg_orders")
    selected_model: CompiledModelView = CompiledModel(
        key=selected_key,
        deps=(upstream_key,),
        name="fact_orders",
        relative_path=Path("sql/models/fact_orders.sql"),
        query_sql='SELECT * FROM __ref("stg_orders")',
        config=CompileModelConfig(),
        destination=CompiledRelationLocation(
            database="ANALYTICS_DB",
            schema="ANALYTICS",
            name="FACT_ORDERS_PHYSICAL",
            qualified_name="ANALYTICS_DB.ANALYTICS.FACT_ORDERS_PHYSICAL",
        ),
    )
    upstream_model: CompiledModelView = CompiledModel(
        key=upstream_key,
        deps=(),
        name="stg_orders",
        relative_path=Path("sql/models/stg_orders.sql"),
        query_sql="SELECT 1 AS id",
        config=CompileModelConfig(),
        destination=CompiledRelationLocation(
            database="ANALYTICS_DB",
            schema="STAGING",
            name="STG_ORDERS_PHYSICAL",
            qualified_name="ANALYTICS_DB.STAGING.STG_ORDERS_PHYSICAL",
        ),
    )
    project: CompiledProjectView = CompiledProject(
        run_id="test",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        models=(selected_model, upstream_model),
    )

    names: tuple[str, ...] | None = planner_metadata_name_filter(
        project=project,
        selected_keys=frozenset({selected_key, upstream_key}),
    )

    assert names is not None
    assert test_case.expected_physical_names.issubset(names)


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderDagExpansionTestCase(
            description="direct terminal source preserves upstream intermediate references only",
            selected_names=frozenset({"raw_orders"}),
            execute_dependency_names=frozenset(),
            expected_selected_names=frozenset({"raw_orders"}),
            expected_upstream_names={
                "raw_orders": ("fetch_orders",),
                "fetch_orders": (),
            },
            expected_intermediate_source_names=(),
            expected_intermediate_loader_flags=(),
        ),
        LoaderDagExpansionTestCase(
            description="expanded terminal source selects upstream intermediate loaders",
            selected_names=frozenset({"raw_orders"}),
            execute_dependency_names=frozenset({"raw_orders"}),
            expected_selected_names=frozenset({"fetch_orders", "raw_orders"}),
            expected_upstream_names={
                "raw_orders": ("fetch_orders",),
                "fetch_orders": (),
            },
            expected_intermediate_source_names=("fetch_orders",),
            expected_intermediate_loader_flags=(True,),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_terminal_source_when_expanding_loader_deps_then_adds_intermediate_source(
    test_case: LoaderDagExpansionTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    selected_keys: frozenset[CompiledObjectKey] = frozenset(
        CompiledObjectKey(resource_type=CompiledResourceType.SOURCE, name=name)
        for name in test_case.selected_names
    )

    expanded_selected, upstream_deps = expand_selected_loader_dependencies(
        project=project,
        selected_keys=selected_keys,
        upstream_deps={},
        executable_dependency_source_keys=frozenset(
            CompiledObjectKey(resource_type=CompiledResourceType.SOURCE, name=name)
            for name in test_case.execute_dependency_names
        ),
    )
    intermediate_sources: dict[str, SourceEntry] = build_intermediate_source_map(
        project=project,
        selected_keys=expanded_selected,
    )

    assert frozenset(key.name for key in expanded_selected) == test_case.expected_selected_names
    actual_upstream_names: dict[str, tuple[str, ...]] = {}
    for key, deps in upstream_deps.items():
        dep_names: list[str] = []
        for dep in deps:
            dep_names.append(dep.name)
        actual_upstream_names[key.name] = tuple(dep_names)
    assert actual_upstream_names == test_case.expected_upstream_names
    assert tuple(intermediate_sources) == test_case.expected_intermediate_source_names
    assert (
        tuple(
            source.meta.get("sqlbuild_loader_node") is True
            for source in intermediate_sources.values()
        )
        == test_case.expected_intermediate_loader_flags
    )


@pytest.mark.parametrize(
    "test_case",
    (
        LoaderDestinationPlanningTestCase(
            description="two-part loader destination retains effective target database",
            destination="loader_schema.fetch_orders",
            expected_parts=("default_db", "loader_schema", "fetch_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_qualified_intermediate_destination_when_building_map_then_parses_loader_target(
    test_case: LoaderDestinationPlanningTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    project = project_with(
        project=project,
        effective_target_database="default_db",
        effective_target_schema="default_schema",
        loader_functions=(
            replace(project.loader_functions[0], destination=test_case.destination),
            *project.loader_functions[1:],
        ),
    )

    entries: dict[str, SourceEntry] = build_intermediate_source_map(
        project=project,
        selected_keys=frozenset({CompiledObjectKey(CompiledResourceType.SOURCE, "fetch_orders")}),
    )

    entry: SourceEntry = entries["fetch_orders"]
    assert (entry.database, entry.schema, entry.table) == test_case.expected_parts


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderDependencyNamesTestCase(
            description="drops non-loader function dependencies and deduplicates repeated loaders",
            expected_names=("fetch_orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_mixed_function_dependencies_when_resolving_then_returns_only_known_loaders(
    test_case: LoaderDependencyNamesTestCase,
) -> None:
    project: CompiledProjectView = build_source_load_nodes_project()
    dependency: DiscoveredLoaderFunction = project.loader_functions[0]
    root: DiscoveredLoaderFunction = replace(
        project.loader_functions[1],
        depends_on=(dependency.function, build_source_load_nodes_project, dependency.function),
    )
    assert (
        upstream_loader_dependency_names(
            loader_function=root,
            loader_functions=project.loader_functions,
        )
        == test_case.expected_names
    )
