"""Real adapter catalog inspection and automatic standalone location synchronization."""

import hashlib
import json
import os
import sqlite3
import subprocess
import sys
import time
from collections import Counter
from contextlib import closing
from pathlib import Path
from typing import Any

import pytest
from testcontainers.postgres import PostgresContainer

from scripts.native_tests.constants import POSTGRES_IMAGE
from sqlbuild.adapters.postgres.classes.postgres_adapter import PostgresAdapter
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CatalogBoundaryTestCase,
    CatalogInspectionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    catalog_facts,
    wait_for_catalog,
    wait_for_statistics,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    location_process,
    open_legacy_warehouse,
    snapshot_api,
    wait_for_generation,
    warehouse_history_page,
    warehouse_history_pages,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        CatalogInspectionTestCase("DuckDB catalog", "duckdb", "database", "BASE TABLE", "VIEW"),
        CatalogInspectionTestCase("SQLite catalog", "sqlite", "path", "BASE TABLE", "VIEW"),
    ],
    ids=lambda case: case.description,
)
def test_given_real_catalog_when_reading_pages_then_columns_and_physical_types_are_read_without_mutation(
    test_case: CatalogInspectionTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json").read_text()
    )
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    _ = adapter.execute(connection=connection, sql=fixture["create_sql"])
    for index in range(fixture["extra_relations"]):
        _ = adapter.execute(
            connection=connection, sql=f"CREATE TABLE catalog_orders_{index:03} (amount INTEGER)"
        )
    adapter.close(connection)
    before: str = hashlib.sha256((project / "warehouse.db").read_bytes()).hexdigest()
    pages: list[dict[str, Any]] = warehouse_history_pages(project, "catalog")
    assert tuple(len(page["records"]) for page in pages) == (100, 7)
    records: list[dict[str, Any]] = []
    for page in pages:
        records.extend(page["records"])
    assert [record["name"] for record in records] == sorted(record["name"] for record in records)
    by_name: dict[str, Any] = {record["name"]: record for record in records}
    expected: dict[str, Any] = fixture["record"] | {"relation_type": test_case.expected_table_type}
    assert by_name[expected["name"]] == expected
    assert by_name[expected["name"]]["relation_type"] == test_case.expected_table_type
    assert hashlib.sha256((project / "warehouse.db").read_bytes()).hexdigest() == before
    assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [CatalogBoundaryTestCase("Case-distinct physical identities", True)],
    ids=lambda case: case.description,
)
def test_given_case_distinct_postgres_relations_when_reading_catalog_then_columns_remain_exact(
    test_case: CatalogBoundaryTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json").read_text()
    )
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "postgres"\n'
                '[connections.local]\nconninfo = "${ENV:SQLBUILD_CATALOG_TEST_URL}"\n'
                '[targets.development]\nschema = "main"\n'
            ),
            "orchestration/orders.py": 'raise AssertionError("unexpected declaration import")\n',
        },
    )
    with PostgresContainer(POSTGRES_IMAGE, driver=None) as postgres:
        url: str = postgres.get_connection_url()
        monkeypatch.setenv("SQLBUILD_CATALOG_TEST_URL", url)
        adapter: PostgresAdapter = PostgresAdapter()
        connection: Any = adapter.connect({"conninfo": url})
        for sql in fixture["postgres_sql"]:
            _ = adapter.execute(connection=connection, sql=sql)
        adapter.close(connection)
        page: Any = warehouse_history_page(project, "catalog")
        assert (page is not None) is test_case.expected_page_delivered
        assert page == {"records": fixture["postgres_records"], "next_after": None}


