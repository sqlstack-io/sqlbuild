from __future__ import annotations

from dataclasses import replace
from pathlib import Path

import pytest

from sqlbuild._native import (
    ChainStep,
    CompiledDirectLogicSqlTestPayload,
    CompiledFunction,
    CompiledModelSqlTestPayload,
    CompiledObjectKey,
    CompiledProject,
    CompiledRelationLocation,
    CompiledResourceType,
    CompiledSqlTest,
    CompileSqlTestCte,
    DiscoveredSqlTestBlock,
    DiscoveredSqlTestFile,
    NativeSqlTestPlan,
    PlannerInputError,
    PlannerScope,
    PlannerSelection,
    PlanOutput,
    PlanWarning,
    SqlTestPlanEntry,
    SqlTestPlanResult,
    SqlTestReferenceError,
    WarningSeverity,
    build_test_command_plan,
    plan_compiled_sql_test_artifacts,
    project_with,
    render_sql_test_comparison_sql,
    resolve_static_command_scope,
    resolve_static_relation_context,
    sql_test_plan_error_messages,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.sqlserver.classes.sqlserver_adapter import SqlServerAdapter
from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledProjectView, SqlTestMode
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly._test_types import (
    AssertionChainCteErrorTestCase,
    NativePlanningDifferentialTestCase,
    PlanMacroTestCase,
    PlanTestChainTestCase,
    RepeatedFixturePlanTestCase,
    SqlAnalysisDialectTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.sql_test_assembly.helpers import (
    assert_native_artifact_matches_runtime_plan,
    build_test_and_project,
    plan_single_test,
    plan_single_test_allowing_errors,
)
from tests.unit.src.sqlbuild.executor.testing.main.helpers import build_comparison_test_adapter


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="commented model ref is not added to the test chain",
            model_queries={
                "unused_upstream": "SELECT 1 AS id",
                "orders": 'SELECT `value--label` AS id\n-- FROM __ref("unused_upstream")',
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={"orders": "`value--label`"},
            expected_cte_bodies={"orders": "SELECT 1 AS id"},
            assertion_ctes={
                "commented_ref_is_ignored": (
                    'SELECT 1 AS violation\n-- FROM __ref("unused_upstream")'
                )
            },
        ),
        PlanTestChainTestCase(
            description="single model with mock dbt ref replaces dbt ref in sql",
            model_queries={
                "orders": 'SELECT order_id, amount FROM __dbt_ref("stg_orders")',
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            mock_dbt_ref_ctes={
                "stg_orders": "SELECT 1 AS order_id, 100 AS amount",
            },
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "orders": "SELECT 1 AS order_id, 100 AS amount",
            },
            expected_cte_bodies={
                "orders": "SELECT 1 AS order_id, 100 AS amount",
            },
        ),
        PlanTestChainTestCase(
            description="single model with package-qualified mock dbt ref replaces dbt ref in sql",
            model_queries={
                "payments": 'SELECT payment_id FROM __dbt_ref("stripe", "payments")',
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            mock_dbt_ref_ctes={
                "stripe__payments": "SELECT 1 AS payment_id",
            },
            helper_ctes={},
            expected_model_names=("payments",),
            expected_chain_length=1,
            expected_sql_fragments={
                "payments": "SELECT 1 AS payment_id",
            },
            expected_cte_bodies={
                "payments": "SELECT 1 AS payment_id",
            },
        ),
        PlanTestChainTestCase(
            description="sql test macro mocks override model macro expansion before refs resolve",
            model_queries={
                "orders": "SELECT 1 AS id, 'real' AS country FROM __source(\"raw\")",
            },
            model_macro_source_queries={
                "orders": 'SELECT 1 AS id, @py.country() AS country FROM __source("raw")',
            },
            loaded_macro_outputs={"country": "'real'"},
            macro_mocks={"country": "'mocked'"},
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={"orders": "'mocked' AS country"},
            expected_cte_bodies={"orders": "SELECT 1 AS id, 'mocked' AS country"},
        ),
        PlanTestChainTestCase(
            description="unmocked macros keep real project macro expansion in sql tests",
            model_queries={
                "orders": "SELECT 1 AS id, 'real' AS country FROM __source(\"raw\")",
            },
            model_macro_source_queries={
                "orders": 'SELECT 1 AS id, @py.country() AS country FROM __source("raw")',
            },
            loaded_macro_outputs={"country": "'real'"},
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={"orders": "'real' AS country"},
            expected_cte_bodies={"orders": "SELECT 1 AS id, 'real' AS country"},
        ),
        PlanTestChainTestCase(
            description="single model with mock ref replaces ref in sql",
            model_queries={
                "orders": 'SELECT id, amount FROM __ref("raw_orders")',
            },
            mock_ref_ctes={
                "raw_orders": "SELECT 1 AS id, 100 AS amount",
            },
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "orders": "SELECT 1 AS id, 100 AS amount",
            },
            expected_cte_bodies={
                "orders": "SELECT 1 AS id, 100 AS amount",
            },
        ),
        PlanTestChainTestCase(
            description="single model with mock source replaces source in sql",
            model_queries={
                "stg_orders": 'SELECT id FROM __source("raw_orders")',
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw_orders": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("stg_orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "stg_orders": "SELECT 1 AS id",
            },
            expected_cte_bodies={
                "stg_orders": "SELECT 1 AS id",
            },
        ),
        PlanTestChainTestCase(
            description="single model with mock seed replaces seed in sql",
            model_queries={
                "orders": 'SELECT code FROM __seed("country_codes")',
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            mock_seed_ctes={
                "country_codes": "SELECT 'US' AS code",
            },
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "orders": "SELECT 'US' AS code",
            },
            expected_cte_bodies={
                "orders": "SELECT 'US' AS code",
            },
        ),
        PlanTestChainTestCase(
            description="single model resolves udf references in sql tests",
            model_queries={
                "orders": 'SELECT __udf("is_ready")(status) AS ready FROM __source("raw")',
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 'completed' AS status",
            },
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            function_locations={"is_ready": "main.is_ready"},
            expected_sql_fragments={
                "orders": "main.is_ready(status) AS ready",
            },
            expected_cte_bodies={
                "orders": "SELECT TRUE AS ready",
            },
        ),
        PlanTestChainTestCase(
            description="single model resolves table function references in sql tests",
            model_queries={
                "orders": ('SELECT order_id FROM __table_fn("customer_orders")(42)'),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            table_function_locations={"customer_orders": "main.customer_orders"},
            expected_cte_bodies={"orders": "SELECT 1 AS order_id"},
            expected_sql_fragments={
                "orders": "FROM main.customer_orders(42)",
            },
            expected_function_deps=("customer_orders",),
        ),
        PlanTestChainTestCase(
            description="model table function fixture replaces the complete invocation",
            model_queries={
                "orders": (
                    'SELECT order_id FROM __table_fn("customer_orders")('
                    "COALESCE((SELECT MAX(customer_id) FROM customers), 42))"
                ),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            mock_table_function_ctes={
                "customer_orders": "SELECT 7 AS order_id",
            },
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            table_function_locations={"customer_orders": "main.customer_orders"},
            expected_cte_bodies={"orders": "SELECT 7 AS order_id"},
            expected_sql_fragments={"orders": "SELECT 7 AS order_id"},
            unexpected_sql_fragments={
                "orders": ("main.customer_orders", "COALESCE", "customer_id"),
            },
            expected_mock_table_function_names=("customer_orders",),
            expected_function_deps=(),
        ),
        PlanTestChainTestCase(
            description="assertion-only table function fixture is reachable",
            model_queries={},
            mock_ref_ctes={},
            mock_source_ctes={},
            mock_table_function_ctes={
                "customer_orders": "SELECT 7 AS order_id",
            },
            helper_ctes={},
            expected_model_names=(),
            expected_chain_length=0,
            table_function_locations={"customer_orders": "main.customer_orders"},
            assertion_ctes={
                "positive_orders": (
                    'SELECT * FROM __table_fn("customer_orders")(7) WHERE order_id < 0'
                )
            },
            expected_assertion_fragments={
                "positive_orders": "SELECT 7 AS order_id",
            },
            expected_mock_table_function_names=("customer_orders",),
        ),
        PlanTestChainTestCase(
            description="two model chain resolves in dependency order",
            model_queries={
                "stg_orders": 'SELECT id FROM __source("raw")',
                "fact_orders": ('SELECT id, 1 AS flag FROM __ref("stg_orders")'),
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("stg_orders", "fact_orders"),
            expected_chain_length=2,
            expected_sql_fragments={
                "fact_orders": "SELECT 1 AS id",
            },
            expected_cte_bodies={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id, 1 AS flag",
            },
        ),
        PlanTestChainTestCase(
            description="model query overrides drive dependency order",
            model_queries={
                "stg_orders": 'SELECT id FROM __source("raw")',
                "fact_orders": "SELECT id, 1 AS flag FROM staging.stg_orders",
            },
            model_query_overrides={
                "fact_orders": 'SELECT id, 1 AS flag FROM __ref("stg_orders")',
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("fact_orders", "stg_orders"),
            expected_chain_length=2,
            expected_sql_fragments={
                "fact_orders": "SELECT 1 AS id",
            },
            expected_cte_bodies={
                "stg_orders": "SELECT 1 AS id",
                "fact_orders": "SELECT 1 AS id, 1 AS flag",
            },
        ),
        PlanTestChainTestCase(
            description="three model chain A to B to C",
            model_queries={
                "A": 'SELECT id FROM __source("raw")',
                "B": 'SELECT id, id * 2 AS doubled FROM __ref("A")',
                "C": ('SELECT id, doubled + 1 AS final FROM __ref("B")'),
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("A", "B", "C"),
            expected_chain_length=3,
            expected_sql_fragments={
                "C": "doubled + 1 AS final",
            },
            expected_cte_bodies={
                "A": "SELECT 1 AS id",
                "B": "SELECT 1 AS id",
                "C": "SELECT 1 AS id",
            },
        ),
        PlanTestChainTestCase(
            description="final model expectation resolves the complete unmocked model chain",
            model_queries={
                "A": 'SELECT id FROM __source("raw")',
                "B": 'SELECT id, id * 2 AS doubled FROM __ref("A")',
                "C": 'SELECT id, doubled + 1 AS final FROM __ref("B")',
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("C",),
            expected_chain_length=3,
            expected_sql_fragments={
                "C": "doubled + 1 AS final",
            },
            expected_cte_bodies={
                "C": "SELECT 1 AS id, 3 AS final",
            },
        ),
        PlanTestChainTestCase(
            description="helper ctes included in mock subquery",
            model_queries={
                "orders": 'SELECT id, amount FROM __ref("raw")',
            },
            mock_ref_ctes={
                "raw": "SELECT id, amount FROM gen_data",
            },
            mock_source_ctes={},
            helper_ctes={
                "gen_data": "SELECT 1 AS id, 100 AS amount",
            },
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "orders": "gen_data",
            },
            expected_cte_bodies={
                "orders": "SELECT 1 AS id, 100 AS amount",
            },
        ),
        PlanTestChainTestCase(
            description=("unreachable mock ref produces warning"),
            model_queries={
                "B": 'SELECT id FROM __ref("A")',
                "C": 'SELECT id FROM __ref("B")',
            },
            mock_ref_ctes={
                "A": "SELECT 1 AS id",
                "B": "SELECT 1 AS id",
            },
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("C",),
            expected_chain_length=1,
            expected_sql_fragments={
                "C": "SELECT 1 AS id",
            },
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.WARNING,
            expected_cte_bodies={
                "C": "SELECT 1 AS id",
            },
        ),
        PlanTestChainTestCase(
            description="diamond dependency A to B and C both to D",
            model_queries={
                "B": 'SELECT id FROM __source("raw")',
                "C": 'SELECT id FROM __source("raw")',
                "D": ('SELECT b.id FROM __ref("B") b JOIN __ref("C") c ON b.id = c.id'),
            },
            mock_ref_ctes={},
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("B", "C", "D"),
            expected_chain_length=3,
            expected_sql_fragments={
                "D": "SELECT 1 AS id",
            },
            expected_cte_bodies={
                "B": "SELECT 1 AS id",
                "C": "SELECT 1 AS id",
                "D": "SELECT 1 AS id",
            },
        ),
        PlanTestChainTestCase(
            description=("model with both mock ref and chain ref resolves both"),
            model_queries={
                "stg": 'SELECT id FROM __source("raw")',
                "final": ('SELECT a.id FROM __ref("stg") a JOIN __ref("lookup") b ON a.id = b.id'),
            },
            mock_ref_ctes={
                "lookup": "SELECT 1 AS id, 'US' AS country",
            },
            mock_source_ctes={
                "raw": "SELECT 1 AS id",
            },
            helper_ctes={},
            expected_model_names=("stg", "final"),
            expected_chain_length=2,
            expected_sql_fragments={
                "final": "SELECT 1 AS id",
            },
            expected_warning_count=0,
            expected_cte_bodies={
                "stg": "SELECT 1 AS id",
                "final": "SELECT 1 AS id",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_test_and_project_when_planning_then_produces_expected_chain(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    entry: SqlTestPlanEntry
    warnings: tuple[PlanWarning, ...]
    entry, warnings = plan_single_test(test=compiled_test, project=project, adapter=DuckDbAdapter())

    assert len(entry.chain) == test_case.expected_chain_length

    model_name: str
    expected_fragment: str
    chain_by_model_name: dict[str, ChainStep] = {step.model_name: step for step in entry.chain}
    assert len(chain_by_model_name) == len(entry.chain)
    for model_name, expected_fragment in test_case.expected_sql_fragments.items():
        assert expected_fragment in chain_by_model_name[model_name].resolved_sql
    for model_name, unexpected_fragments in test_case.unexpected_sql_fragments.items():
        for unexpected_fragment in unexpected_fragments:
            assert unexpected_fragment not in chain_by_model_name[model_name].resolved_sql
    assertions_by_name: dict[str, str] = {
        assertion.name: assertion.resolved_sql for assertion in entry.assertions
    }
    for assertion_name, expected_fragment in test_case.expected_assertion_fragments.items():
        assert expected_fragment in assertions_by_name[assertion_name]

    assert entry.mock_table_function_names == test_case.expected_mock_table_function_names
    assert tuple(dependency.name for dependency in entry.function_deps) == (
        test_case.expected_function_deps
    )

    assert len(warnings) == test_case.expected_warning_count
    expected_sev: WarningSeverity | None = test_case.expected_warning_severity
    actual_sevs: tuple[WarningSeverity, ...] = tuple(w.severity for w in warnings)
    assert (expected_sev is None) or all(s == expected_sev for s in actual_sevs)
    expected_error_fragment: str
    for expected_error_fragment in test_case.expected_error_fragments:
        assert any(expected_error_fragment in w.message for w in warnings)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="single model with unmocked dbt ref reports missing mock error",
            model_queries={
                "payments": 'SELECT payment_id FROM __dbt_ref("stripe", "payments")',
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("payments",),
            expected_chain_length=1,
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_error_fragments=("__dbt_ref__stripe__payments which has no mock",),
            expected_cte_bodies={"payments": "SELECT 1 AS payment_id"},
        ),
        PlanTestChainTestCase(
            description=("unresolved ref produces error warning"),
            model_queries={
                "orders": ('SELECT id FROM __ref("missing_model")'),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_cte_bodies={
                "orders": "SELECT 1",
            },
        ),
        PlanTestChainTestCase(
            description=("unresolved source produces error warning"),
            model_queries={
                "orders": ('SELECT id FROM __source("missing_source")'),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_cte_bodies={
                "orders": "SELECT 1",
            },
        ),
        PlanTestChainTestCase(
            description=("unresolved seed produces error warning"),
            model_queries={
                "orders": ('SELECT code FROM __seed("missing_seed")'),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_cte_bodies={
                "orders": "SELECT 1",
            },
        ),
        PlanTestChainTestCase(
            description=("multiple unresolved refs produce multiple errors"),
            model_queries={
                "orders": ('SELECT a.id FROM __ref("x") a JOIN __source("y") b ON a.id = b.id'),
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_warning_count=2,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_cte_bodies={
                "orders": "SELECT 1",
            },
        ),
        PlanTestChainTestCase(
            description="missing expected model produces error warning",
            model_queries={},
            mock_ref_ctes={
                "raw": "SELECT 1 AS id",
            },
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("nonexistent",),
            expected_chain_length=0,
            expected_warning_count=2,
            expected_warning_severity=None,
            expected_cte_bodies={
                "nonexistent": "SELECT 1",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unresolved_chain_input_when_planning_then_test_is_blocked_with_error_diagnostics(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    result: SqlTestPlanResult
    plan: NativeSqlTestPlan
    result, plan = plan_single_test_allowing_errors(
        test=compiled_test, project=project, adapter=DuckDbAdapter()
    )

    assert result.entry is None
    assert result.fixture_diagnostics == sql_test_plan_error_messages(warnings=plan.warnings)
    assert result.fixture_diagnostics
    assert len(plan.chain) == test_case.expected_chain_length
    chain_by_model_name: dict[str, ChainStep] = {step.model_name: step for step in plan.chain}
    model_name: str
    expected_fragment: str
    for model_name, expected_fragment in test_case.expected_sql_fragments.items():
        assert expected_fragment in chain_by_model_name[model_name].resolved_sql
    assert len(plan.warnings) == test_case.expected_warning_count
    expected_error_fragment: str
    for expected_error_fragment in test_case.expected_error_fragments:
        assert any(expected_error_fragment in message for message in result.fixture_diagnostics)


@pytest.mark.parametrize(
    "test_case",
    (
        NativePlanningDifferentialTestCase(
            description="analyzed model chain",
            planning_case=PlanTestChainTestCase(
                description="analyzed model chain",
                model_queries={
                    "stg_orders": 'SELECT * FROM __source("raw_orders")',
                    "orders": 'SELECT order_id FROM __ref("stg_orders")',
                },
                mock_ref_ctes={},
                mock_source_ctes={"raw_orders": "SELECT 1 AS order_id"},
                helper_ctes={},
                expected_model_names=("orders",),
                expected_chain_length=2,
                expected_cte_bodies={"orders": "SELECT 1 AS order_id"},
            ),
        ),
        NativePlanningDifferentialTestCase(
            description="textual udf fallback",
            planning_case=PlanTestChainTestCase(
                description="textual udf fallback",
                model_queries={
                    "orders": (
                        'SELECT __udf("is_ready")(status) AS ready FROM __source("raw_orders")'
                    )
                },
                mock_ref_ctes={},
                mock_source_ctes={"raw_orders": "SELECT 'ready' AS status"},
                helper_ctes={},
                expected_model_names=("orders",),
                expected_chain_length=1,
                function_locations={"is_ready": "main.is_ready"},
                expected_cte_bodies={"orders": "SELECT TRUE AS ready"},
            ),
        ),
        NativePlanningDifferentialTestCase(
            description="table function fixture",
            planning_case=PlanTestChainTestCase(
                description="table function fixture",
                model_queries={"orders": 'SELECT order_id FROM __table_fn("customer_orders")(42)'},
                mock_ref_ctes={},
                mock_source_ctes={},
                mock_table_function_ctes={"customer_orders": "SELECT 7 AS order_id"},
                helper_ctes={"fixture_helper": "SELECT 7 AS order_id"},
                expected_model_names=("orders",),
                expected_chain_length=1,
                table_function_locations={"customer_orders": "main.customer_orders"},
                expected_cte_bodies={"orders": "SELECT 7 AS order_id"},
            ),
        ),
        NativePlanningDifferentialTestCase(
            description="analysis disabled",
            sql_analysis_enabled=False,
            planning_case=PlanTestChainTestCase(
                description="analysis disabled",
                model_queries={"orders": 'SELECT * FROM __source("raw_orders")'},
                mock_ref_ctes={},
                mock_source_ctes={"raw_orders": "SELECT 1 AS order_id"},
                helper_ctes={"helper": "SELECT 1 AS one"},
                expected_model_names=("orders",),
                expected_chain_length=1,
                expected_cte_bodies={"orders": "SELECT 1 AS order_id"},
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_model_chain_when_native_planning_then_artifact_matches_runtime_sql_and_warnings(
    test_case: NativePlanningDifferentialTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case.planning_case)
    assert isinstance(compiled_test.payload, CompiledModelSqlTestPayload)
    compiled_test = replace(
        compiled_test,
        payload=replace(
            compiled_test.payload,
            expected_ctes=tuple(
                CompileSqlTestCte(
                    name=f"__expected__{name}",
                    sql_body=sql_body,
                )
                for name, sql_body in test_case.planning_case.expected_cte_bodies.items()
            ),
        ),
    )
    project = project_with(project=project, sql_tests=(compiled_test,))
    adapter: DuckDbAdapter = DuckDbAdapter()
    entry, warnings = plan_single_test(
        test=compiled_test,
        project=project,
        adapter=adapter,
        sql_analysis_enabled=test_case.sql_analysis_enabled,
    )
    assert (
        assert_native_artifact_matches_runtime_plan(
            project=project,
            sql_test=compiled_test,
            entry=entry,
            warnings=warnings,
            sql_analysis_enabled=test_case.sql_analysis_enabled,
        )
        is test_case.expected_matches
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="comment and literal refs are ignored without rewriting SQL",
            model_queries={
                "orders": (
                    "SELECT `value--label` AS marker, "
                    "'-- __ref(\"literal_model\")' AS literal_value\n"
                    '/* optimizer hint __ref("block_model") */\n'
                    '-- __ref("line_model")'
                ),
                "literal_model": "SELECT 1",
                "block_model": "SELECT 1",
                "line_model": "SELECT 1",
                "literal_assertion_model": "SELECT 1",
                "block_assertion_model": "SELECT 1",
                "line_assertion_model": "SELECT 1",
            },
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_cte_bodies={"orders": "SELECT 1"},
            assertion_ctes={
                "comments_are_preserved": (
                    "SELECT '-- __ref(\"literal_assertion_model\")' AS value\n"
                    '/* assertion hint __ref("block_assertion_model") */\n'
                    '-- __ref("line_assertion_model")'
                )
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_refs_in_comments_and_literals_when_planning_then_preserves_sql_and_ignores_refs(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    entry, warnings = plan_single_test(test=compiled_test, project=project, adapter=DuckDbAdapter())

    assert not warnings
    assert tuple(step.model_name for step in entry.chain) == test_case.expected_model_names
    assert entry.chain[0].resolved_sql == test_case.model_queries["orders"]
    assert len(entry.assertions) == 1
    assert entry.assertions[0].resolved_sql == test_case.assertion_ctes["comments_are_preserved"]


@pytest.mark.parametrize(
    "test_case",
    [
        RepeatedFixturePlanTestCase(
            description="two fixture rows stay isolated",
            fixture_ids=(1, 2),
            expected_sql_fragments=("SELECT 1 AS id", "SELECT 2 AS id"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_query_with_different_fixture_rows_when_planning_then_keeps_cases_isolated(
    test_case: RepeatedFixturePlanTestCase,
) -> None:
    entries: list[SqlTestPlanEntry] = []
    fixture_id: int
    for fixture_id in test_case.fixture_ids:
        fixture_case: PlanTestChainTestCase = PlanTestChainTestCase(
            description=f"fixture case {fixture_id}",
            model_queries={"orders": 'SELECT id FROM __source("raw")'},
            mock_ref_ctes={},
            mock_source_ctes={"raw": f"SELECT {fixture_id} AS id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_cte_bodies={"orders": f"SELECT {fixture_id} AS id"},
        )
        compiled_test: CompiledSqlTest
        project: CompiledProjectView
        compiled_test, project = build_test_and_project(fixture_case)
        entry, warnings = plan_single_test(
            test=compiled_test,
            project=project,
            adapter=DuckDbAdapter(),
            sql_analysis_enabled=True,
        )
        assert not warnings
        entries.append(entry)

    for entry, expected_fragment in zip(entries, test_case.expected_sql_fragments, strict=True):
        assert expected_fragment in entry.chain[0].resolved_sql
    assert entries[0].chain[0].resolved_sql != entries[1].chain[0].resolved_sql


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="unresolved marker inside generated mock CTE",
            model_queries={"orders": 'SELECT id FROM __ref("raw_orders")'},
            mock_ref_ctes={"raw_orders": 'SELECT id FROM __source("missing_source")'},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_warning_count=1,
            expected_warning_severity=WarningSeverity.ERROR,
            expected_error_fragments=("missing_source",),
            expected_cte_bodies={"orders": "SELECT 1 AS id"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unresolved_marker_in_mock_when_planning_with_analysis_then_reports_error(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    result: SqlTestPlanResult
    plan: NativeSqlTestPlan
    result, plan = plan_single_test_allowing_errors(
        test=compiled_test,
        project=project,
        adapter=DuckDbAdapter(),
        sql_analysis_enabled=True,
    )
    warnings: tuple[PlanWarning, ...] = plan.warnings

    assert result.entry is None
    assert result.fixture_diagnostics == (warnings[0].message,)
    assert len(warnings) == test_case.expected_warning_count
    assert warnings[0].severity is test_case.expected_warning_severity
    assert test_case.expected_error_fragments[0] in warnings[0].message


@pytest.mark.parametrize(
    "test_case",
    [
        PlanMacroTestCase(
            description="plans macro test as one direct comparison chain step with helpers",
            helper_ctes={"input_values": "SELECT '  PAID  ' AS raw_status"},
            actual_sql="SELECT LOWER(TRIM(raw_status)) AS status FROM input_values",
            expected_sql="SELECT 'paid' AS status",
            expected_actual_fragment="SELECT LOWER(TRIM(raw_status)) AS status FROM input_values",
            expected_expected_fragment="SELECT 'paid' AS status",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_macro_sql_test_when_planning_then_compares_actual_to_expected_directly(
    test_case: PlanMacroTestCase,
) -> None:
    test_file: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
        file_path=Path("tests/unit/test_macro.sql"),
        relative_path=Path("tests/unit/test_macro.sql"),
        contents="",
        blocks=(),
    )
    test_block: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
        test_index=1,
        header_values={"mode": "macro", "name": "normalizes_status"},
        sql_body="",
        name="normalizes_status",
        mode=SqlTestMode.MACRO,
    )
    helper_ctes: tuple[CompileSqlTestCte, ...] = tuple(
        CompileSqlTestCte(name=name, sql_body=sql) for name, sql in test_case.helper_ctes.items()
    )
    sql_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="normalizes_status",
        ),
        scope_deps=(CompiledObjectKey(resource_type=CompiledResourceType.MODEL, name="orders"),),
        name="normalizes_status",
        test_file=test_file,
        test_block=test_block,
        sql_body="",
        mode=SqlTestMode.MACRO,
        payload=CompiledDirectLogicSqlTestPayload(
            mode=SqlTestMode.MACRO,
            helper_ctes=helper_ctes,
            actual_cte=CompileSqlTestCte(
                name="__macro_actual__",
                sql_body=test_case.actual_sql,
            ),
            expected_cte=CompileSqlTestCte(
                name="__macro_expected__",
                sql_body=test_case.expected_sql,
            ),
            tested_resource_names=("normalize_status",),
        ),
    )

    entry, warnings = plan_single_test(
        test=sql_test,
        adapter=DuckDbAdapter(),
        project=CompiledProject(
            run_id="test_run",
            effective_target_name=None,
            effective_connection={},
            effective_vars={},
        ),
    )

    assert warnings == ()
    assert len(entry.chain) == 1
    assert entry.chain[0].model_name == "macro normalizes_status"
    assert test_case.expected_actual_fragment in entry.chain[0].resolved_sql
    assert entry.chain[0].expected_cte_sql is not None
    assert test_case.expected_expected_fragment in entry.chain[0].expected_cte_sql


@pytest.mark.parametrize(
    "test_case",
    [
        PlanMacroTestCase(
            description="plans udf test as one direct comparison chain step with resolved udf call",
            helper_ctes={"input_values": "SELECT 1250 AS amount_cents"},
            actual_sql='SELECT __udf("format_cents")(amount_cents) AS formatted FROM input_values',
            expected_sql="SELECT '$12.50' AS formatted",
            expected_actual_fragment="main.format_cents(amount_cents) AS formatted",
            expected_expected_fragment="SELECT '$12.50' AS formatted",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_udf_sql_test_when_planning_then_compares_resolved_actual_to_expected_directly(
    test_case: PlanMacroTestCase,
) -> None:
    test_file: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
        file_path=Path("tests/unit/test_udf.sql"),
        relative_path=Path("tests/unit/test_udf.sql"),
        contents="",
        blocks=(),
    )
    test_block: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
        test_index=1,
        header_values={"mode": "udf", "name": "formats_cents"},
        sql_body="",
        name="formats_cents",
        mode=SqlTestMode.UDF,
    )
    helper_ctes: tuple[CompileSqlTestCte, ...] = tuple(
        CompileSqlTestCte(name=name, sql_body=sql) for name, sql in test_case.helper_ctes.items()
    )
    sql_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="formats_cents",
        ),
        scope_deps=(
            CompiledObjectKey(resource_type=CompiledResourceType.UDF, name="format_cents"),
        ),
        name="formats_cents",
        test_file=test_file,
        test_block=test_block,
        sql_body="",
        mode=SqlTestMode.UDF,
        payload=CompiledDirectLogicSqlTestPayload(
            mode=SqlTestMode.UDF,
            helper_ctes=helper_ctes,
            actual_cte=CompileSqlTestCte(
                name="__udf_actual__",
                sql_body=test_case.actual_sql,
            ),
            expected_cte=CompileSqlTestCte(
                name="__udf_expected__",
                sql_body=test_case.expected_sql,
            ),
            tested_resource_names=("format_cents",),
        ),
    )

    project: CompiledProjectView = CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        functions=(
            CompiledFunction(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.UDF,
                    name="format_cents",
                ),
                deps=(),
                name="format_cents",
                relative_path=Path("functions/sql/format_cents.sql"),
                arguments=(),
                returns="VARCHAR",
                body_sql="",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="format_cents",
                    qualified_name="main.format_cents",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="format_cents__fingerprint",
                    qualified_name="main.format_cents__fingerprint",
                ),
            ),
        ),
        sql_tests=(sql_test,),
    )
    entry, warnings = plan_single_test(
        test=sql_test,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert warnings == ()
    assert len(entry.chain) == 1
    assert entry.chain[0].model_name == "udf formats_cents"
    assert test_case.expected_actual_fragment in entry.chain[0].resolved_sql
    assert entry.chain[0].expected_cte_sql is not None
    assert test_case.expected_expected_fragment in entry.chain[0].expected_cte_sql
    assert entry.function_deps == (
        CompiledObjectKey(resource_type=CompiledResourceType.UDF, name="format_cents"),
    )
    assert_native_artifact_matches_runtime_plan(
        project=project,
        sql_test=sql_test,
        entry=entry,
        warnings=warnings,
        sql_analysis_enabled=False,
    )
    scope: PlannerScope = resolve_static_command_scope(
        project=project, selection=PlannerSelection()
    )
    command_plan: PlanOutput = build_test_command_plan(
        project=project,
        adapter=DuckDbAdapter(),
        scope=scope,
        relations=resolve_static_relation_context(
            project=project,
            adapter=DuckDbAdapter(),
            scope=scope,
        ),
    )
    assert tuple(function.name for function in command_plan.function_entries) == ("format_cents",)


@pytest.mark.parametrize(
    "test_case",
    [
        PlanMacroTestCase(
            description=(
                "plans table function test as one direct comparison chain step with resolved call"
            ),
            helper_ctes={},
            actual_sql='SELECT customer_id, order_id FROM __table_fn("customer_orders")(42)',
            expected_sql="SELECT 42 AS customer_id, 1 AS order_id",
            expected_actual_fragment="FROM main.customer_orders(42)",
            expected_expected_fragment="SELECT 42 AS customer_id, 1 AS order_id",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_table_function_sql_test_when_planning_then_compares_resolved_actual_to_expected(
    test_case: PlanMacroTestCase,
) -> None:
    test_file: DiscoveredSqlTestFile = DiscoveredSqlTestFile(
        file_path=Path("tests/unit/test_table_fn.sql"),
        relative_path=Path("tests/unit/test_table_fn.sql"),
        contents="",
        blocks=(),
    )
    test_block: DiscoveredSqlTestBlock = DiscoveredSqlTestBlock(
        test_index=1,
        header_values={"mode": "table_fn", "name": "returns_customer_orders"},
        sql_body="",
        name="returns_customer_orders",
        mode=SqlTestMode.TABLE_FN,
    )
    sql_test: CompiledSqlTest = CompiledSqlTest(
        key=CompiledObjectKey(
            resource_type=CompiledResourceType.SQL_TEST,
            name="returns_customer_orders",
        ),
        scope_deps=(
            CompiledObjectKey(resource_type=CompiledResourceType.TABLE_FN, name="customer_orders"),
        ),
        name="returns_customer_orders",
        test_file=test_file,
        test_block=test_block,
        sql_body="",
        mode=SqlTestMode.TABLE_FN,
        payload=CompiledDirectLogicSqlTestPayload(
            mode=SqlTestMode.TABLE_FN,
            helper_ctes=(),
            actual_cte=CompileSqlTestCte(
                name="__table_fn_actual__",
                sql_body=test_case.actual_sql,
            ),
            expected_cte=CompileSqlTestCte(
                name="__table_fn_expected__",
                sql_body=test_case.expected_sql,
            ),
            tested_resource_names=("customer_orders",),
        ),
    )

    project: CompiledProjectView = CompiledProject(
        run_id="test_run",
        effective_target_name=None,
        effective_connection={},
        effective_vars={},
        functions=(
            CompiledFunction(
                key=CompiledObjectKey(
                    resource_type=CompiledResourceType.TABLE_FN,
                    name="customer_orders",
                ),
                deps=(),
                name="customer_orders",
                relative_path=Path("functions/sql/customer_orders.sql"),
                arguments=(),
                returns="TABLE",
                body_sql="",
                destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="customer_orders",
                    qualified_name="main.customer_orders",
                ),
                fingerprint_destination=CompiledRelationLocation(
                    database=None,
                    schema="main",
                    name="customer_orders__fingerprint",
                    qualified_name="main.customer_orders__fingerprint",
                ),
            ),
        ),
        sql_tests=(sql_test,),
    )
    entry, warnings = plan_single_test(
        test=sql_test,
        adapter=DuckDbAdapter(),
        project=project,
    )

    assert warnings == ()
    assert len(entry.chain) == 1
    assert entry.chain[0].model_name == "table_fn returns_customer_orders"
    assert test_case.expected_actual_fragment in entry.chain[0].resolved_sql
    assert entry.chain[0].expected_cte_sql is not None
    assert test_case.expected_expected_fragment in entry.chain[0].expected_cte_sql
    assert entry.function_deps == (
        CompiledObjectKey(
            resource_type=CompiledResourceType.TABLE_FN,
            name="customer_orders",
        ),
    )
    assert_native_artifact_matches_runtime_plan(
        project=project,
        sql_test=sql_test,
        entry=entry,
        warnings=warnings,
        sql_analysis_enabled=False,
    )
    scope: PlannerScope = resolve_static_command_scope(
        project=project, selection=PlannerSelection()
    )
    command_plan: PlanOutput = build_test_command_plan(
        project=project,
        adapter=DuckDbAdapter(),
        scope=scope,
        relations=resolve_static_relation_context(
            project=project,
            adapter=DuckDbAdapter(),
            scope=scope,
        ),
    )
    assert tuple(function.name for function in command_plan.function_entries) == (
        "customer_orders",
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="sql_analysis path lifts refs and sources into readable top-level ctes",
            model_queries={
                "stg_orders": 'SELECT id, amount FROM __source("raw")',
                "fact_orders": (
                    "WITH local_helper AS (SELECT 1 AS one) "
                    "SELECT id, amount + one AS adjusted "
                    'FROM __ref("stg_orders") CROSS JOIN local_helper'
                ),
            },
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id, 100 AS amount"},
            helper_ctes={},
            expected_model_names=("stg_orders", "fact_orders"),
            expected_chain_length=2,
            expected_cte_bodies={
                "stg_orders": "SELECT 1 AS id, 100 AS amount",
                "fact_orders": "SELECT 1 AS id, 101 AS adjusted",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_enabled_when_planning_test_then_it_uses_top_level_generated_ctes(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    entry: SqlTestPlanEntry
    warnings: tuple[PlanWarning, ...]
    entry, warnings = plan_single_test(
        test=compiled_test,
        project=project,
        adapter=DuckDbAdapter(),
        sql_analysis_enabled=True,
    )

    assert not warnings
    assert len(entry.chain) == test_case.expected_chain_length
    step_map: dict[str, ChainStep] = {step.model_name: step for step in entry.chain}
    assert (
        "WITH __source__raw AS (SELECT 1 AS id, 100 AS amount)"
        in step_map["stg_orders"].resolved_sql
    )
    assert "FROM __source__raw" in step_map["stg_orders"].resolved_sql
    assert (
        "WITH __source__raw AS (SELECT 1 AS id, 100 AS amount), "
        "__ref__stg_orders AS (SELECT id, amount FROM __source__raw)"
        in step_map["fact_orders"].resolved_sql
    )
    assert "__ref__stg_orders AS (WITH" not in step_map["fact_orders"].resolved_sql
    assert "local_helper AS (SELECT 1 AS one)" in step_map["fact_orders"].resolved_sql
    assert "FROM __ref__stg_orders CROSS JOIN local_helper" in step_map["fact_orders"].resolved_sql


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="sql_analysis path errors on generated cte name collision",
            model_queries={
                "orders": 'WITH __source__raw AS (SELECT 9 AS id) SELECT id FROM __source("raw")',
            },
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_error_fragments=("names starting with a SQLBuild test prefix",),
            expected_cte_bodies={"orders": "SELECT 1 AS id"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_enabled_when_generated_cte_name_conflicts_then_it_raises_clear_error(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    with pytest.raises(ValueError, match=test_case.expected_error_fragments[0]):
        plan_single_test(
            test=compiled_test,
            project=project,
            adapter=DuckDbAdapter(),
            sql_analysis_enabled=True,
        )
    with pytest.raises(CompileInputError, match=test_case.expected_error_fragments[0]):
        plan_compiled_sql_test_artifacts(
            project=project,
            tests=(compiled_test,),
            adapter=DuckDbAdapter(),
            sql_analysis_enabled=True,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="sql analysis resolves assertions against shared chain ctes",
            model_queries={"stg_orders": 'SELECT id AS order_id FROM __source("raw")'},
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id"},
            helper_ctes={},
            expected_model_names=("stg_orders",),
            expected_chain_length=1,
            expected_cte_bodies={"stg_orders": "SELECT 1 AS order_id"},
            assertion_ctes={
                "order_ids_are_not_null": (
                    'SELECT * FROM __ref("stg_orders") AS stg_orders WHERE order_id IS NULL'
                )
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_assertion_when_planning_then_uses_shared_chain_ctes(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    entry: SqlTestPlanEntry
    warnings: tuple[PlanWarning, ...]
    entry, warnings = plan_single_test(
        test=compiled_test,
        project=project,
        adapter=DuckDbAdapter(),
        sql_analysis_enabled=True,
    )

    assert not warnings
    assert len(entry.chain) == test_case.expected_chain_length
    assert len(entry.assertions) == 1
    assertion_sql: str = entry.assertions[0].resolved_sql
    assert "__ref__stg_orders AS (SELECT id AS order_id FROM __source__raw)" in assertion_sql
    assert "FROM __ref__stg_orders AS stg_orders" in assertion_sql
    assert "FROM (WITH" not in assertion_sql


@pytest.mark.parametrize(
    "test_case",
    [
        SqlAnalysisDialectTestCase(
            description="Snowflake STARTSWITH spelling is preserved",
            query_sql=(
                "WITH picked AS (SELECT STARTSWITH(name, 'A') AS matches "
                'FROM __ref("items")) SELECT matches FROM picked'
            ),
            dialect="snowflake",
            expected_sql_fragment="STARTSWITH(name, 'A')",
            expected_absent_sql_fragment="STARTS_WITH",
        ),
        SqlAnalysisDialectTestCase(
            description="Snowflake SUBSTR spelling is preserved",
            query_sql=(
                "WITH picked AS (SELECT SUBSTR(name, 1, 2) AS matches "
                'FROM __ref("items")) SELECT matches FROM picked'
            ),
            dialect="snowflake",
            expected_sql_fragment="picked AS (SELECT SUBSTR(name, 1, 2) AS matches FROM",
            expected_absent_sql_fragment="SUBSTRING",
        ),
        SqlAnalysisDialectTestCase(
            description="Snowflake TIMESTAMPDIFF spelling is preserved",
            query_sql=(
                "WITH picked AS (SELECT TIMESTAMPDIFF(day, created_at, updated_at) AS matches "
                'FROM __ref("items")) SELECT matches FROM picked'
            ),
            dialect="snowflake",
            expected_sql_fragment="TIMESTAMPDIFF(day, created_at, updated_at) AS matches",
            expected_absent_sql_fragment="DATEDIFF",
        ),
        SqlAnalysisDialectTestCase(
            description="Snowflake TRY_TO_DECIMAL spelling is preserved",
            query_sql=(
                "WITH picked AS (SELECT TRY_TO_DECIMAL(name, 10, 2) AS matches "
                'FROM __ref("items")) SELECT matches FROM picked'
            ),
            dialect="snowflake",
            expected_sql_fragment="TRY_TO_DECIMAL(name, 10, 2) AS matches",
            expected_absent_sql_fragment="TRY_CAST",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_function_when_resolving_with_analysis_then_keeps_authored_spelling(
    test_case: SqlAnalysisDialectTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(
        PlanTestChainTestCase(
            description=test_case.description,
            model_queries={"matches": test_case.query_sql},
            mock_ref_ctes={"items": "SELECT 'Alice' AS name"},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=("matches",),
            expected_chain_length=1,
            expected_cte_bodies={"matches": "SELECT TRUE AS matches"},
        )
    )
    assert isinstance(compiled_test.payload, CompiledModelSqlTestPayload)
    compiled_test = replace(
        compiled_test,
        payload=replace(
            compiled_test.payload,
            expected_ctes=(
                CompileSqlTestCte(name="__expected__matches", sql_body="SELECT TRUE AS matches"),
            ),
        ),
    )
    adapter: BaseAdapter = build_comparison_test_adapter(test_case.dialect)

    entry, _ = plan_single_test(
        test=compiled_test, project=project, adapter=adapter, sql_analysis_enabled=True
    )
    comparison_sql: str = render_sql_test_comparison_sql(
        (entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )[0]

    assert test_case.expected_sql_fragment in entry.chain[0].resolved_sql
    assert test_case.expected_sql_fragment in comparison_sql
    assert test_case.expected_absent_sql_fragment not in comparison_sql


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="model and helper CTEs share a name on T-SQL",
            model_queries={
                "orders": (
                    'WITH helper_rows AS (SELECT order_id FROM __ref("raw_orders")) '
                    "SELECT order_id FROM helper_rows"
                )
            },
            mock_ref_ctes={"raw_orders": "SELECT 1 AS order_id"},
            mock_source_ctes={},
            helper_ctes={"helper_rows": "SELECT 2 AS order_id"},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_sql_fragments={
                "orders": "__helper__helper_rows AS (SELECT 2 AS order_id)",
                "__actual__orders": "helper_rows AS (SELECT order_id FROM __ref__raw_orders)",
                "__expected__orders": (
                    "__expected__orders AS (SELECT order_id FROM __helper__helper_rows AS "
                    "helper_rows)"
                ),
            },
            expected_cte_bodies={"orders": "SELECT order_id FROM helper_rows"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_tsql_shared_cte_name_when_planning_then_helper_and_model_cte_stay_distinct(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)
    adapter: BaseAdapter = build_comparison_test_adapter("sqlserver")

    entry, _ = plan_single_test(
        test=compiled_test, project=project, adapter=adapter, sql_analysis_enabled=True
    )
    comparison_sql: str = render_sql_test_comparison_sql(
        (entry,),
        set_difference_operator=adapter.render_set_difference_operator(),
        sql_analysis_dialect=adapter.sql_analysis_dialect(),
    )[0]

    for expected_fragment in test_case.expected_sql_fragments.values():
        assert expected_fragment in comparison_sql, comparison_sql


@pytest.mark.parametrize(
    "test_case",
    [
        AssertionChainCteErrorTestCase(
            description="rejects referenced model fallback beginning with with",
            assertion_sql='SELECT * FROM __ref("stg_orders")',
            model_queries={
                "stg_orders": "WITH model_rows AS (SELECT 1 AS id) SELECT * FROM model_rows"
            },
            expected_error_fragment="referenced model beginning with WITH",
        ),
        AssertionChainCteErrorTestCase(
            description="rejects assertion fallback beginning with with",
            assertion_sql=(
                'WITH invalid_orders AS (SELECT * FROM __ref("stg_orders")) '
                "SELECT * FROM invalid_orders"
            ),
            model_queries={"stg_orders": "SELECT 1 AS id"},
            expected_error_fragment="assertion beginning with WITH",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unflattened_with_when_building_assertion_ctes_then_raises_clear_error(
    test_case: AssertionChainCteErrorTestCase,
) -> None:
    compiled_test, project = build_test_and_project(
        PlanTestChainTestCase(
            description=test_case.description,
            model_queries=test_case.model_queries,
            mock_ref_ctes={},
            mock_source_ctes={},
            helper_ctes={},
            expected_model_names=(),
            expected_chain_length=1,
            expected_cte_bodies={},
            assertion_ctes={"flattened": test_case.assertion_sql},
        )
    )

    with pytest.raises(PlannerInputError, match=test_case.expected_error_fragment):
        plan_single_test(test=compiled_test, project=project, adapter=SqlServerAdapter())


@pytest.mark.parametrize(
    "test_case",
    [
        PlanTestChainTestCase(
            description="assertion calling an unmocked source fails planning with located P013",
            model_queries={"orders": 'SELECT id FROM __source("raw")'},
            mock_ref_ctes={},
            mock_source_ctes={"raw": "SELECT 1 AS id"},
            helper_ctes={},
            expected_model_names=("orders",),
            expected_chain_length=1,
            expected_error_fragments=(
                "tests/unit/test_chain.sql:1:1: SQL test CTE '__assert__no_returns' calls "
                '__source("returns"), which the test query cannot resolve',
                "__source__returns AS (SELECT ...)",
            ),
            expected_cte_bodies={"orders": "SELECT 1 AS id"},
            assertion_ctes={"no_returns": 'SELECT id FROM __source("returns")'},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_reference_the_compiler_missed_when_planning_then_it_fails_with_located_p013(
    test_case: PlanTestChainTestCase,
) -> None:
    compiled_test: CompiledSqlTest
    project: CompiledProjectView
    compiled_test, project = build_test_and_project(test_case)

    with pytest.raises(SqlTestReferenceError) as raised:
        plan_compiled_sql_test_artifacts(
            project=project,
            tests=(compiled_test,),
            adapter=DuckDbAdapter(),
            sql_analysis_enabled=True,
        )

    assert raised.value.code == "P013"
    assert raised.value.location.path == Path("tests/unit/test_chain.sql")
    assert raised.value.message == test_case.expected_error_fragments[0]
    assert raised.value.help is not None
    assert test_case.expected_error_fragments[1] in raised.value.help
