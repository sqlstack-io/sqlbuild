"""DuckShop house rules for the staging layer."""

import re

from sqlbuild.rules import Finding, Model, RuleContext, rule

_SOURCE_BLOCK = re.compile(r"^  - name: (\S+)\n(.*?)(?=^  - name: |\Z)", re.MULTILINE | re.DOTALL)
_SOURCE_CALL = re.compile(r'__source\(\s*"([^"]+)"\s*\)')
_CHANGE_LOG_MARKER = re.compile(r"^\s+- name: _op\s*$", re.MULTILINE)


def _change_log_sources(ctx: RuleContext) -> set[str]:
    names: set[str] = set()
    for path in ctx.project.tree.glob("sql/sources/*.yml"):
        text = ctx.project.tree.read_text(path.value)
        for match in _SOURCE_BLOCK.finditer(text):
            if _CHANGE_LOG_MARKER.search(match.group(2)):
                names.add(match.group(1))
    return names


@rule(
    code="XSQBRDUCK001",
    message="Staging models over a change-log source must deduplicate it with @sql.cdc_latest",
    remediation=(
        "Read the source in an import CTE and follow it with "
        "@sql.cdc_latest(<cte>, src.key := (<key>)), then select from cdc_latest so deletes and "
        "superseded versions are handled the same way everywhere."
    ),
)
def staging_uses_cdc_latest(*, model: Model, ctx: RuleContext) -> list[Finding]:
    parts = ctx.project.tree.relative_parts(path=model.path, under="sql/models")
    if not parts or parts[0] != "staging":
        return []
    sql = ctx.sql.for_model(model).authored.source
    if "@sql.cdc_latest(" in sql:
        return []
    change_logs = _change_log_sources(ctx)
    for match in _SOURCE_CALL.finditer(sql):
        if match.group(1) in change_logs:
            line = sql[: match.start()].count("\n") + 1
            return [ctx.finding(subject=model, line=line)]
    return []
