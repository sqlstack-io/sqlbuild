"""Compiled-project captures are classified into the analysis kinds the seed corpus must exercise."""

from __future__ import annotations

import json

import pytest

from scripts.compiler_differential._helpers.coverage.analysis import (
    analysed_input_kinds,
    analysis_capture_problems,
    required_analysis_kinds,
)
from scripts.compiler_differential.constants import ANALYSIS_NON_COLLECTION_FIELDS
from tests.unit.scripts.compiler_differential._helpers.coverage._test_types import (
    AnalysisCaptureKindsTestCase,
    AnalysisCaptureProblemsTestCase,
    RequiredKindsTestCase,
)
from tests.unit.scripts.compiler_differential._helpers.coverage.helpers import (
    compiled_model,
    compiled_source,
    empty_compiled_capture,
    hooked_compiled_capture,
    inferred_column,
    reference,
)

_TYPED_ORDERS: list[object] = [
    inferred_column(name="id", type_sql="INT"),
    inferred_column(name="placed_at", type_sql="TIMESTAMP"),
]
_CONTRACT_COLUMNS: list[dict[str, object]] = [
    {"name": "id", "type": "INTEGER", "nullable": False},
    {"name": "label", "type": "VARCHAR(24)", "nullable": None},
]
_CONTRACT: dict[str, object] = {"contract": "enforced", "materialized": "table"}
_CONFIG_KINDS: tuple[str, ...] = (
    "explicit_promotion_mode",
    "inline_sql_hook",
    "named_sql_hook",
    "hook_list",
)


