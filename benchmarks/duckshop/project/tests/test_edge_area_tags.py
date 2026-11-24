from rules.edge_area_tags import edge_area_is_tagged

from sqlbuild.rules import RuleCase, evaluate_rule

_TAGGED = 'MODEL (\n  description "Edge orders",\n  tags [edge_case],\n);\nSELECT 1 AS order_id\n'
_UNTAGGED = 'MODEL (\n  description "Orders",\n);\nSELECT 1 AS order_id\n'
_OTHER_TAG = (
    'MODEL (\n  description "Orders",\n  tags [known_divergence],\n);\nSELECT 1 AS order_id\n'
)


def test_given_tagged_model_in_edge_folder_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=edge_area_is_tagged,
        test_case=RuleCase(
            description="edge model declares the edge_case tag",
            source=_TAGGED,
            path="sql/models/intermediate/edge_cases/int_edge_orders.sql",
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0


def test_given_untagged_model_in_edge_folder_when_evaluating_then_reports_it() -> None:
    result = evaluate_rule(
        rule=edge_area_is_tagged,
        test_case=RuleCase(
            description="edge model without the edge_case tag",
            source=_OTHER_TAG,
            path="sql/models/staging/edge_cases/stg_edge_orders.sql",
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
    assert result.findings[0].line == 3


def test_given_tagged_model_outside_edge_folder_when_evaluating_then_reports_it() -> None:
    result = evaluate_rule(
        rule=edge_area_is_tagged,
        test_case=RuleCase(
            description="core model declares the edge_case tag",
            source=_TAGGED,
            path="sql/models/marts/orders/fct_orders.sql",
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1


def test_given_untagged_model_outside_edge_folder_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=edge_area_is_tagged,
        test_case=RuleCase(
            description="core model without the tag",
            source=_UNTAGGED,
            path="sql/models/marts/orders/fct_orders.sql",
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0


def test_given_tags_text_in_description_when_evaluating_then_reads_only_header_tags() -> None:
    result = evaluate_rule(
        rule=edge_area_is_tagged,
        test_case=RuleCase(
            description="a description holding ');' and tags text",
            source=(
                'MODEL (\n  description """Orders (see note);\n  tags [edge_case]""",\n'
                "  columns (\n    order_id (type VARCHAR),\n  ),\n"
                "  tags [orders],\n);\nSELECT 1 AS order_id\n"
            ),
            path="sql/models/staging/edge_cases/stg_edge_orders.sql",
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
    assert result.findings[0].line == 7
