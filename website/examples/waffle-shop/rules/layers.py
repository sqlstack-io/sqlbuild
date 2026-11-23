from sqlbuild.rules import Finding, Model, RuleContext, rule


@rule(
    code="XSQBRARCH001",
    message="Marts must read sources through staging",
    remediation="Reference a staging model with __ref() instead.",
)
def marts_use_staging(*, model: Model, ctx: RuleContext) -> list[Finding]:
    layer = ctx.project.tree.relative_parts(path=model.path, under="sql/models")[0]
    sql = ctx.sql.for_model(model).authored.source
    if layer != "marts" or "__source(" not in sql:
        return []
    line = sql[: sql.index("__source(")].count("\n") + 1
    return [ctx.finding(subject=model, line=line)]
