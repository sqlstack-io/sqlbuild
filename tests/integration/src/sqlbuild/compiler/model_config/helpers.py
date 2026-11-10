"""Seeded MODEL header values and engine-parity outcomes for the native model config tests."""

from __future__ import annotations

import random
import re
from collections.abc import Callable, Iterator
from dataclasses import dataclass, fields, replace
from enum import StrEnum
from pathlib import Path
from typing import TYPE_CHECKING, ClassVar, TypeGuard, cast

import pytest

import sqlbuild._native as _native
from sqlbuild._native import (
    CompileAdapterContext,
    CompileInputError,
    DiscoveredProjectInputs,
    DiscoveredSqlModelFile,
    SchemaModelEntry,
    SourceLocation,
    build_compile_inputs,
    discover_project_inputs,
    parse_audit_instances,
    resolve_effective_collection_rendering,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter

if TYPE_CHECKING:
    from _typeshed import DataclassInstance

    from sqlbuild._native import CompileInputsView, CompileModelConfig

_TEXTS: tuple[object, ...] = (
    "INTEGER",
    "Order amount",
    "caf\u00e9 total",
    "",
    "   ",
    "\u00a0",
    "mail@example.com",
    "\u3000",
    1,
    True,
    None,
    1.5,
    ("tuple",),
)
_AUDIT_NAMES: tuple[str, ...] = (
    "not_null",
    "unique",
    "accepted_values",
    "NotNull",
    "row_count_",
    "",
    "_private",
    "a",
    "na\u00efve_check",
    "\u00dcniqueCheck",
    " \u2003unique\u00a0",
)
_COLUMN_NAMES: tuple[str, ...] = (
    "order_id",
    "Order_ID",
    "amount",
    "caf\u00e9",
    "CAF\u00c9",
    "Stra\u00dfe",
    "STRASSE",
    "\u03a3\u0391\u03a3",
    "\u03c3\u03b1\u03c2",
    "\u0130d",
    "status",
    " ",
    "\u00a0",
    "id",
)
_COLUMN_KEYS: tuple[str, ...] = ("type", "nullable", "description", "audits", "migrate_from")
_AUDIT_OPTIONS: tuple[tuple[str, tuple[object, ...]], ...] = (
    ("name", ("custom_name", "Bad Name", "", None, 1)),
    ("description", ("Checks the rows", "", None)),
    ("severity", ("warn", "error", "fatal", None, 1)),
    ("run_scope", ("final", "", None)),
    ("always_run", (True, False, None, "yes")),
    (
        "thresholds",
        (
            None,
            {"warn": {"above": 1}},
            {"warn": {"below": 5}, "error": {"below": 1}},
            {"warn": {"below": 5}, "error": {"below": 5.0}},
            {"error": {"above": 1.5}, "warn": {"above": 1}},
            {"warn": {"outside": (1, 5)}, "error": {"outside": (0, 6.5)}},
            {"warn": {"outside": (1, 5)}, "error": {"outside": (2, 6)}},
            {"warn": {"outside": (5, 1)}},
            {"warn": {"outside": [1, 2]}},
            {"warn": {"outside": (1, True)}},
            {"warn": {"above": 1}, "error": {"below": 2}},
            {"warn": {"above": True}},
            {"warn": {"above": "1"}},
            {"warn": {"above": 2**70}},
            {"warn": {"above": 1, "below": 2}},
            {"warn": {"sideways": 1}},
            {"warn": None, "error": None},
            {"warn": 1},
            {"bogus": 1, "other": 2},
            {},
            "x",
        ),
    ),
    ("minimum_samples", (0, 5, -1, True, None, 2**63 - 1)),
    ("evidence_limit", (0, 10, -3, None)),
    ("values", (["PLACED", "SHIPPED"], None, {"nested": [1, 2]})),
    ("minimum", (1, 2.5)),
)


NATIVE_MODEL_CONFIG_ENTRIES: tuple[str, ...] = ("expand_template_data",)
_NATIVE_ENTRIES: dict[str, Callable[..., object]] = {
    name: getattr(_native, name) for name in NATIVE_MODEL_CONFIG_ENTRIES
}
MODEL_CONFIG_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connections.local]\ndatabase = "orders.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n\n'
        '[targets.dev]\nconnection = "local"\n'
        "schema = \"${coalesce(ENV:SQB_MISSING_SCHEMA, 'dev')}_${CTX:model.name}\"\n"
    ),
    "models/orders.sql": (
        'MODEL (\n  materialized table,\n  description "Orders.",\n'
        "  tags [\"${if(eq(CTX:run.target, 'dev'), 'development', 'release')}\"],\n"
        "  columns (\n"
        "    order_id (type INTEGER, nullable false, audits [not_null (severity error), unique]),\n"
        '    status (audits [accepted_values (values ["placed", "shipped"])]),\n'
        "  ),\n);\n\nSELECT 1 AS order_id, 'placed' AS status\n"
    ),
    "models/customers.sql": (
        "MODEL (\n  materialized view,\n  columns (customer_id (type INTEGER)),\n);\n\n"
        "SELECT 1 AS customer_id\n"
    ),
}


