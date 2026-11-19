from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredPythonNode,
    PlanOutput,
    PythonNodeGraph,
    SourceEntry,
    build_python_relation_targets,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.refs import model, source
from tests.unit.src.sqlbuild.compiler.pipeline.main._test_types import (
    PythonRelationTargetScopeTestCase,
    PythonRelationTargetsTestCase,
    SelectedPythonRelationRefsTestCase,
)
from tests.unit.src.sqlbuild.compiler.pipeline.main.helpers import (
    RelationTargetTestAdapter,
    build_relation_target_project,
    relation_target_python_node,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonRelationTargetsTestCase(
            description="source refs use source read map instead of load map",
            expected_source_relation="deferred_raw.orders",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_read_map_when_building_python_relation_targets_then_uses_read_relation(
    test_case: PythonRelationTargetsTestCase,
) -> None:
    project: CompiledProjectView = build_relation_target_project()
    raw_source: SourceEntry = project.sources[0].source_entry
    read_source: SourceEntry = SourceEntry(name="orders", schema="deferred_raw", table="orders")
    plan_output: PlanOutput = PlanOutput(
        source_map={"orders": raw_source},
        source_read_map={"orders": read_source},
    )

    targets: dict[SqlResourceRef, str] = build_python_relation_targets(
        adapter=RelationTargetTestAdapter(),
        project=project,
        plan_output=plan_output,
    )

    assert targets[source("orders")] == test_case.expected_source_relation


@pytest.mark.parametrize(
    "test_case",
    [
        PythonRelationTargetScopeTestCase(
            description="SQL-only seed ignores unrelated project source declarations",
            required_refs=frozenset(),
            expected_targets={},
        ),
        PythonRelationTargetScopeTestCase(
            description="selected Python-only source ref preserves deferred plan mapping",
            required_refs=frozenset({source("orders")}),
            expected_targets={source("orders"): "deferred_raw.orders"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_narrow_plan_when_building_python_targets_then_only_required_refs_are_resolved(
    test_case: PythonRelationTargetScopeTestCase,
) -> None:
    deferred_source: SourceEntry = SourceEntry(name="orders", schema="deferred_raw", table="orders")
    targets: dict[SqlResourceRef, str] = build_python_relation_targets(
        adapter=RelationTargetTestAdapter(),
        project=build_relation_target_project(),
        plan_output=PlanOutput(source_read_map={"orders": deferred_source}),
        required_refs=test_case.required_refs,
    )

    assert targets == test_case.expected_targets


@pytest.mark.parametrize(
    "test_case",
    [
        SelectedPythonRelationRefsTestCase(
            description="selected loaders tasks assets and checks retain their SQL refs",
            selected_python_names=frozenset({"orders_check"}),
            expected_refs=frozenset(
                {
                    source("loader_source"),
                    source("task_source"),
                    model("asset_model"),
                    model("check_model"),
                }
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_selected_python_declarations_when_collecting_refs_then_all_kinds_are_retained(
    test_case: SelectedPythonRelationRefsTestCase,
) -> None:
    project_path: Path = Path("python_nodes/orders.py")
    nodes: tuple[DiscoveredPythonNode, ...] = (
        DiscoveredPythonNode(
            kind=PythonNodeKind.LOADER,
            file_path=project_path,
            relative_path=project_path,
            name="load_orders",
            function=relation_target_python_node,
            sql_deps=(source("loader_source"),),
        ),
        DiscoveredPythonNode(
            kind=PythonNodeKind.ASSET,
            file_path=project_path,
            relative_path=project_path,
            name="score_orders",
            function=relation_target_python_node,
            sql_deps=(source("task_source"),),
        ),
        DiscoveredPythonNode(
            kind=PythonNodeKind.ASSET,
            file_path=project_path,
            relative_path=project_path,
            name="orders_asset",
            function=relation_target_python_node,
            sql_deps=(model("asset_model"),),
        ),
        DiscoveredPythonNode(
            kind=PythonNodeKind.CHECK,
            file_path=project_path,
            relative_path=project_path,
            name="orders_check",
            function=relation_target_python_node,
            sql_deps=(model("check_model"),),
        ),
    )
    graph: PythonNodeGraph = PythonNodeGraph(
        nodes=nodes,
        dependency_edges=(),
        upstream_deps={
            "load_orders": (),
            "score_orders": ("load_orders",),
            "orders_asset": ("score_orders",),
            "orders_check": ("orders_asset",),
        },
        downstream_deps={
            "load_orders": ("score_orders",),
            "score_orders": ("orders_asset",),
            "orders_asset": ("orders_check",),
            "orders_check": (),
        },
        tag_index={},
        path_index={},
        nodes_by_name={node.name: node for node in nodes},
        nodes_by_typed_selector={},
    )

    refs: frozenset[SqlResourceRef] = graph.selected_sql_refs(
        selected_names=test_case.selected_python_names,
    )

    assert refs == test_case.expected_refs