@pytest.mark.parametrize(
    "test_case",
    [CatalogBoundaryTestCase("Non-authoritative schema-only namespace", False)],
    ids=lambda case: case.description,
)
def test_given_non_authoritative_adapter_scope_when_reading_catalog_then_no_page_can_authorize_removal(
    test_case: CatalogBoundaryTestCase,
    tmp_path: Path,
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json").read_text()
    )
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(project, "sqlite", "path")
    adapter.close(connection)
    # Exercise Databricks' real schema-only capability with a local read-only connection.
    result: subprocess.CompletedProcess[str] = subprocess.run(
        [
            sys.executable,
            "-c",
            "from pathlib import Path\n"
            "import sys\n"
            "from sqlbuild._native import run_location_command\n"
            "from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter\n"
            "from sqlbuild.adapters.databricks.classes.databricks_adapter import DatabricksAdapter\n"
            "SqliteAdapter.lists_relations_authoritatively = DatabricksAdapter.lists_relations_authoritatively\n"
            "raise SystemExit(run_location_command(project_dir=Path(sys.argv[1]), "
            "location_id='00000000-0000-0000-0000-000000000001', selected_target='development', "
            "generation=1, server_url='http://127.0.0.1:1', action='runtime'))\n",
            str(project),
        ],
        input=json.dumps(
            {
                "catalog": {
                    "location": {
                        "database": fixture["non_authoritative_database"],
                        "schema": "main",
                    }
                }
            }
        )
        + "\n",
        text=True,
        capture_output=True,
        timeout=20,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert (json.loads(result.stdout) is not None) is test_case.expected_page_delivered
    assert "No page delivered" in result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        CatalogInspectionTestCase(
            "DuckDB automatic catalog", "duckdb", "database", "BASE TABLE", "VIEW", 0
        ),
        CatalogInspectionTestCase(
            "SQLite automatic catalog", "sqlite", "path", "BASE TABLE", "VIEW"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_served_location_when_catalog_changes_then_facts_and_current_projection_follow_the_warehouse(
    test_case: CatalogInspectionTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/warehouse_catalog.json").read_text()
    )
    project: Path = tmp_path / "project"
    adapter: Any
    connection: Any
    adapter, connection = open_legacy_warehouse(
        project, test_case.adapter_name, test_case.connection_key
    )
    _ = adapter.execute(connection=connection, sql=fixture["create_sql"])
    adapter.close(connection)
    (project / "orchestration/orders.py").write_text("")
    write_files(project_dir=project, files={"models/orders.sql": fixture["model"]})
    expected: dict[str, Any] = fixture["record"] | {"relation_type": test_case.expected_table_type}
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"
    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        wait_for_catalog(control, [expected], location, log)
        samples: list[dict[str, Any]] = wait_for_statistics(
            control, expected["name"], location, log
        )
        assert len(samples) == 1
        assert samples[0]["name"] == expected["name"]
        assert samples[0]["byte_count"] is None
        assert samples[0]["byte_count_basis"] is None
        assert samples[0]["row_count"] == test_case.expected_metadata_rows
        assert [fact["present"] for fact in catalog_facts(control)] == [True]
        time.sleep(6)
        assert [fact["present"] for fact in catalog_facts(control)] == [True]
        connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
        _ = adapter.execute(connection=connection, sql=fixture["alter_sql"])
        adapter.close(connection)
        expected = expected | {
            "columns": [*expected["columns"], {"name": "shipped", "data_type": "INTEGER"}]
        }
        wait_for_catalog(control, [expected], location, log)
        connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
        for statement in fixture["replace_sql"]:
            _ = adapter.execute(connection=connection, sql=statement)
        adapter.close(connection)
        expected = expected | {
            "relation_type": test_case.expected_view_type,
            "columns": [{"name": "order_id", "data_type": "INTEGER"}],
        }
        wait_for_catalog(control, [expected], location, log)
        assert catalog_facts(control)[-1]["record"]["relation_type"] == test_case.expected_view_type
        connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
        _ = adapter.execute(connection=connection, sql=fixture["drop_sql"])
        adapter.close(connection)
        wait_for_catalog(control, [], location, log)
        facts: list[dict[str, Any]] = catalog_facts(control)
        assert [fact["present"] for fact in facts] == [True, True, True, False]
        assert facts[-1]["record"] == expected
        connection = adapter.connect({test_case.connection_key: str(project / "warehouse.db")})
        _ = adapter.execute(connection=connection, sql=fixture["create_sql"])
        adapter.close(connection)
        expected = fixture["record"] | {"relation_type": test_case.expected_table_type}
        wait_for_catalog(control, [expected], location, log)
        with closing(sqlite3.connect(control)) as database:
            _ = database.execute("DELETE FROM sqb_warehouse_catalog")
            database.commit()
        wait_for_catalog(control, [expected], location, log)
        assert catalog_facts(control)[-1]["record"] == expected
        current: list[dict[str, Any]] = wait_for_statistics(
            control, expected["name"], location, log
        )
        assert samples[0] in current
        assert (
            Counter(record["observed_at"][:13] for record in current)[
                samples[0]["observed_at"][:13]
            ]
            == 1
        )
