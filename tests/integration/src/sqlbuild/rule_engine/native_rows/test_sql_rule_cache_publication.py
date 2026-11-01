"""Bound native SQL rule cache publication work while preserving persisted findings."""

import json
from pathlib import Path
from time import monotonic
from types import SimpleNamespace
from typing import Any, cast

import pytest

import sqlbuild._native as native
from sqlbuild._native import Finding, SqlRulesEvaluation
from tests.integration.src.sqlbuild.rule_engine.native_rows._test_types import (
    SqlRulePublicationTestCase,
)

_PROJECT_TOML: str = 'name = "orders"\nadapter = "duckdb"\n'


@pytest.mark.parametrize(
    "test_case",
    [SqlRulePublicationTestCase("doubling paths and findings", (4, 8, 16), 30.0)],
    ids=lambda case: case.description,
)
def test_given_increasing_findings_when_publishing_cache_then_findings_are_stored_by_path(
    test_case: SqlRulePublicationTestCase, tmp_path: Path
) -> None:
    """Lint each model natively, then publish its findings to the cache file in finding order."""
    started: float = monotonic()
    for size in test_case.sizes:
        project_dir: Path = tmp_path / str(size)
        (project_dir / "models").mkdir(parents=True)
        (project_dir / "sqlbuild_project.toml").write_text(_PROJECT_TOML, encoding="utf-8")
        paths: list[Path] = [Path(f"models/orders_{index}.sql") for index in range(size)]
        for index, path in enumerate(paths[:-1]):
            (project_dir / path).write_text(
                f'MODEL (description "Orders {index}");\n'
                "SELECT o.order_id FROM orders AS o LIMIT 1\n",
                encoding="utf-8",
            )
        (project_dir / paths[-1]).write_text(
            'MODEL (description "Orders without a limit");\nSELECT o.order_id FROM orders AS o\n',
            encoding="utf-8",
        )
        models: list[SimpleNamespace] = [
            SimpleNamespace(
                relative_path=path,
                authored_sql="SELECT 1 AS order_id",
                query_sql="SELECT 1 AS order_id",
                config=SimpleNamespace(values={}),
                dynamic_column_contract=None,
                relation_key_row=("model", path.stem, "", "", "", False, None, [], []),
            )
            for path in paths
        ]
        project: SimpleNamespace = SimpleNamespace(
            settings=SimpleNamespace(sql_analysis=True),
            effective_target_name=None,
            effective_vars={},
            models=models,
            seeds=(),
            sources=(),
            sql_expansions={},
        )
        plan: SimpleNamespace = SimpleNamespace(
            project_dir=project_dir,
            project=project,
            model_paths=frozenset(paths),
            selected=(SimpleNamespace(code="SQLCLEAN004", custom=False),),
            effective_config=SimpleNamespace(thresholds={}, cache=SimpleNamespace(enabled=True)),
        )
        result: SqlRulesEvaluation = native.run_planned_sql_rules(
            plan=cast(Any, plan),
            graph=cast(Any, SimpleNamespace(project=project)),
            dialect="duckdb",
            discovered_inputs=cast(Any, None),
            early_lint=None,
        )
        assert result.cache_misses == size
        assert len(result.findings) == size - 1
        stored: dict[str, tuple[str, str]] = native.load_sql_rule_cache(
            path=native.rules_bulk_cache_path(project_dir=project_dir, file_name="sql.json")
        )
        assert set(stored) == {path.as_posix() for path in paths}
        by_path: dict[str, list[Finding]] = {path.as_posix(): [] for path in paths}
        for finding in result.findings:
            by_path[finding.path.as_posix()].append(finding)
        for path, own in by_path.items():
            assert stored[path][1] == native.encode_sql_rule_findings(findings=own)
        assert json.loads(stored[paths[-1].as_posix()][1]) == []
    assert monotonic() - started < test_case.expected_max_seconds


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
