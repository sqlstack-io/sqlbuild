"""Exercise the dense guard's generated SQL tests with real DuckDB execution."""

import json
from pathlib import Path

import pytest

from scripts.cold_compile_performance._helpers.dense_project import (
    write_dense_compile_project,
)
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import DenseCompileFixtureTestCase


@pytest.mark.parametrize(
    "test_case",
    (DenseCompileFixtureTestCase("joins unions macros and functions", 64, 131, 134),),
    ids=lambda case: case.description,
)
def test_given_dense_fixture_when_compiling_and_testing_then_all_rules_and_rows_pass(
    test_case: DenseCompileFixtureTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project: Path = tmp_path / "orders"
    write_dense_compile_project(project_dir=project, model_count=test_case.model_count)
    config: Path = project / "sqlbuild_project.toml"
    config.write_text(
        config.read_text()
        .replace('adapter = "snowflake"', 'adapter = "duckdb"')
        .replace('database = "warehouse"\n', "")
        .replace('schema = "analytics"', 'schema = "main"')
        + f'\n[connection]\ndatabase = "{project / "orders.duckdb"}"\n',
        encoding="utf-8",
    )
    compiled: int = main(["--project-dir", str(project), "compile", "--no-cache", "--json"])
    output: str = capsys.readouterr().out
    assert compiled == 0, output
    payload: dict = json.loads(output)
    assert payload["diagnostics"] == []
    assert payload["compile_timings"]["rule_cache_hits"] == 0
    assert payload["compile_timings"]["rule_cache_misses"] == test_case.expected_rule_misses
    assert (
        payload["resources"]["models"][54]["lineage"]["edge_count"]
        == test_case.expected_tail_lineage_edges
    )
    built: int = main(
        [
            "--project-dir",
            str(project),
            "build",
            "--select",
            "fn_00000",
            "--no-tests",
            "--no-audits",
        ]
    )
    captured: str = "".join(capsys.readouterr())
    assert built == 0, captured
    tested: int = main(["--project-dir", str(project), "test"])
    captured = "".join(capsys.readouterr())
    assert tested == 0, captured


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
