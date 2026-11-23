"""An expansion never opens or extends a comment, built by the real CLI on DuckDB."""

from pathlib import Path

import pytest

from sqlbuild._native import (
    MICROBATCH_END_SENTINEL,
    MICROBATCH_START_SENTINEL,
    replace_cursor_markers,
    resolve_cursor_intrinsics,
)
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    CursorBoundaryTestCase,
    ExpansionBoundaryTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    query_duckdb_rows,
    write_boundary_project,
)

_NEG: dict[str, str] = {
    "models/_macros/python/neg.py": 'def neg():\n    """Return minus one."""\n    return "-1"\n'
}
_NOTED: dict[str, str] = {
    "models/_macros/python/noted.py": (
        'def noted():\n    """Return one with a note."""\n    return "1 -- one"\n'
    )
}


@pytest.mark.parametrize(
    "test_case",
    [
        ExpansionBoundaryTestCase(
            "a negative Python macro after a minus",
            "SELECT -@py.neg() AS x, 7 AS y",
            ((1, 7),),
            files=_NEG,
        ),
        ExpansionBoundaryTestCase(
            "a Python macro ending in a line comment",
            "SELECT @py.noted() AS x, 7 AS y",
            ((1, 7),),
            files=_NOTED,
        ),
        ExpansionBoundaryTestCase(
            "an empty Python macro between two minuses",
            "SELECT 5 -@py.blank()-2 AS x, 7 AS y",
            ((7, 7),),
            files={
                "models/_macros/python/blank.py": (
                    'def blank():\n    """Return nothing."""\n    return ""\n'
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a deliberate join",
            "SELECT 4 AS @py.stem()_count",
            ((4,),),
            files={
                "models/_macros/python/stem.py": (
                    'def stem():\n    """Return a stem."""\n    return "orders"\n'
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a negative constant",
            'SELECT -@const("minus_one") AS x, 7 AS y',
            ((1, 7),),
            files={"models/_constants/minus_one.sql": "CONSTANT (name minus_one, value -1);\n"},
        ),
        ExpansionBoundaryTestCase(
            "a negative enum member",
            'SELECT -@enum("sign").DOWN AS x, 7 AS y',
            ((1, 7),),
            files={"models/_enums/sign.sql": "ENUM (name sign, members (DOWN -1, UP 1));\n"},
        ),
        ExpansionBoundaryTestCase(
            "a negative variable",
            "SELECT -@@negative AS x, 7 AS y",
            ((1, 7),),
        ),
        ExpansionBoundaryTestCase(
            "a negative SQL macro",
            "SELECT -@sql.negative_one() AS x, 7 AS y",
            ((1, 7),),
            files={
                "models/_macros/sql/negative_one.sql": (
                    'MACRO (\n  description "Minus one",\n);\n\n-1\n'
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a negative audit argument",
            "SELECT 1 AS x, 7 AS y",
            ((1, 7),),
            model_config=", audits [x_is (value -1)]",
            files={
                "audits/generic/x_is.sql": (
                    "AUDIT ();\nSELECT * FROM @relation WHERE x <> -@value AND y > 0\n"
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a negative hook argument",
            "SELECT 1 AS x, 7 AS y",
            ((1, 7),),
            model_config=', post_hooks [sql("note_value", value: -1)]',
            files={
                "hooks/sql/note_value.sql": (
                    'HOOK (description "Read a value");\nSELECT -@value AS x, 7 AS y\n'
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a negative test parameter",
            'SELECT x, 7 AS y FROM __ref("amounts")',
            ((1, 7),),
            files={
                "models/amounts.sql": (
                    "MODEL (description 'Amounts.', materialized table);\nSELECT 1 AS x\n"
                ),
                "tests/unit/test_orders.sql": (
                    "TEST (parameters (value INTEGER), cases (single (value -1)));\n"
                    "WITH __ref__amounts AS (SELECT 1 AS x),\n"
                    "__expected__orders AS (\n"
                    '  SELECT -@param("value") AS x, 7 AS y\n'
                    ")\nSELECT 1\n"
                ),
            },
        ),
        ExpansionBoundaryTestCase(
            "a source expression ending in a line comment",
            'SELECT x, 7 AS y FROM __source("amounts") WHERE x = 2',
            ((2, 7),),
            files={
                "sources/amounts.yml": (
                    "sources:\n  - name: amounts\n    description: Amounts.\n"
                    '    expression: "(SELECT * FROM (VALUES (1), (2)) AS v(x)) -- amounts"\n'
                )
            },
        ),
        ExpansionBoundaryTestCase(
            "a SQL macro argument ending in a line comment",
            "SELECT @sql.same(2 + 3 -- five\n) AS x, 7 AS y",
            ((5, 7),),
            files={
                "models/_macros/sql/same.sql": (
                    'MACRO (\n  description "The amount",\n  arguments (amount INTEGER),\n);\n\n'
                    "@amount\n"
                )
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_expansion_beside_its_neighbour_when_building_then_neither_opens_a_comment(
    test_case: ExpansionBoundaryTestCase, tmp_path: Path
) -> None:
    write_boundary_project(project_dir=tmp_path, test_case=test_case)
    assert main(["--project-dir", str(tmp_path), "build"]) == 0, test_case.description
    assert (
        query_duckdb_rows(db_path=tmp_path / "orders.duckdb", sql="SELECT * FROM main.orders")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        ExpansionBoundaryTestCase(
            "a space between a minus and a negative",
            "select - @py.neg() as x, 7 as y",
            ((1, 7),),
            files=_NEG,
        ),
        ExpansionBoundaryTestCase(
            "a newline after a macro ending in a comment",
            "select @py.noted()\n as x, 7 as y",
            ((1, 7),),
            files=_NOTED,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_formatted_expansion_site_when_building_then_results_do_not_change(
    test_case: ExpansionBoundaryTestCase, tmp_path: Path
) -> None:
    write_boundary_project(project_dir=tmp_path, test_case=test_case)
    built: list[tuple[tuple[object, ...], ...]] = []
    for command in (("build",), ("format",), ("build", "--full-refresh")):
        assert main(["--project-dir", str(tmp_path), *command]) == 0, command
        built.append(
            query_duckdb_rows(db_path=tmp_path / "orders.duckdb", sql="SELECT * FROM main.orders")
        )
    assert built == [test_case.expected_rows] * 3


@pytest.mark.parametrize(
    "test_case",
    [
        CursorBoundaryTestCase("a negative start", "-5", "SELECT x- -5, 'a-5', x-9"),
        CursorBoundaryTestCase("a positive start", "5", "SELECT x-5, 'a5', x-9"),
    ],
    ids=lambda case: case.description,
)
def test_given_a_cursor_bound_after_a_minus_when_substituted_then_it_opens_no_comment(
    test_case: CursorBoundaryTestCase,
) -> None:
    intrinsic: str = "SELECT x-__cursor_start(), 'a{start}', x-__cursor_end()"
    resolved, _ = resolve_cursor_intrinsics(
        sql=intrinsic.format(start=test_case.start), start_sql=test_case.start, end_sql="9"
    )
    marked: str = (
        f"SELECT x-{MICROBATCH_START_SENTINEL}, "
        f"'a{MICROBATCH_START_SENTINEL}', x-{MICROBATCH_END_SENTINEL}"
    )
    replaced: str = replace_cursor_markers(sql=marked, start_sql=test_case.start, end_sql="9")
    assert (resolved, replaced) == (test_case.expected_sql, test_case.expected_sql)
