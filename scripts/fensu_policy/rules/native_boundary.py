"""Rules keeping the native bindings crate within its Python edges and module layers."""

from __future__ import annotations

import ast

from fensu import AnalyzerId, Family, Fault, File, RuleContext, rule

from scripts.fensu_policy._helpers.native_crossings import (
    budget_faults,
    budgeted_faults,
    budgets_by_path,
    crossing_lines,
    python_field_lines,
)
from scripts.fensu_policy._helpers.native_layers import (
    helper_module,
    layer_plan,
    upward_reference_lines,
)
from scripts.fensu_policy._helpers.native_patches import native_patch_lines
from scripts.fensu_policy.constants import (
    NATIVE_EDGE_PATHS,
    NATIVE_LAYER_ORDER,
    NATIVE_MODULE_LAYERS,
    NATIVE_PENDING_BUDGETS,
    NATIVE_TEST_PATCH_BUDGETS,
    NATIVE_TEST_PATH_PREFIX,
)
from scripts.fensu_policy.models import LayerPlan


@rule(
    code="XSB072",
    family=Family.CUSTOM,
    slug="native-python-crossing",
    message="native bindings must not call back into Python outside the configured edges",
    remediation=(
        "Read native records with sqlbuild_python_base::records::main::fields, build them with "
        "sqlbuild_python_base::records::main::construct, call Rust functions directly, or route "
        "a real edge through sqlbuild_python_base::native_host."
    ),
    analyzers=(AnalyzerId.RUST,),
    options=(NATIVE_EDGE_PATHS, NATIVE_PENDING_BUDGETS),
)
def native_python_crossing(*, file: File, ctx: RuleContext) -> list[Fault]:
    del file
    return budgeted_faults(ctx=ctx, lines=crossing_lines, what="Python crossings")


@rule(
    code="XSB073",
    family=Family.CUSTOM,
    slug="native-python-storage",
    message="native bindings must not store Python objects outside the configured edges",
    remediation=(
        "Store Rust values or typed native records; keep Python objects only in "
        "sqlbuild_python_base::native_host for user values, adapters, hooks and streams."
    ),
    analyzers=(AnalyzerId.RUST,),
    options=(NATIVE_EDGE_PATHS, NATIVE_PENDING_BUDGETS),
)
def native_python_storage(*, file: File, ctx: RuleContext) -> list[Fault]:
    del file
    return budgeted_faults(ctx=ctx, lines=python_field_lines, what="Python-typed fields")


@rule(
    code="XSB075",
    family=Family.CUSTOM,
    slug="native-module-layering",
    message="native binding helper modules must import only from lower layers",
    remediation=(
        "Move the shared item into a module of a lower layer, or move the importing code up; "
        "assign every new helper module a layer in module_layers."
    ),
    analyzers=(AnalyzerId.RUST,),
    options=(NATIVE_LAYER_ORDER, NATIVE_MODULE_LAYERS, NATIVE_PENDING_BUDGETS),
)
def native_module_layering(*, file: File, ctx: RuleContext) -> list[Fault]:
    del file
    path: str = "/".join(ctx.repo_relative_parts())
    if helper_module(path=path) is None:
        return []
    plan: LayerPlan = layer_plan(
        order=ctx.option(NATIVE_LAYER_ORDER), modules=ctx.option(NATIVE_MODULE_LAYERS)
    )
    found: list[int] | None = upward_reference_lines(path=path, source=ctx.source, plan=plan)
    if found is None:
        return [
            ctx.fault_for(
                path=ctx.path,
                line=1,
                column=0,
                message="helper module has no layer",
                remediation="Assign the module a layer in module_layers.",
            )
        ]
    return budget_faults(ctx=ctx, path=path, found=found, what="upward module imports")


@rule(
    code="XSB074",
    family=Family.CUSTOM,
    slug="native-module-test-patch",
    message="tests must not replace attributes of sqlbuild._native",
    remediation=(
        "Assert observable behaviour or native work counters instead; native code calls its own "
        "functions directly, so a patched module attribute is not seen."
    ),
    options=(NATIVE_TEST_PATCH_BUDGETS,),
)
def native_module_test_patch(*, module: object, ctx: RuleContext) -> list[Fault]:
    path: str = "/".join(ctx.repo_relative_parts())
    if not path.startswith(NATIVE_TEST_PATH_PREFIX):
        return []
    tree: ast.Module = module if isinstance(module, ast.Module) else ast.parse(ctx.text.source)
    found: list[int] = native_patch_lines(tree=tree)
    budget: int = budgets_by_path(entries=ctx.option(NATIVE_TEST_PATCH_BUDGETS)).get(path, 0)
    if len(found) == budget:
        return []
    if len(found) < budget:
        return [
            ctx.fault_for(
                path=ctx.path,
                line=1,
                column=0,
                message=(
                    f"{len(found)} native patches, below the pending budget of {budget}; lower it"
                ),
            )
        ]
    return [ctx.fault_for(path=ctx.path, line=line, column=0) for line in found[budget:]]