LATER_VALIDATION_PATTERNS: tuple[re.Pattern[str], ...] = (
    re.compile(r"is missing argument '[^']+' for generic audit '[^']+'$"),
    re.compile(r"passes argument '[^']+', which the audit's SQL in .+ never uses; remove .+$"),
    re.compile(r"column '[^']+' migrate_from must name one column of the model's existing table$"),
    re.compile(
        r"audit '[^']+': thresholds and minimum_samples are only valid for measurement audits$"
    ),
)
"""Validations the loop runs after parsing a header, which the YAML schema parsers never run."""
_PARITY_PROJECT: str = 'name = "orders"\nadapter = "duckdb"\n'
_PARITY_FILES: dict[str, str] = {
    "models/orders.sql": (
        "MODEL (description 'Orders.', materialized incremental, incremental_strategy append);\n"
        "SELECT 1 AS order_id\n"
    ),
    **{
        f"audits/generic/{name}.sql": 'AUDIT ();\nSELECT * FROM __ref("@model") WHERE FALSE\n'
        for name in ("a",)
    },
}


@dataclass(frozen=True)
class HeaderMetadataParity:
    """How the model loop's header attachment compared with the YAML parsers over one corpus;
    the loop's later validation may reject a header the parsers accept."""

    mismatches: list[tuple[object, object, object]]
    parsed: int
    rejected: int
    later_validated: int


def generated_header_metadata(*, rng: random.Random, count: int) -> list[tuple[object, object]]:
    """Return seeded `(columns, audits)` header values mixing valid and invalid shapes."""

    return [(_columns(rng=rng), _audit_list(rng=rng)) for _ in range(count)]


def model_config_outcome(
    *, project_dir: Path, monkeypatch: pytest.MonkeyPatch
) -> tuple[dict[str, int], str]:
    """Build compile inputs; count native entry calls and describe model config."""

    for relative_path, contents in MODEL_CONFIG_PROJECT.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
    calls: dict[str, int] = dict.fromkeys(NATIVE_MODEL_CONFIG_ENTRIES, 0)
    for name in NATIVE_MODEL_CONFIG_ENTRIES:
        monkeypatch.setattr(_native, name, _counted(calls=calls, name=name))
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    adapter: DuckDbAdapter = DuckDbAdapter()
    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=CompileAdapterContext(
            value_renderer=adapter,
            collection_rendering=resolve_effective_collection_rendering(
                project_config=discovered.project_config, declaration_override=None
            ),
            python_functions_inherit_default_namespace=(
                adapter.python_functions_inherit_default_namespace()
            ),
            sql_lexical_syntax=adapter.sql_lexical_syntax,
            sql_analysis_dialect=adapter.sql_analysis_dialect_name,
        ),
        run_id="20261007T000000Z_orders",
        resolved_connection={},
        no_sql_validation=True,
        defer_model_sql_validation=True,
        no_cache=True,
    )
    return calls, repr(
        [(model.config, model.schema_entry, model.enum_columns) for model in inputs.model_inputs]
    )


def error_shape(error: Exception) -> tuple[object, ...]:
    """Return what a user sees of an error: its type, message, code and help."""

    return (
        type(error).__name__,
        str(error),
        getattr(error, "code", None),
        getattr(error, "help", None),
    )


