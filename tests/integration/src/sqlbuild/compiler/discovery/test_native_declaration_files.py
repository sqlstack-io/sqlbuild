"""Engine parity for the enum, constant, schema, audit, hook, function, seed and macro files."""

from __future__ import annotations

from collections import Counter
from pathlib import Path

import pytest

from sqlbuild import _native
from sqlbuild._native import DiscoveredProjectInputs, DiscoveryFileFault, discover_project_inputs
from tests.integration.src.sqlbuild.compiler.discovery._test_types import (
    DeclarationFilesParityTestCase,
    DeepDeclarationHeaderTestCase,
    GeneratedDeclarationFileTestCase,
    NativeSessionTestCase,
    TolerantDeclarationFilesTestCase,
)
from tests.integration.src.sqlbuild.compiler.discovery.helpers import (
    declaration_files_outcome,
    generated_declaration_tags,
    native_declaration_tag,
    stage_outcome,
    tolerant_declaration_outcome,
    write_project,
)

_NESTING_SUFFIX: str = "contains invalid SQLBuild header syntax: values nest deeper than 256 levels"
_MODEL: bytes = b'MODEL (description "Orders");\nSELECT 1 AS order_id'
_MACRO: bytes = b"def cents(value):\n    return f'{value} * 100'\n"
_ENUMS: bytes = (
    b"ENUM (name order_status, members [PLACED, SHIPPED]);\n"
    b"ENUM (\n  name priority,\n  members (LOW 1, HIGH 2),\n);\n"
)
_CONSTANTS: bytes = (
    b"CONSTANT (name cap, value 3);\n"
    b"CONSTANT (name regions, value ['eu', 'us'], render_as value_list);\n"
    b"CONSTANT (name rate, value constant(value '1.50', type decimal));\n"
)
_SCHEMA: bytes = (
    b'SCHEMA (\r\n  name order_shape,\r\n  description "Order shape",\r\n'
    b"  columns (\r\n    id (type INTEGER, audits [not_null]),\r\n"
    b'    total (type DECIMAL(10, 2), description "Total"),\r\n  ),\r\n);\r\n'
    b"SCHEMA (name wide_order, extends order_shape, columns (note (type VARCHAR)));\n"
)
_AUDIT: bytes = b'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE NOT (@expression)\n'
_MEASUREMENTS: bytes = (
    b"AUDIT (name row_count, evaluation measurement, value total, sample_unit 'rows');\n"
    b'MEASURE (SELECT count(*) AS total FROM __ref("@model"));\n'
    b'EVIDENCE (SELECT * FROM __ref("@model") LIMIT 5);\n'
    b'audit (name no_negative, severity warn);\n-- body\nSELECT * FROM __ref("@model") WHERE x < 0\n'
)
_HOOK: bytes = b'HOOK (\n  description "Record a refresh"\n);\n\n    SELECT 1 AS refreshed\n'
_FUNCTION: bytes = (
    b'FUNCTION (arguments (amount DECIMAL), returns DECIMAL, description "Doubles");\n'
    b"SELECT amount * 2\n"
)
_EVERY_KIND: tuple[tuple[str, bytes], ...] = (
    ("sql/models/marts/orders.sql", _MODEL),
    ("sql/macros/python/money.py", _MACRO),
    ("sql/macros/python/__init__.py", b""),
    ("sql/models/marts/_macros/python/local.py", _MACRO),
    ("sql/enums/status.sql", _ENUMS),
    ("sql/models/marts/_enums/tier.sql", b"ENUM (name tier, members [GOLD]);\n"),
    ("sql/models/marts/_sqlbuild/constants/limits.sql", _CONSTANTS),
    ("sql/constants/global.sql", b"CONSTANT (name ratio, value 1.5);\n"),
    ("sql/models/marts/_sqlbuild/_audits/generic/is_true.sql", _AUDIT),
    ("sql/models/marts/_sqlbuild/audits/singular/orders_ok.sql", _AUDIT),
    ("sql/models/marts/_sqlbuild/_schemas/order_shape.sql", _SCHEMA),
    ("sql/models/marts/_sqlbuild/hooks/sql/record_marts_refresh.sql", _HOOK),
    ("sql/audits/generic/is_true.sql", _AUDIT),
    ("sql/audits/generic/nested/measured.sql", _MEASUREMENTS),
    ("sql/audits/singular/orders_ok.sql", _AUDIT),
    ("sql/schemas/order_shape.sql", _SCHEMA),
    ("sql/hooks/sql/record_refresh.sql", _HOOK),
    ("sql/hooks/sql/_private.sql", b"not a hook"),
    ("sql/functions/sql/double_amount.sql", _FUNCTION),
    ("sql/functions/sql/finance/net_amount.sql", _FUNCTION),
    ("sql/functions/sql/_macros/python/helpers.py", _MACRO),
    ("sql/seeds/channels.csv", b"id\n1\n"),
    (
        "sql/seeds/seeds.yml",
        b"seeds:\n  - name: channels\n    description: Sales channels.\n"
        b"    columns:\n      - name: id\n        type: INTEGER\n"
        b"  - name: stores\n    description: Stores.\n"
        b"    columns:\n      - name: id\n        type: INTEGER\n",
    ),
    ("sql/seeds/regional/stores.csv", b"id\n2\n"),
    ("sql/seeds/readme.md", b"seeds"),
    ("sql/seeds/archive.csv/notes.txt", b"a directory named like a seed"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedDeclarationFileTestCase(
            description="generated enum files",
            kind="enum",
            relative_path="sql/enums/generated.sql",
            case_count=150,
            expected_minimum_parsed=20,
            expected_minimum_failed=60,
            expected_maximum_deferred=8,
        ),
        GeneratedDeclarationFileTestCase(
            description="generated constant files",
            kind="constant",
            relative_path="sql/models/marts/_constants/generated.sql",
            case_count=150,
            expected_minimum_parsed=60,
            expected_minimum_failed=30,
            expected_maximum_deferred=8,
        ),
        GeneratedDeclarationFileTestCase(
            description="generated reusable schema files",
            kind="model_schema",
            relative_path="sql/schemas/generated.sql",
            case_count=150,
            expected_minimum_parsed=50,
            expected_minimum_failed=30,
            expected_maximum_deferred=8,
        ),
        GeneratedDeclarationFileTestCase(
            description="generated audit files",
            kind="audit",
            relative_path="sql/models/marts/_sqlbuild/audits/generic/generated.sql",
            case_count=150,
            expected_minimum_parsed=30,
            expected_minimum_failed=60,
            expected_maximum_deferred=4,
        ),
        GeneratedDeclarationFileTestCase(
            description="generated SQL hook files",
            kind="sql_hook",
            relative_path="sql/hooks/sql/generated.sql",
            case_count=150,
            expected_minimum_parsed=40,
            expected_minimum_failed=40,
            expected_maximum_deferred=4,
        ),
        GeneratedDeclarationFileTestCase(
            description="generated SQL function files",
            kind="sql_function",
            relative_path="sql/functions/sql/generated.sql",
            case_count=150,
            expected_minimum_parsed=50,
            expected_minimum_failed=30,
            expected_maximum_deferred=4,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generated_declaration_files_when_discovering_then_native_parses_or_fails_them(
    test_case: GeneratedDeclarationFileTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    tags: Counter[str] = generated_declaration_tags(project_root=tmp_path, test_case=test_case)

    assert tags["ok"] >= test_case.expected_minimum_parsed, tags
    assert tags["error"] >= test_case.expected_minimum_failed, tags
    assert tags["defer"] <= test_case.expected_maximum_deferred, tags


@pytest.mark.parametrize(
    "test_case",
    [
        DeclarationFilesParityTestCase(
            description="global, inherited, local and grouped declarations of every kind",
            files=_EVERY_KIND,
        ),
        DeclarationFilesParityTestCase(
            description="the first failing enum file in path order is reported",
            files=(
                ("sql/enums/a_status.sql", b"ENUM (name a_status, members [placed]);\n"),
                ("sql/enums/b_status.sql", b"ENUM (name b_status);\n"),
            ),
            expected_failure_type="DeclarationParseError",
            expected_message_fragment="member identifiers must be uppercase: 'placed'",
        ),
        DeclarationFilesParityTestCase(
            description="an earlier constant's value fails before a later constant's name",
            files=(
                (
                    "sql/constants/limits.sql",
                    b"CONSTANT (name launch, value 'soon', type DATE);\n"
                    b"CONSTANT (name Bad, value 1);\n",
                ),
            ),
            expected_failure_type="DeclarationParseError",
            expected_message_fragment="constant 'launch'",
        ),
        DeclarationFilesParityTestCase(
            description="an earlier schema's columns fail before a later schema's keys",
            files=(
                (
                    "sql/schemas/shapes.sql",
                    b"SCHEMA (name first_shape, columns (id (nullable maybe)));\n"
                    b"SCHEMA (name second_shape, extra 1, columns (id (type INT)));\n",
                ),
            ),
            expected_failure_type="DeclarationParseError",
            expected_message_fragment="first_shape",
        ),
        DeclarationFilesParityTestCase(
            description="a non-canonical declaration name fails with the identity help",
            files=(("sql/enums/status.sql", b"ENUM (name OrderStatus, members [PLACED]);\n"),),
            expected_failure_type="ResourceIdentityError",
            expected_message_fragment="use snake_case 'order_status'",
        ),
        DeclarationFilesParityTestCase(
            description="a nested declaration root fails before any file is parsed",
            files=(
                ("sql/models/marts/_enums/macros/python/money.py", _MACRO),
                ("sql/enums/status.sql", b"broken"),
            ),
            expected_failure_type="DeclarationParseError",
            expected_message_fragment="is nested inside another declaration tree",
        ),
        DeclarationFilesParityTestCase(
            description="an unreadable audit file fails with Python's decode error",
            files=(("sql/audits/generic/is_true.sql", b"AUDIT ();\nSELECT '\xff'\n"),),
            expected_failure_type="UnicodeDecodeError",
            expected_message_fragment="invalid start byte",
        ),
        DeclarationFilesParityTestCase(
            description="an unreadable macro file fails with Python's decode error",
            files=(("sql/macros/python/money.py", b"# \xc3"),),
            expected_failure_type="UnicodeDecodeError",
            expected_message_fragment="unexpected end of data",
        ),
        DeclarationFilesParityTestCase(
            description="a hook with an unsupported key fails with the key suggestion",
            files=(("sql/hooks/sql/refresh.sql", b'HOOK (descripton "Refresh");\nSELECT 1\n'),),
            expected_failure_type="SqlHookParseError",
            expected_message_fragment="has unsupported keys: descripton",
        ),
        DeclarationFilesParityTestCase(
            description="a measurement audit without MEASURE fails",
            files=(
                (
                    "sql/audits/generic/measured.sql",
                    b"AUDIT (evaluation measurement, value total);\nSELECT 1\n",
                ),
            ),
            expected_failure_type="SqlAuditParseError",
            expected_message_fragment="must define exactly one MEASURE(...) block",
        ),
        DeclarationFilesParityTestCase(
            description="a SQL function without a body fails",
            files=(("sql/functions/sql/empty.sql", b"FUNCTION (returns INT);\n  \n"),),
            expected_failure_type="ModelSqlParseError",
            expected_message_fragment="must contain SQL after FUNCTION(...)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_files_when_discovering_then_inputs_or_failures_are_expected(
    test_case: DeclarationFilesParityTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)
    native: tuple[object, ...] = stage_outcome(project_dir=tmp_path)

    assert native[1] == test_case.expected_failure_type
    assert test_case.expected_message_fragment in str(native[2])


@pytest.mark.parametrize(
    "test_case",
    [
        DeepDeclarationHeaderTestCase(
            description="a constant value nested 1k levels",
            kind="constant",
            relative_path="sql/constants/limits.sql",
            prefix="CONSTANT (name cap, value 3);\nCONSTANT (\n  name regions,\n  value ",
            suffix=",\n);\n",
            depth=1_000,
            expected_failure_type="DeclarationParseError",
            expected_message_suffix=f"limits.sql:4' {_NESTING_SUFFIX}",
        ),
        DeepDeclarationHeaderTestCase(
            description="a hook description nested 20k levels",
            kind="sql_hook",
            relative_path="sql/hooks/sql/refresh.sql",
            prefix="HOOK (\n  description ",
            suffix=",\n);\n\nSELECT 1\n",
            depth=20_000,
            expected_failure_type="SqlHookParseError",
            expected_message_suffix=f"refresh.sql:2' {_NESTING_SUFFIX}",
        ),
        DeepDeclarationHeaderTestCase(
            description="a function return type nested 100k levels",
            kind="sql_function",
            relative_path="sql/functions/sql/doubled.sql",
            prefix='FUNCTION (\n  description "Doubles",\n  returns ',
            suffix=",\n);\nSELECT 2\n",
            depth=100_000,
            expected_failure_type="ModelSqlParseError",
            expected_message_suffix=f"doubled.sql:3' {_NESTING_SUFFIX}",
        ),
        DeepDeclarationHeaderTestCase(
            description="a later audit block's value nested 100k levels",
            kind="audit",
            relative_path="sql/audits/generic/is_true.sql",
            prefix="AUDIT (name first);\nSELECT 1\nAUDIT (\n  name second,\n  value ",
            suffix=",\n);\nSELECT 2\n",
            depth=100_000,
            expected_failure_type="SqlAuditParseError",
            expected_message_suffix=f"is_true.sql:5' {_NESTING_SUFFIX}",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_deeply_nested_declaration_header_when_discovering_then_its_line_is_reported(
    test_case: DeepDeclarationHeaderTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    nested: str = "[" * test_case.depth + "1" + "]" * test_case.depth
    contents: str = test_case.prefix + nested + test_case.suffix
    write_project(project_dir=tmp_path, files=((test_case.relative_path, contents.encode()),))
    native: tuple[object, ...] = declaration_files_outcome(
        project_dir=tmp_path, kind=test_case.kind
    )

    assert native[0] == test_case.expected_failure_type
    assert str(native[1]).endswith(test_case.expected_message_suffix), native[1]
    assert native[2] == test_case.expected_help
    assert (
        native_declaration_tag(file_path=tmp_path / test_case.relative_path, kind=test_case.kind)
        == "error"
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TolerantDeclarationFilesTestCase(
            description="broken files of every kind become the same faults",
            files=(
                *_EVERY_KIND,
                ("sql/enums/broken.sql", b"ENUM (name broken);\n"),
                ("sql/constants/broken.sql", b"CONSTANT (name Broken, value 1);\n"),
                ("sql/schemas/broken.sql", b"SCHEMA (name broken_shape);\n"),
                ("sql/audits/generic/broken.sql", b"SELECT 1\n"),
                ("sql/hooks/sql/broken.sql", b"HOOK ();\n"),
                ("sql/functions/sql/broken.sql", b"SELECT 1\n"),
            ),
            expected_resource_faults=3,
            expected_declaration_faults=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_broken_declaration_files_when_discovering_tolerantly_then_faults_match(
    test_case: TolerantDeclarationFilesTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=test_case.files)
    native: tuple[object, tuple[DiscoveryFileFault, ...], tuple[DiscoveryFileFault, ...]] = (
        tolerant_declaration_outcome(project_dir=tmp_path)
    )

    assert len(native[1]) == test_case.expected_resource_faults
    assert len(native[2]) == test_case.expected_declaration_faults


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSessionTestCase(
            description="discovery keeps its declaration session",
            expected_session=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_discovering_then_native_session_is_kept(
    test_case: NativeSessionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    write_project(project_dir=tmp_path, files=_EVERY_KIND)

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    assert (
        isinstance(discovered.native_session, _native.NativeDiscoverySession)
        is test_case.expected_session
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
