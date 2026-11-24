"""Seeded YAML and TOML generators and canonical values for the native configuration oracles."""

from __future__ import annotations

import csv
import dataclasses
import datetime
import functools
import itertools
import json
import math
import random
import struct
import tomllib
from collections import Counter
from collections.abc import Callable
from contextlib import suppress
from itertools import compress
from pathlib import Path
from typing import Any, cast

from sqlbuild import _native
from sqlbuild._native import (
    AuthoredSqlSet,
    AuthoredSqlValueCall,
    DiscoveredProjectInputs,
    DiscoveredSourceFile,
    DiscoveredSqlModelFile,
    DiscoveryFileFault,
    NamedSqlHookEntry,
    PythonHookEntry,
    SqlHookEntry,
    TolerantScopeDiscovery,
    discover_audit_files,
    discover_constant_files,
    discover_enum_files,
    discover_model_description_inputs,
    discover_model_schema_files,
    discover_project_inputs,
    discover_scope_snapshot,
    discover_sql_function_files,
    discover_sql_hook_files,
    render_stage_capture,
)
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    GeneratedDeclarationFileTestCase,
)
from tests.integration.src.sqlbuild.compiler.helpers import random_float, random_text

SQL_AUDIT_HEADER_KEYS: frozenset[str] = frozenset(
    {
        "always_run",
        "evaluation",
        "minimum_samples",
        "name",
        "run_scope",
        "sample_count",
        "sample_unit",
        "severity",
        "sql_analysis",
        "thresholds",
        "value",
    }
)
SQL_FUNCTION_HEADER_KEYS: frozenset[str] = frozenset(
    {"arguments", "database", "description", "returns", "schema", "tags"}
)
SQL_HOOK_HEADER_KEYS: frozenset[str] = frozenset({"description"})

REPOSITORY_ROOT: Path = Path(__file__).resolve().parents[6]
EXCLUDED_DIRECTORIES: frozenset[str] = frozenset({".venv", "node_modules", "target", ".git"})
PYTHON_ERROR: dict[str, str] = {"error": "rejected"}
DEFERRED: dict[str, str] = {"error": "deferred to Python"}
UNCHECKED_PYTHON_OUTCOMES: tuple[dict[str, str], ...] = (PYTHON_ERROR,)
UNDECODABLE_BYTES: tuple[int, ...] = (
    0x80, 0x9F, 0xA0, 0xBF, 0xC0, 0xC1, 0xC2, 0xE0, 0xE2, 0xED, 0xF0, 0xF4, 0xF5, 0xFF,
)  # fmt: skip
YAML_LINE_BREAKS: dict[int, None] = {0x85: None, 0x2028: None, 0x2029: None}
BYTE_ORDER_MARKS: tuple[str, ...] = ("",) * 19 + ("\ufeff",)


def yaml_text(*, rng: random.Random, max_length: int) -> str:
    """Return random text without NEL, LINE SEPARATOR and PARAGRAPH SEPARATOR line breaks."""

    return random_text(rng=rng, max_length=max_length).translate(YAML_LINE_BREAKS)


def _alias_chain(links: int) -> str:
    return "a0: &a0 [x]\n" + "".join(
        f"a{link}: &a{link} [*a{link - 1}]\n" for link in range(1, links)
    )


def _billion_laughs(levels: int) -> str:
    return 'a0: &a0 "lol"\n' + "".join(
        f"a{level}: &a{level} [{', '.join([f'*a{level - 1}'] * 10)}]\n"
        for level in range(1, levels)
    )


def _merge_chain(levels: int) -> str:
    return "m0: &m0 {k0: 1}\n" + "".join(
        f"m{level}: &m{level} {{<<: [*m{level - 1}, *m{level - 1}], k{level}: 1}}\n"
        for level in range(1, levels)
    )


HOSTILE_YAML_DOCUMENTS: dict[str, Callable[[], str]] = {
    "alias chain": lambda: _alias_chain(10_000),
    "billion laughs": lambda: _billion_laughs(12),
    "merge chain": lambda: _merge_chain(30),
    "deep flow": lambda: "a: " + "[" * 100_000 + "]" * 100_000,
    "deep block": lambda: "- " * 100_000,
    "long integer": lambda: "a: " + "9" * 4_301,
}
LARGE_YAML_DOCUMENTS: dict[str, Callable[[], str]] = {
    "flow mapping": lambda: "{" + ", ".join(f"k{index}: {index}" for index in range(100_000)) + "}",
    "flow sequence": lambda: "[" + ",".join("1" for _ in range(200_000)) + "]",
    "anchored flow sequence": lambda: (
        "[" + ", ".join(f"&a{index} x{index}" for index in range(50_000)) + "]"
    ),
}
HOSTILE_TOML_DOCUMENTS: dict[str, Callable[[], str]] = {
    "deep arrays": lambda: "a = " + "[" * 10_000 + "]" * 10_000,
    "deep inline tables": lambda: "a = " + "{b = " * 10_000 + "}" * 10_000,
    "long dotted key": lambda: "a" + ".a" * 100_000 + " = 1",
    "long table header": lambda: "[a" + ".a" * 100_000 + "]",
}


def _toml_key(rng: random.Random, index: int) -> str:
    bare: str = f"k{index}_{rng.choice(('a', 'B', '1', '-', '_'))}"
    quoted: str = json.dumps(yaml_text(rng=rng, max_length=5) + str(index), ensure_ascii=False)
    literal: str = f"'{rng.choice(('é', 'x y', 'dot.ted'))}{index}'"
    return rng.choice((bare, bare, quoted, literal))


