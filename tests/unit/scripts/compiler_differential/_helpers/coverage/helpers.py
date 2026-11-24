"""Capture fragments for discovery, render and analysis coverage tests."""

from __future__ import annotations

import dataclasses
from itertools import filterfalse
from typing import cast

from sqlbuild._native import (
    STAGE_CAPTURE_OMITTED_ATTRIBUTES,
    CompiledProject,
    CompileProjectInputs,
    DiscoveredProjectInputs,
)

_VALUE_KIND: str = "sqlbuild.sql_values.types:SqlValueKind"


def constant_declaration(*, kind: str, value: object) -> dict[str, object]:
    """Return one captured constant declaration of a logical kind."""

    return {
        "value": {"logical_type": {"kind": {"__enum__": f"{_VALUE_KIND}.{kind}"}}, "value": value}
    }


def empty_inputs_capture(**overrides: object) -> dict[str, object]:
    """Return a capture with every captured `DiscoveredProjectInputs` field, then overrides."""

    type_name: str = "sqlbuild.compiler.discovery.models:DiscoveredProjectInputs"
    names: list[str] = [field.name for field in dataclasses.fields(DiscoveredProjectInputs)]
    return {
        "__type__": type_name,
        **dict.fromkeys(
            filterfalse(STAGE_CAPTURE_OMITTED_ATTRIBUTES[type_name].__contains__, names), []
        ),
        **overrides,
    }


_DECLARATION_KIND: str = "sqlbuild.compiler.scopes.types:DeclarationKind"
_RESOURCE_KIND: str = "sqlbuild.compiler.scopes.types:ResourceKind"
_AUDIT_TARGET: str = "sqlbuild.compiler.compile.types:AttachedAuditTargetKind"
_REFERENCE_KIND: str = "sqlbuild.compiler.references.types:SqlReferenceKind"


def empty_render_capture(**overrides: object) -> dict[str, object]:
    """Return every captured `CompileProjectInputs` field, in order, then overrides."""

    type_name: str = "sqlbuild.compiler.compile.models:CompileProjectInputs"
    names: list[str] = [field.name for field in dataclasses.fields(CompileProjectInputs)]
    return {
        "__type__": type_name,
        **dict.fromkeys(
            filterfalse(STAGE_CAPTURE_OMITTED_ATTRIBUTES[type_name].__contains__, names), []
        ),
        **overrides,
    }


def usage(*, kind: str, name: str, enum_member: str | None = None) -> dict[str, object]:
    """Return one captured direct usage of a project declaration."""

    return {
        "declaration": {
            "kind": {"__enum__": f"{_DECLARATION_KIND}.{kind}"},
            "name": name,
            "owner": None,
        },
        "through": None,
        "enum_member": enum_member,
    }


def private_usage(*, kind: str, name: str, owner: str) -> dict[str, object]:
    """Return one captured usage of a declaration private to the model `owner`."""

    direct: dict[str, object] = usage(kind=kind, name=name)
    return {
        **direct,
        "declaration": {**_as_object(direct["declaration"]), "owner": _model(owner)},
    }


def granted_usage(*, kind: str, name: str, through: str) -> dict[str, object]:
    """Return one captured usage granted through the expected model `through`."""

    return {**usage(kind=kind, name=name), "through": _model(through)}


def _model(name: str) -> dict[str, object]:
    return {"kind": {"__enum__": f"{_RESOURCE_KIND}.MODEL"}, "name": name}


def _as_object(value: object) -> dict[str, object]:
    return cast(dict[str, object], value)


def reference(*, kind: str, name: str) -> dict[str, object]:
    """Return one captured SQL reference."""

    return {"ref_kind": {"__enum__": f"{_REFERENCE_KIND}.{kind}"}, "ref_name": name}


def audit_input(*, definition: str, template: str, rendered: str) -> dict[str, object]:
    """Return one captured generic audit attached to column `amount` of model `orders`."""

    return {
        "audit_file": {"relative_path": {"__path__": f"sql/audits/generic/{definition}.sql"}},
        "audit_block": {"sql_body": template},
        "sql_body": rendered,
        "attached_target_kind": {"__enum__": f"{_AUDIT_TARGET}.MODEL"},
        "attached_target_name": "orders",
        "attached_column_name": "amount",
    }


_NULLABILITY: str = "sqlbuild.compiler.lineage.types:InferredNullability"


def empty_compiled_capture(**overrides: object) -> dict[str, object]:
    """Return a capture with every captured `CompiledProject` field, in order, then overrides."""

    type_name: str = "sqlbuild.compiler.compile.models:CompiledProject"
    omitted: frozenset[str] = STAGE_CAPTURE_OMITTED_ATTRIBUTES.get(type_name, frozenset())
    names: list[str] = [field.name for field in dataclasses.fields(CompiledProject)]
    return {
        "__type__": type_name,
        **dict.fromkeys(filterfalse(omitted.__contains__, names), []),
        **overrides,
    }


def inferred_column(*, name: str, type_sql: str | None, nullability: str = "UNKNOWN") -> object:
    """Return one captured inferred output column."""

    return {
        "name": name,
        "type": type_sql,
        "nullability": {"__enum__": f"{_NULLABILITY}.{nullability}"},
    }


def compiled_model(
    *,
    name: str,
    query_sql: str = "SELECT 1 AS id",
    columns: list[object] | None = None,
    references: list[dict[str, object]] | None = None,
    validated: bool = True,
    values: dict[str, object] | None = None,
    schema_columns: list[dict[str, object]] | None = None,
    dynamic_columns: list[dict[str, object]] | None = None,
    star: tuple[bool, bool] = (False, False),
) -> dict[str, object]:
    """Return one captured compiled model; `columns=None` means it was not analysed."""

    return {
        "name": name,
        "query_sql": query_sql,
        "config": {"values": values or {}},
        "references": references or [],
        "schema_entry": {
            "columns": schema_columns or [],
            "dynamic_columns": dynamic_columns or [],
            "audits": [],
        },
        "inferred_columns": columns,
        "binding_validated": validated,
        "binding_diagnostics": [],
        "fast_lineage_has_star": star[0],
        "fast_lineage_star_resolved": star[1],
        "dynamic_column_contract": None,
        "rejected_sql_analysis_opt_out": None,
        "destination": {"schema": "analytics"},
    }


def compiled_source(*, name: str, expression: str | None) -> dict[str, object]:
    """Return one captured compiled source with an optional SQL expression."""

    return {"source_entry": {"name": name, "expression": expression, "columns": []}}


def hooked_compiled_capture(*, columns: list[object] | None) -> dict[str, object]:
    """Return a staged-promotion capture whose contracted model has inline and named SQL hooks."""

    hooks: list[dict[str, object]] = [
        {"__type__": "sqlbuild:SqlHookEntry", "definition_sql": None},
        {"__type__": "sqlbuild:SqlHookEntry", "definition_sql": "SELECT 1"},
    ]
    return empty_compiled_capture(
        settings={"table_promotion_mode": "staged"},
        models=[
            compiled_model(
                name="contracted",
                columns=columns,
                validated=columns is not None,
                values={"contract": "enforced", "materialized": "table", "post_hooks": hooks},
                schema_columns=[{"name": "id", "type": "INTEGER", "nullable": False}],
            )
        ],
    )
