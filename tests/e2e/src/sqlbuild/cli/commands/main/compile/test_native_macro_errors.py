"""Macro errors are reported once with Python's wording, and each macro call runs once."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential.constants import FAILURE_BASE_FILES, FAILURE_BASE_MART
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    MacroErrorLifecycleTestCase,
    NativeMacroErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    MACRO_CALL_LOG_CODE,
    CompileReuseRun,
    lifecycle_error_type,
    macro_call_log,
    run_reuse_compile,
    write_project_file,
)

_MACROS: str = (
    "from pathlib import Path\n\n\n"
    "def _log(name: str) -> None:\n"
    f"    with {MACRO_CALL_LOG_CODE}.open(\n"
    '        "a", encoding="utf-8"\n'
    "    ) as log:\n"
    '        _ = log.write(f"{name}\\n")\n\n\n'
    "def counted(expression: str) -> str:\n"
    '    """Return the expression, logging each execution."""\n'
    '    _log(f"counted {expression}")\n'
    "    return expression\n\n\n"
    "def explode(expression: str) -> str:\n"
    '    """Fail after logging the call."""\n'
    '    _log(f"explode {expression}")\n'
    '    raise ValueError(f"cannot render {expression}")\n\n\n'
    "def answer() -> int:\n"
    '    """Return a value that is not SQL."""\n'
    '    _log("answer")\n'
    "    return 42\n\n\n"
    "def upstream() -> str:\n"
    '    """Read a model that does not exist."""\n'
    '    _log("upstream")\n'
    "    return '__ref(\"ghost\")'\n"
)
_MART_MACROS: str = (
    "def mart_only(expression: str) -> str:\n"
    '    """Qualify an expression for marts."""\n'
    "    return expression\n"
)
_CYCLIC_MACROS: str = (
    'def ping(expression: str) -> str:\n    """Call pong."""\n    return pong(expression)\n\n\n'
    'def pong(expression: str) -> str:\n    """Call ping."""\n    return ping(expression)\n'
)
_COUNTED_MART: str = FAILURE_BASE_MART.replace("SUM(amount)", "SUM(@py.counted('amount'))")
_STAGING_PATH: str = "sql/models/staging/stg_orders.sql"
_MART_PATH: str = "sql/models/marts/customer_totals.sql"
_STAGING_PREFIX: str = (
    'MODEL (\n  description "Staged orders",\n);\n\nSELECT order_id, customer_id, '
)
_STAGING_SUFFIX: str = ' AS amount, status\nFROM __source("raw_orders")\n'


@pytest.mark.parametrize(
    "test_case",
    [
        NativeMacroErrorTestCase(
            description="macro_raises_after_a_side_effect",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.explode('amount')" + _STAGING_SUFFIX
            },
            expected_report_text="stg_orders.sql' failed: cannot render amount",
            expected_macro_runs=["counted amount", "explode amount"],
        ),
        NativeMacroErrorTestCase(
            description="macro_returns_a_value_that_is_not_sql",
            project_files={_STAGING_PATH: _STAGING_PREFIX + "@py.answer()" + _STAGING_SUFFIX},
            expected_report_text="must return a SQL string when used directly in SQL",
            expected_macro_runs=["counted amount", "answer"],
        ),
        NativeMacroErrorTestCase(
            description="macro_arguments_do_not_parse",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.counted('amount',,)" + _STAGING_SUFFIX
            },
            expected_report_text="could not be parsed",
            expected_macro_runs=["counted amount"],
        ),
        NativeMacroErrorTestCase(
            description="unknown_macro",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.missing('amount')" + _STAGING_SUFFIX
            },
            expected_report_text="Unknown macro '@py.missing'",
            expected_macro_runs=["counted amount"],
        ),
        NativeMacroErrorTestCase(
            description="private_macro_helper",
            project_files={_STAGING_PATH: _STAGING_PREFIX + "@py._log('amount')" + _STAGING_SUFFIX},
            expected_report_text="Unknown macro '@py._log'",
            expected_macro_runs=["counted amount"],
        ),
        NativeMacroErrorTestCase(
            description="macro_in_a_sibling_scope",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.mart_only('amount')" + _STAGING_SUFFIX,
                "sql/models/marts/_sqlbuild/macros/python/local.py": _MART_MACROS,
            },
            expected_report_text="stg_orders.sql' is inaccessible",
            expected_macro_runs=["counted amount"],
        ),
        NativeMacroErrorTestCase(
            description="macro_call_cycle",
            project_files={"sql/macros/python/cyclic.py": _CYCLIC_MACROS},
            expected_report_text="Macro call cycle in 'sql/macros/python/cyclic.py': ping -> pong -> ping",
            expected_macro_runs=[],
        ),
        NativeMacroErrorTestCase(
            description="first_of_two_failing_models",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.explode('amount')" + _STAGING_SUFFIX,
                _MART_PATH: FAILURE_BASE_MART.replace("SUM(amount)", "SUM(@py.explode('total'))"),
            },
            expected_report_text="customer_totals.sql' failed: cannot render total",
            expected_macro_runs=["explode total"],
        ),
        NativeMacroErrorTestCase(
            description="reference_error_in_macro_sql",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX
                + "amount FROM @py.upstream() UNION ALL SELECT 1, 2, amount"
                + _STAGING_SUFFIX
            },
            expected_report_text="references unknown model 'ghost'",
            expected_macro_runs=["counted amount", "upstream"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_error_when_compiling_then_error_matches_python_once(
    test_case: NativeMacroErrorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "orders"
    for relative_path, contents in {
        **FAILURE_BASE_FILES,
        "sql/macros/python/orders.py": _MACROS,
        _MART_PATH: _COUNTED_MART,
        **test_case.project_files,
    }.items():
        write_project_file(project_dir, relative_path, contents)

    _ = macro_call_log(project_dir).write_text("", encoding="utf-8")
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    macro_runs: list[str] = macro_call_log(project_dir).read_text(encoding="utf-8").splitlines()

    assert (
        run.returncode,
        test_case.expected_report_text in run.report + run.stderr,
        macro_runs,
    ) == (1, True, test_case.expected_macro_runs), run.report + run.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        MacroErrorLifecycleTestCase(
            description="macro_raises_after_a_side_effect",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.explode('amount')" + _STAGING_SUFFIX
            },
            expected_error_type="CompileInputError",
        ),
        MacroErrorLifecycleTestCase(
            description="unknown_macro",
            project_files={
                _STAGING_PATH: _STAGING_PREFIX + "@py.missing('amount')" + _STAGING_SUFFIX
            },
            expected_error_type="CompileInputError",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_error_when_compiling_then_lifecycle_error_type_matches(
    test_case: MacroErrorLifecycleTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = tmp_path / "orders"
    for relative_path, contents in {
        **FAILURE_BASE_FILES,
        "sql/macros/python/orders.py": _MACROS,
        _MART_PATH: _COUNTED_MART,
        **test_case.project_files,
    }.items():
        write_project_file(project_dir, relative_path, contents)

    error_type: str = lifecycle_error_type(project_dir=project_dir, monkeypatch=monkeypatch)

    assert error_type == test_case.expected_error_type, test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
