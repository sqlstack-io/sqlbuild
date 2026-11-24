"""Unit tests for native header lint rules and formatting."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    LintConfig,
    SqlScenarioParseError,
    format_native_headers,
    lint_native_headers,
    scan_headers,
)
from tests.unit.src.sqlbuild.compiler.discovery.helpers import (
    parse_model_sql,
    parse_sql_scenario_file,
)
from tests.unit.src.sqlbuild.lint._helpers._test_types import (
    FormatDescriptionTestCase,
    FormatNativeTestCase,
    LintNativeTestCase,
    LintRemediationTestCase,
)

FILE_PATH: Path = Path("sql/models/example.sql")
DEFAULT_CONFIG: LintConfig = LintConfig()


@pytest.mark.parametrize(
    "test_case",
    [
        LintNativeTestCase(
            description="model without description faults",
            contents="MODEL (\n  materialized table\n);\nSELECT 1\n",
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="model with description passes",
            contents='MODEL (\n  materialized table,\n  description "ok"\n);\nSELECT 1\n',
            expected_codes=(),
        ),
        LintNativeTestCase(
            description="scenario without description faults",
            contents='SCENARIO (tags ["x"]);\nSELECT 1\n',
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="function without description faults",
            contents="FUNCTION (\n  arguments (amount INTEGER),\n  returns BOOLEAN,\n);\namount > 1\n",
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="named hook without description faults",
            contents="HOOK ();\nSELECT 1\n",
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="header-only check flags a model_schema model for project resolution",
            contents="MODEL (model_schema order_columns);\nSELECT 1 AS order_id\n",
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="long single-line scenario description faults by formatted length",
            contents=(
                'SCENARIO (description "'
                + " ".join(f"word{index}" for index in range(400))
                + '");\nSELECT 1\n'
            ),
            expected_codes=("description-length",),
        ),
        LintNativeTestCase(
            description="short manually wrapped scenario description passes",
            contents=(
                'SCENARIO (description "'
                + "\n ".join(f"line {index}" for index in range(11))
                + '");\nSELECT 1\n'
            ),
            expected_codes=(),
        ),
        LintNativeTestCase(
            description="whitespace-only model description faults as missing",
            contents='MODEL (description "   ");\nSELECT 1\n',
            expected_codes=("description-present",),
        ),
        LintNativeTestCase(
            description="paragraphs fault against canonical formatted length",
            contents=(
                'MODEL (description "'
                + "\n\n".join(f"paragraph {index}" for index in range(11))
                + '");\nSELECT 1\n'
            ),
            expected_codes=("description-length",),
        ),
        LintNativeTestCase(
            description="broken model header faults with parse error",
            contents="MODEL (\n  materialized table,\n  description\n);\nSELECT 1\n",
            expected_codes=("header-parse",),
        ),
        LintNativeTestCase(
            description="legacy scenario colon syntax faults with parse error",
            contents='SCENARIO (description: "legacy");\nSELECT 1\n',
            expected_codes=("header-parse",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_contents_when_linting_then_codes_match_expected(
    test_case: LintNativeTestCase,
) -> None:
    spans: tuple = scan_headers(contents=test_case.contents)
    violations: tuple = lint_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        headers=spans,
        config=DEFAULT_CONFIG,
    )
    assert tuple(violation.code for violation in violations) == test_case.expected_codes


@pytest.mark.parametrize(
    "test_case",
    [
        LintRemediationTestCase(
            description="header nested past the limit suggests flattening it",
            contents="MODEL (\n  tags " + "[" * 20_000 + "1" + "]" * 20_000 + "\n);\nSELECT 1\n",
            expected_remediations=("flatten the value so it nests at most 256 levels deep",),
        ),
        LintRemediationTestCase(
            description="other header syntax errors suggest correcting the header",
            contents="MODEL (\n  materialized table,\n  description\n);\nSELECT 1\n",
            expected_remediations=("Correct the MODEL() header syntax.",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unparseable_header_when_linting_then_remediation_matches_the_error(
    test_case: LintRemediationTestCase,
) -> None:
    violations: tuple = lint_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        headers=scan_headers(contents=test_case.contents),
        config=DEFAULT_CONFIG,
    )

    assert tuple(violation.remediation for violation in violations) == (
        test_case.expected_remediations
    )


@pytest.mark.parametrize(
    "test_case",
    [
        LintNativeTestCase(
            description="legacy scenario header is rejected by lint and compile",
            contents='SCENARIO (description: "legacy");\nSELECT 1\n',
            expected_codes=("header-parse",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_legacy_scenario_header_when_linting_and_compiling_then_both_reject_it(
    test_case: LintNativeTestCase,
) -> None:
    headers: tuple = scan_headers(contents=test_case.contents)

    violations: tuple = lint_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        headers=headers,
        config=DEFAULT_CONFIG,
    )

    assert tuple(violation.code for violation in violations) == test_case.expected_codes
    with pytest.raises(SqlScenarioParseError, match="unexpected ':' after key 'description'"):
        parse_sql_scenario_file(
            contents=test_case.contents,
            file_path=Path("tests/scenarios/example.sql"),
            relative_path=Path("tests/scenarios/example.sql"),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        FormatNativeTestCase(
            description="leading block comment relocates into description",
            contents="/* Orders daily fact. */\nMODEL (\n  materialized table\n);\nSELECT 1\n",
            expected_contents=(
                'MODEL (\n  description "Orders daily fact.",\n  materialized table\n);\nSELECT 1\n'
            ),
            expected_fault_codes=(),
        ),
        FormatNativeTestCase(
            description="leading line comments relocate into description",
            contents="-- One.\n-- Two.\nMODEL (\n  materialized table\n);\nSELECT 1\n",
            expected_contents=(
                'MODEL (\n  description "One. Two.",\n  materialized table\n);\nSELECT 1\n'
            ),
            expected_fault_codes=(),
        ),
        FormatNativeTestCase(
            description="existing description leaves comment untouched",
            contents=(
                '/* A note. */\nMODEL (\n  materialized table,\n  description "kept"\n);\n'
                "SELECT 1\n"
            ),
            expected_contents=(
                '/* A note. */\nMODEL (\n  materialized table,\n  description "kept"\n);\n'
                "SELECT 1\n"
            ),
            expected_fault_codes=(),
        ),
        FormatNativeTestCase(
            description="trailing whitespace in header is trimmed",
            contents='MODEL ( \n  materialized table,\t\n  description "d" \n);\nSELECT 1\n',
            expected_contents='MODEL (\n  materialized table,\n  description "d"\n);\nSELECT 1\n',
            expected_fault_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_contents_when_formatting_then_contents_match_expected(
    test_case: FormatNativeTestCase,
) -> None:
    updated: str
    faults: tuple
    updated, faults = format_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        config=DEFAULT_CONFIG,
    )
    assert updated == test_case.expected_contents
    assert tuple(fault.code for fault in faults) == test_case.expected_fault_codes


@pytest.mark.parametrize(
    "test_case",
    [
        FormatDescriptionTestCase(
            description="long description wraps at the configured line width",
            contents=(
                'MODEL (\n  description "Builds canonical customer records from every available '
                'source while retaining unmatched customers."\n);\nSELECT 1\n'
            ),
            line_width=60,
            expected_contents=(
                'MODEL (\n  description """\n'
                "    Builds canonical customer records from every available\n"
                "    source while retaining unmatched customers.\n"
                '  """\n);\nSELECT 1\n'
            ),
        ),
        FormatDescriptionTestCase(
            description="inconsistent authored wrapping normalizes deterministically",
            contents=(
                'MODEL (\n  description "Builds canonical customer\nrecords from every available '
                'source while retaining\nunmatched customers."\n);\nSELECT 1\n'
            ),
            line_width=60,
            expected_contents=(
                'MODEL (\n  description """\n'
                "    Builds canonical customer records from every available\n"
                "    source while retaining unmatched customers.\n"
                '  """\n);\nSELECT 1\n'
            ),
        ),
        FormatDescriptionTestCase(
            description="blank lines preserve intentional paragraphs",
            contents=(
                'MODEL (\n  description "Builds canonical customer records from every source.\n\n'
                'Retains unmatched customers for complete downstream coverage."\n);\nSELECT 1\n'
            ),
            line_width=60,
            expected_contents=(
                'MODEL (\n  description """\n'
                "    Builds canonical customer records from every source.\n\n"
                "    Retains unmatched customers for complete downstream\n"
                "    coverage.\n"
                '  """\n);\nSELECT 1\n'
            ),
        ),
        FormatDescriptionTestCase(
            description="backslashes in a description are kept as written",
            contents=(
                'MODEL (\n  description "Builds order summary.\\nRetains unmatched orders.'
                '\\n\\nSource \\u2192 canonical orders."\n);\nSELECT 1\n'
            ),
            line_width=100,
            expected_contents=(
                'MODEL (\n  description "Builds order summary.\\nRetains unmatched orders.'
                '\\n\\nSource \\u2192 canonical orders."\n);\nSELECT 1\n'
            ),
        ),
        FormatDescriptionTestCase(
            description="a backslash that is no escape is retained losslessly",
            contents=(
                'MODEL (\n  description "Builds order summary.\\u-123 retains text."\n);\n'
                "SELECT 1\n"
            ),
            line_width=100,
            expected_contents=(
                'MODEL (\n  description "Builds order summary.\\u-123 retains text."\n);\n'
                "SELECT 1\n"
            ),
        ),
        FormatDescriptionTestCase(
            description="short description remains compact",
            contents='MODEL (description "Canonical customer records.");\nSELECT 1\n',
            line_width=60,
            expected_contents='MODEL (description "Canonical customer records.");\nSELECT 1\n',
        ),
        FormatDescriptionTestCase(
            description="nested column description is laid out at its own depth",
            contents=(
                'MODEL (\n  columns (\n    customer_id (type BIGINT, description "A deliberately '
                'long column description that remains authored text.")\n  ),\n  description "Builds '
                'canonical customer records from every available source."\n);\nSELECT 1\n'
            ),
            line_width=60,
            expected_contents=(
                'MODEL (\n  columns (\n    customer_id (type BIGINT, description """\n'
                "      A deliberately long column description that remains\n"
                '      authored text.\n    """)\n  ),\n  description """\n'
                "    Builds canonical customer records from every available\n"
                '    source.\n  """\n);\nSELECT 1\n'
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_description_when_formatting_then_wrapping_is_uniform(
    test_case: FormatDescriptionTestCase,
) -> None:
    updated, faults = format_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        config=LintConfig(line_width=test_case.line_width),
    )

    assert updated == test_case.expected_contents
    assert faults == ()
    reformatted, reformat_faults = format_native_headers(
        contents=updated,
        file_path=FILE_PATH,
        config=LintConfig(line_width=test_case.line_width),
    )
    assert reformatted == updated
    assert reformat_faults == ()


@pytest.mark.parametrize(
    "test_case",
    [
        FormatNativeTestCase(
            description="manually wrapped leading comments reflow as prose",
            contents=(
                "-- "
                + "\n-- ".join(f"line {index}" for index in range(11))
                + "\nMODEL (\n  materialized table\n);\nSELECT 1\n"
            ),
            expected_contents=(
                'MODEL (\n  description "'
                + " ".join(f"line {index}" for index in range(11))
                + '",\n  materialized table\n);\nSELECT 1\n'
            ),
            expected_fault_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_wrapped_leading_comment_when_formatting_then_reflows_as_prose(
    test_case: FormatNativeTestCase,
) -> None:
    updated: str
    faults: tuple
    updated, faults = format_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        config=DEFAULT_CONFIG,
    )
    assert updated == test_case.expected_contents
    assert tuple(fault.code for fault in faults) == test_case.expected_fault_codes


@pytest.mark.parametrize(
    "test_case",
    [
        FormatNativeTestCase(
            description="relocated description round trips through the model parser",
            contents="-- The orders fact.\nMODEL (\n  materialized table\n);\nSELECT 1\n",
            expected_contents=(
                'MODEL (\n  description "The orders fact.",\n  materialized table\n);\nSELECT 1\n'
            ),
            expected_fault_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_relocated_model_when_parsing_then_description_round_trips(
    test_case: FormatNativeTestCase,
) -> None:
    updated: str
    faults: tuple
    updated, faults = format_native_headers(
        contents=test_case.contents,
        file_path=FILE_PATH,
        config=DEFAULT_CONFIG,
    )
    assert updated == test_case.expected_contents
    assert tuple(fault.code for fault in faults) == ()
    values, _query = parse_model_sql(contents=updated, file_path=FILE_PATH)
    assert values.get("description") == "The orders fact."