def _toml_datetime(rng: random.Random) -> str:
    date: str = f"{rng.randint(1, 9999):04d}-{rng.randint(1, 12):02d}-{rng.randint(1, 28):02d}"
    time: str = f"{rng.randint(0, 23):02d}:{rng.randint(0, 59):02d}:{rng.randint(0, 59):02d}"
    fraction: str = rng.choice(("", f".{rng.randint(0, 10**9)}", ".5"))
    offset: str = rng.choice(
        ("", "Z", "z", f"{rng.choice('+-')}{rng.randint(0, 23):02d}:{rng.randint(0, 59):02d}")
    )
    return rng.choice(
        (date, f"{time}{fraction}", f"{date}T{time}{fraction}{offset}", f"{date} {time}{offset}")
    )


def _toml_float(rng: random.Random) -> str:
    value: float = random_float(rng=rng)
    exponent: str = f"{rng.randint(-99, 99)}.{rng.randint(0, 99)}e{rng.randint(-30, 30)}"
    return rng.choice((repr(value), exponent, "1_000.5", "-0.0", "+inf", "-nan"))


TOML_ESCAPES: tuple[str, ...] = (
    "\\b", "\\t", "\\n", "\\f", "\\r", '\\"', "\\\\", "\\u00e9", "\\U0001F600",
)  # fmt: skip
TOML_STRING_LITERALS: tuple[str, ...] = (
    "'C:\\path'",
    "'a \"q\"'",
    "'é'",
    '"""\nline"""',
    '"""\none\\\n   two"""',
    "'''\nraw \\n'''",
    "'''\ntwo\nlines'''",
)
TOML_1_1_SYNTAX: tuple[str, ...] = (
    '"\\e"',
    '"\\x41"',
    "{x = 1,}",
    "{\n  x = 1\n}",
    "{x = 1, # note\n}",
    "07:32",
    "1979-05-27T07:32Z",
    "1979-05-27 07:32",
)
TOML_TABLE_NAMES: tuple[str, ...] = ("p", "p.q", "p.q.r", "p.s", "t", "t.u", '"p".v')
TOML_SCALARS: tuple[Callable[[random.Random], str], ...] = (
    lambda rng: rng.choice(TOML_1_1_SYNTAX),
    lambda rng: str(rng.randint(-(2**63), 2**63 - 1)),
    lambda rng: rng.choice(("0x1F", "0o17", "0b101", "1_000", "+7", "-0")),
    _toml_float,
    lambda rng: rng.choice(("true", "false")),
    lambda rng: json.dumps(yaml_text(rng=rng, max_length=10), ensure_ascii=False),
    lambda rng: '"' + "".join(rng.choice(TOML_ESCAPES) for _ in range(rng.randint(0, 3))) + 'x"',
    lambda rng: rng.choice(TOML_STRING_LITERALS),
    _toml_datetime,
)


def _toml_inline(rng: random.Random, depth: int) -> str:
    entries: str = ", ".join(
        f"{_toml_key(rng, index)} = {random_toml_value(rng=rng, depth=depth - 1)}"
        for index in range(rng.randint(0, 3))
    )
    return "{" + entries + "}"


def _toml_array(rng: random.Random, depth: int) -> str:
    values: list[str] = [
        random_toml_value(rng=rng, depth=depth - 1) for _ in range(rng.randint(0, 3))
    ]
    return "[" + rng.choice((", ", ",\n  ")).join(values) + rng.choice(("", ",")) + "]"


TOML_GENERATORS: tuple[Callable[[random.Random, int], str], ...] = (
    *(lambda rng, depth, scalar=scalar: scalar(rng) for scalar in TOML_SCALARS),
    _toml_inline,
    _toml_array,
)


def random_toml_value(*, rng: random.Random, depth: int) -> str:
    """Return the TOML text of a random value."""

    available: int = len(TOML_SCALARS) + 2 * min(depth, 1)
    return rng.choice(TOML_GENERATORS[:available])(rng, depth)


def _toml_pairs(rng: random.Random, prefix: str) -> str:
    return "".join(
        f"{prefix}{_toml_key(rng, index)} = {random_toml_value(rng=rng, depth=2)}"
        f"{rng.choice(('', '  # note'))}\n"
        for index in range(rng.randint(0, 4))
    )


def toml_document(*, rng: random.Random) -> str:
    """Return a TOML document with root keys, dotted keys, tables and arrays of tables.

    Table names repeat across sections, so implicit tables are later declared explicitly or
    extended through dotted keys, as `tomllib` orders them by first creation.
    """

    sections: list[str] = [
        rng.choice(
            (
                f"[table{index}]\n{_toml_pairs(rng, '')}",
                f"[parent.child{index}]\n{_toml_pairs(rng, '')}",
                f"[[rows]]\n{_toml_pairs(rng, '')}",
                f"{_toml_pairs(rng, f'dotted{index}.')}",
                f"[{rng.choice(TOML_TABLE_NAMES)}]\n{_toml_pairs(rng, '')}",
                f"[[{rng.choice(TOML_TABLE_NAMES)}]]\n{_toml_pairs(rng, '')}",
                f"{_toml_pairs(rng, rng.choice(TOML_TABLE_NAMES) + '.')}",
                f"[late{index}.inner]\n{_toml_pairs(rng, '')}[side{index}]\n"
                f"{_toml_pairs(rng, '')}[late{index}]\n{_toml_pairs(rng, '')}",
                f"[[late{index}.rows]]\n{_toml_pairs(rng, '')}[late{index}]\n"
                f"{_toml_pairs(rng, f'more{index}.')}",
            )
        )
        for index in range(rng.randint(0, 6))
    ]
    document: str = _toml_pairs(rng, "") + "\n".join(sections)
    line_endings: str = rng.choice((document, document, document.replace("\n", "\r\n")))
    return rng.choice(BYTE_ORDER_MARKS) + line_endings


