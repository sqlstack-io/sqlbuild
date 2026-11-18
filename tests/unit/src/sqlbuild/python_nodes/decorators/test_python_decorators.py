"""Tests for public Python node decorator metadata."""

from __future__ import annotations

from typing import Any, cast

import pytest

from sqlbuild.assets import asset, get_asset_definition
from sqlbuild.checks import check, get_check_definition
from sqlbuild.errors.contracts.exceptions import SharedInputError
from sqlbuild.factories import factory, get_factory_definition
from sqlbuild.hooks import HookDefinition, get_hook_definition, hook
from sqlbuild.python_nodes.models import (
    AssetDefinition,
    CheckDefinition,
    ColumnLineageRef,
    FactoryDefinition,
    NodeSetRef,
    RetryPolicy,
    TaskDefinition,
)
from sqlbuild.python_nodes.types import NodeSetKind, PythonCheckSeverity
from sqlbuild.refs import job, model, seed, tag
from sqlbuild.tasks import get_task_definition, task
from tests.unit.src.sqlbuild.python_nodes.decorators._test_types import (
    AssetDecoratorMetadataTestCase,
    CheckDecoratorMetadataTestCase,
    FactoryDecoratorErrorTestCase,
    FactoryDecoratorMetadataTestCase,
    HookReadsTestCase,
    NodeSetDependencyTestCase,
    TaskDecoratorMetadataTestCase,
)
from tests.unit.src.sqlbuild.python_nodes.decorators.helpers import upstream_asset, upstream_task


