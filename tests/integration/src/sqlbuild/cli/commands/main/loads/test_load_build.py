from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.loads._test_types import (
    LoadBuildTestCase,
    LoadPlanTestCase,
    MissingTableTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.loads.helpers import (
    CliRun,
    land_file,
    query,
    run_sqb,
    write_project,
)

_SOURCES: str = """load:
  name: shop
  kind: files
  connection: landing
  path: orders/
  format: csv
  write_strategy: merge
  unique_key: [id]
  order: path
  table_name: shop__orders
sources:
  - name: shop__orders
    description: Orders
    columns:
      - name: id
        type: INTEGER
"""
_ORDER_NAMES: str = """MODEL (
  materialized view,
  description "Order names"
);

select id, name from __source("shop__orders")
"""
_REFUND_IDS: str = """MODEL (
  materialized view,
  description "Refund ids"
);

select id from __source("shop__refunds")
"""


@pytest.mark.parametrize(
    "test_case",
    [
        LoadPlanTestCase(
            description="a model's plan runs the load for the source it reads",
            selector="order_names",
            expected_load=("shop", False, ("shop__orders",)),
            expected_line_fragment="merge on id",
        ),
        LoadPlanTestCase(
            description="selecting the load runs every table it loads",
            selector="load:shop",
            expected_load=("shop", True, ()),
            expected_line_fragment="raw.shop__orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_load_when_planning_then_the_plan_shows_sqlloads_plan_of_it(
    test_case: LoadPlanTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_project(
        root=tmp_path, sources_yml=_SOURCES, models={"order_names": _ORDER_NAMES}
    )
    land_file(root=tmp_path, name="1.csv", text="id,name\n1,first\n")

    run: CliRun = run_sqb(
        project_dir=project_dir, args=("plan", "--json", test_case.selector), capsys=capsys
    )

    plan: dict[str, Any] = json.loads(run.stdout)
    load: dict[str, Any] = plan["loads"][0]
    assert run.exit_code == 0, run.output
    assert (load["name"], load["every_table"], tuple(load["sources"])) == test_case.expected_load
    assert any(test_case.expected_line_fragment in line for line in load["plan"]), load["plan"]


@pytest.mark.parametrize(
    "test_case",
    [
        LoadBuildTestCase(
            description="a build loads the files, then the model reads them",
            landed=((("1.csv", "id,name\n1,first\n2,second\n"),),),
            expected_rows=((1, "first"), (2, "second")),
        ),
        LoadBuildTestCase(
            description="a later build merges only the new file",
            landed=(
                (("1.csv", "id,name\n1,first\n2,second\n"),),
                (("2.csv", "id,name\n2,second again\n3,third\n"),),
            ),
            expected_rows=((1, "first"), (2, "second again"), (3, "third")),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_model_reading_a_loaded_source_when_building_then_the_load_runs_first(
    test_case: LoadBuildTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_project(
        root=tmp_path, sources_yml=_SOURCES, models={"order_names": _ORDER_NAMES}
    )
    runs: list[CliRun] = []
    for files in test_case.landed:
        for name, text in files:
            land_file(root=tmp_path, name=name, text=text)
        runs.append(run_sqb(project_dir=project_dir, args=("build",), capsys=capsys))

    rows: list[tuple[Any, ...]] = query(
        project_dir=project_dir, sql="SELECT id, name FROM dev.order_names ORDER BY id"
    )
    results: list[tuple[Any, ...]] = query(
        project_dir=project_dir,
        sql="SELECT node_name, status FROM dev._sqlbuild_node_results WHERE node_type = 'load'",
    )

    assert [run.exit_code for run in runs] == [0] * len(runs), [run.output for run in runs]
    assert tuple(rows) == test_case.expected_rows, test_case.description
    assert results == [("shop", "success")] * len(runs), test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        MissingTableTestCase(
            description="planning names the source the load doesn't fill",
            command="plan",
            expected_fragment=(
                "error[S304]: load 'shop' loads no table into 'shop__refunds'; its tables "
                "load into shop__orders"
            ),
        ),
        MissingTableTestCase(
            description="building fails the load before it loads anything",
            command="build",
            expected_fragment=(
                "load 'shop' loads no table into 'shop__refunds'; its tables load into shop__orders"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_model_reading_a_table_the_load_lacks_when_running_then_it_fails_by_name(
    test_case: MissingTableTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_project(
        root=tmp_path, sources_yml=_SOURCES, models={"refund_ids": _REFUND_IDS}
    )
    land_file(root=tmp_path, name="1.csv", text="id,name\n1,first\n")

    run: CliRun = run_sqb(project_dir=project_dir, args=(test_case.command,), capsys=capsys)

    assert run.exit_code != 0, run.output
    assert test_case.expected_fragment in run.output, run.output