TOML_NAMESPACE_NAMES: tuple[str, ...] = (
    "a", "b", "a.b", "a.c", "b.a", "a.b.c", "a.b.c.d", "b.c.d", '"a".b', "a.'b'.c",
)  # fmt: skip
TOML_NAMESPACE_VALUES: tuple[str, ...] = (
    "1", "{}", "{x = 1}", "{c = {d = 1}}", "[]", "[{x = 1}]", "{b.c = 1}", "[1, 2]",
)  # fmt: skip
TOML_NAMESPACE_STATEMENTS: tuple[Callable[[random.Random], str], ...] = (
    lambda rng: f"[{rng.choice(TOML_NAMESPACE_NAMES)}]",
    lambda rng: f"[[{rng.choice(TOML_NAMESPACE_NAMES)}]]",
    lambda rng: f"{rng.choice(TOML_NAMESPACE_NAMES)} = {rng.choice(TOML_NAMESPACE_VALUES)}",
    lambda rng: f"{rng.choice(TOML_NAMESPACE_NAMES)}.k{rng.randint(0, 2)} = 1",
    lambda rng: (
        f"x = {{{rng.choice(TOML_NAMESPACE_NAMES)} = 1, {rng.choice(TOML_NAMESPACE_NAMES)} = 2}}"
    ),
)


def toml_namespace_document(*, rng: random.Random) -> str:
    """Return statements that reopen tables, arrays of tables and dotted keys in random orders."""

    return "\n".join(rng.choice(TOML_NAMESPACE_STATEMENTS)(rng) for _ in range(rng.randint(1, 8)))


TOML_DOCUMENT_GENERATORS: dict[str, Callable[[random.Random], str]] = {
    "toml": lambda rng: toml_document(rng=rng),
    "toml namespaces": lambda rng: toml_namespace_document(rng=rng),
}


def _float_canonical(value: float) -> list[object]:
    bits: str = str(struct.unpack("<Q", struct.pack("<d", value))[0])
    return ["float", {True: "nan", False: bits}[math.isnan(value)]]


OFFSET_SECONDS: dict[type, Callable[[Any], int | None]] = {
    type(None): lambda offset: None,
    datetime.timedelta: lambda offset: int(offset.total_seconds()),
}


def _utc_offset(value: datetime.datetime) -> int | None:
    offset: datetime.timedelta | None = value.utcoffset()
    return OFFSET_SECONDS[type(offset)](offset)


CANONICAL: dict[type, Callable[[Any], object]] = {
    type(None): lambda value: ["null"],
    bool: lambda value: ["bool", value],
    int: lambda value: ["int", str(value)],
    float: _float_canonical,
    str: lambda value: ["str", value],
    datetime.date: lambda value: ["date", [value.year, value.month, value.day]],
    datetime.datetime: lambda value: [
        "datetime",
        [value.year, value.month, value.day],
        [value.hour, value.minute, value.second, value.microsecond],
        _utc_offset(value),
    ],
    datetime.time: lambda value: [
        "time",
        [value.hour, value.minute, value.second, value.microsecond],
    ],
    list: lambda value: ["list", [canonical(item) for item in value]],
    dict: lambda value: ["map", [[canonical(key), canonical(item)] for key, item in value.items()]],
}


def canonical(value: object) -> object:
    """Return the tagged canonical form the native oracle hooks also produce."""

    return CANONICAL[type(value)](value)


def python_toml_outcome(*, text: str) -> object:
    """Return `tomllib`'s canonical value, or the rejection marker when it raises."""

    outcome: list[object] = [PYTHON_ERROR]
    with suppress(tomllib.TOMLDecodeError, ValueError):
        outcome[0] = canonical(tomllib.loads(text))
    return outcome[0]


def native_outcome(payload: str) -> object:
    """Return the native canonical value, or the marker of its failure kind."""

    parsed: object = json.loads(payload)
    return {list: parsed, dict: DEFERRED}[type(parsed)]


def native_yaml_outcome(*, text: str) -> object:
    """Return the native YAML loader's outcome for `text`."""

    return native_outcome(_native._oracle_yaml_load(text))


def native_toml_outcome(*, text: str) -> object:
    """Return the native TOML loader's outcome for `text`."""

    return native_outcome(_native._oracle_toml_load(text))


def repository_files(*, pattern: str) -> list[Path]:
    """Return tracked-looking repository files matching `pattern`, outside build directories."""

    return sorted(
        filter(
            lambda path: EXCLUDED_DIRECTORIES.isdisjoint(path.relative_to(REPOSITORY_ROOT).parts),
            REPOSITORY_ROOT.glob(pattern),
        )
    )


def deferred_valid_count(*, expected: list[object], actual: list[object]) -> int:
    """Return how many documents Python accepts but the native reader defers back to Python."""

    return sum(
        map(
            lambda python, native: native == DEFERRED and python not in UNCHECKED_PYTHON_OUTCOMES,
            expected,
            actual,
        )
    )


def toml_harmful_mismatches(
    *, inputs: list[object], expected: list[object], actual: list[object]
) -> list[tuple[object, object, object]]:
    """Return the first few documents `tomllib` reads whose native value differs.

    Config files `tomllib` accepts must load identically. For invalid edge cases the native
    reader follows its TOML library, so accepting a document `tomllib` rejects is not harmful.
    """

    return list(
        compress(
            zip(inputs, expected, actual, strict=True),
            map(_is_toml_harmful, expected, actual),
        )
    )[:3]


def _is_toml_harmful(expected: object, actual: object) -> bool:
    return expected != PYTHON_ERROR and actual != DEFERRED and actual != expected


PROJECT_CONFIG: str = 'name = "orders"\nadapter = "duckdb"\n'


class FailureCapture:
    """Swallow and keep the discovery failure a block raises."""

    def __init__(self) -> None:
        self.failure: BaseException | None = None

    def __enter__(self) -> FailureCapture:
        return self

    def __exit__(self, error_type: object, error: BaseException | None, traceback: object) -> bool:
        self.failure = error
        return isinstance(error, OSError | ValueError | RuntimeError)


