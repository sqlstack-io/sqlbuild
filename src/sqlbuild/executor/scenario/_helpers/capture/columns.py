"""Scenario snapshot column metadata helpers."""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass
from typing import Any

from sqlbuild._native import parse_warehouse_type
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from sqlbuild.adapter.contract.models import ColumnInfo
from sqlbuild.adapter.contract.types import TypeDialect
from sqlbuild.diagnostics.main.log_debug_event import log_debug_event
from sqlbuild.errors.contracts.exceptions import ExecutorInputError
from sqlbuild.executor.scenario.constants import (
    BIGQUERY_BIGNUMERIC_TYPE,
    BIGQUERY_TIMESTAMP_TYPE,
    BIGQUERY_VARCHAR_TYPES,
    BIGQUERY_WIDEN_TO_DOUBLE_TYPES,
    DATABRICKS_ARRAY_TYPE,
    DATABRICKS_INTERVAL_PREFIX,
    DATABRICKS_MAP_ARGUMENT_COUNT,
    DATABRICKS_MAP_TYPE,
    DATABRICKS_NESTED_TYPES,
    DATABRICKS_STRUCT_FIELD_PATTERN,
    DATABRICKS_STRUCT_TYPE,
    DATABRICKS_TIMESTAMP_NTZ_TYPE,
    DATABRICKS_TIMESTAMP_TYPE,
    DATABRICKS_VARIANT_TYPE,
    DATABRICKS_VOID_TYPE,
    DUCKDB_BLOB_TYPE,
    DUCKDB_HUGEINT_TYPE,
    DUCKDB_INTEGER_TYPE,
    DUCKDB_LIST_TYPE,
    DUCKDB_SPATIAL_TYPES,
    DUCKDB_UHUGEINT_TYPE,
    FALLBACK_BINARY_TYPES,
    FALLBACK_BOOLEAN_TYPES,
    FALLBACK_DATE_TYPE,
    FALLBACK_DECIMAL_TYPES,
    FALLBACK_DOUBLE_TYPES,
    FALLBACK_FLOAT_TYPES,
    FALLBACK_JSON_TYPES,
    FALLBACK_SIGNED_INTEGER_LOCAL_TYPES,
    FALLBACK_STRUCTURED_TYPES,
    FALLBACK_TIME_TYPES,
    FALLBACK_TIMESTAMP_TOKEN,
    FALLBACK_TIMESTAMP_TYPES,
    FALLBACK_UNSIGNED_INTEGER_TYPES,
    FALLBACK_VARCHAR_TYPES,
    GENERIC_LOCAL_BASE_TYPES,
    GENERIC_WIDEN_TO_DOUBLE_TYPES,
    LOCAL_TYPE_BIGINT,
    LOCAL_TYPE_BOOLEAN,
    LOCAL_TYPE_DATE,
    LOCAL_TYPE_DECIMAL,
    LOCAL_TYPE_DIALECTS,
    LOCAL_TYPE_DOUBLE,
    LOCAL_TYPE_FLOAT,
    LOCAL_TYPE_INT,
    LOCAL_TYPE_INT128,
    LOCAL_TYPE_JSON,
    LOCAL_TYPE_REAL,
    LOCAL_TYPE_SMALLINT,
    LOCAL_TYPE_TEXT,
    LOCAL_TYPE_TIME,
    LOCAL_TYPE_TIMESTAMP,
    LOCAL_TYPE_TIMESTAMPTZ,
    LOCAL_TYPE_TIMETZ,
    LOCAL_TYPE_UINT128,
    LOCAL_TYPE_VARCHAR,
    LOCAL_TYPE_VARIANT,
    POSTGRES_BIGSERIAL_TYPE,
    POSTGRES_PARAMETERIZED_DECIMAL_TYPES,
    POSTGRES_PARAMETERIZED_TEXT_TYPES,
    POSTGRES_SERIAL_TYPES,
    POSTGRES_SMALLSERIAL_TYPE,
    POSTGRES_VARCHAR_TYPES,
    SCENARIO_LOCAL_TYPE_INVALID,
    SNOWFLAKE_DOUBLE_TYPES,
    SNOWFLAKE_NUMBER_TYPE,
    SNOWFLAKE_REAL_TYPE,
    SNOWFLAKE_TIMESTAMP_NTZ_TYPE,
    SNOWFLAKE_TIMEZONE_TIMESTAMP_TYPES,
    SNOWFLAKE_VECTOR_TYPE,
    SNOWFLAKE_WIDEN_TO_DOUBLE_TYPES,
    STRUCT_FIELD_NAME_PATTERN,
    TSQL_LOCAL_TYPES,
    TYPE_ARGUMENT_CLOSE_ANGLE,
    TYPE_ARGUMENT_CLOSE_PAREN,
    TYPE_ARGUMENT_CLOSE_TOKENS,
    TYPE_ARGUMENT_OPEN_PAREN,
    TYPE_ARGUMENT_OPEN_TOKENS,
    TYPE_ARGUMENT_SEPARATOR,
    TYPE_PATTERN_WILDCARD,
    TYPE_TREE_COLLECTIONS,
    TYPE_TREE_DECIMAL,
    TYPE_TREE_FLOAT,
    TYPE_TREE_KIND,
    TYPE_TREE_LOCAL_TYPES,
    TYPE_TREE_MAP,
    TYPE_TREE_STRUCT,
    TYPE_TREE_TEXT_KINDS,
    TYPE_TREE_TIME,
    TYPE_TREE_TIMESTAMP,
)
from sqlbuild.executor.scenario.models import ScenarioSnapshotColumn

