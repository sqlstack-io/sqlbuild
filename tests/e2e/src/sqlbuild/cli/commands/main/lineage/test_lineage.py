from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.lineage._test_types import (
    ColumnLineageCacheCliTestCase,
    DiamondLineageTreeCliTestCase,
    LineageCacheCliTestCase,
    LineageCliTestCase,
    LineageErrorCliTestCase,
    SharedSelectorCliTestCase,
    SharedSelectorErrorCliTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.lineage.helpers import (
    DIAMOND_EDGE_COUNT,
    DIAMOND_LAYERS,
    diamond_model_names,
    lineage_node_ids,
    prepare_diamond_lineage_project,
    prepare_lineage_cache_project,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_waffle_shop,
    run_sqb,
)

NATIVE_CACHE_NAMESPACE_SUFFIX: str = "-native-v1"

_TREE_LINE: re.Pattern[str] = re.compile(r"^[│ ]*[├└]── .*$", re.MULTILINE)
_EXPANDED_DIAMOND_NODE: re.Pattern[str] = re.compile(
    r"^[│ ]*[├└]── (?![^\n]*\(already shown\))[^\n]*?(orders_(?:left_|right_)?\d+(?:\.amount)?)",
    re.MULTILINE,
)
_LINEAGE_CACHE_RELATIVE_PATH: Path = (
    Path("target/cache")
    / f"lineage{NATIVE_CACHE_NAMESPACE_SUFFIX}"
    / "v1"
    / "structural-graph.sqlite3"
)


@pytest.mark.parametrize(
    "test_case",
    (
        LineageCliTestCase(
            description="directional expansion preserves exclusions",
            command=(
                "lineage",
                "--select",
                "fact_orders",
                "--exclude",
                "stg_orders",
                "--direction",
                "upstream",
                "--depth",
                "1",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=("model:fact_orders",),
            expected_edge_ids=(),
        ),
        LineageCliTestCase(
            description="tag selections expand with depth",
            command=(
                "lineage",
                "--select",
                "tag:finance",
                "--direction",
                "upstream",
                "--depth",
                "1",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=("model:fact_orders", "model:stg_orders"),
            expected_edge_ids=("model:stg_orders->model:fact_orders",),
        ),
        LineageCliTestCase(
            description="path selections expand with depth and tag exclusions",
            command=(
                "lineage",
                "--select",
                "path:sql/models/staging",
                "--exclude",
                "tag:finance",
                "--direction",
                "downstream",
                "--depth",
                "1",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=("model:stg_customers", "model:stg_orders"),
            expected_edge_ids=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_directional_selectors_when_expanding_then_depth_and_exclusions_are_respected(
    test_case: LineageCliTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_lineage_cache_project(tmp_path=tmp_path)
    staging_dir: Path = project_dir / "sql" / "models" / "staging"
    staging_dir.mkdir()
    for name in ("stg_orders.sql", "stg_customers.sql"):
        (project_dir / "sql" / "models" / name).rename(staging_dir / name)
    (project_dir / "sql" / "models" / "fact_orders.sql").write_text(
        'MODEL (description "Test model fact_orders.", materialized view, tags [finance]);\nSELECT * FROM __ref("stg_orders")\n',
        encoding="utf-8",
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    assert lineage_node_ids(payload=payload) == test_case.expected_node_ids


@pytest.mark.parametrize(
    "test_case",
    [
        LineageCliTestCase(
            description="renders upstream fact orders lineage json without warehouse tables",
            command=("lineage", "fact_orders", "--format", "json"),
            expected_exit_code=0,
            expected_node_ids=(
                "model:fact_orders",
                "model:stg_orders",
                "model:stg_payments",
                "seed:waffle_types",
                "source:raw_orders",
                "source:raw_payments",
                "udf:is_completed_order",
                "udf:is_completed_order_py",
            ),
            expected_edge_ids=(
                "udf:is_completed_order->model:fact_orders",
                "udf:is_completed_order_py->model:fact_orders",
                "model:stg_orders->model:fact_orders",
                "seed:waffle_types->model:fact_orders",
                "model:stg_payments->model:fact_orders",
                "source:raw_orders->model:stg_orders",
                "source:raw_payments->model:stg_payments",
            ),
        ),
        LineageCliTestCase(
            description="renders path-between selector lineage as json",
            command=(
                "lineage",
                "--select",
                "fact_orders~daily_activity_rollup",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=(
                "model:daily_activity_rollup",
                "model:fact_orders",
                "model:hourly_order_activity",
                "udf:is_completed_order",
                "udf:is_completed_order_py",
            ),
            expected_edge_ids=(
                "model:hourly_order_activity->model:daily_activity_rollup",
                "udf:is_completed_order->model:fact_orders",
                "udf:is_completed_order_py->model:fact_orders",
                "model:fact_orders->model:hourly_order_activity",
            ),
        ),
        LineageCliTestCase(
            description="unions direct parents of several targets",
            command=(
                "lineage",
                "fact_orders",
                "dim_customers",
                "--direction",
                "upstream",
                "--depth",
                "1",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=(
                "model:dim_customers",
                "model:fact_orders",
                "model:stg_customers",
                "model:stg_orders",
                "model:stg_payments",
                "seed:waffle_types",
                "udf:is_completed_order",
                "udf:is_completed_order_py",
            ),
            expected_edge_ids=(
                "model:stg_customers->model:dim_customers",
                "model:stg_orders->model:dim_customers",
                "model:stg_payments->model:dim_customers",
                "udf:is_completed_order->model:fact_orders",
                "udf:is_completed_order_py->model:fact_orders",
                "model:stg_orders->model:fact_orders",
                "seed:waffle_types->model:fact_orders",
                "model:stg_payments->model:fact_orders",
            ),
        ),
        LineageCliTestCase(
            description="accepts a printed kind-prefixed target",
            command=("lineage", "model:fact_orders", "--depth", "1", "--format", "json"),
            expected_exit_code=0,
            expected_node_ids=(
                "model:fact_orders",
                "model:stg_orders",
                "model:stg_payments",
                "seed:waffle_types",
                "udf:is_completed_order",
                "udf:is_completed_order_py",
            ),
            expected_edge_ids=(
                "udf:is_completed_order->model:fact_orders",
                "udf:is_completed_order_py->model:fact_orders",
                "model:stg_orders->model:fact_orders",
                "seed:waffle_types->model:fact_orders",
                "model:stg_payments->model:fact_orders",
            ),
        ),
        LineageCliTestCase(
            description="expands a selection by direction and depth",
            command=(
                "lineage",
                "--select",
                "stg_orders",
                "--direction",
                "downstream",
                "--depth",
                "1",
                "--format",
                "json",
            ),
            expected_exit_code=0,
            expected_node_ids=(
                "model:daily_order_partitioned",
                "model:daily_revenue",
                "model:dim_customers",
                "model:fact_orders",
                "model:scenario_order_prices",
                "model:stg_orders",
            ),
            expected_edge_ids=(
                "model:stg_orders->model:daily_order_partitioned",
                "model:stg_orders->model:daily_revenue",
                "model:stg_orders->model:dim_customers",
                "model:stg_orders->model:fact_orders",
                "model:stg_orders->model:scenario_order_prices",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_lineage_command_when_running_then_outputs_expected_json_graph(
    test_case: LineageCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    node_ids: tuple[str, ...] = tuple(node["id"] for node in payload["nodes"])  # type: ignore[index]
    edge_ids: tuple[str, ...] = tuple(
        f"{edge['from']}->{edge['to']}"
        for edge in payload["edges"]  # type: ignore[index]
    )
    assert node_ids == test_case.expected_node_ids
    assert edge_ids == test_case.expected_edge_ids


@pytest.mark.parametrize(
    "test_case",
    (
        LineageCacheCliTestCase(
            description="unchanged project reuses structural lineage cache",
            command=("lineage", "fact_orders", "--format", "json"),
            expected_node_id="model:stg_orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_unchanged_project_when_lineage_runs_twice_then_reuses_structural_cache(
    test_case: LineageCacheCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_lineage_cache_project(tmp_path=tmp_path)

    first: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    assert first.returncode == 0, first.stdout + first.stderr
    cache_path: Path = project_dir / _LINEAGE_CACHE_RELATIVE_PATH
    first_modified_ns: int = cache_path.stat().st_mtime_ns
    second: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert second.returncode == 0, second.stdout + second.stderr
    assert json.loads(first.stdout) == json.loads(second.stdout)
    assert test_case.expected_node_id in lineage_node_ids(payload=json.loads(second.stdout))
    assert cache_path.stat().st_mtime_ns == first_modified_ns


@pytest.mark.parametrize(
    "test_case",
    (
        LineageCacheCliTestCase(
            description="authored dependency change invalidates structural lineage cache",
            command=("lineage", "fact_orders", "--json"),
            expected_node_id="model:stg_customers",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_authored_dependency_change_when_lineage_runs_then_rebuilds_structural_cache(
    test_case: LineageCacheCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_lineage_cache_project(tmp_path=tmp_path)
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    assert first.returncode == 0, first.stdout + first.stderr
    (project_dir / "sql/models/fact_orders.sql").write_text(
        "MODEL (description 'Test model fact_orders.', materialized view);\n\n"
        'SELECT orders.order_id FROM __ref("stg_orders") AS orders\n'
        'JOIN __ref("stg_customers") AS customers USING (customer_id)\n',
        encoding="utf-8",
    )

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert second.returncode == 0, second.stdout + second.stderr
    assert test_case.expected_node_id in lineage_node_ids(payload=json.loads(second.stdout))


@pytest.mark.parametrize(
    "test_case",
    (
        LineageCacheCliTestCase(
            description="corrupt structural cache falls back to graph rebuild",
            command=("lineage", "fact_orders", "--format", "json"),
            expected_node_id="model:stg_orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_corrupt_structural_cache_when_lineage_runs_then_rebuilds_safely(
    test_case: LineageCacheCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_lineage_cache_project(tmp_path=tmp_path)
    first: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )
    assert first.returncode == 0, first.stdout + first.stderr
    cache_path: Path = project_dir / _LINEAGE_CACHE_RELATIVE_PATH
    cache_path.write_bytes(b"not a sqlite database")

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert second.returncode == 0, second.stdout + second.stderr
    assert test_case.expected_node_id in lineage_node_ids(payload=json.loads(second.stdout))
    assert cache_path.read_bytes().startswith(b"SQLite format 3")


@pytest.mark.parametrize(
    "test_case",
    (
        ColumnLineageCacheCliTestCase(
            description="column target retains analyzed lineage path",
            command=("lineage", "fact_orders.order_id", "--format", "json"),
            expected_source_resource="stg_orders",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_column_target_when_lineage_runs_then_bypasses_structural_cache(
    test_case: ColumnLineageCacheCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_lineage_cache_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stdout + result.stderr
    payload: dict[str, object] = json.loads(result.stdout)
    trace: list[dict[str, object]] = payload["trace"]  # type: ignore[assignment]
    source: dict[str, object] = trace[0]["source"]  # type: ignore[assignment]
    assert source["resource_name"] == test_case.expected_source_resource
    assert not (project_dir / _LINEAGE_CACHE_RELATIVE_PATH).exists()


@pytest.mark.parametrize(
    "test_case",
    (
        LineageErrorCliTestCase(
            description="graph operator target explains direction and depth",
            command=("lineage", "1+fact_orders"),
            expected_fragments=("C305", "--direction", "--depth"),
        ),
        LineageErrorCliTestCase(
            description="kind prefix must match the resource",
            command=("lineage", "source:fact_orders"),
            expected_fragments=("C305", "source:fact_orders"),
        ),
        LineageErrorCliTestCase(
            description="column target cannot be combined with other targets",
            command=("lineage", "fact_orders.order_id", "dim_customers"),
            expected_fragments=("C320",),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_lineage_target_when_running_then_explains_the_error(
    test_case: LineageErrorCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 1, result.stdout + result.stderr
    for fragment in test_case.expected_fragments:
        assert fragment in result.stderr, result.stderr


@pytest.mark.parametrize(
    "test_case",
    (
        DiamondLineageTreeCliTestCase(
            description="downstream tree expands each shared model once",
            command=("--no-color", "lineage", "orders_0", "--direction", "downstream"),
            expected_expanded_names=diamond_model_names()[1:],
            expected_max_lines=DIAMOND_EDGE_COUNT,
        ),
        DiamondLineageTreeCliTestCase(
            description="upstream tree expands each shared model once",
            command=(
                "--no-color",
                "lineage",
                f"orders_{DIAMOND_LAYERS}",
                "--direction",
                "upstream",
            ),
            expected_expanded_names=diamond_model_names()[:-1],
            expected_max_lines=DIAMOND_EDGE_COUNT,
        ),
        DiamondLineageTreeCliTestCase(
            description="both-direction tree expands each shared model once",
            command=(
                "--no-color",
                "lineage",
                f"orders_{DIAMOND_LAYERS // 2}",
                "--direction",
                "both",
            ),
            expected_expanded_names=(
                diamond_model_names()[: 3 * (DIAMOND_LAYERS // 2)]
                + diamond_model_names()[3 * (DIAMOND_LAYERS // 2) + 1 :]
            ),
            expected_max_lines=DIAMOND_EDGE_COUNT,
        ),
        DiamondLineageTreeCliTestCase(
            description="upstream column trace expands each shared column once",
            command=(
                "--no-color",
                "lineage",
                f"orders_{DIAMOND_LAYERS}.amount",
                "--direction",
                "upstream",
            ),
            expected_expanded_names=(
                f"orders_left_{DIAMOND_LAYERS}.amount",
                f"orders_right_{DIAMOND_LAYERS}.amount",
                f"orders_{DIAMOND_LAYERS - 1}.amount",
            ),
            expected_max_lines=DIAMOND_EDGE_COUNT,
        ),
        DiamondLineageTreeCliTestCase(
            description="downstream column trace expands each shared column once",
            command=(
                "--no-color",
                "lineage",
                "orders_0.amount",
                "--direction",
                "downstream",
            ),
            expected_expanded_names=(
                "orders_left_1.amount",
                "orders_right_1.amount",
                "orders_1.amount",
            ),
            expected_max_lines=DIAMOND_EDGE_COUNT,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_diamond_dependencies_when_rendering_lineage_tree_then_output_stays_linear(
    test_case: DiamondLineageTreeCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_diamond_lineage_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stdout + result.stderr
    tree_lines: list[str] = _TREE_LINE.findall(result.stdout)
    expanded_nodes: list[str] = _EXPANDED_DIAMOND_NODE.findall(result.stdout)
    assert len(tree_lines) <= test_case.expected_max_lines
    assert len(expanded_nodes) == len(set(expanded_nodes))
    assert set(test_case.expected_expanded_names) <= set(expanded_nodes)


@pytest.mark.parametrize(
    "test_case",
    (
        SharedSelectorCliTestCase(
            description="models-rooted path",
            selector="path:sql/models/staging",
            expected_node_ids=("model:stg_customers", "model:stg_orders", "model:stg_payments"),
            expected_selected_models=3,
            expected_selected_functions=0,
        ),
        SharedSelectorCliTestCase(
            description="name glob",
            selector="stg_*",
            expected_node_ids=("model:stg_customers", "model:stg_orders", "model:stg_payments"),
            expected_selected_models=3,
            expected_selected_functions=0,
        ),
        SharedSelectorCliTestCase(
            description="path and tag intersection",
            selector="path:sql/models/marts,tag:acceptance",
            expected_node_ids=(
                "model:daily_activity_rollup",
                "model:hourly_activity_with_daily_context",
                "udf:is_completed_order",
                "udf:is_completed_order_py",
            ),
            expected_selected_models=2,
            expected_selected_functions=2,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_selector_when_running_lineage_and_compile_then_both_select_the_same_resources(
    test_case: SharedSelectorCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    lineage: subprocess.CompletedProcess[str] = run_sqb(
        command=("lineage", "--select", test_case.selector, "--format", "json"),
        project_dir=project_dir,
    )
    compiled: subprocess.CompletedProcess[str] = run_sqb(
        command=("compile", "--select", test_case.selector, "--json"),
        project_dir=project_dir,
    )

    assert lineage.returncode == 0, lineage.stdout + lineage.stderr
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    summary: dict[str, int] = json.loads(compiled.stdout)["summary"]
    assert lineage_node_ids(payload=json.loads(lineage.stdout)) == test_case.expected_node_ids
    assert summary["selected_models"] == test_case.expected_selected_models
    assert summary["selected_functions"] == test_case.expected_selected_functions


@pytest.mark.parametrize(
    "test_case",
    (
        SharedSelectorErrorCliTestCase(
            description="path without the models root",
            selector="path:staging",
            expected_fragment="error[S012]",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_invalid_selector_when_running_lineage_and_compile_then_both_report_it(
    test_case: SharedSelectorErrorCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waffle_shop(tmp_path)

    results: tuple[subprocess.CompletedProcess[str], ...] = tuple(
        run_sqb(command=command, project_dir=project_dir)
        for command in (
            ("--no-color", "lineage", "--select", test_case.selector),
            ("--no-color", "compile", "--select", test_case.selector),
        )
    )

    for result in results:
        assert result.returncode == 1, result.stdout + result.stderr
        assert test_case.expected_fragment in result.stdout + result.stderr