def write_project(*, project_dir: Path, files: tuple[tuple[str, bytes], ...]) -> None:
    """Write a project config and authored files."""

    project_dir.mkdir(parents=True, exist_ok=True)
    _ = (project_dir / "sqlbuild_project.toml").write_text(PROJECT_CONFIG, encoding="utf-8")
    for relative_path, data in files:
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_bytes(data)


def stage_outcome(*, project_dir: Path) -> tuple[object, ...]:
    """Return the rendered discovery stage with its failure type and message."""

    capture: FailureCapture = FailureCapture()
    rendered: list[object] = [None]
    with capture:
        rendered[0] = render_stage_capture(discover_project_inputs(project_dir=project_dir))
    return (rendered[0], type(capture.failure).__name__, str(capture.failure))


def random_undecodable_bytes(*, rng: random.Random) -> bytes:
    """Return text bytes with UTF-8 sequences cut, corrupted or replaced by invalid bytes."""

    text: bytes = random_text(rng=rng, max_length=12).encode("utf-8")
    cut: int = rng.randint(0, len(text))
    noise: bytes = bytes(rng.choice(UNDECODABLE_BYTES) for _ in range(rng.randint(0, 3)))
    return rng.choice((text[:cut] + noise + text[cut:], text[:cut], noise + text))


def native_read_outcomes(*, project_dir: Path, relative_paths: list[str]) -> list[object]:
    """Return the type and text of the read error native reading reports for each file, if any."""

    return [
        {True: (type(row).__name__, str(row)), False: ("NoneType", "None")}[
            isinstance(row, UnicodeError | OSError)
        ]
        for row in _native_yaml_rows(project_dir=project_dir, relative_paths=relative_paths)
    ]


def _native_yaml_rows(*, project_dir: Path, relative_paths: list[str]) -> list[object]:
    """Each file's `(contents, loaded)`, or the error native source YAML loading raises for it."""

    return cast(
        list[object],
        _native.load_yaml_files(
            {"project_dir": str(project_dir), "display_prefix": "", "kind": "source"},
            relative_paths,
            _native.NativeProjectTree(str(project_dir)),
        ),
    )


def python_read_outcomes(*, project_dir: Path, relative_paths: list[str]) -> list[object]:
    """Return the type and text of the error `Path.read_text` raises for each file, if any."""

    outcomes: list[object] = []
    for relative_path in relative_paths:
        capture: FailureCapture = FailureCapture()
        with capture:
            _ = (project_dir / relative_path).read_text(encoding="utf-8")
        outcomes.append((type(capture.failure).__name__, str(capture.failure)))
    return outcomes


def compile_failure(*, project_dir: Path) -> tuple[str, str]:
    """Return the type and text of the error project discovery raises."""

    capture: FailureCapture = FailureCapture()
    with capture:
        _ = discover_project_inputs(project_dir=project_dir)
    return type(capture.failure).__name__, str(capture.failure)


def discovery_failure_with_help(*, project_dir: Path) -> tuple[str, str, bool]:
    """Return the type and text of the error project discovery raises, and whether it has help."""

    capture: FailureCapture = FailureCapture()
    with capture:
        _ = discover_project_inputs(project_dir=project_dir)
    return (
        type(capture.failure).__name__,
        str(capture.failure),
        getattr(capture.failure, "help", None) is not None,
    )


def tolerant_scope_fault_outcome(*, project_dir: Path) -> tuple[object, ...]:
    """Return tolerant scope discovery's model and source paths and its per-file faults."""

    discovery: TolerantScopeDiscovery = discover_scope_snapshot(project_dir=project_dir)
    return (
        *_inputs_paths(discovery.discovered_inputs),
        tuple(
            (str(fault.path), fault.message)
            for fault in (*discovery.resource_faults, *discovery.relationship_faults)
        ),
    )


def description_inputs_outcome(*, project_dir: Path) -> tuple[object, ...]:
    """Return the model and source paths model description resolution reads, without faults."""

    return (*_inputs_paths(discover_model_description_inputs(project_dir=project_dir)), ())


def selected_contract_outcome(*, project_dir: Path) -> tuple[object, ...]:
    """Return the model and source paths selected-contract discovery reads for `orders`."""

    with _native.directory_snapshot_scope(project_dir=project_dir):
        models: tuple[DiscoveredSqlModelFile, ...] = _native.discover_model_files(
            project_dir=project_dir,
            extract_implicit_alias_columns=False,
            extract_output_column_locations=False,
            selected_model_names=frozenset({"orders"}),
        )
        sources: tuple[DiscoveredSourceFile, ...] = _native.discover_source_files(
            project_dir=project_dir
        )
    return (
        tuple(model.relative_path.as_posix() for model in models),
        tuple(source.relative_path.as_posix() for source in sources),
        (),
    )


def _inputs_paths(inputs: DiscoveredProjectInputs) -> tuple[object, ...]:
    return (
        tuple(model.relative_path.as_posix() for model in inputs.model_files),
        tuple(source.relative_path.as_posix() for source in inputs.source_files),
    )


DECLARATION_DISCOVERERS: dict[str, Callable[..., tuple[object, ...]]] = {
    "enum": discover_enum_files,
    "constant": discover_constant_files,
    "model_schema": discover_model_schema_files,
    "audit": discover_audit_files,
    "sql_hook": discover_sql_hook_files,
    "sql_function": discover_sql_function_files,
}


def declaration_files_outcome(*, project_dir: Path, kind: str) -> tuple[object, ...]:
    """Return the rendered `kind` collection, or its failure type, text and help."""

    capture: FailureCapture = FailureCapture()
    with capture:
        return (
            "parsed",
            render_stage_capture(DECLARATION_DISCOVERERS[kind](project_dir=project_dir)),
        )
    failure: BaseException | None = capture.failure
    return (type(failure).__name__, str(failure), getattr(failure, "help", None))


