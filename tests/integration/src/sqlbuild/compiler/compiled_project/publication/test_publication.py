"""Real compile, native transport, authenticated server and durable publication."""

import json
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
    SnapshotPublicationTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    snapshot_api,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotPublicationTestCase(
            description="ordinary target",
            target="development",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
        SnapshotPublicationTestCase(
            description="target requiring URL path encoding",
            target="preview #1",
            expected_generation=2,
            expected_nodes=frozenset({"model:orders", "model:stg_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_retained_compiler_data_when_publishing_then_http_updates_atomically_and_rejections_keep_last_good(
    test_case: SnapshotPublicationTestCase, tmp_path: Path
) -> None:
    project_dir: Path = tmp_path / "project"
    write_files(project_dir=project_dir, files=TYPE_PROOF_FILES)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=project_dir),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    assert not any(item.severity == "error" for item in project.diagnostics)
    with snapshot_api(tmp_path / "control") as api:
        metadata: dict[str, Any] = METADATA | {
            "location_id": api.location_id,
            "target": test_case.target,
        }
        first: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, json.dumps(metadata), project_dir
        )
        record: dict[str, Any] = json.loads(first.publish(api.url, api.bearer))
        assert record["generation"] == 1
        assert record["location_id"] == api.location_id
        initial: dict[str, Any] = current_snapshot(api, test_case.target)
        assert frozenset(initial["nodes"]) == test_case.expected_nodes
        assert initial["nodes"]["model:orders"]["definition"]["deps"] == ["model:stg_orders"]
        edited: dict[str, str] = TYPE_PROOF_FILES | {
            "models/stg_orders.sql": TYPE_PROOF_FILES["models/stg_orders.sql"].replace(
                "CAST(5 AS INTEGER)", "CAST(9 AS INTEGER)"
            )
        }
        write_files(project_dir=project_dir, files=edited)
        changed: CompiledProjectView = build_compiled_project(
            discovered_inputs=discover_project_inputs(project_dir=project_dir),
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        next_snapshot: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            changed, json.dumps(metadata | {"generation": 2}), project_dir
        )
        updated: dict[str, Any] = json.loads(next_snapshot.publish(api.url, api.bearer))
        assert updated["generation"] == test_case.expected_generation
        assert updated["snapshot_id"] != record["snapshot_id"]
        with pytest.raises(RuntimeError, match="HTTP 409"):
            _ = first.publish(api.url, api.bearer)
        with pytest.raises(RuntimeError, match="HTTP 401"):
            _ = next_snapshot.publish(api.url, "invalid-credential")
        current: dict[str, Any] = current_snapshot(api, test_case.target)
        assert current["record"] == updated
        assert "CAST(9 AS INTEGER)" in current["nodes"]["model:stg_orders"]["raw_sql"]
