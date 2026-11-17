"""DuckShop house rule that keeps case-sensitive output names inside partner export models."""

from sqlbuild.rules import Finding, Model, RuleContext, rule

EXPORT_SUFFIX = "_export"


@rule(
    code="XSQBRDUCK003",
    message="Only partner export models may give columns quoted mixed-case names",
    remediation=(
        "Name the column in unquoted snake_case, or move the projection into a model whose name "
        "ends in _export, which writes a partner's own feed format."
    ),
)
def mixed_case_names_stay_in_exports(*, model: Model, ctx: RuleContext) -> list[Finding]:
    if model.path.stem.endswith(EXPORT_SUFFIX):
        return []
    sql = ctx.sql.for_model(model).expanded
    if '"' not in sql.source:
        return []
    findings: list[Finding] = []
    for node in sql.ast().walk():
        if node.kind != "alias" or not node.quoted or node.name is None:
            continue
        if node.name == node.name.lower():
            continue
        text = sql.source[node.span[0] : node.span[1]] if node.span else node.name
        findings.append(
            ctx.finding(
                subject=model,
                message=f"Quoted mixed-case name {node.name!r} in `{text}`",
            )
        )
    return findings