@pytest.mark.parametrize(
    "test_case",
    [
        TaskDecoratorMetadataTestCase(
            description="stores task metadata and normalizes single dependency",
            expected_name="fetch_window",
            expected_dep_count=1,
            expected_tags=("api", "hourly"),
            expected_group="ingestion",
            expected_description="Fetch an API window.",
            expected_meta={"owner": "data-eng"},
            expected_retry=RetryPolicy(
                max_attempts=3,
                retry_on=[TimeoutError, ConnectionError],
                initial_delay_seconds=0.5,
                jitter=False,
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_task_decorator_when_reading_definition_then_returns_metadata(
    test_case: TaskDecoratorMetadataTestCase,
) -> None:
    @task(
        name=test_case.expected_name,
        depends_on=upstream_task,
        tags=test_case.expected_tags,
        group=test_case.expected_group,
        meta=test_case.expected_meta,
        retry=test_case.expected_retry,
    )
    def fetch_window(_ctx: object) -> dict[str, object]:
        """Fetch an API window."""

        return {"window": "2026-05-30"}

    definition: TaskDefinition | None = get_task_definition(fetch_window)

    assert definition == TaskDefinition(
        name=test_case.expected_name,
        depends_on=(upstream_task,),
        tags=test_case.expected_tags,
        group=test_case.expected_group,
        description=test_case.expected_description,
        meta=test_case.expected_meta,
        retry=test_case.expected_retry,
    )
    assert definition is not None
    assert len(definition.depends_on) == test_case.expected_dep_count
    assert get_asset_definition(fetch_window) is None


@pytest.mark.parametrize(
    "test_case",
    [
        AssetDecoratorMetadataTestCase(
            description="stores asset schema metadata and column lineage",
            expected_name="export_customers",
            expected_dep_count=1,
            expected_tags=("exports",),
            expected_group="customer_exports",
            expected_description="Published customer export.",
            expected_meta={"format": "parquet"},
            expected_column_names=("customer_id", "email"),
            expected_column_types=("string", "string"),
            expected_column_descriptions=("Stable customer id", None),
            expected_column_lineage={
                "customer_id": (ColumnLineageRef(node="dim_customers", column="customer_id"),),
                "email": (ColumnLineageRef(node="dim_customers", column="email"),),
            },
            expected_retry=RetryPolicy(max_attempts=2, retry_on=TimeoutError, jitter=False),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_asset_decorator_when_reading_definition_then_returns_metadata(
    test_case: AssetDecoratorMetadataTestCase,
) -> None:
    @asset(
        name=test_case.expected_name,
        depends_on=[upstream_asset],
        tags=test_case.expected_tags,
        group=test_case.expected_group,
        description=test_case.expected_description,
        meta=test_case.expected_meta,
        columns=[
            {
                "name": "customer_id",
                "type": "string",
                "description": "Stable customer id",
                "meta": {"pii": False},
            },
            {"name": "email", "type": "string", "nullable": True},
        ],
        column_lineage={
            "customer_id": [{"node": "dim_customers", "column": "customer_id"}],
            "email": [{"node": "dim_customers", "column": "email"}],
        },
        retry=test_case.expected_retry,
    )
    def export_customers(_ctx: object) -> dict[str, object]:
        return {"uri": "s3://exports/customers.parquet"}

    definition: AssetDefinition | None = get_asset_definition(export_customers)

    assert definition is not None
    assert definition.name == test_case.expected_name
    assert definition.depends_on == (upstream_asset,)
    assert len(definition.depends_on) == test_case.expected_dep_count
    assert definition.tags == test_case.expected_tags
    assert definition.group == test_case.expected_group
    assert definition.description == test_case.expected_description
    assert definition.meta == test_case.expected_meta
    assert definition.retry == test_case.expected_retry
    assert tuple(column.name for column in definition.columns) == test_case.expected_column_names
    assert tuple(column.type for column in definition.columns) == test_case.expected_column_types
    assert tuple(column.description for column in definition.columns) == (
        test_case.expected_column_descriptions
    )
    assert definition.column_lineage == test_case.expected_column_lineage
    assert get_task_definition(export_customers) is None


@pytest.mark.parametrize(
    "test_case",
    [
        CheckDecoratorMetadataTestCase(
            description="stores check metadata and normalizes dependency list",
            expected_name="export_customers_exists",
            expected_dep_count=1,
            expected_severity=PythonCheckSeverity.WARN,
            expected_tags=("exports",),
            expected_group="customer_exports",
            expected_description="Validate exported customers exist.",
            expected_meta={"owner": "data-eng"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_check_decorator_when_reading_definition_then_returns_metadata(
    test_case: CheckDecoratorMetadataTestCase,
) -> None:
    @check(
        name=test_case.expected_name,
        depends_on=[upstream_asset],
        severity="warn",
        tags=test_case.expected_tags,
        group=test_case.expected_group,
        meta=test_case.expected_meta,
    )
    def export_customers_exists(_ctx: object) -> bool:
        """Validate exported customers exist."""

        return True

    definition: CheckDefinition | None = get_check_definition(export_customers_exists)

    assert definition == CheckDefinition(
        name=test_case.expected_name,
        depends_on=(upstream_asset,),
        severity=test_case.expected_severity,
        tags=test_case.expected_tags,
        group=test_case.expected_group,
        description=test_case.expected_description,
        meta=test_case.expected_meta,
    )
    assert definition is not None
    assert len(definition.depends_on) == test_case.expected_dep_count
    assert get_asset_definition(export_customers_exists) is None
    assert get_task_definition(export_customers_exists) is None


@pytest.mark.parametrize(
    "test_case",
    [
        FactoryDecoratorMetadataTestCase(
            description="stores factory metadata and returns original function",
            expected_name="generated_nodes",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_factory_decorator_when_reading_definition_then_returns_metadata(
    test_case: FactoryDecoratorMetadataTestCase,
) -> None:
    def generated_nodes() -> list[object]:
        return []

    decorated: object = factory(generated_nodes)
    definition: FactoryDefinition | None = get_factory_definition(generated_nodes)

    assert decorated is generated_nodes
    assert definition == FactoryDefinition(name=test_case.expected_name)
    assert get_task_definition(generated_nodes) is None
    assert get_asset_definition(generated_nodes) is None
    assert get_check_definition(generated_nodes) is None


@pytest.mark.parametrize(
    "test_case",
    [
        FactoryDecoratorErrorTestCase(
            description="factory rejects argument decorator usage",
            expected_error_fragment="required positional argument",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_factory_decorator_when_called_without_function_then_raises(
    test_case: FactoryDecoratorErrorTestCase,
) -> None:
    factory_ref: Any = cast(Any, factory)
    with pytest.raises(TypeError, match=test_case.expected_error_fragment):
        factory_ref()


@pytest.mark.parametrize(
    "test_case",
    [
        HookReadsTestCase(
            description="single reference becomes one read",
            reads=model("orders"),
            expected_reads=("orders",),
        ),
        HookReadsTestCase(
            description="duplicate references keep first-seen order",
            reads=[model("orders"), seed("customers"), model("orders")],
            expected_reads=("orders", "customers"),
        ),
        HookReadsTestCase(
            description="None as the first invalid item is dropped",
            reads=[model("orders"), None, model("products")],
            expected_reads=("orders", "products"),
        ),
        HookReadsTestCase(
            description="items after a leading None are filtered, not rejected",
            reads=[None, 5, model("orders")],
            expected_reads=("orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_hook_reads_when_decorating_then_keeps_distinct_references(
    test_case: HookReadsTestCase,
) -> None:
    def refresh_orders() -> None:
        return None

    decorated: Any = cast(Any, hook(reads=cast(Any, test_case.reads)))(refresh_orders)
    definition: HookDefinition | None = get_hook_definition(decorated)

    assert definition is not None
    assert tuple(read.name for read in definition.reads) == test_case.expected_reads


@pytest.mark.parametrize(
    "test_case",
    [
        HookReadsTestCase(
            description="a non-reference item is rejected",
            reads=[model("orders"), "customers"],
            expected_error="got str",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_reference_hook_read_when_decorating_then_raises(
    test_case: HookReadsTestCase,
) -> None:
    def refresh_orders() -> None:
        return None

    with pytest.raises(SharedInputError, match=test_case.expected_error):
        cast(Any, hook(reads=cast(Any, test_case.reads)))(refresh_orders)


@pytest.mark.parametrize(
    "test_case",
    [
        NodeSetDependencyTestCase(
            description="a job dependency",
            dependency=job("nightly_job"),
            expected_dependency=NodeSetRef(kind=NodeSetKind.JOB, name="nightly_job"),
            expected_text='job("nightly_job")',
        ),
        NodeSetDependencyTestCase(
            description="a tag dependency with an exclude",
            dependency=tag("finance", exclude="tag:draft orders"),
            expected_dependency=NodeSetRef(
                kind=NodeSetKind.TAG, name="finance", exclude="tag:draft orders"
            ),
            expected_text='tag("finance", exclude="tag:draft orders")',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_job_or_tag_dependency_when_decorating_a_task_then_keeps_it_as_declared(
    test_case: NodeSetDependencyTestCase,
) -> None:
    @task(depends_on=cast(Any, test_case.dependency))
    def report() -> None:
        """Test task report."""

    definition: TaskDefinition | None = get_task_definition(report)

    assert definition is not None
    assert definition.depends_on == (test_case.expected_dependency,)
    assert str(definition.depends_on[0]) == test_case.expected_text
