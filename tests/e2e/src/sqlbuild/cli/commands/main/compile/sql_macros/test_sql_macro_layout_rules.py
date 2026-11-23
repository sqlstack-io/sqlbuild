"""E2E coverage for declaration layout Rules over a macro folder's python/nodes/ and sql/ kind folders."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCommandTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    output,
    python_normalise_macros,
    run_project,
    sql_text_macros,
    with_rules,
)

_LAYOUT_RULES: dict[str, str] = with_rules("SQBRDECLARATION304", "SQBRDECLARATION306")


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCommandTestCase(
            description="six_python_and_seven_sql_files_fit_their_own_folders",
            files={
                **_LAYOUT_RULES,
                **python_normalise_macros(["trim", "upper", "pad", "strip", "fold", "squash"]),
                **sql_text_macros(
                    "normalise", ["lower", "title", "slug", "ascii", "spaces", "digits", "letters"]
                ),
            },
            command=("compile", "--no-cache"),
            expected_returncode=0,
            expected_fragments=("0 errors",),
        ),
        SqlMacroCommandTestCase(
            description="eleven_sql_files_overflow_their_real_folder",
            files={
                **_LAYOUT_RULES,
                **sql_text_macros("normalise", [f"tidy_{word}" for word in "abcdefghijk"]),
            },
            command=("compile", "--no-cache"),
            expected_returncode=1,
            expected_fragments=(
                "error[SQBRDECLARATION304]",
                'declaration container "sql/macros/sql/normalise" has 11 files',
            ),
        ),
        SqlMacroCommandTestCase(
            description="sql_files_sharing_a_prefix_move_into_a_subfolder_of_sql",
            files={
                **_LAYOUT_RULES,
                **sql_text_macros("text", ["name_first", "name_last"]),
            },
            command=("compile", "--no-cache"),
            expected_returncode=1,
            expected_fragments=(
                "error[SQBRDECLARATION306]",
                "2 sibling SQL macro files",
                "Move these files into sql/macros/sql/text/name/",
                "each macro keeps its name",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_kind_folders_when_checking_layout_rules_then_each_folder_counts_alone(
    test_case: SqlMacroCommandTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=test_case.command)

    assert result.returncode == test_case.expected_returncode, output(result)
    for fragment in test_case.expected_fragments:
        assert fragment in output(result)
