"""Classify discovery captures into input kinds; CONFIG_ONLY_KINDS lists the known gaps."""

from __future__ import annotations

import dataclasses
import json
import re
from collections.abc import Iterator
from pathlib import Path

from scripts.compiler_differential._helpers.comparing.capture_text import expand_capture_text
from scripts.compiler_differential._helpers.comparing.compare import as_json_object
from scripts.compiler_differential._helpers.coverage.capture_values import (
    capture_problems,
    enum_name,
    has_content,
    mapping_size,
    named_values,
    records,
)
from scripts.compiler_differential.constants import (
    CALLABLE_CAPTURE_FIELDS,
    CRLF_BYTES,
    DBT_REF_CALL,
    DISCOVERY_DECLARATION_FILE_COLLECTIONS,
    DISCOVERY_DETAIL_KINDS,
    DISCOVERY_NON_COLLECTION_FIELDS,
    DISCOVERY_STAGE_CAPTURE_SUFFIX,
    DISCOVERY_TEXT_FILE_COLLECTIONS,
    GENERATOR_BOM,
    MICROBATCH_MODE,
    PATH_MARKER,
    RELATIVE_PATH_FIELDS,
    SINGULAR_AUDIT_KIND,
    STRATEGY_HEADER_KEYS,
    TAB,
)
from sqlbuild._native import STAGE_CAPTURE_OMITTED_ATTRIBUTES, DiscoveredProjectInputs
from sqlbuild.adapter.discovery.constants import BUILTIN_ADAPTER_IMPORTS

_OMITTED_DISCOVERY_FIELDS: frozenset[str] = STAGE_CAPTURE_OMITTED_ATTRIBUTES.get(
    "sqlbuild.compiler.discovery.models:DiscoveredProjectInputs", frozenset()
)

_CROSS_FILE_MACRO_IMPORT: re.Pattern[str] = re.compile(r"^(?:from|import) sql\.macros\.", re.MULTILINE)
_NON_ASCII_COMMENT: re.Pattern[str] = re.compile(r"--[^\n]*[^\x00-\x7f]")
_NON_ASCII_STRING: re.Pattern[str] = re.compile(r"'[^'\n]*[^\x00-\x7f][^'\n]*'")


def required_discovery_kinds() -> tuple[str, ...]:
    """Return every `DiscoveredProjectInputs` collection field plus the finer required kinds."""

    return (*discovery_collection_kinds(), *DISCOVERY_DETAIL_KINDS)


def discovery_collection_kinds() -> tuple[str, ...]:
    """Return the `DiscoveredProjectInputs` fields that hold discovered inputs."""

    return tuple(
        field.name
        for field in dataclasses.fields(DiscoveredProjectInputs)
        if field.name not in DISCOVERY_NON_COLLECTION_FIELDS
    )


def project_discovery_kinds(
    *, captures: dict[str, dict[str, Path]], source_dir: Path
) -> frozenset[str]:
    """Return the kinds of every discovery capture plus CRLF, which only the bytes show."""

    kinds: set[str] = set()
    read_paths: set[str] = set()
    for text in _discovery_capture_texts(captures):
        kinds.update(discovered_input_kinds(text))
        read_paths.update(discovered_relative_paths(text))
    kinds.update(authored_byte_kinds(source_dir=source_dir, relative_paths=frozenset(read_paths)))
    return frozenset(kinds)


def discovered_relative_paths(capture_text: str) -> frozenset[str]:
    """Return the project-relative path of every file a discovery capture says it read."""

    capture: object = expand_capture_text(capture_text)
    return frozenset(
        str(path)
        for _, value in named_values(value=capture, names=RELATIVE_PATH_FIELDS)
        if (path := (as_json_object(value) or {}).get(PATH_MARKER)) is not None
    )


def authored_byte_kinds(*, source_dir: Path, relative_paths: frozenset[str]) -> frozenset[str]:
    """Return the line-ending kinds of the authored files discovery read."""

    paths: list[Path] = [source_dir / relative_path for relative_path in sorted(relative_paths)]
    return frozenset(
        {"crlf"}
        if any(path.is_file() and CRLF_BYTES in path.read_bytes() for path in paths)
        else set()
    )


def _discovery_capture_texts(captures: dict[str, dict[str, Path]]) -> Iterator[str]:
    for files in captures.values():
        for name, path in files.items():
            if name.endswith(DISCOVERY_STAGE_CAPTURE_SUFFIX):
                yield path.read_text(encoding="utf-8")


def discovered_input_kinds(capture_text: str) -> frozenset[str]:
    """Return the input kinds one canonical discovery capture proves were discovered."""

    capture: dict[str, object] = as_json_object(expand_capture_text(capture_text)) or {}
    kinds: set[str] = {
        kind for kind in discovery_collection_kinds() if has_content(capture.get(kind))
    }
    kinds.update(_model_kinds(capture))
    kinds.update(_source_kinds(capture))
    kinds.update(_declaration_kinds(capture))
    kinds.update(_test_and_audit_kinds(capture))
    kinds.update(_config_kinds(capture))
    kinds.update(_text_kinds(capture))
    return frozenset(kinds)