_DEBUG_LOGGER: logging.Logger = logging.getLogger("sqlbuild.execution")


@dataclass(frozen=True)
class _TypePattern:
    base: str
    args: tuple[str, ...] = ()


_POSTGRES_DIRECT_LOCAL_TYPES: dict[str, str] = {
    "SMALLINT": LOCAL_TYPE_SMALLINT,
    "INT2": LOCAL_TYPE_SMALLINT,
    "SMALLSERIAL": LOCAL_TYPE_SMALLINT,
    "INTEGER": LOCAL_TYPE_INT,
    "INT": LOCAL_TYPE_INT,
    "INT4": LOCAL_TYPE_INT,
    "SERIAL": LOCAL_TYPE_INT,
    "BIGSERIAL": LOCAL_TYPE_BIGINT,
    "REAL": LOCAL_TYPE_REAL,
    "FLOAT4": LOCAL_TYPE_REAL,
    "TEXT": LOCAL_TYPE_TEXT,
    "UUID": "UUID",
    "INET": "INET",
    "INTERVAL": "INTERVAL",
    "TIMESTAMPTZ": LOCAL_TYPE_TIMESTAMPTZ,
    "TIMESTAMP WITH TIME ZONE": LOCAL_TYPE_TIMESTAMPTZ,
}
_POSTGRES_ARRAY_ELEMENT_LOCAL_TYPES: dict[str, str] = {
    "SMALLINT": LOCAL_TYPE_SMALLINT,
    "INT2": LOCAL_TYPE_SMALLINT,
    "INTEGER": LOCAL_TYPE_INT,
    "INT": LOCAL_TYPE_INT,
    "INT4": LOCAL_TYPE_INT,
    "BIGINT": LOCAL_TYPE_BIGINT,
    "INT8": LOCAL_TYPE_BIGINT,
    "TEXT": LOCAL_TYPE_TEXT,
    "VARCHAR": LOCAL_TYPE_VARCHAR,
    "BOOLEAN": LOCAL_TYPE_BOOLEAN,
    "BOOL": LOCAL_TYPE_BOOLEAN,
    "REAL": LOCAL_TYPE_REAL,
    "FLOAT4": LOCAL_TYPE_REAL,
    "DOUBLE PRECISION": LOCAL_TYPE_DOUBLE,
    "FLOAT8": LOCAL_TYPE_DOUBLE,
    "NUMERIC": LOCAL_TYPE_DECIMAL,
    "DECIMAL": LOCAL_TYPE_DECIMAL,
    "UUID": "UUID",
}


def build_scenario_snapshot_columns(
    *,
    adapter: BaseAdapter,
    connection: Any,
    relation_name: str,
    local_type_overrides: dict[str, str] | None = None,
) -> tuple[ScenarioSnapshotColumn, ...]:
    """Return local snapshot column metadata for a materialized relation."""

    column_infos: tuple[ColumnInfo, ...] = adapter.describe_relation(
        connection=connection,
        relation=relation_name,
    )
    return tuple(
        ScenarioSnapshotColumn(
            name=column.name,
            warehouse_type=column.type,
            local_type=local_type_for_warehouse_type(
                warehouse_type=column.type,
                sql_analysis_dialect=adapter.sql_analysis_dialect(),
                local_type_overrides=local_type_overrides,
            ),
        )
        for column in column_infos
    )


