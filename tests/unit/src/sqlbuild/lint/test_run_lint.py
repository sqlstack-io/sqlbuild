"""Unit tests for lint and format orchestration over a synthetic project."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    LintConfig,
    LintRunResult,
    LintSeverity,
    LintViolation,
    reject_lint_suppressions,
    run_lint,
)
from tests.unit.src.sqlbuild.lint._test_types import (
    FixtureCeremonyFormatTestCase,
    FormatDescriptionResolutionTestCase,
    FormatNewlineTestCase,
    FormatProjectTestCase,
    InlineDirectiveTestCase,
    LintBehaviorTestCase,
    LintProjectTestCase,
)
from tests.unit.src.sqlbuild.lint.helpers import (
    format_project_json,
    formatted_files,
    violation_reports,
)

LINT_ENGINE_SQLBUILD: str = "sqlbuild"

CLEAN_MODEL: str = 'MODEL (\n  materialized table,\n  description "ok"\n);\nSELECT 1 AS x FROM t\n'
NO_DESCRIPTION_MODEL: str = "MODEL (\n  materialized table\n);\nSELECT 1 AS x FROM t\n"
PROJECT_TOML: str = 'name = "demo"\nadapter = "duckdb"\n'
_FIXTURE_CTES: str = (
    "with __source__orders as (select 1 as id),\n__expected__orders as (select 1 as id)"
)
_FORMATTED_FIXTURE_CTES: str = (
    "WITH __source__orders AS (\n  SELECT 1 AS id\n),\n\n"
    "__expected__orders AS (\n  SELECT 1 AS id\n)"
)


@pytest.mark.parametrize(
    "test_case",
    [
        LintProjectTestCase(
            description="model without description faults",
            files={"models/no_description.sql": NO_DESCRIPTION_MODEL},
            expected_fault_codes=(("no_description.sql", "description-present"),),
        ),
        LintProjectTestCase(
            description="clean project reports no violations",
            files={"models/fine.sql": CLEAN_MODEL},
            expected_fault_codes=(),
        ),
        LintProjectTestCase(
            description="schema declarations are discovered for linting",
            files={"schemas/order.sql": "SCHEMA (name order, columns (id (type INTEGER)));\n"},
            expected_fault_codes=(),
        ),
        LintProjectTestCase(
            description="scoped declaration file preserves every header",
            files={
                "models/_enums/status.sql": (
                    'ENUM (name "status", members [OPEN, CLOSED]);\n'
                    'ENUM (name "tier", members [ONE, TWO]);\n'
                )
            },
            expected_fault_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_synthetic_project_when_linting_then_results_match_expected(
    test_case: LintProjectTestCase, tmp_path: Path
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    relative_path: str
    contents: str
    for relative_path, contents in {**test_case.files, **test_case.extra_files}.items():
        target: Path = tmp_path / relative_path
        _ = target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_text(contents, encoding="utf-8")
    result: LintRunResult = run_lint(
        project_dir=tmp_path,
        config=LintConfig(),
    )
    assert result.files_checked == test_case.expected_files_checked
    codes: set = {(violation.file_path.name, violation.code) for violation in result.violations}
    expected_code: tuple[str, str]
    for expected_code in test_case.expected_fault_codes:
        assert expected_code in codes


@pytest.mark.parametrize(
    "test_case",
    [
        FormatProjectTestCase(
            description="leading comment is relocated into the description",
            files={
                "models/commented.sql": (
                    "-- A comment.\nMODEL (\n  materialized table\n);\nSELECT 1 AS x FROM t\n"
                )
            },
            expected_written_fragments={"models/commented.sql": 'description "A comment.",'},
            expected_fault_codes=(),
            expected_formatted_count=1,
        ),
        FormatProjectTestCase(
            description="missing description is a fault and the file is still formatted",
            files={"models/no_description.sql": NO_DESCRIPTION_MODEL},
            expected_written_fragments={},
            expected_fault_codes=("description-present",),
            expected_formatted_count=1,
        ),
        FormatProjectTestCase(
            description="SQLBuild reference calls survive canonical formatting",
            files={
                "models/reference.sql": (
                    'MODEL (description "Reference formatting.");\n'
                    'select order_id from __ref("orders")\n'
                )
            },
            expected_written_fragments={
                "models/reference.sql": 'FROM __ref("orders")',
            },
            expected_fault_codes=(),
            expected_formatted_count=1,
        ),
        FormatProjectTestCase(
            description="measurement audit queries format as separate bodies",
            files={
                "audits/generic/measurement.sql": (
                    "AUDIT (evaluation measurement, value measured_value);\n"
                    "MEASURE (SELECT COUNT(*) AS measured_value FROM @relation);\n"
                    "EVIDENCE (SELECT * FROM @relation WHERE @condition);\n"
                )
            },
            expected_written_fragments={"audits/generic/measurement.sql": "MEASURE (SELECT"},
            expected_fault_codes=(),
            expected_formatted_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_synthetic_project_when_formatting_then_results_match_expected(
    test_case: FormatProjectTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        target: Path = tmp_path / relative_path
        _ = target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_text(contents, encoding="utf-8")
    report: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)
    assert len(formatted_files(report)) == test_case.expected_formatted_count
    remaining: tuple = tuple(
        (violation["code"], violation["severity"]) for violation in violation_reports(report)
    )
    fault_code: str
    for fault_code in test_case.expected_fault_codes:
        assert (fault_code, "fault") in remaining
    relative_path: str
    fragment: str
    for relative_path, fragment in test_case.expected_written_fragments.items():
        written: str = (tmp_path / relative_path).read_text(encoding="utf-8")
        assert fragment in written


@pytest.mark.parametrize(
    "test_case",
    [
        FormatDescriptionResolutionTestCase(
            description="path default describes only the models under its path",
            project_toml=PROJECT_TOML + '\n[path_defaults.marts]\ndescription = "Mart model"\n',
            files={
                "models/marts/orders.sql": "MODEL ();\nSELECT 1 AS x\n",
                "models/staging/stg_orders.sql": "MODEL ();\nSELECT 1 AS x\n",
            },
            expected_description_faults=(("stg_orders.sql", "description-present"),),
        ),
        FormatDescriptionResolutionTestCase(
            description="description resolution never imports project Python",
            project_toml=PROJECT_TOML + '\n[path_defaults.marts]\ndescription = "Mart model"\n',
            files={
                "providers/orders_api.py": 'raise RuntimeError("project Python was imported")\n',
                "python/tasks/refresh.py": 'raise RuntimeError("project Python was imported")\n',
                "models/marts/orders.sql": "MODEL ();\nSELECT 1 AS x\n",
                "models/staging/stg_orders.sql": "MODEL ();\nSELECT 1 AS x\n",
            },
            expected_description_faults=(("stg_orders.sql", "description-present"),),
        ),
        FormatDescriptionResolutionTestCase(
            description="model schema description satisfies the header only when present",
            project_toml=PROJECT_TOML,
            files={
                "schemas/orders.sql": (
                    'SCHEMA (name order_shape, description "One row per order", '
                    "columns (id (type INTEGER)));\n"
                    "SCHEMA (name bare_shape, columns (id (type INTEGER)));\n"
                ),
                "models/orders.sql": "MODEL (model_schema order_shape);\nSELECT 1 AS id\n",
                "models/returns.sql": "MODEL (model_schema bare_shape);\nSELECT 1 AS id\n",
            },
            expected_description_faults=(("returns.sql", "description-present"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inherited_descriptions_when_formatting_then_faults_match_compile_resolution(
    test_case: FormatDescriptionResolutionTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(test_case.project_toml, encoding="utf-8")
    relative_path: str
    contents: str
    for relative_path, contents in test_case.files.items():
        target: Path = tmp_path / relative_path
        _ = target.parent.mkdir(parents=True, exist_ok=True)
        _ = target.write_text(contents, encoding="utf-8")

    report: dict[str, object] = format_project_json(
        project_dir=tmp_path, capsys=capsys, arguments=("--check",)
    )

    assert tuple(
        sorted(
            (Path(str(violation["file"])).name, violation["code"], violation["severity"])
            for violation in violation_reports(report)
        )
    ) == tuple((*fault, "fault") for fault in test_case.expected_description_faults)


@pytest.mark.parametrize(
    "test_case",
    [
        FormatNewlineTestCase(
            description="CRLF newline style is preserved",
            contents=(
                b"-- Description.\r\nMODEL (\r\n  materialized table  \r\n);\r\nSELECT 1\r\n"
            ),
            expected_newline=b"\r\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_newline_style_when_formatting_then_style_is_preserved(
    test_case: FormatNewlineTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    target: Path = tmp_path / "models" / "crlf.sql"
    _ = target.parent.mkdir(parents=True)
    target.write_bytes(test_case.contents)
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")

    _ = format_project_json(project_dir=tmp_path, capsys=capsys)
    second: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)

    written: bytes = target.read_bytes()
    assert test_case.expected_newline in written
    assert written.count(test_case.expected_newline) == written.count(b"\n")
    assert second["formatted_files"] == []


@pytest.mark.parametrize(
    "test_case",
    [LintBehaviorTestCase(description="Unicode source span mapping", expected_value=(2, 44))],
    ids=lambda case: case.description,
)
def test_given_non_ascii_sql_when_native_linting_then_span_maps_to_authored_column(
    test_case: LintBehaviorTestCase,
    tmp_path: Path,
) -> None:
    _ = test_case
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    target: Path = tmp_path / "models" / "unicode.sql"
    _ = target.parent.mkdir(parents=True)
    _ = target.write_text(
        "MODEL (description \"ok\");\nSELECT 'é' AS label FROM items WHERE value = NULL\n",
        encoding="utf-8",
    )

    result: LintRunResult = run_lint(
        project_dir=tmp_path,
        config=LintConfig(dialect="duckdb"),
    )

    assert tuple(item.code for item in result.violations) == ("SQLCLEAN001",)
    assert (result.violations[0].line, result.violations[0].column) == test_case.expected_value
    assert (result.violations[0].end_line, result.violations[0].end_column) == (2, 45)
    assert result.violations[0].remediation == ("Use IS NULL or IS NOT NULL when testing for NULL.")


@pytest.mark.parametrize(
    "test_case",
    [LintBehaviorTestCase(description="canonical idempotent SQL body", expected_value=1)],
    ids=lambda case: case.description,
)
def test_given_comment_free_body_when_formatting_twice_then_output_is_canonical_and_idempotent(
    test_case: LintBehaviorTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = test_case
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    target: Path = tmp_path / "models" / "messy.sql"
    target.parent.mkdir()
    _ = target.write_text(
        'MODEL (description "ok");\nselect a,b from items where a=1\n',
        encoding="utf-8",
    )

    first: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)
    second: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)

    assert len(formatted_files(first)) == test_case.expected_value
    assert second["formatted_files"] == []
    assert "SELECT\n  a,\n  b\nFROM items\nWHERE a = 1\n" in target.read_text(encoding="utf-8")


@pytest.mark.parametrize(
    "test_case",
    [
        LintBehaviorTestCase(
            description="canonical commented SQL body",
            expected_value="/* preserve exactly */",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_commented_body_when_formatting_then_comment_is_preserved_in_canonical_output(
    test_case: LintBehaviorTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    _ = test_case
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    contents: str = 'MODEL (description "ok");\nSELECT a, /* preserve exactly */ b FROM items\n'
    target: Path = tmp_path / "models" / "commented_body.sql"
    target.parent.mkdir()
    _ = target.write_text(contents, encoding="utf-8")

    report: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)

    assert report["formatted_files"] == [str(target)]
    written: str = target.read_text(encoding="utf-8")
    assert str(test_case.expected_value) in written
    assert written == (
        'MODEL (description "ok");\nSELECT\n  a, /* preserve exactly */\n  b\nFROM items\n'
    )


@pytest.mark.parametrize(
    "test_case",
    [
        FixtureCeremonyFormatTestCase(
            description="test without a trailing select stays without one",
            relative_path="tests/unit/test_orders.sql",
            contents=f"TEST();\n{_FIXTURE_CTES}\n",
            expected_contents=f"TEST();\n{_FORMATTED_FIXTURE_CTES}\n",
        ),
        FixtureCeremonyFormatTestCase(
            description="semicolon and trailing comment after the last cte are kept",
            relative_path="tests/unit/test_orders.sql",
            contents=f"TEST();\n{_FIXTURE_CTES}; -- done\n",
            expected_contents=f"TEST();\n{_FORMATTED_FIXTURE_CTES}; -- done\n",
        ),
        FixtureCeremonyFormatTestCase(
            description="explicit header and trailing select are kept",
            relative_path="tests/unit/test_orders.sql",
            contents=f"TEST();\n{_FIXTURE_CTES}\nselect 1\n",
            expected_contents=f"TEST();\n{_FORMATTED_FIXTURE_CTES}\n\nSELECT 1\n",
        ),
        FixtureCeremonyFormatTestCase(
            description="scenario without a trailing select stays without one",
            relative_path="tests/scenarios/orders.sql",
            contents=(
                'SCENARIO (description "Orders");\n'
                + _FIXTURE_CTES.replace("__source__", "__ref__")
                + "\n"
            ),
            expected_contents=(
                'SCENARIO (description "Orders");\n'
                + _FORMATTED_FIXTURE_CTES.replace("__source__", "__ref__")
                + "\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fixture_ceremony_variants_when_formatting_and_linting_then_forms_are_kept(
    test_case: FixtureCeremonyFormatTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    target: Path = tmp_path / test_case.relative_path
    target.parent.mkdir(parents=True)
    _ = target.write_text(test_case.contents, encoding="utf-8")

    formatted: dict[str, object] = format_project_json(project_dir=tmp_path, capsys=capsys)
    linted: LintRunResult = run_lint(project_dir=tmp_path, config=LintConfig(dialect="duckdb"))

    assert formatted["violations"] == []
    assert target.read_text(encoding="utf-8") == test_case.expected_contents
    assert linted.violations == ()


@pytest.mark.parametrize(
    "test_case",
    [
        InlineDirectiveTestCase(
            description="matching directive keeps the finding and fails",
            query="SELECT value FROM items LIMIT 1\n",
            expected_violations=(("SQBRSUPPRESSION000", 2, "fault"), ("SQLCLEAN004", 3, "warning")),
        ),
        InlineDirectiveTestCase(
            description="directive without a finding still fails",
            query="SELECT value FROM items ORDER BY value LIMIT 1\n",
            expected_violations=(("SQBRSUPPRESSION000", 2, "fault"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inline_directive_when_linting_then_it_is_a_fault_printing_the_exception_entry(
    test_case: InlineDirectiveTestCase,
    tmp_path: Path,
) -> None:
    _ = (tmp_path / "sqlbuild_project.toml").write_text(PROJECT_TOML, encoding="utf-8")
    target: Path = tmp_path / "models" / "sample.sql"
    target.parent.mkdir()
    _ = target.write_text(
        'MODEL (description "ok");\n'
        "-- sqb: ignore SQLCLEAN004 because this fixture intentionally samples one row\n"
        f"{test_case.query}",
        encoding="utf-8",
    )

    result: LintRunResult = run_lint(
        project_dir=tmp_path,
        config=LintConfig(dialect="duckdb"),
    )

    assert (
        tuple((item.code, item.line, str(item.severity)) for item in result.violations)
        == test_case.expected_violations
    )
    remediation: str = result.violations[0].remediation or ""
    assert (
        "[[rules.rule_exceptions]]\n"
        '            rule = "SQLCLEAN004"\n'
        '            path = "models/sample.sql"\n'
        '            reason = "this fixture intentionally samples one row"'
    ) in remediation


@pytest.mark.parametrize(
    "test_case",
    [
        LintBehaviorTestCase(
            description="mandatory fault directive",
            expected_value={"SQBRSUPPRESSION000", "description-present"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_header_fault_directive_when_rejecting_then_mandatory_fault_remains(
    test_case: LintBehaviorTestCase,
    tmp_path: Path,
) -> None:
    target: Path = tmp_path / "models" / "missing_description.sql"
    contents: str = (
        "-- sqb: ignore description-present because this must not bypass compiler policy\n"
        "MODEL (materialized table);\n"
    )
    mandatory: LintViolation = LintViolation(
        file_path=target,
        line=2,
        column=1,
        code="description-present",
        message="MODEL header must include a description",
        severity=LintSeverity.FAULT,
        engine=LINT_ENGINE_SQLBUILD,
    )

    result: list[LintViolation] = reject_lint_suppressions(
        [mandatory], {target: contents}, tmp_path
    )

    assert {item.code for item in result} == test_case.expected_value

    assert {item.code for item in result} == test_case.expected_value
