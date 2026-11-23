"""Real CLI builds of the cases the removed housekeeping statements used to guard."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.commands.main.statement_counts._test_types import (
    FingerprintLossTestCase,
    FingerprintWriteTestCase,
    LeftoverRelationTestCase,
    ScenarioLeftoverTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.statement_counts.helpers import (
    add_post_hook,
    add_upstream_reads,
    execute_sql,
    fingerprint_rows,
    intermediate_relations,
    revise_model,
    row_counts,
    run_counted,
    scenario_relation_kinds,
    scenario_relation_name,
    write_orders_project,
)

_FINGERPRINT_SETUP: str = "CREATE TABLE IF NOT EXISTS _sqlbuild_fingerprints"
_FINGERPRINT_INSERT: str = "INSERT _sqlbuild_fingerprints"
_ALL_MODELS: tuple[str, ...] = ("orders_daily", "orders_merge", "orders_table", "orders_view")
_RETAINED: tuple[tuple[str, str], ...] = (
    ("orders_table", "BASE TABLE"),
    ("orders_view", "VIEW"),
    ("raw_orders", "BASE TABLE"),
)


@pytest.mark.parametrize(
    "test_case",
    [
        FingerprintWriteTestCase(
            description="an unchanged build appends no row and sets up no table",
            edited_models=(),
            expected_setups=0,
            expected_rows=dict.fromkeys(_ALL_MODELS, 1),
        ),
        FingerprintWriteTestCase(
            description="a changed model records its new identity once, as it builds",
            edited_models=("orders_view",),
            expected_setups=1,
            expected_rows={**dict.fromkeys(_ALL_MODELS, 1), "orders_view": 2},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_second_build_when_identities_are_unchanged_then_only_changed_models_append(
    test_case: FingerprintWriteTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_orders_project(project_dir=tmp_path)
    first_code, first_output, _, _ = run_counted(
        project_dir=project_dir, args=("build",), capsys=capsys
    )
    name: str
    for name in test_case.edited_models:
        revise_model(project_dir, name)

    code, output, counts, _ = run_counted(project_dir=project_dir, args=("build",), capsys=capsys)
    third_code, third_output, third_counts, _ = run_counted(
        project_dir=project_dir, args=("build",), capsys=capsys
    )

    assert (first_code, code, third_code) == (0, 0, 0), first_output + output + third_output
    assert counts.get(_FINGERPRINT_SETUP, 0) == test_case.expected_setups
    assert counts.get(_FINGERPRINT_INSERT, 0) == test_case.expected_setups
    assert third_counts.get(_FINGERPRINT_INSERT, 0) == test_case.expected_later_inserts
    assert fingerprint_rows(project_dir) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        FingerprintLossTestCase(
            description="a view's post hook drops the table after every model it reads recorded",
            edited_models=("orders_table",),
            hooked_model="orders_view",
            hooked_model_reads=("orders_daily", "orders_merge"),
            expected_setups=2,
            expected_rows_after_loss={"orders_view": 1},
            expected_models_after_rebuild=_ALL_MODELS,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_fingerprint_table_dropped_mid_build_when_building_then_later_models_record(
    test_case: FingerprintLossTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_orders_project(project_dir=tmp_path)
    _ = (project_dir / "sqlbuild_project.toml").write_text(
        (project_dir / "sqlbuild_project.toml").read_text(encoding="utf-8")
        + "\n[settings]\nconcurrency = 1\n",
        encoding="utf-8",
    )
    first_code, first_output, _, _ = run_counted(
        project_dir=project_dir, args=("build",), capsys=capsys
    )
    name: str
    for name in test_case.edited_models:
        revise_model(project_dir, name)
    original: str = add_post_hook(
        project_dir, test_case.hooked_model, "DROP TABLE main._sqlbuild_fingerprints"
    )
    add_upstream_reads(project_dir, test_case.hooked_model, test_case.hooked_model_reads)

    code, output, counts, _ = run_counted(project_dir=project_dir, args=("build",), capsys=capsys)
    after_loss: dict[str, int] = fingerprint_rows(project_dir)
    _ = (project_dir / "sql" / "models" / f"{test_case.hooked_model}.sql").write_text(
        original, encoding="utf-8"
    )
    third_code, third_output, _, _ = run_counted(
        project_dir=project_dir, args=("build",), capsys=capsys
    )

    assert (first_code, code, third_code) == (0, 0, 0), first_output + output + third_output
    assert "fingerprint write" not in output.lower()
    assert counts.get(_FINGERPRINT_SETUP, 0) == test_case.expected_setups
    assert after_loss == test_case.expected_rows_after_loss
    assert tuple(fingerprint_rows(project_dir)) == test_case.expected_models_after_rebuild


@pytest.mark.parametrize(
    "test_case",
    [
        LeftoverRelationTestCase(
            description="a staging table with other columns left by an interrupted rebuild",
            leftover_sql="CREATE TABLE main.orders_table__staging AS SELECT 'stale' AS note",
            expected_rows={"orders_table": 3, "orders_view": 3},
        ),
        LeftoverRelationTestCase(
            description="a delta table with other columns left by an interrupted incremental build",
            leftover_sql="CREATE TABLE main.orders_merge__delta AS SELECT 'stale' AS note",
            expected_rows={"orders_merge": 3},
        ),
        LeftoverRelationTestCase(
            description="a microbatch delta left by a failed batch",
            leftover_sql=(
                "CREATE TABLE main.orders_daily__delta AS "
                "SELECT 99 AS order_id, TIMESTAMP '2026-01-02 00:00:00' AS ordered_at, "
                "1.00 AS amount"
            ),
            expected_rows={"orders_daily": 3},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_leftover_intermediate_when_building_then_it_is_replaced(
    test_case: LeftoverRelationTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_orders_project(project_dir=tmp_path)
    first_code, first_output, _, _ = run_counted(
        project_dir=project_dir, args=("build",), capsys=capsys
    )
    execute_sql(project_dir, test_case.leftover_sql)

    code, output, _, _ = run_counted(project_dir=project_dir, args=("build",), capsys=capsys)

    assert (first_code, code) == (0, 0), first_output + output
    assert row_counts(project_dir, tuple(test_case.expected_rows)) == test_case.expected_rows
    assert intermediate_relations(project_dir) == test_case.expected_intermediates


@pytest.mark.parametrize(
    "test_case",
    [
        ScenarioLeftoverTestCase(
            description="a retained run's relations are dropped and rebuilt",
            retained_runs=1,
            planted_sql=(),
            expected_kinds=_RETAINED,
        ),
        ScenarioLeftoverTestCase(
            description="a view at a scenario table's name is replaced by the table",
            retained_runs=1,
            planted_sql=("DROP TABLE {table}; CREATE VIEW {table} AS SELECT 'stale' AS note",),
            expected_kinds=_RETAINED,
        ),
        ScenarioLeftoverTestCase(
            description="a first run in an empty namespace",
            retained_runs=0,
            planted_sql=(),
            expected_kinds=_RETAINED,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_scenario_leftovers_when_testing_again_then_the_scenario_passes(
    test_case: ScenarioLeftoverTestCase, tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    project_dir: Path = write_orders_project(project_dir=tmp_path)
    retain: tuple[str, ...] = ("scenario", "test", "--retain")
    earlier: tuple[tuple[int, str, dict[str, int], tuple[str, ...]], ...] = tuple(
        run_counted(project_dir=project_dir, args=retain, capsys=capsys)
        for _ in range(test_case.retained_runs)
    )
    sql: str
    for sql in test_case.planted_sql:
        execute_sql(
            project_dir,
            sql.format(
                table=scenario_relation_name(project_dir, "\\_\\_model\\_\\_orders\\_table")
            ),
        )

    code, output, _, _ = run_counted(project_dir=project_dir, args=retain, capsys=capsys)
    rerun: tuple[tuple[str, str], ...] = scenario_relation_kinds(project_dir)
    final_code, final_output, _, _ = run_counted(
        project_dir=project_dir, args=("scenario", "test"), capsys=capsys
    )

    assert tuple(run[0] for run in earlier) == (0,) * test_case.retained_runs
    assert (code, final_code) == (0, 0), output + final_output
    assert "PASS=1" in output
    assert rerun == test_case.expected_kinds
    assert scenario_relation_kinds(project_dir) == test_case.expected_after_cleanup


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
