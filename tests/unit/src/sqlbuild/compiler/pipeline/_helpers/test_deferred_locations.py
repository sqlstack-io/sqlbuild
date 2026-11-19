"""Tests for deferred target resolution."""

from __future__ import annotations

from typing import cast

import pytest

from sqlbuild._native import (
    CompiledRelationLocation,
    TargetConfig,
    build_deferred_locations,
    gather_deferred_relations,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import RelationInfo
from sqlbuild.adapters.bigquery.classes.bigquery_adapter import BigQueryAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.unit.src.sqlbuild.compiler.pipeline._helpers._test_types import (
    DeferredListingTestCase,
    DeferredTargetTestCase,
)
from tests.unit.src.sqlbuild.compiler.pipeline._helpers.helpers import (
    ListingAdapter,
    build_single_model_project,
    deferred_location,
)

_DEFERRED_RELATIONS: tuple[RelationInfo, ...] = (
    RelationInfo(database="analytics", schema="prod", name="orders", relation_type="table"),
    RelationInfo(database="archive", schema="prod", name="order_history", relation_type="table"),
    RelationInfo(database="archive", schema="audit", name="refunds", relation_type="table"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        DeferredTargetTestCase(
            description="preserve schema returns logical schema",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema="analytics",
            logical_database=None,
            env_schema="preserve",
            env_database=None,
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="analytics",
            expected_database=None,
            expected_qualified_name="analytics.test_model",
        ),
        DeferredTargetTestCase(
            description="literal env schema overrides logical schema",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema="analytics",
            logical_database=None,
            env_schema="prod_analytics",
            env_database=None,
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="prod_analytics",
            expected_database=None,
            expected_qualified_name="prod_analytics.test_model",
        ),
        DeferredTargetTestCase(
            description="template env schema resolves CTX with logical value",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema="analytics",
            logical_database=None,
            env_schema="prod_${CTX:schema}",
            env_database=None,
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="prod_analytics",
            expected_database=None,
            expected_qualified_name="prod_analytics.test_model",
        ),
        DeferredTargetTestCase(
            description="template env schema resolves vars",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema=None,
            logical_database=None,
            env_schema="dev_${user}",
            env_database=None,
            effective_vars={"user": "kevin"},
            default_schema="main",
            default_database=None,
            expected_schema="dev_kevin",
            expected_database=None,
            expected_qualified_name="dev_kevin.test_model",
        ),
        DeferredTargetTestCase(
            description="none env schema falls through to adapter default",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema=None,
            logical_database=None,
            env_schema=None,
            env_database=None,
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="main",
            expected_database=None,
            expected_qualified_name="main.test_model",
        ),
        DeferredTargetTestCase(
            description="database and schema both resolve for three-part name",
            render_qualified_name=DuckDbAdapter().render_qualified_name,
            logical_schema="analytics",
            logical_database="warehouse",
            env_schema="preserve",
            env_database="prod_warehouse",
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="analytics",
            expected_database="prod_warehouse",
            expected_qualified_name="prod_warehouse.analytics.test_model",
        ),
        DeferredTargetTestCase(
            description="bigquery deferred locations use adapter-qualified names",
            render_qualified_name=BigQueryAdapter().render_qualified_name,
            logical_schema="analytics",
            logical_database="warehouse",
            env_schema="prod",
            env_database="example-project",
            effective_vars={},
            default_schema="main",
            default_database=None,
            expected_schema="prod",
            expected_database="example-project",
            expected_qualified_name="`example-project.prod.test_model`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deferred_target_config_when_building_targets_then_resolves_expected_naming(
    test_case: DeferredTargetTestCase,
) -> None:
    project: CompiledProjectView = build_single_model_project(
        logical_schema=test_case.logical_schema,
        logical_database=test_case.logical_database,
        physical_schema=test_case.logical_schema or test_case.default_schema,
        physical_database=test_case.logical_database or test_case.default_database,
    )
    deferred_target_config: TargetConfig = TargetConfig(
        schema=test_case.env_schema,
        database=test_case.env_database,
    )

    locations: dict[str, CompiledRelationLocation] = build_deferred_locations(
        project=project,
        deferred_target_config=deferred_target_config,
        effective_vars=test_case.effective_vars,
        default_schema=test_case.default_schema,
        default_database=test_case.default_database,
        render_qualified_name=test_case.render_qualified_name,
    )

    result: CompiledRelationLocation = locations["test_model"]
    assert result.schema == test_case.expected_schema
    assert result.database == test_case.expected_database
    assert result.qualified_name == test_case.expected_qualified_name


@pytest.mark.parametrize(
    "test_case",
    [
        DeferredListingTestCase(
            description="locations in two databases are listed in each database",
            locations=(
                ("analytics", "prod", "orders"),
                ("archive", "prod", "order_history"),
                ("archive", "audit", "refunds"),
            ),
            expected_listings=[("analytics", ("prod",)), ("archive", ("audit", "prod"))],
            expected_names=["order_history", "orders", "refunds"],
        ),
        DeferredListingTestCase(
            description="locations in one database are listed once",
            locations=(("archive", "prod", "order_history"), ("archive", "audit", "refunds")),
            expected_listings=[("archive", ("audit", "prod"))],
            expected_names=["order_history", "refunds"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deferred_locations_when_gathering_relations_then_lists_every_database(
    test_case: DeferredListingTestCase,
) -> None:
    adapter: ListingAdapter = ListingAdapter(relations=_DEFERRED_RELATIONS)

    relations: dict[str, RelationInfo] = gather_deferred_relations(
        adapter=cast(BaseAdapter, adapter),
        connection=None,
        deferred_locations={
            name: deferred_location(database=database, schema=schema, name=name)
            for database, schema, name in test_case.locations
        },
    )

    assert adapter.listed == test_case.expected_listings
    assert sorted(relations) == test_case.expected_names