def native_declaration_tag(*, file_path: Path, kind: str) -> str:
    """Return whether native parsing parses, fails or defers one file's contents."""

    return _native.parse_declaration_contents(
        {
            "kind": kind,
            "function_keys": sorted(SQL_FUNCTION_HEADER_KEYS),
            "audit_keys": sorted(SQL_AUDIT_HEADER_KEYS),
            "hook_keys": sorted(SQL_HOOK_HEADER_KEYS),
        },
        (str(file_path), file_path.stem),
        file_path.read_text(encoding="utf-8"),
    )


def tolerant_declaration_outcome(
    *, project_dir: Path
) -> tuple[object, tuple[DiscoveryFileFault, ...], tuple[DiscoveryFileFault, ...]]:
    """Return tolerant scope discovery's declaration collections and faults."""

    discovery: TolerantScopeDiscovery = discover_scope_snapshot(project_dir=project_dir)
    return (
        render_stage_capture(discovery.discovered_inputs),
        discovery.resource_faults,
        discovery.declaration_faults,
    )


# Seeded generators of declaration file contents: each builds a valid file, then often applies one
# authored mistake or awkward form, so the corpus covers every failure path and its position.
VALID_IDENTIFIERS: tuple[str, ...] = ("order_status", "orders", "cap", "a", "a1", "tier_2")
ODD_IDENTIFIERS: tuple[str, ...] = (
    "OrderStatus", "HTTPCode", "x2Y", "__dunder", "_private", "order_", "9lives", "order-status",
    "caf\u00e9", "PLACED", "_", "order__status", "'quoted_name'", "1", "true", "null", "1.5",
)  # fmt: skip
ENUM_MEMBERS: tuple[str, ...] = ("PLACED", "SHIPPED", "RETURNED", "A", "B_2")
ODD_ENUM_MEMBERS: tuple[str, ...] = ("placed", "Placed", "1", "'quoted'", "true", "x\u00e9", "_")
HEADER_SCALARS: tuple[str, ...] = (
    "1", "-2", "+3", "0", "007", "1.5", ".5", "5.", "true", "false", "null", "'text'",
    '"quoted"', "''", "' '", "word", "PLACED", "\u0661\u0662", "x\u00e9", "1e5", "'it''s'",
    "'a\\'b'", "99999999999999999999999",
)  # fmt: skip
HEADER_VALUES: tuple[str, ...] = (
    "1", "'text'", "[1, 2]", "{'a', 'b'}", "1.5", "true", "'2024-01-01'", "[]", "{}", "null",
    "['a', 'b']", "(x 1)", "[1, 'a']", "[[1], [2]]", "-7",
)  # fmt: skip
DECLARATION_SPACES: tuple[str, ...] = (
    " ",
    "",
    "  ",
    "\n",
    "\n  ",
    " ",
    "\n",
    "\t",
    "\u00a0",
    "\r\n",
) * 6 + (" -- note\n", " /* c */ ", "\u2028", "\x1c", "\u3000")
SQL_BODIES: tuple[str, ...] = (
    "SELECT 1",
    '\n  SELECT *\n  FROM __ref("@model")\n  WHERE NOT (@expression)\n',
    "SELECT ';' AS x",
    "WITH t AS (SELECT 1) SELECT * FROM t",
    "SELECT 'it''s' AS x, \"q\" FROM `t`",
    "\tSELECT\n\t\t1\n",
    "seLECT 1 -- trailing\n",
    "SELECT 1 /* ( */",
)
ODD_SQL_BODIES: tuple[str, ...] = (
    "", "   ", "-- only a comment", "SELECT 1; SELECT 2", "SELECT (1", "MEASURE (SELECT 1);",
    "\u017felect 1", "EVIDENCE (SELECT 1);", "SELECT 1;", "SELECT ')' AS x",
)  # fmt: skip
AUDIT_OPTIONAL_KEYS: tuple[str, ...] = (
    "severity", "run_scope", "always_run", "sample_count", "sample_unit", "thresholds",
    "minimum_samples", "sql_analysis", "unknown_key", "nme",
)  # fmt: skip
FUNCTION_HEADER_KEYS: tuple[str, ...] = (
    "arguments", "returns", "database", "schema", "tags", "description", "argumnts", "body",
)  # fmt: skip
VALID_FUNCTION_HEADER: str = 'arguments (amount DECIMAL), returns DECIMAL, description "Doubles"'
ODD_WEIGHTS: tuple[int, int] = (85, 15)


def _space(rng: random.Random) -> str:
    return rng.choice(DECLARATION_SPACES)


def _pick(rng: random.Random, valid: tuple[str, ...], odd: tuple[str, ...]) -> str:
    """Mostly a valid form, sometimes an odd one."""

    return rng.choice(rng.choices((valid, odd), weights=ODD_WEIGHTS)[0])


def _maybe(rng: random.Random, chance: float, entry: str) -> list[str]:
    """`[entry]` with probability `chance`, else no entry."""

    return [entry] * (rng.random() < chance)


def _scalar_value(rng: random.Random, depth: int) -> str:
    _ = depth
    return rng.choice((*HEADER_VALUES, *HEADER_SCALARS))


def _list_value(rng: random.Random, depth: int) -> str:
    return "[" + ", ".join(_value(rng, depth + 1) for _ in range(rng.randint(0, 3))) + "]"


def _set_value(rng: random.Random, depth: int) -> str:
    return "{" + ", ".join(_value(rng, depth + 1) for _ in range(rng.randint(0, 3))) + "}"


def _map_value(rng: random.Random, depth: int) -> str:
    entries: str = ", ".join(
        f"key_{index} {_value(rng, depth + 1)}" for index in range(rng.randint(0, 3))
    )
    return f"({entries})"


def _wrapped_value(rng: random.Random, depth: int) -> str:
    keys: list[str] = rng.sample(("value", "type", "render_as", "extra"), k=rng.randint(0, 3))
    return "constant(" + ", ".join(f"{key} {_value(rng, depth + 1)}" for key in keys) + ")"