def local_type_for_warehouse_type(
    *,
    warehouse_type: str,
    sql_analysis_dialect: str | None = None,
    local_type_overrides: dict[str, str] | None = None,
) -> str:
    """Map a warehouse type string to a DuckDB-compatible local replay type."""

    override_type: str | None = _local_type_from_overrides(
        warehouse_type=warehouse_type,
        local_type_overrides=local_type_overrides or {},
    )
    if override_type is not None:
        return override_type

    dialect: TypeDialect | None = _coerce_type_dialect(sql_analysis_dialect)
    if dialect == TypeDialect.POSTGRES:
        postgres_type: str | None = _postgres_pre_local_type(warehouse_type)
        if postgres_type is not None:
            return postgres_type

    dialect_type: str | None = _dialect_pre_local_type(
        warehouse_type=warehouse_type,
        sql_analysis_dialect=sql_analysis_dialect,
    )
    if dialect_type is not None:
        return dialect_type

    parsed_local_type: str | None = _local_type_with_sqlquack(
        warehouse_type=warehouse_type,
        sql_analysis_dialect=sql_analysis_dialect,
    )
    if parsed_local_type is not None:
        return parsed_local_type

    return _fallback_local_type_for_warehouse_type(warehouse_type)


def _local_type_from_overrides(
    *, warehouse_type: str, local_type_overrides: dict[str, str]
) -> str | None:
    if not local_type_overrides:
        return None
    warehouse_pattern: _TypePattern = _parse_type_pattern(warehouse_type)
    matches: list[tuple[int, str]] = []
    pattern: str
    local_type_template: str
    for pattern, local_type_template in local_type_overrides.items():
        override_pattern: _TypePattern = _parse_type_pattern(pattern)
        specificity: int | None = _type_pattern_specificity(
            pattern=override_pattern,
            warehouse_type=warehouse_pattern,
        )
        if specificity is None:
            continue
        matches.append(
            (
                specificity,
                _render_local_type_template(
                    template=local_type_template,
                    args=warehouse_pattern.args,
                ),
            )
        )
    if not matches:
        return None
    best_specificity: int = max(specificity for specificity, _local_type in matches)
    best_local_types: set[str] = {
        local_type for specificity, local_type in matches if specificity == best_specificity
    }
    if len(best_local_types) > 1:
        raise ExecutorInputError(
            "Multiple local type override patterns match "
            f"'{warehouse_type}' with equal specificity",
            code=SCENARIO_LOCAL_TYPE_INVALID,
            help=(
                "Make one scenario local type override pattern more specific or remove the "
                "duplicate."
            ),
        )
    return best_local_types.pop()


def _postgres_pre_local_type(warehouse_type: str) -> str | None:
    pattern: _TypePattern = _parse_type_pattern(warehouse_type)
    base: str = pattern.base
    args: tuple[str, ...] = pattern.args
    if base in _POSTGRES_DIRECT_LOCAL_TYPES:
        return _POSTGRES_DIRECT_LOCAL_TYPES[base]
    if base in POSTGRES_PARAMETERIZED_TEXT_TYPES and args:
        return f"TEXT({args[0]})"
    if base in POSTGRES_PARAMETERIZED_DECIMAL_TYPES and args:
        return "DECIMAL(" + ", ".join(args) + ")"
    if base.endswith("[]"):
        element_base: str = base[:-2].strip()
        mapped: str | None = _POSTGRES_ARRAY_ELEMENT_LOCAL_TYPES.get(element_base)
        if mapped is not None:
            return f"{mapped}[]"
    return None