def _counted(*, calls: dict[str, int], name: str) -> Callable[..., object]:
    entry: Callable[..., object] = _NATIVE_ENTRIES[name]

    def counted(*args: object) -> object:
        calls[name] += 1
        return entry(*args)

    return counted


def header_metadata_parity(
    *, project_dir: Path, headers: list[tuple[object, object]]
) -> HeaderMetadataParity:
    """Compare each header the model loop attaches or rejects with the YAML schema parsers."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(_PARITY_PROJECT, encoding="utf-8")
    for relative_path, contents in _PARITY_FILES.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    model_files: list[DiscoveredSqlModelFile] = [
        _model_file(base=discovered.model_files[0], index=index, columns=columns, audits=audits)
        for index, (columns, audits) in enumerate(headers)
    ]
    actual: list[object] = [
        _loop_shape(discovered=discovered, model_file=model_file) for model_file in model_files
    ]
    expected: list[object] = [
        _shape(_python_metadata(model_file=model_file)) for model_file in model_files
    ]
    outcomes: list[tuple[object, object, object]] = list(
        zip([model_file.header_values for model_file in model_files], expected, actual, strict=True)
    )
    rejected: int = sum(isinstance(shape, list) for shape in actual)
    return HeaderMetadataParity(
        mismatches=list(filter(_unexplained, outcomes))[:3],
        parsed=len(actual) - rejected,
        rejected=rejected,
        later_validated=sum(
            _later_validation(expected=expected, actual=shape) for _, expected, shape in outcomes
        ),
    )


def _unexplained(outcome: tuple[object, object, object]) -> bool:
    """A differing outcome other than a parsed header a known later loop validation rejects."""

    _, expected, actual = outcome
    return expected != actual and not _later_validation(expected=expected, actual=actual)


def _later_validation(*, expected: object, actual: object) -> bool:
    """Whether the parsers accepted the header and the loop rejected it with a known later check."""

    return (
        isinstance(expected, tuple)
        and isinstance(actual, list)
        and any(pattern.search(str(actual[1])) for pattern in LATER_VALIDATION_PATTERNS)
    )


def _loop_shape(
    *, discovered: DiscoveredProjectInputs, model_file: DiscoveredSqlModelFile
) -> object:
    """The model loop's attached columns and audits, or the error it raised."""

    try:
        inputs: CompileInputsView = build_compile_inputs(
            discovered_inputs=replace(discovered, model_files=(model_file,)),
            adapter_context=CompileAdapterContext(
                value_renderer=DuckDbAdapter(),
                collection_rendering=resolve_effective_collection_rendering(
                    project_config=discovered.project_config, declaration_override=None
                ),
                python_functions_inherit_default_namespace=True,
                sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
                sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
            ),
            no_sql_validation=True,
            defer_model_sql_validation=True,
            no_cache=True,
        )
    except Exception as error:  # noqa: BLE001 - the exact loop outcome, whatever it is
        return list(error_shape(error)[:4])
    entry: SchemaModelEntry | None = inputs.model_inputs[0].schema_entry
    return _shape(_entry_metadata(entry))


def _entry_metadata(entry: SchemaModelEntry | None) -> tuple[object, object]:
    entries: Iterator[SchemaModelEntry] = filter(_attached, (entry,))
    return next(((item.columns, item.audits) for item in entries), ((), ()))


def _attached(entry: SchemaModelEntry | None) -> TypeGuard[SchemaModelEntry]:
    return entry is not None


def _python_metadata(*, model_file: DiscoveredSqlModelFile) -> object:
    try:
        return (
            _native.parse_schema_columns(
                model_file.header_values.get("columns"),
                (str(model_file.relative_path), "model"),
                (False, True, False),
                model_file.header_column_locations or {},
            ),
            parse_audit_instances(
                raw_audits=model_file.header_values.get("audits"),
                file_path=model_file.relative_path,
                label="model",
                error_class=CompileInputError,
                null_as_empty=True,
            ),
        )
    except Exception as error:  # noqa: BLE001 - the exact Python outcome, whatever it is
        return list(error_shape(error)[:4])


def _shape(value: object) -> object:
    return _SHAPES.get(type(value), _unchanged)(value)


def _unchanged(value: object) -> object:
    return value


