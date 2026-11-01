"""Real CLI coverage of mixed, empty, expression and projected SQL scopes."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest
from _pytest.capture import CaptureResult

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import SemanticCompileCase
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    SEMANTIC_BINDING_PROJECT_TOML,
    write_semantic_binding_project,
)

_UPSTREAM: str = "SELECT 1 AS id, 'placed' AS status"


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticCompileCase(
            "mixed closed and open",
            _UPSTREAM,
            'SELECT u.missing FROM __ref("upstream") u CROSS JOIN __source("open_orders") o',
            "B002",
        ),
        SemanticCompileCase(
            "unqualified might belong to open",
            _UPSTREAM,
            'SELECT missing FROM __ref("upstream") u CROSS JOIN __source("open_orders") o',
            None,
        ),
        SemanticCompileCase(
            "no relation references",
            _UPSTREAM,
            "WITH orders AS (SELECT 1 AS id) SELECT missing FROM orders",
            "B002",
        ),
        SemanticCompileCase(
            "explicit names over open",
            'SELECT id FROM __source("open_orders")',
            'SELECT missing FROM __ref("upstream")',
            "B002",
        ),
        SemanticCompileCase(
            "open star",
            'SELECT * FROM __source("open_orders")',
            'SELECT missing FROM __ref("upstream")',
            None,
        ),
        SemanticCompileCase(
            "closed star",
            'SELECT * FROM __source("closed_orders")',
            'SELECT missing FROM __ref("upstream")',
            "B002",
        ),
        SemanticCompileCase(
            "expression source",
            _UPSTREAM,
            'SELECT missing FROM __source("expression_orders")',
            "B002",
        ),
        SemanticCompileCase(
            "grouping", _UPSTREAM, 'SELECT id, COUNT(*) AS n FROM __ref("upstream")', "B230"
        ),
        SemanticCompileCase(
            "aggregate placement",
            _UPSTREAM,
            'SELECT id FROM __ref("upstream") WHERE COUNT(*) > 0',
            "B231",
        ),
        SemanticCompileCase(
            "unique key",
            _UPSTREAM,
            "SELECT 1 AS id",
            "B300",
            "MODEL (description 'Test model.', materialized table, unique_key [missing]);\n",
        ),
        SemanticCompileCase(
            "date literal valid control",
            _UPSTREAM,
            "SELECT 1 AS id WHERE TIMESTAMP '2026-01-02' > '2026-01-01'",
            None,
        ),
        SemanticCompileCase(
            "timestamp numeric comparison rejected",
            _UPSTREAM,
            "SELECT 1 AS id WHERE TIMESTAMP '2026-01-02' > 5",
            "B217",
        ),
        SemanticCompileCase(
            "excess derived relation aliases",
            _UPSTREAM,
            "SELECT * FROM (SELECT 1 AS id) AS orders(first_id, second_id)",
            "B005",
        ),
        SemanticCompileCase(
            "fewer derived relation aliases remain valid",
            _UPSTREAM,
            "SELECT first_id, quantity FROM (SELECT 1 AS id, 2 AS quantity) AS orders(first_id)",
            None,
        ),
        SemanticCompileCase(
            "unknown unpivot input",
            _UPSTREAM,
            "SELECT * FROM (SELECT 1 AS id, 2 AS books) orders UNPIVOT (amount FOR category IN (missing))",
            "B002",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_semantic_scope_when_compiling_then_closed_shapes_are_checked(
    test_case: SemanticCompileCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql="MODEL (description 'Test model.', materialized view);\n" + test_case.upstream,
        downstream_sql=test_case.header + test_case.downstream,
        project_toml=SEMANTIC_BINDING_PROJECT_TOML + '\n[rules]\nignore = ["SQLCLEAN039"]\n',
    )
    (tmp_path / "sources").mkdir(exist_ok=True)
    (tmp_path / "sources" / "orders.yml").write_text(
        "sources:\n"
        "  - name: open_orders\n    description: Test source open_orders.\n    table: orders\n"
        "  - name: closed_orders\n    description: Test source closed_orders.\n    table: orders\n    contract: enforced\n"
        "    columns:\n      - name: id\n        type: INTEGER\n"
        "  - name: expression_orders\n    description: Test source expression_orders.\n    expression: SELECT 1 AS id\n",
        encoding="utf-8",
    )
    exit_code: int = main(
        ["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache", "--json"]
    )
    report: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert exit_code == int(bool(test_case.expected_code)), report
    codes: str = " ".join(diagnostic["code"] for diagnostic in report["diagnostics"])
    assert (test_case.expected_code or "") in codes, report


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticCompileCase(
            "aggregate after macro",
            "SELECT 1 AS id",
            'SELECT id FROM __ref("upstream") WHERE SUM(id) > 0',
            "B231",
            expected_column=44,
        ),
        SemanticCompileCase(
            "count star after macro",
            "SELECT 1 AS id",
            'SELECT id FROM __ref("upstream") WHERE COUNT(*) > 0',
            "B231",
            expected_column=40,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_macro_before_invalid_aggregate_when_compiling_then_reports_authored_position(
    test_case: SemanticCompileCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=test_case.header + test_case.upstream,
        downstream_sql=test_case.header + test_case.downstream,
    )
    exit_code: int = main(["--no-color", "--project-dir", str(tmp_path), "compile", "--no-cache"])
    output: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 1
    assert f"downstream.sql:2:{test_case.expected_column}" in output.out + output.err
    assert test_case.expected_code is not None
    assert test_case.expected_code in output.out + output.err


@pytest.mark.parametrize(
    "test_case",
    [
        SemanticCompileCase(
            "runtime equality", _UPSTREAM, "SELECT 1 = 'not-a-number' AS result", "B218"
        ),
        SemanticCompileCase(
            "runtime cast",
            _UPSTREAM,
            "SELECT CAST(TIMESTAMP '2026-01-01' AS INTEGER) AS result",
            "B218",
        ),
        SemanticCompileCase(
            "identical conversions are each reported at their own literal",
            _UPSTREAM,
            "SELECT 1 = 'not-a-number' AS first_result, 2 = 'not-a-number' AS second_result",
            "B218",
            expected_diagnostic_count=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_implicit_conversion_when_compiling_cold_and_warm_then_errors_block(
    test_case: SemanticCompileCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    write_semantic_binding_project(
        project_dir=tmp_path,
        upstream_sql=test_case.header + test_case.upstream,
        downstream_sql=test_case.header + test_case.downstream,
    )
    arguments: list[str] = ["--no-color", "--project-dir", str(tmp_path), "compile", "--json"]
    assert main(arguments) == 1
    cold: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert main(arguments) == 1
    warm: dict[str, Any] = json.loads(capsys.readouterr().out)
    assert cold["diagnostics"] == warm["diagnostics"]
    assert cold["summary"]["warnings"] == 0
    errors: list[dict[str, Any]] = cold["diagnostics"]
    assert cold["summary"]["errors"] == len(errors) == test_case.expected_diagnostic_count
    assert {error["code"] for error in errors} == {test_case.expected_code}
    assert all(error["severity"] == "error" for error in errors)
    assert main(arguments[:-1]) == 1
    human: str = capsys.readouterr().out
    assert f"error[{test_case.expected_code}]" in human


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
