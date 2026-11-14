from __future__ import annotations

import json
from collections import defaultdict
from collections.abc import Callable
from datetime import date, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, cast

import duckdb
import pytest
from _pytest.capture import CaptureFixture, CaptureResult
from _pytest.monkeypatch import MonkeyPatch
from duckdb import DuckDBPyConnection

from sqlbuild._native import (
    AuditCommandRequest,
    BuildCommandRequest,
    CliUserError,
    CursorOverrides,
    DiscoveredProjectInputs,
    DiscoveredSourceFile,
    LoadCommandRequest,
    PlanCommandRequest,
    PlannerInputError,
    ScenarioTestCommandRequest,
    SourceEntry,
    TestCommandRequest,
    discover_project_inputs,
    format_load_execution_json,
    run_audit_command,
    run_build_command,
    run_load_command,
    run_plan_command,
    run_scenario_test_command,
    run_test_command,
    select_load_entries,
)
from sqlbuild.adapter.contract.types import CursorKind
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.executor.load.main.run import run_load_pipeline
from sqlbuild.executor.load.models import (
    LoadCallbacks,
    LoadExecutionResult,
    LoadRuntimeParams,
)
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    BuildRunAutoLoadFailureTestCase,
    BuildRunAutoLoadFlagTestCase,
    BuildRunAutoLoadJsonTestCase,
    BuildRunAutoLoadSelectionTestCase,
    BuildRunAutoLoadTestCase,
    LoadCommandAdapterCallTestCase,
    LoadCommandBatchedRowsTestCase,
    LoadCommandBatchedYieldTestCase,
    LoadCommandConcurrencyTestCase,
    LoadCommandCursorNoneTestCase,
    LoadCommandCursorOverrideContextTestCase,
    LoadCommandEmptyRowsTestCase,
    LoadCommandEmptySelectionTestCase,
    LoadCommandFailureCleanupTestCase,
    LoadCommandFailureTestCase,
    LoadCommandInferredColumnsTestCase,
    LoadCommandIntegrationTestCase,
    LoadCommandLifecycleOrderTestCase,
    LoadCommandLifecycleSqlTestCase,
    LoadCommandMultipleYieldTestCase,
    LoadCommandReloadContextTestCase,
    LoadCommandSelectionErrorTestCase,
    LoadCommandWriteStrategyLifecycleTestCase,
    LoadCommandWriteStrategyTestCase,
    ManagedSourcePlanningErrorTestCase,
    PlanAutoLoadJsonTestCase,
    PlanAutoLoadOutputTestCase,
    SourceDeferralArtifactTestCase,
    SourceDeferralBuildTestCase,
    SourceDeferralNoErrorTestCase,
)

_PROJECT_FILE: str = 'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'

_RAW_ORDERS_LOADER: str = """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 1, "status": "loaded"}]
"""

_BUILD_RUN_AUTO_LOAD_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILE,
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 7, "status": "loaded"}]
""",
    "models/stg_orders.sql": (
        'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
    ),
}

_BUILD_RUN_AUTO_LOAD_CONFIG_FALSE_PROJECT_FILES: dict[str, str] = {
    **_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n\n'
        "[settings]\nauto_load_sources = false\n"
    ),
}

_SOURCE_DEFERRAL_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
defer_sources_to = "prod"

[targets.prod]
schema = "prod"
""".strip()
    + "\n",
    "sources/raw.yml": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["sources/raw.yml"],
    "python/loaders/raw_orders.py": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES[
        "python/loaders/raw_orders.py"
    ],
    "models/stg_orders.sql": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["models/stg_orders.sql"],
}

_SOURCE_DEFERRAL_MISSING_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"

[targets.prod]
schema = "prod"
""".strip()
    + "\n",
}

_SOURCE_LOADER_SCHEMA_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
loader_schema = "raw_dev"

[targets.prod]
schema = "prod"
loader_schema = "raw_prod"
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_LOCAL_OVERRIDE_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
defer_sources_to = "dev"

[targets.prod]
schema = "prod"
""".strip()
    + "\n",
    "sqlbuild_local.toml": """
[targets.dev]
defer_sources_to = "prod"
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_UNMANAGED_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
""".strip()
    + "\n",
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "models/stg_orders.sql": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["models/stg_orders.sql"],
}

_SOURCE_DEFERRAL_UNSELECTED_MANAGED_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
    "models/fact_orders.sql": (
        'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id, status FROM __ref("stg_orders")'
    ),
}

_SOURCE_DEFERRAL_AUTO_LOAD_FALSE_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[settings]
auto_load_sources = false

[targets.dev]
schema = "dev"
defer_sources_to = "prod"

[targets.prod]
schema = "prod"
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_LOADER_DAG_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _SOURCE_DEFERRAL_PROJECT_FILES["sqlbuild_project.toml"],
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader(
    write_strategy="table",
    columns=[
        {"name": "order_id", "type": "INTEGER"},
        {"name": "status", "type": "VARCHAR"},
    ],
)
def fetch_orders(ctx):
    '''Test loader fetch_orders.'''
    return [{"order_id": 7, "status": "intermediate"}]

@loader(depends_on=[fetch_orders])
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    rows = ctx.query(
        f"SELECT order_id, status FROM {ctx.loader(fetch_orders).destination}"
    ).fetchall()
    return [{"order_id": row[0], "status": "loaded-" + row[1]} for row in rows]
""",
    "models/stg_orders.sql": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["models/stg_orders.sql"],
}

_SOURCE_DEFERRAL_EXPLICIT_SCHEMA_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
loader_schema = "raw_dev"
""".strip()
    + "\n",
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    schema: external_raw
    table: raw_orders
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_orders.py": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES[
        "python/loaders/raw_orders.py"
    ],
    "models/stg_orders.sql": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["models/stg_orders.sql"],
}

_SOURCE_LOADER_COLLISION_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[targets.dev]
schema = "dev"
loader_schema = "shared_raw"

[targets.prod]
schema = "prod"
loader_schema = "shared_raw"
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_EXPRESSION_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _SOURCE_DEFERRAL_PROJECT_FILES["sqlbuild_project.toml"],
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    expression: |
      SELECT 22 AS order_id, 'expression' AS status
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_orders.py": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES[
        "python/loaders/raw_orders.py"
    ],
    "models/stg_orders.sql": _BUILD_RUN_AUTO_LOAD_PROJECT_FILES["models/stg_orders.sql"],
}

_SOURCE_DEFERRAL_SQL_FUNCTION_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "functions/sql/order_statuses.sql": """
FUNCTION (description "Test function order_statuses.",
  returns table (
    order_id INTEGER,
    status VARCHAR
  ),
);

SELECT order_id, status FROM __source("raw_orders")
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_TEMPLATED_ENV_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "demo.duckdb"

[vars]
source_prefix = "prod"

[targets.dev]
schema = "dev"
defer_sources_to = "prod"

[targets.prod]
schema = "${source_prefix}_raw"
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_AUDIT_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    audits: [source_status]
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "audits/generic/source_status.sql": """
AUDIT ();

SELECT order_id FROM @relation WHERE status != 'prod-audit'
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_BUILD_WITH_FUNCTION_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_PROJECT_FILES,
    "functions/sql/order_statuses.sql": """
FUNCTION (description "Test function order_statuses.",
  returns table (
    order_id INTEGER,
    status VARCHAR
  ),
);

SELECT order_id, status FROM __source("raw_orders")
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_TEST_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
    "tests/unit/test_stg_orders.sql": """
TEST ();

WITH
__source__raw_orders AS (
  SELECT 21 AS order_id, 'mocked' AS status
),
__expected__stg_orders AS (
  SELECT 21 AS order_id, 'mocked' AS status
)
SELECT 1
""".strip()
    + "\n",
}

_SOURCE_DEFERRAL_SCENARIO_PROJECT_FILES: dict[str, str] = {
    **_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
    "tests/scenarios/source_mock_pass.sql": """
SCENARIO (description "Test scenario source_mock_pass.");

WITH
__source__raw_orders AS (
  SELECT 31 AS order_id, 'scenario-mock' AS status
),
__expected__stg_orders AS (
  SELECT 31 AS order_id, 'scenario-mock' AS status
)
SELECT 1
""".strip()
    + "\n",
}

_BUILD_RUN_AUTO_LOAD_SELECTION_PROJECT_FILES: dict[str, str] = {
    **_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
    "models/fact_orders.sql": (
        'MODEL (description "Test model fact_orders.", materialized table);\n\nSELECT order_id, status FROM __ref("stg_orders")'
    ),
}

_BUILD_RUN_AUTO_LOAD_FAILURE_PROJECT_FILES: dict[str, str] = {
    **_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
    "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    raise RuntimeError("loader exploded")
""",
}

_BUILD_RUN_AUTO_LOAD_RELOAD_PROJECT_FILES: dict[str, str] = {
    **_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
    "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    status = "reload" if ctx.is_reload else "incremental"
    return [{"order_id": 7, "status": status}]
""",
}

_BUILD_RUN_AUTO_LOAD_TWO_SOURCE_PROJECT_FILES: dict[str, str] = {
    **_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
  - name: raw_customers
    description: Test source raw_customers.
    managed: true
    write_strategy: table
    columns:
      - name: customer_id
        type: INTEGER
      - name: name
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_customers.py": """
from sqlbuild.loaders import loader

@loader
def raw_customers(ctx):
    '''Test loader raw_customers.'''
    return [{"customer_id": 10, "name": "Ada"}]
""",
    "models/stg_customers.sql": (
        'MODEL (description "Test model stg_customers.", materialized table);\n\nSELECT customer_id, name FROM __source("raw_customers")'
    ),
}

_BUILD_RUN_AUTO_LOAD_SELF_MANAGED_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILE,
    "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    ctx.execute_sql(
        "CREATE OR REPLACE TABLE raw_orders AS "
        "SELECT 7 AS order_id, 'self-managed' AS status"
    )
""",
    "models/stg_orders.sql": (
        'MODEL (description "Test model stg_orders.", materialized table);\n\nSELECT order_id, status FROM __source("raw_orders")'
    ),
}

