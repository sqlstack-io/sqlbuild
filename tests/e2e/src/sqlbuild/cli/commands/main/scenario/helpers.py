"""Helpers for sqb scenario command e2e tests."""

from __future__ import annotations

import json
import os
import re
import signal
import subprocess
import sys
import time
from collections import defaultdict
from itertools import takewhile
from pathlib import Path
from typing import IO, Any, NamedTuple, cast

from sqlbuild._native import COMPILE_REUSE_DISABLE_ENV_VAR as REUSE_DISABLE_ENV_VAR
from sqlbuild._native import ScenarioArtifactKind
from sqlbuild.executor.scenario._helpers.snapshots.core import (
    build_scenario_snapshot_input_fingerprint,
)
from sqlbuild.executor.scenario.models import ScenarioSnapshotInputSpec
from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    ScenarioPromotionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)


def prepare_namespace_project(tmp_path: Path) -> Path:
    """Create the shared-schema namespace test project."""
    return prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_namespaces",
        repo_files=build_scenario_project_files(),
    )


def change_namespace_fixture(project: Path) -> None:
    """Give the second job different inputs and an independently correct expectation."""
    scenario: Path = project / "tests/scenarios/order_totals_pass.sql"
    scenario.write_text(
        scenario.read_text()
        .replace("10 AS amount", "22 AS amount")
        .replace("15 AS total_amount", "27 AS total_amount")
    )


def run_namespaced_scenario(project: Path, namespace: str, *, retain: bool = False) -> None:
    """Run the real CLI and verify machine output and progress attribution."""
    retain_args: tuple[str, ...] = {True: ("--retain",), False: ()}[retain]
    result: subprocess.CompletedProcess[str] = run_sqb(
        project_dir=project,
        command=(
            "--no-color",
            "scenario",
            "test",
            "order_totals_pass",
            "--scenario-namespace",
            namespace,
            "--json",
            *retain_args,
        ),
    )
    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert payload["execution"] == {
        "scenario_namespace": namespace,
        "scenario_namespace_source": "cli",
    }
    assert f"Scenario namespace: {namespace} (source: cli)" in result.stderr


