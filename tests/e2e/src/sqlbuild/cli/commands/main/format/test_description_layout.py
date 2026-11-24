"""E2E coverage of description layout and quote repair by `sqb format`."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.format._test_types import (
    DescriptionLayoutTestCase,
    MacroLayoutTestCase,
    QuoteFixTestCase,
    RefusedQuoteFixTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.format.helpers import (
    stored_column_descriptions,
    stored_descriptions,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_FILE: str = (
    'name = "shop"\nadapter = "duckdb"\n\n[connection]\ndatabase = "shop.duckdb"\n\n'
    "[scopes]\nenforce_placement = false\n"
)
_BODY: str = "\nSELECT 1 AS customer_id\n"


@pytest.mark.parametrize(
    "test_case",
    [
        DescriptionLayoutTestCase(
            description="continuation_lines_at_column_zero_become_an_indented_block",
            header=(
                "MODEL (\n"
                "  description \"True when a name starts with a woman's title: Mme, Mlle, Miss, "
                "Mrs, Ms. Any case,\n"
                "with or without a dot ('MISS ANNA PARKER', 'Mrs. C. O'Brien', 'Ms.Robinson').\",\n"
                ");\n"
            ),
            expected_header=(
                "MODEL (\n"
                '  description """\n'
                "    True when a name starts with a woman's title: Mme, Mlle, Miss, Mrs, Ms. Any "
                "case, with or\n"
                "    without a dot ('MISS ANNA PARKER', 'Mrs. C. O'Brien', 'Ms.Robinson').\n"
                '  """,\n'
                ");\n"
            ),
        ),
        DescriptionLayoutTestCase(
            description="a_short_wrapped_description_is_joined",
            header='MODEL (\n  description "One row per\n    customer.",\n);\n',
            expected_header='MODEL (\n  description "One row per customer.",\n);\n',
        ),
        DescriptionLayoutTestCase(
            description="a_short_description_with_quotes_stays_one_triple_quoted_line",
            header='MODEL (\n  description """Matches "Miss" titles.""",\n);\n',
            expected_header='MODEL (\n  description """Matches "Miss" titles.""",\n);\n',
        ),
        DescriptionLayoutTestCase(
            description="a_long_description_with_quotes_becomes_a_block",
            header=(
                "MODEL (\n"
                '  description """True when a name starts with a title, with or without a dot '
                '("MISS ANNA PARKER", "Mrs. C. O\'Brien", "Ms.Robinson").""",\n'
                ");\n"
            ),
            expected_header=(
                "MODEL (\n"
                '  description """\n'
                "    True when a name starts with a title, with or without a dot "
                '("MISS ANNA PARKER", "Mrs. C.\n'
                '    O\'Brien", "Ms.Robinson").\n'
                '  """,\n'
                ");\n"
            ),
        ),
        DescriptionLayoutTestCase(
            description="a_long_column_description_becomes_a_block_at_its_depth",
            header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                "  columns (\n"
                '    customer_id (type INTEGER, description "The customer key, unique across '
                'every channel the shop sells through, assigned at sign-up."),\n'
                "  ),\n"
                ");\n"
            ),
            expected_header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                "  columns (\n"
                '    customer_id (type INTEGER, description """\n'
                "      The customer key, unique across every channel the shop sells through, "
                "assigned at sign-up.\n"
                '    """),\n'
                "  ),\n"
                ");\n"
            ),
        ),
        DescriptionLayoutTestCase(
            description="a_short_wrapped_column_description_is_joined",
            header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                '  columns (customer_id (type INTEGER, description """The customer\n'
                '    key.""")),\n'
                ");\n"
            ),
            expected_header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                '  columns (customer_id (type INTEGER, description "The customer key.")),\n'
                ");\n"
            ),
        ),
        DescriptionLayoutTestCase(
            description="a_long_audit_description_becomes_a_block_at_its_depth",
            header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                '  audits [not_null (column customer_id, description "Every customer must have '
                'a key, otherwise downstream joins silently drop rows from reports.")],\n'
                ");\n"
            ),
            expected_header=(
                "MODEL (\n"
                '  description "Customers.",\n'
                "  audits [\n"
                "    not_null (\n"
                "      column customer_id,\n"
                '      description """\n'
                "        Every customer must have a key, otherwise downstream joins silently drop "
                "rows from reports.\n"
                '      """,\n'
                "    ),\n"
                "  ],\n"
                ");\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_description_when_formatting_then_layout_is_canonical_and_text_unchanged(
    test_case: DescriptionLayoutTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/models/customers.sql": test_case.header + _BODY,
        },
    )
    before: dict[str, str] = stored_descriptions(project_dir)
    columns_before: dict[str, str] = stored_column_descriptions(project_dir)

    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format")
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--check")
    )

    assert formatted.returncode == 0, formatted.stdout + formatted.stderr
    assert checked.returncode == 0, checked.stdout + checked.stderr
    assert ", 0 changed)" in checked.stderr
    model: str = (project_dir / "sql/models/customers.sql").read_text()
    assert model == test_case.expected_header + _BODY
    assert stored_descriptions(project_dir) == before
    assert stored_column_descriptions(project_dir) == columns_before


