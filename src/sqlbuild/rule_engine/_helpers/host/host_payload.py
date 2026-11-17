"""Compiler facts a custom-rule host decodes lazily from its native payload."""

from __future__ import annotations

import json
import pickle
from functools import cached_property
from pathlib import Path
from typing import Any

from sqlbuild._native import (
    CUSTOM_HOST_PAYLOAD_MAGIC,
    CompiledSqlTest,
    ConstantDeclaration,
    EnumDeclaration,
    InferredColumn,
    InferredNullability,
    RulesConfig,
    RulesError,
    custom_host_declared_columns,
)
from sqlbuild.compiler.compile.types import (
    CompiledAuditView,
    CompiledFunctionView,
    CompiledSeedView,
    CompiledSourceView,
)
from sqlbuild.rule_engine.types import RuleFactProject

_TRAILER_SIZE: int = 8
_PICKLE_TAG: str = "p"
_TEXT_TAG: str = "s"
_TEXTS_TAG: str = "t"
_INFERRED_TAG: str = "r"
_COLUMNS_TAG: str = "c"


class _Payload:
    """The payload bytes and the values its header names by tag, offset and length."""

    __slots__ = ("_data",)

    def __init__(self, data: bytes) -> None:
        self._data: bytes = data

    def value(self, encoded: Any) -> Any:
        if not isinstance(encoded, list):
            return encoded
        tag: str = encoded[0]
        if tag == _TEXTS_TAG:
            return tuple(encoded[1])
        start: int = encoded[1]
        part: memoryview = memoryview(self._data)[start : start + encoded[2]]
        if tag == _PICKLE_TAG:
            return pickle.loads(part)
        if tag == _TEXT_TAG:
            return str(part, "utf-8")
        if tag == _INFERRED_TAG:
            return tuple(
                InferredColumn(
                    name=name, type=column_type, nullability=InferredNullability(nullability)
                )
                for name, column_type, nullability in json.loads(bytes(part))
            )
        if tag == _COLUMNS_TAG:
            return custom_host_declared_columns(bytes(part))
        raise RulesError(f"custom host project payload has an unknown part {tag!r}")


class _HostModel:
    """One compiled model's rule facts, each decoded on first read."""

    def __init__(self, *, payload: _Payload, entry: dict[str, Any]) -> None:
        self._payload: _Payload = payload
        self._entry: dict[str, Any] = entry
        self._config: dict[str, object] = {}

    @cached_property
    def name(self) -> str:
        return self._entry["name"]

    @cached_property
    def relative_path(self) -> Path:
        path: object = self._entry["path"]
        return Path(path) if isinstance(path, str) else self._payload.value(path)

    @cached_property
    def key(self) -> tuple[str, str]:
        resource_type, name = self._entry["key"]
        return (resource_type, name)

    @cached_property
    def deps(self) -> tuple[tuple[str, str], ...]:
        return tuple((resource_type, name) for resource_type, name in self._entry["deps"])

    @cached_property
    def authored_sql(self) -> str:
        return self._payload.value(self._entry["authored"])

    @cached_property
    def query_sql(self) -> str:
        return self._payload.value(self._entry["query"])

    @cached_property
    def query_place(self) -> str | None:
        return self._payload.value(self._entry["place"])

    def config_value(self, name: str) -> object:
        """Return `config.values.get(name)` for one config value rule facts read."""

        if name not in self._config:
            config: dict[str, Any] = self._entry["config"]
            self._config[name] = self._payload.value(config[name]) if name in config else None
        return self._config[name]

    @cached_property
    def declared_columns(self) -> tuple[object, ...]:
        encoded: object = self._entry["declared"]
        return () if encoded is None else self._payload.value(encoded)

    @cached_property
    def inferred_columns(self) -> tuple[InferredColumn, ...] | None:
        return self._payload.value(self._entry["inferred"])

    @cached_property
    def enum_declarations(self) -> tuple[EnumDeclaration, ...]:
        encoded: object = self._entry["enums"]
        return () if encoded is None else self._payload.value(encoded)

    @cached_property
    def constant_declarations(self) -> tuple[ConstantDeclaration, ...]:
        encoded: object = self._entry["constants"]
        return () if encoded is None else self._payload.value(encoded)

    @cached_property
    def enum_type_names(self) -> dict[str, str]:
        return {column: enum_name for column, enum_name in self._entry["enum_types"]}


class _HostProject:
    """One compiled project's rule facts, each collection decoded on first read."""

    def __init__(self, *, payload: _Payload, header: dict[str, Any]) -> None:
        self._payload: _Payload = payload
        self._header: dict[str, Any] = header
        self.models: tuple[_HostModel, ...] = tuple(
            _HostModel(payload=payload, entry=entry) for entry in header["models"]
        )
        self.effective_target_name: str | None = payload.value(header["target"])

    def _section(self, name: str) -> Any:
        return self._payload.value(self._header["sections"][name])

    @cached_property
    def sources(self) -> tuple[CompiledSourceView, ...]:
        return self._section("sources")

    @cached_property
    def seeds(self) -> tuple[CompiledSeedView, ...]:
        return self._section("seeds")

    @cached_property
    def functions(self) -> tuple[CompiledFunctionView, ...]:
        return self._section("functions")

    @cached_property
    def audits(self) -> tuple[CompiledAuditView, ...]:
        return self._section("audits")

    @cached_property
    def sql_tests(self) -> tuple[CompiledSqlTest, ...]:
        return self._section("sql_tests")

    @cached_property
    def public_enums(self) -> dict[str, EnumDeclaration]:
        return self._section("public_enums")

    @cached_property
    def public_constants(self) -> dict[str, ConstantDeclaration]:
        return self._section("public_constants")


def decode_host_payload(data: bytes) -> tuple[RuleFactProject, RulesConfig]:
    """Read the header of one payload; every fact is decoded only when a rule reads it."""

    magic: bytes = CUSTOM_HOST_PAYLOAD_MAGIC
    if len(data) < len(magic) + _TRAILER_SIZE or not data.startswith(magic):
        raise RulesError("custom host project payload is not a payload")
    header_start: int = int.from_bytes(data[-_TRAILER_SIZE:], "little")
    header: object = json.loads(data[header_start:-_TRAILER_SIZE])
    if not isinstance(header, dict):
        raise RulesError("custom host project payload header is invalid")
    payload: _Payload = _Payload(data)
    config: object = payload.value(header["config"])
    if not isinstance(config, RulesConfig):
        raise RulesError("custom host project payload has invalid types")
    return _HostProject(payload=payload, header=header), config
