from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import PlannerInputError
from sqlbuild.integrations.dagster import (
    SqlBuildDagsterTranslator,
    build_sqlbuild_asset_selection,
    sqlbuild_assets,
    sqlbuild_scenario_checks,
)
from sqlbuild.integrations.dagster._helpers.assets import (
    build_asset_specs,
    build_check_specs,
    build_scenario_check_specs,
)
from sqlbuild.integrations.dagster.exceptions import DagsterDagInputError
from sqlbuild.integrations.dagster.models import SqlBuildProject
from tests.unit.src.sqlbuild.integrations.dagster._test_types import (
    DagsterAssetCheckFilterTestCase,
    DagsterAssetSelectionErrorTestCase,
    DagsterAssetSelectionTestCase,
    DagsterAssetSpecTestCase,
    DagsterCheckTranslatorCallTestCase,
    DagsterConflictingInputTestCase,
    DagsterDecoratorTestCase,
    DagsterDescriptionTestCase,
    DagsterExternalAssetOwnershipTestCase,
    DagsterPythonArtifactCompatibilityTestCase,
    DagsterScenarioCheckDecoratorTestCase,
    DagsterSourceAssetKeyTestCase,
    DagsterUnitTestSelectorTestCase,
)
from tests.unit.src.sqlbuild.integrations.dagster.helpers import (
    RecordingDagsterTranslator,
    build_dagster_selection_graph_dag,
    build_dagster_test_dag,
    build_python_augmented_dagster_test_dag,
)

