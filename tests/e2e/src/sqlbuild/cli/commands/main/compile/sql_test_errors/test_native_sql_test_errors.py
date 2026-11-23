"""SQL test, scenario and relationship errors are reported with their code, message and type."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.corpus.attachment_failure_cases import (
    attachment_failure_cases,
)
from scripts.compiler_differential._helpers.corpus.scope_failure_cases import scope_failure_cases
from scripts.compiler_differential.constants import FAILURE_BASE_MART, FAILURE_MART_PATH
from scripts.compiler_differential.models import FailureCase
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    CompileReuseRun,
    lifecycle_error_type,
    run_reuse_compile,
    write_project_file,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_test_errors._test_types import (
    NativeSqlTestErrorTestCase,
)

_CALL_LOG: str = "macro_calls.log"
_COUNTED_MACRO: str = (
    "from pathlib import Path\n\n\n"
    "def counted(expression: str) -> str:\n"
    '    """Return the expression, logging each execution."""\n\n'
    f"    with (Path(__file__).resolve().parents[2] / {_CALL_LOG!r}).open(\n"
    '        "a", encoding="utf-8"\n'
    "    ) as log:\n"
    '        _ = log.write("call\\n")\n'
    "    return expression\n"
)
_CASES: dict[str, FailureCase] = {
    case.name: case for case in (*attachment_failure_cases(), *scope_failure_cases())
}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeSqlTestErrorTestCase(
            description="a scenario fixture without a target name",
            case_name="scenario-fixture-without-target",
        ),
        NativeSqlTestErrorTestCase(
            description="a scenario macro mock",
            case_name="scenario-macro-mock",
        ),
        NativeSqlTestErrorTestCase(
            description="a scenario without checks",
            case_name="scenario-without-checks",
        ),
        NativeSqlTestErrorTestCase(
            description="a scenario statement after its CTEs",
            case_name="scenario-statement-after-ctes",
        ),
        NativeSqlTestErrorTestCase(
            description="a scenario check reading a project source",
            case_name="scenario-check-reads-source",
        ),
        NativeSqlTestErrorTestCase(
            description="dependent scenario checks, still checked by Python's Polyglot parse",
            case_name="scenario-dependent-checks",
        ),
        NativeSqlTestErrorTestCase(
            description="the first of two broken scenarios",
            case_name="scenario-first-of-two-errors",
        ),
        NativeSqlTestErrorTestCase(
            description="a quoted scenario CTE name",
            case_name="scenario-quoted-cte-name",
            expected_location="--> tests/scenarios/orders.sql:6:1",
        ),
        NativeSqlTestErrorTestCase(
            description="a materialized scenario CTE",
            case_name="scenario-materialized-cte",
            expected_location="--> tests/scenarios/orders.sql:6:25",
        ),
        NativeSqlTestErrorTestCase(
            description="a quoted macro test CTE name",
            case_name="macro-test-quoted-cte-name",
            expected_location="--> tests/unit/test_stg_orders.sql:4:1",
        ),
        NativeSqlTestErrorTestCase(
            description="a test mocking an unknown source",
            case_name="test-mocks-unknown-source",
        ),
        NativeSqlTestErrorTestCase(
            description="a scoped scenario's expected CTE without a target",
            case_name="scope-scenario-expected-without-target",
        ),
        NativeSqlTestErrorTestCase(
            description="a scoped macro test whose actual CTE has no body",
            case_name="scope-macro-test-cte-without-body",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sql_test_error_when_compiling_then_the_error_and_type_are_reported(
    test_case: NativeSqlTestErrorTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    case: FailureCase = _CASES[test_case.case_name]
    project_dir: Path = tmp_path / "orders"
    files: dict[str, str] = {
        **case.files,
        "sql/macros/python/counted.py": _COUNTED_MACRO,
        FAILURE_MART_PATH: FAILURE_BASE_MART.replace("SUM(amount)", "SUM(@py.counted('amount'))"),
        _CALL_LOG: "",
    }
    for relative_path, contents in files.items():
        write_project_file(project_dir, relative_path, contents)
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    error_type: str = lifecycle_error_type(project_dir=project_dir, monkeypatch=monkeypatch)
    output: str = run.report + run.stderr

    assert (
        run.returncode,
        f"[{case.expected_code}]" in output,
        str(case.expected_message) in output,
        (case.expected_help or "") in output,
        test_case.expected_location in output,
        error_type,
    ) == (1, True, True, True, True, "CompileInputError"), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
