from __future__ import annotations

from collections.abc import Mapping
from typing import Any

import pytest

from sqlbuild.integrations.rivers._helpers import assets as rivers_assets
from sqlbuild.integrations.rivers._helpers.assets import build_asset_defs
from sqlbuild.integrations.rivers.classes.sqlbuild_rivers_translator import (
    SqlBuildRiversTranslator,
)
from tests.unit.src.sqlbuild.integrations.dagster.helpers import (
    build_python_augmented_dagster_test_dag,
)
from tests.unit.src.sqlbuild.integrations.rivers._test_types import (
    RiversPythonArtifactCompatibilityTestCase,
    RiversTranslatorArgumentTestCase,
)
from tests.unit.src.sqlbuild.integrations.rivers.helpers import (
    FakeRiversModule,
    RecordingRiversTranslator,
)


@pytest.mark.parametrize(
    "test_case",
    [
        RiversPythonArtifactCompatibilityTestCase(
            description="maps Python DAG artifact additions into asset defs",
            expected_asset_names=(
                "raw__orders",
                "shared_order_feed",
                "raw_orders_loader",
                "analytics__waffle_types",
                "analytics__normalize_email",
                "analytics__orders",
                "analytics__customers",
                "asset__prepare_orders",
                "asset__orders_export",
            ),
            expected_order_deps=("raw__orders", "analytics__normalize_email"),
            expected_task_deps=("analytics__orders",),
            expected_asset_deps=("asset__prepare_orders",),
            expected_task_kinds=["sqlbuild", "asset"],
            expected_asset_kinds=["sqlbuild", "asset"],
            expected_task_group="python",
            expected_asset_group="exports",
            expected_asset_metadata_keys=(
                "columns",
                "column_lineage",
                "materialization_type",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_augmented_dag_when_building_asset_defs_then_maps_python_nodes(
    test_case: RiversPythonArtifactCompatibilityTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    dag: Mapping[str, Any] = build_python_augmented_dagster_test_dag()
    monkeypatch.setattr(rivers_assets, "load_rivers", lambda: FakeRiversModule())

    asset_defs: tuple[Any, ...] = build_asset_defs(
        dag=dag,
        translator=SqlBuildRiversTranslator(),
    )
    asset_defs_by_name: dict[str, tuple[Any, ...]] = {asset.name: (asset,) for asset in asset_defs}
    assert len(asset_defs_by_name) == len(asset_defs)
    order_def: Any = next(iter(asset_defs_by_name.get("analytics__orders", ())))
    task_def: Any = next(iter(asset_defs_by_name.get("asset__prepare_orders", ())))
    python_asset_def: Any = next(iter(asset_defs_by_name.get("asset__orders_export", ())))

    assert tuple(asset.name for asset in asset_defs) == test_case.expected_asset_names
    assert tuple(dep.name for dep in order_def.deps) == test_case.expected_order_deps
    assert tuple(dep.name for dep in task_def.deps) == test_case.expected_task_deps
    assert tuple(dep.name for dep in python_asset_def.deps) == test_case.expected_asset_deps
    assert task_def.kinds == test_case.expected_task_kinds
    assert python_asset_def.kinds == test_case.expected_asset_kinds
    assert task_def.group == test_case.expected_task_group
    assert python_asset_def.group == test_case.expected_asset_group
    assert all(key in python_asset_def.metadata for key in test_case.expected_asset_metadata_keys)


@pytest.mark.parametrize(
    "test_case",
    [
        RiversTranslatorArgumentTestCase(
            description="dependencies are the artifact's own nodes without the project name",
            node_id="model:orders",
            expected_dependency_name_count=1,
            expected_owned_node_keys_end=("materialization_type", "project_name"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_augmented_dag_when_building_asset_defs_then_translator_sees_artifact_nodes(
    test_case: RiversTranslatorArgumentTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    dag: Mapping[str, Any] = build_python_augmented_dagster_test_dag()
    monkeypatch.setattr(rivers_assets, "load_rivers", lambda: FakeRiversModule())
    translator: RecordingRiversTranslator = RecordingRiversTranslator()

    _ = build_asset_defs(dag=dag, translator=translator)
    artifact_node: Mapping[str, Any] = {str(node["id"]): node for node in dag["nodes"]}[
        test_case.node_id
    ]
    dependencies: list[Mapping[str, Any]] = translator.named_nodes[(test_case.node_id, False)]
    owned_nodes: list[Mapping[str, Any]] = translator.named_nodes[(test_case.node_id, True)]

    assert len(dependencies) == test_case.expected_dependency_name_count
    assert all(dependency is artifact_node for dependency in dependencies)
    assert [tuple(node)[-2:] for node in owned_nodes] == [test_case.expected_owned_node_keys_end]
