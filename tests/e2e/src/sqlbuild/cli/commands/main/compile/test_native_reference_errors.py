"""Reference scan errors are located and reported as CompileInputError."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential.constants import (
    FAILURE_BASE_CONFIG,
    FAILURE_BASE_FILES,
    FAILURE_MART_PATH,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    ReferenceScanErrorTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import (
    MACRO_CALL_LOG_CODE,
    CompileReuseRun,
    lifecycle_error_type,
    macro_call_log,
    run_reuse_compile,
    write_project_file,
)

_EMIT_MACRO: str = (
    "from pathlib import Path\n\n\n"
    "def emit(expression: str) -> str:\n"
    '    """Return the expression, logging each execution."""\n\n'
    f"    with {MACRO_CALL_LOG_CODE}.open(\n"
    '        "a", encoding="utf-8"\n'
    "    ) as log:\n"
    '        _ = log.write("call\\n")\n'
    "    return expression\n"
)
_MART_HEADER: str = 'MODEL (\n  description "Order totals per customer",\n);\n\n'
_QUOTED_NOTE: str = "over 12\\\" boxes, by customer's orders"
_QUOTED_DESCRIPTION: str = "over 12\" boxes, by customer's orders"
_NO_SQL_ANALYSIS_CONFIG: str = FAILURE_BASE_CONFIG + "\n[settings]\nsql_analysis = false\n"


@pytest.mark.parametrize(
    "test_case",
    [
        ReferenceScanErrorTestCase(
            description="unclosed_reference_call",
            project_files={
                FAILURE_MART_PATH: _MART_HEADER
                + 'SELECT customer_id, 1 AS total_amount\nFROM __ref("stg_orders"\n'
            },
            expected_message=(
                f"{FAILURE_MART_PATH}:6:6: SQL reference contains an unclosed parenthesis"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="unclosed_quote_after_a_rejected_call",
            project_files={
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, 1 AS total_amount\nFROM __ref(stg_orders)\n"
                "WHERE 'é' <> 'open\n"
            },
            expected_message=(
                f"{FAILURE_MART_PATH}:7:14: SQL reference contains an unclosed quoted string"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="empty_table_function_argument",
            project_files={
                FAILURE_MART_PATH: _MART_HEADER + "SELECT customer_id, 1 AS total_amount\n"
                'FROM __ref("stg_orders") JOIN __table_fn("orders_for")(1, , 2) USING (customer_id)\n'
            },
            expected_message=(
                f"{FAILURE_MART_PATH}:6:31: SQL reference contains an empty argument"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="triple_quoted_header_string_with_a_quote_and_apostrophe",
            project_files={
                FAILURE_MART_PATH: f'MODEL (\n  description """{_QUOTED_DESCRIPTION}""",\n);\n\n'
                'SELECT customer_id, 1 AS total_amount\nFROM __ref("stg_orders")\nWHERE "open = 1\n'
            },
            expected_message=(
                f"{FAILURE_MART_PATH}:7:7: SQL reference contains an unclosed quoted string"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="macro_output_fault_before_an_authored_fault_is_unlocated",
            project_files={
                "sql/macros/python/emit.py": _EMIT_MACRO,
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, @py.emit('__ref(1') AS total_amount\n"
                'FROM __ref("stg_orders"\n',
            },
            expected_message=(
                f"{FAILURE_MART_PATH}: SQL reference contains an unclosed parenthesis"
            ),
            expected_macro_calls=1,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="authored_fault_after_a_macro_is_located_natively",
            project_files={
                "sql/macros/python/emit.py": _EMIT_MACRO,
                FAILURE_MART_PATH: _MART_HEADER
                + "SELECT customer_id, @py.emit('1') AS total_amount\n"
                'FROM __ref("stg_orders"\n',
            },
            expected_message=(
                f"{FAILURE_MART_PATH}:6:6: SQL reference contains an unclosed parenthesis"
            ),
            expected_macro_calls=1,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="test_file_header_names_the_file",
            project_files={
                "tests/unit/test_customer_totals.sql": (
                    'TEST (\n  name "totals_by_note",\n  parameters (note string),\n'
                    f'  cases (boxes (note "{_QUOTED_NOTE}")),\n);\n\nWITH\n'
                    "__ref__stg_orders AS (\n"
                    "  SELECT 10 AS customer_id, CAST(5 AS DOUBLE) AS amount\n),\n"
                    "__expected__customer_totals AS (\n"
                    '  SELECT 10 AS customer_id, 5 AS total_amount, @param("note") AS note\n),\n'
                    "__assert__totals AS (\n"
                    '  SELECT * FROM __table_fn("orders_for")(1, , 2)\n)\n'
                )
            },
            expected_message=(
                "tests/unit/test_customer_totals.sql: SQL reference contains an empty argument"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="audit_file_header_names_the_file",
            project_files={
                "sql/audits/generic/positive_total.sql": (
                    "AUDIT (\n  -- totals over 12\" boxes, by customer's orders\n"
                    "  severity warn\n);\n\n"
                    'SELECT * FROM __ref("@model") WHERE note = \'open\n'
                ),
                FAILURE_MART_PATH: FAILURE_BASE_FILES[FAILURE_MART_PATH].replace(
                    "  description", "  audits [positive_total],\n  description"
                ),
            },
            expected_message=(
                "sql/audits/generic/positive_total.sql: SQL reference contains an unclosed quoted string"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
        ReferenceScanErrorTestCase(
            description="function_file_header_names_the_file",
            project_files={
                "sqlbuild_project.toml": _NO_SQL_ANALYSIS_CONFIG,
                "sql/functions/sql/orders_for.sql": (
                    f'FUNCTION (\n  description """{_QUOTED_DESCRIPTION}""",\n'
                    "  arguments (p_customer_id INTEGER),\n  returns INTEGER\n);\n\n"
                    'SELECT COUNT(*) FROM __ref("stg_orders" WHERE customer_id = p_customer_id\n'
                ),
            },
            expected_message=(
                "sql/functions/sql/orders_for.sql: SQL reference contains an unclosed parenthesis"
            ),
            expected_macro_calls=0,
            expected_error_type="CompileInputError",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reference_scan_error_when_compiling_then_it_is_located_and_typed(
    test_case: ReferenceScanErrorTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project_dir: Path = tmp_path / "orders"
    for relative_path, contents in {
        **FAILURE_BASE_FILES,
        **test_case.project_files,
    }.items():
        write_project_file(project_dir, relative_path, contents)
    _ = macro_call_log(project_dir).write_text("", encoding="utf-8")
    run: CompileReuseRun = run_reuse_compile(project_dir=project_dir)
    macro_calls: int = len(macro_call_log(project_dir).read_text(encoding="utf-8").splitlines())
    error_type: str = lifecycle_error_type(project_dir=project_dir, monkeypatch=monkeypatch)

    assert (
        run.returncode,
        test_case.expected_message in run.report,
        macro_calls,
        error_type,
    ) == (1, True, test_case.expected_macro_calls, test_case.expected_error_type), run.report


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
