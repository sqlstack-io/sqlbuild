"""`sqb build --changed` runs exactly what consumed changed inputs, across crashes, on DuckDB and SQLite."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    ChangedBuildE2ETestCase,
    ChangedCrashE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    DUCKDB_CHANGED,
    SQLITE_CHANGED,
    add_changed_function_calls,
    apply_changed_step,
    changed_build_steps,
    kill_changed_build_mid_write,
    prepare_changed_project,
    run_changed_build,
)

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
    add_changed_function_calls(project_dir)
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
