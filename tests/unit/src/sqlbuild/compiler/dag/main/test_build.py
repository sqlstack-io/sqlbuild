from __future__ import annotations

import json
from collections.abc import Callable
from dataclasses import replace
from pathlib import Path
from typing import TYPE_CHECKING, cast

import pytest

from sqlbuild._native import (
    CompiledObjectKey,
    DiscoveredProjectInputs,
    ProjectGraph,
    assemble_project,
    build_compile_inputs,
    build_dag_json,
    discover_project_inputs,
    project_lineage_views,
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.compiler.dag.types import NodeKind
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
)
from tests.unit.src.sqlbuild.compiler.compile._test_helpers import base_repo_files
from tests.unit.src.sqlbuild.compiler.dag.main._test_types import (
    DagArtifactTestCase,
    DagJsonTestCase,
    DagLoaderDestinationTestCase,
    DagProducedKindsTestCase,
    DagResourceNamespaceTestCase,
)
from tests.unit.src.sqlbuild.compiler.dag.main.helpers import build_dag_artifact_test_graph
from tests.unit.src.sqlbuild.compiler.graph.helpers import project_graph_from_indexes

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@pytest.mark.parametrize(
    "test_case",
    [
        DagArtifactTestCase(
            description="builds static dag nodes edges and checks",
            expected_node_ids=(
                "source:raw_orders",
                "loader:shared_order_feed",
                "seed:country_codes",
                "udf:normalize_email",
                "model:orders",
            ),
            expected_edge_pairs=(
                ("loader:shared_order_feed", "source:raw_orders"),
                ("seed:country_codes", "model:orders"),
                ("source:raw_orders", "model:orders"),
                ("udf:normalize_email", "model:orders"),
            ),
            expected_check_ids=(
                "audit:orders_audit:model:orders:order_id",
                "sql_scenario:orders_scenario",
                "sql_test:orders_test",
            ),
            expected_function_asset_key=("analytics", "normalize_email"),
            expected_seed_asset_key=("analytics", "country_codes"),
            expected_source_asset_key=("raw", "orders"),
            expected_loader_asset_key=("shared_order_feed",),
            expected_seed_tags=("reference",),
            expected_function_tags=("normalization",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_graph_when_building_dag_artifact_then_includes_assets_edges_and_checks(
    test_case: DagArtifactTestCase,
) -> None:
    graph: ProjectGraph = build_dag_artifact_test_graph()
    payload: dict[str, object] = json.loads(build_dag_json(graph=graph, project_name="dag_project"))
    nodes: list[dict[str, object]] = payload["nodes"]
    edges: list[dict[str, object]] = payload["edges"]
    checks: list[dict[str, object]] = payload["checks"]
    nodes_by_id: dict[str, dict[str, object]] = {str(node["id"]): node for node in nodes}

    assert tuple(nodes_by_id) == test_case.expected_node_ids
    assert tuple((edge["from_id"], edge["to_id"]) for edge in edges) == (
        test_case.expected_edge_pairs
    )
    assert tuple(check["id"] for check in checks) == test_case.expected_check_ids
    assert tuple(cast(list[str], nodes_by_id["udf:normalize_email"]["asset_key"])) == (
        test_case.expected_function_asset_key
    )
    assert tuple(cast(list[str], nodes_by_id["seed:country_codes"]["asset_key"])) == (
        test_case.expected_seed_asset_key
    )
    assert tuple(cast(list[str], nodes_by_id["seed:country_codes"]["tags"])) == (
        test_case.expected_seed_tags
    )
    function_target: dict[str, object] = cast(
        dict[str, object], nodes_by_id["udf:normalize_email"]["target"]
    )
    assert function_target["schema"] == "analytics_dev"
    assert function_target["logical_schema"] == "analytics"
    assert tuple(cast(list[str], nodes_by_id["source:raw_orders"]["asset_key"])) == (
        test_case.expected_source_asset_key
    )
    assert tuple(cast(list[str], nodes_by_id["loader:shared_order_feed"]["asset_key"])) == (
        test_case.expected_loader_asset_key
    )
    assert nodes_by_id["model:orders"]["materialization_type"] == "table"
    assert nodes_by_id["model:orders"]["sql"] == (
        "MODEL (materialized table);\n\nSELECT 1 AS order_id"
    )
    assert nodes_by_id["udf:normalize_email"]["sql"] == "lower(email)"
    assert tuple(cast(list[str], nodes_by_id["udf:normalize_email"]["tags"])) == (
        test_case.expected_function_tags
    )
    assert nodes_by_id["loader:shared_order_feed"]["kind"] == "loader"
    assert tuple(checks[0]["checked_asset_ids"]) == ("model:orders",)
    assert checks[0]["severity"] == "warn"
    tag_index: dict[str, frozenset[CompiledObjectKey]] = project_lineage_views(
        graph.project
    ).tag_index
    assert tag_index["reference"] == frozenset({graph.project.seeds[0].key})
    assert tag_index["normalization"] == frozenset({graph.project.functions[0].key})


@pytest.mark.parametrize(
    "test_case",
    (
        DagProducedKindsTestCase(
            description="compiled project emits canonical DAG kinds",
            expected_kinds=frozenset(
                {
                    "source",
                    "loader",
                    "seed",
                    "udf",
                    "model",
                    "sql_test",
                    "audit",
                    "scenario",
                }
            ),
            expected_enum_values=frozenset(
                {
                    "source",
                    "loader",
                    "seed",
                    "udf",
                    "table_fn",
                    "model",
                    "asset",
                    "check",
                    "sql_test",
                    "audit",
                    "scenario",
                    "python_check",
                }
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_building_dag_then_produced_kinds_are_canonical(
    test_case: DagProducedKindsTestCase,
) -> None:
    payload: dict[str, object] = json.loads(
        build_dag_json(graph=build_dag_artifact_test_graph(), project_name="dag_project")
    )
    nodes: list[dict[str, object]] = payload["nodes"]
    checks: list[dict[str, object]] = payload["checks"]
    produced_kinds: frozenset[str] = frozenset(str(item["kind"]) for item in (*nodes, *checks))

    assert produced_kinds == test_case.expected_kinds
    canonical_kinds: frozenset[str] = frozenset(kind.value for kind in NodeKind)
    assert canonical_kinds == test_case.expected_enum_values
    assert produced_kinds <= canonical_kinds


@pytest.mark.parametrize(
    "test_case",
    (
        DagResourceNamespaceTestCase(
            description="serializes resource defaults as preserved logical and physical namespaces",
            expected_seed_namespace=("seed_db", "seed_schema", "seed_db", "seed_schema"),
            expected_function_namespace=(
                "function_db",
                "function_schema",
                "function_db",
                "function_schema",
            ),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_resource_namespace_defaults_when_building_dag_then_targets_retain_namespaces(
    test_case: DagResourceNamespaceTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        base_repo_files()
        | {
            "sqlbuild_project.toml": """
name = "demo"
adapter = "duckdb"
default_target = "dev"

[defaults]
database = "model_db"
schema = "model_schema"
seed_database = "seed_db"
seed_schema = "seed_schema"
function_database = "function_db"
function_schema = "function_schema"

[targets.dev]
database = "preserve"
schema = "preserve"
""".strip()
            + "\n",
            "seeds/schema.yml": """
seeds:
  - name: country_codes
    columns:
      - name: code
        type: VARCHAR
""".strip()
            + "\n",
            "seeds/country_codes.csv": "code\nUS\n",
            "functions/sql/normalize.sql": "FUNCTION (returns VARCHAR);\n\n'normalized'\n",
        },
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )
    project: CompiledProjectView = assemble_project(inputs=inputs)
    graph: ProjectGraph = project_graph_from_indexes(
        project=project,
        upstream_deps={},
        downstream_deps={},
        tag_index={},
        path_index={},
        all_keys={},
    )

    payload: dict[str, object] = json.loads(build_dag_json(graph=graph, project_name="demo"))
    nodes: dict[str, dict[str, object]] = {
        str(node["id"]): node for node in cast(list[dict[str, object]], payload["nodes"])
    }
    seed_target: dict[str, object] = cast(dict[str, object], nodes["seed:country_codes"]["target"])
    function_target: dict[str, object] = cast(dict[str, object], nodes["udf:normalize"]["target"])
    assert (
        seed_target["database"],
        seed_target["schema"],
        seed_target["logical_database"],
        seed_target["logical_schema"],
    ) == test_case.expected_seed_namespace
    assert (
        function_target["database"],
        function_target["schema"],
        function_target["logical_database"],
        function_target["logical_schema"],
    ) == test_case.expected_function_namespace


@pytest.mark.parametrize(
    "test_case",
    [
        DagJsonTestCase(
            description="serializes dag artifact as compact public json",
            expected_version=1,
            expected_project_name="dag_project",
            expected_node_count=5,
            expected_absent_fragments=(
                '"description": null',
                '"tags": []',
                '"arguments": []',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dag_artifact_when_formatting_json_then_serializes_public_shape(
    test_case: DagJsonTestCase,
) -> None:
    rendered_json: str = build_dag_json(
        graph=build_dag_artifact_test_graph(),
        project_name=test_case.expected_project_name,
    )
    payload: dict[str, object] = json.loads(rendered_json)

    assert payload["version"] == test_case.expected_version
    assert payload["project_name"] == test_case.expected_project_name
    assert len(payload["nodes"]) == test_case.expected_node_count
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in rendered_json
    assert "query_sql" not in json.dumps(payload)
    assert "fingerprint" not in json.dumps(payload)


@pytest.mark.parametrize(
    "test_case",
    (
        DagLoaderDestinationTestCase(
            description="three-part loader destination overrides DAG target defaults",
            destination="loader_db.loader_schema.shared_orders",
            expected_target_parts=("loader_db", "loader_schema", "shared_orders"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_qualified_loader_destination_when_building_dag_then_parses_loader_target(
    test_case: DagLoaderDestinationTestCase,
) -> None:
    graph: ProjectGraph = build_dag_artifact_test_graph()
    graph = ProjectGraph(
        native=graph.native,
        project=project_with(
            project=graph.project,
            effective_target_database="default_db",
            effective_target_schema="default_schema",
            loader_functions=(
                replace(
                    graph.project.loader_functions[0],
                    destination=test_case.destination,
                ),
                *graph.project.loader_functions[1:],
            ),
        ),
    )

    payload: dict[str, object] = json.loads(build_dag_json(graph=graph, project_name="demo"))
    nodes: dict[str, dict[str, object]] = {
        str(node["id"]): node for node in cast(list[dict[str, object]], payload["nodes"])
    }
    target: dict[str, object] = cast(dict[str, object], nodes["loader:shared_order_feed"]["target"])

    assert (
        target["database"],
        target["schema"],
        target["name"],
    ) == test_case.expected_target_parts