def _metadata_shape(value: object) -> object:
    return (repr(value), _attribute_orders(value), _locations(value))


_SHAPES: dict[type, Callable[[object], object]] = {tuple: _metadata_shape}


def _attribute_orders(value: object) -> object:
    return _item_orders(_flattened(value))


def _field_names(item: object) -> tuple[str, ...]:
    return tuple(field.name for field in fields(cast("DataclassInstance", item)))


def _item_orders(items: tuple[object, ...]) -> object:
    return tuple(
        (
            type(item).__name__,
            _field_names(item),
            _item_orders(getattr(item, "audits", ())),
        )
        for item in items
    )


def _locations(value: object) -> object:
    return tuple(_item_locations(item) for item in _flattened(value))


def _item_locations(item: object) -> object:
    audit_locations: tuple[object, ...] = tuple(
        audit.location for audit in getattr(item, "audits", ())
    )
    return (getattr(item, "location", None), audit_locations)


def _flattened(value: object) -> tuple[object, ...]:
    columns, audits = cast(tuple[tuple[object, ...], tuple[object, ...]], value)
    return (*columns, *audits)


def _model_file(
    *, base: DiscoveredSqlModelFile, index: int, columns: object, audits: object
) -> DiscoveredSqlModelFile:
    names: list[str] = [*filter(_is_text, getattr(columns, "keys", tuple)())]
    locations: dict[str, SourceLocation] = {
        name: SourceLocation(path=base.relative_path, line=line + 2, column=3)
        for line, name in enumerate(names[: len(names) * (index % 2)])
    }
    return DiscoveredSqlModelFile(
        file_path=base.file_path,
        relative_path=base.relative_path,
        contents=base.contents,
        header_values={**base.header_values, "columns": columns, "audits": audits},
        header_column_locations=locations,
        output_column_locations=base.output_column_locations,
        query_sql=base.query_sql,
        enum_declarations=base.enum_declarations,
        constant_declarations=base.constant_declarations,
        extract_implicit_alias_columns=base.extract_implicit_alias_columns,
    )


def _is_text(value: object) -> bool:
    return isinstance(value, str)


def _columns(*, rng: random.Random) -> object:
    return rng.choices(
        (None, rng.choice(([], "columns", 1, ("order_id",))), _column_mapping(rng=rng)),
        weights=(8, 4, 88),
    )[0]


def _column_mapping(*, rng: random.Random) -> dict[object, object]:
    return {
        rng.choices((rng.choice(_COLUMN_NAMES), rng.choice((1, None))), weights=(9, 1))[0]: (
            _column(rng=rng)
        )
        for _ in range(rng.randint(0, 5))
    }


def _column(*, rng: random.Random) -> object:
    keys: list[str] = rng.sample(_COLUMN_KEYS, k=rng.randint(0, len(_COLUMN_KEYS)))
    column: dict[object, object] = {key: _COLUMN_VALUES[key](rng) for key in keys}
    extra_keys: tuple[object, ...] = rng.choices(
        ((), (rng.choice(("format", "zone")),)), weights=(19, 1)
    )[0]
    column.update(dict.fromkeys(extra_keys, "x"))
    return rng.choices((column, rng.choice((None, "INTEGER", []))), weights=(19, 1))[0]


def _text(rng: random.Random) -> object:
    return rng.choice(_TEXTS)


def _nullable(rng: random.Random) -> object:
    return rng.choice((True, False, None, "yes"))


def _audits(rng: random.Random) -> object:
    return _audit_list(rng=rng)


_COLUMN_VALUES: dict[str, Callable[[random.Random], object]] = {
    "type": _text,
    "nullable": _nullable,
    "description": _text,
    "audits": _audits,
    "migrate_from": _text,
}


def _audit_list(*, rng: random.Random) -> object:
    return rng.choices(
        (
            None,
            rng.choice(("not_null", {"unique": None}, ("unique",))),
            [_audit(rng=rng) for _ in range(rng.randint(0, 3))],
        ),
        weights=(30, 4, 66),
    )[0]


