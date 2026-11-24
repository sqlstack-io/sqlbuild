"""The project's custom Rule and its public-harness tests."""

from __future__ import annotations

from duckshop10k.core import Project

RULE = '''"""DuckShop 10K house rules for the staging layer."""

import re

from sqlbuild.rules import Finding, Model, RuleContext, rule

_SOURCE_CALL = re.compile(r'__source\\(\\s*"([^"]+)"\\s*\\)')
_DEDUPE = ("@sql.cdc_latest(", "@sql.cdc_history(")


def _is_change_log(ctx: RuleContext, source: str) -> bool:
    """Whether a source declares an _op column, read from its instance's own sources file."""
    path = f"sql/sources/{source.split('__', 1)[0]}.yml"
    if not ctx.project.tree.glob(path):
        return False
    text = ctx.project.tree.read_text(path)
    start = text.find(f"  - name: {source}\\n")
    if start < 0:
        return False
    end = text.find("\\n  - name: ", start + 1)
    block = text[start:] if end < 0 else text[start:end]
    return "      - name: _op\\n" in block


@rule(
    code="XSQBRDUCK001",
    message="Staging models over a change-log source must deduplicate it with a change-log macro",
    remediation=(
        "Read the source in an import CTE and follow it with "
        "@sql.cdc_latest(<cte>, src.key := (<key>)), or "
        "@sql.cdc_history(<cte>, src.key := (<key>)) for a model that keeps every version, so "
        "deletes and superseded versions are handled the same way everywhere."
    ),
)
def staging_uses_change_log_macro(*, model: Model, ctx: RuleContext) -> list[Finding]:
    parts = ctx.project.tree.relative_parts(path=model.path, under="sql/models")
    if not parts or parts[0] != "staging":
        return []
    sql = ctx.sql.for_model(model).authored.source
    if any(macro in sql for macro in _DEDUPE):
        return []
    for match in _SOURCE_CALL.finditer(sql):
        if _is_change_log(ctx, match.group(1)):
            line = sql[: match.start()].count("\\n") + 1
            return [ctx.finding(subject=model, line=line)]
    return []
'''

RULE_TEST = '''from rules.staging_dedupe import staging_uses_change_log_macro

from sqlbuild.rules import RuleFile
from sqlbuild.rules.testing import RuleCase, evaluate_rule

_SOURCES = RuleFile(
    path="sql/sources/shop_xx.yml",
    source="""sources:
  - name: shop_xx__orders
    description: Orders change log
    columns:
      - name: order_id
        type: VARCHAR
      - name: _op
        type: VARCHAR
  - name: shop_xx__page_views
    description: Append-only page views
    columns:
      - name: event_id
        type: VARCHAR
""",
)

_RAW_CHANGE_LOG_READ = (
    'MODEL (description "Orders");\\n'
    "WITH src AS (\\n"
    '  SELECT * FROM __source("shop_xx__orders")\\n'
    "),\\n"
    "final AS (\\n"
    "  SELECT s.order_id FROM src AS s\\n"
    ")\\n"
    "SELECT * FROM final\\n"
)


def test_given_change_log_read_without_dedupe_macro_when_evaluating_then_reports_it() -> None:
    result = evaluate_rule(
        rule=staging_uses_change_log_macro,
        test_case=RuleCase(
            description="staging model reads a change log directly",
            source=_RAW_CHANGE_LOG_READ,
            path="sql/models/staging/storefronts/shop_xx/base_shop_xx__orders.sql",
            files=(_SOURCES,),
            expected_finding_count=1,
        ),
    )

    assert result.finding_count == 1
    assert result.findings[0].line == 3


def test_given_append_only_source_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=staging_uses_change_log_macro,
        test_case=RuleCase(
            description="append-only sources are deduplicated by delivery sequence instead",
            source=_RAW_CHANGE_LOG_READ.replace("shop_xx__orders", "shop_xx__page_views").replace(
                "order_id", "event_id"
            ),
            path="sql/models/staging/storefronts/shop_xx/base_shop_xx__page_views.sql",
            files=(_SOURCES,),
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0


def test_given_change_log_read_outside_staging_when_evaluating_then_passes() -> None:
    result = evaluate_rule(
        rule=staging_uses_change_log_macro,
        test_case=RuleCase(
            description="snapshots and other layers may read the full change log",
            source=_RAW_CHANGE_LOG_READ,
            path="sql/models/snapshots/snap_orders.sql",
            files=(_SOURCES,),
            expected_finding_count=0,
        ),
    )

    assert result.finding_count == 0
'''


def build_rules(project: Project) -> None:
    project.write("rules/staging_dedupe.py", RULE)
    project.write("tests/test_rule_staging_dedupe.py", RULE_TEST)
