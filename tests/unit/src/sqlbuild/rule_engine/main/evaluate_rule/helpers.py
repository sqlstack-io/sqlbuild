"""Custom rules used by public harness tests."""

from sqlbuild.rules import Finding, Model, RuleContext, RuleOption, rule

_EVALUATED_MODELS: set[str] = set()

REQUIRED_DOMAIN: RuleOption[str] = RuleOption.string(
    name="required_domain",
    default="commerce",
    description="Domain that models must belong to.",
)


@rule(
    code="XSQBRD001",
    slug="required-domain",
    message="model belongs to the wrong domain",
    remediation="Move this model beneath sql/models/<required-domain>/ and rename its domain prefix.",
    options=(REQUIRED_DOMAIN,),
)
def required_domain(*, model: Model, ctx: RuleContext) -> list[Finding]:
    matches: bool = model.name.startswith(f"{ctx.option(REQUIRED_DOMAIN)}__")
    return [ctx.finding(subject=model)] * int(not matches)


@rule(
    code="XSQBRS001",
    slug="first-evaluation-only",
    message="model was already evaluated by this process",
    remediation="Derive findings from the subject and context, not module-level state.",
)
def first_evaluation_only(*, model: Model, ctx: RuleContext) -> list[Finding]:
    repeated: bool = model.name in _EVALUATED_MODELS
    _EVALUATED_MODELS.add(model.name)
    return [ctx.finding(subject=model)] * int(repeated)


@rule(
    code="XSQBRE001",
    slug="status-enum",
    message="status column must use the order_status enum",
    remediation="Declare the column as status (type order_status).",
)
def status_enum(*, model: Model, ctx: RuleContext) -> list[Finding]:
    names: tuple[str, ...] = ctx.columns.names(model) or ()
    typed: bool = ctx.columns.enum_types(model).get("status") == "order_status"
    return [ctx.finding(subject=model)] * int("status" in names and not typed)