def discovery_capture_problems(capture_text: str) -> tuple[str, ...]:
    """Return why a discovery capture is incomplete or not canonical; empty when it is sound."""

    return capture_problems(
        capture_text=capture_text,
        fields=tuple(
            field.name
            for field in dataclasses.fields(DiscoveredProjectInputs)
            if field.name not in _OMITTED_DISCOVERY_FIELDS
        ),
        callable_fields=CALLABLE_CAPTURE_FIELDS,
    )


def _model_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    materializations: set[str] = {
        str(record.get("name")) for record in records(capture.get("materialization_files"))
    }
    for model in records(capture.get("model_files")):
        header: dict[str, object] = as_json_object(model.get("header_values")) or {}
        materialized: object = header.get("materialized")
        if header.get("incremental_mode") == MICROBATCH_MODE:
            kinds.add(f"microbatch_{header.get('microbatch_strategy')}")
        elif materialized in STRATEGY_HEADER_KEYS:
            kinds.add(f"{materialized}_{header.get(STRATEGY_HEADER_KEYS[str(materialized)])}")
        elif materialized in materializations:
            kinds.add("custom_materialized_model")
        if records(model.get("constant_declarations")) or records(model.get("enum_declarations")):
            kinds.add("model_local_declaration")
        if DBT_REF_CALL in str(model.get("contents", "")):
            kinds.add("dbt_ref_model")
    return kinds


def _source_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    for source_file in records(capture.get("source_files")):
        for entry in records(source_file.get("source_entries")):
            kinds.add("managed_source" if entry.get("managed") is True else "unmanaged_source")
    return kinds


def _declaration_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    for collection in DISCOVERY_DECLARATION_FILE_COLLECTIONS:
        for record in records(capture.get(collection)):
            scope: str | None = enum_name(record.get("scope_kind"))
            if scope is not None:
                kinds.add(f"scope_{scope}")
    for constant_file in records(capture.get("constant_files")):
        for declaration in records(constant_file.get("declarations")):
            value: dict[str, object] = as_json_object(declaration.get("value")) or {}
            logical: dict[str, object] = as_json_object(value.get("logical_type")) or {}
            kind: str | None = enum_name(logical.get("kind"))
            if kind is not None:
                kinds.add(f"constant_{kind}")
            if not json.dumps(value.get("value"), ensure_ascii=False).isascii():
                kinds.add("non_ascii_constant")
    for enum_file in records(capture.get("enum_files")):
        for declaration in records(enum_file.get("declarations")):
            members: list[dict[str, object]] = records(declaration.get("members"))
            kinds.update(
                "enum_integer" if isinstance(member.get("value"), int) else "enum_string"
                for member in members
            )
    if any(
        _CROSS_FILE_MACRO_IMPORT.search(str(record.get("contents", "")))
        for record in records(capture.get("macro_files"))
    ):
        kinds.add("cross_file_macro_import")
    return kinds


def _test_and_audit_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    for test_file in records(capture.get("test_files")):
        for block in records(test_file.get("blocks")):
            mode: str | None = enum_name(block.get("mode"))
            if mode is not None:
                kinds.add(f"test_mode_{mode}")
            if records(block.get("cases")):
                kinds.add("parameterized_test")
    for audit_file in records(capture.get("audit_files")):
        declaration_kind: str | None = enum_name(audit_file.get("declaration_kind"))
        kinds.add(
            SINGULAR_AUDIT_KIND if declaration_kind == SINGULAR_AUDIT_KIND else "generic_audit"
        )
    return kinds


def _config_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    project: dict[str, object] = as_json_object(capture.get("project_config")) or {}
    local: dict[str, object] = as_json_object(capture.get("local_config")) or {}
    if has_content(project.get("path_defaults")):
        kinds.add("path_defaults")
    if (
        has_content(local.get("vars"))
        or has_content(local.get("setting_overrides"))
        or local.get("target") is not None
    ):
        kinds.add("local_config")
    if local.get("target") is not None and mapping_size(project.get("targets")) > 1:
        kinds.add("target_override")
    dbt: dict[str, object] = as_json_object(project.get("dbt")) or {}
    if dbt.get("target_path") is not None:
        kinds.add("dbt_target_path_config")
    if project.get("adapter") not in BUILTIN_ADAPTER_IMPORTS:
        kinds.add("project_adapter_config")
    return kinds


def _text_kinds(capture: dict[str, object]) -> set[str]:
    kinds: set[str] = set()
    for collection in DISCOVERY_TEXT_FILE_COLLECTIONS:
        for record in records(capture.get(collection)):
            contents: str = str(record.get("contents", ""))
            if TAB in contents:
                kinds.add("tab")
            if contents.startswith(GENERATOR_BOM):
                kinds.add("bom")
            if _NON_ASCII_COMMENT.search(contents):
                kinds.add("non_ascii_comment")
            if _NON_ASCII_STRING.search(contents):
                kinds.add("non_ascii_string")
    return kinds
