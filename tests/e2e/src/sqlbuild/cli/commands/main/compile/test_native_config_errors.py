"""Model config, template and header errors are reported without Python fallbacks."""

from __future__ import annotations

from pathlib import Path
from types import ModuleType

import pytest

import sqlbuild._native as _native
from scripts.compiler_differential.constants import FAILURE_BASE_MART
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    MacroExpandedValidatorErrorTestCase,
    NativeConfigErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    CompileReuseRun,
    fallback_free_preview_compile,
    run_reuse_compile,
    write_counted_error_project,
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
_COUNTED_MART: str = FAILURE_BASE_MART.replace("SUM(amount)", "SUM(@py.counted('amount'))")
_UPSTREAM_MACRO: str = _COUNTED_MACRO.replace(
    "def counted(expression: str) -> str:", "def upstream() -> str:"
).replace("    return expression\n", "    return '__ref(\"ghost\")'\n")
_STAGING_PATH: str = "sql/models/staging/stg_orders.sql"
_STAGING_BODY: str = (
    '\n);\n\nSELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\n'
)
_INCREMENTAL: str = (
    "  materialized incremental,\n  incremental_strategy append,\n  cursor order_id,\n"
    "  cursor_type integer,\n"
)
_FALLBACKS: tuple[tuple[ModuleType, str], ...] = ((_native, "expand_template_data"),)
_PROJECT_FILES: dict[str, str] = {
    "sql/macros/python/counted.py": _COUNTED_MACRO,
    "sql/models/marts/customer_totals.sql": _COUNTED_MART,
    _CALL_LOG: "",
}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeConfigErrorTestCase(
            description="validator_error_after_a_macro_ran",
            project_files={
                _STAGING_PATH: "MODEL (\n  materialized incremental,\n  incremental_strategy upsert,"
                + _STAGING_BODY
            },
            expected_report_text="unknown incremental_strategy 'upsert'",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
        NativeConfigErrorTestCase(
            description="first_of_several_validator_faults",
            project_files={
                _STAGING_PATH: "MODEL (\n"
                + _INCREMENTAL
                + "  cursor_start 20,\n  cursor_end 10,\n  contract strict,"
                + _STAGING_BODY
            },
            expected_report_text="cursor_start must be before exclusive cursor_end",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
        NativeConfigErrorTestCase(
            description="template_error_after_a_macro_ran",
            project_files={
                _STAGING_PATH: 'MODEL (\n  schema "${missing_region}_core",' + _STAGING_BODY
            },
            expected_report_text="model config references unknown variable 'missing_region'",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
        NativeConfigErrorTestCase(
            description="header_column_error_after_a_macro_ran",
            project_files={
                _STAGING_PATH: "MODEL (\n  columns (order_id (type INTEGER, format plain)),"
                + _STAGING_BODY
            },
            expected_report_text="column 'order_id' has unknown metadata keys: format",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
        NativeConfigErrorTestCase(
            description="header_audit_error_after_a_macro_ran",
            project_files={
                _STAGING_PATH: "MODEL (\n  audits [unique_combination (columns [order_id], "
                "severity fatal)]," + _STAGING_BODY
            },
            expected_report_text="'severity' must be one of: warn, error",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
        NativeConfigErrorTestCase(
            description="config_build_error_after_a_macro_ran",
            project_files={_STAGING_PATH: 'MODEL (\n  tags "orders",' + _STAGING_BODY},
            expected_report_text="tags must be a list",
            expected_macro_calls=1,
            expected_python_fallbacks=[],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_config_error_when_compiling_then_the_error_is_reported_without_fallbacks(
    test_case: NativeConfigErrorTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_counted_error_project(
        project_dir=project_dir, files={**_PROJECT_FILES, **test_case.project_files}
    )
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    macro_calls: int = len((project_dir / _CALL_LOG).read_text(encoding="utf-8").splitlines())
    write_counted_error_project(
        project_dir=tmp_path / "in_process", files={**_PROJECT_FILES, **test_case.project_files}
    )
    in_process_run, fallbacks = fallback_free_preview_compile(
        project_dir=tmp_path / "in_process",
        fallbacks=_FALLBACKS,
        monkeypatch=monkeypatch,
        capsys=capsys,
    )

    assert (
        run.returncode,
        test_case.expected_report_text in run.report + run.stderr,
        macro_calls,
        in_process_run.returncode,
        in_process_run.report.replace("in_process", "orders"),
        fallbacks,
    ) == (
        1,
        True,
        test_case.expected_macro_calls,
        1,
        run.report,
        test_case.expected_python_fallbacks,
    ), (run.report, run.stderr)


@pytest.mark.parametrize(
    "test_case",
    [
        MacroExpandedValidatorErrorTestCase(
            description="unknown_model_reference_from_a_macro",
            project_files={
                "sql/macros/python/upstream.py": _UPSTREAM_MACRO,
                "sql/models/marts/customer_totals.sql": (
                    'MODEL (\n  description "Order totals per customer",\n);\n\n'
                    "SELECT * FROM @py.upstream()\n"
                ),
                _CALL_LOG: "",
            },
            expected_report_text="references unknown model 'ghost'",
            expected_macro_calls=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_validator_error_from_macro_sql_when_compiling_then_macros_run_once(
    test_case: MacroExpandedValidatorErrorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "orders"
    write_counted_error_project(project_dir=project_dir, files=test_case.project_files)
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    macro_calls: int = len((project_dir / _CALL_LOG).read_text(encoding="utf-8").splitlines())

    assert (
        run.returncode,
        test_case.expected_report_text in run.report + run.stderr,
        macro_calls,
    ) == (1, True, test_case.expected_macro_calls), run.report + run.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