def build_scenario_project_files() -> dict[str, str]:
    """Build an inline project with passing and failing SQL scenarios."""

    return {
        "sqlbuild_project.toml": (
            'name = "scenario_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "scenario_demo.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized table);\n\n"
            "SELECT\n"
            "  id AS order_id,\n"
            "  amount\n"
            'FROM __source("raw_orders")\n'
        ),
        "models/order_totals.sql": (
            "MODEL (description 'Test model order_totals.', materialized table);\n\n"
            "SELECT\n"
            "  SUM(amount) AS total_amount\n"
            'FROM __ref("orders")\n'
        ),
        "tests/scenarios/order_totals_pass.sql": (
            'SCENARIO (description "Order totals scenario", tags ["scenario"]);\n\n'
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 10 AS amount\n"
            "  UNION ALL\n"
            "  SELECT 2 AS id, 5 AS amount\n"
            "),\n"
            "__expected__order_totals AS (\n"
            "  SELECT 15 AS total_amount\n"
            ")\n"
            "SELECT 1\n"
        ),
        "tests/scenarios/nested/orders_assert_pass.sql": (
            "SCENARIO (description 'Test scenario orders_assert_pass.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 10 AS amount\n"
            "),\n"
            "__assert__no_negative_orders AS (\n"
            '  SELECT * FROM __ref("orders") WHERE amount < 0\n'
            ")\n"
            "SELECT 1\n"
        ),
        "tests/scenarios/order_totals_fail.sql": (
            "SCENARIO (description 'Test scenario order_totals_fail.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 10 AS amount\n"
            "),\n"
            "__expected__order_totals AS (\n"
            "  SELECT 11 AS total_amount\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_partial_fixture_scenario_project_files() -> dict[str, str]:
    """Build a scenario whose required nullable source column is omitted."""

    return {
        "sqlbuild_project.toml": (
            'name = "partial_fixture_scenario"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "partial_fixture_scenario.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
            "      - name: status\n        type: VARCHAR\n"
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT order_id, status FROM __source("raw_orders")\n'
        ),
        "tests/scenarios/partial_orders.sql": (
            "SCENARIO (description 'Test scenario partial_orders.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS order_id),\n"
            "__expected__orders AS (\n"
            "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_scenario_python_hooks_project_files() -> dict[str, str]:
    """Build an inline scenario project with a Python lifecycle hook."""

    repo_files: dict[str, str] = build_scenario_project_files()
    repo_files["hooks/python/scenario_hooks.py"] = (
        "from sqlbuild.hooks import hook\n\n"
        "@hook\n"
        "def log_scenario_model(ctx):\n"
        "    '''Test hook log_scenario_model.'''\n    ctx.execute_sql(\n"
        '        f"CREATE TABLE {ctx.destination.schema}.scenario_hook_log AS "\n'
        "        f\"SELECT '{ctx.model_name}' AS model_name, '{ctx.phase}' AS phase\"\n"
        "    )\n"
    )
    repo_files["models/orders.sql"] = (
        'MODEL (description "Test model orders.", '
        'materialized table, post_hooks [python("log_scenario_model")]);\n\n'
        "SELECT\n"
        "  id AS order_id,\n"
        "  amount\n"
        'FROM __source("raw_orders")\n'
    )
    return repo_files


def build_scenario_dependency_chain_project_files() -> dict[str, str]:
    """Build a three-model scenario with partial source fixtures and final checks."""

    repo_files: dict[str, str] = build_partial_fixture_scenario_project_files()
    repo_files["models/orders_summary.sql"] = (
        'MODEL (description "Summarize fixture orders.", materialized table);\n'
        'SELECT order_id, status FROM __ref("orders")\n'
    )
    repo_files["models/orders_final.sql"] = (
        'MODEL (description "Final scenario output.", materialized table);\n'
        'SELECT order_id, status FROM __ref("orders_summary")\n'
    )
    repo_files["tests/scenarios/partial_orders.sql"] = (
        "SCENARIO (description 'Check a dependency chain.');\n"
        "WITH helper AS (SELECT 1 AS order_id),\n"
        "__source__raw_orders AS (SELECT order_id FROM helper),\n"
        "__expected__orders_final AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        "),\n"
        '__assert__positive_ids AS (SELECT * FROM __ref("orders_final") WHERE order_id < 1)\n'
        "SELECT 1\n"
    )
    return repo_files


def build_capture_safety_project_files(*, use_project_row_limit: bool) -> dict[str, str]:
    """Build scenario e2e files with optional project snapshot limit config."""

    repo_files: dict[str, str] = build_scenario_project_files()
    snapshot_limit_blocks: dict[bool, str] = {
        False: "",
        True: "\n[scenario.snapshot_limits]\nmax_rows_per_relation = 1\n",
    }
    repo_files["sqlbuild_project.toml"] += snapshot_limit_blocks[use_project_row_limit]
    return repo_files


def build_real_warehouse_local_replay_project_files(
    *,
    project_toml: str,
    model_sql: str,
    scenario_sql: str,
    scenario_name: str = "transpilable_event_rollup",
) -> dict[str, str]:
    """Build an inline scenario project for remote capture followed by local replay."""

    return {
        "sqlbuild_project.toml": project_toml,
        "sources/raw.yml": (
            "sources:\n  - name: raw_events\n    description: Test source raw_events.\n"
            "    schema: raw\n    table: raw_events\n"
        ),
        "models/event_rollup.sql": model_sql,
        f"tests/scenarios/{scenario_name}.sql": scenario_sql,
    }


def build_real_warehouse_remote_scenario_project_files(*, project_toml: str) -> dict[str, str]:
    """Build an inline scenario project for live remote scenario execution."""

    return {
        "sqlbuild_project.toml": project_toml,
        "sources/raw.yml": (
            "sources:\n  - name: raw_events\n    description: Test source raw_events.\n"
            "    schema: raw\n    table: raw_events\n"
        ),
        "models/stg_events.sql": (
            "MODEL (description 'Test model stg_events.', materialized view);\n\n"
            "SELECT customer_id, amount_cents\n"
            'FROM __source("raw_events")\n'
            "WHERE amount_cents >= 1000\n"
        ),
        "models/event_rollup.sql": (
            "MODEL (description 'Test model event_rollup.', materialized table);\n\n"
            "SELECT\n"
            "  customer_id,\n"
            "  SUM(amount_cents) AS large_amount_cents,\n"
            "  COUNT(*) AS event_count\n"
            'FROM __ref("stg_events")\n'
            "GROUP BY customer_id\n"
        ),
        "tests/scenarios/remote_event_rollup.sql": (
            "SCENARIO (description 'Test scenario remote_event_rollup.');\n\n"
            "WITH\n"
            "__source__raw_events AS (\n"
            "  SELECT 10 AS customer_id, 1500 AS amount_cents\n"
            "  UNION ALL\n"
            "  SELECT 10 AS customer_id, 500 AS amount_cents\n"
            "),\n"
            "__expected__event_rollup AS (\n"
            "  SELECT 10 AS customer_id, 1500 AS large_amount_cents, 1 AS event_count\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def list_scenario_relation_names(*, db_path: Path) -> tuple[str, ...]:
    """Return DuckDB relation names owned by scenario artifact prefixes."""

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=(
            "SELECT table_name FROM information_schema.tables "
            "WHERE table_schema = 'main' AND table_name LIKE '__sqb_%' "
            "ORDER BY table_name"
        ),
    )
    return tuple(str(row[0]) for row in rows)


def scenario_relation_name_by_suffix(*, db_path: Path, suffix: str) -> str:
    """Return one retained scenario relation name ending with the requested suffix."""

    names_by_suffix_match: defaultdict[bool, list[str]] = defaultdict(list)
    for relation_name in list_scenario_relation_names(db_path=db_path):
        names_by_suffix_match[relation_name.endswith(suffix)].append(relation_name)
    matches: tuple[str, ...] = tuple(names_by_suffix_match[True])
    assert len(matches) == 1
    return matches[0]


def scenario_relation_row_count(*, db_path: Path, relation_name: str) -> int:
    """Return row count for one retained scenario relation."""

    rows: list[tuple[object, ...]] = query_duckdb(
        db_path=db_path,
        sql=f'SELECT COUNT(*) FROM main."{relation_name}"',
    )
    return int(rows[0][0])


def assert_runtime_artifact_contains(
    *, project_dir: Path, relative_path: Path, expected_fragments: tuple[str, ...]
) -> None:
    """Assert a scenario runtime artifact exists and contains expected fragments."""

    artifact_path: Path = project_dir / relative_path
    assert artifact_path.exists(), f"missing runtime artifact: {artifact_path}"
    content: str = artifact_path.read_text(encoding="utf-8")
    expected_fragment: str
    for expected_fragment in expected_fragments:
        assert expected_fragment in content


def assert_scenario_snapshot(
    *,
    project_dir: Path,
    scenario_name: str,
    expected_row_count: int,
    expected_local_types: dict[str, str] | None = None,
) -> None:
    """Assert a scenario snapshot manifest and source JSONL file were written."""

    snapshot_root: Path = project_dir / "tests" / "_scenario_snapshots" / scenario_name
    manifest_path: Path = snapshot_root / "scenario.json"
    jsonl_path: Path = snapshot_root / "sources" / "raw_orders.jsonl"
    assert manifest_path.exists()
    assert jsonl_path.exists()

    manifest_data: object = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert isinstance(manifest_data, dict)
    assert manifest_data["scenario_name"] == scenario_name
    assert manifest_data["format"] == "jsonl"
    assert manifest_data["total_rows"] == expected_row_count
    assert manifest_data["relations"][0]["file"] == "sources/raw_orders.jsonl"
    assert manifest_data["relations"][0]["row_count"] == expected_row_count
    columns: object = manifest_data["relations"][0]["columns"]
    assert isinstance(columns, list)
    assert columns
    assert all(isinstance(column, dict) for column in columns)
    typed_columns: list[dict[str, object]] = cast(list[dict[str, object]], columns)
    column_names: set[str] = {str(column["name"]) for column in typed_columns}
    assert {"id", "amount"}.issubset(column_names)
    local_types_by_name: dict[str, str] = {
        str(column["name"]): str(column["local_type"]) for column in typed_columns
    }
    assert local_types_by_name | (expected_local_types or {}) == local_types_by_name
    column: dict[str, object]
    for column in typed_columns:
        assert column["warehouse_type"]
        assert column["local_type"]

    rows: list[str] = jsonl_path.read_text(encoding="utf-8").splitlines()
    assert len(rows) == expected_row_count


def maybe_corrupt_scenario_snapshot_jsonl(
    *, project_dir: Path, scenario_name: str, enabled: bool
) -> None:
    """Optionally replace one captured source JSONL file with malformed content."""

    for _ in range(int(enabled)):
        jsonl_path: Path = (
            project_dir
            / "tests"
            / "_scenario_snapshots"
            / scenario_name
            / "sources"
            / "raw_orders.jsonl"
        )
        jsonl_path.write_text('{"id": 1, "amount": 10}\nnot-json\n', encoding="utf-8")


def maybe_corrupt_scenario_snapshot_dialect(
    *, project_dir: Path, scenario_name: str, enabled: bool
) -> None:
    """Optionally replace the captured dialect with an unsupported dialect name."""

    for _ in range(int(enabled)):
        manifest_path: Path = (
            project_dir / "tests" / "_scenario_snapshots" / scenario_name / "scenario.json"
        )
        manifest_data: object = json.loads(manifest_path.read_text(encoding="utf-8"))
        assert isinstance(manifest_data, dict)
        manifest_data["capture_dialect"] = "not_a_sql_analysis_dialect"
        manifest_path.write_text(json.dumps(manifest_data, indent=2) + "\n", encoding="utf-8")


def maybe_capture_scenario_snapshot(
    *, project_dir: Path, scenario_name: str, enabled: bool
) -> None:
    """Optionally capture a scenario snapshot for an e2e project."""

    for _ in range(int(enabled)):
        capture_result: subprocess.CompletedProcess[str] = run_sqb(
            command=("--no-color", "scenario", "capture", scenario_name),
            project_dir=project_dir,
        )
        assert capture_result.returncode == 0, capture_result.stdout + capture_result.stderr


def write_committed_order_totals_pass_snapshot(*, project_dir: Path) -> None:
    """Write a small snapshot fixture without running scenario capture."""

    snapshot_root: Path = project_dir / "tests" / "_scenario_snapshots" / "order_totals_pass"
    sources_root: Path = snapshot_root / "sources"
    sources_root.mkdir(parents=True)
    (snapshot_root / "scenario.json").write_text(
        json.dumps(
            {
                "capture_adapter": "duckdb",
                "capture_dialect": "duckdb",
                "captured_at": "2026-05-10T13:16:38Z",
                "format": "jsonl",
                "input_fingerprint": (
                    "b768a0f159b65141c9eb9f460b80f350679e1362477a8fc527b24911fb211112"
                ),
                "relations": [
                    {
                        "bytes": 41,
                        "columns": [
                            {
                                "local_type": "INT",
                                "name": "id",
                                "warehouse_type": "INTEGER",
                            },
                            {
                                "local_type": "INT",
                                "name": "amount",
                                "warehouse_type": "INTEGER",
                            },
                        ],
                        "file": "sources/raw_orders.jsonl",
                        "kind": "source",
                        "logical_name": "raw_orders",
                        "row_count": 2,
                    }
                ],
                "scenario_name": "order_totals_pass",
                "sqlbuild_version": "0.2.1",
                "total_bytes": 41,
                "total_rows": 2,
                "version": 1,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    (sources_root / "raw_orders.jsonl").write_text(
        '{"amount":10,"id":1}\n{"amount":5,"id":2}\n',
        encoding="utf-8",
    )


def write_stale_order_totals_scenario(*, project_dir: Path) -> None:
    """Change the order totals scenario so a prior snapshot becomes stale."""

    scenario_path: Path = project_dir / "tests" / "scenarios" / "order_totals_pass.sql"
    scenario_path.write_text(
        'SCENARIO (description "Order totals scenario", tags ["scenario"]);\n\n'
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 1 AS id, 10 AS amount\n"
        "  UNION ALL\n"
        "  SELECT 2 AS id, 5 AS amount\n"
        "  UNION ALL\n"
        "  SELECT 3 AS id, 3 AS amount\n"
        "),\n"
        "__expected__order_totals AS (\n"
        "  SELECT 18 AS total_amount\n"
        ")\n"
        "SELECT 1\n",
        encoding="utf-8",
    )


def maybe_write_stale_order_totals_scenario(*, project_dir: Path, enabled: bool) -> None:
    """Optionally change a scenario so a prior snapshot becomes stale."""

    for _ in range(int(enabled)):
        write_stale_order_totals_scenario(project_dir=project_dir)


def assert_local_duckdb_state(
    *,
    db_path: Path,
    stdout: str,
    expected_exists: bool,
    query_when_exists: bool,
    count_sql: str,
    expected_count: int,
    rows_sql: str | None = None,
    expected_rows: tuple[tuple[object, ...], ...] = (),
) -> None:
    """Assert retained local DuckDB state for a local scenario run."""

    assert db_path.exists() is expected_exists
    for _ in range(int(query_when_exists)):
        assert f"Retained local DuckDB: {db_path.as_posix()}" in stdout
        rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=count_sql)
        assert rows == [(expected_count,)]
        for value_rows_sql in (rows_sql,) * int(rows_sql is not None):
            assert value_rows_sql is not None
            value_rows: list[tuple[object, ...]] = query_duckdb(db_path=db_path, sql=value_rows_sql)
            assert tuple(value_rows) == expected_rows


def assert_optional_local_replay_rows(
    *,
    project_dir: Path,
    scenario_name: str,
    local_rows_sql: str,
    expected_local_rows: tuple[tuple[object, ...], ...],
) -> None:
    """Assert local replay rows when a test case expects inspectable local output."""

    for query_sql in (local_rows_sql,) * int(bool(local_rows_sql)):
        rows: list[tuple[object, ...]] = query_duckdb(
            db_path=project_dir / "target" / "run" / "scenarios" / scenario_name / "local.duckdb",
            sql=query_sql,
        )
        assert tuple(rows) == expected_local_rows


def build_offline_snowflake_project_toml() -> str:
    """Return a Snowflake project config that compiles offline for local replay."""

    return (
        'name = "scenario_demo"\n'
        'adapter = "snowflake"\n\n'
        "[connection]\n"
        'account = "example"\n'
        'user = "example"\n'
        'database = "analytics"\n'
        'warehouse = "compute"\n\n'
        "[defaults]\n"
        'materialized = "table"\n'
        'database = "analytics"\n'
        'schema = "main"\n'
    )


def restamp_scenario_snapshot_capture_adapter(
    *,
    project_dir: Path,
    scenario_name: str,
    source_fixture_sql: dict[str, str],
    capture_adapter: str,
) -> None:
    """Rewrite a captured snapshot so it reads as captured by another project adapter."""

    manifest_path: Path = (
        project_dir / "tests" / "_scenario_snapshots" / scenario_name / "scenario.json"
    )
    manifest_data: object = json.loads(manifest_path.read_text(encoding="utf-8"))
    assert isinstance(manifest_data, dict)
    manifest_data["capture_adapter"] = capture_adapter
    manifest_data["capture_dialect"] = capture_adapter
    manifest_data["input_fingerprint"] = build_scenario_snapshot_input_fingerprint(
        scenario_name=scenario_name,
        input_specs=tuple(
            ScenarioSnapshotInputSpec(
                kind=ScenarioArtifactKind.SOURCE,
                logical_name=source_name,
                file_path=Path("sources") / f"{source_name}.jsonl",
                capture_sql=fixture_sql,
            )
            for source_name, fixture_sql in source_fixture_sql.items()
        ),
        capture_adapter=capture_adapter,
        capture_dialect=capture_adapter,
    )
    manifest_path.write_text(json.dumps(manifest_data, indent=2) + "\n", encoding="utf-8")


def build_promotion_project_files(test_case: ScenarioPromotionE2ETestCase) -> dict[str, str]:
    """Build a one-model project whose scenario exercises table promotion settings."""

    return {
        "sqlbuild_project.toml": (
            'name = "scenario_promotion"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "scenario_promotion.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
            f"{test_case.defaults_config}\n"
            f"{test_case.settings_config}"
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n"
            "    description: Raw orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            "      - name: amount\n"
            "        type: INTEGER\n"
        ),
        "models/order_totals.sql": (
            "MODEL (\n"
            "  description 'Order totals.',\n"
            "  materialized table,\n"
            "  sql_analysis false,\n"
            f"{test_case.model_columns}"
            ");\n\n"
            "SELECT\n"
            "  CAST(SUM(amount) AS BIGINT) AS total_amount,\n"
            "  CAST(COUNT(*) AS BIGINT) AS order_count\n"
            'FROM __source("raw_orders")\n'
        ),
        "tests/scenarios/order_totals_pass.sql": (
            "SCENARIO (description 'Order totals scenario.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 10 AS amount\n"
            "  UNION ALL\n"
            "  SELECT 2 AS id, 5 AS amount\n"
            "),\n"
            "__expected__order_totals AS (\n"
            "  SELECT 15 AS total_amount, 2 AS order_count\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def scenario_result_lines(stdout: str) -> tuple[str, ...]:
    """Return scenario result rows and expectation rows without timings, in output order."""

    rows: list[str] = re.findall(
        r"^(?:order_|orders_|    expect|    expected |    assertion ).*$", stdout, re.M
    )
    return tuple(re.sub(r"\s+\d+\.\d\ds$", "", row).rstrip() for row in rows)


def build_empty_fixture_scenario_project_files(*, customer_columns_yaml: str) -> dict[str, str]:
    """Build a scenario that mocks one source as empty through `__empty_fixture()`."""

    return {
        "sqlbuild_project.toml": (
            'name = "scenario_empty_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "scenario_empty_fixture.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n\n'
            "[rules]\n"
            'select = ["SQLCLEAN021"]\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n"
            "    description: Raw orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: id\n"
            "        type: INTEGER\n"
            "      - name: customer_id\n"
            "        type: INTEGER\n"
            "  - name: raw_customers\n"
            "    description: Raw customers.\n"
            "    schema: main\n"
            "    table: raw_customers\n"
            f"{customer_columns_yaml}"
        ),
        "models/customer_orders.sql": (
            "MODEL (description 'Orders joined to known customers.', materialized table);\n\n"
            "SELECT o.id AS order_id, c.name AS customer_name\n"
            'FROM __source("raw_orders") AS o\n'
            'JOIN __source("raw_customers") AS c ON c.id = o.customer_id\n'
        ),
        "tests/scenarios/orders_without_customers.sql": (
            "SCENARIO (description 'Orders without known customers produce no rows.');\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 7 AS customer_id\n"
            "),\n"
            "__source__raw_customers AS (\n"
            "  SELECT * FROM __empty_fixture()\n"
            "),\n"
            "__assert__no_unmatched_orders AS (\n"
            '  SELECT order_id FROM __ref("customer_orders")\n'
            ")\n"
            "SELECT 1\n"
        ),
    }


INTERRUPT_HOOK_SECONDS_ENV_VAR: str = "SCENARIO_INTERRUPT_HOOK_SECONDS"


class InterruptedScenarioRun(NamedTuple):
    """Outcome of a scenario command interrupted through SIGINT."""

    returncode: int
    stdout: str
    output: str
    stop_seconds: float


_ENDLESS_SUM: str = "(SELECT SUM(big.n) FROM range(1000000000000) AS big(n)) * 0"
_FIXTURE_AMOUNTS: dict[bool, str] = {True: f"CAST(10 + {_ENDLESS_SUM} AS INTEGER)", False: "10"}
_MODEL_TOTALS: dict[bool, str] = {True: f"SUM(o.amount) + {_ENDLESS_SUM}", False: "SUM(o.amount)"}
_POST_HOOKS: dict[bool, str] = {True: ', post_hooks [python("pause_after_build")]', False: ""}
_HOOK_FILES: dict[bool, dict[str, str]] = {
    True: {
        "hooks/python/slow_hooks.py": (
            "import os\n"
            "import time\n\n"
            "from sqlbuild.hooks import hook\n\n\n"
            "@hook\n"
            "def pause_after_build(ctx):\n"
            '    """Pause so an interrupt lands while scenarios run."""\n'
            f"    time.sleep(float(os.environ.get('{INTERRUPT_HOOK_SECONDS_ENV_VAR}', '0')))\n"
        )
    },
    False: {},
}


def build_interrupt_project_files(
    *, scenario_count: int, long_model: bool, long_fixture: bool, slow_hook: bool
) -> dict[str, str]:
    """Build scenarios whose model, fixture, or post-hook runs until interrupted."""

    scenario_sql: str = (
        "WITH\n"
        "__source__raw_orders AS (\n"
        f"  SELECT 1 AS id, {_FIXTURE_AMOUNTS[long_fixture]} AS amount\n"
        "),\n"
        "__expected__order_totals AS (\n"
        "  SELECT 10 AS total_amount\n"
        ")\n"
        "SELECT 1\n"
    )
    return {
        **_HOOK_FILES[slow_hook],
        "sqlbuild_project.toml": (
            'name = "scenario_interrupt"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "scenario_interrupt.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n"
            "    description: Raw orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
        ),
        "models/order_totals.sql": (
            "MODEL (description 'Order totals.', materialized table, sql_analysis false"
            f"{_POST_HOOKS[slow_hook]});\n\n"
            f"SELECT {_MODEL_TOTALS[long_model]} AS total_amount "
            'FROM __source("raw_orders") AS o\n'
        ),
        **{
            f"tests/scenarios/slow_{index}.sql": (
                f"SCENARIO (description 'Slow scenario {index}.');\n\n{scenario_sql}"
            )
            for index in range(scenario_count)
        },
    }


def interrupt_scenario_run(
    *,
    project_dir: Path,
    args: tuple[str, ...],
    trigger: str,
    trigger_stream: str,
    interrupt_count: int,
    hook_seconds: float = 0.0,
) -> InterruptedScenarioRun:
    """Interrupt a scenario command once ``trigger`` is printed on ``trigger_stream``."""

    process: subprocess.Popen[str] = subprocess.Popen(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--no-color",
            "--project-dir",
            str(project_dir),
            "scenario",
            *args,
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env={**os.environ, INTERRUPT_HOOK_SECONDS_ENV_VAR: str(hook_seconds)},
    )
    streams: dict[str, IO[str] | None] = {"stdout": process.stdout, "stderr": process.stderr}
    watched: IO[str] | None = streams[trigger_stream]
    assert watched is not None
    started: str = "".join(takewhile(lambda line: trigger not in line, watched))
    time.sleep(1.0)
    interrupted_at: float = time.monotonic()
    for _ in range(interrupt_count):
        process.send_signal(signal.SIGINT)
        time.sleep(0.5)
    stdout, stderr = process.communicate(timeout=120)
    return InterruptedScenarioRun(
        returncode=process.returncode,
        stdout=stdout,
        output=started + stdout + stderr,
        stop_seconds=time.monotonic() - interrupted_at,
    )


def scenario_relation_names_if_present(*, db_path: Path) -> tuple[str, ...]:
    """Return scenario-owned DuckDB relation names, or none when the database was never made."""

    return {True: list_scenario_relation_names, False: lambda **_: ()}[db_path.exists()](
        db_path=db_path
    )


COMPILE_REUSE_HIT_LINE: str = "Inputs unchanged; reused the previous compile"


def write_project_settings(*, project_dir: Path, base_toml: str, settings: str) -> None:
    """Rewrite the project file with a settings block appended to its base content."""

    _ = (project_dir / "sqlbuild_project.toml").write_text(base_toml + settings, encoding="utf-8")


def run_reusable_compile(*, project_dir: Path) -> subprocess.CompletedProcess[str]:
    """Compile in a fresh process with whole-project compile reuse enabled."""

    return subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(project_dir),
            "--no-color",
            "compile",
        ],
        capture_output=True,
        text=True,
        env={**os.environ, REUSE_DISABLE_ENV_VAR: "0"},
        check=False,
    )


_EXPECTED_COLUMNS_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "scenario_expected_columns"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "warehouse.duckdb"\n'
    ),
    "models/orders.sql": (
        'MODEL (description "Orders", materialized table);\n\n'
        "SELECT order_id, status\n"
        "FROM (VALUES ('o1', 'placed')) AS t(order_id, status)\n"
    ),
    "models/order_flags.sql": (
        'MODEL (description "Orders with a flag", materialized table);\n\n'
        'WITH orders AS (SELECT * FROM __ref("orders"))\n\n'
        "SELECT order_id, status, status = 'cancelled' AS is_cancelled\n"
        "FROM orders\n"
    ),
}
_EXPECTED_COLUMNS_SCENARIO_PATH: str = "tests/scenarios/order_flags__expected.sql"
_EXPECTED_COLUMNS_DESCRIPTION: str = 'description "Order flags keep their status"'
_EXPECTED_COLUMNS_FIXTURE: str = (
    "WITH __ref__orders AS (\n  SELECT 'o1' AS order_id, 'placed' AS status\n),\n\n"
)


def build_expected_columns_scenario_project_files(
    *, expected_sql: str, header_options: str
) -> dict[str, str]:
    """Build an order-flags project whose scenario expects `expected_sql` for the model."""

    return {
        **_EXPECTED_COLUMNS_PROJECT_FILES,
        _EXPECTED_COLUMNS_SCENARIO_PATH: (
            f"SCENARIO ({_EXPECTED_COLUMNS_DESCRIPTION}{header_options});\n\n"
            f"{_EXPECTED_COLUMNS_FIXTURE}"
            f"__expected__order_flags AS (\n  {expected_sql}\n)\n"
        ),
    }


def selected_fields(result: dict[str, Any], expected: dict[str, object]) -> dict[str, object]:
    """Return the fields of `result` that `expected` names."""

    return {key: result.get(key) for key in expected}


def build_helper_scenario_project_files(*, helper_ctes: str, check_ctes: str) -> dict[str, str]:
    """Build an order-flags project whose scenario declares helpers ahead of its checks."""

    return {
        **_EXPECTED_COLUMNS_PROJECT_FILES,
        _EXPECTED_COLUMNS_SCENARIO_PATH: (
            f"SCENARIO ({_EXPECTED_COLUMNS_DESCRIPTION});\n\n"
            f"{_EXPECTED_COLUMNS_FIXTURE}{helper_ctes},\n\n{check_ctes}\n\nSELECT 1\n"
        ),
    }


_UNMOCKED_REP_SOURCES: tuple[str, ...] = tuple(f"raw_reps_{index}" for index in range(8))


def build_unmocked_input_scenario_project_files() -> dict[str, str]:
    """Build scenarios that mock one input of order_reps and leave a source-backed one real."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "unmocked_inputs"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "unmocked_inputs.duckdb"\n'
        ),
        "sources/raw.yml": "sources:\n"
        + "".join(
            f"  - name: {name}\n    description: Test source {name}.\n"
            f"    schema: main\n    table: {name}\n"
            for name in ("raw_orders", *_UNMOCKED_REP_SOURCES)
        ),
        "models/stg_orders.sql": (
            'MODEL (description "Test model stg_orders.");\n\n'
            'SELECT order_id, rep_id FROM __source("raw_orders")\n'
        ),
        "models/stg_sales_reps.sql": (
            'MODEL (description "Test model stg_sales_reps.");\n\n'
            + "\nUNION ALL\n".join(
                f'SELECT rep_id, rep_name FROM __source("{name}")' for name in _UNMOCKED_REP_SOURCES
            )
            + "\n"
        ),
        "models/order_reps.sql": (
            'MODEL (description "Test model order_reps.");\n\n'
            "SELECT o.order_id, r.rep_name\n"
            'FROM __ref("stg_orders") AS o\n'
            'JOIN __ref("stg_sales_reps") AS r ON o.rep_id = r.rep_id\n'
        ),
        "tests/scenarios/order_reps_ok.sql": (
            "SCENARIO (description 'Test scenario order_reps_ok.');\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 7 AS rep_id),\n"
            "__ref__stg_sales_reps AS (SELECT 7 AS rep_id, 'Ana' AS rep_name),\n"
            "__expected__order_reps AS (SELECT 1 AS order_id, 'Ana' AS rep_name)\n"
            "SELECT 1\n"
        ),
        "tests/scenarios/order_reps_unmocked.sql": (
            "SCENARIO (description 'Test scenario order_reps_unmocked.');\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 7 AS rep_id),\n"
            "__expected__order_reps AS (SELECT 1 AS order_id, 'Ana' AS rep_name)\n"
            "SELECT 1\n"
        ),
    }
    return files


def build_lambda_scenario_project_files() -> dict[str, str]:
    """Build a scenario whose model reads list items through lambdas beside a star CTE."""

    return {
        "sqlbuild_project.toml": (
            'name = "lambda_scenario"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "lambda_scenario.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_sessions\n    description: Test source raw_sessions.\n"
            "    schema: main\n    table: raw_sessions\n"
        ),
        "models/sessions.sql": (
            "MODEL (description 'Test model sessions.');\n"
            'SELECT * FROM __source("raw_sessions")\n'
        ),
        "models/carts.sql": (
            "MODEL (description 'Test model carts.');\n"
            "SELECT 1 AS session_id, [1, 2, 3] AS quantities\n"
        ),
        "models/cart_totals.sql": (
            "MODEL (description 'Test model cart_totals.');\n"
            'WITH sess AS (SELECT * FROM __ref("sessions")),\n'
            'crt AS (SELECT c.session_id, c.quantities FROM __ref("carts") AS c)\n'
            "SELECT\n  s.session_id,\n"
            "  list_reduce(list_transform(c.quantities, lambda q: q * 2), lambda acc, x: acc + x)"
            " AS total,\n"
            "  len(list_filter(c.quantities, lambda i: i > 1)) AS big\n"
            "FROM sess AS s\nINNER JOIN crt AS c ON s.session_id = c.session_id\n"
            "WHERE len(list_filter(c.quantities, lambda w: w < 0)) = 0\n"
        ),
        "tests/scenarios/cart_totals_doubled.sql": (
            "SCENARIO (description 'Test scenario cart_totals_doubled.');\n"
            "WITH\n__ref__sessions AS (SELECT 1 AS session_id),\n"
            "__ref__carts AS (SELECT 1 AS session_id, [1, 2, 3] AS quantities),\n"
            "__expected__cart_totals AS (SELECT 1 AS session_id, 12 AS total, 2 AS big)\n"
            "SELECT 1\n"
        ),
    }


def build_contracted_star_empty_fixture_project_files() -> dict[str, str]:
    """Build a scenario that empties a contract-enforced model whose query selects a star."""

    return {
        "sqlbuild_project.toml": (
            'name = "contracted_star"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "contracted_star.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_customers\n    description: Test source raw_customers.\n"
            "    schema: main\n    table: raw_customers\n"
        ),
        "models/customers.sql": (
            "MODEL (description 'Test model customers.', contract enforced,\n"
            "  columns (id (type INTEGER), name (type VARCHAR)));\n"
            'SELECT * FROM __source("raw_customers")\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.');\nSELECT 1 AS id, 7 AS customer_id\n"
        ),
        "models/customer_orders.sql": (
            "MODEL (description 'Test model customer_orders.');\n"
            "SELECT o.id AS order_id, c.name AS customer_name\n"
            'FROM __ref("orders") AS o\nJOIN __ref("customers") AS c ON c.id = o.customer_id\n'
        ),
        "tests/scenarios/orders_without_customers.sql": (
            "SCENARIO (description 'Test scenario orders_without_customers.');\n"
            "WITH\n__ref__orders AS (SELECT 1 AS id, 7 AS customer_id),\n"
            "__ref__customers AS (SELECT * FROM __empty_fixture()),\n"
            "__expected__customer_orders AS (\n"
            "  SELECT CAST(NULL AS INTEGER) AS order_id, CAST(NULL AS VARCHAR) AS customer_name\n"
            "  WHERE FALSE\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_cursor_intrinsic_scenario_project_files() -> dict[str, str]:
    """Build a scenario over a non-microbatch incremental model that reads cursor intrinsics."""

    return {
        "sqlbuild_project.toml": (
            'name = "cursor_scenario"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "cursor_scenario.duckdb"\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.');\n"
            "SELECT 1 AS order_id, TIMESTAMP '2026-01-02 10:00:00' AS ordered_at\n"
        ),
        "models/hourly_orders.sql": (
            "MODEL (\n  description 'Test model hourly_orders.',\n  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n  cursor ordered_at,\n"
            "  cursor_type timestamp,\n  cursor_grain hour,\n  cursor_inputs (orders ordered_at),\n"
            ");\n"
            'SELECT o.order_id, o.ordered_at FROM __ref("orders") AS o\n'
            "WHERE o.ordered_at >= __cursor_start() AND o.ordered_at < __cursor_end()\n"
        ),
        "tests/scenarios/hourly_orders_window.sql": (
            "SCENARIO (description 'Test scenario hourly_orders_window.');\n"
            "WITH\n__ref__orders AS (\n"
            "  SELECT 1 AS order_id, TIMESTAMP '2026-01-02 10:00:00' AS ordered_at\n),\n"
            "__expected__hourly_orders AS (\n"
            "  SELECT 1 AS order_id, TIMESTAMP '2026-01-02 10:00:00' AS ordered_at\n)\n"
            "SELECT 1\n"
        ),
    }


def build_connect_by_scenario_project_files() -> dict[str, str]:
    """Build a Snowflake scenario whose model walks a hierarchy beside a star CTE."""

    return {
        "sqlbuild_project.toml": (
            'name = "connect_by_scenario"\nadapter = "snowflake"\ndefault_target = "dev"\n\n'
            '[targets.dev]\ndatabase = "warehouse"\nschema = "analytics"\n\n'
            "[scopes]\nenforce_placement = false\n"
        ),
        "models/categories.sql": (
            "MODEL (description 'Test model categories.');\n"
            "SELECT CAST(1 AS INTEGER) AS category_id, CAST(NULL AS INTEGER) AS parent_id,\n"
            "  CAST('root' AS VARCHAR) AS name\n"
        ),
        "models/category_paths.sql": (
            "MODEL (description 'Test model category_paths.');\n"
            'WITH categories AS (SELECT * FROM __ref("categories")),\n'
            "tree AS (\n"
            "  SELECT c.category_id, LEVEL AS depth, SYS_CONNECT_BY_PATH(c.name, '/') AS path\n"
            "  FROM categories AS c\n"
            "  START WITH c.parent_id IS NULL\n"
            "  CONNECT BY PRIOR c.category_id = c.parent_id\n"
            ")\n"
            "SELECT category_id, depth, path FROM tree\n"
        ),
        "tests/scenarios/category_paths_walk.sql": (
            "SCENARIO (description 'Test scenario category_paths_walk.');\n"
            "WITH\n__ref__categories AS (\n"
            "  SELECT 1 AS category_id, CAST(NULL AS INTEGER) AS parent_id, 'root' AS name\n),\n"
            "__expected__category_paths AS (SELECT 1 AS category_id, 1 AS depth, '/root' AS path)\n"
            "SELECT 1\n"
        ),
    }


def build_local_variant_scenario_project_files(
    *,
    output: str = "o.payload IS NOT NULL AS has_payload",
    expected: str = "TRUE AS has_payload",
    fixture: str = '\'{"qty": 2, "tags": ["a"]}\'::JSON::VARIANT',
) -> dict[str, str]:
    """Build an in-memory DuckDB scenario whose source fixture holds a VARIANT column."""

    return {
        "sqlbuild_project.toml": (
            'name = "local_variant"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n\n'
            "[scopes]\nenforce_placement = false\n"
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n    columns:\n"
            "      - {name: order_id, type: INTEGER}\n      - {name: payload, type: VARIANT}\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.');\n"
            f"SELECT o.order_id, {output}\n"
            'FROM __source("raw_orders") AS o\n'
        ),
        "tests/scenarios/orders__keep_payload.sql": (
            "SCENARIO (description 'Test scenario orders__keep_payload.');\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            f"  SELECT 1 AS order_id,\n  {fixture} AS payload\n),\n"
            f"__expected__stg_orders AS (SELECT 1 AS order_id, {expected})\n"
            "SELECT 1\n"
        ),
    }


def build_helper_fixture_scenario_project_files() -> dict[str, str]:
    """Build an in-memory DuckDB scenario whose source fixture reads a helper CTE."""

    return {
        "sqlbuild_project.toml": (
            'name = "helper_fixture"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n\n'
            "[scopes]\nenforce_placement = false\n"
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n    columns:\n"
            "      - {name: order_id, type: INTEGER}\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.');\n"
            'SELECT o.order_id FROM __source("raw_orders") AS o\n'
        ),
        "tests/scenarios/orders__from_helper.sql": (
            "SCENARIO (description 'Test scenario orders__from_helper.');\n"
            "WITH\n"
            "helper_ids AS (SELECT 1 AS order_id),\n"
            "__source__raw_orders AS (SELECT h.order_id FROM helper_ids AS h),\n"
            "__expected__stg_orders AS (SELECT 1 AS order_id)\n"
            "SELECT 1\n"
        ),
    }


SNOWFLAKE_VARIANT_FIXTURE_SQL: str = (
    'SELECT 1 AS order_id, \'a\' AS status, PARSE_JSON(\'{"qty": 2, "note": "x"}\') AS payload'
)


def build_snowflake_variant_scenario_project_files(*, expected_payload_json: str) -> dict[str, str]:
    """Build an offline Snowflake project whose scenario compares a VARIANT with PARSE_JSON."""

    return {
        "sqlbuild_project.toml": build_offline_snowflake_project_toml(),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
            "    schema: main\n    table: raw_orders\n    columns:\n"
            '      - {name: order_id, type: "NUMBER(38,0)"}\n'
            "      - {name: status, type: VARCHAR}\n"
            "      - {name: payload, type: VARIANT}\n"
        ),
        "models/stg_payloads.sql": (
            'MODEL (description "Order payloads.");\n'
            'SELECT order_id, payload FROM __source("raw_orders")\n'
        ),
        "tests/scenarios/payloads__variant_compare.sql": (
            "SCENARIO (description 'Payload VARIANT values compare equal locally.');\n"
            "WITH\n"
            f"__source__raw_orders AS ({SNOWFLAKE_VARIANT_FIXTURE_SQL}),\n"
            "__expected__stg_payloads AS (SELECT 1 AS order_id, "
            f"PARSE_JSON('{expected_payload_json}') AS payload)\n"
            "SELECT 1\n"
        ),
    }


def write_snowflake_variant_snapshot(
    *, project_dir: Path, scenario_name: str, payload_text: str
) -> None:
    """Write the snapshot a Snowflake capture stores: the VARIANT as Snowflake's JSON text."""

    snapshot_root: Path = project_dir / "tests" / "_scenario_snapshots" / scenario_name
    (snapshot_root / "sources").mkdir(parents=True)
    row: str = json.dumps({"ORDER_ID": 1, "PAYLOAD": payload_text, "STATUS": "a"}) + "\n"
    (snapshot_root / "sources" / "raw_orders.jsonl").write_text(row, encoding="utf-8")
    manifest: dict[str, object] = {
        "capture_adapter": "snowflake",
        "capture_dialect": "snowflake",
        "captured_at": "2026-10-25T11:10:54Z",
        "format": "jsonl",
        "input_fingerprint": build_scenario_snapshot_input_fingerprint(
            scenario_name=scenario_name,
            input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw_orders",
                    file_path=Path("sources") / "raw_orders.jsonl",
                    capture_sql=SNOWFLAKE_VARIANT_FIXTURE_SQL,
                ),
            ),
            capture_adapter="snowflake",
            capture_dialect="snowflake",
        ),
        "relations": [
            {
                "bytes": len(row.encode("utf-8")),
                "columns": [
                    {"local_type": local, "name": name, "warehouse_type": warehouse}
                    for name, local, warehouse in (
                        ("order_id", "DECIMAL(1, 0)", "NUMBER(1,0)"),
                        ("status", "TEXT(1)", "VARCHAR(1)"),
                        ("payload", "VARIANT", "VARIANT"),
                    )
                ],
                "file": "sources/raw_orders.jsonl",
                "kind": "source",
                "logical_name": "raw_orders",
                "row_count": 1,
            }
        ],
        "scenario_name": scenario_name,
        "sqlbuild_version": "0.177.0",
        "total_bytes": len(row.encode("utf-8")),
        "total_rows": 1,
        "version": 1,
    }
    (snapshot_root / "scenario.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )


SNOWFLAKE_ORDER_IDS_FIXTURE_SQL: str = "SELECT 1 AS order_id"


def build_snowflake_lambda_scenario_project_files() -> dict[str, str]:
    """Build an offline Snowflake project whose model filters an array with an arrow lambda."""

    return {
        "sqlbuild_project.toml": build_offline_snowflake_project_toml(),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
            "    schema: main\n    table: raw_orders\n    columns:\n"
            '      - {name: order_id, type: "NUMBER(38,0)"}\n'
        ),
        "models/order_quantities.sql": (
            'MODEL (description "Quantities above each order id.");\n'
            "SELECT order_id,\n"
            "  FILTER(ARRAY_CONSTRUCT(1, 2, 3), x -> x > order_id) AS big_quantities\n"
            'FROM __source("raw_orders")\n'
        ),
        "tests/scenarios/order_quantities__lambda.sql": (
            "SCENARIO (description 'An arrow lambda replays locally.');\n"
            "WITH\n"
            f"__source__raw_orders AS ({SNOWFLAKE_ORDER_IDS_FIXTURE_SQL}),\n"
            "__expected__order_quantities AS (\n"
            "  SELECT 1 AS order_id, ARRAY_CONSTRUCT(2, 3) AS big_quantities\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def write_snowflake_order_ids_snapshot(*, project_dir: Path, scenario_name: str) -> None:
    """Write the snapshot a Snowflake capture stores for a one-column order source."""

    snapshot_root: Path = project_dir / "tests" / "_scenario_snapshots" / scenario_name
    (snapshot_root / "sources").mkdir(parents=True)
    row: str = json.dumps({"ORDER_ID": 1}) + "\n"
    (snapshot_root / "sources" / "raw_orders.jsonl").write_text(row, encoding="utf-8")
    manifest: dict[str, object] = {
        "capture_adapter": "snowflake",
        "capture_dialect": "snowflake",
        "captured_at": "2026-10-25T11:10:54Z",
        "format": "jsonl",
        "input_fingerprint": build_scenario_snapshot_input_fingerprint(
            scenario_name=scenario_name,
            input_specs=(
                ScenarioSnapshotInputSpec(
                    kind=ScenarioArtifactKind.SOURCE,
                    logical_name="raw_orders",
                    file_path=Path("sources") / "raw_orders.jsonl",
                    capture_sql=SNOWFLAKE_ORDER_IDS_FIXTURE_SQL,
                ),
            ),
            capture_adapter="snowflake",
            capture_dialect="snowflake",
        ),
        "relations": [
            {
                "bytes": len(row.encode("utf-8")),
                "columns": [
                    {
                        "local_type": "DECIMAL(1, 0)",
                        "name": "order_id",
                        "warehouse_type": "NUMBER(1,0)",
                    }
                ],
                "file": "sources/raw_orders.jsonl",
                "kind": "source",
                "logical_name": "raw_orders",
                "row_count": 1,
            }
        ],
        "scenario_name": scenario_name,
        "sqlbuild_version": "0.177.0",
        "total_bytes": len(row.encode("utf-8")),
        "total_rows": 1,
        "version": 1,
    }
    (snapshot_root / "scenario.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )
