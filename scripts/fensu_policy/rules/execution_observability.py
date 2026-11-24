"""Ownership rules for statement execution, lifecycle events, and event exporters."""

from __future__ import annotations

import ast

from fensu import (
    Family,
    Fault,
    ImportFact,
    NamedCallFact,
    QualifiedReferenceFact,
    RuleContext,
    rule,
)

from scripts.fensu_policy._helpers.adapter_contracts import adapter_contract_class_names
from scripts.fensu_policy._helpers.execution_observability import (
    ast_nodes,
    expression_reference_parts,
    import_source_parts,
    imported_reference_matches,
    private_exporter_imports_are_allowed,
    raw_execution_calls,
)
from scripts.fensu_policy.constants import (
    ADAPTER_PUBLIC_EXECUTE_NAME,
    ADAPTER_PUBLIC_EXECUTE_OWNER,
    APPROVED_RAW_STATEMENT_MODULES,
    CORE_EVENT_EXPORTER_CLASS_NAMES,
    DIAGNOSTIC_CONSTRUCTION_OWNER_PATHS,
    DIAGNOSTIC_LOG_CLASS_NAME,
    EVENT_EXPORTER_DECORATOR_NAMES,
    EVENT_EXPORTER_DIRECTORY_PARTS,
    EVENT_EXPORTER_MODULE_PARTS,
    EVENT_EXPORTER_RUNTIME_PREFIX,
    LIFECYCLE_CATALOG_NAME_PREFIX,
    LIFECYCLE_CATALOG_OWNER_PATH,
    LIFECYCLE_CONSTRUCTION_OWNER_PATHS,
    LIFECYCLE_DEFINITION_CLASS_NAME,
    LIFECYCLE_DEFINITION_FACTORY_NAME,
    LIFECYCLE_EVENT_CLASS_NAME,
    LIFECYCLE_EVENT_FACTORY_NAME,
    LIFECYCLE_FACTORY_OWNER_PATHS,
    OBSERVABILITY_DEFINITION_MODULES,
    OBSERVABILITY_FACTORY_MODULES,
    OBSERVABILITY_LIFECYCLE_EVENT_MODULES,
    PUBLIC_EVENT_EXPORTER_MODULE_PARTS,
    ROOT_SCOPE_NAME,
    RUNTIME_ROOT_PARTS,
)


@rule(
    code="XSB068",
    family=Family.CUSTOM,
    slug="adapter-public-execute-override",
    message="adapter implementations must inherit the framework-owned execute entrypoint",
    remediation=(
        "Implement the protected _execute hook; do not override public execute or delegate it "
        "to super()."
    ),
)
def adapter_public_execute_override(*, module: object, ctx: RuleContext) -> list[Fault]:
    """Keep lifecycle instrumentation in the contract template; adapters own only `_execute`."""

    del module
    if "/".join(ctx.repo_relative_parts()) == ADAPTER_PUBLIC_EXECUTE_OWNER:
        return []
    adapter_names: frozenset[str] = adapter_contract_class_names(
        path_parts=ctx.repo_relative_parts(), ctx=ctx
    )
    faults: list[Fault] = []
    for declaration in ctx.facts.class_declarations():
        if not declaration.top_level or declaration.name not in adapter_names:
            continue
        for method in declaration.methods:
            if method.name == ADAPTER_PUBLIC_EXECUTE_NAME:
                faults.append(ctx.fault_at(location=method.location))
    return faults


@rule(
    code="XSB069",
    family=Family.CUSTOM,
    slug="raw-driver-execution-boundary",
    message="raw driver execution must stay inside an approved observed statement executor",
    remediation=(
        "Call adapter.execute(), or route the driver call through "
        "ObservedConnection/ObservedCursor."
    ),
)
def raw_driver_execution_boundary(*, module: object, ctx: RuleContext) -> list[Fault]:
    """Reject proven raw calls; reflection and opaque factory returns remain dynamic limits."""

    if ctx.scope() != ROOT_SCOPE_NAME or ctx.repo_relative_parts()[:2] != RUNTIME_ROOT_PARTS:
        return []
    path: str = "/".join(ctx.repo_relative_parts())
    if path in APPROVED_RAW_STATEMENT_MODULES:
        return []
    tree: ast.Module = module if isinstance(module, ast.Module) else ast.parse(ctx.text.source)
    return [
        ctx.fault_for(path=ctx.path, line=call.lineno, column=call.col_offset)
        for call in raw_execution_calls(tree=tree, ctx=ctx)
    ]


@rule(
    code="XSB070",
    family=Family.CUSTOM,
    slug="event-construction-ownership",
    message="canonical lifecycle records and catalogs must be created by observability owners",
    remediation=(
        "Use MicrobatchLifecycle, OperationLifecycle, ResourceAttemptLifecycle, "
        "StatementLifecycle, or dispatcher APIs."
    ),
)
def event_construction_ownership(*, module: object, ctx: RuleContext) -> list[Fault]:
    """Restrict construction while permitting public event annotations, reads, and projections."""

    del module
    if ctx.scope() != ROOT_SCOPE_NAME or ctx.repo_relative_parts()[:2] != RUNTIME_ROOT_PARTS:
        return []
    path: str = "/".join(ctx.repo_relative_parts())
    faults: list[Fault] = []
    call: NamedCallFact
    for call in ctx.facts.named_calls():
        reference: QualifiedReferenceFact | None = call.reference
        if reference is None:
            continue
        called_name: str | None = reference.base_name
        if called_name is None:
            continue
        if imported_reference_matches(
            ctx=ctx,
            reference_parts=reference.parts,
            symbol=LIFECYCLE_EVENT_CLASS_NAME,
            source_modules=OBSERVABILITY_LIFECYCLE_EVENT_MODULES,
        ):
            if path not in LIFECYCLE_CONSTRUCTION_OWNER_PATHS:
                faults.append(ctx.fault_at(location=call.location))
        elif imported_reference_matches(
            ctx=ctx,
            reference_parts=reference.parts,
            symbol=DIAGNOSTIC_LOG_CLASS_NAME,
            source_modules=OBSERVABILITY_LIFECYCLE_EVENT_MODULES,
        ):
            if path not in DIAGNOSTIC_CONSTRUCTION_OWNER_PATHS:
                faults.append(ctx.fault_at(location=call.location))
        elif imported_reference_matches(
            ctx=ctx,
            reference_parts=reference.parts,
            symbol=LIFECYCLE_EVENT_FACTORY_NAME,
            source_modules=OBSERVABILITY_FACTORY_MODULES,
        ):
            if path not in LIFECYCLE_FACTORY_OWNER_PATHS:
                faults.append(ctx.fault_at(location=call.location))
        elif (
            called_name == LIFECYCLE_DEFINITION_FACTORY_NAME
            and imported_reference_matches(
                ctx=ctx,
                reference_parts=reference.parts[:-1],
                symbol=LIFECYCLE_DEFINITION_CLASS_NAME,
                source_modules=OBSERVABILITY_DEFINITION_MODULES,
            )
            and path != LIFECYCLE_CATALOG_OWNER_PATH
        ):
            faults.append(ctx.fault_at(location=call.location))
    if path != LIFECYCLE_CATALOG_OWNER_PATH:
        for statement in ctx.facts.module_declarations().statements:
            if any(
                name.startswith(LIFECYCLE_CATALOG_NAME_PREFIX)
                for name in statement.assignment_target_names
            ):
                faults.append(ctx.fault_at(location=statement.location))
    return faults


@rule(
    code="XSB071",
    family=Family.CUSTOM,
    slug="sink-location",
    message=("sink declarations and private runtime imports must stay in their owner boundary"),
    remediation=(
        "Put project sink declarations under python/event_sinks/**/*.py and use public APIs "
        "elsewhere."
    ),
)
def event_exporter_location(*, module: object, ctx: RuleContext) -> list[Fault]:
    """Keep project declarations recursive and core internals behind narrow seams."""

    tree: ast.Module = module if isinstance(module, ast.Module) else ast.parse(ctx.text.source)
    path_parts: tuple[str, ...] = ctx.repo_relative_parts()
    path: str = "/".join(path_parts)
    if ctx.scope() != ROOT_SCOPE_NAME:
        return []
    faults: list[Fault] = []
    for node in ast_nodes(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        for decorator in node.decorator_list:
            expression: ast.expr = decorator.func if isinstance(decorator, ast.Call) else decorator
            reference_parts: tuple[str, ...] | None = expression_reference_parts(expression)
            if reference_parts is None or not any(
                imported_reference_matches(
                    ctx=ctx,
                    reference_parts=reference_parts,
                    symbol=decorator_name,
                    source_modules=(PUBLIC_EVENT_EXPORTER_MODULE_PARTS,),
                )
                for decorator_name in EVENT_EXPORTER_DECORATOR_NAMES
            ):
                continue
            if path_parts[: len(EVENT_EXPORTER_DIRECTORY_PARTS)] != EVENT_EXPORTER_DIRECTORY_PARTS:
                faults.append(
                    ctx.fault_for(path=ctx.path, line=decorator.lineno, column=decorator.col_offset)
                )
    if not path.startswith(EVENT_EXPORTER_RUNTIME_PREFIX):
        for declaration in ctx.facts.class_declarations():
            if declaration.top_level and declaration.name in CORE_EVENT_EXPORTER_CLASS_NAMES:
                faults.append(ctx.fault_at(location=declaration.location))
    if private_exporter_imports_are_allowed(path=path):
        return faults
    imported: ImportFact
    for imported in ctx.facts.references().imports:
        if any(
            import_source_parts(imported=imported, alias=alias)[:3] == EVENT_EXPORTER_MODULE_PARTS
            for alias in imported.aliases
        ):
            faults.append(ctx.fault_at(location=imported.location))
    return faults
