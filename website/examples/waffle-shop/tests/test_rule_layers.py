from rules.layers import marts_use_staging

from sqlbuild.rules.testing import RuleCase, evaluate_rule


def test_given_mart_reading_source_when_evaluating_then_reports_finding() -> None:
    result = evaluate_rule(
        rule=marts_use_staging,
        test_case=RuleCase(
            description="mart reads a raw source",
            source='MODEL ();\nSELECT id FROM __source("raw__payments")\n',
            path="sql/models/marts/payments.sql",
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
