from __future__ import annotations

from dataclasses import replace

import pytest

from sqlbuild._native import (
    ColumnLineage,
    ColumnLineageConfidence,
    ColumnLineageMode,
    ColumnTransformKind,
    CompiledLineageColumnFact,
    CompiledLineageSourceFact,
    CompiledResourceType,
    ModelColumnLineage,
    ProjectColumnLineage,
    compiled_model_record,
)
from sqlbuild.compiler.compile.types import (
    CompiledModelView,
    CompiledProjectView,
)
from tests.unit.src.sqlbuild.compiler.lineage._test_types import (
    ColumnLineageAnalyzerTestCase,
    ExpectedCountTestCase,
    ProjectLineageGraphTestCase,
    RichLineageAnalysisScalingTestCase,
    SqlAnalysisDisabledLineageTestCase,
)
from tests.unit.src.sqlbuild.compiler.lineage.helpers import (
    edge_label,
    make_chain_models,
    make_compiled_model,
    make_compiled_project,
    make_compiled_seed,
    project_column_lineage,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnLineageAnalyzerTestCase(
            description="infers standard model column passthrough",
            model_name="orders_out",
            query_sql='SELECT order_id FROM __ref("orders")',
            inferred_columns=("order_id",),
            upstream_model_columns={"orders": ("order_id",)},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.order_id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="infers aliased model column passthrough",
            model_name="orders_out",
            query_sql='SELECT id AS order_id FROM __ref("orders")',
            inferred_columns=("order_id",),
            upstream_model_columns={"orders": ("id",)},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="classifies casts",
            model_name="payments_out",
            query_sql='SELECT CAST(amount AS DECIMAL(10, 2)) AS amount_decimal FROM __ref("payments")',
            inferred_columns=("amount_decimal",),
            upstream_model_columns={"payments": ("amount",)},
            upstream_seed_columns={},
            expected_column="amount_decimal",
            expected_upstream_columns=("model:payments.amount",),
            expected_transform_kind=ColumnTransformKind.CAST,
        ),
        ColumnLineageAnalyzerTestCase(
            description="infers multi-column arithmetic expression",
            model_name="orders_out",
            query_sql='SELECT quantity * price_cents AS total_cents FROM __ref("orders")',
            inferred_columns=("total_cents",),
            upstream_model_columns={"orders": ("quantity", "price_cents")},
            upstream_seed_columns={},
            expected_column="total_cents",
            expected_upstream_columns=("model:orders.price_cents", "model:orders.quantity"),
            expected_transform_kind=ColumnTransformKind.EXPRESSION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="infers join expression across model and seed",
            model_name="orders_out",
            query_sql=(
                'SELECT o.quantity * w.price_cents AS total_cents FROM __ref("orders") o '
                'JOIN __seed("waffle_types") w ON o.waffle_type_id = w.waffle_type_id'
            ),
            inferred_columns=("total_cents",),
            upstream_model_columns={"orders": ("quantity", "waffle_type_id")},
            upstream_seed_columns={"waffle_types": ("price_cents", "waffle_type_id")},
            expected_column="total_cents",
            expected_upstream_columns=("model:orders.quantity", "seed:waffle_types.price_cents"),
            expected_transform_kind=ColumnTransformKind.EXPRESSION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="classifies aggregations",
            model_name="payments_out",
            query_sql=(
                'SELECT customer_id, SUM(amount_cents) AS revenue_cents FROM __ref("payments") '
                "GROUP BY customer_id"
            ),
            inferred_columns=("customer_id", "revenue_cents"),
            upstream_model_columns={"payments": ("customer_id", "amount_cents")},
            upstream_seed_columns={},
            expected_column="revenue_cents",
            expected_upstream_columns=("model:payments.amount_cents",),
            expected_transform_kind=ColumnTransformKind.AGGREGATION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="collapses CTE upstream dependency",
            model_name="payments_out",
            query_sql=(
                'WITH x AS (SELECT amount_cents FROM __ref("payments")) SELECT amount_cents FROM x'
            ),
            inferred_columns=("amount_cents",),
            upstream_model_columns={"payments": ("amount_cents",)},
            upstream_seed_columns={},
            expected_column="amount_cents",
            expected_upstream_columns=("model:payments.amount_cents",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="infers both branches of a union",
            model_name="unioned",
            query_sql='SELECT id FROM __ref("a") UNION ALL SELECT id FROM __ref("b")',
            inferred_columns=("id",),
            upstream_model_columns={"a": ("id",), "b": ("id",)},
            upstream_seed_columns={},
            expected_column="id",
            expected_upstream_columns=("model:a.id", "model:b.id"),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="classifies constants with no upstream columns",
            model_name="constants",
            query_sql="SELECT 1 AS one",
            inferred_columns=("one",),
            upstream_model_columns={},
            upstream_seed_columns={},
            expected_column="one",
            expected_upstream_columns=(),
            expected_transform_kind=ColumnTransformKind.CONSTANT,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_building_column_lineage_then_infers_expected_column(
    test_case: ColumnLineageAnalyzerTestCase,
) -> None:
    upstream_models: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=model_name,
            query_sql="SELECT 1",
            inferred_columns=columns,
        )
        for model_name, columns in test_case.upstream_model_columns.items()
    )
    target_model: CompiledModelView = make_compiled_model(
        name=test_case.model_name,
        query_sql=test_case.query_sql,
        inferred_columns=test_case.inferred_columns,
    )
    project: CompiledProjectView = make_compiled_project(
        models=upstream_models + (target_model,),
        seeds=tuple(
            make_compiled_seed(name=seed_name, columns=columns)
            for seed_name, columns in test_case.upstream_seed_columns.items()
        ),
    )

    result: ProjectColumnLineage | None = project_column_lineage(project=project)

    assert result is not None
    model_lineage: ModelColumnLineage = result.models[test_case.model_name]
    columns_by_output: dict[str, tuple[ColumnLineage, ...]] = {
        column.output_column: (column,) for column in model_lineage.columns
    }
    assert len(columns_by_output) == len(model_lineage.columns)
    column_lineage: ColumnLineage = next(iter(columns_by_output.get(test_case.expected_column, ())))
    upstream_columns: tuple[str, ...] = tuple(
        sorted(
            f"{CompiledResourceType(source.resource_type).value}:{source.resource_name}.{source.column_name}"
            for source in column_lineage.upstream_columns
        )
    )
    assert upstream_columns == tuple(sorted(test_case.expected_upstream_columns))
    assert column_lineage.transform_kind == test_case.expected_transform_kind


@pytest.mark.parametrize(
    "test_case",
    (
        ColumnLineageAnalyzerTestCase(
            description="fast infers standard model column passthrough",
            model_name="orders_out",
            query_sql='SELECT order_id FROM __ref("orders")',
            inferred_columns=("order_id",),
            upstream_model_columns={"orders": ("order_id",)},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.order_id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast infers aliased model column passthrough",
            model_name="orders_out",
            query_sql='SELECT id AS order_id FROM __ref("orders")',
            inferred_columns=("order_id",),
            upstream_model_columns={"orders": ("id",)},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast classifies casts",
            model_name="payments_out",
            query_sql='SELECT CAST(amount AS DECIMAL(10, 2)) AS amount_decimal FROM __ref("payments")',
            inferred_columns=("amount_decimal",),
            upstream_model_columns={"payments": ("amount",)},
            upstream_seed_columns={},
            expected_column="amount_decimal",
            expected_upstream_columns=("model:payments.amount",),
            expected_transform_kind=ColumnTransformKind.CAST,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast infers multi-column arithmetic expression",
            model_name="orders_out",
            query_sql='SELECT quantity * price_cents AS total_cents FROM __ref("orders")',
            inferred_columns=("total_cents",),
            upstream_model_columns={"orders": ("quantity", "price_cents")},
            upstream_seed_columns={},
            expected_column="total_cents",
            expected_upstream_columns=("model:orders.price_cents", "model:orders.quantity"),
            expected_transform_kind=ColumnTransformKind.EXPRESSION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast infers join expression across model and seed",
            model_name="orders_out",
            query_sql=(
                'SELECT o.quantity * w.price_cents AS total_cents FROM __ref("orders") o '
                'JOIN __seed("waffle_types") w ON o.waffle_type_id = w.waffle_type_id'
            ),
            inferred_columns=("total_cents",),
            upstream_model_columns={"orders": ("quantity", "waffle_type_id")},
            upstream_seed_columns={"waffle_types": ("price_cents", "waffle_type_id")},
            expected_column="total_cents",
            expected_upstream_columns=("model:orders.quantity", "seed:waffle_types.price_cents"),
            expected_transform_kind=ColumnTransformKind.EXPRESSION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast classifies aggregations",
            model_name="payments_out",
            query_sql=(
                'SELECT customer_id, SUM(amount_cents) AS revenue_cents FROM __ref("payments") '
                "GROUP BY customer_id"
            ),
            inferred_columns=("customer_id", "revenue_cents"),
            upstream_model_columns={"payments": ("customer_id", "amount_cents")},
            upstream_seed_columns={},
            expected_column="revenue_cents",
            expected_upstream_columns=("model:payments.amount_cents",),
            expected_transform_kind=ColumnTransformKind.AGGREGATION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="fast classifies constants with no upstream columns",
            model_name="constants",
            query_sql="SELECT 1 AS one",
            inferred_columns=("one",),
            upstream_model_columns={},
            upstream_seed_columns={},
            expected_column="one",
            expected_upstream_columns=(),
            expected_transform_kind=ColumnTransformKind.CONSTANT,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_building_fast_column_lineage_then_infers_expected_column(
    test_case: ColumnLineageAnalyzerTestCase,
) -> None:
    upstream_models: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=model_name,
            query_sql="SELECT 1",
            inferred_columns=columns,
        )
        for model_name, columns in test_case.upstream_model_columns.items()
    )
    target_model: CompiledModelView = make_compiled_model(
        name=test_case.model_name,
        query_sql=test_case.query_sql,
        inferred_columns=test_case.inferred_columns,
    )
    project: CompiledProjectView = make_compiled_project(
        models=upstream_models + (target_model,),
        seeds=tuple(
            make_compiled_seed(name=seed_name, columns=columns)
            for seed_name, columns in test_case.upstream_seed_columns.items()
        ),
    )

    result: ProjectColumnLineage | None = project_column_lineage(
        project=project,
        dialect="duckdb",
        mode=ColumnLineageMode.FAST,
    )

    assert result is not None
    model_lineage: ModelColumnLineage = result.models[test_case.model_name]
    columns_by_output: dict[str, tuple[ColumnLineage, ...]] = {
        column.output_column: (column,) for column in model_lineage.columns
    }
    assert len(columns_by_output) == len(model_lineage.columns)
    column_lineage: ColumnLineage = next(iter(columns_by_output.get(test_case.expected_column, ())))
    upstream_columns: tuple[str, ...] = tuple(
        sorted(
            f"{CompiledResourceType(source.resource_type).value}:{source.resource_name}.{source.column_name}"
            for source in column_lineage.upstream_columns
        )
    )
    assert upstream_columns == tuple(sorted(test_case.expected_upstream_columns))
    assert column_lineage.transform_kind == test_case.expected_transform_kind


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnLineageAnalyzerTestCase(
            description="expands select star from known upstream schema with medium confidence",
            model_name="orders_out",
            query_sql='SELECT * FROM __ref("orders")',
            inferred_columns=None,
            upstream_model_columns={"orders": ("order_id", "amount_cents")},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.order_id",),
            expected_transform_kind=ColumnTransformKind.STAR,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_select_star_when_building_column_lineage_then_expands_known_schema_columns(
    test_case: ColumnLineageAnalyzerTestCase,
) -> None:
    project: CompiledProjectView = make_compiled_project(
        models=(
            make_compiled_model(
                name="orders",
                query_sql="SELECT 1",
                inferred_columns=("order_id", "amount_cents"),
            ),
            make_compiled_model(
                name=test_case.model_name,
                query_sql=test_case.query_sql,
                inferred_columns=test_case.inferred_columns,
            ),
        )
    )

    result: ProjectColumnLineage | None = project_column_lineage(project=project)

    assert result is not None
    model_lineage: ModelColumnLineage = result.models[test_case.model_name]
    columns_by_output: dict[str, tuple[ColumnLineage, ...]] = {
        column.output_column: (column,) for column in model_lineage.columns
    }
    assert len(columns_by_output) == len(model_lineage.columns)
    order_id_lineage: ColumnLineage = next(iter(columns_by_output.get("order_id", ())))
    assert model_lineage.has_star
    assert order_id_lineage.transform_kind == test_case.expected_transform_kind
    assert order_id_lineage.confidence == ColumnLineageConfidence.MEDIUM


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectLineageGraphTestCase(
            description="finds column consumers through linear project graph",
            expected_consumers=("a.id->b.id",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_linear_project_when_listing_column_consumers_then_returns_expected_edges(
    test_case: ProjectLineageGraphTestCase,
) -> None:
    project: CompiledProjectView = make_compiled_project(
        models=(
            make_compiled_model(name="a", query_sql="SELECT 1 AS id", inferred_columns=("id",)),
            make_compiled_model(
                name="b", query_sql='SELECT id FROM __ref("a")', inferred_columns=("id",)
            ),
            make_compiled_model(
                name="c", query_sql='SELECT id FROM __ref("b")', inferred_columns=("id",)
            ),
        )
    )

    result: ProjectColumnLineage | None = project_column_lineage(project=project)

    assert result is not None
    consumers: tuple[str, ...] = tuple(
        edge_label(
            edge.source.resource_name,
            edge.source.column_name,
            edge.target.resource_name,
            edge.target.column_name,
        )
        for edge in result.column_consumers(resource_name="a", column_name="id")
    )
    assert consumers == test_case.expected_consumers


@pytest.mark.parametrize(
    "test_case",
    [ExpectedCountTestCase(description="compact facts preserve graph", expected_count=1)],
    ids=lambda case: case.description,
)
def test_given_compact_analysis_facts_when_building_fast_lineage_then_preserves_public_graph(
    test_case: ExpectedCountTestCase,
) -> None:
    upstream: CompiledModelView = make_compiled_model(
        name="orders",
        query_sql="SELECT 1 AS order_id",
        inferred_columns=("order_id",),
    )
    target: CompiledModelView = replace(
        compiled_model_record(
            make_compiled_model(
                name="order_summary",
                query_sql='SELECT order_id FROM __ref("orders")',
                inferred_columns=("order_id",),
            )
        ),
        fast_lineage_columns=(
            CompiledLineageColumnFact(
                output_column="order_id",
                upstream_columns=(
                    CompiledLineageSourceFact(
                        resource_type=CompiledResourceType.MODEL,
                        resource_name="orders",
                        column_name="order_id",
                    ),
                ),
                transform_kind=ColumnTransformKind.DIRECT,
                confidence=ColumnLineageConfidence.HIGH,
            ),
        ),
    )

    result: ProjectColumnLineage | None = project_column_lineage(
        project=make_compiled_project(models=(upstream, target)),
        dialect="duckdb",
        mode=ColumnLineageMode.FAST,
    )

    assert result is not None
    assert result.has_model("order_summary")
    assert not result.model_has_star("order_summary")
    assert result.edge_count_targeting("order_summary") == test_case.expected_count
    assert result.models["order_summary"].columns[0].upstream_columns[0].resource_name == "orders"
    assert tuple(
        edge_label(
            edge.source.resource_name,
            edge.source.column_name,
            edge.target.resource_name,
            edge.target.column_name,
        )
        for edge in result.edges
    ) == ("orders.order_id->order_summary.order_id",)


@pytest.mark.parametrize(
    "test_case",
    (
        ColumnLineageAnalyzerTestCase(
            description="resolves aggregated column sourced through a CTE",
            model_name="int_order_payments",
            query_sql=(
                'WITH payments AS (SELECT * FROM __ref("stg_payments")) '
                "SELECT COALESCE(SUM(payments.amount_cents), 0) AS order_amount_cents "
                "FROM payments GROUP BY payments.order_id"
            ),
            inferred_columns=("order_amount_cents",),
            upstream_model_columns={"stg_payments": ("order_id", "amount_cents")},
            upstream_seed_columns={},
            expected_column="order_amount_cents",
            expected_upstream_columns=("model:stg_payments.amount_cents",),
            expected_transform_kind=ColumnTransformKind.AGGREGATION,
        ),
        ColumnLineageAnalyzerTestCase(
            description="resolves passthrough column sourced through a select-star CTE",
            model_name="int_orders",
            query_sql=(
                'WITH orders AS (SELECT * FROM __ref("stg_orders")) '
                "SELECT orders.customer_id FROM orders"
            ),
            inferred_columns=("customer_id",),
            upstream_model_columns={"stg_orders": ("order_id", "customer_id")},
            upstream_seed_columns={},
            expected_column="customer_id",
            expected_upstream_columns=("model:stg_orders.customer_id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        ),
        ColumnLineageAnalyzerTestCase(
            description="resolves aggregated column through CTE join with table aliases",
            model_name="daily_revenue",
            query_sql=(
                'WITH orders AS (SELECT * FROM __ref("stg_orders")), '
                'payments AS (SELECT * FROM __ref("stg_payments")) '
                "SELECT SUM(p.amount_cents) AS total_revenue_cents "
                "FROM orders o JOIN payments p ON p.order_id = o.order_id "
                "GROUP BY o.customer_id"
            ),
            inferred_columns=("total_revenue_cents",),
            upstream_model_columns={
                "stg_orders": ("order_id", "customer_id"),
                "stg_payments": ("order_id", "amount_cents"),
            },
            upstream_seed_columns={},
            expected_column="total_revenue_cents",
            expected_upstream_columns=("model:stg_payments.amount_cents",),
            expected_transform_kind=ColumnTransformKind.AGGREGATION,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_cte_sourced_column_when_building_rich_column_lineage_then_resolves_upstream(
    test_case: ColumnLineageAnalyzerTestCase,
) -> None:
    upstream_models: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=model_name,
            query_sql="SELECT 1",
            inferred_columns=columns,
        )
        for model_name, columns in test_case.upstream_model_columns.items()
    )
    target_model: CompiledModelView = make_compiled_model(
        name=test_case.model_name,
        query_sql=test_case.query_sql,
        inferred_columns=test_case.inferred_columns,
    )
    project: CompiledProjectView = make_compiled_project(
        models=upstream_models + (target_model,),
    )

    result: ProjectColumnLineage | None = project_column_lineage(
        project=project,
        mode=ColumnLineageMode.RICH,
    )

    assert result is not None
    model_lineage: ModelColumnLineage = result.models[test_case.model_name]
    columns_by_output: dict[str, tuple[ColumnLineage, ...]] = {
        column.output_column: (column,) for column in model_lineage.columns
    }
    assert len(columns_by_output) == len(model_lineage.columns)
    column_lineage: ColumnLineage = next(iter(columns_by_output.get(test_case.expected_column, ())))
    upstream_columns: tuple[str, ...] = tuple(
        sorted(
            f"{CompiledResourceType(source.resource_type).value}:{source.resource_name}.{source.column_name}"
            for source in column_lineage.upstream_columns
        )
    )
    assert upstream_columns == tuple(sorted(test_case.expected_upstream_columns))
    assert column_lineage.transform_kind == test_case.expected_transform_kind


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnLineageAnalyzerTestCase(
            description="normalizes postgres dialect for Polyglot rich lineage",
            model_name="orders_distinct",
            query_sql=(
                'SELECT DISTINCT ON (orders.order_id) orders.order_id FROM __ref("orders") orders'
            ),
            inferred_columns=("order_id",),
            upstream_model_columns={"orders": ("order_id",)},
            upstream_seed_columns={},
            expected_column="order_id",
            expected_upstream_columns=("model:orders.order_id",),
            expected_transform_kind=ColumnTransformKind.DIRECT,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_postgres_dialect_when_building_rich_column_lineage_then_uses_polyglot_name(
    test_case: ColumnLineageAnalyzerTestCase,
) -> None:
    upstream_models: tuple[CompiledModelView, ...] = tuple(
        make_compiled_model(
            name=model_name,
            query_sql="SELECT 1",
            inferred_columns=columns,
        )
        for model_name, columns in test_case.upstream_model_columns.items()
    )
    target_model: CompiledModelView = make_compiled_model(
        name=test_case.model_name,
        query_sql=test_case.query_sql,
        inferred_columns=test_case.inferred_columns,
    )
    project: CompiledProjectView = make_compiled_project(models=upstream_models + (target_model,))

    result: ProjectColumnLineage | None = project_column_lineage(
        project=project,
        dialect="postgres",
        mode=ColumnLineageMode.RICH,
    )

    assert result is not None
    model_lineage: ModelColumnLineage = result.models[test_case.model_name]
    columns_by_output: dict[str, tuple[ColumnLineage, ...]] = {
        column.output_column: (column,) for column in model_lineage.columns
    }
    assert len(columns_by_output) == len(model_lineage.columns)
    column_lineage: ColumnLineage = next(iter(columns_by_output.get(test_case.expected_column, ())))
    upstream_columns: tuple[str, ...] = tuple(
        sorted(
            f"{CompiledResourceType(source.resource_type).value}:{source.resource_name}.{source.column_name}"
            for source in column_lineage.upstream_columns
        )
    )
    assert upstream_columns == tuple(sorted(test_case.expected_upstream_columns))
    assert column_lineage.transform_kind == test_case.expected_transform_kind


@pytest.mark.parametrize(
    "test_case",
    [
        SqlAnalysisDisabledLineageTestCase(
            description="returns no lineage when SQL analysis is disabled",
            sql_analysis_enabled=False,
            expected_result_is_none=True,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_disabled_when_building_column_lineage_then_returns_no_lineage(
    test_case: SqlAnalysisDisabledLineageTestCase,
) -> None:
    project: CompiledProjectView = make_compiled_project(
        models=(
            make_compiled_model(
                name="orders_out",
                query_sql='SELECT order_id FROM __ref("orders")',
                inferred_columns=("order_id",),
            ),
        ),
        sql_analysis_enabled=test_case.sql_analysis_enabled,
    )

    result: ProjectColumnLineage | None = project_column_lineage(project=project)

    assert (result is None) is test_case.expected_result_is_none


@pytest.mark.parametrize(
    "test_case",
    [
        RichLineageAnalysisScalingTestCase(
            description="analyses only the traced models, whatever else the project holds",
            chain_length=4,
            unrelated_model_counts=(0, 250),
            expected_model_names=("orders_0", "orders_1", "orders_2", "orders_3"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unrelated_models_when_tracing_rich_lineage_then_analysis_work_stays_linear(
    test_case: RichLineageAnalysisScalingTestCase,
) -> None:
    chain_names: frozenset[str] = frozenset(
        f"orders_{index}" for index in range(test_case.chain_length)
    )
    edges_by_project_size: list[tuple[str, ...]] = []
    for unrelated_model_count in test_case.unrelated_model_counts:
        project: CompiledProjectView = make_compiled_project(
            models=make_chain_models(
                chain_length=test_case.chain_length,
                unrelated_model_count=unrelated_model_count,
            )
        )

        result: ProjectColumnLineage | None = project_column_lineage(
            project=project, model_names=chain_names
        )

        assert result is not None
        assert tuple(result.models) == test_case.expected_model_names, test_case.description
        edges_by_project_size.append(
            tuple(
                edge_label(
                    edge.source.resource_name,
                    edge.source.column_name,
                    edge.target.resource_name,
                    edge.target.column_name,
                )
                for edge in result.edges
            )
        )
    assert len(set(edges_by_project_size)) == 1
    assert "orders_2.amount->orders_3.amount" in edges_by_project_size[0]
