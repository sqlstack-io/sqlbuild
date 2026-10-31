"""Real compilation produces immutable, relocatable control-plane catalog data."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any
from uuid import UUID

import pytest

from sqlbuild._native import (
    NativeOrchestrationSnapshot,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    SnapshotCatalogTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    FUNCTION_PROJECT_FILES,
    PARITY_FILES,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

METADATA: dict[str, Any] = {
    "location_id": str(UUID(int=1)),
    "target": "development",
    "generation": 1,
    "git_revision": None,
    "git_dirty": False,
    "git_author": None,
    "image_id": None,
    "environment_hash": "test-environment",
    "sqb_version": "not-the-installed-version",
    "nodes": {},
    "documents": {},
}


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotCatalogTestCase(
            "model and seed catalog",
            TYPE_PROOF_FILES
            | {
                "seeds/payment_methods.csv": PARITY_FILES["seeds/payment_methods.csv"],
                "seeds/payment_methods.yml": PARITY_FILES["seeds/payment_methods.yml"],
                "models/payments.sql": (
                    "MODEL (description 'Payment methods.');\n"
                    'SELECT method FROM __seed("payment_methods")\n'
                ),
            },
            frozenset({"model", "seed"}),
        ),
        SnapshotCatalogTestCase(
            "attached audits",
            TYPE_PROOF_FILES
            | {
                "models/orders.sql": (
                    "MODEL (description 'Orders.', columns (order_id (audits [not_null])));\n"
                    'SELECT order_id, amount FROM __ref("stg_orders")\n'
                ),
            },
            frozenset({"model", "audit"}),
        ),
        SnapshotCatalogTestCase(
            "SQL and Python functions",
            {
                path: value
                for path, value in FUNCTION_PROJECT_FILES.items()
                if path != "enums/status.sql"
            }
            | {"functions/sql/_enums/status.sql": FUNCTION_PROJECT_FILES["enums/status.sql"]},
            frozenset({"model", "udf", "table_fn"}),
        ),
        SnapshotCatalogTestCase(
            "Python hooks",
            TYPE_PROOF_FILES
            | {
                "hooks/python/notifications.py": (
                    "from sqlbuild.hooks import hook\n\n"
                    "@hook\ndef notify(ctx, channel):\n"
                    '    """Notify support."""\n    return None\n'
                ),
                "models/orders.sql": (
                    "MODEL (description 'Orders.', pre_hooks [python('notify', channel: 'support')]);\n"
                    'SELECT order_id, amount FROM __ref("stg_orders")\n'
                ),
            },
            frozenset({"model", "hook"}),
        ),
        SnapshotCatalogTestCase(
            "SQL hooks",
            TYPE_PROOF_FILES
            | {
                "hooks/sql/record_load.sql": "HOOK (description 'Record load.');\nSELECT 3 AS load_count\n",
                "models/orders.sql": (
                    "MODEL (description 'Orders.', pre_hooks [sql('record_load')]);\n"
                    'SELECT order_id FROM __ref("stg_orders")\n'
                ),
            },
            frozenset({"model", "sql_hook"}),
        ),
        SnapshotCatalogTestCase(
            "source catalog",
            TYPE_PROOF_FILES
            | {
                "sources/orders.yml": (
                    "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
                    "    schema: raw\n    table: orders\n"
                    "    columns:\n      - name: order_id\n        type: INTEGER\n"
                ),
            },
            frozenset({"model", "source"}),
        ),
        SnapshotCatalogTestCase(
            "DuckShop inherited seed and function namespaces",
            TYPE_PROOF_FILES
            | {
                path: (Path("benchmarks/duckshop/project") / path).read_text()
                for path in (
                    "seeds/edge_cases/edge_quantity_steps.csv",
                    "seeds/edge_cases/edge_quantity_steps.yml",
                    "functions/sql/edge_cases/edge_next_quantity.sql",
                )
            },
            frozenset({"model", "seed", "udf"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_real_projects_when_building_snapshots_then_catalogs_are_stable_and_relocatable(
    test_case: SnapshotCatalogTestCase,
    tmp_path: Path,
) -> None:
    publications: list[dict[str, Any]] = []
    metadata: str = json.dumps(METADATA)
    for directory in (tmp_path / "original", tmp_path / "relocated"):
        write_files(project_dir=directory, files=test_case.files)
        project: CompiledProjectView = build_compiled_project(
            discovered_inputs=discover_project_inputs(project_dir=directory),
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        assert not [item for item in project.diagnostics if item.severity == "error"]
        snapshot: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, metadata, directory
        )
        publication: dict[str, Any] = json.loads(snapshot.to_json())
        publications.append(publication)
        assert publication["sqb_version"] != "not-the-installed-version"
        assert publication["environment_hash"] != "test-environment"
        assert test_case.expected_kinds <= {node["kind"] for node in publication["nodes"].values()}
        assert str(directory) not in snapshot.to_json()
        for model in project.models:
            node: dict[str, Any] = publication["nodes"][f"model:{model.name}"]
            assert node["compiled_sql"] == model.query_sql
            assert (
                node["definition"]["destination"]["qualified_name"]
                == model.destination.qualified_name
            )
            assert set(node["definition"]["deps"]) == {
                f"{dependency.resource_type}:{dependency.name}" for dependency in model.deps
            }
        for function in project.functions:
            assert (
                publication["nodes"][f"{function.key.resource_type}:{function.name}"]["definition"][
                    "destination"
                ]["qualified_name"]
                == function.destination.qualified_name
            )
        for seed in project.seeds:
            assert (
                publication["nodes"][f"seed:{seed.name}"]["definition"]["destination"][
                    "qualified_name"
                ]
                == seed.destination.qualified_name
            )
    assert publications[0] == publications[1]


def test_given_invalid_compilation_or_forged_nodes_when_publishing_then_no_snapshot_is_emitted(
    tmp_path: Path,
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    with pytest.raises(ValueError, match="nodes must come from the compiled project"):
        build_orchestration_snapshot(
            project,
            json.dumps(
                METADATA
                | {
                    "nodes": {
                        "model:forged": {
                            "kind": "model",
                            "definition": {},
                            "raw_sql": None,
                            "compiled_sql": "select 2",
                            "python_source": None,
                        }
                    }
                }
            ),
            tmp_path,
        )
    (tmp_path / "models/orders.sql").write_text("MODEL ();\nSELECT 1 AS order_id\n")
    broken: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    assert any(item.severity == "error" for item in broken.diagnostics)
    with pytest.raises(ValueError, match="cannot publish a project with compiler errors"):
        build_orchestration_snapshot(broken, json.dumps(METADATA), tmp_path)


def test_given_a_dependency_lock_when_it_changes_then_environment_identity_changes_not_node_content(
    tmp_path: Path,
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    installed: dict[str, Any] = json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
    (tmp_path / "uv.lock").write_text("version = 1\n# dependency revision one\n")
    locked: dict[str, Any] = json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
    (tmp_path / "uv.lock").write_text("version = 1\n# dependency revision two\n")
    changed: dict[str, Any] = json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
    assert (
        len(
            {installed["environment_hash"], locked["environment_hash"], changed["environment_hash"]}
        )
        == 3
    )
    assert installed["nodes"] == locked["nodes"] == changed["nodes"]
    assert changed == json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