@pytest.mark.parametrize(
    "test_case",
    [
        QuoteFixTestCase(
            description="model_and_wrapped_macro_descriptions",
            files={
                "sql/models/customers.sql": (
                    'MODEL (\n  description "Matches a title (\\"Miss\\").",\n);\n'
                    "\nSELECT @sql.title('Miss') AS title\n"
                ),
                "sql/macros/sql/title.sql": (
                    "MACRO (\n"
                    "  description \"True when a name starts with a woman's title: Mme, Mlle, "
                    "Miss, Mrs, Ms. Any case,\n"
                    'with or without a dot (\\"MISS ANNA PARKER\\", \\"Mrs. C. O\'Brien\\").",\n'
                    "  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n\n@value\n"
                ),
            },
            expected_fixes=[
                ("description-quotes", "customers.sql", 2, "applied"),
                ("description-quotes", "title.sql", 2, "applied"),
            ],
            expected_starts={
                "sql/models/customers.sql": (
                    'MODEL (\n  description """Matches a title ("Miss").""",\n);\n'
                ),
                "sql/macros/sql/title.sql": (
                    "MACRO (\n"
                    '  description """\n'
                    "    True when a name starts with a woman's title: Mme, Mlle, Miss, Mrs, Ms. "
                    "Any case, with or\n"
                    '    without a dot ("MISS ANNA PARKER", "Mrs. C. O\'Brien").\n'
                    '  """,\n'
                ),
            },
            expected_description='Matches a title ("Miss").',
        ),
        QuoteFixTestCase(
            description="model_and_macro_descriptions_doubling_quotes",
            files={
                "sql/models/customers.sql": (
                    'MODEL (\n  description "say ""hi"" now",\n);\n'
                    "\nSELECT @sql.title('Miss') AS title\n"
                ),
                "sql/macros/sql/title.sql": (
                    'MACRO (\n  description "say ""hi"" now",\n'
                    "  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n\n@value\n"
                ),
            },
            expected_fixes=[
                ("description-quotes", "customers.sql", 2, "applied"),
                ("description-quotes", "title.sql", 2, "applied"),
            ],
            expected_starts={
                "sql/models/customers.sql": 'MODEL (\n  description """say "hi" now""",\n);\n',
                "sql/macros/sql/title.sql": 'MACRO (\n  description """say "hi" now""",\n',
            },
            expected_description='say "hi" now',
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_descriptions_escaping_quotes_when_format_fix_then_triple_quoted_real_quotes(
    test_case: QuoteFixTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={"sqlbuild_project.toml": _PROJECT_FILE, **test_case.files},
    )
    failed: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )

    fixed: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--fix", "--json")
    )
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )
    checked: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--check")
    )

    assert failed.returncode != 0
    payload: dict[str, Any] = json.loads(fixed.stdout)
    assert (
        sorted(
            (fix["code"], Path(fix["file"]).name, fix["line"], fix["status"])
            for fix in payload["rule_fixes"]
        )
        == test_case.expected_fixes
    )
    assert any(violation["code"] == "format-fix-skipped" for violation in payload["violations"])
    assert {
        path: (project_dir / path).read_text()[: len(start)]
        for path, start in test_case.expected_starts.items()
    } == test_case.expected_starts
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert checked.returncode == 0, checked.stdout + checked.stderr
    assert stored_descriptions(project_dir)["customers"] == test_case.expected_description


@pytest.mark.parametrize(
    "test_case",
    [
        MacroLayoutTestCase(
            description="long_single_quoted_description_with_a_doubled_quote",
            macro=(
                "MACRO (\n  description 'it''s a "
                + " ".join(["word"] * 30)
                + " end',\n  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n\n"
                "LOWER(@value)\n"
            ),
            expected_macro=(
                'MACRO (\n  description """\n'
                "    it's a "
                + " ".join(["word"] * 18)
                + "\n    "
                + " ".join(["word"] * 12)
                + ' end\n  """,\n  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n\n'
                "LOWER(@value)\n"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_description_when_formatting_then_its_text_survives_and_it_compiles(
    test_case: MacroLayoutTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sql/macros/sql/lowered.sql": test_case.macro,
            "sql/models/customers.sql": (
                "MODEL (description \"Customers.\");\n\nSELECT @sql.lowered('Miss') AS title\n"
            ),
        },
    )

    formatted: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format")
    )
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "compile")
    )

    assert formatted.returncode == 0, formatted.stdout + formatted.stderr
    assert (project_dir / "sql/macros/sql/lowered.sql").read_text() == test_case.expected_macro
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        RefusedQuoteFixTestCase(
            description="macro_with_another_header_error",
            path="sql/macros/sql/title.sql",
            contents=(
                'MACRO (\n  description "say \\"hi\\"",\n  arguments (value VARCHAR),\n'
                "  returns VARCHAR,\n  unknown_key x,\n);\n\n@value\n"
            ),
            expected_fix=("description-quotes", "title.sql", 2, "refused"),
        ),
        RefusedQuoteFixTestCase(
            description="model_with_another_header_error",
            path="sql/models/orders.sql",
            contents=(
                'MODEL (\n  description "say \\"hi\\"",\n  tags ["a" "b",\n);\n'
                "\nSELECT 1 AS order_id\n"
            ),
            expected_fix=("description-quotes", "orders.sql", 2, "refused"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_file_still_broken_after_quote_fix_when_format_fix_then_it_is_left_and_refused(
    test_case: RefusedQuoteFixTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="shop",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            test_case.path: test_case.contents,
            "sql/models/customers.sql": (
                'MODEL (\n  description "Matches \\"Miss\\".",\n);\n\nSELECT 1 AS customer_id\n'
            ),
        },
    )

    fixed: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project_dir, command=("--no-color", "format", "--fix", "--json")
    )

    payload: dict[str, Any] = json.loads(fixed.stdout)
    reported: list[tuple[str, str, int, str]] = [
        (fix["code"], Path(fix["file"]).name, fix["line"], fix["status"])
        for fix in payload["rule_fixes"]
    ]
    assert test_case.expected_fix in reported
    assert ("description-quotes", "customers.sql", 2, "applied") in reported
    assert (project_dir / test_case.path).read_text() == test_case.contents
