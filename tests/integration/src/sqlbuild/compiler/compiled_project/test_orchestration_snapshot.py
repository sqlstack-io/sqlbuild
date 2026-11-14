"""Real compilation produces immutable, relocatable control-plane catalog data."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    DiscoveryConflictError,
    NativeOrchestrationSnapshot,
    PythonNodeDiscoveryError,
    build_compiled_project,
    build_orchestration_snapshot,
    discover_project_inputs,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.orchestration import compile_snapshot
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    PythonCheckJobRejectionTestCase,
    PythonCheckJobTestCase,
    PythonJobGroupCycleTestCase,
    PythonJobGroupTestCase,
    PythonJobSelectionTestCase,
    SnapshotCatalogTestCase,
    SnapshotDeclarationRejectionTestCase,
    SnapshotDiscoveryRejectionTestCase,
    SnapshotDiscoveryTestCase,
    SnapshotEnvironmentTestCase,
    SnapshotExpressionRangeTestCase,
    SnapshotIdentityTestCase,
    SnapshotLineageTestCase,
    SnapshotProvidersTestCase,
    SnapshotRejectionTestCase,
    SnapshotScheduleTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    FUNCTION_PROJECT_FILES,
    METADATA,
    PARITY_FILES,
    PYTHON_PIPELINE_FILES,
    TYPE_PROOF_FILES,
    duckshop_orchestration,
    published_texts,
    python_pipeline_snapshot,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

ORCHESTRATION_NAME_CONFLICT: str = (
    "; model, source, seed, function, loader, task, asset, check, provider, hook, unit test, "
    "scenario, job, job group, sensor, and calendar names must be globally unique; rename one of "
    "them (a job or job group takes its name from `name` in its TOML file, and a calendar or "
    "sensor from its `name` argument)"
)

_DUCKSHOP: Path = Path("benchmarks/duckshop/project")


_DAILY_BUILD: dict[str, str] = {
    "name": '"daily_build_job"',
    "description": '"Orders."',
    "select": '"orders"',
    "schedule": '{ cron = "0 2 * * *", timezone = "UTC" }',
}


def _edited(fixture: str, old: str, new: str) -> dict[str, str]:
    """A DuckShop orchestration fixture with one edit to the file that holds `old`."""
    files: dict[str, str] = duckshop_orchestration(fixture)
    (path,) = [path for path, text in files.items() if old in text]
    return files | {path: files[path].replace(old, new)}


_PROVIDER_PROJECT: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "models/orders.sql": "MODEL (description 'Orders.');\n\nSELECT 1 AS order_id\n",
}


_PRICE_MODELS: dict[str, str] = {
    "models/stg_prices.sql": (
        'MODEL (description "Staged prices");\n'
        "SELECT 'W-1' AS window_id, CAST(12345 AS BIGINT) AS price_minor, "
        "CAST('2026-03-30 08:00:00' AS TIMESTAMP) AS loaded_at\n"
    ),
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
                **duckshop_orchestration("custom_orders"),
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
        discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=directory)
        project: CompiledProjectView = build_compiled_project(
            discovered_inputs=discovered,
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        assert not any(item.severity == "error" for item in project.diagnostics)
        snapshot: NativeOrchestrationSnapshot = build_orchestration_snapshot(
            project, metadata, directory, discovered_inputs=discovered
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
        discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=directory)
        project: CompiledProjectView = build_compiled_project(
            discovered_inputs=discovered,
            adapter=DuckDbAdapter(),
            resolved_connection={"database": "warehouse", "schema": "main"},
        )
        assert not any(item.severity == "error" for item in project.diagnostics)
        snapshot: dict[str, Any] = json.loads(
            build_orchestration_snapshot(
                project, json.dumps(METADATA), directory, discovered_inputs=discovered
            ).to_json()
        )
        hashes: dict[str, str] = {}
        for key, expected_identity_keys in (
            ("model:orders", {"local_hash", "dialect"}),
            ("model:stg_orders", {"local_hash", "dialect"}),
            ("udf:edge_next_quantity", {"local_hash"}),
            ("seed:edge_quantity_steps", {"local_hash"}),
        ):
            identity: dict[str, str] = snapshot["nodes"][key]["definition"]["planner_identity"]
            assert set(identity) == expected_identity_keys
            assert identity.get("dialect", "duckdb") == "duckdb"
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
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
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
            discovered_inputs=discovered,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotLineageTestCase(
            description="published column lineage follows passthroughs, casts and aggregations",
            files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                "models/stg_orders.sql": (
                    "MODEL (description 'Staged orders.');\n\n"
                    "SELECT 1 AS order_id, 'eu' AS region, '5.00' AS amount_text\n"
                ),
                "models/orders.sql": (
                    "MODEL (description 'Typed orders.');\n\n"
                    "SELECT order_id, region, CAST(amount_text AS DECIMAL(10, 2)) AS amount\n"
                    'FROM __ref("stg_orders")\n'
                ),
                "models/region_revenue.sql": (
                    "MODEL (description 'Revenue by region.');\n\n"
                    "SELECT region, SUM(amount) AS revenue\n"
                    'FROM __ref("orders")\nGROUP BY region\n'
                ),
            },
            expected_edges=[
                ["model:orders", "amount", "model:region_revenue", "revenue", "aggregation"],
                ["model:orders", "region", "model:region_revenue", "region", "direct"],
                ["model:stg_orders", "amount_text", "model:orders", "amount", "cast"],
                ["model:stg_orders", "order_id", "model:orders", "order_id", "direct"],
                ["model:stg_orders", "region", "model:orders", "region", "direct"],
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_compiled_project_when_publishing_then_its_column_lineage_is_a_snapshot_document(
    test_case: SnapshotLineageTestCase,
    tmp_path: Path,
) -> None:
    write_files(project_dir=tmp_path, files=test_case.files)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    publication: dict[str, Any] = json.loads(
        build_orchestration_snapshot(
            project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
        ).to_json()
    )
    edges: list[list[str]] = publication["documents"]["lineage"]["edges"]
    assert sorted(edges) == test_case.expected_edges
    sources: set[str] = {edge[0] for edge in edges}
    targets: set[str] = {edge[2] for edge in edges}
    assert sources | targets <= set(publication["nodes"])


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotProvidersTestCase(
            description="a provider is published with its fields and consumers but no values",
            files=_PROVIDER_PROJECT
            | {
                "providers/fulfillment.py": (
                    "from typing import Literal\n\n"
                    "from pydantic import AliasChoices, Field, SecretStr\n"
                    "from pydantic_settings import SettingsConfigDict\n"
                    "from sqlbuild.providers import Provider\n\n\n"
                    "class FulfillmentApi(Provider):\n"
                    '    """Client for the fulfillment service.\n\n    Sends shipped orders.\n    """\n\n'
                    "    model_config = SettingsConfigDict(env_prefix='FULFILLMENT_')\n"
                    "    api_key: SecretStr\n"
                    "    backup_key: SecretStr | None = None\n"
                    "    regions: list[str] = ['eu']\n"
                    "    compression: Literal['none', 'gzip'] = 'gzip'\n"
                    "    endpoint: str = Field('https://fulfillment.example.com', description='Base URL')\n"
                    "    timeout_seconds: int | None = Field(\n"
                    "        None, validation_alias=AliasChoices('SHIP_TIMEOUT', 'TIMEOUT')\n"
                    "    )\n"
                ),
                "python/tasks/publish_orders.py": (
                    "from providers.fulfillment import FulfillmentApi\n"
                    "from sqlbuild.tasks import task\n\n\n"
                    "@task\n"
                    "def publish_orders(fulfillment_api: FulfillmentApi):\n"
                    '    """Send orders to fulfillment."""\n'
                ),
                "models/orders.sql": (
                    "MODEL (description 'Orders.', post_hooks [python(\"notify_fulfillment\")]);\n\n"
                    "SELECT 1 AS order_id\n"
                ),
                "hooks/python/notify_fulfillment.py": (
                    "from sqlbuild.hooks import hook\n\n\n"
                    "@hook\n"
                    "def notify_fulfillment(ctx, fulfillment_api):\n"
                    '    """Tell fulfillment a model changed."""\n'
                ),
            },
            environment={"FULFILLMENT_API_KEY": "synthetic-fulfillment-key"},
            expected_providers={
                "providers": [
                    {
                        "name": "fulfillment_api",
                        "class": "providers.fulfillment.FulfillmentApi",
                        "description": "Client for the fulfillment service.\n\nSends shipped orders.",
                        "path": "providers/fulfillment.py",
                        "settings": [
                            {
                                "name": "api_key",
                                "type": "SecretStr",
                                "required": True,
                                "secret": True,
                                "env": "FULFILLMENT_API_KEY",
                                "description": None,
                            },
                            {
                                "name": "backup_key",
                                "type": "SecretStr | None",
                                "required": False,
                                "secret": True,
                                "env": "FULFILLMENT_BACKUP_KEY",
                                "description": None,
                            },
                            {
                                "name": "regions",
                                "type": "list[str]",
                                "required": False,
                                "secret": False,
                                "env": "FULFILLMENT_REGIONS",
                                "description": None,
                            },
                            {
                                "name": "compression",
                                "type": "Literal['none', 'gzip']",
                                "required": False,
                                "secret": False,
                                "env": "FULFILLMENT_COMPRESSION",
                                "description": None,
                            },
                            {
                                "name": "endpoint",
                                "type": "str",
                                "required": False,
                                "secret": False,
                                "env": "FULFILLMENT_ENDPOINT",
                                "description": "Base URL",
                            },
                            {
                                "name": "timeout_seconds",
                                "type": "int | None",
                                "required": False,
                                "secret": False,
                                "env": "SHIP_TIMEOUT",
                                "description": None,
                            },
                        ],
                        "used_by": [
                            {"kind": "task", "name": "publish_orders"},
                            {"kind": "hook", "name": "notify_fulfillment"},
                        ],
                        "nodes": ["hook:notify_fulfillment", "model:orders", "task:publish_orders"],
                    }
                ]
            },
        ),
        SnapshotProvidersTestCase(
            description="a prefix that applies only to aliases leaves plain field names unprefixed",
            files=_PROVIDER_PROJECT
            | {
                "providers/inventory.py": (
                    "from pydantic import Field\n"
                    "from pydantic_settings import SettingsConfigDict\n"
                    "from sqlbuild.providers import Provider\n\n\n"
                    "class InventoryApi(Provider):\n"
                    '    """Client for the inventory service."""\n\n'
                    "    model_config = SettingsConfigDict(\n"
                    "        env_prefix='INVENTORY_', env_prefix_target='alias'\n"
                    "    )\n"
                    "    region: str = 'eu'\n"
                    "    token: str = Field('unset', alias='TOKEN')\n"
                ),
            },
            environment={},
            expected_providers={
                "providers": [
                    {
                        "name": "inventory_api",
                        "class": "providers.inventory.InventoryApi",
                        "description": "Client for the inventory service.",
                        "path": "providers/inventory.py",
                        "settings": [
                            {
                                "name": "region",
                                "type": "str",
                                "required": False,
                                "secret": False,
                                "env": "REGION",
                                "description": None,
                            },
                            {
                                "name": "token",
                                "type": "str",
                                "required": False,
                                "secret": False,
                                "env": "INVENTORY_TOKEN",
                                "description": None,
                            },
                        ],
                        "used_by": [],
                        "nodes": [],
                    }
                ]
            },
        ),
        SnapshotProvidersTestCase(
            description="a project without providers publishes no providers document",
            files=_PROVIDER_PROJECT,
            environment={},
            expected_providers=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_providers_when_publishing_then_they_are_a_snapshot_document_without_values(
    test_case: SnapshotProvidersTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    for name, value in test_case.environment.items():
        monkeypatch.setenv(name, value)
    write_files(project_dir=tmp_path, files=test_case.files)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    published: str = build_orchestration_snapshot(
        project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
    ).to_json()
    assert json.loads(published)["documents"].get("providers") == test_case.expected_providers
    assert "synthetic-fulfillment-key" not in published
    assert "fulfillment.example.com" not in published


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotExpressionRangeTestCase(
            description="SQL and Python macros, a variable and cursor bounds rewrite the SQL",
            adapter=DuckDbAdapter,
            files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n\n[vars]\nreporting_currency = "USD"\n'
                ),
                "models/_macros/sql/minor_to_major.sql": (
                    "MACRO (\n"
                    '  description "Minor units to major units",\n'
                    "  arguments (amount BIGINT, places INT CONST DEFAULT 2),\n"
                    "  returns DECIMAL(18, 2),\n"
                    ");\n\n"
                    "ROUND(@amount / 100, @places)::DECIMAL(18, 2)\n"
                ),
                "models/_macros/python/rounding.py": (
                    "def round_half_even(ctx, expression: str, places: int = 2) -> str:\n"
                    "    return f'ROUND_EVEN({expression}, {places})'\n"
                ),
                **_PRICE_MODELS,
                "models/price_spans.sql": (
                    "MODEL (\n"
                    '  description "Prices with rewritten expressions",\n'
                    "  materialized incremental,\n"
                    "  incremental_strategy delete_insert,\n"
                    "  cursor loaded_at,\n"
                    "  cursor_type timestamp,\n"
                    "  cursor_grain second,\n"
                    ");\n\n"
                    "-- Prix à jour €\n"
                    "SELECT\n"
                    "  p.window_id AS window_id,\n"
                    "  @sql.minor_to_major(p.price_minor) AS price_amount,\n"
                    '  @py.round_half_even("p.price_minor / 100", 2) AS rounded_price,\n'
                    "  '@@reporting_currency' AS currency,\n"
                    "  'Prix à jour €' AS label,\n"
                    "  p.loaded_at AS loaded_at\n"
                    'FROM __ref("stg_prices") AS p\n'
                    "WHERE p.loaded_at >= __cursor_start() AND p.loaded_at < __cursor_end()\n"
                ),
            },
            model_name="price_spans",
            expected_texts={
                "window_id": "p.window_id AS window_id",
                "price_amount": ("ROUND(p.price_minor / 100, 2)::DECIMAL(18, 2) AS price_amount"),
                "rounded_price": "ROUND_EVEN(p.price_minor / 100, 2) AS rounded_price",
                "currency": "'USD' AS currency",
                "label": "'Prix à jour €' AS label",
                "loaded_at": "p.loaded_at AS loaded_at",
            },
        ),
        SnapshotExpressionRangeTestCase(
            description="a column qualified by its reference call has no range rather than a moved one",
            adapter=DuckDbAdapter,
            files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                **_PRICE_MODELS,
                "models/price_refs.sql": (
                    'MODEL (description "Price references");\n'
                    'SELECT __ref("stg_prices").window_id AS window_id, '
                    "'EUR' AS currency FROM __ref(\"stg_prices\")\n"
                ),
            },
            model_name="price_refs",
            expected_texts={"currency": "'EUR' AS currency"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rewritten_model_sql_when_publishing_then_expression_ranges_slice_the_compiled_sql(
    test_case: SnapshotExpressionRangeTestCase,
    tmp_path: Path,
) -> None:
    assert published_texts(test_case, tmp_path) == test_case.expected_texts, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotExpressionRangeTestCase(
            description="an unaliased qualified column covers its qualifier",
            adapter=DuckDbAdapter,
            files={
                "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
                **_PRICE_MODELS,
                "models/price_columns.sql": (
                    'MODEL (description "Price columns");\n'
                    'SELECT p.window_id, p.price_minor FROM __ref("stg_prices") AS p\n'
                ),
            },
            model_name="price_columns",
            expected_texts={
                "window_id": "p.window_id",
                "price_minor": "p.price_minor",
            },
        ),
        SnapshotExpressionRangeTestCase(
            description="DuckShop macros, variable and cursor bounds rewrite the SQL",
            adapter=SnowflakeAdapter,
            files={
                "sqlbuild_project.toml": (
                    'name = "duckshop"\nadapter = "snowflake"\n\n'
                    '[defaults]\ndatabase = "WAREHOUSE"\nschema = "MAIN"\n\n'
                    '[vars]\nreporting_currency = "USD"\n'
                ),
                "sources/edge_cases.yml": (
                    "sources:\n"
                    "  - name: edge__price_windows\n"
                    "    description: Price windows\n"
                    "    columns:\n"
                    "      - name: window_id\n"
                    "        type: VARCHAR\n"
                    "      - name: price_minor\n"
                    "        type: NUMBER(38,0)\n"
                    "      - name: _loaded_at\n"
                    "        type: TIMESTAMP_NTZ\n"
                ),
                # A second consumer keeps the DuckShop macros project-wide, as they are there.
                "models/price_checks.sql": (
                    'MODEL (description "Price checks");\n'
                    "SELECT @sql.money_from_minor(150, 2) AS amount, "
                    '@py.round_half_even("1.25", 1) AS rounded\n'
                ),
                **{
                    path: (_DUCKSHOP / path).read_text()
                    for path in (
                        "macros/sql/money_from_minor.sql",
                        "macros/sql/safe_divide.sql",
                        "macros/python/rounding.py",
                        "models/intermediate/edge_cases/int_edge_expression_spans.sql",
                    )
                },
            },
            model_name="int_edge_expression_spans",
            expected_texts={
                "window_id": "w.window_id",
                "price_amount": (
                    "ROUND((\n  w.price_minor / NULLIF((POWER(10, 2)::NUMBER(38, 0)), 0)\n)"
                    "::NUMBER(38, 6), 2)::NUMBER(38, 6) AS price_amount"
                ),
                "rounded_price": (
                    "ROUND(w.price_minor / 100, 2, 'HALF_TO_EVEN')::NUMBER(18, 2) AS rounded_price"
                ),
                "currency": "'USD'::VARCHAR AS currency",
                "label": "'Prix à jour €'::VARCHAR AS label",
                "loaded_at": "w._loaded_at AS loaded_at",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_spans_strict_parsers_omit_when_publishing_then_expression_ranges_slice_the_compiled_sql(
    test_case: SnapshotExpressionRangeTestCase,
    tmp_path: Path,
) -> None:
    assert published_texts(test_case, tmp_path) == test_case.expected_texts, test_case.description


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
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    installed: dict[str, Any] = json.loads(
        build_orchestration_snapshot(
            project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
        ).to_json()
    )
    (tmp_path / "uv.lock").write_text(test_case.first_lockfile)
    locked: dict[str, Any] = json.loads(
        build_orchestration_snapshot(
            project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
        ).to_json()
    )
    (tmp_path / "uv.lock").write_text(test_case.second_lockfile)
    changed: dict[str, Any] = json.loads(
        build_orchestration_snapshot(
            project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
        ).to_json()
    )
    assert (
        len(
            {installed["environment_hash"], locked["environment_hash"], changed["environment_hash"]}
        )
        == test_case.expected_environment_count
    )
    assert installed["nodes"] == locked["nodes"] == changed["nodes"]
    assert changed == json.loads(
        build_orchestration_snapshot(
            project, json.dumps(METADATA), tmp_path, discovered_inputs=discovered
        ).to_json()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotScheduleTestCase(
            description="daylight saving zone",
            schedule='{ cron = "15 2 * * *", timezone = "Europe/London" }',
            expected_timezone="Europe/London",
            expected_selection=("model:orders",),
        ),
        SnapshotScheduleTestCase(
            description="fixed offset zone",
            schedule='{ cron = "15 2 * * *", timezone = "UTC" }',
            expected_timezone="UTC",
            expected_selection=("model:orders",),
        ),
        SnapshotScheduleTestCase(
            description="paused with a zoned start",
            schedule=(
                '{ cron = "15 2 * * *", timezone = "Europe/London", '
                'starts_at = "2026-07-03T01:17:23.456789+05:30", default_status = "paused" }'
            ),
            expected_timezone="Europe/London",
            expected_selection=("model:orders",),
            expected_controls={
                "starts_at": "2026-07-02T19:47:23.456789Z",
                "default_status": "paused",
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_jobs_when_compiling_then_native_snapshots_resolve_selection_and_cron(
    tmp_path: Path, test_case: SnapshotScheduleTestCase
) -> None:
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES)
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )

    def snapshot(schedule: str) -> dict[str, Any]:
        write_files(
            project_dir=tmp_path,
            files={
                "orchestration/jobs/nightly_orders_job.toml": (
                    'name = "nightly_orders_job"\n'
                    'description = "Refresh order reporting."\n'
                    'select = "+orders"\n'
                    'exclude = "stg_orders"\n'
                    'targets = ["development"]\n'
                    'owner = "Orders team"\n'
                    'overlap = "skip"\n'
                    'concurrency_groups = ["order_reporting"]\n'
                    f"schedule = {schedule}\n"
                    "\n[on_failure]\nretries = 2\nretry_delay_seconds = 600\n"
                    '\n[settings]\nrun_tests = false\nvars = { batch = 3 }\nwarehouse = "reporting"\n'
                    "concurrency = 4\n"
                ),
                "orchestration/jobs/other_target_job.toml": (
                    'name = "other_target_job"\n'
                    'description = "Refresh order reporting."\n'
                    'select = "+orders"\n'
                    'targets = ["production"]\n'
                    f"schedule = {schedule}\n"
                ),
            },
        )
        return json.loads(
            compile_snapshot(
                project=project,
                metadata_json=json.dumps(METADATA),
                project_dir=tmp_path,
                discovered_inputs=discover_project_inputs(project_dir=tmp_path),
            ).to_json()
        )

    first: dict[str, Any] = snapshot(test_case.schedule)
    assert first["documents"]["jobs"] == {
        "nightly_orders_job": {
            "description": "Refresh order reporting.",
            "selection": list(test_case.expected_selection),
            "settings": {
                "concurrency": 4,
                "run_tests": False,
                "vars": {"batch": 3},
                "warehouse": "reporting",
            },
            "concurrency_groups": ["order_reporting"],
            "select": "+orders",
            "exclude": "stg_orders",
            "targets": ["development"],
            "owner": "Orders team",
            "overlap": "skip",
            "on_failure": {"retries": 2, "retry_delay_seconds": 600},
            "declared_at": {"path": "orchestration/jobs/nightly_orders_job.toml", "line": 1},
        }
    }
    trigger: dict[str, Any] = first["documents"]["triggers"]["nightly_orders_job"]
    assert trigger["job_key"] == "nightly_orders_job"
    assert trigger["targets"] == ["development"]
    assert trigger["schedule"] == {
        "kind": "cron",
        "expression": "15 2 * * *",
        "timezone": test_case.expected_timezone,
    }
    assert {key: trigger[key] for key in test_case.expected_controls} == test_case.expected_controls
    second: dict[str, Any] = snapshot('{ cron = "30 3 * * *", timezone = "UTC" }')
    assert second["documents"]["triggers"].keys() == first["documents"]["triggers"].keys()
    assert second["documents"]["triggers"]["nightly_orders_job"]["schedule"]["expression"] == (
        "30 3 * * *"
    )
    assert second["nodes"] == first["nodes"]


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDeclarationRejectionTestCase(
            description="blank description",
            fields={"description": '" "'},
            expected_message="2:15: description must not be blank",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="blank selection",
            fields={"select": '""'},
            expected_message="3:10: select must not be blank",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="missing model",
            fields={"select": '"missing"'},
            expected_message="missing",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="invalid cron",
            fields={"schedule": '{ cron = "not a cron", timezone = "UTC" }'},
            expected_message="cron",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="invalid timezone",
            fields={"schedule": '{ cron = "0 2 * * *", timezone = "Unknown/Zone" }'},
            expected_message="time zone",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="start needs an explicit offset",
            fields={
                "schedule": '{ cron = "0 2 * * *", timezone = "UTC", starts_at = "2026-07-03T01:17:23" }'
            },
            expected_message="starts_at",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="duplicate names",
            copies=2,
            expected_message="name 'daily_build_job' is declared as both job",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="unknown setting",
            tables="[settings]\nconcurreny = 4\n",
            expected_message="unknown build setting concurreny",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="selection cannot escape its pinned run",
            tables='[settings]\nselect = "customers"\n',
            expected_message="owned by the run request",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="zero concurrency",
            tables="[settings]\nconcurrency = 0\n",
            expected_message="must be positive",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="boolean is not concurrency",
            tables="[settings]\nconcurrency = true\n",
            expected_message="concurrency must be an integer",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="integer is not a boolean flag",
            tables="[settings]\nfull_refresh = 1\n",
            expected_message="must be boolean",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="integer strings are not typed settings",
            tables='[settings]\nconcurrency = "3"\n',
            expected_message="must be an integer",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="variables need an object",
            tables='[settings]\nvars = ["orders"]\n',
            expected_message="must be a JSON object",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="mutually exclusive load settings",
            tables="[settings]\nload_sources = false\nreload = true\n",
            expected_message="mutually exclusive",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="conflicting cursor families",
            tables='[settings]\nstart_cursor = "2"\nend_cursor_int = "4"\n',
            expected_message="cannot be combined",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="blank owner",
            fields={"owner": '" "'},
            expected_message="owner must not be blank",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="unknown overlap",
            fields={"overlap": '"coalesce"'},
            expected_message="unknown variant `coalesce`, expected `queue` or `skip`",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="no retries",
            tables="[on_failure]\nretries = 0\n",
            expected_message="retries must be a whole number from 1 to 10",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="too many retries",
            tables="[on_failure]\nretries = 11\n",
            expected_message="retries must be a whole number from 1 to 10",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="retry delay over a day",
            tables="[on_failure]\nretries = 1\nretry_delay_seconds = 86401\n",
            expected_message="retry_delay_seconds must be a whole number from 0 to 86400",
        ),
        SnapshotDeclarationRejectionTestCase(
            description="concurrency belongs under settings",
            fields={"concurrency": "3"},
            expected_message="unknown job key 'concurrency'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_declarations_when_compiling_then_native_validation_rejects_them(
    tmp_path: Path, test_case: SnapshotDeclarationRejectionTestCase
) -> None:
    text: str = "".join(
        f"{key} = {value}\n" for key, value in (_DAILY_BUILD | test_case.fields).items()
    )
    copies: dict[str, str] = {
        f"orchestration/jobs/{'copy/' * copy}daily_build_job.toml": text + "\n" + test_case.tables
        for copy in range(test_case.copies)
    }
    write_files(project_dir=tmp_path, files=TYPE_PROOF_FILES | copies)
    with pytest.raises(ValueError, match=test_case.expected_message):
        discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
        compile_snapshot(
            project=build_compiled_project(
                discovered_inputs=discovered,
                adapter=DuckDbAdapter(),
                resolved_connection={"database": "warehouse", "schema": "main"},
            ),
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            discovered_inputs=discovered,
        )


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDiscoveryTestCase(
            description="group-only references retain lineage waits",
            files={
                "orchestration/jobs/staging_job.toml": (
                    'name = "staging_job"\ndescription = "Stage orders."\nselect = "stg_orders"\n'
                ),
                "orchestration/jobs/inventory_job.toml": (
                    'name = "inventory_job"\n'
                    'description = "Repeat staging."\n'
                    'select = "stg_orders"\n'
                ),
                "orchestration/jobs/marts_job.toml": (
                    'name = "marts_job"\ndescription = "Build orders."\nselect = "orders"\n'
                ),
                "orchestration/job_groups/daily_job_group.toml": (
                    'name = "daily_job_group"\n'
                    'description = "Ordered jobs."\n'
                    'jobs = ["marts_job", "inventory_job", "staging_job"]\n'
                ),
            },
            expected_jobs=("inventory_job", "marts_job", "staging_job"),
            expected_groups={
                "daily_job_group": {
                    "inventory_job": [],
                    "marts_job": ["inventory_job", "staging_job"],
                    "staging_job": [],
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="DuckShop layer jobs that share a function wait only by lineage",
            files={
                **duckshop_orchestration("shared_function_layers"),
                "functions/sql/order_cents.sql": (
                    "FUNCTION (description 'Amount in cents.',"
                    " arguments (amount INTEGER), returns INTEGER);\nSELECT amount * 100\n"
                ),
                "models/stg_orders.sql": (
                    "MODEL (description 'Staged orders.', tags [staging],\n"
                    "  columns (order_id (type INTEGER), cents (type INTEGER)),\n"
                    ");\n\n"
                    "SELECT CAST(1 AS INTEGER) AS order_id,"
                    ' __udf("order_cents")(CAST(5 AS INTEGER)) AS cents\n'
                ),
                "models/orders.sql": (
                    "MODEL (description 'Typed orders.', tags [marts],\n"
                    "  columns (order_id (type INTEGER), cents (type INTEGER)),\n"
                    ");\n\n"
                    'SELECT order_id, __udf("order_cents")(cents) AS cents\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_jobs=("marts_layer_job", "staging_layer_job"),
            expected_groups={
                "layered_orders_job_group": {
                    "marts_layer_job": ["staging_layer_job"],
                    "staging_layer_job": [],
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="DuckShop edge-case declaration containers",
            files={
                **{
                    path.relative_to(_DUCKSHOP).as_posix(): path.read_text()
                    for path in (_DUCKSHOP / "orchestration").rglob("*")
                    if path.is_file()
                },
                "models/orders.sql": TYPE_PROOF_FILES["models/orders.sql"].replace(
                    "MODEL (", "MODEL (tags ['edge_case'],", 1
                ),
            },
            expected_jobs=(
                "edge_snapshot_inventory_job",
                "edge_snapshot_orders_job",
                "edge_snapshot_products_job",
            ),
            expected_groups={
                "edge_snapshot_job_group": {"edge_snapshot_orders_job": []},
                "edge_inventory_job_group": {"edge_snapshot_inventory_job": []},
                "edge_products_job_group": {"edge_snapshot_products_job": []},
            },
            expected_sensors={
                "edge_platform_orders_sensor": {
                    "description": "Catch up retained platform observations with a stopped group sensor.",
                    "job_key": "edge_snapshot_job_group",
                    "when": {
                        "kind": "platform_event",
                        "kinds": ["tick_overdue", "location_broken", "launch_failed"],
                    },
                    "minimum_interval_seconds": 7,
                    "default_status": "paused",
                    "declared_at": {
                        "path": "orchestration/sensors/edge_cases/edge_platform_orders_sensor.py",
                        "line": 5,
                    },
                },
                "edge_job_outcomes_sensor": {
                    "description": "Keep warning and partial outcomes distinct from success.",
                    "job_key": "edge_snapshot_inventory_job",
                    "when": {
                        "kind": "run_event",
                        "job_key": "edge_snapshot_orders_job",
                        "kinds": ["warn", "partial"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "paused",
                    "declared_at": {
                        "path": "orchestration/sensors/edge_cases/edge_job_outcomes_sensor.py",
                        "line": 5,
                    },
                },
                "edge_group_outcomes_sensor": {
                    "description": "Observe group completion including cancellation and presumed failure.",
                    "job_key": "edge_snapshot_inventory_job",
                    "when": {
                        "kind": "run_event",
                        "job_key": "edge_snapshot_job_group",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "paused",
                    "declared_at": {
                        "path": "orchestration/sensors/edge_cases/edge_group_outcomes_sensor.py",
                        "line": 5,
                    },
                },
            },
            expected_schedules=("edge_snapshot_orders_job",),
            expected_trigger_declared_at={
                "edge_snapshot_orders_job": {
                    "path": "orchestration/jobs/edge_cases/edge_snapshot_orders_job.toml",
                    "line": 2,
                },
            },
            expected_declared_at={
                "edge_snapshot_orders_job": {
                    "path": "orchestration/jobs/edge_cases/edge_snapshot_orders_job.toml",
                    "line": 2,
                },
                "edge_snapshot_job_group": {
                    "path": "orchestration/job_groups/edge_cases/edge_snapshot_job_group.toml",
                    "line": 1,
                },
            },
        ),
        SnapshotDiscoveryTestCase(
            description="a run_event on a group freezes its members' execution selectors",
            files={
                "orchestration/jobs/source_orders_job.toml": (
                    'name = "source_orders_job"\ndescription = "Orders."\nselect = "orders"\n'
                ),
                "orchestration/jobs/inventory_job.toml": (
                    'name = "inventory_job"\ndescription = "Inventory."\nselect = "stg_orders"\n'
                ),
                "orchestration/job_groups/source_job_group.toml": (
                    'name = "source_job_group"\ndescription = "Source orders."\n'
                    'jobs = ["source_orders_job"]\n'
                ),
                "orchestration/sensors/on_orders_sensor.py": (
                    "from sqlbuild.orchestration import RunEventKind, run_event, sensor\n"
                    "on_orders = sensor(\n"
                    "    'on_orders_sensor',\n"
                    "    description='Build inventory after group completion.',\n"
                    "    when=run_event(\n"
                    "        job='source_job_group',\n"
                    "        kinds=[RunEventKind.COMPLETED],\n"
                    "        executed_successfully=('orders', 'orders*'),\n"
                    "    ),\n"
                    "    run='inventory_job',\n"
                    ")\n"
                ),
            },
            expected_jobs=("inventory_job", "source_orders_job"),
            expected_groups={"source_job_group": {"source_orders_job": []}},
            expected_sensors={
                "on_orders_sensor": {
                    "description": "Build inventory after group completion.",
                    "job_key": "inventory_job",
                    "when": {
                        "kind": "run_event",
                        "job_key": "source_job_group",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                    "declared_at": {"path": "orchestration/sensors/on_orders_sensor.py", "line": 2},
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="any-run node selectors freeze canonical keys without a source job reference",
            files={**duckshop_orchestration("node_orders")},
            expected_jobs=("inventory_job", "source_orders_job"),
            expected_sensors={
                "on_orders_sensor": {
                    "description": "Build inventory for every genuine orders rebuild.",
                    "job_key": "inventory_job",
                    "when": {
                        "kind": "node_event",
                        "selection": ["model:orders"],
                        "kinds": ["success"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                    "declared_at": {"path": "orchestration/sensors/on_orders_sensor.py", "line": 5},
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="failure and recovery selectors preserve their individual transition kinds",
            files={
                **_edited(
                    "node_orders",
                    "NodeEventKind.SUCCESS",
                    "NodeEventKind.FAILED, NodeEventKind.RECOVERED",
                )
            },
            expected_jobs=("inventory_job", "source_orders_job"),
            expected_sensors={
                "on_orders_sensor": {
                    "description": "Build inventory for every genuine orders rebuild.",
                    "job_key": "inventory_job",
                    "when": {
                        "kind": "node_event",
                        "selection": ["model:orders"],
                        "kinds": ["failed", "recovered"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                    "declared_at": {"path": "orchestration/sensors/on_orders_sensor.py", "line": 5},
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source_new_data selectors freeze only source nodes without event kinds",
            files={
                **duckshop_orchestration("source_orders"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_jobs=("orders_refresh_job",),
            expected_sensors={
                "on_source_orders_sensor": {
                    "description": "Refresh orders for the first source version and each changed version.",
                    "job_key": "orders_refresh_job",
                    "when": {"kind": "source_new_data", "selection": ["source:raw_orders"]},
                    "minimum_interval_seconds": 1,
                    "default_status": "running",
                    "declared_at": {
                        "path": "orchestration/sensors/on_source_orders_sensor.py",
                        "line": 5,
                    },
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source_stale freezes source nodes and a positive timestamp-age threshold",
            files={
                **duckshop_orchestration("stale_orders"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_jobs=("orders_refresh_job",),
            expected_sensors={
                "on_stale_orders_sensor": {
                    "description": "Refresh orders once for each timestamp version older than one minute.",
                    "job_key": "orders_refresh_job",
                    "when": {
                        "kind": "source_stale",
                        "selection": ["source:raw_orders"],
                        "after_seconds": 60,
                    },
                    "minimum_interval_seconds": 1,
                    "default_status": "running",
                    "declared_at": {
                        "path": "orchestration/sensors/on_stale_orders_sensor.py",
                        "line": 5,
                    },
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="source-job execution selectors compile to frozen canonical keys",
            files={**duckshop_orchestration("run_orders")},
            expected_jobs=("inventory_job", "source_orders_job"),
            expected_sensors={
                "on_orders_sensor": {
                    "description": "Build inventory only after orders actually succeeds.",
                    "job_key": "inventory_job",
                    "when": {
                        "kind": "run_event",
                        "job_key": "source_orders_job",
                        "kinds": ["completed"],
                        "executed_successfully": ["model:orders"],
                    },
                    "minimum_interval_seconds": 30,
                    "default_status": "running",
                    "declared_at": {"path": "orchestration/sensors/on_orders_sensor.py", "line": 5},
                }
            },
        ),
        SnapshotDiscoveryTestCase(
            description="sensor-only roots preserve referenced jobs and filter targets",
            files={**duckshop_orchestration("platform_orders")},
            expected_jobs=("health_orders_job",),
            expected_sensors={
                "overdue_orders_sensor": {
                    "description": "Catch up overdue order reporting.",
                    "job_key": "health_orders_job",
                    "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
                    "minimum_interval_seconds": 7,
                    "default_status": "paused",
                    "targets": ["development"],
                    "declared_at": {
                        "path": "orchestration/sensors/overdue_orders_sensor.py",
                        "line": 8,
                    },
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
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    project: CompiledProjectView = build_compiled_project(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        resolved_connection={"database": "warehouse", "schema": "main"},
    )
    publication: dict[str, Any] = json.loads(
        compile_snapshot(
            project=project,
            metadata_json=json.dumps(METADATA),
            project_dir=tmp_path,
            discovered_inputs=discovered,
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
    definitions: dict[str, Any] = {
        **publication["documents"].get("jobs", {}),
        **publication["documents"].get("groups", {}),
    }
    assert {
        name: definitions[name].get("declared_at") for name in test_case.expected_declared_at
    } == test_case.expected_declared_at
    assert {
        name: publication["documents"]["triggers"][name].get("declared_at")
        for name in test_case.expected_trigger_declared_at
    } == test_case.expected_trigger_declared_at


@pytest.mark.parametrize(
    "test_case",
    [
        SnapshotDiscoveryRejectionTestCase(
            description="group members are jobs, not nested groups",
            files=duckshop_orchestration("nested_groups"),
            expected_error=ValueError,
            expected_message="daily_orders_job_group",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a polling sensor needs an actual callback",
            files={
                "orchestration/jobs/refresh_orders_job.toml": (
                    'name = "refresh_orders_job"\ndescription = "Build orders."\nselect = "orders"\n'
                ),
                "orchestration/sensors/poll_sensor.py": (
                    "from sqlbuild.orchestration import sensor\n"
                    "poll = sensor('poll_sensor', description='Poll orders.', run='refresh_orders_job')\n"
                ),
            },
            expected_error=ValueError,
            expected_message="either a built-in condition or a decorated polling callback",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="callbacks with event conditions remain explicitly unsupported",
            files={
                "orchestration/jobs/refresh_orders_job.toml": (
                    'name = "refresh_orders_job"\ndescription = "Build orders."\nselect = "orders"\n'
                ),
                "orchestration/sensors/poll_sensor.py": (
                    "from sqlbuild.orchestration import sensor, platform_event, PlatformEventKind\n"
                    "@sensor('poll_sensor', description='Poll orders.', run='refresh_orders_job', "
                    "when=platform_event(kinds=[PlatformEventKind.TICK_OVERDUE]))\n"
                    "def poll(ctx):\n    return []\n"
                ),
            },
            expected_error=ValueError,
            expected_message="callbacks with when are not supported yet",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a job description past max_description_lines cannot publish",
            files={
                "orchestration/jobs/daily_job.toml": (
                    'name = "daily_job"\n'
                    'description = """Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders. Build orders."""\n'
                    'select = "orders"\n'
                ),
            },
            expected_error=ValueError,
            expected_message=re.escape(
                "job 'daily_job' description exceeds 10 lines; shorten it to 10 lines"
            ),
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a group cannot declare an order between its jobs",
            files={
                "orchestration/jobs/staging_job.toml": (
                    'name = "staging_job"\ndescription = "Stage orders."\nselect = "stg_orders"\n'
                ),
                "orchestration/jobs/marts_job.toml": (
                    'name = "marts_job"\ndescription = "Build orders."\nselect = "orders"\n'
                ),
                "orchestration/job_groups/daily_job_group.toml": (
                    'name = "daily_job_group"\n'
                    'description = "Ordered jobs."\n'
                    'jobs = ["marts_job", "staging_job"]\n'
                    'after = { staging_job = ["marts_job"] }\n'
                ),
            },
            expected_error=ValueError,
            expected_message="after",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="two job files cannot share a name",
            files={
                "orchestration/jobs/daily_job.toml": (
                    'name = "daily_job"\ndescription = "Orders."\nselect = "orders"\n'
                ),
                "orchestration/jobs/copy/daily_job.toml": (
                    'name = "daily_job"\ndescription = "Orders."\nselect = "orders"\n'
                ),
            },
            expected_error=DiscoveryConflictError,
            expected_message=re.escape(
                "Project resource name 'daily_job' is declared as both job in "
                "orchestration/jobs/copy/daily_job.toml and job in orchestration/jobs/daily_job.toml"
                + ORCHESTRATION_NAME_CONFLICT
            ),
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="DuckShop jobs cannot reuse a model name",
            files=duckshop_orchestration("name_collision")
            | {"models/orders_job.sql": "MODEL (description 'Orders.');\nSELECT 1 AS order_id\n"},
            expected_error=DiscoveryConflictError,
            expected_message=re.escape(
                "Project resource name 'orders_job' is declared as both model in "
                "models/orders_job.sql and job in orchestration/jobs/orders_job.toml"
                + ORCHESTRATION_NAME_CONFLICT
            ),
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="broken imports fail on every retry",
            files=duckshop_orchestration("failed_import"),
            expected_error=PythonNodeDiscoveryError,
            expected_message=(
                "Failed to import orchestration file orchestration/sensors/broken_orders_sensor.py"
            ),
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="DuckShop settings cannot replace the selection",
            files={**duckshop_orchestration("invalid_settings")},
            expected_error=ValueError,
            expected_message="owned by the run request",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="DuckShop mutually exclusive settings fail on every retry",
            files={**duckshop_orchestration("conflicting_settings")},
            expected_error=ValueError,
            expected_message="mutually exclusive",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="a zero sensor interval cannot publish",
            files={
                **_edited(
                    "platform_orders", "minimum_interval_seconds=7", "minimum_interval_seconds=0"
                )
            },
            expected_error=ValueError,
            expected_message="nonzero u32",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="empty platform conditions cannot publish",
            files={
                **_edited("platform_orders", "kinds=[PlatformEventKind.TICK_OVERDUE]", "kinds=[]")
            },
            expected_error=ValueError,
            expected_message="nonempty event kinds",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="platform kinds are a closed vocabulary",
            files={
                **_edited(
                    "platform_orders",
                    "kinds=[PlatformEventKind.TICK_OVERDUE]",
                    "kinds=['unknown_event']",
                )
            },
            expected_error=ValueError,
            expected_message="unknown variant",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="empty node selectors cannot broaden matching to all nodes",
            files={**_edited("node_orders", 'select="orders orders*"', 'select=" "')},
            expected_error=ValueError,
            expected_message="node_event needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unknown node event kinds cannot publish",
            files={
                **_edited("node_orders", "kinds=[NodeEventKind.SUCCESS]", "kinds=['unknown_event']")
            },
            expected_error=ValueError,
            expected_message="unknown variant",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_new_data rejects model selections",
            files={
                **_edited("source_orders", 'select="source:raw_orders"', 'select="orders"'),
            },
            expected_error=ValueError,
            expected_message="source_new_data must select only source nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_new_data rejects blank selections",
            files={
                **_edited("source_orders", 'select="source:raw_orders"', 'select=" "'),
            },
            expected_error=ValueError,
            expected_message="source_new_data needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects a zero threshold",
            files={
                **_edited("stale_orders", "after_seconds=60", "after_seconds=0"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders_sensor",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects a boolean threshold",
            files={
                **_edited("stale_orders", "after_seconds=60", "after_seconds=True"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders_sensor",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects an overflowing threshold",
            files={
                **_edited("stale_orders", "after_seconds=60", "after_seconds=2**32"),
                "sources/orders.yml": Path(
                    "benchmarks/duckshop/edge_cases/orchestration/source_orders.yml"
                ).read_text(),
            },
            expected_error=ValueError,
            expected_message="sensor on_stale_orders_sensor",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="source_stale rejects model selections",
            files={
                **_edited("stale_orders", 'select="source:raw_orders"', 'select="orders"'),
            },
            expected_error=ValueError,
            expected_message="source_stale must select only source nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="explicit empty execution selectors cannot broaden matching",
            files={**_edited("run_orders", '("orders",)', "()")},
            expected_error=ValueError,
            expected_message="executed_successfully needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="blank execution selectors cannot broaden matching",
            files={**_edited("run_orders", '("orders",)', '(" ",)')},
            expected_error=ValueError,
            expected_message="executed_successfully needs nonempty selectors",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unresolved execution selectors fail closed",
            files={**_edited("run_orders", '("orders",)', '("unknown_orders",)')},
            expected_error=ValueError,
            expected_message="unknown selector name 'unknown_orders'",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="unmatched execution patterns cannot become an omitted filter",
            files={**_edited("run_orders", '("orders",)', '("unknown_*",)')},
            expected_error=ValueError,
            expected_message="executed_successfully: unknown selector name 'unknown_\\*'",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="invalid execution selector syntax fails closed",
            files={**_edited("run_orders", '("orders",)', '("tag:",)')},
            expected_error=ValueError,
            expected_message="has empty value after ':'",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="execution selectors outside the source job fail closed",
            files={**_edited("run_orders", '("orders",)', '("stg_orders",)')},
            expected_error=ValueError,
            expected_message="executed_successfully must select nonempty source job/group nodes",
        ),
        SnapshotDiscoveryRejectionTestCase(
            description="sensor code defaults use running or paused",
            files={
                **_edited("platform_orders", 'default_status="paused"', 'default_status="stopped"')
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
    for _ in range(2):
        with pytest.raises(test_case.expected_error, match=test_case.expected_message):
            discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
            compile_snapshot(
                project=build_compiled_project(
                    discovered_inputs=discovered,
                    adapter=DuckDbAdapter(),
                    resolved_connection={"database": "warehouse", "schema": "main"},
                ),
                metadata_json=json.dumps(METADATA),
                project_dir=tmp_path,
                discovered_inputs=discovered,
            )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonJobSelectionTestCase(
            description="a model selector selects models",
            select="+orders",
            expected_selection=("model:orders", "model:stg_orders"),
        ),
        PythonJobSelectionTestCase(
            description="a task selector selects only the task",
            select="task:request_carrier_collection",
            expected_selection=("task:request_carrier_collection",),
        ),
        PythonJobSelectionTestCase(
            description="typed selectors select the task and the asset it depends on",
            select="task:request_carrier_collection asset:partner_orders_file",
            expected_selection=("asset:partner_orders_file", "task:request_carrier_collection"),
        ),
        PythonJobSelectionTestCase(
            description="upstream of a task crosses its Python and SQL dependencies",
            select="+task:request_carrier_collection",
            expected_selection=(
                "asset:partner_orders_file",
                "model:orders",
                "model:stg_orders",
                "task:request_carrier_collection",
            ),
        ),
        PythonJobSelectionTestCase(
            description="a python folder selects every node under it",
            select="python/orders",
            expected_selection=("asset:partner_orders_file",),
        ),
        PythonJobSelectionTestCase(
            description="a tag shared by models and Python nodes selects both",
            select="tag:orders",
            expected_selection=(
                "asset:partner_orders_file",
                "model:orders",
                "model:stg_orders",
                "task:request_carrier_collection",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_selectors_when_publishing_jobs_then_selections_match_the_cli(
    tmp_path: Path, test_case: PythonJobSelectionTestCase
) -> None:
    snapshot: dict[str, Any] = json.loads(
        python_pipeline_snapshot(tmp_path, {"orders_job": test_case.select}).to_json()
    )

    assert snapshot["documents"]["jobs"]["orders_job"]["selection"] == list(
        test_case.expected_selection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonJobGroupTestCase(
            description="a job waits for the job building the Python node its task depends on",
            jobs=(
                ("orders_export_job", "python/orders/partner_export"),
                ("carrier_sync_job", "python/fulfillment/carrier_sync"),
            ),
            expected_groups={
                "orders_nightly_job_group": {
                    "carrier_sync_job": ["orders_export_job"],
                    "orders_export_job": [],
                }
            },
        ),
        PythonJobGroupTestCase(
            description="a Python node's SQL dependency orders it after the model's job",
            jobs=(("orders_export_job", "asset:partner_orders_file"), ("models_job", "+orders")),
            expected_groups={
                "orders_nightly_job_group": {"models_job": [], "orders_export_job": ["models_job"]}
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_dependencies_across_jobs_when_publishing_a_group_then_jobs_wait_for_them(
    tmp_path: Path, test_case: PythonJobGroupTestCase
) -> None:
    jobs: dict[str, str] = dict(test_case.jobs)
    snapshot: dict[str, Any] = json.loads(
        python_pipeline_snapshot(
            tmp_path, jobs, {"orders_nightly_job_group": tuple(jobs)}
        ).to_json()
    )

    assert {
        name: definition["jobs"] for name, definition in snapshot["documents"]["groups"].items()
    } == test_case.expected_groups


@pytest.mark.parametrize(
    "test_case",
    [
        PythonJobGroupCycleTestCase(
            description="a Python edge that closes a cycle names both jobs and the edges",
            jobs=(
                ("staging_job", "stg_orders task:request_carrier_collection"),
                ("export_job", "asset:partner_orders_file"),
            ),
            expected_message=(
                "group dependencies contain a cycle in job group orders_nightly_job_group: "
                "export_job waits for staging_job (model:orders reads model:stg_orders); "
                "staging_job waits for export_job "
                "(task:request_carrier_collection reads asset:partner_orders_file)"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_python_dependency_cycle_across_jobs_when_publishing_a_group_then_it_is_rejected(
    tmp_path: Path, test_case: PythonJobGroupCycleTestCase
) -> None:
    jobs: dict[str, str] = dict(test_case.jobs)
    with pytest.raises(ValueError, match=re.escape(test_case.expected_message)):
        python_pipeline_snapshot(tmp_path, jobs, {"orders_nightly_job_group": tuple(jobs)})


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckJobTestCase(
            description="a job matching a check and the model it checks publishes the model",
            select="tag:order_checks orders",
            expected_selection=["model:orders"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_matching_a_check_and_its_node_when_publishing_then_the_check_runs_with_it(
    tmp_path: Path, test_case: PythonCheckJobTestCase
) -> None:
    snapshot: dict[str, Any] = json.loads(
        python_pipeline_snapshot(tmp_path, {"nightly_orders_job": test_case.select}).to_json()
    )

    assert snapshot["documents"]["jobs"]["nightly_orders_job"]["selection"] == (
        test_case.expected_selection
    )


@pytest.mark.parametrize(
    "test_case",
    [
        PythonCheckJobRejectionTestCase(
            description="a job matching a check without the model it checks names both",
            select="tag:order_checks",
            expected_message=(
                "job nightly_orders_job: Python check 'orders_present' checks model:orders, which "
                "the selection does not include; select it too, or narrow the selector so it no "
                "longer matches the check."
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_matching_a_check_without_its_node_when_publishing_then_it_is_rejected(
    tmp_path: Path, test_case: PythonCheckJobRejectionTestCase
) -> None:
    with pytest.raises(ValueError, match=re.escape(test_case.expected_message)):
        python_pipeline_snapshot(tmp_path, {"nightly_orders_job": test_case.select})
