"""Attachment errors are reported with their code and type, and each macro call runs once."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.corpus.attachment_failure_cases import (
    attachment_failure_cases,
)
from scripts.compiler_differential.constants import FAILURE_BASE_MART, FAILURE_MART_PATH
from scripts.compiler_differential.models import FailureCase
from tests.e2e.src.sqlbuild.cli.commands.main.compile.attachment_errors._test_types import (
    NativeAttachmentErrorTestCase,
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
_ERROR_TYPE: str = "CompileInputError"
_CASES: dict[str, FailureCase] = {case.name: case for case in attachment_failure_cases()}


@pytest.mark.parametrize(
    "test_case",
    [
        NativeAttachmentErrorTestCase(
            description="an unknown audit run scope",
            case_name="generic-audit-unknown-run-scope",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="an audit argument overriding the implicit model",
            case_name="generic-audit-overrides-implicit-model",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="a cursor intrinsic in a generic audit reads expanded SQL",
            case_name="generic-audit-cursor-intrinsic",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="an unknown project variable in audit SQL",
            case_name="audit-unknown-project-variable",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="an unknown project variable inside doubled backticks in audit SQL",
            case_name="audit-variable-in-doubled-backticks",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="a SQL function return type the adapter dialect cannot parse",
            case_name="sql-function-invalid-return-type",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="a SQL function without returns",
            case_name="sql-function-missing-returns",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="an unsupported template namespace in a function header",
            case_name="sql-function-template-namespace",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
        NativeAttachmentErrorTestCase(
            description="an unknown variable in a source description",
            case_name="source-description-unknown-variable",
            expected_macro_calls=1,
            expected_error_type=_ERROR_TYPE,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_attachment_error_when_compiling_then_the_error_and_type_are_reported(
    test_case: NativeAttachmentErrorTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
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
    macro_calls: int = len((project_dir / _CALL_LOG).read_text(encoding="utf-8").splitlines())
    error_type: str = lifecycle_error_type(project_dir=project_dir, monkeypatch=monkeypatch)
    output: str = run.report + run.stderr

    assert (
        run.returncode,
        f"[{case.expected_code}]" in output,
        str(case.expected_message) in output,
        macro_calls,
        error_type,
    ) == (1, True, True, test_case.expected_macro_calls, test_case.expected_error_type), output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