def _audit(*, rng: random.Random) -> object:
    name: str = rng.choice(_AUDIT_NAMES)
    options: list[tuple[str, tuple[object, ...]]] = rng.sample(_AUDIT_OPTIONS, k=rng.randint(0, 4))
    return rng.choices(
        (
            name,
            {name: None},
            rng.choice(({name: None, "unique": None}, {1: None}, {name: "x"}, 3)),
            {name: {key: rng.choice(choices) for key, choices in options}},
        ),
        weights=(35, 10, 5, 50),
    )[0]


def column_metadata_outcome(*, project_dir: Path, header_column: str) -> tuple[object, object]:
    """Compile a one-model project whose header declares `header_column`; return the attached
    column's type, or the error the loop raised."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "orders"\nadapter = "duckdb"\n\n[vars]\ncolumn_type = "BIGINT"\n'
        ),
        "macros/python/amounts.py": (
            'def cents(column: str) -> str:\n    return f"{column} * 100"\n'
        ),
        "models/orders.sql": (
            f"MODEL (description 'Orders.', columns ({header_column}));\nSELECT 1 AS amount\n"
        ),
    }
    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_text(contents, encoding="utf-8")
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    try:
        inputs: CompileInputsView = build_compile_inputs(
            discovered_inputs=discovered,
            adapter_context=CompileAdapterContext(
                value_renderer=DuckDbAdapter(),
                collection_rendering=resolve_effective_collection_rendering(
                    project_config=discovered.project_config, declaration_override=None
                ),
                python_functions_inherit_default_namespace=True,
                sql_lexical_syntax=DuckDbAdapter.sql_lexical_syntax,
                sql_analysis_dialect=DuckDbAdapter.sql_analysis_dialect_name,
            ),
            no_sql_validation=True,
            defer_model_sql_validation=True,
            no_cache=True,
        )
    except Exception as error:  # noqa: BLE001 - the exact loop outcome, whatever it is
        return None, str(error)
    columns: tuple[object, ...] = cast(
        tuple[object, ...], _entry_metadata(inputs.model_inputs[0].schema_entry)[0]
    )
    return getattr(columns[0], "type", None), None


CONFIG_VALUE_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connections.local]\ndatabase = "orders.duckdb"\n\n'
        '[vars]\nregion = "north"\nratio = "0.5"\n\n'
        '[defaults]\nmaterialized = "table"\ncontract = "enforced"\ntags = ["core"]\n\n'
        '[path_defaults."marts"]\nrow_diff_exclude_columns = ["loaded_at"]\ntags = ["mart"]\n\n'
        '[targets.dev]\nconnection = "local"\nschema = "${region}_${CTX:model.name}"\n'
    ),
    "models/marts/orders.sql": 'MODEL (description "Orders.");\n\nSELECT 1 AS order_id\n',
}


class Severity(StrEnum):
    """A string enum whose members config values and keys hold as themselves."""

    WARN = "warn"


class TagList(list[object]):
    """A list subclass a Python caller may pass as a config value."""


class SettingMap(dict[str, object]):
    """A dict subclass a Python caller may pass as a config value."""


class Marker:
    """An opaque object the build carries through without reading it."""

    def __repr__(self) -> str:
        return "Marker()"


MARKER: Marker = Marker()


def config_value_inputs(
    *, project_dir: Path, header: dict[object, object], cli_vars: dict[str, object] | None = None
) -> CompileInputsView:
    """Build compile inputs for the config value project with its one model's header replaced."""

    for relative_path, contents in CONFIG_VALUE_PROJECT.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    model_file: DiscoveredSqlModelFile = discovered.model_files[0]
    given: DiscoveredSqlModelFile = DiscoveredSqlModelFile(
        file_path=model_file.file_path,
        relative_path=model_file.relative_path,
        contents=model_file.contents,
        header_values=cast("dict[str, object]", header),
        header_column_locations={},
        output_column_locations={},
        query_sql=model_file.query_sql,
    )
    adapter: DuckDbAdapter = DuckDbAdapter()
    return build_compile_inputs(
        discovered_inputs=replace(discovered, model_files=(given,)),
        adapter_context=CompileAdapterContext(
            value_renderer=adapter,
            collection_rendering=resolve_effective_collection_rendering(
                project_config=discovered.project_config, declaration_override=None
            ),
            python_functions_inherit_default_namespace=(
                adapter.python_functions_inherit_default_namespace()
            ),
            sql_lexical_syntax=adapter.sql_lexical_syntax,
            sql_analysis_dialect=adapter.sql_analysis_dialect_name,
        ),
        cli_vars=cli_vars,
        run_id="20261007T000000Z_orders",
        resolved_connection={},
        no_sql_validation=True,
        defer_model_sql_validation=True,
        no_cache=True,
    )


