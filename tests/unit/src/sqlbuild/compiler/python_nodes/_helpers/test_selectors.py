"""Tests for executable Python-node selector helpers."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    PythonNodeGraph,
    resolve_python_node_selection,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonNodeSelectorErrorTestCase,
    PythonNodeSelectorTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    build_orders_python_node_graph,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeSelectorTestCase(
            description="selects Python node by bare name",
            select=("prepare_orders",),
            exclude=(),
            expected_names=frozenset({"prepare_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python node by typed selector",
            select=("asset:export_orders",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by bare name glob",
            select=("*orders*",),
            exclude=(),
            expected_names=frozenset({"prepare_orders", "export_orders", "check_orders_export"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by typed name glob",
            select=("asset:export_*",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects upstream Python nodes with leading plus",
            select=("+check_orders_export",),
            exclude=(),
            expected_names=frozenset({"prepare_orders", "export_orders", "check_orders_export"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects downstream Python nodes with trailing plus",
            select=("prepare_orders+",),
            exclude=(),
            expected_names=frozenset(
                {"prepare_orders", "load_events", "export_orders", "check_orders_export"}
            ),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by tag",
            select=("tag:daily",),
            exclude=(),
            expected_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects every Python node by the python root path",
            select=("path:python",),
            exclude=(),
            expected_names=frozenset(
                {"prepare_orders", "load_events", "export_orders", "check_orders_export"}
            ),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by explicit tasks path",
            select=("path:python/tasks",),
            exclude=(),
            expected_names=frozenset({"prepare_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by explicit loaders path",
            select=("path:python/loaders",),
            exclude=(),
            expected_names=frozenset({"load_events"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by explicit assets path",
            select=("path:python/assets",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by explicit assets slash path",
            select=("python/assets/",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by leading slash assets path",
            select=("/python/assets",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects Python nodes by explicit checks path with upstream expansion",
            select=("+path:python/checks",),
            exclude=(),
            expected_names=frozenset({"prepare_orders", "export_orders", "check_orders_export"}),
        ),
        PythonNodeSelectorTestCase(
            description="selects tagged Python nodes with downstream expansion",
            select=("tag:exports+",),
            exclude=(),
            expected_names=frozenset({"export_orders", "check_orders_export"}),
        ),
        PythonNodeSelectorTestCase(
            description="intersects comma-separated Python-node selectors",
            select=("tag:daily,asset:export_orders",),
            exclude=(),
            expected_names=frozenset({"export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="excludes Python nodes after expansion",
            select=("prepare_orders+",),
            exclude=("check:check_orders_export",),
            expected_names=frozenset({"prepare_orders", "load_events", "export_orders"}),
        ),
        PythonNodeSelectorTestCase(
            description="returns all Python nodes when select is empty",
            select=(),
            exclude=(),
            expected_names=frozenset(
                {"load_events", "prepare_orders", "export_orders", "check_orders_export"}
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_node_selectors_when_resolving_then_returns_expected_names(
    test_case: PythonNodeSelectorTestCase,
) -> None:
    graph: PythonNodeGraph = build_orders_python_node_graph()

    result: frozenset[str] = resolve_python_node_selection(
        select=test_case.select,
        exclude=test_case.exclude,
        graph=graph,
    )

    assert result == test_case.expected_names


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeSelectorErrorTestCase(
            description="raises when typed selector has wrong Python node kind",
            select=("asset:export_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="unknown Python node selector 'export_orders'",
        ),
        PythonNodeSelectorErrorTestCase(
            description="raises when tag selector matches no Python nodes",
            select=("tag:missing",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="no Python nodes found with tag 'missing'",
        ),
        PythonNodeSelectorErrorTestCase(
            description="raises when path selector omits explicit root",
            select=("path:missing",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="path selectors require an explicit root",
        ),
        PythonNodeSelectorErrorTestCase(
            description="raises when path selector uses a kind folder outside the python root",
            select=("path:tasks",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="use 'models/' or 'python/'",
        ),
        PythonNodeSelectorErrorTestCase(
            description="raises when explicit path selector matches no Python nodes",
            select=("path:python/tasks/missing",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="no Python nodes found under path 'python/tasks/missing'",
        ),
        PythonNodeSelectorErrorTestCase(
            description="raises when SQL source selector is used for Python nodes",
            select=("source:raw_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="does not map to a Python node type",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_python_node_selector_when_resolving_then_raises_clear_error(
    test_case: PythonNodeSelectorErrorTestCase,
) -> None:
    graph: PythonNodeGraph = build_orders_python_node_graph()

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_python_node_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            graph=graph,
        )