def _dialect_pre_local_type(*, warehouse_type: str, sql_analysis_dialect: str | None) -> str | None:
    pattern: _TypePattern = _parse_type_pattern(warehouse_type)
    base: str = pattern.base
    args: tuple[str, ...] = pattern.args
    dialect: TypeDialect | None = _coerce_type_dialect(sql_analysis_dialect)
    if dialect == TypeDialect.SNOWFLAKE:
        return _snowflake_pre_local_type(base=base, args=args)
    if dialect == TypeDialect.BIGQUERY:
        return _bigquery_pre_local_type(base=base, args=args)
    if dialect == TypeDialect.DATABRICKS:
        return _databricks_local_type(warehouse_type)
    if dialect in LOCAL_TYPE_DIALECTS:
        return _duckdb_pre_local_type(base=base, args=args)
    if dialect == TypeDialect.TSQL:
        return TSQL_LOCAL_TYPES.get(base)
    if dialect is None and base not in GENERIC_LOCAL_BASE_TYPES:
        return LOCAL_TYPE_VARCHAR
    return None


def _snowflake_pre_local_type(*, base: str, args: tuple[str, ...]) -> str | None:
    if base == SNOWFLAKE_NUMBER_TYPE:
        precision_and_scale_count: int = 2
        if len(args) >= precision_and_scale_count:
            return f"DECIMAL({args[0]}, {args[1]})"
        if len(args) == 1:
            return f"DECIMAL({args[0]})"
        return "DECIMAL(38, 0)"
    if base in SNOWFLAKE_DOUBLE_TYPES:
        return LOCAL_TYPE_DOUBLE
    if base == SNOWFLAKE_REAL_TYPE:
        return LOCAL_TYPE_REAL
    if base in SNOWFLAKE_TIMEZONE_TIMESTAMP_TYPES:
        return LOCAL_TYPE_TIMESTAMPTZ
    if base == SNOWFLAKE_TIMESTAMP_NTZ_TYPE:
        return f"TIMESTAMP({args[0]})" if args else LOCAL_TYPE_TIMESTAMP
    return None


def _bigquery_pre_local_type(*, base: str, args: tuple[str, ...]) -> str | None:
    if base == BIGQUERY_BIGNUMERIC_TYPE:
        precision_and_scale_count: int = 2
        if len(args) >= precision_and_scale_count:
            return f"DECIMAL({args[0]}, {args[1]})"
        return "DECIMAL(38, 5)"
    if base in BIGQUERY_VARCHAR_TYPES:
        return LOCAL_TYPE_VARCHAR
    if base == BIGQUERY_TIMESTAMP_TYPE:
        return LOCAL_TYPE_TIMESTAMPTZ
    return None


def _databricks_local_type(type_text: str) -> str | None:
    """Map a Databricks type, nesting included; None leaves a scalar to the fallback."""

    pattern: _TypePattern = _split_type_text(type_text.strip())
    base: str = pattern.base.upper()
    if base in DATABRICKS_NESTED_TYPES:
        return _databricks_nested_local_type(type_text=type_text.strip(), pattern=pattern)
    if base == DATABRICKS_TIMESTAMP_TYPE:
        return LOCAL_TYPE_TIMESTAMPTZ
    if base == DATABRICKS_TIMESTAMP_NTZ_TYPE:
        return LOCAL_TYPE_TIMESTAMP
    if base == DATABRICKS_VOID_TYPE:
        return LOCAL_TYPE_VARCHAR
    if base.startswith(DATABRICKS_INTERVAL_PREFIX):
        return " ".join(base.split())
    if base == DATABRICKS_VARIANT_TYPE:
        return LOCAL_TYPE_VARIANT
    return None


def _databricks_nested_local_type(*, type_text: str, pattern: _TypePattern) -> str:
    base: str = pattern.base.upper()
    args: tuple[str, ...] = pattern.args
    if base == DATABRICKS_ARRAY_TYPE and len(args) == 1:
        return _databricks_element_local_type(element=args[0], type_text=type_text) + "[]"
    if base == DATABRICKS_MAP_TYPE and len(args) == DATABRICKS_MAP_ARGUMENT_COUNT:
        key: str = _databricks_element_local_type(element=args[0], type_text=type_text)
        value: str = _databricks_element_local_type(element=args[1], type_text=type_text)
        return f"MAP({key}, {value})"
    if base == DATABRICKS_STRUCT_TYPE and args:
        fields: list[str] = []
        for field in args:
            match: re.Match[str] | None = DATABRICKS_STRUCT_FIELD_PATTERN.fullmatch(field)
            if match is None:
                raise _unmapped_databricks_type(element=field, type_text=type_text)
            name: str = match.group("name").strip("`")
            field_type: str = _databricks_element_local_type(
                element=match.group("type"), type_text=type_text
            )
            fields.append(f"{_struct_field_name(name)} {field_type}")
        return f"STRUCT({', '.join(fields)})"
    raise _unmapped_databricks_type(element=type_text, type_text=type_text)


def _databricks_element_local_type(*, element: str, type_text: str) -> str:
    local_type: str | None = _databricks_local_type(element) or _known_local_type(element)
    if local_type is None:
        raise _unmapped_databricks_type(element=element, type_text=type_text)
    return local_type


def _unmapped_databricks_type(*, element: str, type_text: str) -> ExecutorInputError:
    return ExecutorInputError(
        f"Databricks type '{type_text}' has no local DuckDB type: SQLBuild cannot map "
        f"'{element.strip()}' inside it",
        code=SCENARIO_LOCAL_TYPE_INVALID,
        help="Add a scenario local type override for this warehouse type.",
    )


def _duckdb_pre_local_type(*, base: str, args: tuple[str, ...]) -> str | None:
    if base == DUCKDB_HUGEINT_TYPE:
        return LOCAL_TYPE_INT128
    if base == DUCKDB_UHUGEINT_TYPE:
        return LOCAL_TYPE_UINT128
    if base == DUCKDB_BLOB_TYPE or base in DUCKDB_SPATIAL_TYPES:
        return LOCAL_TYPE_VARCHAR
    if base == DUCKDB_LIST_TYPE and args:
        inner_type: str = _fallback_local_type_for_warehouse_type(args[0])
        if inner_type == LOCAL_TYPE_BIGINT and args[0].strip().upper() == DUCKDB_INTEGER_TYPE:
            inner_type = LOCAL_TYPE_INT
        return inner_type + "[]"
    return None


def _local_type_with_sqlquack(
    *, warehouse_type: str, sql_analysis_dialect: str | None
) -> str | None:
    """Map a type through SQLQuack's parser for its dialect; DuckDB spellings are already local."""

    dialect: TypeDialect | None = _coerce_type_dialect(sql_analysis_dialect)
    if dialect in LOCAL_TYPE_DIALECTS:
        return warehouse_type.strip()
    try:
        tree_json: str | None = parse_warehouse_type(warehouse_type, sql_analysis_dialect or "")
    except ValueError as error:
        log_debug_event(
            logger=_DEBUG_LOGGER,
            message="scenario snapshot type parse failed; falling back",
            warehouse_type=warehouse_type,
            sql_analysis_dialect=sql_analysis_dialect,
            sqlbuild_error=str(error),
        )
        return None
    if tree_json is None:
        return None
    base_type: str = _parse_type_pattern(warehouse_type).base
    if dialect == TypeDialect.POSTGRES and base_type in POSTGRES_VARCHAR_TYPES:
        return LOCAL_TYPE_VARCHAR
    if dialect == TypeDialect.POSTGRES and base_type in POSTGRES_SERIAL_TYPES:
        return _postgres_serial_local_type(base_type)
    if dialect == TypeDialect.SNOWFLAKE and base_type == SNOWFLAKE_VECTOR_TYPE:
        return LOCAL_TYPE_JSON
    local_type: str | None = _local_type_from_tree(json.loads(tree_json))
    if local_type is None:
        return _fallback_local_type_for_warehouse_type(warehouse_type)
    if local_type == LOCAL_TYPE_REAL and _should_widen_real_to_double(
        warehouse_type=warehouse_type,
        sql_analysis_dialect=sql_analysis_dialect,
    ):
        return LOCAL_TYPE_DOUBLE
    return local_type


def _local_type_from_tree(tree: dict[str, Any]) -> str | None:
    """The DuckDB type of a SQLQuack type tree, or None when it has no fixed local type."""

    kind: str = str(tree.get(TYPE_TREE_KIND, ""))
    if kind in TYPE_TREE_LOCAL_TYPES:
        return TYPE_TREE_LOCAL_TYPES[kind]
    if kind in TYPE_TREE_TEXT_KINDS:
        length: Any = tree.get("length")
        return LOCAL_TYPE_TEXT if length is None else f"{LOCAL_TYPE_TEXT}({length})"
    if kind == TYPE_TREE_FLOAT:
        return LOCAL_TYPE_REAL if tree.get("real_spelling") else LOCAL_TYPE_FLOAT
    if kind == TYPE_TREE_DECIMAL:
        arguments: list[str] = [
            str(value) for value in (tree.get("precision"), tree.get("scale")) if value is not None
        ]
        return LOCAL_TYPE_DECIMAL + (f"({', '.join(arguments)})" if arguments else "")
    if kind == TYPE_TREE_TIME:
        return LOCAL_TYPE_TIMETZ if tree.get("timezone") else LOCAL_TYPE_TIME
    if kind == TYPE_TREE_TIMESTAMP:
        return LOCAL_TYPE_TIMESTAMPTZ if tree.get("timezone") else LOCAL_TYPE_TIMESTAMP
    if kind in TYPE_TREE_COLLECTIONS:
        element: str | None = _local_type_from_tree(tree.get("element_type") or {})
        return None if element is None else f"{element}[]"
    if kind == TYPE_TREE_STRUCT:
        fields: list[str] = []
        for field in tree.get("fields") or []:
            field_type: str | None = _local_type_from_tree(field.get("data_type") or {})
            if field_type is None:
                return None
            fields.append(f"{_struct_field_name(str(field.get('name', '')))} {field_type}")
        return f"STRUCT({', '.join(fields)})" if fields else None
    if kind == TYPE_TREE_MAP:
        key: str | None = _local_type_from_tree(tree.get("key_type") or {})
        value: str | None = _local_type_from_tree(tree.get("value_type") or {})
        return None if key is None or value is None else f"MAP({key}, {value})"
    return None


def _struct_field_name(name: str) -> str:
    if STRUCT_FIELD_NAME_PATTERN.fullmatch(name):
        return name
    return '"' + name.replace('"', '""') + '"'


def _fallback_local_type_for_warehouse_type(warehouse_type: str) -> str:
    """Return a conservative local type when SQL analysis cannot classify a type."""

    return _known_local_type(warehouse_type) or LOCAL_TYPE_VARCHAR


def _known_local_type(warehouse_type: str) -> str | None:
    """The local type of a type name SQLBuild knows by its base name alone, else None."""

    normalized_type: str = warehouse_type.strip().upper()
    base_type: str = _parse_type_pattern(normalized_type).base
    if base_type in FALLBACK_BOOLEAN_TYPES:
        return LOCAL_TYPE_BOOLEAN
    if base_type in FALLBACK_SIGNED_INTEGER_LOCAL_TYPES:
        return FALLBACK_SIGNED_INTEGER_LOCAL_TYPES[base_type]
    if base_type in FALLBACK_UNSIGNED_INTEGER_TYPES:
        return LOCAL_TYPE_BIGINT
    if base_type in FALLBACK_FLOAT_TYPES:
        return LOCAL_TYPE_FLOAT
    if base_type in FALLBACK_DOUBLE_TYPES:
        return LOCAL_TYPE_DOUBLE
    if base_type in FALLBACK_DECIMAL_TYPES:
        return _decimal_local_type(normalized_type)
    if base_type == FALLBACK_DATE_TYPE:
        return LOCAL_TYPE_DATE
    if base_type in FALLBACK_TIME_TYPES:
        return LOCAL_TYPE_TIME
    if FALLBACK_TIMESTAMP_TOKEN in base_type or base_type in FALLBACK_TIMESTAMP_TYPES:
        return LOCAL_TYPE_TIMESTAMP
    if base_type in FALLBACK_VARCHAR_TYPES:
        return LOCAL_TYPE_VARCHAR
    if base_type in FALLBACK_JSON_TYPES:
        return LOCAL_TYPE_JSON
    if base_type in FALLBACK_BINARY_TYPES:
        return LOCAL_TYPE_VARCHAR
    if base_type in FALLBACK_STRUCTURED_TYPES:
        return LOCAL_TYPE_JSON
    return None


def _postgres_serial_local_type(base_type: str) -> str:
    if base_type == POSTGRES_BIGSERIAL_TYPE:
        return LOCAL_TYPE_BIGINT
    if base_type == POSTGRES_SMALLSERIAL_TYPE:
        return LOCAL_TYPE_SMALLINT
    return LOCAL_TYPE_INT