@pytest.mark.parametrize(
    "test_case",
    [
        AnalysisCaptureKindsTestCase(
            description="analysed_star_over_a_source_expression_and_a_model",
            capture=empty_compiled_capture(
                sources=[compiled_source(name="raw_orders", expression="(SELECT 1 AS id)")],
                binding_catalog={"expression_shapes": {"(SELECT 1 AS id)": {"id": "INT"}}},
                sql_analysis_dialect="duckdb",
                models=[
                    compiled_model(
                        name="stg_orders",
                        columns=[*_TYPED_ORDERS, inferred_column(name="note", type_sql=None)],
                        references=[reference(kind="SOURCE", name="raw_orders")],
                        star=(True, True),
                    ),
                    compiled_model(
                        name="every_order",
                        columns=_TYPED_ORDERS,
                        references=[reference(kind="REF", name="stg_orders")],
                        star=(True, True),
                    ),
                ],
            ),
            expected_present=frozenset(
                {
                    "models",
                    "sources",
                    "typed_column",
                    "untyped_column",
                    "star_over_complete_input",
                    "star_over_published_shape",
                    "source_expression_shape",
                    "dataflow_binding",
                    "dialect_duckdb",
                }
            ),
            expected_absent=frozenset({"star_unresolved", "batch_binding", "upstream_chain"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="columns_without_binding_validation_prove_nothing",
            capture=empty_compiled_capture(
                sql_analysis_dialect="duckdb",
                unchecked_output_columns={"__set__": []},
                models=[
                    compiled_model(
                        name="stg_orders",
                        query_sql="WITH a AS (SELECT 1 AS id) SELECT id FROM a UNION SELECT 2",
                        columns=_TYPED_ORDERS,
                        references=[reference(kind="REF", name="upstream")],
                        validated=False,
                        star=(True, False),
                    ),
                    compiled_model(name="upstream", columns=None, validated=False),
                ],
            ),
            expected_present=frozenset({"models"}),
            expected_absent=frozenset(
                {
                    "sql_analysis_disabled",
                    "typed_column",
                    "star_unresolved",
                    "set_operation",
                    "cte",
                    "dialect_duckdb",
                    "unchecked_output_columns",
                }
            ),
        ),
        AnalysisCaptureKindsTestCase(
            description="contract_types_and_nullability_published_downstream",
            capture=empty_compiled_capture(
                models=[
                    compiled_model(
                        name="contracted",
                        columns=[
                            inferred_column(name="id", type_sql="INT"),
                            inferred_column(name="label", type_sql="VARCHAR(24)"),
                        ],
                        values=_CONTRACT,
                        schema_columns=_CONTRACT_COLUMNS,
                    ),
                    compiled_model(
                        name="below",
                        columns=[
                            inferred_column(name="id", type_sql="INT", nullability="NON_NULL"),
                            inferred_column(name="label", type_sql="TEXT(24)"),
                        ],
                        references=[reference(kind="REF", name="contracted")],
                    ),
                ],
                settings={"table_promotion_mode": "staged"},
            ),
            expected_present=frozenset(
                {
                    "sized_contract_shape",
                    "contract_nullability_shape",
                    "batch_binding",
                    "explicit_promotion_mode",
                }
            ),
            expected_absent=frozenset({"dataflow_binding"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="nullability_already_inferred_upstream_is_not_contract_evidence",
            capture=empty_compiled_capture(
                models=[
                    compiled_model(
                        name="contracted",
                        columns=[
                            inferred_column(name="id", type_sql="INT", nullability="NON_NULL")
                        ],
                        values=_CONTRACT,
                        schema_columns=_CONTRACT_COLUMNS,
                    ),
                    compiled_model(
                        name="below",
                        columns=[
                            inferred_column(name="id", type_sql="INT", nullability="NON_NULL")
                        ],
                        references=[reference(kind="REF", name="contracted")],
                    ),
                ],
            ),
            expected_present=frozenset({"typed_column"}),
            expected_absent=frozenset({"contract_nullability_shape", "sized_contract_shape"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="selection_leaves_unselected_models_unanalysed",
            capture=empty_compiled_capture(
                sources=[compiled_source(name="raw_orders", expression="(SELECT 1 AS id)")],
                binding_catalog={
                    "expression_shapes": {
                        "__unordered_mapping__": [["(SELECT 1 AS id)", {"id": "INT"}]]
                    }
                },
                models=[
                    compiled_model(
                        name="stg_orders",
                        columns=_TYPED_ORDERS,
                        references=[reference(kind="SOURCE", name="raw_orders")],
                    ),
                    compiled_model(
                        name="totals",
                        columns=None,
                        references=[reference(kind="REF", name="stg_orders")],
                        validated=False,
                    ),
                ],
            ),
            expected_present=frozenset({"select_limited_analysis", "batch_binding"}),
            expected_absent=frozenset({"sql_analysis_disabled", "dataflow_binding"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="unresolved_expression_source_is_not_a_complete_input",
            capture=empty_compiled_capture(
                sources=[compiled_source(name="raw_orders", expression="(SELECT 1 AS id)")],
                binding_catalog={
                    "expression_shapes": {"__unordered_mapping__": [["(SELECT 1 AS id)", None]]}
                },
                models=[
                    compiled_model(
                        name="stg_orders",
                        columns=_TYPED_ORDERS,
                        references=[reference(kind="SOURCE", name="raw_orders")],
                    )
                ],
            ),
            expected_present=frozenset({"dataflow_binding"}),
            expected_absent=frozenset({"batch_binding"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="dynamic_column_contract_is_not_a_complete_input",
            capture=empty_compiled_capture(
                models=[
                    compiled_model(
                        name="contracted",
                        columns=[inferred_column(name="id", type_sql="INT")],
                        values=_CONTRACT,
                        schema_columns=_CONTRACT_COLUMNS,
                        dynamic_columns=[{"name": "status_amounts"}],
                    ),
                    compiled_model(
                        name="below",
                        columns=[inferred_column(name="id", type_sql="INT")],
                        references=[reference(kind="REF", name="contracted")],
                    ),
                ],
            ),
            expected_present=frozenset({"dataflow_binding"}),
            expected_absent=frozenset({"batch_binding"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="hooks_and_promotion_on_an_analysed_model",
            capture=hooked_compiled_capture(columns=_TYPED_ORDERS),
            expected_present=frozenset(_CONFIG_KINDS),
            expected_absent=frozenset(),
        ),
        AnalysisCaptureKindsTestCase(
            description="hooks_and_promotion_on_an_unanalysed_model_are_not_credited",
            capture=hooked_compiled_capture(columns=None),
            expected_present=frozenset({"models"}),
            expected_absent=frozenset(_CONFIG_KINDS),
        ),
        AnalysisCaptureKindsTestCase(
            description="literal_sql_relations_need_explicit_references",
            capture=empty_compiled_capture(
                enforce_explicit_references=False,
                unmatched_literal_sql_relations=[{"owner_label": "asset:export"}],
            ),
            expected_present=frozenset({"unmatched_literal_sql_relations"}),
            expected_absent=frozenset({"python_sql_literal_relation"}),
        ),
        AnalysisCaptureKindsTestCase(
            description="quoted_identifier_bound_only_by_ignoring_case",
            capture=empty_compiled_capture(
                models=[
                    compiled_model(name="stg_orders", columns=_TYPED_ORDERS),
                    compiled_model(
                        name="quoted",
                        query_sql='SELECT "ID" AS "OrderKey" FROM __ref("stg_orders")',
                        columns=[inferred_column(name="OrderKey", type_sql="INT")],
                        references=[reference(kind="REF", name="stg_orders")],
                    ),
                ],
            ),
            expected_present=frozenset({"quoted_identifier_case"}),
            expected_absent=frozenset({"cte", "set_operation"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_capture_when_classifying_then_only_analysed_evidence_counts(
    test_case: AnalysisCaptureKindsTestCase,
) -> None:
    kinds: frozenset[str] = analysed_input_kinds(json.dumps(test_case.capture))

    assert test_case.expected_present <= kinds, test_case.expected_present - kinds
    assert not test_case.expected_absent & kinds, test_case.expected_absent & kinds


@pytest.mark.parametrize(
    "test_case",
    [
        RequiredKindsTestCase(
            description="compiled_collections_and_detail_kinds",
            expected_required=frozenset(
                {"models", "sql_tests", "sql_expansions", "typed_column", "dialect_snowflake"}
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_project_fields_when_listing_required_kinds_then_settings_are_excluded(
    test_case: RequiredKindsTestCase,
) -> None:
    required: tuple[str, ...] = required_analysis_kinds()

    assert test_case.expected_required <= set(required)
    assert not ANALYSIS_NON_COLLECTION_FIELDS & set(required)
    assert len(required) == len(set(required))


@pytest.mark.parametrize(
    "test_case",
    [
        AnalysisCaptureProblemsTestCase(
            description="binding_catalog_native_handle_is_expected",
            capture_text=json.dumps(
                empty_compiled_capture(
                    binding_catalog={"native": {"__opaque__": "sqlbuild._native:ProjectCatalog"}}
                )
            ),
            expected_problem_count=0,
        ),
        AnalysisCaptureProblemsTestCase(
            description="any_other_opaque_value_is_a_problem",
            capture_text=json.dumps(
                empty_compiled_capture(models=[{"__opaque__": "sqlbuild._native:Unexpected"}])
            ),
            expected_problem_count=1,
        ),
        AnalysisCaptureProblemsTestCase(
            description="missing_field_is_a_problem",
            capture_text=json.dumps(
                {
                    key: value
                    for key, value in empty_compiled_capture().items()
                    if key != "sql_scenarios"
                }
            ),
            expected_problem_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_compiled_capture_when_checking_soundness_then_problems_are_counted(
    test_case: AnalysisCaptureProblemsTestCase,
) -> None:
    assert len(analysis_capture_problems(test_case.capture_text)) == (
        test_case.expected_problem_count
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
