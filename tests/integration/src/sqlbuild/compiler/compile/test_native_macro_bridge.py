"""The native macro bridge renders compile inputs as expected, running each call class once."""

from __future__ import annotations

import random
from pathlib import Path

import pytest

from sqlbuild.compiler.macro_bridge.classes.macro_bridge import MacroBridge
from tests.integration.src.sqlbuild.compiler.compile._test_types import (
    FailingMacroProjectParityTestCase,
    MacroBridgeFailureTestCase,
    MacroBridgeMemoTestCase,
    MacroBridgeParityTestCase,
    MacroExpansionDifferentialTestCase,
    MacroExpansionEdgeTestCase,
)
from tests.integration.src.sqlbuild.compiler.compile.helpers import (
    MACRO_BRIDGE_PROJECT_FILES,
    MACRO_CALL_LOG_ENV_VAR,
    FailingMacroParity,
    comparable_capture,
    expansion_outcome,
    failing_macro_parity,
    generated_failing_macro_project,
    random_macro_sql,
    render_compile_inputs,
    render_error,
    write_project,
)
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    read_golden,
)

_FAILURE_BASE_FILES: dict[str, str] = {
    "sqlbuild_project.toml": MACRO_BRIDGE_PROJECT_FILES["sqlbuild_project.toml"],
    "sql/macros/python/common.py": (
        "def cents(column: str) -> str:\n"
        '    """Convert an amount to cents."""\n'
        '    return f"{column} * 100"\n\n\n'
        "def explode(column: str) -> str:\n"
        '    """Fail while rendering."""\n'
        '    raise ValueError(f"cannot render {column}")\n\n\n'
        "def number(column: str) -> int:\n"
        '    """Return a value that is not SQL."""\n'
        "    return 1\n"
    ),
    "sql/models/a_first.sql": (
        'MODEL (description "First");\n\nSELECT @py.cents("amount") AS amount_cents\n'
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        MacroBridgeParityTestCase(
            description="nested, context, scoped, typed-reference, hook, test, audit, source "
            "and function macros",
            files=MACRO_BRIDGE_PROJECT_FILES,
            expected_fragments=(
                "'north:x'",
                "amount * 11",
                "amount * 21",
                "'base_rate, north_rate'",
                "'base_rate, south_rate'",
                "'__SQLBUILD_RELATION_0__'",
                "'__SQLBUILD_RELATION_1__'",
                "P006",
                "SELECT 2 * 100 AS stamped",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_heavy_project_when_rendering_then_inputs_are_expected(
    test_case: MacroBridgeParityTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = write_project(root=tmp_path / "project", files=test_case.files)

    native_capture: str = comparable_capture(render_compile_inputs(project_dir=project_dir))

    assert all(map(native_capture.__contains__, test_case.expected_fragments))


@pytest.mark.parametrize(
    "test_case",
    [
        MacroBridgeMemoTestCase(
            description="one global call text in three models",
            files=MACRO_BRIDGE_PROJECT_FILES,
            expected_native_executions=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_macro_calls_when_rendering_natively_then_each_call_class_runs_once(
    test_case: MacroBridgeMemoTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = write_project(root=tmp_path / "project", files=test_case.files)
    log_path: Path = tmp_path / "macros.log"
    monkeypatch.setenv(MACRO_CALL_LOG_ENV_VAR, str(log_path))

    _ = render_compile_inputs(project_dir=project_dir)

    assert len(log_path.read_text(encoding="utf-8").splitlines()) == (
        test_case.expected_native_executions
    )


@pytest.mark.parametrize(
    "test_case",
    [
        MacroBridgeFailureTestCase(
            description="macro raises in a later model",
            files={
                **_FAILURE_BASE_FILES,
                "sql/models/b_second.sql": (
                    'MODEL (description "Second");\n\nSELECT @py.explode("amount") AS bogus\n'
                ),
            },
            expected_message_fragment="failed: cannot render amount",
        ),
        MacroBridgeFailureTestCase(
            description="unknown macro after a successful call",
            files={
                **_FAILURE_BASE_FILES,
                "sql/models/b_second.sql": (
                    'MODEL (description "Second");\n\n'
                    'SELECT @py.cents("amount") AS a, @py.nope("amount") AS b\n'
                ),
            },
            expected_message_fragment="Unknown macro '@py.nope'",
        ),
        MacroBridgeFailureTestCase(
            description="macro returns a value that is not SQL",
            files={
                **_FAILURE_BASE_FILES,
                "sql/models/b_second.sql": (
                    'MODEL (description "Second");\n\nSELECT @py.number("amount") AS b\n'
                ),
            },
            expected_message_fragment="must return a SQL string",
        ),
        MacroBridgeFailureTestCase(
            description="malformed call after a successful call",
            files={
                **_FAILURE_BASE_FILES,
                "sql/models/b_second.sql": (
                    'MODEL (description "Second");\n\n'
                    'SELECT @py.cents("amount") AS a, @py.cents x("amount") AS b\n'
                ),
            },
            expected_message_fragment="is not followed by its opening parenthesis",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_render_when_rendering_natively_then_raises_the_expected_error(
    test_case: MacroBridgeFailureTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = write_project(root=tmp_path / "project", files=test_case.files)

    native_error: tuple[type[BaseException], str, type[object]] = render_error(
        project_dir=project_dir
    )

    assert test_case.expected_message_fragment in native_error[1]


@pytest.mark.parametrize(
    "test_case",
    [
        MacroExpansionDifferentialTestCase(
            description="seeded fragments",
            seed=540,
            samples=3000,
            expected_minimum_successes=600,
            expected_mismatches=[],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_random_macro_sql_when_expanding_through_bridge_then_matches_python(
    test_case: MacroExpansionDifferentialTestCase,
) -> None:
    rng: random.Random = random.Random(test_case.seed)
    texts: list[str] = [random_macro_sql(rng=rng) for _ in range(test_case.samples)]
    bridge: MacroBridge = MacroBridge()
    unbridged: list[tuple[object, ...]] = [expansion_outcome(sql=sql, bridge=None) for sql in texts]
    bridged: list[tuple[object, ...]] = [expansion_outcome(sql=sql, bridge=bridge) for sql in texts]
    expected: list[GoldenEntry] = read_golden(golden_name("macro_expansion", test_case.description))

    assert (
        golden_differences(expected, [golden_entry("macro_expansion", item) for item in bridged]),
        golden_differences(expected, [golden_entry("macro_expansion", item) for item in unbridged]),
    ) == (test_case.expected_mismatches, test_case.expected_mismatches)
    assert sum(len(outcome) == 5 for outcome in bridged) >= test_case.expected_minimum_successes
    assert bridge.stats()[0] > 0


@pytest.mark.parametrize(
    "test_case",
    [
        MacroExpansionEdgeTestCase(
            description="nested argument scan edges",
            texts=(
                "@py.n(@py.nope(1))",
                "@py.n(@py.m(1)",
                "@py.n(@py.ab c(1))",
                "@py.n(@py.m('x), 1)",
                "@py.n(@py.m(1), 'oops)",
                "@py.n(@py.mm (2), @py.m('é'))",
                "@py.n(@py.é(1))",
                "@py.n(/* @py.m( */ @py.m(1))",
                "@py.n(@py.nope(1), @py.m(')",
                "@py.n(@py.m(1) @py.xy z(2))",
                "@py.n(@py.m(1) @py.mm z(2))",
                "@py.mm z(2)",
                "@py.n(@m(1))",
                "@py.n(@sql.m(1))",
                "@py.n($$ ) $$, @py.m(1))",
                '@py.n("@py.m(1)")',
                "@py.n(@py.n(@py.n(@py.nope())))",
                "@py.n(@py.m(1), -- )\n @py.m(2))",
                "@py.n(@py.m(1), /* open)",
                "@py.n(@py.m(@py.n(@py.m('é'))))",
            ),
            expected_mismatches=[],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_nested_macro_arguments_when_expanding_then_matches_python_golden(
    test_case: MacroExpansionEdgeTestCase,
) -> None:
    bridge: MacroBridge = MacroBridge()
    unbridged: list[tuple[object, ...]] = [
        expansion_outcome(sql=sql, bridge=None) for sql in test_case.texts
    ]
    bridged: list[tuple[object, ...]] = [
        expansion_outcome(sql=sql, bridge=bridge) for sql in test_case.texts
    ]
    expected: list[GoldenEntry] = read_golden(golden_name("macro_expansion", test_case.description))

    assert (
        golden_differences(expected, [golden_entry("macro_expansion", item) for item in bridged]),
        golden_differences(expected, [golden_entry("macro_expansion", item) for item in unbridged]),
    ) == (test_case.expected_mismatches, test_case.expected_mismatches)


@pytest.mark.parametrize(
    "test_case",
    [
        FailingMacroProjectParityTestCase(
            description="seeded projects with raising, mistyped, unparsable and unreachable macros",
            seed=1043,
            projects=150,
            expected_minimum_failures=100,
            expected_minimum_successes=10,
            expected_minimum_errors_after_macro_runs=80,
            expected_minimum_distinct_errors=10,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_macro_projects_when_rendering_then_each_raises_one_error(
    test_case: FailingMacroProjectParityTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rng: random.Random = random.Random(test_case.seed)

    parity: FailingMacroParity = failing_macro_parity(
        projects=[generated_failing_macro_project(rng=rng) for _ in range(test_case.projects)],
        root=tmp_path,
        monkeypatch=monkeypatch,
    )

    assert (
        parity.failures >= test_case.expected_minimum_failures,
        parity.successes >= test_case.expected_minimum_successes,
        parity.failures_after_macro_runs >= test_case.expected_minimum_errors_after_macro_runs,
        len(parity.distinct_errors) >= test_case.expected_minimum_distinct_errors,
    ) == (True, True, True, True), parity


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
