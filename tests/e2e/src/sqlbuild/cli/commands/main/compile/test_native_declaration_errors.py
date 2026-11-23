"""Declaration and scope errors are reported as Python does, without a re-run."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential.constants import FAILURE_BASE_FILES, FAILURE_BASE_MART
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    DeclarationErrorLifecycleTestCase,
    NativeDeclarationErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    CompileReuseRun,
    lifecycle_error_type,
    run_reuse_compile,
    write_project_file,
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
_STAGING_PATH: str = "sql/models/staging/stg_orders.sql"
_STAGING_PREFIX: str = (
    'MODEL (\n  description "Staged orders",\n);\n\n'
    'SELECT order_id, customer_id, amount, status\nFROM __source("raw_orders")\nWHERE '
)
_STATUS_ENUM: dict[str, str] = {
    "sql/enums/order_status.sql": "ENUM (name order_status, members [PLACED, SHIPPED]);\n",
}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeDeclarationErrorTestCase(
            description="unknown_enum_after_a_macro_ran",
            project_files={
                **_STATUS_ENUM,
                _STAGING_PATH: _STAGING_PREFIX + 'status = @enum("missing_status").PLACED\n',
            },
            expected_report_text="Unknown enum 'missing_status'",
            expected_macro_calls=1,
        ),
        NativeDeclarationErrorTestCase(
            description="first_of_two_reference_errors_after_a_macro_ran",
            project_files={
                **_STATUS_ENUM,
                _STAGING_PATH: _STAGING_PREFIX
                + 'status = @enum("order_status").HELD OR amount > @const(cap)\n',
            },
            expected_report_text="Unknown member 'HELD' for enum 'order_status'",
            expected_macro_calls=1,
        ),
        NativeDeclarationErrorTestCase(
            description="invalid_constant_reference_after_a_macro_ran",
            project_files={_STAGING_PATH: _STAGING_PREFIX + "amount > @const(cap)\n"},
            expected_report_text="Invalid constant reference",
            expected_macro_calls=1,
        ),
        NativeDeclarationErrorTestCase(
            description="inaccessible_local_constant_after_a_macro_ran",
            project_files={
                "sql/models/marts/_constants/limits.sql": "CONSTANT (name order_cap, value 3);\n",
                _STAGING_PATH: _STAGING_PREFIX + 'amount > @const("order_cap")\n',
            },
            expected_report_text="Constant 'order_cap' is known but inaccessible",
            expected_macro_calls=1,
        ),
        NativeDeclarationErrorTestCase(
            description="unclosed_quote_after_a_reference_only_python_scans",
            project_files={
                **_STATUS_ENUM,
                _STAGING_PATH: _STAGING_PREFIX + "status = @enum\u00e9 OR status = 'open\n",
            },
            expected_report_text="Enum and constant expansion contains an unclosed quoted string",
            expected_macro_calls=1,
        ),
        NativeDeclarationErrorTestCase(
            description="duplicate_scoped_enum_before_any_macro_runs",
            project_files={
                **_STATUS_ENUM,
                "sql/models/staging/_enums/order_status.sql": (
                    "ENUM (name order_status, members [HELD]);\n"
                ),
            },
            expected_report_text="order_status",
            expected_macro_calls=0,
        ),
        NativeDeclarationErrorTestCase(
            description="invalid_enum_declaration_file",
            project_files={"sql/enums/order_status.sql": "ENUM (name order_status, members [held]);\n"},
            expected_report_text="member identifiers must be uppercase",
            expected_macro_calls=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_error_when_compiling_natively_then_error_matches_python_once(
    test_case: NativeDeclarationErrorTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "orders"
    files: dict[str, str] = {
        **FAILURE_BASE_FILES,
        "sql/macros/python/counted.py": _COUNTED_MACRO,
        "sql/models/marts/customer_totals.sql": _COUNTED_MART,
        **test_case.project_files,
    }
    for relative_path, contents in {**files, _CALL_LOG: ""}.items():
        write_project_file(project_dir, relative_path, contents)

    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    macro_calls: int = len((project_dir / _CALL_LOG).read_text(encoding="utf-8").splitlines())

    assert (
        run.returncode,
        test_case.expected_report_text in run.report + run.stderr,
        macro_calls,
    ) == (1, True, test_case.expected_macro_calls), run.report + run.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DeclarationErrorLifecycleTestCase(
            description="unknown_enum_after_a_macro_ran",
            project_files={
                **_STATUS_ENUM,
                _STAGING_PATH: _STAGING_PREFIX + 'status = @enum("missing_status").PLACED\n',
            },
            expected_error_type="CompileInputError",
        ),
        DeclarationErrorLifecycleTestCase(
            description="unclosed_quote_after_a_reference_only_python_scans",
            project_files={
                **_STATUS_ENUM,
                _STAGING_PATH: _STAGING_PREFIX + "status = @enum\u00e9 OR status = 'open\n",
            },
            expected_error_type="CompileInputError",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_error_when_compiling_then_lifecycle_error_type_matches(
    test_case: DeclarationErrorLifecycleTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = tmp_path / "orders"
    for relative_path, contents in {
        **FAILURE_BASE_FILES,
        "sql/macros/python/counted.py": _COUNTED_MACRO,
        "sql/models/marts/customer_totals.sql": _COUNTED_MART,
        _CALL_LOG: "",
        **test_case.project_files,
    }.items():
        write_project_file(project_dir, relative_path, contents)

    error_type: str = lifecycle_error_type(project_dir=project_dir, monkeypatch=monkeypatch)

    assert error_type == test_case.expected_error_type, test_case.description


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
