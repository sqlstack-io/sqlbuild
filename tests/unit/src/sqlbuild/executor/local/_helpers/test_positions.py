"""Where a refusal is placed, and the help SQLBuild gives for it."""

from types import SimpleNamespace

import pytest

from sqlbuild.executor.local._helpers.positions import macro_place, sqlbuild_help
from sqlbuild.executor.local.models import MacroPlace
from tests.unit.src.sqlbuild.executor.local._helpers._test_types import (
    MacroPlaceTestCase,
    SqlbuildHelpTestCase,
)

_HASHING_MACRO: str = (
    "def status_hash(column):\n"
    '    """Hash a status column."""\n'
    "    # HASH(...) differs between engines\n"
    '    return f"HASH({column})"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        SqlbuildHelpTestCase(
            description="a known difference is allowed in SQLBuild's own settings",
            help_text=(
                "allow it with hash = true under [testing.local] allow in sqlbuild_project.toml;"
                " `sqlquack explain hash` says when it is safe"
            ),
            expected_help=(
                "allow it for local tests in sqlbuild_project.toml with"
                " `[testing.local] allow = { hash = true }`;"
                " `sqlquack explain hash` says when it is safe"
            ),
        ),
        SqlbuildHelpTestCase(
            description="SQLQuack's source files are SQLBuild's",
            help_text="declare it in a source file of the project (`sources/*.yml`)",
            expected_help="declare it in a source file of the project (`sql/sources/`)",
        ),
        SqlbuildHelpTestCase(
            description="other help is unchanged",
            help_text="run this test on the warehouse",
            expected_help="run this test on the warehouse",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sqlquack_help_when_shown_then_points_at_sqlbuild_settings(
    test_case: SqlbuildHelpTestCase,
) -> None:
    assert sqlbuild_help(test_case.help_text) == test_case.expected_help


@pytest.mark.parametrize(
    "test_case",
    [
        MacroPlaceTestCase(
            description="a function is found where the macro calls it, not in its docstring",
            call="@py.status_hash",
            token="HASH(status)",
            raw_source=_HASHING_MACRO,
            expected_place=MacroPlace(
                path="python/macros/hashing.py",
                line=4,
                column=14,
                snippet='    return f"HASH({column})"',
            ),
        ),
        MacroPlaceTestCase(
            description="a construct the macro's source does not write has no place",
            call="@py.status_hash",
            token="IS_DOUBLE(payload)",
            raw_source=_HASHING_MACRO,
            expected_place=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_refusal_in_macro_output_when_placing_then_finds_the_macro_source_line(
    test_case: MacroPlaceTestCase,
) -> None:
    macro: SimpleNamespace = SimpleNamespace(
        raw_source=test_case.raw_source, relative_path="python/macros/hashing.py"
    )

    assert (
        macro_place(call=test_case.call, token=test_case.token, macros={"status_hash": macro})
        == test_case.expected_place
    )
