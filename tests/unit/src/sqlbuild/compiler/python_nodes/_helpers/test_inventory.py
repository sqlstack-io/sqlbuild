"""Tests for executable Python-node graph inventory helpers."""

from __future__ import annotations

import json
from collections import defaultdict
from dataclasses import asdict
from pathlib import Path
from typing import cast

import pytest

from sqlbuild._native import (
    DiscoveredAssetFunction,
    DiscoveredCheckFunction,
    DiscoveredLoaderFunction,
    DiscoveredProjectInputs,
    DiscoveredProvider,
    DiscoveredPythonNode,
    LocalConfig,
    ProjectConfig,
    PythonNodeGraph,
    PythonNodeIdentity,
    SourceColumnEntry,
    build_python_node_graph,
)
from sqlbuild.compiler.python_nodes.types import PythonNodeKind
from sqlbuild.python_nodes.models import RetryPolicy
from sqlbuild.python_nodes.types import PythonCheckSeverity
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers._test_types import (
    PythonNodeGraphInventoryTestCase,
    PythonNodeIdentityConfigTestCase,
)
from tests.unit.src.sqlbuild.compiler.python_nodes._helpers.helpers import (
    SlackProvider,
    check_orders_export,
    export_orders,
    imported_prepare_orders,
    load_events,
    notify_orders,
    prepare_orders,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeGraphInventoryTestCase(
            description="builds graph inventory across all executable Python node kinds",
            expected_names=(
                "load_events",
                "prepare_orders",
                "export_orders",
                "notify_orders",
                "check_orders_export",
            ),
            expected_kinds=(
                PythonNodeKind.LOADER,
                PythonNodeKind.ASSET,
                PythonNodeKind.ASSET,
                PythonNodeKind.ASSET,
                PythonNodeKind.CHECK,
            ),
            expected_typed_selectors=(
                "loader:load_events",
                "asset:prepare_orders",
                "asset:export_orders",
                "asset:notify_orders",
                "check:check_orders_export",
            ),
            expected_dependency_edges=(
                ("prepare_orders", "load_events"),
                ("prepare_orders", "export_orders"),
                ("export_orders", "check_orders_export"),
            ),
            expected_task_tags=("orders", "daily"),
            expected_asset_column_names=("order_id", "export_uri"),
            expected_check_severity="warn",
            expected_provider_usage_names=("slack_provider",),
            expected_provider_usage_parameters=("slack_provider",),
            expected_provider_usage_annotation_classes=("SlackProvider",),
            expected_provider_usage_annotation_modules=(SlackProvider.__module__,),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_python_functions_when_building_graph_then_indexes_nodes_and_edges(
    test_case: PythonNodeGraphInventoryTestCase,
) -> None:
    retry: RetryPolicy = RetryPolicy(max_attempts=2, retry_on=RuntimeError, jitter=False)
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(name="demo", adapter="duckdb"),
        local_config=LocalConfig(),
        loader_functions=(
            DiscoveredLoaderFunction(
                file_path=Path("/project/loaders/events.py"),
                relative_path=Path("python/loaders/events.py"),
                name="load_events",
                function=load_events,
                depends_on=(prepare_orders,),
            ),
        ),
        asset_functions=(
            DiscoveredAssetFunction(
                file_path=Path("/project/tasks/orders.py"),
                relative_path=Path("python/tasks/orders.py"),
                name="prepare_orders",
                function=prepare_orders,
                tags=test_case.expected_task_tags,
                group="ingestion",
                description="Prepare order export inputs.",
                meta={"owner": "data-eng"},
                retry=retry,
            ),
            DiscoveredAssetFunction(
                file_path=Path("/project/assets/orders.py"),
                relative_path=Path("python/assets/orders.py"),
                name="export_orders",
                function=export_orders,
                depends_on=(imported_prepare_orders,),
                columns=(
                    SourceColumnEntry(name="order_id", type="INTEGER"),
                    SourceColumnEntry(name="export_uri", type="VARCHAR"),
                ),
                retry=retry,
            ),
            DiscoveredAssetFunction(
                file_path=Path("/project/assets/notifications.py"),
                relative_path=Path("python/assets/notifications.py"),
                name="notify_orders",
                function=notify_orders,
                meta={"provider_usages": "user-owned"},
            ),
        ),
        check_functions=(
            DiscoveredCheckFunction(
                file_path=Path("/project/checks/orders.py"),
                relative_path=Path("python/checks/orders.py"),
                name="check_orders_export",
                function=check_orders_export,
                depends_on=(export_orders,),
                severity=PythonCheckSeverity.WARN,
            ),
        ),
        providers=(
            DiscoveredProvider(
                file_path=Path("/project/providers/slack.py"),
                relative_path=Path("providers/slack.py"),
                name="slack_provider",
                provider_class=SlackProvider,
                settings=SlackProvider(),
            ),
        ),
    )

    graph: PythonNodeGraph = build_python_node_graph(discovered_inputs=discovered_inputs)

    assert tuple(node.name for node in graph.nodes) == test_case.expected_names
    assert tuple(node.kind for node in graph.nodes) == test_case.expected_kinds
    assert tuple(graph.nodes_by_name) == test_case.expected_names
    assert tuple(graph.nodes_by_typed_selector) == test_case.expected_typed_selectors
    assert (
        tuple((edge.upstream_name, edge.downstream_name) for edge in graph.dependency_edges)
        == test_case.expected_dependency_edges
    )

    loader_node: DiscoveredPythonNode = graph.nodes_by_name["load_events"]
    assert loader_node.identity is not None
    assert loader_node.identity.node_type == "loader"
    assert loader_node.identity.node_name == "load_events"

    task_node: DiscoveredPythonNode = graph.nodes_by_name["prepare_orders"]
    assert task_node.tags == test_case.expected_task_tags
    assert task_node.group == "ingestion"
    assert task_node.description == "Prepare order export inputs."
    assert task_node.meta == {"owner": "data-eng"}
    assert task_node.asset is not None
    assert task_node.asset.retry is retry
    assert task_node.identity is not None
    assert task_node.identity.node_type == "asset"
    assert task_node.identity.node_name == "prepare_orders"
    assert "Prepare order export inputs." in task_node.identity.definition_json

    asset_node: DiscoveredPythonNode = graph.nodes_by_typed_selector["asset:export_orders"]
    assert asset_node.asset is not None
    assert asset_node.identity is not None
    assert asset_node.identity.node_type == "asset"
    assert tuple(column.name for column in asset_node.asset.columns) == (
        test_case.expected_asset_column_names
    )
    assert asset_node.asset.retry is retry

    check_node: DiscoveredPythonNode = graph.nodes_by_typed_selector["check:check_orders_export"]
    assert check_node.check is not None
    assert check_node.identity is not None
    assert check_node.identity.node_type == "check"
    assert check_node.check.severity.value == test_case.expected_check_severity

    notify_node: DiscoveredPythonNode = graph.nodes_by_name["notify_orders"]
    assert notify_node.meta == {"provider_usages": "user-owned"}
    assert tuple(usage.provider_name for usage in notify_node.provider_usages) == (
        test_case.expected_provider_usage_names
    )
    assert tuple(usage.parameter_name for usage in notify_node.provider_usages) == (
        test_case.expected_provider_usage_parameters
    )
    assert tuple(usage.annotation_class_name for usage in notify_node.provider_usages) == (
        test_case.expected_provider_usage_annotation_classes
    )
    assert tuple(usage.annotation_module for usage in notify_node.provider_usages) == (
        test_case.expected_provider_usage_annotation_modules
    )
    assert tuple(asdict(usage) for usage in notify_node.provider_usages) == (
        {
            "provider_name": "slack_provider",
            "parameter_name": "slack_provider",
            "annotation_class_name": "SlackProvider",
            "annotation_module": SlackProvider.__module__,
        },
    )
    edges_by_downstream: defaultdict[str, list[tuple[str, str]]] = defaultdict(list)
    for edge in graph.dependency_edges:
        edges_by_downstream[edge.downstream_name].append((edge.upstream_name, edge.downstream_name))
    assert tuple(edges_by_downstream["notify_orders"]) == ()


@pytest.mark.parametrize(
    "test_case",
    [
        PythonNodeIdentityConfigTestCase(
            description="identity decorator config keeps each kind's authored field set",
            expected_decorator_config_keys_by_selector={
                "loader:load_events": (
                    "columns",
                    "contract",
                    "cursor_column",
                    "destination",
                    "unique_key",
                    "write_strategy",
                ),
                "asset:prepare_orders": ("description", "group", "meta", "tags"),
                "asset:export_orders": (
                    "column_lineage",
                    "columns",
                    "description",
                    "group",
                    "meta",
                    "tags",
                ),
                "check:check_orders_export": (
                    "description",
                    "group",
                    "meta",
                    "severity",
                    "tags",
                ),
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_fully_configured_python_nodes_when_building_then_identity_config_is_per_kind(
    test_case: PythonNodeIdentityConfigTestCase,
) -> None:
    retry: RetryPolicy = RetryPolicy(max_attempts=2, retry_on=RuntimeError, jitter=False)
    columns: tuple[SourceColumnEntry, ...] = (SourceColumnEntry(name="order_id", type="INTEGER"),)

    nodes: tuple[DiscoveredPythonNode, ...] = build_python_node_graph(
        discovered_inputs=DiscoveredProjectInputs(
            project_config=ProjectConfig(name="demo", adapter="duckdb"),
            local_config=LocalConfig(),
            loader_functions=(
                DiscoveredLoaderFunction(
                    file_path=Path("/project/loaders/events.py"),
                    relative_path=Path("python/loaders/events.py"),
                    name="load_events",
                    function=load_events,
                    destination="raw.events",
                    cursor_column="updated_at",
                    unique_key=("event_id",),
                    columns=columns,
                    contract="enforced",
                ),
            ),
            asset_functions=(
                DiscoveredAssetFunction(
                    file_path=Path("/project/tasks/orders.py"),
                    relative_path=Path("python/tasks/orders.py"),
                    name="prepare_orders",
                    function=prepare_orders,
                    tags=("orders",),
                    group="ingestion",
                    description="Prepare order export inputs.",
                    meta={"owner": "data-eng"},
                    retry=retry,
                ),
                DiscoveredAssetFunction(
                    file_path=Path("/project/assets/orders.py"),
                    relative_path=Path("python/assets/orders.py"),
                    name="export_orders",
                    function=export_orders,
                    tags=("orders",),
                    group="exports",
                    description="Export orders.",
                    meta={"owner": "data-eng"},
                    columns=columns,
                    retry=retry,
                ),
            ),
            check_functions=(
                DiscoveredCheckFunction(
                    file_path=Path("/project/checks/orders.py"),
                    relative_path=Path("python/checks/orders.py"),
                    name="check_orders_export",
                    function=check_orders_export,
                    depends_on=(export_orders,),
                    severity=PythonCheckSeverity.WARN,
                    tags=("orders",),
                    group="exports",
                    description="Check exported orders.",
                    meta={"owner": "data-eng"},
                ),
            ),
        )
    ).nodes

    assert {
        f"{node.kind.value}:{node.name}": tuple(
            sorted(
                json.loads(cast(PythonNodeIdentity, node.identity).definition_json)[
                    "decorator_config"
                ]
            )
        )
        for node in nodes
    } == test_case.expected_decorator_config_keys_by_selector
