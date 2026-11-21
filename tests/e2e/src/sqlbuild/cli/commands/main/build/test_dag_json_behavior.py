"""E2E tests for dag --json behavior."""

from __future__ import annotations

import json
import subprocess
from itertools import chain
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    DagJsonBuildE2ETestCase,
    DiamondDagE2ETestCase,
    DiamondDagJsonE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    ALLOW_ROUND_TRIPS,
    SCORING_TASK,
    round_trip_project_files,
)
from tests.e2e.src.sqlbuild.cli.commands.main.lineage.helpers import (
    DIAMOND_EDGE_COUNT,
    diamond_model_names,
    prepare_diamond_lineage_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    prepare_waffle_shop,
    run_sqb,
)


@pytest.mark.parametrize(
    "test_case",
    [
        DagJsonBuildE2ETestCase(
            description="dag json reports static graph without warehouse planning",
            command=("dag", "--json"),
            expected_exit_code=0,
            expected_project_name="waffle_shop",
            expected_node_ids=(
                "source:raw_orders",
                "seed:waffle_types",
                "udf:is_completed_order",
                "model:fact_orders",
            ),
            expected_edge_pairs=(
                ("source:raw_orders", "model:stg_orders"),
                ("seed:waffle_types", "model:fact_orders"),
                ("udf:is_completed_order", "model:fact_orders"),
            ),
            expected_check_ids=(
                "sql_test:test_fact_orders",
                "audit:not_null:model:fact_orders:order_id",
                "sql_scenario:daily_revenue_minimal",
            ),
            expected_absent_fragments=(
                '"description": null',
                '"tags": []',
                '"arguments": []',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_waffle_shop_when_running_dag_json_then_it_reports_static_graph(
    test_case: DagJsonBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    nodes: list[dict[str, object]] = payload["nodes"]
    edges: list[dict[str, object]] = payload["edges"]
    checks: list[dict[str, object]] = payload["checks"]
    nodes_by_id: dict[str, dict[str, object]] = {str(node["id"]): node for node in nodes}
    edge_pairs: set[tuple[str, str]] = {
        (str(edge["from_id"]), str(edge["to_id"])) for edge in edges
    }
    check_ids: set[str] = {str(check["id"]) for check in checks}

    assert payload["project_name"] == test_case.expected_project_name
    for node_id in test_case.expected_node_ids:
        assert node_id in nodes_by_id
    for edge_pair in test_case.expected_edge_pairs:
        assert edge_pair in edge_pairs
    for check_id in test_case.expected_check_ids:
        assert check_id in check_ids
    assert nodes_by_id["model:fact_orders"]["asset_key"] == ["main", "fact_orders"]
    assert nodes_by_id["udf:is_completed_order"]["asset_key"] == [
        "main",
        "is_completed_order",
    ]
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in result.stdout
    assert "query_sql" not in result.stdout
    assert "action" not in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DagJsonBuildE2ETestCase(
            description="a task upstream of a declared loader points at the loader's source",
            command=("dag", "--json"),
            expected_exit_code=0,
            expected_project_name="partner_scoring",
            expected_node_ids=(
                "model:orders",
                "asset:score_orders",
                "source:partner_scores",
                "model:scored_orders",
            ),
            expected_edge_pairs=(
                ("model:orders", "asset:score_orders"),
                ("asset:score_orders", "source:partner_scores"),
                ("source:partner_scores", "model:scored_orders"),
            ),
            expected_check_ids=(),
            expected_absent_fragments=("loader:partner_scores",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_round_trip_when_running_dag_json_then_every_edge_ends_at_a_node(
    test_case: DagJsonBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="partner_scoring",
        repo_files=round_trip_project_files(
            project_settings=ALLOW_ROUND_TRIPS, task_body=SCORING_TASK
        ),
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    node_ids: set[str] = {str(node["id"]) for node in payload["nodes"]}
    edge_pairs: set[tuple[str, str]] = {
        (str(edge["from_id"]), str(edge["to_id"])) for edge in payload["edges"]
    }
    assert payload["project_name"] == test_case.expected_project_name, test_case.description
    assert set(test_case.expected_node_ids) <= node_ids, test_case.description
    assert set(test_case.expected_edge_pairs) <= edge_pairs, test_case.description
    assert set(chain.from_iterable(edge_pairs)) <= node_ids, edge_pairs
    for fragment in test_case.expected_absent_fragments:
        assert fragment not in result.stdout, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        DiamondDagE2ETestCase(
            description="dag text summarizes a layered diamond graph",
            command=("--no-color", "dag"),
            expected_fragments=(
                f"DAG ready ({len(diamond_model_names())} nodes, "
                f"{DIAMOND_EDGE_COUNT} edges, 0 checks)",
            ),
            expected_max_lines_per_node=1,
        ),
        DiamondDagE2ETestCase(
            description="dag json lists a layered diamond graph once per node and edge",
            command=("--no-color", "dag", "--json"),
            expected_fragments=('"project_name": "diamond_lineage"',),
            expected_max_lines_per_node=40,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_layered_diamonds_when_running_dag_then_output_stays_linear(
    test_case: DiamondDagE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diamond_lineage_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in result.stdout, result.stdout
    assert len(result.stdout.splitlines()) <= test_case.expected_max_lines_per_node * len(
        diamond_model_names()
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DiamondDagJsonE2ETestCase(
            description="dag json lists every diamond model and dependency exactly once",
            expected_node_ids=tuple(f"model:{name}" for name in diamond_model_names()),
            expected_edge_count=DIAMOND_EDGE_COUNT,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_layered_diamonds_when_running_dag_json_then_lists_each_node_and_edge_once(
    test_case: DiamondDagJsonE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diamond_lineage_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "dag", "--json"), project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, list[dict[str, object]]] = json.loads(result.stdout)
    node_ids: list[str] = [str(node["id"]) for node in payload["nodes"]]
    edge_pairs: list[tuple[str, str]] = [
        (str(edge["from_id"]), str(edge["to_id"])) for edge in payload["edges"]
    ]
    assert sorted(node_ids) == sorted(test_case.expected_node_ids)
    assert len(edge_pairs) == len(set(edge_pairs)) == test_case.expected_edge_count