VALUE_FORMS: tuple[Callable[[random.Random, int], str], ...] = (
    _scalar_value, _list_value, _set_value, _map_value, _wrapped_value,
)  # fmt: skip
VALUE_FORM_WEIGHTS: dict[int, tuple[int, ...]] = {
    0: (50, 15, 10, 10, 15),
    1: (50, 15, 10, 10, 15),
    2: (100, 0, 0, 0, 0),
}


def _value(rng: random.Random, depth: int = 0) -> str:
    form: Callable[[random.Random, int], str] = rng.choices(
        VALUE_FORMS, weights=VALUE_FORM_WEIGHTS[min(depth, 2)]
    )[0]
    return form(rng, depth)


def _shuffled(rng: random.Random, entries: list[str]) -> str:
    """Entries in authored order, or shuffled: the parsers must not depend on key order."""

    return ", ".join(rng.choice((entries, rng.sample(entries, k=len(entries)))))


def _declaration_count(rng: random.Random) -> int:
    return rng.choice((1, 1, 1, 1, 2, 2, 3))


def _statement(rng: random.Random, keyword: str, header: str) -> list[str]:
    return [_space(rng), keyword, _space(rng), f"({header})", _space(rng), ";"]


STATEMENT_MISTAKES: tuple[Callable[[list[str]], None], ...] = (
    lambda statement: statement.__setitem__(5, ""),
    lambda statement: statement.__setitem__(5, ";;"),
    lambda statement: statement.__setitem__(5, ","),
    lambda statement: statement.__setitem__(1, statement[1].lower()),
    lambda statement: statement.__setitem__(1, "SCHEMA"),
    lambda statement: statement.__setitem__(3, statement[3][:-1]),
    lambda statement: None,
)
FILE_MISTAKES: tuple[Callable[[str], str], ...] = (
    lambda text: "-- header\n" + text,
    lambda text: text + "SELECT 1",
    lambda text: text[:0] + "\n",
    lambda text: text,
)


def _statements(rng: random.Random, keyword: str, headers: list[str]) -> str:
    """Join `KEYWORD (header);` statements with at most one statement and one file mistake."""

    statements: list[list[str]] = [_statement(rng, keyword, header) for header in headers]
    rng.choices(STATEMENT_MISTAKES, weights=(4, 3, 3, 3, 3, 3, 81))[0](rng.choice(statements))
    text: str = "".join("".join(statement) for statement in statements) + _space(rng)
    return rng.choices(FILE_MISTAKES, weights=(4, 4, 4, 88))[0](text)


def _identity(rng: random.Random, index: int) -> str:
    return _pick(rng, (f"{rng.choice(VALID_IDENTIFIERS)}_{index}",), ODD_IDENTIFIERS)


def _shorthand_members(rng: random.Random, names: list[str]) -> str:
    return "[" + ", ".join(_pick(rng, (name,), ODD_ENUM_MEMBERS) for name in names) + "]"


def _explicit_members(rng: random.Random, names: list[str]) -> str:
    values: tuple[str, ...] = rng.choice((("'x'", "'y'", "'z'"), ("1", "2", "-3")))
    entries: str = ", ".join(
        f"{_pick(rng, (name,), ODD_ENUM_MEMBERS)} {_pick(rng, values, HEADER_SCALARS)}"
        for name in names
    )
    return f"({entries})"


def _odd_members(rng: random.Random, names: list[str]) -> str:
    _ = names
    return rng.choice(("[]", "()", "1", "'x'", "{A, B}", "null", _value(rng)))


ENUM_MEMBER_FORMS: tuple[Callable[[random.Random, list[str]], str], ...] = (
    _shorthand_members, _explicit_members, _odd_members,
)  # fmt: skip


def _enum_header(rng: random.Random, index: int) -> str:
    names: list[str] = rng.sample(ENUM_MEMBERS, k=rng.randint(1, 3))
    names.extend(names[:1] * (rng.random() < 0.1))
    members: str = rng.choices(ENUM_MEMBER_FORMS, weights=(50, 42, 8))[0](rng, names)
    entries: list[str] = [
        *_maybe(rng, 0.97, f"name {_identity(rng, index)}"),
        *_maybe(rng, 0.97, f"members {members}"),
        *_maybe(rng, 0.05, "extra 1"),
    ]
    return _shuffled(rng, entries)


def enum_file(*, rng: random.Random) -> str:
    """Return enum file contents, mostly valid, some with one authored mistake."""

    headers: list[str] = [_enum_header(rng, index) for index in range(_declaration_count(rng))]
    return _statements(rng, "ENUM", headers)


def _constant_header(rng: random.Random, index: int) -> str:
    value: str = rng.choice((rng.choice(HEADER_VALUES), _value(rng)))
    types: tuple[str, ...] = ("INTEGER", "VARCHAR", "BOOLEAN", "decimal", "1", "true", "''")
    renderings: tuple[str, ...] = ("value_list", "array", "list", "1", "''")
    entries: list[str] = [
        f"name {_identity(rng, index)}",
        *_maybe(rng, 0.95, f"value {value}"),
        *_maybe(rng, 0.25, f"type {rng.choice(types)}"),
        *_maybe(rng, 0.2, f"render_as {rng.choice(renderings)}"),
        *_maybe(rng, 0.05, "unknown 1"),
    ]
    return _shuffled(rng, entries)


def constant_file(*, rng: random.Random) -> str:
    """Return constant file contents with wrapped, typed and malformed constants."""

    headers: list[str] = [_constant_header(rng, index) for index in range(_declaration_count(rng))]
    return _statements(rng, "CONSTANT", headers)


