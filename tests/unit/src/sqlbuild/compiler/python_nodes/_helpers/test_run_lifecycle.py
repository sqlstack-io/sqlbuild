"""Tests for lifecycle-aware run classification."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    CompiledResourceType,
    PythonNodeGraph,
    PythonSqlRunLifecyclePlan,
    PythonSqlRunSelection,
    build_python_sql_run_lifecycle,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonSqlRunLifecycleTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    build_python_node_graph_for_case,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlRunLifecycleTestCase(
            description="classifies loader upstream task into pre sql ingress",
            python_graph_case="orders",
            selection=PythonSqlRunSelection(
                sql_keys=frozenset(
                    {
                        CompiledObjectKey(
                            resource_type=CompiledResourceType.SOURCE, name="raw_orders"
                        )
                    }
                ),
                python_node_names=frozenset({"prepare_orders", "load_events"}),
            ),
            expected_ingress_python_names=frozenset({"prepare_orders", "load_events"}),
            expected_ingress_loader_names=frozenset({"load_events"}),
            expected_read_side_python_names=frozenset(),
        ),
        PythonSqlRunLifecycleTestCase(
            description="classifies read only asset into read-side Python",
            python_graph_case="orders",
            selection=PythonSqlRunSelection(
                sql_keys=frozenset(
                    {CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders")}
                ),
                python_node_names=frozenset({"export_orders"}),
            ),
            expected_ingress_python_names=frozenset(),
            expected_ingress_loader_names=frozenset(),
            expected_read_side_python_names=frozenset({"export_orders"}),
        ),
        PythonSqlRunLifecycleTestCase(
            description="classifies intermediate loader dependency into pre sql ingress",
            python_graph_case="intermediate_loader_asset_dependency",
            selection=PythonSqlRunSelection(
                sql_keys=frozenset(
                    {
                        CompiledObjectKey(
                            resource_type=CompiledResourceType.SOURCE, name="fetch_pages"
                        )
                    }
                ),
                python_node_names=frozenset({"fetch_pages", "export_orders"}),
            ),
            expected_ingress_python_names=frozenset({"fetch_pages"}),
            expected_ingress_loader_names=frozenset({"fetch_pages"}),
            expected_read_side_python_names=frozenset({"export_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_run_selection_when_building_lifecycle_plan_then_classifies_phases(
    test_case: PythonSqlRunLifecycleTestCase,
) -> None:
    graph: PythonNodeGraph = build_python_node_graph_for_case(test_case.python_graph_case)

    result: PythonSqlRunLifecyclePlan = build_python_sql_run_lifecycle(
        selection=test_case.selection,
        python_graph=graph,
    )

    assert result.ingress_python_node_names == test_case.expected_ingress_python_names
    assert result.ingress_loader_names == test_case.expected_ingress_loader_names
    assert result.read_side_python_node_names == test_case.expected_read_side_python_names
