"""`sqb build --changed` runs exactly what consumed changed inputs, across crashes, on DuckDB and SQLite."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ChangedBuildE2ETestCase,
    ChangedCrashE2ETestCase,
    ChangedReachE2ETestCase,
    ChangedTwoUpE2ETestCase,
    ChangedWarehouse,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    CHANGED_ANY,
    CHANGED_BUILDS_REACH_FIXTURE,
    CHANGED_BUILDS_TWO_UP_FIXTURE,
    CHANGED_BUILDS_UDF_FIXTURE,
    CHANGED_BUILDS_UNQUALIFIED_FIXTURE,
    DUCKDB_CHANGED,
    SQLITE_CHANGED,
    add_changed_overlay,
    apply_changed_step,
    changed_build_output,
    changed_build_steps,
    kill_changed_build_mid_write,
    prepare_changed_project,
    run_changed_build,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

EVERY_MODEL: frozenset[str] = frozenset({"stg_orders", "stg_customers", "customer_orders"})
# Functions deploy on every build; `--changed` decides models and seeds only.
FUNCTIONS: frozenset[str] = frozenset(
    {
        "udf__order_key",
        "table_fn__customer_order_keys",
        "udf__raw_customer_count",
        "table_fn__raw_customers_from",
    }
)
# Their functions read raw.customers without declaring it, so they run on every build.
UNDECLARED_READERS: frozenset[str] = frozenset({"customers_via_table_fn", "customer_count_via_udf"})
EXPECTED_CHANGED_RUNS: tuple[tuple[str, frozenset[str]], ...] = (
    ("first --changed build", EVERY_MODEL),
    ("nothing changed", frozenset()),
    ("a new order", frozenset({"stg_orders", "customer_orders"})),
    ("a definition change", frozenset({"stg_customers", "customer_orders"})),
    ("a stale unselected upstream", frozenset({"customer_orders"})),
    ("the upstream is still stale", frozenset({"customer_orders"})),
    ("building the whole selection", frozenset({"stg_customers", "customer_orders"})),
    ("a plain build", EVERY_MODEL),
    ("after the plain build", frozenset()),
    ("full refresh", EVERY_MODEL),
)


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedBuildE2ETestCase(
            description=warehouse.name, warehouse=warehouse, expected_ran=EXPECTED_CHANGED_RUNS
        )
        for warehouse in (DUCKDB_CHANGED, SQLITE_CHANGED)
    ],
    ids=lambda case: case.description,
)
def test_given_input_changes_when_building_with_changed_then_exactly_their_consumers_run(
    test_case: ChangedBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=test_case.warehouse)
    observed: list[tuple[str, frozenset[str]]] = []
    for step in changed_build_steps(test_case.warehouse):
        apply_changed_step(project_dir, test_case.warehouse, step)
        observed.append((step.description, run_changed_build(project_dir, step.arguments)))

    assert tuple(observed) == test_case.expected_ran


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedBuildE2ETestCase(
            description="duckdb",
            warehouse=DUCKDB_CHANGED,
            expected_ran=(
                (
                    "first --changed build",
                    EVERY_MODEL | UNDECLARED_READERS | {"first_customer_order_keys"},
                ),
                ("nothing changed", UNDECLARED_READERS),
                (
                    "a new order",
                    UNDECLARED_READERS
                    | {"stg_orders", "customer_orders", "first_customer_order_keys"},
                ),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_models_calling_functions_when_building_with_changed_then_only_changed_inputs_run(
    test_case: ChangedBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=test_case.warehouse)
    add_changed_overlay(project_dir, CHANGED_BUILDS_UDF_FIXTURE)
    observed: list[tuple[str, frozenset[str]]] = []
    for step in changed_build_steps(test_case.warehouse)[: len(test_case.expected_ran)]:
        apply_changed_step(project_dir, test_case.warehouse, step)
        ran: frozenset[str] = run_changed_build(project_dir, step.arguments)
        observed.append((step.description, ran - FUNCTIONS))
    warehouse_file: Path = project_dir / test_case.warehouse.warehouse_file
    test_case.warehouse.execute(
        warehouse_file, f"INSERT INTO {test_case.warehouse.customers_table} VALUES (2, 3)"
    )
    run_changed_build(project_dir, ("--changed",))
    function_reads: list[tuple[object, ...]] = test_case.warehouse.execute(
        warehouse_file,
        "SELECT (SELECT count(*) FROM main.customers_via_table_fn), "
        "(SELECT customer_count FROM main.customer_count_via_udf)",
    )

    assert tuple(observed) == test_case.expected_ran
    assert function_reads == [(2, 2)]


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedBuildE2ETestCase(
            description="duckdb",
            warehouse=DUCKDB_CHANGED,
            expected_ran=(
                ("nothing changed", frozenset({"unqualified_customer_reader"})),
                ("customers redefined", frozenset({"customers", "unqualified_customer_reader"})),
                ("customers settled", frozenset({"unqualified_customer_reader"})),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_unqualified_name_matching_two_relations_when_building_with_changed_then_it_runs(
    test_case: ChangedBuildE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=test_case.warehouse)
    add_changed_overlay(project_dir, CHANGED_BUILDS_UNQUALIFIED_FIXTURE)
    customers_model: Path = project_dir / "models/customers.sql"
    run_changed_build(project_dir, ("--select", "+customers"))
    run_changed_build(project_dir, ("--changed",))
    nothing_changed: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    customers_model.write_text(
        customers_model.read_text(encoding="utf-8").replace(
            '__ref("stg_customers")', '__ref("stg_customers") WHERE customer_id > 100'
        ),
        encoding="utf-8",
    )
    redefined: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    # Nothing orders the reader after the undeclared `customers`, so it may read it before the
    # rebuild; it runs on every `--changed` build, so the next one reads the settled model.
    settled: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    modelled: list[tuple[object, ...]] = test_case.warehouse.execute(
        project_dir / test_case.warehouse.warehouse_file,
        "SELECT modelled FROM main.unqualified_customer_reader",
    )

    assert (
        ("nothing changed", nothing_changed),
        ("customers redefined", redefined),
        ("customers settled", settled),
    ) == test_case.expected_ran
    assert modelled == [(0,)]


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedCrashE2ETestCase(
            description=warehouse.name,
            warehouse=warehouse,
            expected_ran=EVERY_MODEL,
            expected_killed_rows=[("order_weights", "started")],
        )
        for warehouse in (DUCKDB_CHANGED, SQLITE_CHANGED)
    ],
    ids=lambda case: case.description,
)
def test_given_a_build_killed_mid_write_when_building_with_changed_then_every_interrupted_model_runs(
    test_case: ChangedCrashE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=test_case.warehouse)
    first: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    settled: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    finished, killed_rows = kill_changed_build_mid_write(project_dir, test_case.warehouse)

    assert (first, settled) == (test_case.expected_ran, frozenset())
    assert "stg_orders" in finished
    assert killed_rows == test_case.expected_killed_rows
    assert run_changed_build(project_dir, ("--changed",)) == test_case.expected_ran
    assert run_changed_build(project_dir, ("--changed",)) == frozenset()


REACH_MODELS: frozenset[str] = EVERY_MODEL | {
    "int_orders",
    "fct_revenue",
    "rpt_pnl",
    "orders_export",
    "summary",
}
STG_ORDERS_REDEFINED: str = (
    'MODEL (description "Staged orders.", materialized table);\n'
    'SELECT order_id, customer_id FROM __source("raw_orders") WHERE order_id > 0\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedReachE2ETestCase(
            description=DUCKDB_CHANGED.name,
            warehouse=DUCKDB_CHANGED,
            expected_redefined=frozenset(
                {"stg_orders", "int_orders", "orders_export", "customer_orders"}
            ),
            expected_summary="Changed: 4 of 8 selected models and seeds run; "
            "1 unchanged and 3 held back",
            expected_plan_lines=(
                "Counting: definition (source data not counted) · reach: job",
                "Changed (1)",
                "Downstream of changes (3)",
                "Held back (3)",
                "job finance_daily_job",
                "reads fct_revenue, which is held back",
            ),
            expected_job_run=frozenset({"fct_revenue", "rpt_pnl"}),
            expected_reach_all=frozenset({"summary"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_scheduled_jobs_when_building_with_changed_then_a_change_stops_at_another_jobs_edge(
    test_case: ChangedReachE2ETestCase, tmp_path: Path
) -> None:
    warehouse: ChangedWarehouse = test_case.warehouse
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=warehouse)
    add_changed_overlay(project_dir, CHANGED_BUILDS_REACH_FIXTURE)
    first: frozenset[str] = run_changed_build(project_dir, (CHANGED_ANY,))
    warehouse.execute(
        project_dir / warehouse.warehouse_file, "INSERT INTO raw.orders VALUES (2, 1, 2)"
    )
    new_data: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    (project_dir / "models/stg_orders.sql").write_text(STG_ORDERS_REDEFINED, encoding="utf-8")
    plan: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--changed"), project_dir=project_dir
    )
    redefined, build_output = changed_build_output(project_dir, ("--changed",))
    held: frozenset[str] = run_changed_build(project_dir, ("--changed",))
    job_run: frozenset[str] = run_changed_build(
        project_dir, ("--changed", "--select", "finance_daily_job")
    )
    reach_all: frozenset[str] = run_changed_build(project_dir, ("--changed", "--reach", "all"))
    refused: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--reach", "all"), project_dir=project_dir
    )

    assert first == REACH_MODELS
    assert new_data == frozenset(), "new source data is not a change by default"
    assert redefined == test_case.expected_redefined, (
        "the change stops at finance_daily_job; a job without a trigger and a model in no job run"
    )
    assert test_case.expected_summary in build_output
    assert plan.returncode == 0, plan.stdout + plan.stderr
    for line in test_case.expected_plan_lines:
        assert line in plan.stdout, plan.stdout
    assert held == frozenset(), "held-back models wait for their job, not the next --changed"
    assert job_run == test_case.expected_job_run, "running a job never holds back its own models"
    assert reach_all == test_case.expected_reach_all
    assert refused.returncode != 0
    assert "--reach needs --changed" in refused.stdout + refused.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedTwoUpE2ETestCase(
            description=DUCKDB_CHANGED.name,
            warehouse=DUCKDB_CHANGED,
            expected_crm_run=frozenset({"stg_customers", "customer_orders"}),
            expected_downstream=frozenset({"customer_sales"}),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_two_jobs_each_holding_a_redefined_upstream_when_building_changed_then_a_model_two_levels_down_runs(
    test_case: ChangedTwoUpE2ETestCase, tmp_path: Path
) -> None:
    warehouse: ChangedWarehouse = test_case.warehouse
    project_dir: Path = prepare_changed_project(tmp_path=tmp_path, warehouse=warehouse)
    add_changed_overlay(project_dir, CHANGED_BUILDS_TWO_UP_FIXTURE)
    first: frozenset[str] = run_changed_build(project_dir, (CHANGED_ANY,))
    (project_dir / "models/stg_orders.sql").write_text(STG_ORDERS_REDEFINED, encoding="utf-8")
    (project_dir / "models/stg_customers.sql").write_text(
        'MODEL (description "Staged customers.", materialized table);\n'
        'SELECT customer_id FROM __source("raw_customers") WHERE customer_id > 0\n',
        encoding="utf-8",
    )
    sales_staged: frozenset[str] = run_changed_build(
        project_dir, ("--changed", "--select", "stg_orders")
    )
    crm_run: frozenset[str] = run_changed_build(
        project_dir, ("--changed", "--select", "crm_daily_job")
    )
    downstream: frozenset[str] = run_changed_build(project_dir, ("--changed",))

    assert first == {"stg_customers", "stg_orders", "customer_orders", "customer_sales"}
    assert sales_staged == {"stg_orders"}
    assert crm_run == test_case.expected_crm_run
    assert downstream == test_case.expected_downstream, (
        "sales_daily_job holds the stg_orders change that reached customer_orders, so customer_sales "
        "runs even though crm_daily_job holds the stg_customers change too"
    )