def _column(rng: random.Random, index: int) -> str:
    audits: str = _pick(rng, ("not_null", "unique", "accepted_values(values [1])"), ("1",))
    metadata: list[str] = [
        *_maybe(rng, 0.8, f"type {_pick(rng, ('INTEGER', 'VARCHAR', 'DECIMAL(10, 2)'), ('1',))}"),
        *_maybe(rng, 0.3, f"nullable {_pick(rng, ('true', 'false'), ('1', 'maybe'))}"),
        *_maybe(rng, 0.3, f"description {_pick(rng, ('"Order id"',), ('""', '1'))}"),
        *_maybe(rng, 0.3, f"audits [{audits}]"),
        *_maybe(rng, 0.03, "migrate_from old"),
    ]
    name: str = _pick(rng, (f"column_{index}",), ("ORDER_ID", '"spaced name"', "column_0"))
    return f"{name} ({', '.join(metadata)})"


def _schema_header(rng: random.Random, index: int) -> str:
    columns: str = ", ".join(_column(rng, column) for column in range(rng.randint(0, 3)))
    description: str = _pick(rng, ('"Orders"', "word"), ('""', '" "', "1", "null"))
    extends: str = _pick(rng, ("base_shape", '"base"'), ('"not ident"', "1", "null"))
    entries: list[str] = [
        f"name {_identity(rng, index)}",
        *_maybe(rng, 0.5, f"description {description}"),
        *_maybe(rng, 0.3, f"extends {extends}"),
        *_maybe(
            rng, 0.95, rng.choices((f"columns ({columns})", f"columns {_value(rng)}"), (95, 5))[0]
        ),
        *_maybe(rng, 0.05, "extra 1"),
    ]
    return _shuffled(rng, entries)


def schema_file(*, rng: random.Random) -> str:
    """Return reusable model schema file contents."""

    headers: list[str] = [_schema_header(rng, index) for index in range(_declaration_count(rng))]
    return _statements(rng, "SCHEMA", headers)


def _violation_audit(rng: random.Random, index: int, named: bool) -> tuple[str, str]:
    entries: list[str] = [
        *_maybe(rng, named, f"name {_pick(rng, (f'check_{index}',), ('check_0', '""', '1'))}"),
        *_maybe(rng, 0.05, f"evaluation {rng.choice(('violations', 'count', '1'))}"),
        *(f"{key} {rng.choice(('x', 'true', '1', '""'))}" for key in _optional_keys(rng)),
    ]
    return _shuffled(rng, entries), _pick(rng, SQL_BODIES, ODD_SQL_BODIES)


def _measurement_audit(rng: random.Random, index: int, named: bool) -> tuple[str, str]:
    entries: list[str] = [
        *_maybe(rng, named, f"name {_pick(rng, (f'check_{index}',), ('check_0', '""', '1'))}"),
        "evaluation measurement",
        *_maybe(rng, 0.95, f"value {_pick(rng, ('total', '"amount"'), ('1', '""'))}"),
        *(f"{key} {rng.choice(('x', 'true', '1', '""'))}" for key in _optional_keys(rng)),
    ]
    measure: str = f"MEASURE{_space(rng)}({_pick(rng, SQL_BODIES, ODD_SQL_BODIES)})"
    evidence: str = (
        f"{_pick(rng, ('EVIDENCE',), ('evidence', 'MEASURE', 'EVIDENCES'))} "
        f"({_pick(rng, SQL_BODIES, ODD_SQL_BODIES)});"
    )
    parts: list[str] = [
        *_maybe(rng, 0.95, measure + rng.choices((";", ""), weights=(95, 5))[0]),
        *_maybe(rng, 0.4, evidence),
        *_maybe(rng, 0.05, "SELECT 1"),
    ]
    return _shuffled(rng, entries), "\n".join(parts)


def _optional_keys(rng: random.Random) -> list[str]:
    return rng.sample(AUDIT_OPTIONAL_KEYS, k=rng.choices((0, 1, 2), weights=(70, 20, 10))[0])


AUDIT_FORMS: tuple[Callable[[random.Random, int, bool], tuple[str, str]], ...] = (
    _violation_audit, _measurement_audit,
)  # fmt: skip


def audit_file(*, rng: random.Random) -> str:
    """Return SQL audit file contents with violation and measurement blocks."""

    count: int = _declaration_count(rng)
    named: bool = count > 1 or rng.random() < 0.5
    blocks: list[str] = []
    for index in range(count):
        header, body = rng.choices(AUDIT_FORMS, weights=(65, 35))[0](rng, index, named)
        keyword: str = _pick(rng, ("AUDIT", "audit", "Audit"), ("AUD\u0131T", "AUDITS"))
        close: str = _pick(rng, (");", ") ;", ");\n"), (")", "); -- c"))
        blocks.append(f"{keyword}{_space(rng)}({header}{close}\n{body}\n")
    leading: str = _pick(rng, ("", "\n\n", "  "), ("-- lead\n", "SELECT 1;\n"))
    return leading + "".join(blocks)


def hook_file(*, rng: random.Random) -> str:
    """Return SQL hook file contents."""

    header: str = _pick(
        rng,
        ("", 'description "Refresh"', "description 'it''s (fine)'", "description `q`"),
        (
            'description ""',
            "description 1",
            'descripton "typo"',
            'description "a (b" , other 1',
            "description 'unterminated",
            "description 'a\\'b'",
        ),
    )
    keyword: str = _pick(rng, ("HOOK",), ("hook", "HOOKS", "-- c\nHOOK"))
    close: str = _pick(rng, (");", ") ;", ");\n"), (")", "));"))
    body: str = _pick(rng, SQL_BODIES, ODD_SQL_BODIES)
    return f"{_space(rng)}{keyword}{_space(rng)}({header}{close}{body}"


def function_file(*, rng: random.Random) -> str:
    """Return SQL function file contents."""

    keys: list[str] = rng.sample(FUNCTION_HEADER_KEYS, k=rng.randint(0, 3))
    generated: str = ", ".join(f"{key} {_value(rng)}" for key in keys)
    entries: str = rng.choices((VALID_FUNCTION_HEADER, generated), weights=(70, 30))[0]
    keyword: str = _pick(rng, ("FUNCTION",), ("function", "FUNC"))
    close: str = _pick(rng, (");", ") ;", ");\n"), (")",))
    body: str = _pick(rng, SQL_BODIES, ODD_SQL_BODIES)
    return f"{_space(rng)}{keyword}{_space(rng)}({entries}{close}{body}"


