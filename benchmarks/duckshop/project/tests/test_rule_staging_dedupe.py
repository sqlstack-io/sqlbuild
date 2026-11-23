from rules.staging_dedupe import staging_uses_cdc_latest

from sqlbuild.rules import RuleFile
from sqlbuild.rules.testing import RuleCase, evaluate_rule

_SOURCES = RuleFile(
    path="sql/sources/shop.yml",
    source="""sources:
  - name: shop__orders
    description: Orders change log
    columns:
      - name: order_id
        type: VARCHAR
      - name: _op
        type: VARCHAR
  - name: shop__page_views
    description: Append-only page views
    columns:
      - name: event_id
        type: VARCHAR
""",
)

_RAW_CHANGE_LOG_READ = (
    'MODEL (description "Orders");\n'
    "WITH src AS (\n"
    '  SELECT * FROM __source("shop__orders")\n'
    "),\n"
    "final AS (\n"
    "  SELECT s.order_id FROM src AS s\n"
    ")\n"
    "SELECT * FROM final\n"
)


def test_given_change_log_read_without_cdc_latest_when_evaluating_then_reports_it() -> None:
    result = evaluate_rule(
        rule=staging_uses_cdc_latest,
        test_case=RuleCase(
            description="staging model reads a change log directly",
            source=_RAW_CHANGE_LOG_READ,
            path="sql/models/staging/shop/stg_orders.sql",
            files=(_SOURCES,),
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
    assert result.findings[0].line == 3


def test_given_staging_model_reading_append_only_source_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=staging_uses_cdc_latest,
        test_case=RuleCase(
            description="append-only sources are deduplicated another way",
            source=_RAW_CHANGE_LOG_READ.replace("shop__orders", "shop__page_views").replace(
                "order_id", "event_id"
            ),
            path="sql/models/staging/shop/stg_page_views.sql",
            files=(_SOURCES,),
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0


def test_given_change_log_read_outside_staging_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=staging_uses_cdc_latest,
        test_case=RuleCase(
            description="snapshots and other layers may read the full change log",
            source=_RAW_CHANGE_LOG_READ,
            path="sql/models/snapshots/snap_orders.sql",
            files=(_SOURCES,),
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0