LOAD_COMMAND_CASE_FIXTURES: list[LoadCommandIntegrationTestCase] = [
    LoadCommandIntegrationTestCase(
        description="loads returned dict rows into a source table",
        project_files={
            "sqlbuild_project.toml": (
                'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
            ),
            "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    ctx.log("loading raw orders")
    return [
        {"order_id": 1, "status": "placed"},
        {"order_id": 2, "status": "shipped"},
    ]
""",
        },
        expected_exit_code=0,
        expected_rows=((1, "placed"), (2, "shipped")),
        expected_stdout_fragment="raw_orders",
        expected_stdout_fragments=(
            "Load ready  1 selected",
            "Sources (1)",
            "Execution  sqb load  (concurrency: 1)",
            "1/1  source    raw_orders",
            "rows=2",
            "\u2713 Completed successfully",
            "PASS=1  WARN=0  FAIL=0  SKIP=0  TOTAL=1",
        ),
        expected_json_staging_relation="raw_orders__staging",
        expected_json_rows_loaded=2,
        expected_lifecycle_sql_fragments=(
            "CREATE OR REPLACE TABLE raw_orders__staging",
            "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
            "DROP TABLE IF EXISTS raw_orders__staging",
        ),
    ),
    LoadCommandIntegrationTestCase(
        description="loads only selected managed source",
        project_files={
            "sqlbuild_project.toml": (
                'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
            ),
            "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
  - name: raw_events
    description: Test source raw_events.
    managed: true
    write_strategy: table
""".strip()
            + "\n",
            "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 3, "status": "selected"}]
""",
            "python/loaders/raw_events.py": """
from sqlbuild.loaders import loader

@loader
def raw_events(ctx):
    '''Test loader raw_events.'''
    return [{"event_id": 99}]
""",
        },
        expected_exit_code=0,
        expected_rows=((3, "selected"),),
        expected_stdout_fragment="raw_orders",
        expected_stdout_absent_fragments=("raw_events",),
        expected_json_staging_relation="raw_orders__staging",
        expected_json_rows_loaded=1,
        expected_lifecycle_sql_fragments=(
            "CREATE OR REPLACE TABLE raw_orders__staging",
            "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
            "DROP TABLE IF EXISTS raw_orders__staging",
        ),
        select=("raw_orders",),
    ),
    LoadCommandIntegrationTestCase(
        description="passes effective context values to loader",
        project_files={
            "sqlbuild_project.toml": (
                'name = "demo"\n'
                'adapter = "duckdb"\n'
                'default_target = "dev"\n\n'
                "[connection]\n"
                'database = "demo.duckdb"\n\n'
                "[vars]\n"
                'tier = "project"\n'
                'project_only = "yes"\n\n'
                "[targets.dev.vars]\n"
                'tier = "dev"\n'
            ),
            "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    status = ":".join([
        str(ctx.target),
        str(ctx.vars["tier"]),
        str(ctx.vars["project_only"]),
        str(ctx.run_id != "demo"),
    ])
    return [{
        "order_id": 4,
        "status": status,
    }]
""",
        },
        expected_exit_code=0,
        expected_rows=((4, "dev:cli:yes:True"),),
        expected_stdout_fragment="raw_orders",
        expected_json_staging_relation="raw_orders__staging",
        expected_json_rows_loaded=1,
        expected_lifecycle_sql_fragments=(
            "CREATE OR REPLACE TABLE raw_orders__staging",
            "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
            "DROP TABLE IF EXISTS raw_orders__staging",
        ),
        cli_vars={"tier": "cli"},
    ),
]


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadTestCase(
            description="build auto-loads selected managed source before models",
            command="build",
            expected_stdout_fragments=(
                "raw_orders",
                "Sources to load (1)",
                "raw_orders           table",
                "Execution  sqb build  (concurrency: 1)",
                "1/2  source",
                "2/2  table",
                "rows=1",
            ),
            expected_rows=((7, "loaded"),),
        ),
        BuildRunAutoLoadTestCase(
            description="build no tests auto-loads selected managed source before models",
            command="build",
            expected_stdout_fragments=(
                "raw_orders",
                "Sources to load (1)",
                "raw_orders           table",
                "Execution  sqb build  (concurrency: 1)",
                "1/2  source",
                "2/2  table",
                "rows=1",
            ),
            expected_rows=((7, "loaded"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_model_references_managed_source_when_build_or_run_then_auto_loads(
    test_case: BuildRunAutoLoadTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, _BUILD_RUN_AUTO_LOAD_PROJECT_FILES)

    exit_code: int = run_build_command(
        request=BuildCommandRequest(project_dir=tmp_path, no_color=True, select=("stg_orders",))
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM stg_orders ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadFlagTestCase(
            description="build auto-loads self-managed source before model",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_SELF_MANAGED_PROJECT_FILES,
            args=("--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "self-managed"),),
            expected_stdout_fragments=(
                "Plan ready  1 selected, 1 source to load",
                "raw_orders           self-managed",
                "1/2  source",
                "2/2  table",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_self_managed_source_when_running_build_then_auto_loads_before_model(
    test_case: BuildRunAutoLoadFlagTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_build_command(
        request=BuildCommandRequest(project_dir=tmp_path, no_color=True, select=("stg_orders",))
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM stg_orders ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SourceDeferralBuildTestCase(
            description="build configured target defers managed source reads to prod",
            command="build",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            defer_sources_to=None,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (99, 'prod-existing')",
            ),
            expected_model_rows=((99, "prod-existing"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
        SourceDeferralBuildTestCase(
            description="build cli override reads managed sources from active target",
            command="build",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            defer_sources_to="dev",
            setup_sql=(),
            expected_model_rows=((7, "loaded"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
        SourceDeferralBuildTestCase(
            description="build cli override reads managed sources from active target",
            command="build",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            defer_sources_to="dev",
            setup_sql=(),
            expected_model_rows=((7, "loaded"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
        SourceDeferralBuildTestCase(
            description="build configured target defers managed source reads to prod",
            command="build",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            defer_sources_to=None,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (100, 'prod-run')",
            ),
            expected_model_rows=((100, "prod-run"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
        SourceDeferralBuildTestCase(
            description="local target override defers managed source reads to prod",
            command="build",
            project_files=_SOURCE_DEFERRAL_LOCAL_OVERRIDE_PROJECT_FILES,
            defer_sources_to=None,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (101, 'local-override-prod')",
            ),
            expected_model_rows=((101, "local-override-prod"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
        SourceDeferralBuildTestCase(
            description="loader dag writes active env while model reads deferred source env",
            command="build",
            project_files=_SOURCE_DEFERRAL_LOADER_DAG_PROJECT_FILES,
            defer_sources_to=None,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (102, 'prod-dag')",
            ),
            expected_model_rows=((102, "prod-dag"),),
            expected_loaded_source_rows=((7, "loaded-intermediate"),),
        ),
        SourceDeferralBuildTestCase(
            description="expression managed source remains unchanged by source deferral",
            command="build",
            project_files=_SOURCE_DEFERRAL_EXPRESSION_PROJECT_FILES,
            defer_sources_to=None,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (103, 'prod-expression')",
            ),
            expected_model_rows=((22, "expression"),),
            expected_loaded_source_rows=((7, "loaded"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_managed_source_environment_when_building_or_running_then_reads_configured_source_env(
    test_case: SourceDeferralBuildTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    exit_code: int = run_build_command(
        request=BuildCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=("stg_orders",),
            defer_sources_to=test_case.defer_sources_to,
        )
    )

    assert exit_code == test_case.expected_exit_code
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        model_rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM dev.stg_orders ORDER BY order_id"
            ).fetchall()
        )
        source_rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM dev.raw_orders ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert model_rows == test_case.expected_model_rows
    assert source_rows == test_case.expected_loaded_source_rows


@pytest.mark.parametrize(
    "test_case",
    [
        SourceDeferralNoErrorTestCase(
            description="plan defaults managed source reads to active target",
            project_files=_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
            setup_sql=(),
            select=("stg_orders",),
            result_sql="",
            expected_rows=(),
        ),
        SourceDeferralNoErrorTestCase(
            description="plan configured target accepts deferred source reads",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            setup_sql=(),
            select=("stg_orders",),
            result_sql="",
            expected_rows=(),
        ),
        SourceDeferralNoErrorTestCase(
            description="plan cli override accepts active source target",
            project_files=_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
            setup_sql=(),
            select=("stg_orders",),
            result_sql="",
            expected_rows=(),
            defer_sources_to="dev",
        ),
        SourceDeferralNoErrorTestCase(
            description="selected sql function defaults managed source read to active target",
            project_files={
                **_SOURCE_DEFERRAL_SQL_FUNCTION_PROJECT_FILES,
                "sqlbuild_project.toml": _SOURCE_DEFERRAL_MISSING_PROJECT_FILES[
                    "sqlbuild_project.toml"
                ],
            },
            setup_sql=(),
            select=("order_statuses",),
            result_sql="",
            expected_rows=(),
        ),
        SourceDeferralNoErrorTestCase(
            description="selected sql function managed source read uses configured deferral",
            project_files=_SOURCE_DEFERRAL_SQL_FUNCTION_PROJECT_FILES,
            setup_sql=(),
            select=("order_statuses",),
            result_sql="",
            expected_rows=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_source_deferral_override_when_planning_then_succeeds(
    test_case: SourceDeferralNoErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_plan_command(
        request=PlanCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=test_case.select,
            defer_sources_to=test_case.defer_sources_to,
        )
    )

    assert exit_code == test_case.expected_exit_code


@pytest.mark.parametrize(
    "test_case",
    [
        SourceDeferralNoErrorTestCase(
            description="loader schema separates managed writes from model schema",
            project_files=_SOURCE_LOADER_SCHEMA_PROJECT_FILES,
            setup_sql=(),
            select=("stg_orders",),
            result_sql=(
                "SELECT 'model', order_id, status FROM dev.stg_orders "
                "UNION ALL "
                "SELECT 'source', order_id, status FROM raw_dev.raw_orders "
                "ORDER BY 1"
            ),
            expected_rows=(("model", 7, "loaded"), ("source", 7, "loaded")),
            command_options={"select": ("stg_orders",)},
        ),
        SourceDeferralNoErrorTestCase(
            description="unmanaged source read does not require source deferral config",
            project_files=_SOURCE_DEFERRAL_UNMANAGED_PROJECT_FILES,
            setup_sql=(
                "CREATE TABLE raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO raw_orders VALUES (12, 'unmanaged')",
            ),
            select=("stg_orders",),
            result_sql="SELECT order_id, status FROM dev.stg_orders ORDER BY order_id",
            expected_rows=((12, "unmanaged"),),
            command_options={"select": ("stg_orders",)},
        ),
        SourceDeferralNoErrorTestCase(
            description="unselected managed source read does not require source deferral config",
            project_files=_SOURCE_DEFERRAL_UNSELECTED_MANAGED_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA dev",
                "CREATE TABLE dev.stg_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO dev.stg_orders VALUES (13, 'prebuilt')",
            ),
            select=("fact_orders",),
            result_sql="SELECT order_id, status FROM dev.fact_orders ORDER BY order_id",
            expected_rows=((13, "prebuilt"),),
            command_options={"select": ("fact_orders",)},
        ),
        SourceDeferralNoErrorTestCase(
            description="no-load still reads managed source from deferred target",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (14, 'prod-no-load')",
            ),
            select=("stg_orders",),
            result_sql="SELECT order_id, status FROM dev.stg_orders ORDER BY order_id",
            expected_rows=((14, "prod-no-load"),),
            load_sources=False,
            command_options={"select": ("stg_orders",), "load_sources": False},
        ),
        SourceDeferralNoErrorTestCase(
            description="auto load disabled still reads managed source from deferred target",
            project_files=_SOURCE_DEFERRAL_AUTO_LOAD_FALSE_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (15, 'prod-auto-disabled')",
            ),
            select=("stg_orders",),
            result_sql="SELECT order_id, status FROM dev.stg_orders ORDER BY order_id",
            expected_rows=((15, "prod-auto-disabled"),),
            command_options={"select": ("stg_orders",)},
        ),
        SourceDeferralNoErrorTestCase(
            description="explicit managed source schema overrides target loader schema",
            project_files=_SOURCE_DEFERRAL_EXPLICIT_SCHEMA_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA external_raw",
                "CREATE TABLE external_raw.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO external_raw.raw_orders VALUES (16, 'external-explicit')",
            ),
            select=("stg_orders",),
            result_sql="SELECT order_id, status FROM dev.stg_orders ORDER BY order_id",
            expected_rows=((16, "external-explicit"),),
            load_sources=False,
            command_options={"select": ("stg_orders",), "load_sources": False},
        ),
        SourceDeferralNoErrorTestCase(
            description="templated source deferral target schema resolves project vars",
            project_files=_SOURCE_DEFERRAL_TEMPLATED_ENV_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod_raw",
                "CREATE TABLE prod_raw.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod_raw.raw_orders VALUES (17, 'templated-prod')",
            ),
            select=("stg_orders",),
            result_sql="SELECT order_id, status FROM dev.stg_orders ORDER BY order_id",
            expected_rows=((17, "templated-prod"),),
            load_sources=False,
            command_options={"select": ("stg_orders",), "load_sources": False},
        ),
        SourceDeferralNoErrorTestCase(
            description="audit reads managed source from deferred target",
            project_files=_SOURCE_DEFERRAL_AUDIT_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA dev",
                "CREATE TABLE dev.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO dev.raw_orders VALUES (18, 'dev-would-fail')",
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (18, 'prod-audit')",
            ),
            select=(),
            result_sql="SELECT 1",
            expected_rows=((1,),),
            command="audit",
            command_options={"select": ()},
        ),
        SourceDeferralNoErrorTestCase(
            description="sql test with source mock does not require source deferral",
            project_files=_SOURCE_DEFERRAL_TEST_PROJECT_FILES,
            setup_sql=(),
            select=("stg_orders",),
            result_sql="SELECT 1",
            expected_rows=((1,),),
            command="test",
            command_options={"select": ("stg_orders",)},
        ),
        SourceDeferralNoErrorTestCase(
            description="scenario with source mock does not require source deferral",
            project_files=_SOURCE_DEFERRAL_SCENARIO_PROJECT_FILES,
            setup_sql=(),
            select=("source_mock_pass",),
            result_sql="SELECT 1",
            expected_rows=((1,),),
            command="scenario",
            command_options={"selectors": ("source_mock_pass",)},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_managed_source_read_ambiguity_when_building_then_source_deferral_not_required(
    test_case: SourceDeferralNoErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    command_runners: dict[str, Callable[..., int]] = {
        "audit": lambda **kwargs: run_audit_command(request=AuditCommandRequest(**kwargs)),
        "build": lambda **kwargs: run_build_command(request=BuildCommandRequest(**kwargs)),
        "scenario": lambda **kwargs: run_scenario_test_command(
            request=ScenarioTestCommandRequest(**kwargs)
        ),
        "test": lambda **kwargs: run_test_command(request=TestCommandRequest(**kwargs)),
    }
    exit_code: int = command_runners[test_case.command](
        project_dir=tmp_path,
        no_color=True,
        **test_case.command_options,
    )

    assert exit_code == test_case.expected_exit_code
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(test_case.result_sql).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        ManagedSourcePlanningErrorTestCase(
            description="managed source read with unknown cli deferral target errors",
            project_files=_SOURCE_DEFERRAL_MISSING_PROJECT_FILES,
            defer_sources_to="missing_env",
            expected_error_fragment="Unknown source deferral target 'missing_env'",
        ),
        ManagedSourcePlanningErrorTestCase(
            description="managed loaders cannot share a write namespace across targets",
            project_files=_SOURCE_LOADER_COLLISION_PROJECT_FILES,
            expected_error_fragment=(
                "Managed loader write collision: targets 'dev' and 'prod' both resolve to"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_managed_source_target_config_when_planning_then_errors(
    test_case: ManagedSourcePlanningErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    with pytest.raises(PlannerInputError) as exc_info:
        run_plan_command(
            request=PlanCommandRequest(
                project_dir=tmp_path,
                no_color=True,
                select=test_case.select,
                defer_sources_to=test_case.defer_sources_to,
            )
        )

    assert test_case.expected_error_fragment in str(exc_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        SourceDeferralArtifactTestCase(
            description=(
                "build artifacts write deferred source relation into compiled and runtime model sql"
            ),
            command="build",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (19, 'artifact-prod')",
            ),
            select=("stg_orders",),
            compiled_relative_paths=("target/compiled/models/stg_orders.sql",),
            runtime_relative_paths=("target/run/models/stg_orders.sql",),
            expected_sql_fragments=("prod.raw_orders",),
            unexpected_sql_fragments=("dev.raw_orders",),
        ),
        SourceDeferralArtifactTestCase(
            description="build artifacts write deferred source relation into compiled function sql",
            command="build",
            project_files=_SOURCE_DEFERRAL_BUILD_WITH_FUNCTION_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (20, 'function-artifact')",
            ),
            select=("order_statuses",),
            compiled_relative_paths=("target/compiled/functions/sql/order_statuses.sql",),
            runtime_relative_paths=("target/run/functions/sql/order_statuses.sql",),
            expected_sql_fragments=("prod.raw_orders",),
            unexpected_sql_fragments=("dev.raw_orders",),
        ),
        SourceDeferralArtifactTestCase(
            description="audit compiled artifact writes deferred source relation",
            command="build",
            project_files=_SOURCE_DEFERRAL_AUDIT_PROJECT_FILES,
            setup_sql=(
                "CREATE SCHEMA prod",
                "CREATE TABLE prod.raw_orders(order_id INTEGER, status VARCHAR)",
                "INSERT INTO prod.raw_orders VALUES (21, 'prod-audit')",
            ),
            select=("stg_orders",),
            compiled_relative_paths=(
                "target/compiled/audits/generic/raw_orders/source_status.sql",
            ),
            runtime_relative_paths=(),
            expected_sql_fragments=("prod.raw_orders",),
            unexpected_sql_fragments=("dev.raw_orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_deferral_when_writing_artifacts_then_sql_uses_deferred_relations(
    test_case: SourceDeferralArtifactTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    command_runners: dict[str, Callable[..., int]] = {
        "audit": lambda **kwargs: run_audit_command(request=AuditCommandRequest(**kwargs)),
        "build": lambda **kwargs: run_build_command(request=BuildCommandRequest(**kwargs)),
    }
    exit_code: int = command_runners[test_case.command](
        project_dir=tmp_path,
        no_color=True,
        select=test_case.select,
    )

    assert exit_code == 0
    artifact_relative_path: str
    artifact_contents: str = ""
    for artifact_relative_path in (
        *test_case.compiled_relative_paths,
        *test_case.runtime_relative_paths,
    ):
        artifact_contents += (tmp_path / artifact_relative_path).read_text(encoding="utf-8")
        artifact_contents += "\n"
    expected_fragment: str
    for expected_fragment in test_case.expected_sql_fragments:
        assert expected_fragment in artifact_contents
    unexpected_fragment: str
    for unexpected_fragment in test_case.unexpected_sql_fragments:
        assert unexpected_fragment not in artifact_contents


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadFlagTestCase(
            description="build no-load skips loader and uses existing source table",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            args=("--no-load", "--select", "stg_orders"),
            setup_sql=(
                "CREATE TABLE raw_orders (order_id INTEGER, status VARCHAR)",
                "INSERT INTO raw_orders VALUES (7, 'existing')",
            ),
            expected_rows=((7, "existing"),),
            expected_stdout_absent_fragments=("source", "rows=1"),
            load_sources=False,
        ),
        BuildRunAutoLoadFlagTestCase(
            description="build load forces loader when auto load is disabled",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_CONFIG_FALSE_PROJECT_FILES,
            args=("--load", "--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "loaded"),),
            expected_stdout_fragments=("1/2  source", "rows=1"),
            load_sources=True,
        ),
        BuildRunAutoLoadFlagTestCase(
            description="build auto load disabled skips loader by default",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_CONFIG_FALSE_PROJECT_FILES,
            args=("--select", "stg_orders"),
            setup_sql=(
                "CREATE TABLE raw_orders (order_id INTEGER, status VARCHAR)",
                "INSERT INTO raw_orders VALUES (7, 'existing')",
            ),
            expected_rows=((7, "existing"),),
            expected_stdout_absent_fragments=("source", "rows=1"),
            load_sources=None,
        ),
        BuildRunAutoLoadFlagTestCase(
            description="build reload and full refresh reloads loaders and models",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_RELOAD_PROJECT_FILES,
            args=("--reload", "--full-refresh", "--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "reload"),),
            expected_stdout_fragments=("Sources to reload (1)", "1/2  source", "2/2  table"),
            load_sources=None,
        ),
        BuildRunAutoLoadFlagTestCase(
            description="build reload reloads loaders without full refreshing models",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_RELOAD_PROJECT_FILES,
            args=("--reload", "--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "reload"),),
            expected_stdout_fragments=(
                "Plan ready  1 selected, 1 source to reload",
                "1/2  source",
                "2/2  table",
            ),
            expected_stdout_absent_fragments=("Full refresh",),
            load_sources=None,
        ),
        BuildRunAutoLoadFlagTestCase(
            description="build reload reloads loaders without full refreshing models",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_RELOAD_PROJECT_FILES,
            args=("--reload", "--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "reload"),),
            expected_stdout_fragments=(
                "Plan ready  1 selected, 1 source to reload",
                "1/2  source",
                "2/2  table",
            ),
            expected_stdout_absent_fragments=("Full refresh",),
            load_sources=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_build_or_run_load_flags_when_running_then_applies_loader_control(
    test_case: BuildRunAutoLoadFlagTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    exit_code: int = run_build_command(
        request=BuildCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=(test_case.args[test_case.args.index("--select") + 1],),
            load_sources=test_case.load_sources,
            reload_sources="--reload" in test_case.args,
            full_refresh="--full-refresh" in test_case.args,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    assert all(
        fragment not in captured.out for fragment in test_case.expected_stdout_absent_fragments
    )
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM stg_orders ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadSelectionTestCase(
            description="selected downstream model does not load source referenced only upstream",
            args=("--select", "fact_orders"),
            setup_sql=(
                "CREATE TABLE stg_orders (order_id INTEGER, status VARCHAR)",
                "INSERT INTO stg_orders VALUES (7, 'existing')",
            ),
            expected_rows=((7, "existing"),),
            expected_stdout_absent_fragments=("source", "rows=1"),
        ),
        BuildRunAutoLoadSelectionTestCase(
            description="upstream-expanded downstream model loads source referenced upstream",
            args=("--select", "+fact_orders"),
            setup_sql=(),
            expected_rows=((7, "loaded"),),
            expected_stdout_fragments=(
                "Sources to load (1)",
                "1/3  source",
                "2/3  table",
                "3/3  table",
            ),
        ),
        BuildRunAutoLoadSelectionTestCase(
            description="selected model loads only its direct source among multiple managed sources",
            args=("--select", "stg_orders"),
            setup_sql=(),
            expected_rows=((7, "loaded"),),
            expected_stdout_fragments=("Sources to load (1)", "raw_orders", "1/2  source"),
            expected_stdout_absent_fragments=("raw_customers", "2/3  source"),
            project_files=_BUILD_RUN_AUTO_LOAD_TWO_SOURCE_PROJECT_FILES,
            result_table="stg_orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_downstream_selection_when_running_build_then_loads_only_direct_selected_sources(
    test_case: BuildRunAutoLoadSelectionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(
        tmp_path, test_case.project_files or _BUILD_RUN_AUTO_LOAD_SELECTION_PROJECT_FILES
    )
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    exit_code: int = run_build_command(
        request=BuildCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=(test_case.args[test_case.args.index("--select") + 1],),
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    assert all(
        fragment not in captured.out for fragment in test_case.expected_stdout_absent_fragments
    )
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                f"SELECT order_id, status FROM {test_case.result_table} ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadFailureTestCase(
            description="loader failure blocks downstream model execution",
            project_files=_BUILD_RUN_AUTO_LOAD_FAILURE_PROJECT_FILES,
            expected_exit_code=1,
            expected_stdout_fragments=("loader exploded", "1/2  source", "SKIP"),
            expected_model_exists=False,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_fails_when_running_build_then_downstream_model_is_not_executed(
    test_case: BuildRunAutoLoadFailureTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_build_command(
        request=BuildCommandRequest(project_dir=tmp_path, no_color=True, select=("stg_orders",))
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        model_exists: bool = (
            connection.execute(
                "SELECT 1 FROM information_schema.tables WHERE table_name = 'stg_orders'"
            ).fetchone()
            is not None
        )
    finally:
        connection.close()
    assert model_exists is test_case.expected_model_exists


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadJsonTestCase(
            description="build json output includes source asset",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            expected_exit_code=0,
            expected_source_asset={
                "kind": "source",
                "name": "raw_orders",
                "status": "success",
                "target": "raw_orders",
                "staging_relation": "raw_orders__staging",
                "loader": "raw_orders",
                "rows_loaded": 1,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_build_auto_loads_source_when_json_output_then_includes_source_asset(
    test_case: BuildRunAutoLoadJsonTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    json_output_path: Path = tmp_path / "target" / "build.json"

    exit_code: int = run_build_command(
        request=BuildCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=("stg_orders",),
            json_output_path=json_output_path,
            manifest=True,
        )
    )

    payload: dict[str, Any] = json.loads(json_output_path.read_text(encoding="utf-8"))
    assets_by_kind: defaultdict[str, list[dict[str, object]]] = defaultdict(list)
    for asset in payload["assets"]:
        assets_by_kind[asset["kind"]].append(asset)
    source_assets: list[dict[str, object]] = assets_by_kind["source"]
    assert exit_code == test_case.expected_exit_code
    assert len(source_assets) == 1
    expected_key: str
    for expected_key in test_case.expected_source_asset:
        assert source_assets[0][expected_key] == test_case.expected_source_asset[expected_key]
    manifest: dict[str, Any] = json.loads(
        (tmp_path / "target" / "manifest.json").read_text(encoding="utf-8")
    )
    source_node: dict[str, Any] = manifest["sources"]["source.demo.raw_orders"]
    assert source_node["meta"]["sqlbuild"] == {
        "loader": "raw_orders",
        "auto_load": True,
        "write_strategy": "table",
    }


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAutoLoadOutputTestCase(
            description="plan shows source loaders by default",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            load_sources=None,
            expected_stdout_fragments=("Sources to load (1)", "raw_orders", "table"),
        ),
        PlanAutoLoadOutputTestCase(
            description="plan no-load hides source loaders",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            load_sources=False,
            expected_stdout_fragments=("Plan ready  1 selected", "stg_orders"),
            expected_stdout_absent_fragments=("Sources to load",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_load_controls_when_running_plan_then_formats_source_load_section(
    test_case: PlanAutoLoadOutputTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_plan_command(
        request=PlanCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=("stg_orders",),
            load_sources=test_case.load_sources,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == 0
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    assert all(
        fragment not in captured.out for fragment in test_case.expected_stdout_absent_fragments
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PlanAutoLoadJsonTestCase(
            description="plan json includes source loads",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            load_sources=None,
            expected_source_loads=(
                {
                    "name": "raw_orders",
                    "loader": "raw_orders",
                    "kind": "source",
                    "target": "raw_orders",
                    "is_reload": False,
                    "write_strategy": "table",
                },
            ),
            expected_selected_count=1,
            expected_source_load_count=1,
        ),
        PlanAutoLoadJsonTestCase(
            description="plan json no-load has zero source load count",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            load_sources=False,
            expected_source_loads=(),
            expected_selected_count=1,
            expected_source_load_count=0,
        ),
        PlanAutoLoadJsonTestCase(
            description="plan json keeps source loads when source reads defer to prod",
            project_files=_SOURCE_DEFERRAL_PROJECT_FILES,
            load_sources=None,
            expected_source_loads=(
                {
                    "name": "raw_orders",
                    "loader": "raw_orders",
                    "kind": "source",
                    "target": "raw_orders",
                    "is_reload": False,
                    "write_strategy": "table",
                },
            ),
            expected_selected_count=1,
            expected_source_load_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_json_output_when_source_auto_loads_then_includes_source_loads(
    test_case: PlanAutoLoadJsonTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_plan_command(
        request=PlanCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            json_output=True,
            select=("stg_orders",),
            load_sources=test_case.load_sources,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, Any] = json.loads(captured.out)
    assert exit_code == 0
    assert payload["selected_count"] == test_case.expected_selected_count
    assert payload["source_load_count"] == test_case.expected_source_load_count
    assert payload["source_loads"] == list(test_case.expected_source_loads)


@pytest.mark.parametrize(
    "test_case",
    [
        BuildRunAutoLoadFlagTestCase(
            description="manifest marks source auto load false when loaders are skipped",
            command="build",
            project_files=_BUILD_RUN_AUTO_LOAD_PROJECT_FILES,
            args=("--no-load", "--select", "stg_orders"),
            setup_sql=(
                "CREATE TABLE raw_orders (order_id INTEGER, status VARCHAR)",
                "INSERT INTO raw_orders VALUES (7, 'existing')",
            ),
            expected_rows=((7, "existing"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_build_skips_loader_when_manifest_is_written_then_marks_source_auto_load_false(
    test_case: BuildRunAutoLoadFlagTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    setup_connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            setup_connection.execute(setup_statement)
    finally:
        setup_connection.close()

    exit_code: int = run_build_command(
        request=BuildCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=("stg_orders",),
            load_sources=False,
            manifest=True,
        )
    )

    manifest: dict[str, Any] = json.loads(
        (tmp_path / "target" / "manifest.json").read_text(encoding="utf-8")
    )
    source_node: dict[str, Any] = manifest["sources"]["source.demo.raw_orders"]
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM stg_orders ORDER BY order_id"
            ).fetchall()
        )
    finally:
        connection.close()
    assert exit_code == 0
    assert rows == test_case.expected_rows
    assert source_node["meta"]["sqlbuild"] == {
        "loader": "raw_orders",
        "auto_load": False,
        "write_strategy": "table",
    }


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandIntegrationTestCase(
            description="loads returned dict rows into a source table",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    ctx.log("loading raw orders")
    return [
        {"order_id": 1, "status": "placed"},
        {"order_id": 2, "status": "shipped"},
    ]
""",
            },
            expected_exit_code=0,
            expected_rows=((1, "placed"), (2, "shipped")),
            expected_stdout_fragment="raw_orders",
            expected_stdout_fragments=(
                "Load ready  1 selected",
                "Sources (1)",
                "Execution  sqb load  (concurrency: 1)",
                "1/1  source    raw_orders",
                "rows=2",
                "\u2713 Completed successfully",
                "PASS=1  WARN=0  FAIL=0  SKIP=0  TOTAL=1",
            ),
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=2,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
        ),
        LoadCommandIntegrationTestCase(
            description="loads only selected managed source",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
  - name: raw_events
    description: Test source raw_events.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 3, "status": "selected"}]
""",
                "python/loaders/raw_events.py": """
from sqlbuild.loaders import loader

@loader
def raw_events(ctx):
    '''Test loader raw_events.'''
    return [{"event_id": 99}]
""",
            },
            expected_exit_code=0,
            expected_rows=((3, "selected"),),
            expected_stdout_fragment="raw_orders",
            expected_stdout_absent_fragments=("raw_events",),
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=1,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
            select=("raw_orders",),
        ),
        LoadCommandIntegrationTestCase(
            description="passes effective context values to loader",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connection]\n"
                    'database = "demo.duckdb"\n\n'
                    "[vars]\n"
                    'tier = "project"\n'
                    'project_only = "yes"\n\n'
                    "[targets.dev.vars]\n"
                    'tier = "dev"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    status = ":".join([
        str(ctx.target),
        str(ctx.vars["tier"]),
        str(ctx.vars["project_only"]),
        str(ctx.run_id != "demo"),
    ])
    return [{
        "order_id": 4,
        "status": status,
    }]
""",
            },
            expected_exit_code=0,
            expected_rows=((4, "dev:cli:yes:True"),),
            expected_stdout_fragment="raw_orders",
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=1,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
            cli_vars={"tier": "cli"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_when_running_load_then_writes_source_table(
    test_case: LoadCommandIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    json_output_path: Path = tmp_path / "target" / "load.json"

    exit_code: int = run_load_command(
        request=LoadCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=test_case.select,
            cli_vars=test_case.cli_vars,
            json_output_path=json_output_path,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_stdout_fragment in captured.out
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    assert all(
        fragment not in captured.out for fragment in test_case.expected_stdout_absent_fragments
    )
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT order_id, status FROM raw_orders ORDER BY order_id"
            ).fetchall()
        )
        staging_exists: bool = (
            connection.execute(
                "SELECT 1 FROM information_schema.tables WHERE table_name = 'raw_orders__staging'"
            ).fetchone()
            is not None
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows
    assert not staging_exists
    payload: dict[str, Any] = cast(
        dict[str, Any], json.loads(json_output_path.read_text(encoding="utf-8"))
    )
    assets: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["assets"])
    assert assets[0]["staging_relation"] == test_case.expected_json_staging_relation
    assert assets[0]["rows_loaded"] == test_case.expected_json_rows_loaded


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandIntegrationTestCase(
            description="loads returned dict rows into a source table",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    ctx.log("loading raw orders")
    return [
        {"order_id": 1, "status": "placed"},
        {"order_id": 2, "status": "shipped"},
    ]
""",
            },
            expected_exit_code=0,
            expected_rows=((1, "placed"), (2, "shipped")),
            expected_stdout_fragment="raw_orders",
            expected_stdout_fragments=(
                "Load ready  1 selected",
                "Sources (1)",
                "Execution  sqb load  (concurrency: 1)",
                "1/1  source    raw_orders",
                "rows=2",
                "\u2713 Completed successfully",
                "PASS=1  WARN=0  FAIL=0  SKIP=0  TOTAL=1",
            ),
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=2,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
        ),
        LoadCommandIntegrationTestCase(
            description="loads only selected managed source",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "demo.duckdb"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
  - name: raw_events
    description: Test source raw_events.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    return [{"order_id": 3, "status": "selected"}]
""",
                "python/loaders/raw_events.py": """
from sqlbuild.loaders import loader

@loader
def raw_events(ctx):
    '''Test loader raw_events.'''
    return [{"event_id": 99}]
""",
            },
            expected_exit_code=0,
            expected_rows=((3, "selected"),),
            expected_stdout_fragment="raw_orders",
            expected_stdout_absent_fragments=("raw_events",),
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=1,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
            select=("raw_orders",),
        ),
        LoadCommandIntegrationTestCase(
            description="passes effective context values to loader",
            project_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\n'
                    'adapter = "duckdb"\n'
                    'default_target = "dev"\n\n'
                    "[connection]\n"
                    'database = "demo.duckdb"\n\n'
                    "[vars]\n"
                    'tier = "project"\n'
                    'project_only = "yes"\n\n'
                    "[targets.dev.vars]\n"
                    'tier = "dev"\n'
                ),
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
    columns:
      - name: order_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": """
from sqlbuild.loaders import loader

@loader
def raw_orders(ctx):
    '''Test loader raw_orders.'''
    status = ":".join([
        str(ctx.target),
        str(ctx.vars["tier"]),
        str(ctx.vars["project_only"]),
        str(ctx.run_id != "demo"),
    ])
    return [{
        "order_id": 4,
        "status": status,
    }]
""",
            },
            expected_exit_code=0,
            expected_rows=((4, "dev:cli:yes:True"),),
            expected_stdout_fragment="raw_orders",
            expected_json_staging_relation="raw_orders__staging",
            expected_json_rows_loaded=1,
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
            cli_vars={"tier": "cli"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_when_running_pipeline_then_uses_staging_relation(
    test_case: LoadCommandIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={"tier": "cli", "project_only": "yes"},
            is_reload=False,
        ),
    )

    lifecycle_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in results[0].lifecycle_events:
        lifecycle_content_by_kind[event.kind.value].append(event.content)
    lifecycle_sql: tuple[str, ...] = tuple(lifecycle_content_by_kind["sql"])
    fragments_found: list[bool] = []
    for fragment in test_case.expected_lifecycle_sql_fragments:
        fragment_found: bool = False
        for sql in lifecycle_sql:
            fragment_found = fragment_found or fragment in sql
        fragments_found.append(fragment_found)
    assert all(fragments_found)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandLifecycleOrderTestCase(
            description="drops stale staging before creating new staging table",
            project_files=LOAD_COMMAND_CASE_FIXTURES[0].project_files,
            expected_lifecycle_sql_order=(
                "DROP TABLE IF EXISTS raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders__staging",
                "CREATE OR REPLACE TABLE raw_orders AS SELECT * FROM raw_orders__staging",
                "DROP TABLE IF EXISTS raw_orders__staging",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_when_running_pipeline_then_drops_stale_staging_first(
    test_case: LoadCommandLifecycleOrderTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    lifecycle_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in results[0].lifecycle_events:
        lifecycle_content_by_kind[event.kind.value].append(event.content)
    lifecycle_sql: tuple[str, ...] = tuple(lifecycle_content_by_kind["sql"])
    match_positions: list[int] = []
    start_index: int = 0
    expected_fragment: str
    for expected_fragment in test_case.expected_lifecycle_sql_order:
        indexes_by_match_status: defaultdict[bool, list[int]] = defaultdict(list)
        for index, sql in enumerate(lifecycle_sql[start_index:], start=start_index):
            indexes_by_match_status[expected_fragment in sql].append(index)
        position: int = next(iter(indexes_by_match_status[True]))
        match_positions.append(position)
        start_index = position + 1
    assert match_positions == sorted(match_positions)


WRITE_STRATEGY_CASE_FIXTURES: list[LoadCommandWriteStrategyTestCase] = [
    LoadCommandWriteStrategyTestCase(
        description="append creates target then appends rows using current cursor value",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_append_events
    description: Test source raw_append_events.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_append_events(ctx):
    '''Test loader raw_append_events.'''
    if ctx.current_cursor_value is None:
        return [{"event_id": 1, "cursor_seen": "none"}]
    next_id = int(ctx.current_cursor_value) + 1
    return [{"event_id": next_id, "cursor_seen": str(ctx.current_cursor_value)}]
""",
        },
        select_sql="SELECT event_id, cursor_seen FROM raw_append_events ORDER BY event_id",
        expected_rows=((1, "none"), (2, "1")),
    ),
    LoadCommandWriteStrategyTestCase(
        description="merge updates existing rows and inserts new rows using cursor value",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_merge_customers
    description: Test source raw_merge_customers.
    managed: true
    write_strategy: merge
    unique_key: customer_id
    cursor_column: updated_at
    columns:
      - name: customer_id
        type: INTEGER
      - name: name
        type: VARCHAR
      - name: updated_at
        type: INTEGER
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_merge_customers(ctx):
    '''Test loader raw_merge_customers.'''
    if ctx.current_cursor_value is None:
        return [
            {"customer_id": 1, "name": "old", "updated_at": 1},
            {"customer_id": 2, "name": "kept", "updated_at": 1},
        ]
    return [
        {"customer_id": 1, "name": f"updated:{ctx.current_cursor_value}", "updated_at": 2},
        {"customer_id": 3, "name": "new", "updated_at": 2},
    ]
""",
        },
        select_sql=(
            "SELECT customer_id, name, updated_at FROM raw_merge_customers ORDER BY customer_id"
        ),
        expected_rows=((1, "updated:1", 2), (2, "kept", 1), (3, "new", 2)),
    ),
    LoadCommandWriteStrategyTestCase(
        description="merge supports composite unique keys",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_merge_composite
    description: Test source raw_merge_composite.
    managed: true
    write_strategy: merge
    unique_key: [entity_id, source]
    cursor_column: version
    columns:
      - name: entity_id
        type: INTEGER
      - name: source
        type: VARCHAR
      - name: value
        type: VARCHAR
      - name: version
        type: INTEGER
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_merge_composite(ctx):
    '''Test loader raw_merge_composite.'''
    if ctx.current_cursor_value is None:
        return [{"entity_id": 1, "source": "api", "value": "old", "version": 1}]
    return [
        {"entity_id": 1, "source": "api", "value": "new", "version": 2},
        {"entity_id": 1, "source": "feed", "value": "inserted", "version": 2},
    ]
""",
        },
        select_sql=(
            "SELECT entity_id, source, value, version FROM raw_merge_composite ORDER BY source"
        ),
        expected_rows=((1, "api", "new", 2), (1, "feed", "inserted", 2)),
    ),
    LoadCommandWriteStrategyTestCase(
        description="delete insert replaces loaded cursor range and preserves outside rows",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_delete_insert_events
    description: Test source raw_delete_insert_events.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_delete_insert_events(ctx):
    '''Test loader raw_delete_insert_events.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_id": 1, "status": "old-outside-low"},
            {"event_id": 2, "status": "old-range"},
            {"event_id": 3, "status": "old-range"},
            {"event_id": 5, "status": "old-outside-high"},
        ]
    return [
        {"event_id": 2, "status": "new-range"},
        {"event_id": 3, "status": "new-range"},
    ]
""",
        },
        select_sql=(
            "SELECT event_id, status FROM raw_delete_insert_events ORDER BY event_id, status"
        ),
        expected_rows=(
            (1, "old-outside-low"),
            (2, "new-range"),
            (3, "new-range"),
            (5, "old-outside-high"),
        ),
    ),
    LoadCommandWriteStrategyTestCase(
        description="delete insert supports timestamp cursor ranges",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_delete_insert_ts
    description: Test source raw_delete_insert_ts.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_at
    columns:
      - name: event_at
        type: TIMESTAMP
      - name: status
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from datetime import datetime

from sqlbuild.loaders import loader

@loader
def raw_delete_insert_ts(ctx):
    '''Test loader raw_delete_insert_ts.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_at": datetime(2026, 1, 1, 0, 0, 0), "status": "outside-low"},
            {"event_at": datetime(2026, 1, 1, 1, 0, 0), "status": "old-range"},
            {"event_at": datetime(2026, 1, 1, 2, 0, 0), "status": "old-range"},
            {"event_at": datetime(2026, 1, 1, 3, 0, 0), "status": "outside-high"},
        ]
    return [
        {"event_at": datetime(2026, 1, 1, 1, 0, 0), "status": "new-range"},
        {"event_at": datetime(2026, 1, 1, 2, 0, 0), "status": "new-range"},
    ]
""",
        },
        select_sql=(
            "SELECT strftime(event_at, '%Y-%m-%d %H:%M:%S'), status "
            "FROM raw_delete_insert_ts ORDER BY event_at, status"
        ),
        expected_rows=(
            ("2026-01-01 00:00:00", "outside-low"),
            ("2026-01-01 01:00:00", "new-range"),
            ("2026-01-01 02:00:00", "new-range"),
            ("2026-01-01 03:00:00", "outside-high"),
        ),
    ),
    LoadCommandWriteStrategyTestCase(
        description="delete insert empty rerun leaves existing target unchanged",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_delete_insert_empty
    description: Test source raw_delete_insert_empty.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_delete_insert_empty(ctx):
    '''Test loader raw_delete_insert_empty.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_id": 1, "status": "existing"},
            {"event_id": 2, "status": "existing"},
        ]
    return []
""",
        },
        select_sql="SELECT event_id, status FROM raw_delete_insert_empty ORDER BY event_id",
        expected_rows=((1, "existing"), (2, "existing")),
    ),
]

CURSOR_NONE_CASE_FIXTURES: list[LoadCommandCursorNoneTestCase] = [
    LoadCommandCursorNoneTestCase(
        description="current cursor value is none when no cursor column is configured",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_no_cursor
    description: Test source raw_no_cursor.
    managed: true
    write_strategy: append
    columns:
      - name: run_number
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

run_count = 0

@loader
def raw_no_cursor(ctx):
    '''Test loader raw_no_cursor.'''
    global run_count
    run_count += 1
    return [{"run_number": run_count, "cursor_seen": str(ctx.current_cursor_value)}]
""",
        },
        select_sql="SELECT run_number, cursor_seen FROM raw_no_cursor ORDER BY run_number",
        expected_rows=((1, "None"), (1, "None")),
    ),
    LoadCommandCursorNoneTestCase(
        description="current cursor value is none when existing target is empty",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_empty_cursor
    description: Test source raw_empty_cursor.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_empty_cursor(ctx):
    '''Test loader raw_empty_cursor.'''
    return [{"event_id": 1, "cursor_seen": str(ctx.current_cursor_value)}]
""",
        },
        select_sql="SELECT event_id, cursor_seen FROM raw_empty_cursor ORDER BY event_id",
        expected_rows=((1, "None"),),
        run_count=1,
        setup_sql=("CREATE TABLE raw_empty_cursor(event_id INTEGER, cursor_seen VARCHAR)",),
    ),
    LoadCommandCursorNoneTestCase(
        description="current cursor value is none when target does not exist",
        project_files={
            "sqlbuild_project.toml": _PROJECT_FILE,
            "sources/raw.yml": """
sources:
  - name: raw_missing_cursor_target
    description: Test source raw_missing_cursor_target.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
            + "\n",
            "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_missing_cursor_target(ctx):
    '''Test loader raw_missing_cursor_target.'''
    return [{"event_id": 1, "cursor_seen": str(ctx.current_cursor_value)}]
""",
        },
        select_sql=(
            "SELECT event_id, cursor_seen FROM raw_missing_cursor_target ORDER BY event_id"
        ),
        expected_rows=((1, "None"),),
        run_count=1,
    ),
]

RELOAD_CONTEXT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILE,
    "sources/raw.yml": """
sources:
  - name: raw_reload_context
    description: Test source raw_reload_context.
    managed: true
    write_strategy: table
    columns:
      - name: is_reload
        type: BOOLEAN
""".strip()
    + "\n",
    "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_reload_context(ctx):
    '''Test loader raw_reload_context.'''
    return [{"is_reload": ctx.is_reload}]
""",
}

CURSOR_OVERRIDE_CONTEXT_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_FILE,
    "sources/raw.yml": """
sources:
  - name: raw_cursor_overrides
    description: Test source raw_cursor_overrides.
    managed: true
    write_strategy: table
    columns:
      - name: start_ts
        type: VARCHAR
      - name: end_ts
        type: VARCHAR
      - name: start_int
        type: VARCHAR
      - name: end_int
        type: VARCHAR
""".strip()
    + "\n",
    "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_cursor_overrides(ctx):
    '''Test loader raw_cursor_overrides.'''
    return [{
        "start_ts": None if ctx.start_cursor_ts is None else ctx.start_cursor_ts.isoformat(),
        "end_ts": None if ctx.end_cursor_ts is None else ctx.end_cursor_ts.isoformat(),
        "start_int": None if ctx.start_cursor_int is None else str(ctx.start_cursor_int),
        "end_int": None if ctx.end_cursor_int is None else str(ctx.end_cursor_int),
    }]
""",
}


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandWriteStrategyTestCase(
            description="append creates target then appends rows using current cursor value",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_append_events
    description: Test source raw_append_events.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_append_events(ctx):
    '''Test loader raw_append_events.'''
    if ctx.current_cursor_value is None:
        return [{"event_id": 1, "cursor_seen": "none"}]
    next_id = int(ctx.current_cursor_value) + 1
    return [{"event_id": next_id, "cursor_seen": str(ctx.current_cursor_value)}]
""",
            },
            select_sql="SELECT event_id, cursor_seen FROM raw_append_events ORDER BY event_id",
            expected_rows=((1, "none"), (2, "1")),
        ),
        LoadCommandWriteStrategyTestCase(
            description="merge updates existing rows and inserts new rows using cursor value",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_merge_customers
    description: Test source raw_merge_customers.
    managed: true
    write_strategy: merge
    unique_key: customer_id
    cursor_column: updated_at
    columns:
      - name: customer_id
        type: INTEGER
      - name: name
        type: VARCHAR
      - name: updated_at
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_merge_customers(ctx):
    '''Test loader raw_merge_customers.'''
    if ctx.current_cursor_value is None:
        return [
            {"customer_id": 1, "name": "old", "updated_at": 1},
            {"customer_id": 2, "name": "kept", "updated_at": 1},
        ]
    return [
        {"customer_id": 1, "name": f"updated:{ctx.current_cursor_value}", "updated_at": 2},
        {"customer_id": 3, "name": "new", "updated_at": 2},
    ]
""",
            },
            select_sql=(
                "SELECT customer_id, name, updated_at FROM raw_merge_customers ORDER BY customer_id"
            ),
            expected_rows=((1, "updated:1", 2), (2, "kept", 1), (3, "new", 2)),
        ),
        LoadCommandWriteStrategyTestCase(
            description="merge supports composite unique keys",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_merge_composite
    description: Test source raw_merge_composite.
    managed: true
    write_strategy: merge
    unique_key: [entity_id, source]
    cursor_column: version
    columns:
      - name: entity_id
        type: INTEGER
      - name: source
        type: VARCHAR
      - name: value
        type: VARCHAR
      - name: version
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_merge_composite(ctx):
    '''Test loader raw_merge_composite.'''
    if ctx.current_cursor_value is None:
        return [{"entity_id": 1, "source": "api", "value": "old", "version": 1}]
    return [
        {"entity_id": 1, "source": "api", "value": "new", "version": 2},
        {"entity_id": 1, "source": "feed", "value": "inserted", "version": 2},
    ]
""",
            },
            select_sql=(
                "SELECT entity_id, source, value, version FROM raw_merge_composite ORDER BY source"
            ),
            expected_rows=((1, "api", "new", 2), (1, "feed", "inserted", 2)),
        ),
        LoadCommandWriteStrategyTestCase(
            description="delete insert replaces loaded cursor range and preserves outside rows",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_delete_insert_events
    description: Test source raw_delete_insert_events.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_delete_insert_events(ctx):
    '''Test loader raw_delete_insert_events.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_id": 1, "status": "old-outside-low"},
            {"event_id": 2, "status": "old-range"},
            {"event_id": 3, "status": "old-range"},
            {"event_id": 5, "status": "old-outside-high"},
        ]
    return [
        {"event_id": 2, "status": "new-range"},
        {"event_id": 3, "status": "new-range"},
    ]
""",
            },
            select_sql=(
                "SELECT event_id, status FROM raw_delete_insert_events ORDER BY event_id, status"
            ),
            expected_rows=(
                (1, "old-outside-low"),
                (2, "new-range"),
                (3, "new-range"),
                (5, "old-outside-high"),
            ),
        ),
        LoadCommandWriteStrategyTestCase(
            description="delete insert supports timestamp cursor ranges",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_delete_insert_ts
    description: Test source raw_delete_insert_ts.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_at
    columns:
      - name: event_at
        type: TIMESTAMP
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from datetime import datetime

from sqlbuild.loaders import loader

@loader
def raw_delete_insert_ts(ctx):
    '''Test loader raw_delete_insert_ts.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_at": datetime(2026, 1, 1, 0, 0, 0), "status": "outside-low"},
            {"event_at": datetime(2026, 1, 1, 1, 0, 0), "status": "old-range"},
            {"event_at": datetime(2026, 1, 1, 2, 0, 0), "status": "old-range"},
            {"event_at": datetime(2026, 1, 1, 3, 0, 0), "status": "outside-high"},
        ]
    return [
        {"event_at": datetime(2026, 1, 1, 1, 0, 0), "status": "new-range"},
        {"event_at": datetime(2026, 1, 1, 2, 0, 0), "status": "new-range"},
    ]
""",
            },
            select_sql=(
                "SELECT strftime(event_at, '%Y-%m-%d %H:%M:%S'), status "
                "FROM raw_delete_insert_ts ORDER BY event_at, status"
            ),
            expected_rows=(
                ("2026-01-01 00:00:00", "outside-low"),
                ("2026-01-01 01:00:00", "new-range"),
                ("2026-01-01 02:00:00", "new-range"),
                ("2026-01-01 03:00:00", "outside-high"),
            ),
        ),
        LoadCommandWriteStrategyTestCase(
            description="delete insert empty rerun leaves existing target unchanged",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_delete_insert_empty
    description: Test source raw_delete_insert_empty.
    managed: true
    write_strategy: delete_insert
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_delete_insert_empty(ctx):
    '''Test loader raw_delete_insert_empty.'''
    if ctx.current_cursor_value is None:
        return [
            {"event_id": 1, "status": "existing"},
            {"event_id": 2, "status": "existing"},
        ]
    return []
""",
            },
            select_sql="SELECT event_id, status FROM raw_delete_insert_empty ORDER BY event_id",
            expected_rows=((1, "existing"), (2, "existing")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_write_strategy_when_running_load_twice_then_writes_expected_rows(
    test_case: LoadCommandWriteStrategyTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    for _ in range(test_case.run_count):
        exit_code: int = run_load_command(
            request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
        )
        assert exit_code == 0

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(test_case.select_sql).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandWriteStrategyLifecycleTestCase(
            description="append creates target first then inserts on rerun",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[0].project_files,
            expected_first_run_fragments=(
                "CREATE OR REPLACE TABLE raw_append_events AS SELECT * FROM raw_append_events__staging",
            ),
            expected_second_run_fragments=(
                'INSERT INTO raw_append_events ("event_id", "cursor_seen") '
                "SELECT * FROM raw_append_events__staging",
            ),
            absent_second_run_fragments=(
                "CREATE OR REPLACE TABLE raw_append_events AS SELECT * FROM raw_append_events__staging",
            ),
        ),
        LoadCommandWriteStrategyLifecycleTestCase(
            description="merge creates target first then merges on rerun",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[1].project_files,
            expected_first_run_fragments=(
                "CREATE OR REPLACE TABLE raw_merge_customers AS SELECT * FROM "
                "raw_merge_customers__staging",
            ),
            expected_second_run_fragments=("MERGE INTO raw_merge_customers",),
            absent_second_run_fragments=(
                "CREATE OR REPLACE TABLE raw_merge_customers AS SELECT * FROM "
                "raw_merge_customers__staging",
            ),
        ),
        LoadCommandWriteStrategyLifecycleTestCase(
            description="merge composite key uses every key in merge condition",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[2].project_files,
            expected_first_run_fragments=(
                "CREATE OR REPLACE TABLE raw_merge_composite AS SELECT * FROM "
                "raw_merge_composite__staging",
            ),
            expected_second_run_fragments=(
                "MERGE INTO raw_merge_composite",
                '__target."entity_id" = __source."entity_id"',
                '__target."source" = __source."source"',
            ),
        ),
        LoadCommandWriteStrategyLifecycleTestCase(
            description="delete insert creates target first then deletes cursor range on rerun",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[3].project_files,
            expected_first_run_fragments=(
                "CREATE OR REPLACE TABLE raw_delete_insert_events AS SELECT * FROM "
                "raw_delete_insert_events__staging",
            ),
            expected_second_run_fragments=(
                'SELECT MIN("event_id"), MAX("event_id") FROM raw_delete_insert_events__staging',
                "DELETE FROM raw_delete_insert_events WHERE \"event_id\" >= '2' AND \"event_id\" < '4'",
                "INSERT INTO raw_delete_insert_events SELECT * FROM raw_delete_insert_events__staging",
            ),
            absent_second_run_fragments=(
                "CREATE OR REPLACE TABLE raw_delete_insert_events AS SELECT * FROM "
                "raw_delete_insert_events__staging",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_write_strategy_when_running_pipeline_then_uses_expected_dml(
    test_case: LoadCommandWriteStrategyLifecycleTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()

    first_results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )
    second_results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    first_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in first_results[0].lifecycle_events:
        first_content_by_kind[event.kind.value].append(event.content)
    first_lifecycle_sql: tuple[str, ...] = tuple(first_content_by_kind["sql"])
    second_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in second_results[0].lifecycle_events:
        second_content_by_kind[event.kind.value].append(event.content)
    second_lifecycle_sql: tuple[str, ...] = tuple(second_content_by_kind["sql"])
    first_fragments_found: list[bool] = []
    for fragment in test_case.expected_first_run_fragments:
        first_fragments_found.append(any(fragment in sql for sql in first_lifecycle_sql))
    assert all(first_fragments_found)
    second_fragments_found: list[bool] = []
    for fragment in test_case.expected_second_run_fragments:
        second_fragments_found.append(any(fragment in sql for sql in second_lifecycle_sql))
    assert all(second_fragments_found)
    second_fragments_absent: list[bool] = []
    for fragment in test_case.absent_second_run_fragments:
        second_fragments_absent.append(all(fragment not in sql for sql in second_lifecycle_sql))
    assert all(second_fragments_absent)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandAdapterCallTestCase(
            description="append rerun calls adapter append with staging select",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[0].project_files,
            method_name="append",
            expected_sql="SELECT * FROM raw_append_events__staging",
        ),
        LoadCommandAdapterCallTestCase(
            description="merge rerun calls adapter merge with staging select and unique key",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[1].project_files,
            method_name="merge",
            expected_sql="SELECT * FROM raw_merge_customers__staging",
            expected_arguments=("customer_id",),
        ),
        LoadCommandAdapterCallTestCase(
            description="delete insert rerun calls adapter delete insert cursor with staging select",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[3].project_files,
            method_name="delete_insert_cursor",
            expected_sql="SELECT * FROM raw_delete_insert_events__staging",
            expected_arguments=("event_id", "2", "4", CursorKind.INTEGER),
        ),
        LoadCommandAdapterCallTestCase(
            description="timestamp delete insert rerun propagates native cursor kind",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[4].project_files,
            method_name="delete_insert_cursor",
            expected_sql="SELECT * FROM raw_delete_insert_ts__staging",
            expected_arguments=(
                "event_at",
                "2026-01-01T01:00:00",
                "2026-01-01T02:00:00.000001",
                CursorKind.TIMESTAMP,
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_source_loader_write_strategy_when_rerunning_then_calls_expected_adapter_method(
    test_case: LoadCommandAdapterCallTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    monkeypatch: MonkeyPatch,
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()
    calls: list[tuple[str, str, tuple[str | None, ...]]] = []

    run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    def append_spy(
        connection: object,
        *,
        destination: str,
        sql: str,
        columns: tuple[str, ...] | None = None,
        statement_recorder: object,
    ) -> None:
        calls.append(("append", sql, ()))

    def merge_spy(
        connection: object,
        *,
        destination: str,
        sql: str,
        unique_key: tuple[str, ...],
        statement_recorder: object,
    ) -> None:
        calls.append(("merge", sql, unique_key))

    def delete_insert_cursor_spy(
        connection: object,
        *,
        destination: str,
        sql: str,
        cursor_column: str,
        cursor_start: str,
        cursor_end: str,
        statement_recorder: object,
        cursor_type: str | None = None,
    ) -> None:
        calls.append(
            (
                "delete_insert_cursor",
                sql,
                (cursor_column, cursor_start, cursor_end, cursor_type),
            )
        )

    monkeypatch.setattr(adapter, "append", append_spy)
    monkeypatch.setattr(adapter, "merge", merge_spy)
    monkeypatch.setattr(adapter, "delete_insert_cursor", delete_insert_cursor_spy)

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    assert results[0].status.value == "success"
    assert calls == [(test_case.method_name, test_case.expected_sql, test_case.expected_arguments)]


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandCursorNoneTestCase(
            description="current cursor value is none when no cursor column is configured",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_no_cursor
    description: Test source raw_no_cursor.
    managed: true
    write_strategy: append
    columns:
      - name: run_number
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

run_count = 0

@loader
def raw_no_cursor(ctx):
    '''Test loader raw_no_cursor.'''
    global run_count
    run_count += 1
    return [{"run_number": run_count, "cursor_seen": str(ctx.current_cursor_value)}]
""",
            },
            select_sql="SELECT run_number, cursor_seen FROM raw_no_cursor ORDER BY run_number",
            expected_rows=((1, "None"), (1, "None")),
        ),
        LoadCommandCursorNoneTestCase(
            description="current cursor value is none when existing target is empty",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_empty_cursor
    description: Test source raw_empty_cursor.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_empty_cursor(ctx):
    '''Test loader raw_empty_cursor.'''
    return [{"event_id": 1, "cursor_seen": str(ctx.current_cursor_value)}]
""",
            },
            select_sql="SELECT event_id, cursor_seen FROM raw_empty_cursor ORDER BY event_id",
            expected_rows=((1, "None"),),
            run_count=1,
            setup_sql=("CREATE TABLE raw_empty_cursor(event_id INTEGER, cursor_seen VARCHAR)",),
        ),
        LoadCommandCursorNoneTestCase(
            description="current cursor value is none when target does not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_missing_cursor_target
    description: Test source raw_missing_cursor_target.
    managed: true
    write_strategy: append
    cursor_column: event_id
    columns:
      - name: event_id
        type: INTEGER
      - name: cursor_seen
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_missing_cursor_target(ctx):
    '''Test loader raw_missing_cursor_target.'''
    return [{"event_id": 1, "cursor_seen": str(ctx.current_cursor_value)}]
""",
            },
            select_sql=(
                "SELECT event_id, cursor_seen FROM raw_missing_cursor_target ORDER BY event_id"
            ),
            expected_rows=((1, "None"),),
            run_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_cursor_context_has_no_value_when_running_load_then_passes_none(
    test_case: LoadCommandCursorNoneTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            connection.execute(setup_statement)
    finally:
        connection.close()

    for _ in range(test_case.run_count):
        exit_code: int = run_load_command(
            request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
        )
        assert exit_code == 0

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(test_case.select_sql).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandLifecycleSqlTestCase(
            description="records cursor max query only when target exists",
            project_files=WRITE_STRATEGY_CASE_FIXTURES[0].project_files,
            run_count=2,
            expected_lifecycle_sql_fragments=('SELECT MAX("event_id") FROM raw_append_events',),
        ),
        LoadCommandLifecycleSqlTestCase(
            description="does not query cursor when source has no cursor column",
            project_files=CURSOR_NONE_CASE_FIXTURES[0].project_files,
            run_count=2,
            absent_lifecycle_sql_fragments=("SELECT MAX(",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_cursor_configuration_when_running_pipeline_then_records_expected_cursor_sql(
    test_case: LoadCommandLifecycleSqlTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()
    all_lifecycle_sql: list[str] = []

    for _ in range(test_case.run_count):
        results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
            sources=source_file.source_entries,
            loader_functions=discovered_inputs.loader_functions,
            connection_config={"database": str(tmp_path / "demo.duckdb")},
            adapter=adapter,
            runtime=LoadRuntimeParams(
                run_id="test_run",
                target="dev",
                vars={},
                is_reload=False,
            ),
        )
        lifecycle_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
        for event in results[0].lifecycle_events:
            lifecycle_content_by_kind[event.kind.value].append(event.content)
        all_lifecycle_sql.extend(lifecycle_content_by_kind["sql"])

    fragments_found: list[bool] = []
    for fragment in test_case.expected_lifecycle_sql_fragments:
        fragments_found.append(any(fragment in sql for sql in all_lifecycle_sql))
    assert all(fragments_found)
    fragments_absent: list[bool] = []
    for fragment in test_case.absent_lifecycle_sql_fragments:
        fragments_absent.append(all(fragment not in sql for sql in all_lifecycle_sql))
    assert all(fragments_absent)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandReloadContextTestCase(
            description="passes false reload context by default",
            project_files=RELOAD_CONTEXT_PROJECT_FILES,
            reload=False,
            expected_rows=((False,),),
        ),
        LoadCommandReloadContextTestCase(
            description="passes true reload context when reload flag is used",
            project_files=RELOAD_CONTEXT_PROJECT_FILES,
            reload=True,
            expected_rows=((True,),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reload_flag_when_running_load_then_passes_reload_context_to_loader(
    test_case: LoadCommandReloadContextTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            reload=test_case.reload,
        )
    )

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute("SELECT is_reload FROM raw_reload_context").fetchall()
        )
    finally:
        connection.close()
    assert exit_code == 0
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandCursorOverrideContextTestCase(
            description="passes no cursor override context by default",
            project_files=CURSOR_OVERRIDE_CONTEXT_PROJECT_FILES,
            cursor_overrides=None,
            expected_rows=((None, None, None, None),),
        ),
        LoadCommandCursorOverrideContextTestCase(
            description="passes typed cursor override context values",
            project_files=CURSOR_OVERRIDE_CONTEXT_PROJECT_FILES,
            cursor_overrides=CursorOverrides(
                start_ts="2026-01-01T01:02:03",
                end_ts="2026-01-02T04:05:06",
                start_int="10",
                end_int="20",
            ),
            expected_rows=(("2026-01-01T01:02:03", "2026-01-02T04:05:06", "10", "20"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_override_flags_when_running_load_then_passes_typed_context_to_loader(
    test_case: LoadCommandCursorOverrideContextTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            cursor_overrides=test_case.cursor_overrides,
        )
    )

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(
                "SELECT start_ts, end_ts, start_int, end_int FROM raw_cursor_overrides"
            ).fetchall()
        )
    finally:
        connection.close()
    assert exit_code == 0
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandConcurrencyTestCase(
            description="uses bounded concurrent connections and preserves result order",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_a
    description: Test source raw_a.
    managed: true
    write_strategy: table
    columns:
      - name: source_name
        type: VARCHAR
      - name: connection_id
        type: BIGINT
  - name: raw_b
    description: Test source raw_b.
    managed: true
    write_strategy: table
    columns:
      - name: source_name
        type: VARCHAR
      - name: connection_id
        type: BIGINT
  - name: raw_c
    description: Test source raw_c.
    managed: true
    write_strategy: table
    columns:
      - name: source_name
        type: VARCHAR
      - name: connection_id
        type: BIGINT
""".strip()
                + "\n",
                "python/loaders/raw.py": """
import threading
import time

from sqlbuild.loaders import loader

barrier = threading.Barrier(2)

@loader
def raw_a(ctx):
    '''Test loader raw_a.'''
    barrier.wait(timeout=1)
    time.sleep(0.05)
    return [{"source_name": "raw_a", "connection_id": id(ctx.connection)}]

@loader
def raw_b(ctx):
    '''Test loader raw_b.'''
    barrier.wait(timeout=1)
    return [{"source_name": "raw_b", "connection_id": id(ctx.connection)}]

@loader
def raw_c(ctx):
    '''Test loader raw_c.'''
    return [{"source_name": "raw_c", "connection_id": id(ctx.connection)}]
""",
            },
            max_concurrency=2,
            expected_connection_count=2,
            expected_source_order=("raw_a", "raw_b", "raw_c"),
            expected_json_asset_order=("raw_a", "raw_b", "raw_c"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_multiple_source_loaders_when_running_pipeline_then_uses_concurrent_connections(
    test_case: LoadCommandConcurrencyTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection_starts: list[int] = []

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        max_concurrency=test_case.max_concurrency,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
        callbacks=LoadCallbacks(
            on_connection_start=connection_starts.append,
        ),
    )

    assert connection_starts == [test_case.expected_connection_count]
    assert tuple(result.source_name for result in results) == test_case.expected_source_order
    payload: dict[str, Any] = cast(
        dict[str, Any], json.loads(format_load_execution_json(results=results))
    )
    assets: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["assets"])
    assert tuple(asset["name"] for asset in assets) == test_case.expected_json_asset_order
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        first_row: tuple[int] | None = connection.execute(
            "SELECT connection_id FROM raw_a"
        ).fetchone()
        second_row: tuple[int] | None = connection.execute(
            "SELECT connection_id FROM raw_b"
        ).fetchone()
    finally:
        connection.close()
    assert first_row is not None
    assert second_row is not None
    first_connection_id: int = first_row[0]
    second_connection_id: int = second_row[0]
    assert first_connection_id != second_connection_id


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandConcurrencyTestCase(
            description="runs independent loader branches concurrently before dependent source",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_join
    description: Test source raw_join.
    managed: true
    write_strategy: table
    columns:
      - name: upstream_count
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
import threading
import time

from sqlbuild.loaders import loader

barrier = threading.Barrier(2)
finished = set()
lock = threading.Lock()

@loader(write_strategy='table', columns=[{'name': 'id', 'type': 'INTEGER'}])
def fetch_a(ctx):
    '''Test loader fetch_a.'''
    barrier.wait(timeout=1)
    time.sleep(0.05)
    with lock:
        finished.add('a')
    return [{'id': 1}]

@loader(write_strategy='table', columns=[{'name': 'id', 'type': 'INTEGER'}])
def fetch_b(ctx):
    '''Test loader fetch_b.'''
    barrier.wait(timeout=1)
    with lock:
        finished.add('b')
    return [{'id': 2}]

@loader(depends_on=[fetch_a, fetch_b])
def raw_join(ctx):
    '''Test loader raw_join.'''
    return [{'upstream_count': len(finished)}]
""",
            },
            max_concurrency=2,
            expected_connection_count=2,
            expected_source_order=("fetch_a", "fetch_b", "raw_join"),
            expected_json_asset_order=("fetch_a", "fetch_b", "raw_join"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_loader_dag_when_running_pipeline_then_independent_branches_overlap(
    test_case: LoadCommandConcurrencyTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection_starts: list[int] = []

    plain_selected_sources: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=discovered_inputs,
        select=("raw_join",),
        exclude=(),
        target_config=None,
    )
    assert tuple(source.name for source in plain_selected_sources) == ("raw_join",)

    terminal_loader_selected_sources: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=discovered_inputs,
        select=("raw_join",),
        exclude=(),
        target_config=None,
    )
    assert tuple(source.name for source in terminal_loader_selected_sources) == ("raw_join",)

    selected_sources: tuple[SourceEntry, ...] = select_load_entries(
        discovered_inputs=discovered_inputs,
        select=("+raw_join",),
        exclude=(),
        target_config=None,
    )
    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=selected_sources,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        max_concurrency=test_case.max_concurrency,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
        callbacks=LoadCallbacks(
            on_connection_start=connection_starts.append,
        ),
    )

    assert connection_starts == [test_case.expected_connection_count]
    assert tuple(result.source_name for result in results) == test_case.expected_source_order
    payload: dict[str, Any] = cast(
        dict[str, Any], json.loads(format_load_execution_json(results=results))
    )
    assets: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["assets"])
    assert tuple(asset["name"] for asset in assets) == test_case.expected_json_asset_order
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        row: tuple[int] | None = connection.execute(
            "SELECT upstream_count FROM raw_join"
        ).fetchone()
    finally:
        connection.close()
    assert row == (2,)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandInferredColumnsTestCase(
            description="loads generator rows with declared missing and inferred extra columns",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_inferred
    description: Test source raw_inferred.
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
      - name: notes
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from datetime import date, datetime

from sqlbuild.loaders import loader

@loader
def raw_inferred(ctx):
    '''Test loader raw_inferred.'''
    yield {
        "id": 1,
        "flag": True,
        "amount": 2.5,
        "name": "customer's order",
        "payload": {"source": "loader"},
        "tags": ["new", "priority"],
        "created_at": datetime(2026, 5, 21, 12, 30, 0),
        "service_date": date(2026, 5, 21),
    }
""",
            },
            expected_row=(
                1,
                None,
                True,
                2.5,
                "customer's order",
                '{"source": "loader"}',
                '["new", "priority"]',
                datetime(2026, 5, 21, 12, 30, 0),
                date(2026, 5, 21),
            ),
            expected_column_types={
                "id": "INTEGER",
                "notes": "VARCHAR",
                "flag": "BOOLEAN",
                "amount": "DOUBLE",
                "name": "VARCHAR",
                "payload": "JSON",
                "tags": "JSON",
                "created_at": "TIMESTAMP",
                "service_date": "DATE",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generator_loader_with_inferred_columns_when_running_load_then_writes_schema_and_data(
    test_case: LoadCommandInferredColumnsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
    )

    assert exit_code == 0
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        row: tuple[object, ...] | None = connection.execute(
            "SELECT id, notes, flag, amount, name, CAST(payload AS VARCHAR), "
            "CAST(tags AS VARCHAR), created_at, service_date "
            "FROM raw_inferred"
        ).fetchone()
        column_rows: list[tuple[str, str]] = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name = 'raw_inferred'"
        ).fetchall()
    finally:
        connection.close()
    assert row == test_case.expected_row
    column_types: dict[str, str] = dict(column_rows)
    expected_column: str
    for expected_column, expected_type in test_case.expected_column_types.items():
        assert column_types[expected_column] == expected_type


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandInferredColumnsTestCase(
            description="decimals keep their precision and declared columns take any value",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_inferred
    description: Test source raw_inferred.
    managed: true
    write_strategy: table
    columns:
      - name: order_ref
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from decimal import Decimal
from uuid import UUID

from sqlbuild.loaders import loader

@loader
def raw_inferred(ctx):
    '''Test loader raw_inferred.'''
    yield {"order_ref": UUID("12345678-1234-5678-1234-567812345678"), "price": Decimal("12.50")}
    yield {"order_ref": None, "price": Decimal("1234.5")}
""",
            },
            expected_row=("12345678-1234-5678-1234-567812345678", Decimal("12.50")),
            expected_column_types={"order_ref": "VARCHAR", "price": "DECIMAL(6,2)"},
        ),
        LoadCommandInferredColumnsTestCase(
            description="wide decimals stay exact beside values written with an exponent",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_inferred
    description: Test source raw_inferred.
    managed: true
    write_strategy: table
    columns:
      - name: order_ref
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from decimal import Decimal

from sqlbuild.loaders import loader

@loader
def raw_inferred(ctx):
    '''Test loader raw_inferred.'''
    yield {"order_ref": "a_wide", "price": Decimal("12345678901234567890.123")}
    yield {"order_ref": "b_tiny", "price": Decimal("1E-7")}
""",
            },
            expected_row=("a_wide", Decimal("12345678901234567890.123")),
            expected_column_types={"order_ref": "VARCHAR", "price": "DECIMAL(27,7)"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_decimal_and_declared_values_when_running_load_then_keeps_their_types(
    test_case: LoadCommandInferredColumnsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
    )

    assert exit_code == 0
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        row: tuple[object, ...] | None = connection.execute(
            "SELECT order_ref, price FROM raw_inferred ORDER BY order_ref NULLS LAST LIMIT 1"
        ).fetchone()
        column_rows: list[tuple[str, str]] = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name = 'raw_inferred'"
        ).fetchall()
    finally:
        connection.close()
    assert row == test_case.expected_row
    assert dict(column_rows) == test_case.expected_column_types


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandMultipleYieldTestCase(
            description="loads every row yielded by a generator loader",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_multi_yield
    description: Test source raw_multi_yield.
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_multi_yield(ctx):
    '''Test loader raw_multi_yield.'''
    yield {"id": 1, "status": "first"}
    yield {"id": 2, "status": "second"}
""",
            },
            expected_rows=((1, "first"), (2, "second")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generator_loader_yields_multiple_rows_when_running_load_then_writes_all_rows(
    test_case: LoadCommandMultipleYieldTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
    )

    assert exit_code == 0
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: list[tuple[object, ...]] = connection.execute(
            "SELECT id, status FROM raw_multi_yield ORDER BY id"
        ).fetchall()
    finally:
        connection.close()
    assert tuple(rows) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandBatchedYieldTestCase(
            description="loads generator rows in batches and preserves late extra columns",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_batched_yield
    description: Test source raw_batched_yield.
    managed: true
    write_strategy: table
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_batched_yield(ctx):
    '''Test loader raw_batched_yield.'''
    yield {"id": 1, "status": "first"}
    yield {"id": 2, "status": "second", "late_flag": True}
    yield {"id": 3, "status": "third", "late_flag": False}
""",
            },
            expected_rows=((1, "first", None), (2, "second", True), (3, "third", False)),
            expected_column_types={"id": "INTEGER", "status": "VARCHAR", "late_flag": "BOOLEAN"},
            expected_lifecycle_sql_fragments=(
                "CREATE OR REPLACE TABLE raw_batched_yield__staging",
                'ALTER TABLE raw_batched_yield__staging ADD COLUMN "late_flag" BOOLEAN',
                'INSERT INTO raw_batched_yield__staging ("id", "status", "late_flag")',
                "CREATE OR REPLACE TABLE raw_batched_yield AS SELECT * "
                "FROM raw_batched_yield__staging",
                "DROP TABLE IF EXISTS raw_batched_yield__staging",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_generator_loader_uses_batch_size_when_running_pipeline_then_appends_batches(
    test_case: LoadCommandBatchedYieldTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    assert results[0].rows_loaded == len(test_case.expected_rows)
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: list[tuple[object, ...]] = connection.execute(
            "SELECT id, status, late_flag FROM raw_batched_yield ORDER BY id"
        ).fetchall()
        column_rows: list[tuple[str, str]] = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name = 'raw_batched_yield'"
        ).fetchall()
    finally:
        connection.close()
    lifecycle_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in results[0].lifecycle_events:
        lifecycle_content_by_kind[event.kind.value].append(event.content)
    lifecycle_sql: tuple[str, ...] = tuple(lifecycle_content_by_kind["sql"])
    assert tuple(rows) == test_case.expected_rows
    column_types: dict[str, str] = dict(column_rows)
    expected_column: str
    for expected_column, expected_type in test_case.expected_column_types.items():
        assert column_types[expected_column] == expected_type
    fragments_found: list[bool] = []
    for fragment in test_case.expected_lifecycle_sql_fragments:
        fragments_found.append(any(fragment in sql for sql in lifecycle_sql))
    assert all(fragments_found)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandBatchedRowsTestCase(
            description="loads missing known columns as null in later batches",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_missing_known
    description: Test source raw_missing_known.
    managed: true
    write_strategy: table
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_missing_known(ctx):
    '''Test loader raw_missing_known.'''
    yield {"id": 1, "status": "first"}
    yield {"id": 2}
""",
            },
            select_sql="SELECT id, status FROM raw_missing_known ORDER BY id",
            table_name="raw_missing_known",
            expected_rows=((1, "first"), (2, None)),
            expected_column_types={"id": "INTEGER", "status": "VARCHAR"},
            expected_rows_loaded=2,
        ),
        LoadCommandBatchedRowsTestCase(
            description="loads empty generator into declared table through batched path",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_empty_generator
    description: Test source raw_empty_generator.
    managed: true
    write_strategy: table
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_empty_generator(ctx):
    '''Test loader raw_empty_generator.'''
    if False:
        yield {"id": 1, "status": "unreachable"}
""",
            },
            select_sql="SELECT COUNT(*) FROM raw_empty_generator",
            table_name="raw_empty_generator",
            expected_rows=((0,),),
            expected_column_types={"id": "INTEGER", "status": "VARCHAR"},
            expected_rows_loaded=0,
        ),
        LoadCommandBatchedRowsTestCase(
            description="loads late all null column when typed value arrives later",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_late_null
    description: Test source raw_late_null.
    managed: true
    write_strategy: table
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_late_null(ctx):
    '''Test loader raw_late_null.'''
    yield {"id": 1, "late_note": None}
    yield {"id": 2, "late_note": "filled"}
""",
            },
            select_sql="SELECT id, late_note FROM raw_late_null ORDER BY id",
            table_name="raw_late_null",
            expected_rows=((1, None), (2, "filled")),
            expected_column_types={"id": "INTEGER", "late_note": "VARCHAR"},
            expected_rows_loaded=2,
        ),
        LoadCommandBatchedRowsTestCase(
            description="loads multi-row batches before appending final partial batch",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_batch_size_two
    description: Test source raw_batch_size_two.
    managed: true
    write_strategy: table
    load_batch_size: 2
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_batch_size_two(ctx):
    '''Test loader raw_batch_size_two.'''
    yield {"id": 1}
    yield {"id": 2}
    yield {"id": 3}
""",
            },
            select_sql="SELECT id FROM raw_batch_size_two ORDER BY id",
            table_name="raw_batch_size_two",
            expected_rows=((1,), (2,), (3,)),
            expected_column_types={"id": "INTEGER"},
            expected_rows_loaded=3,
            expected_lifecycle_sql_fragments=("INSERT INTO raw_batch_size_two__staging",),
        ),
        LoadCommandBatchedRowsTestCase(
            description="uses default batch size when source does not declare one",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_default_batch
    description: Test source raw_default_batch.
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_default_batch(ctx):
    '''Test loader raw_default_batch.'''
    yield {"id": 1}
    yield {"id": 2}
    yield {"id": 3}
""",
            },
            select_sql="SELECT id FROM raw_default_batch ORDER BY id",
            table_name="raw_default_batch",
            expected_rows=((1,), (2,), (3,)),
            expected_column_types={"id": "INTEGER"},
            expected_rows_loaded=3,
            absent_lifecycle_sql_fragments=("INSERT INTO raw_default_batch__staging",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_batched_loader_variants_when_running_pipeline_then_writes_expected_rows(
    test_case: LoadCommandBatchedRowsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: list[tuple[object, ...]] = connection.execute(test_case.select_sql).fetchall()
        column_rows: list[tuple[str, str]] = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            f"WHERE table_name = '{test_case.table_name}'"
        ).fetchall()
    finally:
        connection.close()
    lifecycle_content_by_kind: defaultdict[str, list[str]] = defaultdict(list)
    for event in results[0].lifecycle_events:
        lifecycle_content_by_kind[event.kind.value].append(event.content)
    lifecycle_sql: tuple[str, ...] = tuple(lifecycle_content_by_kind["sql"])
    assert results[0].rows_loaded == test_case.expected_rows_loaded
    assert tuple(rows) == test_case.expected_rows
    column_types: dict[str, str] = dict(column_rows)
    expected_column: str
    for expected_column, expected_type in test_case.expected_column_types.items():
        assert column_types[expected_column] == expected_type
    fragments_found: list[bool] = []
    for fragment in test_case.expected_lifecycle_sql_fragments:
        fragments_found.append(any(fragment in sql for sql in lifecycle_sql))
    assert all(fragments_found)
    fragments_absent: list[bool] = []
    for fragment in test_case.absent_lifecycle_sql_fragments:
        fragments_absent.append(all(fragment not in sql for sql in lifecycle_sql))
    assert all(fragments_absent)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandIntegrationTestCase(
            description="formats large load row counts with commas in human output",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_many_rows
    description: Test source raw_many_rows.
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_many_rows(ctx):
    '''Test loader raw_many_rows.'''
    for value in range(1001):
        yield {"id": value}
""",
            },
            expected_exit_code=0,
            expected_rows=(),
            expected_stdout_fragment="rows=1,001",
            expected_json_rows_loaded=1001,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_writes_many_rows_when_running_load_then_formats_human_row_count(
    test_case: LoadCommandIntegrationTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    json_output_path: Path = tmp_path / "target" / "load.json"

    exit_code: int = run_load_command(
        request=LoadCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            json_output_path=json_output_path,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    payload: dict[str, Any] = cast(
        dict[str, Any], json.loads(json_output_path.read_text(encoding="utf-8"))
    )
    assets: list[dict[str, Any]] = cast(list[dict[str, Any]], payload["assets"])
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_stdout_fragment in captured.out
    assert assets[0]["rows_loaded"] == test_case.expected_json_rows_loaded


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandEmptyRowsTestCase(
            description="loads empty returned rows into declared empty table",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_empty
    description: Test source raw_empty.
    managed: true
    write_strategy: table
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_empty(ctx):
    '''Test loader raw_empty.'''
    return []
""",
            },
            expected_column_types={"id": "INTEGER", "status": "VARCHAR"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_returns_empty_rows_when_running_load_then_writes_empty_declared_table(
    test_case: LoadCommandEmptyRowsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
    )

    assert exit_code == 0
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        row_count_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM raw_empty"
        ).fetchone()
        column_rows: list[tuple[str, str]] = connection.execute(
            "SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name = 'raw_empty'"
        ).fetchall()
    finally:
        connection.close()
    assert row_count_row is not None
    row_count: int = row_count_row[0]
    assert row_count == 0
    assert dict(column_rows) == test_case.expected_column_types


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandWriteStrategyTestCase(
            description="self managed loader can return nothing without write strategy",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_self_managed
    description: Test source raw_self_managed.
    managed: true
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_self_managed(ctx):
    '''Test loader raw_self_managed.'''
    ctx.execute_sql("CREATE OR REPLACE TABLE raw_self_managed AS SELECT 1 AS id")
""",
            },
            select_sql="SELECT id FROM raw_self_managed",
            expected_rows=((1,),),
            run_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_self_managed_loader_when_running_load_then_uses_loader_written_table(
    test_case: LoadCommandWriteStrategyTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    for _ in range(test_case.run_count):
        exit_code: int = run_load_command(
            request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
        )
        assert exit_code == 0

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        rows: tuple[tuple[object, ...], ...] = tuple(
            connection.execute(test_case.select_sql).fetchall()
        )
    finally:
        connection.close()
    assert rows == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandFailureTestCase(
            description="fails clearly when returned rows contain conflicting inferred types",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_conflict
    description: Test source raw_conflict.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_conflict(ctx):
    '''Test loader raw_conflict.'''
    return [
        {"id": 1},
        {"id": "two"},
    ]
""",
            },
            expected_exit_code=1,
            expected_stdout_fragment="conflicting types for column 'id'",
        ),
        LoadCommandFailureTestCase(
            description="fails clearly when an undeclared column holds an unrecognised type",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_unknown_type
    description: Test source raw_unknown_type.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from uuid import UUID

from sqlbuild.loaders import loader

@loader
def raw_unknown_type(ctx):
    '''Test loader raw_unknown_type.'''
    return [{"id": 1, "order_ref": UUID("12345678-1234-5678-1234-567812345678")}]
""",
            },
            expected_exit_code=1,
            expected_stdout_fragment=(
                "Source loader returned a UUID value for column 'order_ref', which has no "
                "declared type"
            ),
        ),
        LoadCommandFailureTestCase(
            description="fails clearly when a later batch needs a wider decimal",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_wider_decimal
    description: Test source raw_wider_decimal.
    managed: true
    write_strategy: table
    load_batch_size: 1
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from decimal import Decimal

from sqlbuild.loaders import loader

@loader
def raw_wider_decimal(ctx):
    '''Test loader raw_wider_decimal.'''
    yield {"price": Decimal("1.5")}
    yield {"price": Decimal("123.25")}
""",
            },
            expected_exit_code=1,
            expected_stdout_fragment=(
                "Source loader column 'price' needs DECIMAL(5, 2), wider than the "
                "DECIMAL(2, 1) its earlier rows gave it"
            ),
        ),
        LoadCommandFailureTestCase(
            description="fails when loader returns rows without write strategy",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_missing_strategy
    description: Test source raw_missing_strategy.
    managed: true
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_missing_strategy(ctx):
    '''Test loader raw_missing_strategy.'''
    return [{"id": 1}]
""",
            },
            expected_exit_code=1,
            expected_stdout_fragment="returned rows but source has no write_strategy",
        ),
        LoadCommandFailureTestCase(
            description="fails when loader returns nothing with write strategy",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_unexpected_none
    description: Test source raw_unexpected_none.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_unexpected_none(ctx):
    '''Test loader raw_unexpected_none.'''
    return None
""",
            },
            expected_exit_code=1,
            expected_stdout_fragment=(
                "defines write_strategy but loader 'raw_unexpected_none' returned no rows"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_returns_conflicting_types_when_running_load_then_fails_clearly(
    test_case: LoadCommandFailureTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(project_dir=tmp_path, no_color=True)
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_stdout_fragment in captured.out


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandFailureCleanupTestCase(
            description="drops staging and preserves target when a later loader batch fails",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_batched_conflict
    description: Test source raw_batched_conflict.
    managed: true
    write_strategy: table
    load_batch_size: 1
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_batched_conflict(ctx):
    '''Test loader raw_batched_conflict.'''
    yield {"id": 1}
    yield {"id": "two"}
""",
            },
            staging_table_name="raw_batched_conflict__staging",
            expected_staging_exists=False,
            setup_sql=(
                "CREATE TABLE raw_batched_conflict (id INTEGER)",
                "INSERT INTO raw_batched_conflict VALUES (99)",
            ),
            target_select_sql="SELECT id FROM raw_batched_conflict ORDER BY id",
            expected_target_rows=((99,),),
        ),
        LoadCommandFailureCleanupTestCase(
            description="drops staging when a later loader batch returns non dict row",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_batched_non_dict
    description: Test source raw_batched_non_dict.
    managed: true
    write_strategy: table
    load_batch_size: 1
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_batched_non_dict(ctx):
    '''Test loader raw_batched_non_dict.'''
    yield {"id": 1}
    yield ("id", 2)
""",
            },
            staging_table_name="raw_batched_non_dict__staging",
            expected_staging_exists=False,
            target_select_sql="SELECT NULL WHERE FALSE",
        ),
        LoadCommandFailureCleanupTestCase(
            description="leaves append target unchanged when a later loader batch fails",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_append_failure
    description: Test source raw_append_failure.
    managed: true
    write_strategy: append
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_append_failure(ctx):
    '''Test loader raw_append_failure.'''
    yield {"id": 1}
    yield {"id": "two"}
""",
            },
            staging_table_name="raw_append_failure__staging",
            expected_staging_exists=False,
            setup_sql=(
                "CREATE TABLE raw_append_failure (id INTEGER)",
                "INSERT INTO raw_append_failure VALUES (99)",
            ),
            target_select_sql="SELECT id FROM raw_append_failure ORDER BY id",
            expected_target_rows=((99,),),
        ),
        LoadCommandFailureCleanupTestCase(
            description="leaves merge target unchanged when a later loader batch fails",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_merge_failure
    description: Test source raw_merge_failure.
    managed: true
    write_strategy: merge
    unique_key: id
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_merge_failure(ctx):
    '''Test loader raw_merge_failure.'''
    yield {"id": 1, "status": "new"}
    yield {"id": "two", "status": "bad"}
""",
            },
            staging_table_name="raw_merge_failure__staging",
            expected_staging_exists=False,
            setup_sql=(
                "CREATE TABLE raw_merge_failure (id INTEGER, status VARCHAR)",
                "INSERT INTO raw_merge_failure VALUES (99, 'existing')",
            ),
            target_select_sql="SELECT id, status FROM raw_merge_failure ORDER BY id",
            expected_target_rows=((99, "existing"),),
        ),
        LoadCommandFailureCleanupTestCase(
            description="leaves delete insert target unchanged when a later loader batch fails",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_delete_insert_failure
    description: Test source raw_delete_insert_failure.
    managed: true
    write_strategy: delete_insert
    cursor_column: id
    load_batch_size: 1
    columns:
      - name: id
        type: INTEGER
      - name: status
        type: VARCHAR
""".strip()
                + "\n",
                "python/loaders/raw.py": """
from sqlbuild.loaders import loader

@loader
def raw_delete_insert_failure(ctx):
    '''Test loader raw_delete_insert_failure.'''
    yield {"id": 1, "status": "new"}
    yield {"id": "two", "status": "bad"}
""",
            },
            staging_table_name="raw_delete_insert_failure__staging",
            expected_staging_exists=False,
            setup_sql=(
                "CREATE TABLE raw_delete_insert_failure (id INTEGER, status VARCHAR)",
                "INSERT INTO raw_delete_insert_failure VALUES (99, 'existing')",
            ),
            target_select_sql="SELECT id, status FROM raw_delete_insert_failure ORDER BY id",
            expected_target_rows=((99, "existing"),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_later_loader_batch_fails_when_running_load_then_drops_staging(
    test_case: LoadCommandFailureCleanupTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)
    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    source_file: DiscoveredSourceFile = discovered_inputs.source_files[0]
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        setup_statement: str
        for setup_statement in test_case.setup_sql:
            connection.execute(setup_statement)
    finally:
        connection.close()

    results: tuple[LoadExecutionResult, ...] = run_load_pipeline(
        sources=source_file.source_entries,
        loader_functions=discovered_inputs.loader_functions,
        connection_config={"database": str(tmp_path / "demo.duckdb")},
        adapter=adapter,
        runtime=LoadRuntimeParams(
            run_id="test_run",
            target="dev",
            vars={},
            is_reload=False,
        ),
    )

    connection: DuckDBPyConnection = duckdb.connect(str(tmp_path / "demo.duckdb"))
    try:
        staging_count_row: tuple[int] | None = connection.execute(
            "SELECT COUNT(*) FROM information_schema.tables "
            f"WHERE table_name = '{test_case.staging_table_name}'"
        ).fetchone()
        target_rows: list[tuple[object, ...]] = connection.execute(
            test_case.target_select_sql
        ).fetchall()
    finally:
        connection.close()
    assert staging_count_row is not None
    staging_exists: bool = bool(staging_count_row[0])
    assert results[0].status.value == "failed"
    assert staging_exists is test_case.expected_staging_exists
    assert tuple(target_rows) == test_case.expected_target_rows


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandSelectionErrorTestCase(
            description="raises when selected source does not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": _RAW_ORDERS_LOADER,
            },
            select=("missing_source",),
            exclude=(),
            expected_error_fragment=(
                "selector 'missing_source' does not match any managed source or loader"
            ),
        ),
        LoadCommandSelectionErrorTestCase(
            description="raises when selected source is unmanaged",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_customers
    description: Test source raw_customers.
    expression: SELECT 1 AS customer_id
""".strip()
                + "\n",
            },
            select=("raw_customers",),
            exclude=(),
            expected_error_fragment=(
                "selector 'raw_customers' does not match any managed source or loader"
            ),
        ),
        LoadCommandSelectionErrorTestCase(
            description="raises when excluded source does not exist",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": _RAW_ORDERS_LOADER,
            },
            select=(),
            exclude=("missing_source",),
            expected_error_fragment=(
                "selector 'missing_source' does not match any managed source or loader"
            ),
        ),
        LoadCommandSelectionErrorTestCase(
            description="raises when excluded source is unmanaged",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
  - name: raw_customers
    description: Test source raw_customers.
    expression: SELECT 1 AS customer_id
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": _RAW_ORDERS_LOADER,
            },
            select=(),
            exclude=("raw_customers",),
            expected_error_fragment=(
                "selector 'raw_customers' does not match any managed source or loader"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_load_selectors_when_running_load_then_it_raises_clear_error(
    test_case: LoadCommandSelectionErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    with pytest.raises(CliUserError) as exc_info:
        run_load_command(
            request=LoadCommandRequest(
                project_dir=tmp_path,
                no_color=True,
                select=test_case.select,
                exclude=test_case.exclude,
            )
        )

    assert test_case.expected_error_fragment in str(exc_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        LoadCommandEmptySelectionTestCase(
            description="succeeds without connecting when project has no managed sources",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_customers
    description: Test source raw_customers.
    expression: SELECT 1 AS customer_id
""".strip()
                + "\n",
            },
            select=(),
            exclude=(),
            expected_exit_code=0,
            expected_stdout_fragment="No managed sources selected.",
            expected_stdout_fragments=(
                "Load ready  0 selected",
                "\u2713 Completed successfully",
                "PASS=0  WARN=0  FAIL=0  SKIP=0  TOTAL=0",
            ),
        ),
        LoadCommandEmptySelectionTestCase(
            description="succeeds without connecting when all managed sources are excluded",
            project_files={
                "sqlbuild_project.toml": _PROJECT_FILE,
                "sources/raw.yml": """
sources:
  - name: raw_orders
    description: Test source raw_orders.
    managed: true
    write_strategy: table
""".strip()
                + "\n",
                "python/loaders/raw_orders.py": _RAW_ORDERS_LOADER,
            },
            select=(),
            exclude=("raw_orders",),
            expected_exit_code=0,
            expected_stdout_fragment="No managed sources selected.",
            expected_stdout_fragments=(
                "Load ready  0 selected",
                "\u2713 Completed successfully",
                "PASS=0  WARN=0  FAIL=0  SKIP=0  TOTAL=0",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_selected_managed_sources_when_running_load_then_it_does_not_connect(
    test_case: LoadCommandEmptySelectionTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: CaptureFixture[str],
) -> None:
    write_repo_files(tmp_path, test_case.project_files)

    exit_code: int = run_load_command(
        request=LoadCommandRequest(
            project_dir=tmp_path,
            no_color=True,
            select=test_case.select,
            exclude=test_case.exclude,
        )
    )

    captured: CaptureResult[str] = capsys.readouterr()
    assert exit_code == test_case.expected_exit_code
    assert test_case.expected_stdout_fragment in captured.out
    assert all(fragment in captured.out for fragment in test_case.expected_stdout_fragments)
    assert not (tmp_path / "demo.duckdb").exists()