def config_value_outcome(
    *, project_dir: Path, header: dict[object, object]
) -> tuple[tuple[tuple[str, str, str], ...], tuple[str, ...]]:
    """The model's config values as `(key repr, value type, value repr)` and its header keys."""

    config: CompileModelConfig = (
        config_value_inputs(project_dir=project_dir, header=header).model_inputs[0].config
    )
    return (
        tuple(
            (repr(key), type(value).__name__, repr(value)) for key, value in config.values.items()
        ),
        config.model_header_keys,
    )


def marker_kept(*, project_dir: Path, header: dict[object, object], key: object) -> bool:
    """Whether the opaque value under `key` reaches the model's config values as the same object."""

    config: CompileModelConfig = (
        config_value_inputs(project_dir=project_dir, header=header).model_inputs[0].config
    )
    return config.values[cast("str", key)] is MARKER


def header_column_paths(*, project_dir: Path, headers: tuple[str, ...]) -> tuple[int, int]:
    """How many models reuse their natively parsed header columns, and how many reparse them."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": CONFIG_VALUE_PROJECT["sqlbuild_project.toml"],
        **{
            f"models/marts/orders_{index}.sql": f"MODEL ({header});\n\nSELECT 1 AS order_id\n"
            for index, header in enumerate(headers)
        },
    }
    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=project_dir)
    adapter: DuckDbAdapter = DuckDbAdapter()
    _ = _native.compile_stage_work_info(reset=True)
    _ = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=CompileAdapterContext(
            value_renderer=adapter,
            collection_rendering=resolve_effective_collection_rendering(
                project_config=discovered.project_config, declaration_override=None
            ),
            python_functions_inherit_default_namespace=(
                adapter.python_functions_inherit_default_namespace()
            ),
            sql_lexical_syntax=adapter.sql_lexical_syntax,
            sql_analysis_dialect=adapter.sql_analysis_dialect_name,
        ),
        run_id="20261007T000000Z_orders",
        resolved_connection={},
        no_sql_validation=True,
        defer_model_sql_validation=True,
        no_cache=True,
    )
    work: dict[str, int] = _native.compile_stage_work_info(reset=True)
    return (work.get("native_header_columns", 0), work.get("parsed_header_columns", 0))


class Exploding:
    """An opaque value whose `str()` and `repr()` raise and count each call."""

    calls: ClassVar[list[str]] = []

    def __str__(self) -> str:
        Exploding.calls.append("str")
        raise ValueError("no str")

    def __repr__(self) -> str:
        Exploding.calls.append("repr")
        raise ValueError("no repr")


class ExplodingText(str):
    """A string whose `str()` raises, which config unions print."""

    __slots__ = ()

    def __str__(self) -> str:
        raise ValueError("no str")


def templated_alias(
    *, project_dir: Path, header: dict[object, object], cli_vars: dict[str, object]
) -> tuple[str, str]:
    """The model's built alias as `("alias", text)`, or the raised error as `(type, message)`."""

    try:
        config: CompileModelConfig = (
            config_value_inputs(project_dir=project_dir, header=header, cli_vars=cli_vars)
            .model_inputs[0]
            .config
        )
    except Exception as error:
        return (type(error).__name__, str(error))
    return ("alias", str(config.values.get("alias")))


def opaque_reads(*, project_dir: Path) -> tuple[bool, tuple[str, ...]]:
    """Whether an opaque header value reaches the config, and which of its texts compiling read."""

    Exploding.calls.clear()
    marker: Exploding = Exploding()
    config: CompileModelConfig = (
        config_value_inputs(
            project_dir=project_dir,
            header={"description": "x", "alias": "${region}", Severity.WARN: marker},
        )
        .model_inputs[0]
        .config
    )
    return (config.values[cast("str", Severity.WARN)] is marker, tuple(Exploding.calls))
