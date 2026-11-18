"""Tests for unified SQL/Python selector helpers."""

from __future__ import annotations

import pytest

from sqlbuild._native import (
    ProjectGraph,
    PythonNodeGraph,
    PythonSqlRunSelection,
)
from sqlbuild.refs import seed
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonSqlSelectorErrorTestCase,
    PythonSqlSelectorTestCase,
    SqlOnlySelectorParityTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    build_model_depends_on_intermediate_loader_project_graph,
    build_orders_project_graph,
    build_orders_python_node_graph,
    build_python_node_graph_for_case,
    build_sibling_sql_ref_python_node_graph,
    build_sql_downstream_task_to_loader_python_node_graph,
    build_sql_ref_python_node_graph,
    build_terminal_loader_python_node_graph,
    model_ref,
    resolve_run_selection,
    source_ref,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorTestCase(
            description="selects SQL resource by bare name",
            select=("orders",),
            exclude=(),
            expected_sql_names=frozenset({"orders"}),
            expected_python_node_names=frozenset(),
        ),
        PythonSqlSelectorTestCase(
            description="selects Python node by bare name",
            select=("prepare_orders",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects SQL and Python resources by bare name glob",
            select=("*orders",),
            exclude=(),
            expected_sql_names=frozenset({"raw_orders", "orders"}),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded Python node by typed selector",
            select=("+asset:export_orders",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects tag across SQL models and Python nodes",
            select=("tag:daily",),
            exclude=(),
            expected_sql_names=frozenset({"orders"}),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects explicit models root path as SQL models",
            select=("path:models",),
            exclude=(),
            expected_sql_names=frozenset({"orders"}),
            expected_python_node_names=frozenset(),
        ),
        PythonSqlSelectorTestCase(
            description="selects explicit Python task path",
            select=("path:python/tasks",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded explicit Python asset path",
            select=("+path:python/assets",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded explicit Python asset slash path",
            select=("+python/assets/",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded explicit Python asset leading slash path",
            select=("+/python/assets",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"prepare_orders", "export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="selects expanded managed source by source identity",
            select=("+source:raw_orders",),
            exclude=(),
            expected_sql_names=frozenset({"raw_orders"}),
            expected_python_node_names=frozenset(),
        ),
        PythonSqlSelectorTestCase(
            description="selects source and intermediate loader as its own source",
            select=("+source:raw_orders +loader:load_events",),
            exclude=(),
            expected_sql_names=frozenset({"raw_orders", "load_events"}),
            expected_python_node_names=frozenset({"prepare_orders", "load_events"}),
        ),
        PythonSqlSelectorTestCase(
            description="excludes Python node from unified selection",
            select=("tag:daily",),
            exclude=("asset:export_orders",),
            expected_sql_names=frozenset({"orders"}),
            expected_python_node_names=frozenset({"prepare_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unified_selectors_when_resolving_then_returns_sql_and_python_selection(
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
            description="raises when selector matches no SQL resource or Python node",
            select=("missing",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="unknown selector name 'missing'",
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when path selector omits explicit root",
            select=("path:marts",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="path selectors require an explicit root",
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when slash path selector omits explicit root",
            select=("/marts",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="path selectors require an explicit root",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_unified_selector_when_resolving_then_raises_clear_error(
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


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="typed loader glob cannot select a managed source loader",
            select=("loader:raw_*",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="matches managed source loader 'raw_orders'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_managed_source_loader_glob_when_resolving_then_requires_source_selector(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_terminal_loader_python_node_graph()

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorTestCase(
            description="upstream expansion of a Python node includes its SQL dependencies",
            select=("+profile_orders",),
            exclude=(),
            expected_sql_names=frozenset({"orders", "raw_orders"}),
            expected_python_node_names=frozenset({"profile_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="downstream expansion of a SQL resource includes dependent Python nodes",
            select=("raw_orders+",),
            exclude=(),
            expected_sql_names=frozenset({"orders", "raw_orders"}),
            expected_python_node_names=frozenset({"profile_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="unexpanded Python node selects no SQL dependency",
            select=("profile_orders",),
            exclude=(),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"profile_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_sql_dependency_when_expanding_selectors_then_crosses_the_boundary(
    test_case: PythonSqlSelectorTestCase,
) -> None:
    result: PythonSqlRunSelection = resolve_run_selection(
        select=test_case.select,
        exclude=test_case.exclude,
        project_graph=build_orders_project_graph(),
        python_graph=build_sql_ref_python_node_graph(dependency=model_ref("orders")),
    )

    assert frozenset(key.name for key in result.sql_keys) == test_case.expected_sql_names
    assert result.python_node_names == test_case.expected_python_node_names


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorTestCase(
            description="both-way expansion of a task skips siblings of its upstream",
            select=("+export_orders+",),
            exclude=(),
            expected_sql_names=frozenset({"orders", "raw_orders"}),
            expected_python_node_names=frozenset({"export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="both-way expansion of a model skips tasks of its upstream",
            select=("+orders+",),
            exclude=(),
            expected_sql_names=frozenset({"orders", "raw_orders"}),
            expected_python_node_names=frozenset({"export_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="downstream expansion of the upstream source keeps both tasks",
            select=("raw_orders+",),
            exclude=(),
            expected_sql_names=frozenset({"orders", "raw_orders"}),
            expected_python_node_names=frozenset({"export_orders", "export_raw_orders"}),
        ),
        PythonSqlSelectorTestCase(
            description="excluding a both-way expansion leaves the sibling task",
            select=(),
            exclude=("+export_orders+",),
            expected_sql_names=frozenset(),
            expected_python_node_names=frozenset({"export_raw_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sibling_python_nodes_when_expanding_both_ways_then_starts_from_the_roots(
    test_case: PythonSqlSelectorTestCase,
) -> None:
    result: PythonSqlRunSelection = resolve_run_selection(
        select=test_case.select,
        exclude=test_case.exclude,
        project_graph=build_orders_project_graph(),
        python_graph=build_sibling_sql_ref_python_node_graph(),
    )

    assert frozenset(key.name for key in result.sql_keys) == test_case.expected_sql_names
    assert result.python_node_names == test_case.expected_python_node_names


@pytest.mark.parametrize(
    "test_case",
    [
        SqlOnlySelectorParityTestCase(
            description="orders",
            selector="orders",
            expected_sql_names=frozenset({"orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="upstream of a model",
            selector="+orders",
            expected_sql_names=frozenset({"orders", "raw_orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="downstream of a model",
            selector="orders+",
            expected_sql_names=frozenset({"orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="both ways from a model",
            selector="+orders+",
            expected_sql_names=frozenset({"orders", "raw_orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="downstream of a source",
            selector="raw_orders+",
            expected_sql_names=frozenset({"orders", "raw_orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="both ways from a source",
            selector="+raw_orders+",
            expected_sql_names=frozenset({"orders", "raw_orders"}),
        ),
        SqlOnlySelectorParityTestCase(
            description="downstream of a tag",
            selector="tag:daily+",
            expected_sql_names=frozenset({"orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_only_selector_when_resolving_unified_then_sql_keys_match_sql_selection(
    test_case: SqlOnlySelectorParityTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()

    result: PythonSqlRunSelection = resolve_run_selection(
        select=(test_case.selector,),
        exclude=(),
        project_graph=project_graph,
        python_graph=build_sibling_sql_ref_python_node_graph(),
    )

    assert frozenset(key.name for key in result.sql_keys) == test_case.expected_sql_names


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="raises when model ref is unknown",
            select=("profile_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="Python node 'profile_orders' depends on unknown SQL resource",
            sql_ref_dependency=model_ref("missing_orders"),
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when source ref names a model",
            select=("profile_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="declares source.*orders.*but.*orders.*is a model",
            sql_ref_dependency=source_ref("orders"),
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when model ref names a source",
            select=("profile_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="declares model.*raw_orders.*but.*raw_orders.*is a source",
            sql_ref_dependency=model_ref("raw_orders"),
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when seed ref names a model",
            select=("profile_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment="declares seed.*orders.*but.*orders.*is a model",
            sql_ref_dependency=seed("orders"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_typed_sql_ref_when_resolving_then_raises_clear_error(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    assert test_case.sql_ref_dependency is not None
    python_graph: PythonNodeGraph = build_sql_ref_python_node_graph(
        dependency=test_case.sql_ref_dependency
    )

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="raises when task depends on terminal source loader",
            select=("summarize_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=(
                "Python node 'summarize_orders' depends on terminal loader 'raw_orders'; "
                "depend on source 'raw_orders' instead"
            ),
            python_graph_case="terminal_loader_task_dependency",
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when asset depends on terminal source loader",
            select=("export_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=(
                "Python node 'export_orders' depends on terminal loader 'raw_orders'; "
                "depend on source 'raw_orders' instead"
            ),
            python_graph_case="terminal_loader_asset_dependency",
        ),
        PythonSqlSelectorErrorTestCase(
            description="raises when check depends on terminal source loader",
            select=("check_loaded_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=(
                "Check 'check_loaded_orders' depends on terminal loader 'raw_orders'; "
                "use source audits for source 'raw_orders' instead"
            ),
            python_graph_case="terminal_loader_check_dependency",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_terminal_loader_dependency_when_validating_boundaries_then_raises(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_python_node_graph_for_case(test_case.python_graph_case)

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="raises when SQL model depends on intermediate loader",
            select=("orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=(
                "SQL model 'orders' depends on intermediate loader 'fetch_pages'; "
                "depend on a source populated by a terminal loader instead"
            ),
            python_graph_case="default",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_sql_model_dependency_when_validating_boundaries_then_raises(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_model_depends_on_intermediate_loader_project_graph()
    python_graph: PythonNodeGraph = build_python_node_graph_for_case(test_case.python_graph_case)

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="raises when SQL downstream task feeds loader",
            select=("load_events",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=(
                "loader 'load_events' depends on task 'prepare_orders', which reads SQL"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_downstream_task_feeds_loader_when_validating_then_raises(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_sql_downstream_task_to_loader_python_node_graph()

    with pytest.raises(test_case.expected_error_type, match=test_case.expected_error_fragment):
        resolve_run_selection(
            select=test_case.select,
            exclude=test_case.exclude,
            project_graph=project_graph,
            python_graph=python_graph,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonSqlSelectorErrorTestCase(
            description="allows task dependency on intermediate loader",
            select=("summarize_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=None,
            python_graph_case="intermediate_loader_task_dependency",
        ),
        PythonSqlSelectorErrorTestCase(
            description="allows asset dependency on intermediate loader",
            select=("export_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=None,
            python_graph_case="intermediate_loader_asset_dependency",
        ),
        PythonSqlSelectorErrorTestCase(
            description="allows check dependency on intermediate loader",
            select=("check_loaded_orders",),
            exclude=(),
            expected_error_type=ValueError,
            expected_error_fragment=None,
            python_graph_case="intermediate_loader_check_dependency",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_intermediate_loader_dependency_when_validating_boundaries_then_it_is_allowed(
    test_case: PythonSqlSelectorErrorTestCase,
) -> None:
    project_graph: ProjectGraph = build_orders_project_graph()
    python_graph: PythonNodeGraph = build_python_node_graph_for_case(test_case.python_graph_case)

    result: PythonSqlRunSelection = resolve_run_selection(
        select=(), exclude=(), project_graph=project_graph, python_graph=python_graph
    )

    assert test_case.expected_error_fragment is None
    assert result.python_node_names
