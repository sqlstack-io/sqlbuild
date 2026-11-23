"""Integration tests for concurrent build execution with file-based DuckDB."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from textwrap import dedent

import pytest

from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.build.models import BuildExecutionResult
from sqlbuild.executor.build.types import BuildStatus
from sqlbuild.executor.scheduling.types import ExecutionStatus
from tests.integration.src.sqlbuild.executor.build.concurrent._test_types import (
    ConcurrentBuildTestCase,
)
from tests.integration.src.sqlbuild.executor.build.concurrent.helpers import (
    run_concurrent_build,
    verify_concurrent_warehouse_state,
)

_PROJECT_YML: str = (
    'name = "demo"\n'
    'adapter = "duckdb"\n\n'
    "[connection]\n"
    'database = "test.duckdb"\n\n'
    "[settings]\n"
    'default_audit_severity = "error"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentBuildTestCase(
            description=("two independent tables both materialize with concurrency 2"),
            max_concurrency=2,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/alpha.sql": (
                    "MODEL (description 'Test model alpha.', materialized table);\n\nSELECT 1 AS id, 'alpha' AS name"
                ),
                "sql/models/beta.sql": (
                    "MODEL (description 'Test model beta.', materialized table);\n\nSELECT 2 AS id, 'beta' AS name"
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=2,
            expected_query_results=(
                (
                    "SELECT id, name FROM main.alpha",
                    ((1, "alpha"),),
                ),
                (
                    "SELECT id, name FROM main.beta",
                    ((2, "beta"),),
                ),
            ),
        ),
        ConcurrentBuildTestCase(
            description=("diamond DAG with shared upstream materializes all four nodes correctly"),
            max_concurrency=3,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/staging/stg_raw.sql": (
                    "MODEL (description 'Test model stg_raw.', materialized view);\n\nSELECT 10 AS id, 'row' AS val"
                ),
                "sql/models/branch_a.sql": (
                    "MODEL (description 'Test model branch_a.', materialized table);\n\n"
                    "SELECT id, val || '_a' AS val FROM __ref(\"stg_raw\")"
                ),
                "sql/models/branch_b.sql": (
                    "MODEL (description 'Test model branch_b.', materialized table);\n\n"
                    "SELECT id, val || '_b' AS val FROM __ref(\"stg_raw\")"
                ),
                "sql/models/merged.sql": (
                    "MODEL (description 'Test model merged.', materialized table);\n\n"
                    "SELECT a.id, a.val AS a_val, b.val AS b_val "
                    'FROM __ref("branch_a") a '
                    'JOIN __ref("branch_b") b ON a.id = b.id'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=4,
            expected_query_results=(
                (
                    "SELECT id, a_val, b_val FROM main.merged",
                    ((10, "row_a", "row_b"),),
                ),
            ),
        ),
        ConcurrentBuildTestCase(
            description=("three-layer chain preserves data flow with concurrency 2"),
            max_concurrency=2,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/layer_1.sql": (
                    "MODEL (description 'Test model layer_1.', materialized table);\n\nSELECT 100 AS val"
                ),
                "sql/models/layer_2.sql": (
                    'MODEL (description "Test model layer_2.", materialized table);\n\nSELECT val * 2 AS val FROM __ref("layer_1")'
                ),
                "sql/models/layer_3.sql": (
                    'MODEL (description "Test model layer_3.", materialized table);\n\nSELECT val + 1 AS val FROM __ref("layer_2")'
                ),
            },
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=3,
            expected_query_results=(("SELECT val FROM main.layer_3", ((201,),)),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_build_when_executing_then_succeeds(
    test_case: ConcurrentBuildTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    db_path: Path = tmp_path / "test.duckdb"

    result: BuildExecutionResult = run_concurrent_build(
        test_case=test_case,
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    verify_concurrent_warehouse_state(db_path=db_path, adapter=adapter, test_case=test_case)


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentBuildTestCase(
            description="concurrent provider-backed source loaders share one provider session",
            project_files={},
            max_concurrency=2,
            expected_status=BuildStatus.SUCCESS,
            expected_success_count=4,
            expected_marker_entries=("setup", "alpha", "beta", "teardown"),
            use_provider_session=True,
            expected_query_results=(
                ("SELECT event_id FROM main.fact_alpha", ((1,),)),
                ("SELECT event_id FROM main.fact_beta", ((2,),)),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_provider_backed_loaders_when_executing_then_share_provider_session(
    test_case: ConcurrentBuildTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
) -> None:
    marker_path: Path = tmp_path / "provider-concurrent-marker.log"
    write_repo_files(
        tmp_path,
        {
            "sqlbuild_project.toml": _PROJECT_YML,
            "python/providers/concurrent_marker.py": dedent(
                f"""
                from pathlib import Path
                from time import sleep

                from sqlbuild.providers import Provider


                class ConcurrentMarkerProvider(Provider):
                    '''Test provider ConcurrentMarkerProvider.'''
                    marker_path: str = {str(marker_path)!r}

                    @property
                    def token(self):
                        return str(id(self))

                    def setup(self, ctx):
                        sleep(0.05)
                        self.mark("setup")

                    def teardown(self):
                        self.mark("teardown")

                    def mark(self, label):
                        with Path(self.marker_path).open("a", encoding="utf-8") as marker:
                            marker.write(f"{{label}}:{{self.token}}\\n")
                """
            ).strip()
            + "\n",
            "python/nodes/loaders/events.py": dedent(
                """
                from python.providers.concurrent_marker import ConcurrentMarkerProvider
                from sqlbuild.loaders import loader


                @loader
                def raw_alpha(ctx, concurrent_marker_provider: ConcurrentMarkerProvider):
                    '''Test loader raw_alpha.'''
                    concurrent_marker_provider.mark("alpha")
                    return [{"event_id": 1}]


                @loader
                def raw_beta(ctx, concurrent_marker_provider: ConcurrentMarkerProvider):
                    '''Test loader raw_beta.'''
                    concurrent_marker_provider.mark("beta")
                    return [{"event_id": 2}]
                """
            ).strip()
            + "\n",
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_alpha
                    description: Test source raw_alpha.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: event_id
                        type: INTEGER
                  - name: raw_beta
                    description: Test source raw_beta.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: event_id
                        type: INTEGER
                """
            ).strip()
            + "\n",
            "sql/models/fact_alpha.sql": (
                'MODEL (description "Test model fact_alpha.", materialized table);\n\nSELECT * FROM __source("raw_alpha")\n'
            ),
            "sql/models/fact_beta.sql": (
                'MODEL (description "Test model fact_beta.", materialized table);\n\nSELECT * FROM __source("raw_beta")\n'
            ),
        },
    )
    db_path: Path = tmp_path / "test.duckdb"

    result: BuildExecutionResult = run_concurrent_build(
        test_case=test_case,
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    verify_concurrent_warehouse_state(db_path=db_path, adapter=adapter, test_case=test_case)
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    marker_labels: tuple[str, ...] = tuple(
        entry.split(":", maxsplit=1)[0] for entry in marker_entries
    )
    marker_tokens: set[str] = {entry.split(":", maxsplit=1)[1] for entry in marker_entries}
    assert marker_labels[0] == test_case.expected_marker_entries[0]
    assert set(marker_labels[1:3]) == set(test_case.expected_marker_entries[1:3])
    assert marker_labels[3] == test_case.expected_marker_entries[3]
    assert len(marker_entries) == len(test_case.expected_marker_entries)
    assert len(marker_tokens) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        ConcurrentBuildTestCase(
            description=(
                "failure in one independent branch does not affect the other with concurrency 2"
            ),
            max_concurrency=2,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/broken.sql": (
                    "MODEL (description 'Test model broken.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "sql/models/healthy.sql": (
                    "MODEL (description 'Test model healthy.', materialized table);\n\nSELECT 42 AS val"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_success_count=1,
            expected_failure_count=1,
            expected_model_statuses=(
                ("broken", ExecutionStatus.FAILED),
                ("healthy", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(("SELECT val FROM main.healthy", ((42,),)),),
            expected_missing_relations=("main.broken",),
        ),
        ConcurrentBuildTestCase(
            description=(
                "failure blocks downstream but independent branch still completes with concurrency 2"
            ),
            max_concurrency=2,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/broken.sql": (
                    "MODEL (description 'Test model broken.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "sql/models/downstream.sql": (
                    'MODEL (description "Test model downstream.", materialized table);\n\nSELECT 1 AS id FROM __ref("broken")'
                ),
                "sql/models/independent.sql": (
                    "MODEL (description 'Test model independent.', materialized table);\n\nSELECT 99 AS val"
                ),
            },
            expected_status=BuildStatus.FAILED,
            expected_success_count=1,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("broken", ExecutionStatus.FAILED),
                ("downstream", ExecutionStatus.SKIPPED),
                ("independent", ExecutionStatus.SUCCESS),
            ),
            expected_query_results=(("SELECT val FROM main.independent", ((99,),)),),
            expected_missing_relations=(
                "main.broken",
                "main.downstream",
            ),
        ),
        ConcurrentBuildTestCase(
            description=("fail_fast stops dispatching independent nodes after first failure"),
            max_concurrency=1,
            project_files={
                "sqlbuild_project.toml": _PROJECT_YML,
                "sql/models/aaa_broken.sql": (
                    "MODEL (description 'Test model aaa_broken.', materialized table);\n\nSELECT * FROM nonexistent_table"
                ),
                "sql/models/zzz_independent.sql": (
                    "MODEL (description 'Test model zzz_independent.', materialized table);\n\nSELECT 1 AS id"
                ),
            },
            fail_fast=True,
            expected_status=BuildStatus.FAILED,
            expected_failure_count=1,
            expected_skipped_count=1,
            expected_model_statuses=(
                ("aaa_broken", ExecutionStatus.FAILED),
                ("zzz_independent", ExecutionStatus.SKIPPED),
            ),
            expected_missing_relations=(
                "main.aaa_broken",
                "main.zzz_independent",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_build_when_executing_then_fails(
    test_case: ConcurrentBuildTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    adapter: DuckDbAdapter,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    db_path: Path = tmp_path / "test.duckdb"

    result: BuildExecutionResult = run_concurrent_build(
        test_case=test_case,
        project_dir=tmp_path,
        db_path=db_path,
        adapter=adapter,
    )

    assert result.status == test_case.expected_status
    assert result.success_count == test_case.expected_success_count
    assert result.failure_count == test_case.expected_failure_count
    assert result.skipped_count == test_case.expected_skipped_count

    actual_statuses: dict[str, ExecutionStatus] = {
        r.model_name: r.status for r in result.model_results
    }
    expected_name: str
    expected_status: ExecutionStatus
    for expected_name, expected_status in test_case.expected_model_statuses:
        assert actual_statuses.get(expected_name) == expected_status, (
            f"{expected_name}: expected {expected_status}, got {actual_statuses.get(expected_name)}"
        )

    verify_concurrent_warehouse_state(db_path=db_path, adapter=adapter, test_case=test_case)
