"""Real compilation produces immutable, relocatable control-plane catalog data."""

from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    NativeOrchestrationSnapshot,
    PythonNodeDiscoveryError,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.orchestration import (
    Job,
    RunEventKind,
    compile_snapshot,
    job,
    job_group,
    run_event,
    schedule,
    sensor,
)
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    SnapshotCatalogTestCase,
    SnapshotDeclarationRejectionTestCase,
    SnapshotDiscoveryRejectionTestCase,
    SnapshotDiscoveryTestCase,
    SnapshotEnvironmentTestCase,
    SnapshotIdentityTestCase,
    SnapshotRejectionTestCase,
    SnapshotScheduleTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    FUNCTION_PROJECT_FILES,
    METADATA,
    PARITY_FILES,
    TYPE_PROOF_FILES,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


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
            "DuckShop Python execution inputs",
            TYPE_PROOF_FILES
            | {
                f"python/{kind}s/orders.py": json.loads(
                    Path(
                        "benchmarks/duckshop/edge_cases/orchestration/python_execution_inputs.json"
                    ).read_text()
                )[source]
                for kind, source in (("task", "task"), ("asset", "asset"), ("check", "initial"))
            },
            frozenset({"model", "task", "asset", "check"}),
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
            "DuckShop polling callback with nested code is relocatable",
            TYPE_PROOF_FILES
            | {
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/custom_orders.py"
                ).read_text(),
            },
            frozenset({"model"}),
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
        assert not any(item.severity == "error" for item in project.diagnostics)
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


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotIdentityTestCase(**case)
        for case in json.loads(
            Path("benchmarks/duckshop/edge_cases/orchestration/semantic_identity.json").read_text()
        )["cases"]
    ],
    ids=lambda case: case.description,
)
def test_given_semantic_edits_when_publishing_then_local_identities_keep_planner_boundaries(
    test_case: SnapshotIdentityTestCase, tmp_path: Path
) -> None:
    fixture: dict[str, Any] = json.loads(
        Path("benchmarks/duckshop/edge_cases/orchestration/semantic_identity.json").read_text()
    )
    files: dict[str, str] = (
        TYPE_PROOF_FILES
        | fixture["files"]
        | {
            path: (Path("benchmarks/duckshop/project") / path).read_text()
            for path in (
                "seeds/edge_cases/edge_quantity_steps.csv",
                "seeds/edge_cases/edge_quantity_steps.yml",
                "functions/sql/edge_cases/edge_next_quantity.sql",
            )
        }
    )
    original: str = files[test_case.path]
    assert test_case.before in original
    changed: dict[str, str] = files | {
        test_case.path: original.replace(test_case.before, test_case.after)
    }
    identities: list[dict[str, str]] = []
    for label, inputs in (("original", files), ("changed", changed)):
        directory: Path = tmp_path / label
        write_files(project_dir=directory, files=inputs)
        project: CompiledProjectView = build_compiled_project(
            discovered_inputs=discover_project_inputs(project_dir=directory),
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        assert not any(item.severity == "error" for item in project.diagnostics)
        snapshot: dict[str, Any] = json.loads(
            build_orchestration_snapshot(project, json.dumps(METADATA), directory).to_json()
        )
        hashes: dict[str, str] = {}
        for key in (
            "model:orders",
            "model:stg_orders",
            "udf:edge_next_quantity",
            "seed:edge_quantity_steps",
        ):
            identity: dict[str, str] = snapshot["nodes"][key]["definition"]["planner_identity"]
            assert set(identity) == {"local_hash"}
            assert len(identity["local_hash"]) == 64
            hashes[key] = identity["local_hash"]
        identities.append(hashes)
    assert {key: identities[0][key] != identities[1][key] for key in identities[0]} == {
        key: key in test_case.expected_changed for key in identities[0]
    }


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotRejectionTestCase(
            description="a valid compile cannot publish caller-forged nodes",
            files=TYPE_PROOF_FILES,
            nodes={
                "model:forged": {
                    "kind": "model",
                    "definition": {},
                    "raw_sql": None,
                    "compiled_sql": "select 2",
                    "python_source": None,
                }
            },
            expected_compiler_errors=False,
            expected_message="nodes must come from the compiled project",
        ),
        SnapshotRejectionTestCase(
            description="compiler errors cannot replace the last good publication",
            files=TYPE_PROOF_FILES | {"models/orders.sql": "MODEL ();\nSELECT 1 AS order_id\n"},
            nodes={},
            expected_compiler_errors=True,
            expected_message="cannot publish a project with compiler errors",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_compilation_or_forged_nodes_when_publishing_then_no_snapshot_is_emitted(
    test_case: SnapshotRejectionTestCase,
    tmp_path: Path,
) -> None:
    write_files(project_dir=tmp_path, files=test_case.files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    assert (
        any(item.severity == "error" for item in project.diagnostics)
        == test_case.expected_compiler_errors
    )
    with pytest.raises(ValueError, match=test_case.expected_message):
        build_orchestration_snapshot(
            project,
            json.dumps(METADATA | {"nodes": test_case.nodes}),
            tmp_path,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotEnvironmentTestCase(
            description="installed dependencies and two lock revisions identify different environments",
            first_lockfile="version = 1\n# dependency revision one\n",
            second_lockfile="version = 1\n# dependency revision two\n",
            expected_environment_count=3,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_dependency_lock_when_it_changes_then_environment_identity_changes_not_node_content(
    test_case: SnapshotEnvironmentTestCase,
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
    (tmp_path / "uv.lock").write_text(test_case.first_lockfile)
    locked: dict[str, Any] = json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
    (tmp_path / "uv.lock").write_text(test_case.second_lockfile)
    changed: dict[str, Any] = json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )
    assert (
        len(
            {installed["environment_hash"], locked["environment_hash"], changed["environment_hash"]}
        )
        == test_case.expected_environment_count
    )
    assert installed["nodes"] == locked["nodes"] == changed["nodes"]
    assert changed == json.loads(
        build_orchestration_snapshot(project, json.dumps(METADATA), tmp_path).to_json()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotScheduleTestCase(
            description="daylight saving zone",
            schedule=schedule(cron="15 2 * * *", timezone="Europe/London"),
            expected_selection=("model:orders",),
        ),
        SnapshotScheduleTestCase(
            description="fixed offset zone",
            schedule=schedule(cron="15 2 * * *", timezone="UTC"),
            expected_selection=("model:orders",),
        ),
        SnapshotScheduleTestCase(
            description="paused bounded catch-up with a zoned start",
            schedule=schedule(
                cron="15 2 * * *",
                timezone="Europe/London",
                starts_at="2026-07-03T01:17:23.456789+05:30",
                default_status="paused",
                catch_up="all",
                max_catch_up=3,
            ),
            expected_selection=("model:orders",),
            expected_controls={
                "starts_at": "2026-07-02T19:47:23.456789Z",
                "default_status": "paused",
                "catch_up": {"all": {"max_runs": 3}},
            },
        ),
        SnapshotScheduleTestCase(
            description="largest supported catch-up batch",
            schedule=schedule(
                cron="15 2 * * *", timezone="UTC", catch_up="all", max_catch_up=10_000
            ),
            expected_selection=("model:orders",),
            expected_controls={
                "starts_at": None,
                "default_status": "running",
                "catch_up": {"all": {"max_runs": 10_000}},
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_jobs_when_compiling_then_native_snapshots_resolve_selection_and_cron(
    tmp_path: Path, test_case: SnapshotScheduleTestCase
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    nightly: Job = job(
        "nightly_orders",
        description="Refresh order reporting.",
        select="+orders",
        exclude="stg_orders",
        overlap="queue",
        settings={"run_tests": False, "vars": {"batch": 3}, "warehouse": "reporting"},
        concurrency=4,
        concurrency_groups=("order_reporting",),
        schedule=test_case.schedule,
        targets=("development",),
    )
    excluded: Job = replace(nightly, name="other_target", targets=("production",))
    first: dict[str, Any] = json.loads(
        compile_snapshot(
            project=project,
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            declarations=[nightly, excluded],
        ).to_json()
    )
    assert first["documents"]["jobs"] == {
        "nightly_orders": {
            "description": "Refresh order reporting.",
            "selection": list(test_case.expected_selection),
            "settings": {
                "concurrency": 4,
                "run_tests": False,
                "vars": {"batch": 3},
                "warehouse": "reporting",
            },
            "overlap": "queue",
            "on_failure": "skip_dependents",
            "concurrency_groups": ["order_reporting"],
        }
    }
    trigger: dict[str, Any] = first["documents"]["triggers"]["nightly_orders"]
    assert trigger["job_key"] == "nightly_orders"
    assert trigger["schedule"] == {
        "kind": "cron",
        "expression": "15 2 * * *",
        "timezone": test_case.schedule.timezone,
    }
    assert {key: trigger[key] for key in test_case.expected_controls} == test_case.expected_controls
    edited: Job = replace(nightly, schedule=schedule(cron="30 3 * * *", timezone="UTC"))
    second: dict[str, Any] = json.loads(
        compile_snapshot(
            project=project,
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            declarations=[edited],
        ).to_json()
    )
    assert second["documents"]["triggers"].keys() == first["documents"]["triggers"].keys()
    assert second["documents"]["triggers"]["nightly_orders"]["schedule"]["expression"] == (
        "30 3 * * *"
    )
    assert second["nodes"] == first["nodes"]


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDeclarationRejectionTestCase(
            description="blank description",
            job_description=" ",
            expected_message="needs a description",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="blank selection", select="", expected_message="needs a selection"
        ),
        SnapshotDeclarationRejectionTestCase(
            description="missing model", select="missing", expected_message="missing"
        ),
        SnapshotDeclarationRejectionTestCase(
            description="invalid cron", cron="not a cron", expected_message="cron"
        ),
        SnapshotDeclarationRejectionTestCase(
            description="invalid timezone", timezone="Unknown/Zone", expected_message="time zone"
        ),
        SnapshotDeclarationRejectionTestCase(
            description="zero catch-up bound",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", catch_up="all", max_catch_up=0),
            expected_message="catch_up",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="negative catch-up bound",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", catch_up="all", max_catch_up=-1),
            expected_message="catch_up",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="boolean is not a catch-up bound",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", catch_up="all", max_catch_up=True),
            expected_message="catch_up",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="unbounded all catch-up",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", catch_up="all"),
            expected_message="catch_up",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="catch-up exceeds transaction bound",
            schedule=schedule(
                cron="0 2 * * *", timezone="UTC", catch_up="all", max_catch_up=10_001
            ),
            expected_message="catch-up exceeds 10000",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="latest does not silently ignore a bound",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", max_catch_up=3),
            expected_message="latest.*without a bound",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="start needs an explicit offset",
            schedule=schedule(cron="0 2 * * *", timezone="UTC", starts_at="2026-07-03T01:17:23"),
            expected_message="starts_at",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="duplicate names", copies=2, expected_message="duplicate job orders"
        ),
        SnapshotDeclarationRejectionTestCase(
            description="unknown setting",
            settings={"concurreny": 4},
            expected_message="unknown build setting concurreny",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="selection cannot escape its pinned run",
            settings={"select": "customers"},
            expected_message="owned by the run request",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="zero concurrency",
            concurrency=0,
            expected_message="must be positive",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="boolean is not concurrency",
            concurrency=True,
            expected_message="concurrency must be an integer",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="integer is not a boolean flag",
            settings={"full_refresh": 1},
            expected_message="must be boolean",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="integer strings are not typed settings",
            settings={"concurrency": "3"},
            expected_message="must be an integer",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="variables need an object",
            settings={"vars": ["orders"]},
            expected_message="must be a JSON object",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="mutually exclusive load settings",
            settings={"load_sources": False, "reload": True},
            expected_message="mutually exclusive",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="conflicting cursor families",
            settings={"start_cursor": "2", "end_cursor_int": "4"},
            expected_message="cannot be combined",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="duplicate concurrency declarations",
            concurrency=3,
            settings={"concurrency": 4},
            expected_message="declared twice",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_declarations_when_compiling_then_native_validation_rejects_them(
    tmp_path: Path, test_case: SnapshotDeclarationRejectionTestCase
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    declarations: list[Job] = [
        job(
            "orders",
            description=test_case.job_description,
            select=test_case.select,
            settings=test_case.settings,
            concurrency=test_case.concurrency,
            schedule=test_case.schedule
            or schedule(cron=test_case.cron, timezone=test_case.timezone),
        )
    ] * test_case.copies
    with pytest.raises(ValueError, match=test_case.expected_message):
        compile_snapshot(
            project=project,
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            declarations=declarations,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDiscoveryTestCase(
            description="relative imports aliases and factory containers",
            files={
                "orchestration/shared.py": (
                    "from sqlbuild.orchestration import job\n"
                    "staging = job('staging', description='Stage orders.', select='stg_orders')\n"
                ),
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job\n"
                    "from .shared import staging\n"
                    "alias = staging\n"
                    "def factory():\n"
                    "    return [job('daily', description='Daily orders.', select='orders')]\n"
                    "jobs = [factory(), staging]\n"
                    "jobs.append(jobs)\n"
                ),
            },
            expected_jobs=("daily", "staging"),
        ),
        SnapshotDiscoveryTestCase(
            description="group-only references retain lineage and explicit waits",
            files={
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job, job_group\n"
                    "def factory():\n"
                    "    staging = job('staging', description='Stage orders.', select='stg_orders')\n"
                    "    marts = job('marts', description='Build orders.', select='orders')\n"
                    "    inventory = job('inventory', description='Repeat staging.', select='stg_orders')\n"
                    "    return job_group('daily', description='Ordered jobs.', "
                    "jobs=[marts, inventory, staging], after={inventory: [staging]})\n"
                    "daily = factory()\n"
                ),
            },
            expected_jobs=("inventory", "marts", "staging"),
            expected_groups={
                "daily": {
                    "inventory": ["staging"],
                    "marts": ["inventory", "staging"],
                    "staging": [],
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="explicit empty declarations do not execute project files",
            files={"orchestration/jobs.py": "raise RuntimeError('must not import')\n"},
            expected_jobs=(),
            declarations=(),
        ),
        SnapshotDiscoveryTestCase(
            description="DuckShop edge-case declaration containers",
            files={
                "orchestration/edge_cases/definitions.py": Path(
                    "benchmarks/duckshop/project/orchestration/edge_cases/definitions.py"
                ).read_text(),
                "models/orders.sql": TYPE_PROOF_FILES["models/orders.sql"].replace(
                    "MODEL (", "MODEL (tags ['edge_case'],", 1
                ),
            },
            expected_jobs=(
                "edge_snapshot_inventory",
                "edge_snapshot_orders",
                "edge_snapshot_products",
            ),
            expected_groups={
                "edge_snapshot_group": {"edge_snapshot_orders": []},
                "edge_inventory_group": {"edge_snapshot_inventory": []},
                "edge_products_group": {"edge_snapshot_products": []},
            },
            expected_sensors={
                "edge_platform_orders": {
                    "description": "Catch up retained platform observations with a stopped group sensor.",
                    "job_key": "edge_snapshot_group",
                    "when": {
                        "kind": "platform_event",
                        "kinds": ["tick_overdue", "location_broken", "launch_failed"],
                    },
                    "minimum_interval_seconds": 7,
                    "default_status": "stopped",
                },
                "edge_job_outcomes": {
                    "description": "Keep warning and partial outcomes distinct from success.",
                    "job_key": "edge_snapshot_inventory",
                    "when": {
                        "kind": "run_event",
                        "job_key": "edge_snapshot_orders",
                        "kinds": ["warn", "partial"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "stopped",
                },
                "edge_group_outcomes": {
                    "description": "Observe group completion including cancellation and presumed failure.",
                    "job_key": "edge_snapshot_inventory",
                    "when": {
                        "kind": "run_event",
                        "job_key": "edge_snapshot_group",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "stopped",
                },
            },
            expected_schedules=("edge_snapshot_orders",),
        ),
        SnapshotDiscoveryTestCase(
            description="sensor-only roots collect source groups and destination jobs natively",
            files={},
            declarations=(
                sensor(
                    "on_orders",
                    description="Build inventory after group completion.",
                    when=run_event(
                        job=job_group(
                            "source_group",
                            description="Source orders.",
                            jobs=[job("source_orders", description="Orders.", select="orders")],
                        ),
                        kinds=[RunEventKind.COMPLETED],
                        executed_successfully=("orders", "orders*"),
                    ),
                    run=job("inventory", description="Inventory.", select="stg_orders"),
                ),
            ),
            expected_jobs=("inventory", "source_orders"),
            expected_groups={"source_group": {"source_orders": []}},
            expected_sensors={
                "on_orders": {
                    "description": "Build inventory after group completion.",
                    "job_key": "inventory",
                    "when": {
                        "kind": "run_event",
                        "job_key": "source_group",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="any-run node selectors freeze canonical keys without a source job reference",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/node_orders.py"
                ).read_text()
            },
            expected_jobs=("inventory", "source_orders"),
            expected_sensors={
                "on_orders": {
                    "description": "Build inventory for every genuine orders rebuild.",
                    "job_key": "inventory",
                    "when": {
                        "kind": "node_event",
                        "selection": ["model:orders"],
                        "kinds": ["success"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="failure and recovery selectors preserve their individual transition kinds",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/node_orders.py"
                )
                .read_text()
                .replace("NodeEventKind.SUCCESS", "NodeEventKind.FAILED, NodeEventKind.RECOVERED")
            },
            expected_jobs=("inventory", "source_orders"),
            expected_sensors={
                "on_orders": {
                    "description": "Build inventory for every genuine orders rebuild.",
                    "job_key": "inventory",
                    "when": {
                        "kind": "node_event",
                        "selection": ["model:orders"],
                        "kinds": ["failed", "recovered"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source_new_data selectors freeze only source nodes without event kinds",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.py"
                ).read_text(),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_jobs=("orders_refresh",),
            expected_sensors={
                "on_source_orders": {
                    "description": "Refresh orders for the first source version and each changed version.",
                    "job_key": "orders_refresh",
                    "when": {"kind": "source_new_data", "selection": ["source:raw_orders"]},
                    "minimum_interval_seconds": 1,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source_stale freezes source nodes and a positive timestamp-age threshold",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                ).read_text(),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_jobs=("orders_refresh",),
            expected_sensors={
                "on_stale_orders": {
                    "description": "Refresh orders once for each timestamp version older than one minute.",
                    "job_key": "orders_refresh",
                    "when": {
                        "kind": "source_stale",
                        "selection": ["source:raw_orders"],
                        "after_seconds": 60,
                    },
                    "minimum_interval_seconds": 1,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source-job execution selectors compile to frozen canonical keys",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                ).read_text()
            },
            expected_jobs=("inventory", "source_orders"),
            expected_sensors={
                "on_orders": {
                    "description": "Build inventory only after orders actually succeeds.",
                    "job_key": "inventory",
                    "when": {
                        "kind": "run_event",
                        "job_key": "source_orders",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="sensor-only roots preserve referenced jobs and filter targets",
            files={
                "orchestration/health.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/platform_orders.py"
                ).read_text()
            },
            expected_jobs=("health_orders",),
            expected_sensors={
                "overdue_orders": {
                    "description": "Catch up overdue order reporting.",
                    "job_key": "health_orders",
                    "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
                    "minimum_interval_seconds": 7,
                    "default_status": "stopped",
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="a project without declarations has no job document",
            files={},
            expected_jobs=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declaration_modules_when_compiling_then_native_discovery_collects_jobs(
    tmp_path: Path, test_case: SnapshotDiscoveryTestCase
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES | test_case.files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    publication: dict[str, Any] = json.loads(
        compile_snapshot(
            project=project,
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            declarations=test_case.declarations,
        ).to_json()
    )
    assert tuple(publication["documents"].get("jobs", {})) == test_case.expected_jobs
    assert {
        name: definition["jobs"]
        for name, definition in publication["documents"].get("groups", {}).items()
    } == test_case.expected_groups
    assert {
        name: publication["documents"]["triggers"][name] for name in test_case.expected_sensors
    } == test_case.expected_sensors
    assert tuple(publication["documents"].get("triggers", {})) == tuple(
        sorted((*test_case.expected_schedules, *test_case.expected_sensors))
    )
    for definition in publication["documents"].get("jobs", {}).values():
        assert set(definition["selection"]) <= publication["nodes"].keys()


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDiscoveryRejectionTestCase(
            description="group members are jobs, not nested groups",
            files={
                "orchestration/jobs.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/nested_groups.py"
                ).read_text(),
            },
            expected_error=TypeError,
            expected_message="group members must be Job objects",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a polling sensor needs an actual callback",
            files={
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, sensor\n"
                    "orders = job('orders', description='Build orders.', select='orders')\n"
                    "poll = sensor('poll', description='Poll orders.', run=orders)\n"
                ),
            },
            expected_error=ValueError,
            expected_message="either a built-in condition or a decorated polling callback",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="callbacks with event conditions remain explicitly unsupported",
            files={
                "orchestration/orders.py": (
                    "from sqlbuild.orchestration import job, sensor, platform_event, PlatformEventKind\n"
                    "orders = job('orders', description='Build orders.', select='orders')\n"
                    "@sensor('poll', description='Poll orders.', run=orders, "
                    "when=platform_event(kinds=[PlatformEventKind.TICK_OVERDUE]))\n"
                    "def poll(ctx):\n    return []\n"
                ),
            },
            expected_error=ValueError,
            expected_message="callbacks with when are not supported yet",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="explicit group cycles cannot publish",
            files={
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job, job_group\n"
                    "staging = job('staging', description='Stage orders.', select='stg_orders')\n"
                    "marts = job('marts', description='Build orders.', select='orders')\n"
                    "daily = job_group('daily', description='Cyclic jobs.', "
                    "jobs=[marts, staging], after={staging: [marts]})\n"
                ),
            },
            expected_error=ValueError,
            expected_message="group dependencies contain a cycle",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="group waits cannot refer to nonmember jobs",
            files={
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job, job_group\n"
                    "staging = job('staging', description='Stage orders.', select='stg_orders')\n"
                    "marts = job('marts', description='Build orders.', select='orders')\n"
                    "daily = job_group('daily', description='Unknown upstream.', "
                    "jobs=[marts], after={marts: [staging]})\n"
                ),
            },
            expected_error=ValueError,
            expected_message="group needs known jobs and dependencies within its membership",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="distinct objects cannot share a job name",
            files={
                "orchestration/jobs.py": (
                    "from sqlbuild.orchestration import job\n"
                    "jobs = [job('daily', description='Orders.', select='orders') for _ in range(2)]\n"
                ),
            },
            expected_error=ValueError,
            expected_message="duplicate job daily",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="broken imports fail on every retry",
            files={
                "orchestration/jobs.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/failed_import.py"
                ).read_text()
            },
            expected_error=PythonNodeDiscoveryError,
            expected_message="Failed to import orchestration file orchestration/jobs.py",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="DuckShop settings cannot replace the selection",
            files={
                "orchestration/jobs.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/invalid_settings.py"
                ).read_text()
            },
            expected_error=ValueError,
            expected_message="owned by the run request",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="DuckShop mutually exclusive settings fail on every retry",
            files={
                "orchestration/jobs.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/conflicting_settings.py"
                ).read_text()
            },
            expected_error=ValueError,
            expected_message="mutually exclusive",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a zero sensor interval cannot publish",
            files={
                "orchestration/health.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/platform_orders.py"
                )
                .read_text()
                .replace("minimum_interval_seconds=7", "minimum_interval_seconds=0")
            },
            expected_error=ValueError,
            expected_message="nonzero u32",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="empty platform conditions cannot publish",
            files={
                "orchestration/health.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/platform_orders.py"
                )
                .read_text()
                .replace("kinds=[PlatformEventKind.TICK_OVERDUE]", "kinds=[]")
            },
            expected_error=ValueError,
            expected_message="nonempty event kinds",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="platform kinds are a closed vocabulary",
            files={
                "orchestration/health.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/platform_orders.py"
                )
                .read_text()
                .replace("kinds=[PlatformEventKind.TICK_OVERDUE]", "kinds=['unknown_event']")
            },
            expected_error=ValueError,
            expected_message="unknown variant",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="empty node selectors cannot broaden matching to all nodes",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/node_orders.py"
                )
                .read_text()
                .replace('select="orders orders*"', 'select=" "')
            },
            expected_error=ValueError,
            expected_message="node_event needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unknown node event kinds cannot publish",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/node_orders.py"
                )
                .read_text()
                .replace("kinds=[NodeEventKind.SUCCESS]", "kinds=['unknown_event']")
            },
            expected_error=ValueError,
            expected_message="unknown variant",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_new_data rejects model selections",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.py"
                )
                .read_text()
                .replace('select="source:raw_orders"', 'select="orders"'),
            },
            expected_error=ValueError,
            expected_message="source_new_data must select only source nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_new_data rejects blank selections",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.py"
                )
                .read_text()
                .replace('select="source:raw_orders"', 'select=" "'),
            },
            expected_error=ValueError,
            expected_message="source_new_data needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects a zero threshold",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                )
                .read_text()
                .replace("after_seconds=60", "after_seconds=0"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects a boolean threshold",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                )
                .read_text()
                .replace("after_seconds=60", "after_seconds=True"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects an overflowing threshold",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                )
                .read_text()
                .replace("after_seconds=60", "after_seconds=2**32"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects model selections",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/stale_orders.py"
                )
                .read_text()
                .replace('select="source:raw_orders"', 'select="orders"'),
            },
            expected_error=ValueError,
            expected_message="source_stale must select only source nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="explicit empty execution selectors cannot broaden matching",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', "()")
            },
            expected_error=ValueError,
            expected_message="executed_successfully needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="blank execution selectors cannot broaden matching",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', '(" ",)')
            },
            expected_error=ValueError,
            expected_message="executed_successfully needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unresolved execution selectors fail closed",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', '("unknown_orders",)')
            },
            expected_error=ValueError,
            expected_message="unknown selector name 'unknown_orders'",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unmatched execution patterns cannot become an omitted filter",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', '("unknown_*",)')
            },
            expected_error=ValueError,
            expected_message="unknown selector pattern",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="invalid execution selector syntax fails closed",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', '("tag:",)')
            },
            expected_error=ValueError,
            expected_message="has empty value after ':'",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="execution selectors outside the source job fail closed",
            files={
                "orchestration/orders.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/run_orders.py"
                )
                .read_text()
                .replace('("orders",)', '("stg_orders",)')
            },
            expected_error=ValueError,
            expected_message="executed_successfully must select nonempty source job/group nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="sensor code defaults use running or stopped",
            files={
                "orchestration/health.py": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/platform_orders.py"
                )
                .read_text()
                .replace('default_status="stopped"', 'default_status="paused"')
            },
            expected_error=ValueError,
            expected_message="unknown variant",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_broken_declaration_modules_when_compiling_then_no_partial_snapshot_is_returned(
    tmp_path: Path, test_case: SnapshotDiscoveryRejectionTestCase
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES | test_case.files)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discover_project_inputs(project_dir=tmp_path),
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    for _ in range(2):
        with pytest.raises(test_case.expected_error, match=test_case.expected_message):
            compile_snapshot(
                project=project, metadata_json=json.dumps(METADATA), project_dir=tmp_path
            )
