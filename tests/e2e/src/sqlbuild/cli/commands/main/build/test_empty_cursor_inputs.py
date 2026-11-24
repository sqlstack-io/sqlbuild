"""E2E tests for cursor-based incremental models whose inputs are missing or empty."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from textwrap import dedent
from typing import Any

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.build._test_types import (
    CursorModelWithoutInputsCompileE2ETestCase,
    DeltaFailureRetentionE2ETestCase,
    EmptyCursorInputsFirstBuildE2ETestCase,
    EmptyCursorInputsIncrementalE2ETestCase,
    EmptyCursorInputsRebuildRefusalE2ETestCase,
    EmptyRuntimeInputLabelE2ETestCase,
    SameRunEmptyInputsE2ETestCase,
    WaitingOnEmptyInputsE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.build.helpers import (
    build_empty_cursor_project,
    coded_json_diagnostic,
    empty_cursor_relation_exists,
    empty_cursor_rows,
    execute_empty_cursor_sql,
    load_empty_cursor_payments,
    named_json_entry,
    prepare_emptied_cursor_project,
    prepare_empty_cursor_inputs_project,
    prepare_same_run_empty_inputs_project,
    prepare_waiting_on_empty_input_project,
    run_empty_cursor_command,
)

_DAILY_REVENUE_SQL: str = dedent(
    """
    MODEL (
      description "Daily revenue from payments",
      materialized incremental,
      incremental_strategy delete_insert,
      cursor revenue_date,
      cursor_type timestamp,
      cursor_grain day,
      cursor_inputs (raw_payments paid_at),
      columns (
        total_revenue_cents (audits [not_null]),
      ),
    );

    SELECT CAST(paid_at AS DATE) AS revenue_date, SUM(amount_cents) AS total_revenue_cents
    FROM __source("raw_payments")
    GROUP BY 1
    """
).lstrip()
_MICROBATCH_PAYMENTS_SQL: str = dedent(
    """
    MODEL (
      description "Payments processed in daily batches",
      materialized incremental,
      incremental_strategy delete_insert,
      incremental_mode microbatch,
      microbatch_strategy watermark,
      cursor_watermark_mode all,
      batch_size 1d,
      cursor paid_at,
      cursor_type timestamp,
      cursor_grain hour,
      cursor_inputs (raw_payments (column paid_at, roles [filter, watermark])),
    );

    SELECT payment_id, paid_at, amount_cents FROM __source("raw_payments")
    """
).lstrip()
_REVENUE_SUMMARY_SQL: str = dedent(
    """
    MODEL (description "Revenue across all days", materialized table);

    SELECT COUNT(*) AS revenue_days, SUM(total_revenue_cents) AS total_revenue_cents
    FROM __ref("daily_revenue")
    """
).lstrip()
_NO_INPUTS_SQL: str = dedent(
    """
    MODEL (
      description "Daily revenue from successful payments",
      materialized incremental,
      incremental_strategy delete_insert,
      cursor revenue_date,
      cursor_type timestamp,
      cursor_grain day,
    );

    SELECT CAST('2026-04-01' AS DATE) AS revenue_date, 2850 AS total_revenue_cents
    """
).lstrip()
_REVENUE_ROWS_SQL: str = (
    "SELECT CAST(revenue_date AS VARCHAR), total_revenue_cents "
    "FROM main.daily_revenue ORDER BY revenue_date"
)
_LOADED_REVENUE_ROWS: tuple[tuple[object, ...], ...] = (("2026-04-01", 1200), ("2026-04-02", 1650))
_EMPTIED_MODELS: dict[str, str] = {
    "sql/models/daily_revenue.sql": _DAILY_REVENUE_SQL,
    "sql/models/revenue_summary.sql": _REVENUE_SUMMARY_SQL,
}
_ALIASED_REPLAY_SQL: str = dedent(
    """
    MODEL (
      description "Daily revenue published under a versioned name",
      materialized incremental,
      incremental_strategy delete_insert,
      alias daily_revenue_v2,
      replay_on_change full,
      cursor revenue_date,
      cursor_type timestamp,
      cursor_grain day,
      cursor_inputs (raw_payments paid_at),
    );

    SELECT CAST(paid_at AS DATE) AS revenue_date, SUM(amount_cents) AS total_revenue_cents
    FROM __source("raw_payments")
    GROUP BY 1
    """
).lstrip()
_ANY_MODE_MICROBATCH_SQL: str = _MICROBATCH_PAYMENTS_SQL.replace(
    "cursor_watermark_mode all", "cursor_watermark_mode any"
)
_NET_REVENUE_SQL: str = dedent(
    """
    MODEL (
      description "Daily revenue net of refunds",
      materialized incremental,
      incremental_strategy delete_insert,
      cursor revenue_date,
      cursor_type timestamp,
      cursor_grain day,
      cursor_inputs (raw_payments paid_at, raw_refunds refunded_at),
    );

    SELECT CAST(paid_at AS DATE) AS revenue_date, SUM(amount_cents) AS total_revenue_cents
    FROM __source("raw_payments")
    WHERE payment_id NOT IN (SELECT refund_id FROM __source("raw_refunds"))
    GROUP BY 1
    """
).lstrip()
_STG_PAYMENTS_SQL: str = dedent(
    """
    MODEL (description "Payments staged for reporting", materialized view, alias payments_clean);

    SELECT payment_id, paid_at, amount_cents FROM __source("raw_payments")
    """
).lstrip()
_STAGED_REVENUE_SQL: str = dedent(
    """
    MODEL (
      description "Daily revenue from staged payments",
      materialized incremental,
      incremental_strategy delete_insert,
      cursor revenue_date,
      cursor_type timestamp,
      cursor_grain day,
      cursor_inputs (stg_payments paid_at),
    );

    SELECT CAST(paid_at AS DATE) AS revenue_date, SUM(amount_cents) AS total_revenue_cents
    FROM __ref("stg_payments")
    GROUP BY 1
    """
).lstrip()
_SAME_RUN_MODELS: dict[str, str] = {
    "sql/models/stg_payments.sql": dedent(
        """
        MODEL (description "Payments staged for reporting", materialized view);

        SELECT payment_id, paid_at, amount_cents FROM __source("raw_payments")
        """
    ).lstrip(),
    "sql/models/stg_refunds.sql": dedent(
        """
        MODEL (description "Refunds staged for reporting", materialized view);

        SELECT refund_id, refunded_at, amount_cents FROM __source("raw_refunds")
        """
    ).lstrip(),
    "sql/models/daily_revenue.sql": dedent(
        """
        MODEL (
          description "Daily revenue net of refunds, from staged inputs",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor revenue_date,
          cursor_type timestamp,
          cursor_grain day,
          cursor_inputs (stg_payments paid_at, stg_refunds refunded_at),
        );

        SELECT CAST(paid_at AS DATE) AS revenue_date, SUM(amount_cents) AS total_revenue_cents
        FROM __ref("stg_payments")
        WHERE payment_id NOT IN (SELECT refund_id FROM __ref("stg_refunds"))
        GROUP BY 1
        """
    ).lstrip(),
}
_EMPTY_ALL_SQL: str = "DELETE FROM main.raw_payments; DELETE FROM main.raw_refunds"
_EMPTY_REFUNDS_SQL: str = (
    "DELETE FROM main.raw_refunds; "
    "INSERT INTO main.raw_payments VALUES (2, '2026-04-02 10:00:00', 1650)"
)
_NO_INPUTS_MODELS: dict[str, str] = {"sql/models/finance/daily_revenue.sql": _NO_INPUTS_SQL}


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsFirstBuildE2ETestCase(
            description="delete_insert model",
            model_sql=_DAILY_REVENUE_SQL,
            rows_sql=_REVENUE_ROWS_SQL,
            expected_reason="no input rows (raw_payments.paid_at)",
            expected_rows_after_load=_LOADED_REVENUE_ROWS,
        ),
        EmptyCursorInputsFirstBuildE2ETestCase(
            description="watermark microbatch model",
            model_sql=_MICROBATCH_PAYMENTS_SQL,
            rows_sql="SELECT payment_id, amount_cents FROM main.daily_revenue ORDER BY 1",
            expected_reason="no input rows (raw_payments.paid_at)",
            expected_rows_after_load=((1, 1200), (2, 1650)),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_empty_source_when_building_first_then_creates_empty_table_and_next_build_loads(
    test_case: EmptyCursorInputsFirstBuildE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models={"sql/models/daily_revenue.sql": test_case.model_sql}
    )

    first: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("build",)
    )
    empty_rows: tuple[tuple[object, ...], ...] = empty_cursor_rows(
        project_dir=project_dir, sql="SELECT COUNT(*) FROM main.daily_revenue"
    )
    load_empty_cursor_payments(project_dir=project_dir)
    second: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("build",)
    )

    assert first.returncode == 0, first.stdout + first.stderr
    assert test_case.expected_reason in first.stdout
    assert empty_rows == ((0,),)
    assert second.returncode == 0, second.stdout + second.stderr
    assert test_case.expected_reason not in second.stdout
    assert (
        empty_cursor_rows(project_dir=project_dir, sql=test_case.rows_sql)
        == test_case.expected_rows_after_load
    )
    assert not empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue__delta")


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsIncrementalE2ETestCase(
            description="plan and build text output",
            command=("build",),
            expected_output_fragments=(
                "daily_revenue  (delete_insert)",
                "no input rows (raw_payments.paid_at)",
                "audit     not_null (total_revenue_cents) PASS",
                "Completed successfully  PASS=3",
            ),
            expected_json_empty_inputs=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_emptied_source_when_building_incrementally_then_destination_is_unchanged(
    test_case: EmptyCursorInputsIncrementalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models=_EMPTIED_MODELS
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql="DELETE FROM main.raw_payments")
    execute_empty_cursor_sql(project_dir=project_dir, sql="DROP TABLE main.revenue_summary")

    plan: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("plan",)
    )
    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert "window  no input rows (raw_payments.paid_at)" in plan.stdout
    assert result.returncode == 0, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_output_fragments), (
        result.stdout
    )
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        _LOADED_REVENUE_ROWS
    )
    assert empty_cursor_rows(project_dir=project_dir, sql="SELECT * FROM main.revenue_summary") == (
        (2, 2850),
    )
    assert not empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue__delta")


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsIncrementalE2ETestCase(
            description="build json output",
            command=("build", "--json"),
            expected_output_fragments=(),
            expected_json_empty_inputs=("raw_payments.paid_at",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_emptied_source_when_building_json_then_reports_empty_cursor_inputs(
    test_case: EmptyCursorInputsIncrementalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models=_EMPTIED_MODELS
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql="DELETE FROM main.raw_payments")

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 0, result.stdout + result.stderr
    asset: dict[str, Any] = named_json_entry(
        entries=json.loads(result.stdout)["assets"], name="daily_revenue"
    )
    assert asset["status"] == "success"
    assert asset["empty_cursor_inputs"] == list(test_case.expected_json_empty_inputs or ())
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        _LOADED_REVENUE_ROWS
    )
    assert not empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue__delta")


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsIncrementalE2ETestCase(
            description="plan json output",
            command=("plan", "--json"),
            expected_output_fragments=(),
            expected_json_empty_inputs=("raw_payments.paid_at",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_emptied_source_when_planning_json_then_reports_no_input_rows(
    test_case: EmptyCursorInputsIncrementalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models={"sql/models/daily_revenue.sql": _DAILY_REVENUE_SQL}
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql="DELETE FROM main.raw_payments")

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 0, result.stdout + result.stderr
    model: dict[str, Any] = named_json_entry(
        entries=json.loads(result.stdout)["models"], name="daily_revenue"
    )
    assert model["cursor"] == {
        **model["cursor"],
        "resolution_status": "no_input_rows",
        "empty_inputs": list(test_case.expected_json_empty_inputs or ()),
    }


@pytest.mark.parametrize(
    "test_case",
    [
        CursorModelWithoutInputsCompileE2ETestCase(
            description="compile text output",
            command=("compile",),
            expected_output_fragments=(
                "error[P011]: incremental model 'daily_revenue' reads no inputs, so builds "
                "after the first cannot work out their cursor window",
                "--> sql/models/finance/daily_revenue.sql:5:3",
                "cursor revenue_date,",
                "^^^^^^^^^^^^^^^^^^^",
                "help: read the data through __ref(), __source() or __seed() so its cursor can "
                "bound the window, or use materialized table if the model has no upstream data",
            ),
            expected_json_location=None,
        ),
        CursorModelWithoutInputsCompileE2ETestCase(
            description="build fails before warehouse work",
            command=("build",),
            expected_output_fragments=("error[P011]",),
            expected_json_location=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_model_without_inputs_when_compiling_then_fails_with_p011(
    test_case: CursorModelWithoutInputsCompileE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models=_NO_INPUTS_MODELS
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert all(
        fragment in result.stdout + result.stderr
        for fragment in test_case.expected_output_fragments
    )
    assert not empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue")


@pytest.mark.parametrize(
    "test_case",
    [
        CursorModelWithoutInputsCompileE2ETestCase(
            description="compile json output",
            command=("compile", "--json"),
            expected_output_fragments=(),
            expected_json_location=(5, 3, 22),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_model_without_inputs_when_compiling_json_then_locates_cursor(
    test_case: CursorModelWithoutInputsCompileE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models=_NO_INPUTS_MODELS
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 1, result.stdout + result.stderr
    diagnostic: dict[str, Any] = coded_json_diagnostic(
        document=json.loads(result.stdout), code="P011"
    )
    assert diagnostic["resource_name"] == "daily_revenue"
    assert test_case.expected_json_location is not None
    line, column, end_column = test_case.expected_json_location
    assert diagnostic["location"] == {
        "path": "sql/models/finance/daily_revenue.sql",
        "line": line,
        "column": column,
        "end_line": line,
        "end_column": end_column,
    }


@pytest.mark.parametrize(
    "test_case",
    [
        DeltaFailureRetentionE2ETestCase(
            description="delta audit failure keeps the delta relation",
            failing_rows_sql="INSERT INTO main.raw_payments VALUES (3, '2026-04-03 08:00:00', NULL)",
            expected_output_fragments=(
                "daily_revenue",
                "delta table kept for inspection: main.daily_revenue__delta",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_failing_incremental_build_when_building_then_delta_relation_is_kept(
    test_case: DeltaFailureRetentionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path, models={"sql/models/daily_revenue.sql": _DAILY_REVENUE_SQL}
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql=test_case.failing_rows_sql)

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("build",)
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert all(
        fragment in result.stdout + result.stderr
        for fragment in test_case.expected_output_fragments
    )
    assert empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue__delta")
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        _LOADED_REVENUE_ROWS
    )


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsRebuildRefusalE2ETestCase(
            description="aliased replay_on_change full model after a query change",
            model_sql=_ALIASED_REPLAY_SQL,
            changed_model_sql=_ALIASED_REPLAY_SQL.replace(
                "SUM(amount_cents) AS", "SUM(amount_cents) * 1 AS"
            ),
            command=("build",),
            rows_sql=(
                "SELECT CAST(revenue_date AS VARCHAR), total_revenue_cents "
                "FROM main.daily_revenue_v2 ORDER BY revenue_date"
            ),
            expected_rows=_LOADED_REVENUE_ROWS,
            expected_output_fragments=(
                "error[S302]: model 'daily_revenue' was not rebuilt: its cursor inputs have no "
                "rows (raw_payments.paid_at), so rebuilding it now would leave it empty",
                "help: load the inputs first, or set the window explicitly with "
                "--start-cursor-ts and --end-cursor-ts",
            ),
            expected_json_error_code=None,
        ),
        EmptyCursorInputsRebuildRefusalE2ETestCase(
            description="watermark microbatch full refresh in any mode",
            model_sql=_ANY_MODE_MICROBATCH_SQL,
            changed_model_sql=_ANY_MODE_MICROBATCH_SQL,
            command=("build", "--full-refresh"),
            rows_sql="SELECT payment_id, amount_cents FROM main.daily_revenue ORDER BY 1",
            expected_rows=((1, 1200), (2, 1650)),
            expected_output_fragments=(
                "[S302] model 'daily_revenue' was not rebuilt: its cursor inputs have no rows "
                "(raw_payments.paid_at), so rebuilding it now would leave it empty",
                "help: load the inputs first, or set the window explicitly with "
                "--start-cursor-ts and --end-cursor-ts",
            ),
            expected_json_error_code=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loaded_destination_when_rebuilding_from_empty_source_then_refuses_and_keeps_rows(
    test_case: EmptyCursorInputsRebuildRefusalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_emptied_cursor_project(
        tmp_path=tmp_path,
        model_sql=test_case.model_sql,
        changed_model_sql=test_case.changed_model_sql,
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 1, result.stdout + result.stderr
    assert all(
        fragment in result.stdout + result.stderr
        for fragment in test_case.expected_output_fragments
    ), result.stdout + result.stderr
    assert empty_cursor_rows(project_dir=project_dir, sql=test_case.rows_sql) == (
        test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyCursorInputsRebuildRefusalE2ETestCase(
            description="watermark microbatch full refresh in any mode json output",
            model_sql=_ANY_MODE_MICROBATCH_SQL,
            changed_model_sql=_ANY_MODE_MICROBATCH_SQL,
            command=("build", "--full-refresh", "--json"),
            rows_sql="SELECT payment_id, amount_cents FROM main.daily_revenue ORDER BY 1",
            expected_rows=((1, 1200), (2, 1650)),
            expected_output_fragments=(),
            expected_json_error_code="S302",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loaded_destination_when_rebuilding_json_from_empty_source_then_reports_s302(
    test_case: EmptyCursorInputsRebuildRefusalE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_emptied_cursor_project(
        tmp_path=tmp_path,
        model_sql=test_case.model_sql,
        changed_model_sql=test_case.changed_model_sql,
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 1, result.stdout + result.stderr
    asset: dict[str, Any] = named_json_entry(
        entries=json.loads(result.stdout)["assets"], name="daily_revenue"
    )
    assert asset["status"] == "failed"
    assert asset["error_code"] == test_case.expected_json_error_code
    assert empty_cursor_rows(project_dir=project_dir, sql=test_case.rows_sql) == (
        test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        WaitingOnEmptyInputsE2ETestCase(
            description="plan text output",
            command=("plan",),
            expected_output_fragments=(
                "window  waiting on empty input raw_refunds.refunded_at; "
                "other inputs have new rows",
            ),
            expected_json_empty_inputs=None,
        ),
        WaitingOnEmptyInputsE2ETestCase(
            description="build text output",
            command=("build",),
            expected_output_fragments=(
                "waiting on empty input raw_refunds.refunded_at; other inputs have new rows",
                "Warnings:",
                "Completed with warnings  PASS=0  WARN=1",
            ),
            expected_json_empty_inputs=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_empty_input_when_others_have_rows_then_warns_that_model_is_waiting(
    test_case: WaitingOnEmptyInputsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waiting_on_empty_input_project(
        tmp_path=tmp_path, model_sql=_NET_REVENUE_SQL
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_output_fragments), (
        result.stdout
    )
    assert "no input rows" not in result.stdout
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        ("2026-04-01", 1200),
    )


@pytest.mark.parametrize(
    "test_case",
    [
        WaitingOnEmptyInputsE2ETestCase(
            description="build and plan json output",
            command=("build", "--json"),
            expected_output_fragments=(),
            expected_json_empty_inputs=("raw_refunds.refunded_at",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_one_empty_input_when_reporting_json_then_flags_waiting_on_empty_inputs(
    test_case: WaitingOnEmptyInputsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_waiting_on_empty_input_project(
        tmp_path=tmp_path, model_sql=_NET_REVENUE_SQL
    )

    plan: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("plan", "--json")
    )
    build: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert build.returncode == 0, build.stdout + build.stderr
    expected_inputs: list[str] = list(test_case.expected_json_empty_inputs or ())
    model: dict[str, Any] = named_json_entry(
        entries=json.loads(plan.stdout)["models"], name="daily_revenue"
    )
    assert model["cursor"] == {
        **model["cursor"],
        "resolution_status": "no_input_rows",
        "empty_inputs": expected_inputs,
        "waiting_on_empty_inputs": True,
    }
    asset: dict[str, Any] = named_json_entry(
        entries=json.loads(build.stdout)["assets"], name="daily_revenue"
    )
    assert asset["status"] == "success"
    assert asset["empty_cursor_inputs"] == expected_inputs
    assert asset["waiting_on_empty_inputs"] is True
    assert asset["warnings"] == [
        "waiting on empty input raw_refunds.refunded_at; other inputs have new rows"
    ]
    assert json.loads(build.stdout)["summary"]["warning_count"] == 1


@pytest.mark.parametrize(
    "test_case",
    [
        EmptyRuntimeInputLabelE2ETestCase(
            description="aliased staging view rebuilt in the same run",
            changed_upstream_sql=_STG_PAYMENTS_SQL.replace(
                'FROM __source("raw_payments")',
                'FROM __source("raw_payments") WHERE amount_cents IS NOT NULL',
            ),
            expected_output_fragments=("no input rows (stg_payments.paid_at)",),
            unexpected_output_fragments=("payments_clean.paid_at",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_empty_input_built_in_same_run_when_building_then_labels_it_by_project_name(
    test_case: EmptyRuntimeInputLabelE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_empty_cursor_inputs_project(
        tmp_path=tmp_path,
        models={
            "sql/models/stg_payments.sql": _STG_PAYMENTS_SQL,
            "sql/models/daily_revenue.sql": _STAGED_REVENUE_SQL,
        },
    )
    load_empty_cursor_payments(project_dir=project_dir)
    build_empty_cursor_project(project_dir=project_dir)
    execute_empty_cursor_sql(project_dir=project_dir, sql="DELETE FROM main.raw_payments")
    (project_dir / "sql" / "models" / "stg_payments.sql").write_text(
        test_case.changed_upstream_sql, encoding="utf-8"
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("build",)
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_output_fragments), (
        result.stdout
    )
    assert not any(fragment in result.stdout for fragment in test_case.unexpected_output_fragments)
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        _LOADED_REVENUE_ROWS
    )


@pytest.mark.parametrize(
    "test_case",
    [
        SameRunEmptyInputsE2ETestCase(
            description="all inputs empty",
            emptying_sql=_EMPTY_ALL_SQL,
            command=("build",),
            expected_output_fragments=(
                "no input rows (stg_payments.paid_at, stg_refunds.refunded_at)",
                "Completed successfully  PASS=3  WARN=0",
            ),
            unexpected_output_fragments=("waiting on empty input", "Warnings:"),
            expected_json_empty_inputs=None,
            expected_json_waiting_on_empty_inputs=None,
            expected_json_warnings=(),
        ),
        SameRunEmptyInputsE2ETestCase(
            description="one input empty while the other has new rows",
            emptying_sql=_EMPTY_REFUNDS_SQL,
            command=("build",),
            expected_output_fragments=(
                "waiting on empty input stg_refunds.refunded_at; other inputs have new rows",
                "Completed with warnings  PASS=2  WARN=1",
            ),
            unexpected_output_fragments=("no input rows",),
            expected_json_empty_inputs=None,
            expected_json_waiting_on_empty_inputs=None,
            expected_json_warnings=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inputs_built_in_same_run_when_empty_then_matches_planned_empty_input_handling(
    test_case: SameRunEmptyInputsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_same_run_empty_inputs_project(
        tmp_path=tmp_path, models=_SAME_RUN_MODELS, emptying_sql=test_case.emptying_sql
    )

    plan: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=("plan",)
    )
    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert plan.returncode == 0, plan.stdout + plan.stderr
    assert "bounds  computed at run time" in plan.stdout
    assert result.returncode == 0, result.stdout + result.stderr
    assert all(fragment in result.stdout for fragment in test_case.expected_output_fragments), (
        result.stdout
    )
    assert not any(fragment in result.stdout for fragment in test_case.unexpected_output_fragments)
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        ("2026-04-01", 1200),
    )
    assert not empty_cursor_relation_exists(project_dir=project_dir, name="daily_revenue__delta")


@pytest.mark.parametrize(
    "test_case",
    [
        SameRunEmptyInputsE2ETestCase(
            description="all inputs empty json output",
            emptying_sql=_EMPTY_ALL_SQL,
            command=("build", "--json"),
            expected_output_fragments=(),
            unexpected_output_fragments=(),
            expected_json_empty_inputs=("stg_payments.paid_at", "stg_refunds.refunded_at"),
            expected_json_waiting_on_empty_inputs=None,
            expected_json_warnings=(),
        ),
        SameRunEmptyInputsE2ETestCase(
            description="one input empty json output",
            emptying_sql=_EMPTY_REFUNDS_SQL,
            command=("build", "--json"),
            expected_output_fragments=(),
            unexpected_output_fragments=(),
            expected_json_empty_inputs=("stg_refunds.refunded_at",),
            expected_json_waiting_on_empty_inputs=True,
            expected_json_warnings=(
                "waiting on empty input stg_refunds.refunded_at; other inputs have new rows",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inputs_built_in_same_run_when_empty_then_json_reports_empty_inputs(
    test_case: SameRunEmptyInputsE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_same_run_empty_inputs_project(
        tmp_path=tmp_path, models=_SAME_RUN_MODELS, emptying_sql=test_case.emptying_sql
    )

    result: subprocess.CompletedProcess[str] = run_empty_cursor_command(
        project_dir=project_dir, command=test_case.command
    )

    assert result.returncode == 0, result.stdout + result.stderr
    asset: dict[str, Any] = named_json_entry(
        entries=json.loads(result.stdout)["assets"], name="daily_revenue"
    )
    assert asset["status"] == "success"
    assert asset["empty_cursor_inputs"] == list(test_case.expected_json_empty_inputs or ())
    assert asset.get("waiting_on_empty_inputs") == test_case.expected_json_waiting_on_empty_inputs
    assert asset["warnings"] == list(test_case.expected_json_warnings)
    assert empty_cursor_rows(project_dir=project_dir, sql=_REVENUE_ROWS_SQL) == (
        ("2026-04-01", 1200),
    )
