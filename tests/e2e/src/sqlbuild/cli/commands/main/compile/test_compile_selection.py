"""E2E coverage for focused offline compilation."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile._test_types import (
    CompileSelectionTestCase,
    SelectorFileErrorTestCase,
    SourceSelectionRulesTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.helpers import json_diagnostic_codes
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_PROJECT_TOML: str = (
    'name = "compile_selection"\n'
    'adapter = "duckdb"\n'
    'default_target = "dev"\n\n'
    "[connection]\n"
    'database = "selection.duckdb"\n\n'
    "[targets.dev]\n"
    'schema = "dev"\n'
)
_SOURCE_FILES: dict[str, str] = {
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw__orders\n"
        "    description: Raw orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n"
        "        type: INTEGER\n"
    ),
    "sql/models/stg_orders.sql": (
        "MODEL (description 'Staged orders.');\n\nSELECT order_id FROM __source(\"raw__orders\")\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        CompileSelectionTestCase(
            description="selected models compile without unrelated contract diagnostics",
            selection_args=("--select", "order_record", "result"),
            expected_stdout_fragments=(
                "Compile ready  2 models",
                "order_record",
                "result",
                "Project compiled  2 models",
            ),
            unexpected_stdout_fragments=("unrelated", "error[K001]"),
        ),
        CompileSelectionTestCase(
            description="exclude-only selection compiles every other model",
            selection_args=("--exclude", "unrelated"),
            expected_stdout_fragments=(
                "Compile ready  2 models",
                "order_record",
                "result",
                "Project compiled  2 models",
            ),
            unexpected_stdout_fragments=("unrelated", "error[K001]"),
        ),
        CompileSelectionTestCase(
            description="select file keeps upstream analysis while reporting its downstream",
            selection_args=("--select-file", "selectors.txt"),
            expected_stdout_fragments=(
                "Compile ready  1 model",
                "result",
                "Project compiled  1 model",
            ),
            unexpected_stdout_fragments=("order_record", "unrelated", "error[K001]"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_selection_when_compiling_then_limits_offline_analysis_and_report(
    test_case: CompileSelectionTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="compile_selection",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "compile_selection"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                'database = ":memory:"\n\n'
                "[targets.dev]\n"
                'schema = "dev"\n'
            ),
            "sql/models/order_record.sql": "MODEL (description 'Test model order_record.');\n\nSELECT 1 AS order_id\n",
            "sql/models/result.sql": (
                'MODEL (description "Test model result.", columns (order_id ()));\n\nSELECT order_id FROM __ref("order_record")\n'
            ),
            "sql/models/unrelated.sql": (
                "MODEL (description 'Test model unrelated.', columns (missing_column ()));\n\nSELECT 1 AS present_column\n"
            ),
            "selectors.txt": "result\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "compile",
            *test_case.selection_args,
            "--no-sql-validation",
        ),
        project_dir=project_dir,
        working_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        SelectorFileErrorTestCase(
            description="compile names the missing selector file",
            command=("compile",),
            expected_json_codes=(),
        ),
        SelectorFileErrorTestCase(
            description="compile json reports the missing selector file as a diagnostic",
            command=("compile", "--json"),
            expected_json_codes=("C114",),
        ),
        SelectorFileErrorTestCase(
            description="plan names the missing selector file",
            command=("plan",),
            expected_json_codes=(),
        ),
        SelectorFileErrorTestCase(
            description="build names the missing selector file",
            command=("build",),
            expected_json_codes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_missing_selector_file_when_running_then_reports_a_coded_error(
    test_case: SelectorFileErrorTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="compile_selection",
        repo_files={"sqlbuild_project.toml": _PROJECT_TOML, **_SOURCE_FILES},
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command, "--select-file", "missing.txt"),
        project_dir=project_dir,
        working_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 1, test_case.description + output
    assert "Traceback" not in output, test_case.description + output
    assert "error[C114]: cannot read selector file missing.txt" in result.stderr, (
        test_case.description + output
    )
    assert json_diagnostic_codes(result.stdout) == test_case.expected_json_codes, (
        test_case.description + output
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SourceSelectionRulesTestCase(
            description="an exact source selection evaluates rules without models",
            selection_args=("--select", "source:raw__orders"),
            expected_stdout_fragment="Project compiled  0 models",
        ),
        SourceSelectionRulesTestCase(
            description="a source glob selection evaluates rules without models",
            selection_args=("--select", "source:raw_*"),
            expected_stdout_fragment="Project compiled  0 models",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rules_and_source_only_selection_when_compiling_then_compiles(
    test_case: SourceSelectionRulesTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="compile_selection",
        repo_files={
            "sqlbuild_project.toml": _PROJECT_TOML + '\n[rules]\nselect = ["SQBRDECLARATION201"]\n',
            **_SOURCE_FILES,
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile", *test_case.selection_args),
        project_dir=project_dir,
        working_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == 0, test_case.description + output
    assert "panicked" not in output, test_case.description + output
    assert test_case.expected_stdout_fragment in result.stdout, test_case.description + output


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