def _parse_type_pattern(type_text: str) -> _TypePattern:
    return _split_type_text(type_text.strip().upper())


def _split_type_text(normalized_type: str) -> _TypePattern:
    if not normalized_type:
        return _TypePattern(base="")
    open_index: int = _first_type_arg_open_index(normalized_type)
    if open_index == -1:
        return _TypePattern(base=normalized_type)
    close_char: str = (
        TYPE_ARGUMENT_CLOSE_PAREN
        if normalized_type[open_index] == TYPE_ARGUMENT_OPEN_PAREN
        else TYPE_ARGUMENT_CLOSE_ANGLE
    )
    close_index: int = normalized_type.rfind(close_char)
    if close_index == -1 or close_index < open_index:
        return _TypePattern(base=normalized_type)
    base: str = normalized_type[:open_index].strip()
    args_text: str = normalized_type[open_index + 1 : close_index].strip()
    return _TypePattern(base=base, args=tuple(_split_type_args(args_text)))


def _first_type_arg_open_index(type_text: str) -> int:
    paren_index: int = type_text.find("(")
    angle_index: int = type_text.find("<")
    indexes: tuple[int, ...] = tuple(index for index in (paren_index, angle_index) if index != -1)
    if not indexes:
        return -1
    return min(indexes)


def _split_type_args(args_text: str) -> list[str]:
    if not args_text:
        return []
    args: list[str] = []
    start: int = 0
    depth: int = 0
    index: int
    character: str
    for index, character in enumerate(args_text):
        if character in TYPE_ARGUMENT_OPEN_TOKENS:
            depth += 1
        elif character in TYPE_ARGUMENT_CLOSE_TOKENS:
            depth = max(depth - 1, 0)
        elif character == TYPE_ARGUMENT_SEPARATOR and depth == 0:
            args.append(args_text[start:index].strip())
            start = index + 1
    args.append(args_text[start:].strip())
    return args


def _type_pattern_specificity(*, pattern: _TypePattern, warehouse_type: _TypePattern) -> int | None:
    if pattern.base != TYPE_PATTERN_WILDCARD and pattern.base != warehouse_type.base:
        return None
    if len(pattern.args) != len(warehouse_type.args):
        return None
    specificity: int = 0 if pattern.base == TYPE_PATTERN_WILDCARD else 100
    pattern_arg: str
    warehouse_arg: str
    for pattern_arg, warehouse_arg in zip(pattern.args, warehouse_type.args, strict=True):
        if pattern_arg == TYPE_PATTERN_WILDCARD:
            continue
        if pattern_arg != warehouse_arg:
            return None
        specificity += 10
    return specificity


def _render_local_type_template(*, template: str, args: tuple[str, ...]) -> str:
    rendered: str = template
    index: int
    arg: str
    for index, arg in enumerate(args, start=1):
        rendered = rendered.replace(f"{{{index}}}", arg)
    return rendered


def _decimal_local_type(normalized_type: str) -> str:
    """Return a DuckDB decimal type preserving precision and scale when present."""

    if TYPE_ARGUMENT_OPEN_PAREN not in normalized_type:
        return LOCAL_TYPE_DECIMAL
    precision_scale: str = normalized_type.split("(", 1)[1].split(")", 1)[0].strip()
    if not precision_scale:
        return LOCAL_TYPE_DECIMAL
    return f"DECIMAL({precision_scale})"


def _should_widen_real_to_double(*, warehouse_type: str, sql_analysis_dialect: str | None) -> bool:
    normalized_type: str = warehouse_type.strip().upper()
    base_type: str = normalized_type.split("(", 1)[0].strip()
    dialect: TypeDialect | None = _coerce_type_dialect(sql_analysis_dialect)
    if dialect == TypeDialect.SNOWFLAKE:
        return base_type in SNOWFLAKE_WIDEN_TO_DOUBLE_TYPES
    if dialect == TypeDialect.BIGQUERY:
        return base_type in BIGQUERY_WIDEN_TO_DOUBLE_TYPES
    return base_type in GENERIC_WIDEN_TO_DOUBLE_TYPES


def _coerce_type_dialect(dialect: str | None) -> TypeDialect | None:
    if dialect is None:
        return None
    try:
        return TypeDialect(dialect)
    except ValueError:
        return None
