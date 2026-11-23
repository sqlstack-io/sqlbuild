<!-- generated-by: sqlbuild skills -->

# Overview

> Define repository-owned checks with one typed Python API.

Online: https://sqlbuild.com/docs/concepts/rules/custom-rules/overview/

Custom Rules are ordinary Python under `rules/**/*.py`. Only functions decorated with `@rule`
register; helpers, constants, dataclasses, and classes remain ordinary Python.

## Model-subject Rules

```python
from sqlbuild.rules import Finding, Model, RuleContext, rule

@rule(
    code="XSQBRARCH001",
    message="Final models must declare an order identifier",
    remediation="Declare order_id in the model contract.",
)
def final_order_identifier(*, model: Model, ctx: RuleContext) -> list[Finding]:
    declared = {column.name for column in ctx.columns.declared(model)}
    return [] if "order_id" in declared else [ctx.finding(subject=model)]
```

Annotations determine invocation. The signature must contain exactly one typed `Model` or `Project`
subject and one typed `RuleContext`, all keyword-only. Parameter names and position are not semantic.
A model-subject Rule can still inspect project-wide facts.

## Project-subject Rules

Use `Project` when an invariant has no natural model subject:

```python
from sqlbuild.rules import Finding, Project, RuleContext, rule

@rule(
    code="XSQBRARCH002",
    message="The project must contain a final model directory",
    remediation="Add sql/models/final and place final outputs beneath it.",
)
def final_directory(*, ctx: RuleContext, project: Project) -> list[Finding]:
    del project
    paths = ctx.project.tree.glob("sql/models/final/**")
    return [] if paths else [ctx.finding(subject="models")]
```

Rules cannot mutate SQL, resources, adapter lowering, or compiler output. They return diagnostics
only.

## Typed options

```python
from sqlbuild.rules import Finding, Model, RuleContext, RuleOption, rule

REQUIRED_PREFIX = RuleOption.string(
    name="required_prefix",
    default="customer",
    description="Required model-name prefix.",
)

@rule(
    code="XSQBRNAME001",
    message="The model name uses the wrong prefix",
    remediation="Rename the model with the configured prefix.",
    options=(REQUIRED_PREFIX,),
)
def required_prefix(*, model: Model, ctx: RuleContext) -> list[Finding]:
    prefix = ctx.option(REQUIRED_PREFIX)
    return [] if model.name.startswith(prefix) else [ctx.finding(subject=model)]
```

```toml
[rules.rule_options.XSQBRNAME001]
required_prefix = "order"
```

Unknown codes, option names, or invalid values fail configuration.

## Continue

    Inspect SQL, graph, contracts, tests, audits, declarations, and project structure.
    Test findings and keep helper and project observations cacheable.
