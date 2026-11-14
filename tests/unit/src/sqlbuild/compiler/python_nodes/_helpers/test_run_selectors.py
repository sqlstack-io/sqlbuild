"""Tests for run-command SQL/Python selector helpers."""

from __future__ import annotations

import pytest

from sqlbuild._native import ProjectGraph, PythonNodeGraph, PythonSqlRunSelection
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonSqlSelectorErrorTestCase,
    PythonSqlSelectorTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    build_orders_project_graph,
    build_orders_python_node_graph,
    resolve_run_selection,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorTestCase(
            description="selects task and asset nodes but excludes checks by default",
            select=(),
            exclude=(),
            expected_sql_names=frozenset({"load_events", "raw_orders", "orders"}),
            expected_python_node_names=frozenset(
                {"load_events", "prepare_orders", "export_orders"}
            ),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded Python asset path for run",
            select=("+path:python/assets",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects a Python asset without its unselected Python dependency",
            select=("path:python/assets",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded source terminal loader requirement for run",
            select=("+source:raw_orders",),
            exclude=(),
            expected_sql_names=frozenset({"raw_orders"}),
            expected_python_node_names=frozenset(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_run_selectors_when_resolving_then_excludes_python_checks(
    test_case: PythonSqlSelectorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_orders_python_node_graph()

    result: PythonSqlRunSelection = resolve_run_selection(
        select=test_case.select,
        exclude=test_case.exclude,
        project_graph=project_graph,
        python_graph=python_graph,
    )

    assert frozenset(key.name for key in result.sql_keys) == test_case.expected_sql_names
    assert result.python_node_names == test_case.expected_python_node_names


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="rejects explicit check selector for run",
            select=("check:check_orders_export",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=("Python checks are not selectable here: check_orders_export"),
        ),
        PythonSqlSelectorErrorTestCase(
            description="rejects tag selector that includes a check for run",
            select=("tag:exports",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=("Python checks are not selectable here: check_orders_export"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_run_selector_selects_check_when_resolving_then_raises(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_orders_python_node_graph()

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )
