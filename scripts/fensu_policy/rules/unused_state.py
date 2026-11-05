"""Rules flagging control-store state and Rust enum variants that production code never uses."""

from __future__ import annotations

from fensu import AnalyzerId, Family, Fault, Project, RuleContext, rule

from scripts.fensu_policy._helpers.unused_state import (
    code_view,
    collect_reads,
    enum_variants,
    is_rust_test_path,
    named_variants,
    schema_definitions,
    schema_files,
    sql_read_fragments,
    string_literals,
    unread_definitions,
)
from scripts.fensu_policy.constants import CONTROL_STORE_SCHEMA_PATHS
from scripts.fensu_policy.models import EnumVariant, StateDefinition, StateReads


@rule(
    code="XSB076",
    family=Family.CUSTOM,
    slug="control-store-unread-state",
    message="control-store state is written but never read by production code",
    remediation=(
        "Delete the table or column, or add it together with its first production reader; "
        "writes, indexes and tests do not count as reads."
    ),
    cacheable=False,
    analyzers=(AnalyzerId.RUST,),
    options=(CONTROL_STORE_SCHEMA_PATHS,),
)
def control_store_unread_state(*, project: Project, ctx: RuleContext) -> list[Fault]:
    del project
    schema: dict[str, str] = schema_files(
        root=ctx.repo_root, directories=ctx.option(CONTROL_STORE_SCHEMA_PATHS)
    )
    definitions: list[StateDefinition] = []
    fragments: list[str] = []
    path: str
    sql: str
    for path, sql in schema.items():
        definitions.extend(schema_definitions(path=path, sql=sql))
        fragments.extend(sql_read_fragments(sql=sql))
    if not definitions:
        return []
    for facts in ctx.rust.files:
        if not facts.test and not is_rust_test_path(path=facts.file.path.value):
            fragments.extend(string_literals(source=facts.source))
    reads: StateReads = collect_reads(fragments=fragments, definitions=definitions)
    return [
        ctx.fault_for(
            path=ctx.repo_root / definition.path,
            line=definition.line,
            column=0,
            message=(
                f"table {definition.table} is never read by production code"
                if definition.column is None
                else f"column {definition.table}.{definition.column} is never read by production "
                "code"
            ),
        )
        for definition in unread_definitions(definitions=definitions, reads=reads)
    ]


@rule(
    code="XSB077",
    family=Family.CUSTOM,
    slug="unconstructed-enum-variant",
    message="enum variant is never constructed or matched by production code",
    remediation=(
        "Delete the variant, or add it together with the production code that produces it; "
        "tests and serde input alone do not keep a variant alive."
    ),
    analyzers=(AnalyzerId.RUST,),
)
def unconstructed_enum_variant(*, project: Project, ctx: RuleContext) -> list[Fault]:
    del project
    codes: dict[str, str] = {
        facts.file.path.value: code_view(source=facts.source)
        for facts in ctx.rust.files
        if not facts.test and not is_rust_test_path(path=facts.file.path.value)
    }
    variants: list[EnumVariant] = []
    path: str
    code: str
    for path, code in codes.items():
        variants.extend(enum_variants(path=path, code=code))
    named: set[EnumVariant] = named_variants(variants=variants, codes=codes)
    return [
        ctx.fault_for(
            path=ctx.repo_root / variant.path,
            line=variant.line,
            column=0,
            message=f"variant {variant.enum}::{variant.variant} is never constructed or matched",
        )
        for variant in variants
        if variant not in named
    ]
