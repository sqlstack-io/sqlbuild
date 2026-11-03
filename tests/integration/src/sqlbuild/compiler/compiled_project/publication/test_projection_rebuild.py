"""Installed repair commits every projection or none on both supported stores."""

import json
import shutil
import subprocess
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    NativeOrchestrationSnapshot,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    METADATA,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ProjectionReplayTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.replay.helpers import (
    postgres_store,
    repair,
    sqlite_store,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    administer,
    serve,
    successful,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.helpers import (
    logical_table_state,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        ProjectionReplayTestCase("SQLite installed atomic repair", sqlite_store, 4),
        ProjectionReplayTestCase("PostgreSQL installed atomic repair", postgres_store, 4),
    ],
    ids=lambda case: case.description,
)
def test_given_damaged_projections_when_the_installed_command_repairs_then_sources_and_failure_atomicity_hold(
    test_case: ProjectionReplayTestCase,
    tmp_path: Path,
) -> None:
    binary: str | None = shutil.which("sqb")
    assert binary is not None
    installation: tuple[Path, Path] = Path(binary), tmp_path
    project_dir: Path = tmp_path / "project"
    files: dict[str, str] = TYPE_PROOF_FILES | {
        "orchestration/edge_cases/definitions.py": Path(
            "benchmarks/duckshop/project/orchestration/edge_cases/definitions.py"
        ).read_text(),
        "models/orders.sql": TYPE_PROOF_FILES["models/orders.sql"].replace(
            "MODEL (", "MODEL (tags ['edge_case'],", 1
        ),
    }
    write_files(project_dir=project_dir, files=files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    assert not any(item.severity == "error" for item in project.diagnostics)
    with test_case.open_store(installation) as store:
        location: str = successful(
            administer(
                installation, ["create-location", "orders"], extra_environment=store.environment
            )
        )["location_id"]
        token: dict[str, Any] = successful(
            administer(
                installation,
                [
                    "issue-location-token",
                    location,
                    "--expires-at",
                    (datetime.now(UTC) + timedelta(hours=1)).isoformat(),
                ],
                extra_environment=store.environment,
            )
        )
        with serve(installation, extra_environment=store.environment) as url:
            snapshot: NativeOrchestrationSnapshot = build_orchestration_snapshot(
                project,
                json.dumps(METADATA | {"location_id": location, "target": "development"}),
                project_dir,
            )
            _ = snapshot.publish(url, token["bearer"])
        connection: Any = store.connection
        tables: list[str] = [row[0] for row in connection.execute(store.table_query).fetchall()]
        progress_count: Any = connection.execute(
            "SELECT COUNT(*) FROM sqb_trigger_progress"
        ).fetchone()
        assert progress_count[0] == test_case.expected_progress_facts
        original: dict[str, str] = logical_table_state(connection, tables)
        damage: dict[str, Any] = json.loads(
            Path("benchmarks/duckshop/edge_cases/orchestration/projection_replay.json").read_text()
        )
        for statement in damage["snapshot_damage"] + damage["trigger_damage"]:
            _ = connection.execute(
                statement.replace("job_key='orders'", "job_key='edge_snapshot_orders'")
            )
        connection.commit()
        for _ in range(2):
            output: subprocess.CompletedProcess[str] = repair(installation, store.environment)
            assert successful(output) == {"rebuilt": True}
            assert "Rebuilding control projections" in output.stderr
            assert "rebuilt successfully" in output.stderr
            assert logical_table_state(connection, tables) == original
        _ = connection.execute(
            "UPDATE sqb_schema_migrations SET min_server_version='9999.0.0' WHERE name='trigger_progress'"
        )
        connection.commit()
        incompatible: dict[str, str] = logical_table_state(connection, tables)
        output = repair(installation, store.environment)
        assert output.returncode != 0 and not output.stdout
        assert "minimum server version not met" in output.stderr
        assert logical_table_state(connection, tables) == incompatible
        _ = connection.execute(
            "UPDATE sqb_schema_migrations SET min_server_version='0.0.0' WHERE name='trigger_progress'"
        )
        _ = connection.execute("DELETE FROM sqb_trigger_progress")
        _ = connection.execute("DELETE FROM sqb_current_snapshots")
        connection.commit()
        legacy: dict[str, str] = logical_table_state(connection, tables)
        output = repair(installation, store.environment)
        assert output.returncode != 0 and not output.stdout
        assert "lacks semantic progress" in output.stderr
        assert "failed" in output.stderr
        assert logical_table_state(connection, tables) == legacy