DECLARATION_GENERATORS: dict[str, Callable[..., str]] = {
    "enum": enum_file,
    "constant": constant_file,
    "model_schema": schema_file,
    "audit": audit_file,
    "sql_hook": hook_file,
    "sql_function": function_file,
}


def generated_declaration_tags(
    *, project_root: Path, test_case: GeneratedDeclarationFileTestCase
) -> Counter[str]:
    """Discover each generated file and count whether native parsing parses or fails it."""

    tags: Counter[str] = Counter()
    for seed in range(test_case.case_count):
        project_dir: Path = project_root / str(seed)
        contents: str = DECLARATION_GENERATORS[test_case.kind](
            rng=random.Random(f"{test_case.kind}-{seed}")
        )
        write_project(
            project_dir=project_dir, files=((test_case.relative_path, contents.encode("utf-8")),)
        )
        _ = declaration_files_outcome(project_dir=project_dir, kind=test_case.kind)
        tags[
            native_declaration_tag(
                file_path=project_dir / test_case.relative_path, kind=test_case.kind
            )
        ] += 1
    return tags


def on_deep_stack(*, frames: int, call: Callable[[], object]) -> object:
    """Return `call()` invoked beneath `frames` extra Python frames."""

    nested: Callable[[], object] = functools.reduce(
        lambda inner, _: lambda: inner(), range(frames), call
    )
    return nested()


def _dataclass_children(node: object) -> list[object]:
    return [getattr(node, field.name) for field in dataclasses.fields(cast(Any, node))]


_SHAPE_CHILDREN: dict[type, Callable[[Any], list[object]]] = {
    dict: lambda node: list(itertools.chain.from_iterable(node.items())),
    list: list,
    tuple: list,
    AuthoredSqlSet: _dataclass_children,
    AuthoredSqlValueCall: _dataclass_children,
    NamedSqlHookEntry: _dataclass_children,
    PythonHookEntry: _dataclass_children,
    SqlHookEntry: _dataclass_children,
}
_SHAPE_LABELS: dict[type, Callable[[Any], str]] = dict.fromkeys(_SHAPE_CHILDREN, lambda _: "")


def value_shape(value: object) -> tuple[tuple[int, str], ...]:
    """Each node of `value` as its depth and type or scalar text, walked without recursion."""

    shape: list[tuple[int, str]] = []
    pending: list[tuple[int, object]] = [(0, value)]
    while pending:
        depth, node = pending.pop()
        children: list[object] = _SHAPE_CHILDREN.get(type(node), lambda _: [])(node)
        label: str = _SHAPE_LABELS.get(type(node), repr)(node)
        shape.append((depth, f"{type(node).__name__}:{label}"))
        pending.extend((depth + 1, child) for child in reversed(children))
    return tuple(shape)


def deep_model_header_values(*, project_dir: Path, frames: int) -> dict[str, object]:
    """Discover the project's only model beneath `frames` extra Python frames."""

    inputs: DiscoveredProjectInputs = cast(
        DiscoveredProjectInputs,
        on_deep_stack(frames=frames, call=lambda: discover_project_inputs(project_dir=project_dir)),
    )
    return inputs.model_files[0].header_values


def native_yaml_tags_and_values(
    *, project_dir: Path, relative_paths: list[str]
) -> tuple[list[object], list[object]]:
    """Return whether each file loaded (`ok`) or failed (`error`), and the loaded values."""

    rows: list[object] = _native_yaml_rows(project_dir=project_dir, relative_paths=relative_paths)
    failed: list[bool] = [isinstance(row, Exception) for row in rows]
    loaded: list[tuple[str, object]] = cast(
        list[tuple[str, object]], list(compress(rows, [not failure for failure in failed]))
    )
    return [{True: "error", False: "ok"}[failure] for failure in failed], [row[1] for row in loaded]


VALIDATION_MODEL: tuple[str, bytes] = ("sql/models/orders.sql", b"MODEL ();\nSELECT 1 AS order_id")


def seed_project_files(*, header: bytes, settings: bytes = b"") -> tuple[tuple[str, bytes], ...]:
    """A model and a `countries` seed declaring `code` and `label` with optional CSV settings."""

    return (
        VALIDATION_MODEL,
        ("sql/seeds/countries.csv", header + b"\nGB,United Kingdom\n"),
        (
            "sql/seeds/countries.yml",
            b"seeds:\n  - name: countries\n"
            + settings
            + b"    columns:\n      - name: code\n        type: VARCHAR\n"
            b"      - name: label\n        type: VARCHAR\n",
        ),
    )


def csv_reader_error(settings: dict[str, str]) -> tuple[str, str] | None:
    """The error this Python's `csv.reader` raises for the settings, if any."""

    try:
        csv.reader((), **cast(dict[str, Any], settings))
    except (TypeError, ValueError) as error:
        return type(error).__name__, str(error)
    return None


def discovery_error(*, project_dir: Path) -> tuple[str, str] | None:
    """The exception discovery raises for the project, if any."""

    try:
        discover_project_inputs(project_dir=project_dir)
    except Exception as error:
        return type(error).__name__, str(error)
    return None


def model_file_record() -> DiscoveredSqlModelFile:
    """A model file record built through its keyword constructor."""

    return DiscoveredSqlModelFile(
        file_path=Path("/project/models/orders.sql"),
        relative_path=Path("sql/models/orders.sql"),
        contents="MODEL ();\nselect 1 as order_id",
        header_values={},
        header_column_locations={},
        output_column_locations={},
        query_sql="select 1 as order_id",
    )