dg: Any = pytest.importorskip("dagster")


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterDescriptionTestCase(
            description="authored SQL takes precedence over the source path",
            node={
                "kind": "model",
                "name": "orders",
                "description": "Clean orders",
                "path": "sql/models/orders.sql",
                "sql": "SELECT 1",
            },
            expected_description=("Clean orders\n\n**SQLBuild SQL:**\n```sql\nSELECT 1\n```"),
        ),
        DagsterDescriptionTestCase(
            description="non-SQL asset includes its source path",
            node={"kind": "seed", "name": "waffles", "path": "sql/seeds/waffles.csv"},
            expected_description=(
                "SQLBuild seed `waffles`.\n\n**Source file:** `sql/seeds/waffles.csv`"
            ),
        ),
        DagsterDescriptionTestCase(
            description="source path containing backticks remains one code span",
            node={"kind": "seed", "name": "waffles", "path": "sql/seeds/`waffles`.csv"},
            expected_description=(
                "SQLBuild seed `waffles`.\n\n**Source file:** ``sql/seeds/`waffles`.csv``"
            ),
        ),
        DagsterDescriptionTestCase(
            description="source path whitespace is preserved",
            node={"kind": "seed", "name": "waffles", "path": " sql/seeds/waffles.csv "},
            expected_description=(
                "SQLBuild seed `waffles`.\n\n**Source file:** `  sql/seeds/waffles.csv  `"
            ),
        ),
        DagsterDescriptionTestCase(
            description="missing source path retains the fallback description",
            node={"kind": "seed", "name": "waffles"},
            expected_description="SQLBuild seed `waffles`.",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_node_when_translating_description_then_renders_expected_markdown(
    test_case: DagsterDescriptionTestCase,
) -> None:
    translator: SqlBuildDagsterTranslator = SqlBuildDagsterTranslator()

    assert translator.get_description(test_case.node) == test_case.expected_description


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterSourceAssetKeyTestCase(
            description="source metadata defines an external Dagster identity",
            node={
                "kind": "source",
                "asset_key": ["raw", "orders"],
                "meta": {"dagster": {"asset_key": ["external", "orders"]}},
            },
            expected_asset_key=("external", "orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_source_dagster_metadata_when_translating_then_uses_authored_asset_key(
    test_case: DagsterSourceAssetKeyTestCase,
) -> None:
    translator: SqlBuildDagsterTranslator = SqlBuildDagsterTranslator()

    asset_key: Any = translator.get_asset_key(test_case.node)

    assert tuple(asset_key.path) == test_case.expected_asset_key


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterExternalAssetOwnershipTestCase(
            description="external source remains a dependency without an owned spec or check",
            external_asset_key=("raw", "orders"),
            dependent_asset_key=("analytics", "orders"),
            expected_check_names=("audit__not_null__order_id",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_external_source_translator_when_building_specs_then_retains_dependency_without_ownership(
    test_case: DagsterExternalAssetOwnershipTestCase,
) -> None:
    class ExternalSourceTranslator(SqlBuildDagsterTranslator):
        def is_asset_node(self, node: Mapping[str, Any]) -> bool:
            return str(node.get("kind")) != "source"

        def is_asset_check(self, check: Mapping[str, Any]) -> bool:
            return str(check.get("kind")) not in {"scenario", "sql_test"}

    dag: Mapping[str, Any] = build_dagster_test_dag()
    translator: SqlBuildDagsterTranslator = ExternalSourceTranslator()

    asset_specs: tuple[Any, ...] = build_asset_specs(dag=dag, translator=translator)
    check_specs: tuple[Any, ...] = build_check_specs(dag=dag, translator=translator)
    specs_by_key: dict[tuple[str, ...], Any] = {tuple(spec.key.path): spec for spec in asset_specs}

    assert test_case.external_asset_key not in specs_by_key
    assert test_case.external_asset_key in {
        tuple(dep.asset_key.path) for dep in specs_by_key[test_case.dependent_asset_key].deps
    }
    assert tuple(spec.name for spec in check_specs) == test_case.expected_check_names


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterAssetSelectionTestCase(
            description="name selector",
            select="orders",
            exclude=None,
            expected_asset_keys=(("analytics", "orders"),),
        ),
        DagsterAssetSelectionTestCase(
            description="tag and name intersection",
            select="tag:daily,orders",
            exclude=None,
            expected_asset_keys=(("analytics", "orders"),),
        ),
        DagsterAssetSelectionTestCase(
            description="path selector",
            select="path:sql/models",
            exclude=None,
            expected_asset_keys=(("analytics", "customers"), ("analytics", "orders")),
        ),
        DagsterAssetSelectionTestCase(
            description="upstream graph expansion",
            select="+orders",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "normalize_email"),
                ("analytics", "orders"),
                ("raw", "orders"),
                ("raw_orders_loader",),
                ("shared_order_feed",),
            ),
        ),
        DagsterAssetSelectionTestCase(
            description="path between models",
            select="raw_orders~orders",
            exclude=None,
            expected_asset_keys=(("analytics", "orders"), ("raw", "orders")),
        ),
        DagsterAssetSelectionTestCase(
            description="exclude selector",
            select="orders customers",
            exclude="customers",
            expected_asset_keys=(("analytics", "orders"),),
        ),
        DagsterAssetSelectionTestCase(
            description="unmatched selector produces an empty Dagster selection",
            select="tag:missing",
            exclude=None,
            expected_asset_keys=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sqlbuild_selectors_when_building_asset_selection_then_resolves_dag_membership(
    test_case: DagsterAssetSelectionTestCase,
) -> None:
    dag: Mapping[str, Any] = build_dagster_test_dag()

    @sqlbuild_assets(dag=dag)
    def assets_def() -> dg.MaterializeResult:
        return dg.MaterializeResult()

    selection: Any = build_sqlbuild_asset_selection(
        sqlbuild_assets=[assets_def],
        dag=dag,
        sqlbuild_select=test_case.select,
        sqlbuild_exclude=test_case.exclude,
    )

    assert tuple(sorted(tuple(key.path) for key in selection.resolve([assets_def]))) == tuple(
        sorted(test_case.expected_asset_keys)
    )


@pytest.mark.parametrize(
    "test_case",
    (
        DagsterAssetSelectionTestCase(
            description="both-direction expansion keeps only ancestors and descendants",
            select="+orders+",
            exclude=None,
            expected_asset_keys=(("analytics", "orders"), ("analytics", "stg_orders")),
        ),
        DagsterAssetSelectionTestCase(
            description="downstream expansion includes every child",
            select="stg_orders+",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "customers"),
                ("analytics", "orders"),
                ("analytics", "stg_orders"),
            ),
        ),
        DagsterAssetSelectionTestCase(
            description="path selector keeps every node on every path",
            select="raw_events~events_report",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "events_joined"),
                ("analytics", "events_left"),
                ("analytics", "events_report"),
                ("analytics", "events_right"),
                ("analytics", "raw_events"),
            ),
        ),
        DagsterAssetSelectionTestCase(
            description="path selector keeps both equal-length diamond branches",
            select="raw_events~events_joined",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "events_joined"),
                ("analytics", "events_left"),
                ("analytics", "events_right"),
                ("analytics", "raw_events"),
            ),
        ),
        DagsterAssetSelectionTestCase(
            description="path selector expands upstream of start and downstream of end",
            select="+events_left~events_joined+",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "events_joined"),
                ("analytics", "events_left"),
                ("analytics", "events_report"),
                ("analytics", "raw_events"),
            ),
        ),
        DagsterAssetSelectionTestCase(
            description="both-direction expansion of a middle node skips siblings",
            select="+events_left+",
            exclude=None,
            expected_asset_keys=(
                ("analytics", "events_joined"),
                ("analytics", "events_left"),
                ("analytics", "events_report"),
                ("analytics", "raw_events"),
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_branching_dag_when_building_asset_selection_then_expands_from_matched_nodes(
    test_case: DagsterAssetSelectionTestCase,
) -> None:
    dag: Mapping[str, Any] = build_dagster_selection_graph_dag()

    @sqlbuild_assets(dag=dag)
    def assets_def() -> dg.MaterializeResult:
        return dg.MaterializeResult()

    selection: Any = build_sqlbuild_asset_selection(
        sqlbuild_assets=[assets_def],
        dag=dag,
        sqlbuild_select=test_case.select,
        sqlbuild_exclude=test_case.exclude,
    )

    assert tuple(sorted(tuple(key.path) for key in selection.resolve([assets_def]))) == tuple(
        sorted(test_case.expected_asset_keys)
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterAssetSpecTestCase(
            description="builds materializable specs and check specs from dag artifact",
            expected_asset_keys=(
                ("raw", "orders"),
                ("shared_order_feed",),
                ("raw_orders_loader",),
                ("analytics", "waffle_types"),
                ("analytics", "normalize_email"),
                ("analytics", "orders"),
                ("analytics", "customers"),
            ),
            expected_model_deps=(("raw", "orders"), ("analytics", "normalize_email")),
            expected_check_names=(
                "audit__not_null__order_id",
                "audit__freshness__loaded_at",
                "scenario__orders_minimal",
                "scenario__customers_minimal",
            ),
            expected_model_selector="orders",
            expected_check_selector="audit:not_null:model:orders:order_id",
            expected_kinds_by_asset_key=(
                (("raw", "orders"), frozenset({"sqlbuild", "source"})),
                (("shared_order_feed",), frozenset({"sqlbuild", "loader"})),
                (("raw_orders_loader",), frozenset({"sqlbuild", "loader"})),
                (("analytics", "waffle_types"), frozenset({"sqlbuild", "seed"})),
                (("analytics", "normalize_email"), frozenset({"sqlbuild", "udf"})),
                (("analytics", "customers"), frozenset({"sqlbuild", "view"})),
                (("analytics", "orders"), frozenset({"sqlbuild", "table"})),
            ),
            expected_group_names=("dagster_project",),
            expected_model_description_fragments=(
                "Clean orders",
                "**SQLBuild SQL:**",
                "MODEL (materialized table);",
            ),
            expected_seed_description=(
                "SQLBuild seed `waffle_types`.\n\n**Source file:** `sql/seeds/waffle_types.csv`"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_dag_when_building_specs_then_maps_assets_deps_and_checks(
    test_case: DagsterAssetSpecTestCase,
) -> None:
    dag: Mapping[str, Any] = build_dagster_test_dag()
    translator: SqlBuildDagsterTranslator = SqlBuildDagsterTranslator()
    asset_specs: tuple[Any, ...] = build_asset_specs(dag=dag, translator=translator)
    check_specs: tuple[Any, ...] = build_check_specs(dag=dag, translator=translator)
    asset_keys: tuple[tuple[str, ...], ...] = tuple(tuple(spec.key.path) for spec in asset_specs)
    specs_by_key: dict[tuple[str, ...], tuple[Any, ...]] = {
        tuple(spec.key.path): (spec,) for spec in asset_specs
    }
    assert len(specs_by_key) == len(asset_specs)
    model_spec: Any = next(iter(specs_by_key.get(("analytics", "orders"), ())))
    seed_spec: Any = next(iter(specs_by_key.get(("analytics", "waffle_types"), ())))

    assert asset_keys == test_case.expected_asset_keys
    assert tuple(tuple(dep.asset_key.path) for dep in model_spec.deps) == (
        test_case.expected_model_deps
    )
    assert tuple(spec.name for spec in check_specs) == test_case.expected_check_names
    assert model_spec.metadata["sqlbuild_selector"] == test_case.expected_model_selector
    assert check_specs[0].metadata["sqlbuild_check_selector"] == test_case.expected_check_selector
    assert {tuple(spec.key.path): frozenset(spec.kinds) for spec in asset_specs} == dict(
        test_case.expected_kinds_by_asset_key
    )
    assert (
        tuple(sorted({spec.group_name for spec in asset_specs})) == test_case.expected_group_names
    )
    assert all(
        fragment in model_spec.description
        for fragment in test_case.expected_model_description_fragments
    )
    assert model_spec.metadata["sql"] == (
        'MODEL (materialized table);\n\nSELECT * FROM __source("raw", "orders")'
    )
    assert seed_spec.description == test_case.expected_seed_description


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterPythonArtifactCompatibilityTestCase(
            description="maps Python DAG artifact additions into asset and check specs",
            expected_asset_keys=(
                ("raw", "orders"),
                ("shared_order_feed",),
                ("raw_orders_loader",),
                ("analytics", "waffle_types"),
                ("analytics", "normalize_email"),
                ("analytics", "orders"),
                ("analytics", "customers"),
                ("asset", "prepare_orders"),
                ("asset", "orders_export"),
            ),
            expected_check_names=(
                "audit__not_null__order_id",
                "audit__freshness__loaded_at",
                "scenario__orders_minimal",
                "scenario__customers_minimal",
                "python_check__check_orders_export",
            ),
            expected_task_deps=(("analytics", "orders"),),
            expected_asset_deps=(("asset", "prepare_orders"),),
            expected_python_kinds_by_asset_key=(
                (("asset", "prepare_orders"), frozenset({"sqlbuild", "asset"})),
                (("asset", "orders_export"), frozenset({"sqlbuild", "asset"})),
            ),
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
def test_given_python_augmented_dag_when_building_specs_then_maps_python_nodes(
    test_case: DagsterPythonArtifactCompatibilityTestCase,
) -> None:
    dag: Mapping[str, Any] = build_python_augmented_dagster_test_dag()
    translator: SqlBuildDagsterTranslator = SqlBuildDagsterTranslator()

    asset_specs: tuple[Any, ...] = build_asset_specs(dag=dag, translator=translator)
    check_specs: tuple[Any, ...] = build_check_specs(dag=dag, translator=translator)
    specs_by_key: dict[tuple[str, ...], tuple[Any, ...]] = {
        tuple(spec.key.path): (spec,) for spec in asset_specs
    }
    assert len(specs_by_key) == len(asset_specs)
    task_spec: Any = next(iter(specs_by_key.get(("asset", "prepare_orders"), ())))
    python_asset_spec: Any = next(iter(specs_by_key.get(("asset", "orders_export"), ())))

    assert tuple(tuple(spec.key.path) for spec in asset_specs) == test_case.expected_asset_keys
    assert tuple(spec.name for spec in check_specs) == test_case.expected_check_names
    assert (
        tuple(tuple(dep.asset_key.path) for dep in task_spec.deps) == test_case.expected_task_deps
    )
    assert tuple(tuple(dep.asset_key.path) for dep in python_asset_spec.deps) == (
        test_case.expected_asset_deps
    )
    assert {
        tuple(spec.key.path): frozenset(spec.kinds) for spec in (task_spec, python_asset_spec)
    } == dict(test_case.expected_python_kinds_by_asset_key)
    assert task_spec.group_name == test_case.expected_task_group
    assert python_asset_spec.group_name == test_case.expected_asset_group
    assert all(key in python_asset_spec.metadata for key in test_case.expected_asset_metadata_keys)


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterDecoratorTestCase(
            description="decorates user function as dagster assets definition",
            expected_asset_keys=(
                ("raw", "orders"),
                ("shared_order_feed",),
                ("raw_orders_loader",),
                ("analytics", "waffle_types"),
                ("analytics", "normalize_email"),
                ("analytics", "customers"),
                ("analytics", "orders"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_assets_decorator_when_applied_then_returns_assets_definition(
    test_case: DagsterDecoratorTestCase,
) -> None:
    @sqlbuild_assets(dag=build_dagster_test_dag())
    def assets_def() -> dg.MaterializeResult:
        return dg.MaterializeResult()

    assert tuple(sorted(tuple(key.path) for key in assets_def.keys)) == tuple(
        sorted(test_case.expected_asset_keys)
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterScenarioCheckDecoratorTestCase(
            description="decorates user function as scenario-only asset checks definition",
            expected_check_names=("scenario__orders_minimal", "scenario__customers_minimal"),
            unexpected_check_names=("audit__not_null__order_id", "audit__freshness__loaded_at"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_scenario_checks_decorator_when_applied_then_returns_check_definition(
    test_case: DagsterScenarioCheckDecoratorTestCase,
) -> None:
    @sqlbuild_scenario_checks(dag=build_dagster_test_dag())
    def checks_def() -> dg.AssetCheckResult:
        return dg.AssetCheckResult(
            passed=True,
            asset_key=dg.AssetKey(["analytics", "orders"]),
            check_name="scenario__orders_minimal",
        )

    check_names: tuple[str, ...] = tuple(spec.name for spec in checks_def.check_specs)

    assert check_names == test_case.expected_check_names
    assert not set(test_case.unexpected_check_names).intersection(check_names)


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterScenarioCheckDecoratorTestCase(
            description="builds scenario-only check specs",
            expected_check_names=("scenario__orders_minimal", "scenario__customers_minimal"),
            unexpected_check_names=("audit__not_null__order_id", "audit__freshness__loaded_at"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_dag_when_building_scenario_check_specs_then_filters_non_scenarios(
    test_case: DagsterScenarioCheckDecoratorTestCase,
) -> None:
    check_specs: tuple[Any, ...] = build_scenario_check_specs(
        dag=build_dagster_test_dag(),
        translator=SqlBuildDagsterTranslator(),
    )
    check_names: tuple[str, ...] = tuple(spec.name for spec in check_specs)

    assert check_names == test_case.expected_check_names
    assert not set(test_case.unexpected_check_names).intersection(check_names)


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterConflictingInputTestCase(
            description="assets decorator rejects dag and project together",
            expected_error_fragment="sqlbuild_assets received both 'dag' and 'project'",
            expected_error_code="I002",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dag_and_project_when_building_assets_decorator_then_raises_coded_input_error(
    test_case: DagsterConflictingInputTestCase,
) -> None:
    project: SqlBuildProject = SqlBuildProject(project_dir=Path("."))

    with pytest.raises(DagsterDagInputError, match=test_case.expected_error_fragment) as error:
        sqlbuild_assets(dag=build_dagster_test_dag(), project=project)

    assert error.value.code == test_case.expected_error_code


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterConflictingInputTestCase(
            description="scenario checks decorator rejects dag and project together",
            expected_error_fragment="sqlbuild_scenario_checks received both 'dag' and 'project'",
            expected_error_code="I002",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dag_and_project_when_building_scenario_checks_then_raises_coded_input_error(
    test_case: DagsterConflictingInputTestCase,
) -> None:
    project: SqlBuildProject = SqlBuildProject(project_dir=Path("."))

    with pytest.raises(DagsterDagInputError, match=test_case.expected_error_fragment) as error:
        sqlbuild_scenario_checks(dag=build_dagster_test_dag(), project=project)

    assert error.value.code == test_case.expected_error_code


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterAssetCheckFilterTestCase(
            description="excludes scenario checks when requested",
            expected_check_names=("audit__not_null__order_id", "audit__freshness__loaded_at"),
            unexpected_check_names=("scenario__orders_minimal", "scenario__customers_minimal"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sqlbuild_dag_when_building_check_specs_then_can_exclude_scenarios(
    test_case: DagsterAssetCheckFilterTestCase,
) -> None:
    check_specs: tuple[Any, ...] = build_check_specs(
        dag=build_dagster_test_dag(),
        translator=SqlBuildDagsterTranslator(),
        include_scenarios=False,
    )
    check_names: tuple[str, ...] = tuple(spec.name for spec in check_specs)

    assert check_names == test_case.expected_check_names
    assert not set(test_case.unexpected_check_names).intersection(check_names)


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterAssetSelectionErrorTestCase(
            description="rejects a path whose end is not downstream of its start",
            select="events_report~raw_events",
            expected_error_fragment="no SQLBuild DAG path exists between",
        ),
        DagsterAssetSelectionErrorTestCase(
            description="rejects a path from a node to itself like the planner",
            select="orders~orders",
            expected_error_fragment="no SQLBuild DAG path exists between",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unreachable_path_selector_when_building_asset_selection_then_raises_input_error(
    test_case: DagsterAssetSelectionErrorTestCase,
) -> None:
    dag: Mapping[str, Any] = build_dagster_selection_graph_dag()

    @sqlbuild_assets(dag=dag)
    def assets_def() -> dg.MaterializeResult:
        return dg.MaterializeResult()

    with pytest.raises(DagsterDagInputError, match=test_case.expected_error_fragment):
        build_sqlbuild_asset_selection(
            sqlbuild_assets=[assets_def],
            dag=dag,
            sqlbuild_select=test_case.select,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterUnitTestSelectorTestCase(
            description="rejects a unit-test selector like other non-test commands",
            select="test:orders_status_rules",
            expected_code="S013",
            expected_message=(
                "selector 'test:orders_status_rules' selects a unit test; only `sqb test` and "
                "`sqb build` accept unit-test selectors"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unit_test_selector_when_building_asset_selection_then_raises_s013(
    test_case: DagsterUnitTestSelectorTestCase,
) -> None:
    dag: Mapping[str, Any] = build_dagster_selection_graph_dag()

    @sqlbuild_assets(dag=dag)
    def assets_def() -> dg.MaterializeResult:
        return dg.MaterializeResult()

    with pytest.raises(PlannerInputError) as raised:
        build_sqlbuild_asset_selection(
            sqlbuild_assets=[assets_def],
            dag=dag,
            sqlbuild_select=test_case.select,
        )

    assert raised.value.code == test_case.expected_code
    assert raised.value.message == test_case.expected_message


@pytest.mark.parametrize(
    "test_case",
    [
        DagsterCheckTranslatorCallTestCase(
            description="excluded scenario checks are still offered to the translator",
            include_scenarios=False,
            expected_is_asset_check_ids=(
                "audit:not_null:model:orders:order_id",
                "audit:freshness:source:raw_orders:loaded_at",
                "sql_scenario:orders_minimal",
                "sql_scenario:customers_minimal",
            ),
            expected_checked_node_last_key="project_name",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_check_filter_when_building_check_specs_then_translator_sees_every_check(
    test_case: DagsterCheckTranslatorCallTestCase,
) -> None:
    translator: RecordingDagsterTranslator = RecordingDagsterTranslator()

    _ = build_check_specs(
        dag=build_dagster_test_dag(),
        translator=translator,
        include_scenarios=test_case.include_scenarios,
    )
    checks: list[Mapping[str, Any]] = translator.asset_checks
    checked_nodes: list[Mapping[str, Any]] = translator.asset_nodes

    assert tuple(str(check["id"]) for check in checks) == test_case.expected_is_asset_check_ids
    assert checked_nodes
    assert all(
        tuple(node)[-1] == test_case.expected_checked_node_last_key for node in checked_nodes
    )
