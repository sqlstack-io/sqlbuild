"""A location plans from the server's synced copy and never opens the warehouse to do it."""

import json
import os
import sqlite3
import subprocess
import sys
import time
from contextlib import closing
from pathlib import Path
from textwrap import dedent
from typing import Any

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ChangedBuildPublicationTestCase,
    ChangedBuildReplanTestCase,
    ChangedPlanPreviewTestCase,
    FailedPlanTestCase,
    HeldPlanTestCase,
    LocationCantRunTestCase,
    PlanPreviewTestCase,
    SyncedFreshnessTestCase,
    UnknownSourceAccessTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.catalog.helpers import (
    request_plan,
    request_warehouse_sync,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    ORDER_SKEW_SECONDS,
    REGION_PROVIDER_FILES,
    changed_build_and_plan_from_copy,
    changed_build_report,
    changed_runs,
    live_plan,
    location_process,
    plan_before_sync,
    plan_from_copy,
    plans_from_copy,
    run_sqb,
    snapshot_api,
    wait_for_generation,
    wait_for_location_work,
    wait_for_log_count,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files

_EXPRESSION: str = "SELECT * FROM (VALUES (1, 100)) AS refunds(refund_id, amount_cents)"
_SOURCES: str = dedent(
    """
    sources:
      - name: raw_refunds
        description: Refunds described from an inline query.
        expression: |
          {expression}
        type_enforcement: true
        freshness:
          column: refund_id
        columns:
          - name: refund_id
            type: INTEGER
          - name: amount_cents
            type: INTEGER
    """
).strip()
_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n[connections.local]\ndatabase = "warehouse.db"\n'
        '[targets.development]\nschema = "main"\n'
    ),
    "sql/seeds/raw_orders.csv": "id,ordered_at\n1,2026-01-01 00:00:00\n2,2026-01-01 01:00:00\n",
    "sql/seeds/raw_orders.yml": dedent(
        """
        seeds:
          - name: raw_orders
            description: Raw orders.
            columns:
              - name: id
                type: INTEGER
              - name: ordered_at
                type: TIMESTAMP
        """
    ).strip()
    + "\n",
    "sql/models/orders.sql": dedent(
        """
        MODEL (description "Orders by timestamp cursor.",
          materialized incremental,
          incremental_strategy append,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
        );

        SELECT id, ordered_at FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "sql/models/daily_orders.sql": dedent(
        """
        MODEL (description "Daily orders rebuilt over a lookback.",
          materialized incremental,
          incremental_strategy delete_insert,
          cursor ordered_at,
          cursor_type timestamp,
          cursor_grain second,
          lookback 1d,
        );

        SELECT id, ordered_at FROM __seed("raw_orders")
        WHERE ordered_at >= __cursor_start() AND ordered_at < __cursor_end()
        """
    ).strip()
    + "\n",
    "sql/models/order_count.sql": dedent(
        """
        MODEL (description "Order count without a cursor.", materialized table);

        SELECT COUNT(*) AS order_count FROM __seed("raw_orders")
        """
    ).strip()
    + "\n",
    "sql/models/refunds.sql": dedent(
        """
        MODEL (description "Refunds.", materialized table);

        SELECT refund_id, amount_cents FROM __source("raw_refunds")
        """
    ).strip()
    + "\n",
    "sql/sources/raw.yml": _SOURCES.format(expression=_EXPRESSION) + "\n",
}
_SELECTION: list[str] = [
    "model:daily_orders",
    "model:order_count",
    "model:orders",
    "model:refunds",
]
_UNKNOWN: list[str] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
    "recorded migrations",
]
_HELD: str = "Planning from the published compiled project."
_HELD_TESTS: str = "Planned the publication's 1 SQL tests once for previews."
_REFUNDS_TEST: str = (
    "TEST ();\n\nWITH\n"
    "__source__raw_refunds AS (SELECT 1 AS refund_id, 100 AS amount_cents),\n"
    "__expected__refunds AS (SELECT 1 AS refund_id, 100 AS amount_cents)\n"
)
_SYNCED_READING: str = "Reading the synced warehouse state..."
_REPLAYED: str = (
    "cursor window of daily_orders (the run starts it away from the last build's cursor)"
)

_SYNCED: str = "Warehouse history synchronization completed"
_PLANNED: str = "Planned what changed"


_FUNCTION_CALLER_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "orders"\nadapter = "duckdb"\n[connections.local]\ndatabase = "warehouse.db"\n'
        '[targets.development]\nschema = "main"\n'
    ),
    "sql/seeds/orders.csv": "id,ordered_at,amount\n1,2026-09-01,10\n2,2026-09-02,20\n",
    "sql/seeds/orders.yml": (
        "seeds:\n- name: orders\n  description: Orders.\n  columns:\n"
        "  - name: id\n    type: INTEGER\n  - name: ordered_at\n    type: DATE\n"
        "  - name: amount\n    type: INTEGER\n"
    ),
    "sql/functions/sql/udf__adjust_amount.sql": (
        "FUNCTION (description 'Adjusted amount.',\n  arguments (amount INTEGER),\n"
        "  returns INTEGER,\n);\n\namount + 1\n"
    ),
    "sql/models/order_amounts.sql": (
        "MODEL (description 'Adjusted order amounts.',\n  materialized incremental,\n"
        "  incremental_strategy merge,\n  unique_key [id],\n  cursor ordered_at,\n"
        "  cursor_type timestamp,\n  cursor_grain day,\n"
        "  cursor_inputs (\n    orders ordered_at,\n  ),\n"
        "  replay_on_change full,\n  full_refresh false,\n);\n\n"
        "SELECT id, CAST(ordered_at AS DATE) AS ordered_at,\n"
        '  __udf("udf__adjust_amount")(amount) AS adjusted_amount\nFROM __seed("orders")\n'
    ),
}


_CANT_RUN: list[dict[str, object]] = [
    {
        "reason": "missing_secret",
        "provider": "region",
        "path": "python/providers/region.py",
        "variables": ["WORKER_ENV_REGION_CODE", "WORKER_ENV_TOKEN"],
        "nodes": ["source:raw_regions"],
        "readers": ["model:orders"],
        "sinks": [],
        "env_file": ".env",
    }
]


@pytest.mark.parametrize(
    "test_case",
    [
        PlanPreviewTestCase(
            description="matching observations plan like the live warehouse",
            edited_files={},
            injected_sql=(),
            expected_window_models=("orders",),
            expected_unplanned_cursor_models=(),
            expected_added_unknowns=(_REPLAYED,),
        ),
        PlanPreviewTestCase(
            description="a cursor_max from no successful build is not a cursor position",
            edited_files={},
            injected_sql=(
                "INSERT INTO main._sqlbuild_observations VALUES ('injected-orders', 'cursor_max', "
                "'orders', 'timestamp', '2099-01-01 00:00:00', 'injected-run', "
                "'2099-01-01 00:00:00')",
            ),
            expected_window_models=(),
            expected_unplanned_cursor_models=("orders",),
            expected_added_unknowns=(
                "cursor position of orders (no successful build recorded it)",
                _REPLAYED,
            ),
        ),
        PlanPreviewTestCase(
            description="an edited expression plans its declared columns and says so",
            edited_files={
                "sql/sources/raw.yml": _SOURCES.format(expression=f"{_EXPRESSION} WHERE true")
                + "\n"
            },
            injected_sql=(),
            expected_window_models=("orders",),
            expected_unplanned_cursor_models=(),
            expected_added_unknowns=(
                _REPLAYED,
                "query columns of expression source raw_refunds (planned as declared)",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_synced_location_when_a_plan_is_requested_then_it_plans_from_the_copy_without_the_warehouse(
    test_case: PlanPreviewTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    with duckdb.connect(project / "warehouse.db") as warehouse:
        for statement in test_case.injected_sql:
            _ = warehouse.execute(statement)
    write_files(project_dir=project, files=test_case.edited_files)
    live_run: subprocess.CompletedProcess[str] = live_plan(project, _SELECTION)
    assert live_run.returncode == 0, live_run.stderr
    live: dict[str, Any] = json.loads(live_run.stdout)
    log: Path = tmp_path / "location.log"
    refused, preview = plan_from_copy(tmp_path, project, _SELECTION, {})

    assert refused.returncode != 0
    assert "synced_copy" in preview, log.read_text()
    synced: dict[str, Any] = preview.pop("synced_copy")
    assert synced["synced_at"] is not None
    timings: dict[str, Any] = preview.pop("timings")
    assert "timings" not in live
    assert timings["planned_from"] == "synced_state"
    step_names: list[str] = [step["name"] for step in timings["steps"]]
    assert step_names[-1] == "Other"
    assert "Other" not in step_names[:-1]
    assert sum(step["seconds"] for step in timings["steps"]) == pytest.approx(
        timings["total_seconds"], abs=0.01
    )
    assert synced["unknown_until_run"] == [*_UNKNOWN, *test_case.expected_added_unknowns]
    live_models: dict[str, dict[str, Any]] = {model["name"]: model for model in live.pop("models")}
    preview_models: dict[str, dict[str, Any]] = {
        model["name"]: model for model in preview.pop("models")
    }
    live_replayed: dict[str, Any] = live_models.pop("daily_orders")
    preview_replayed: dict[str, Any] = preview_models.pop("daily_orders")
    assert live_replayed["cursor"]["resolved_bounds"]["start"] is not None
    assert (preview_replayed["action"], preview_replayed["reason"]) == (
        live_replayed["action"],
        live_replayed["reason"],
    )
    live_freshness: dict[str, Any] = live["metadata"].pop("direct_source_freshness")
    preview_freshness: dict[str, Any] = preview["metadata"].pop("direct_source_freshness")
    assert live_freshness["observed_source_names"] == ["raw_refunds"]
    assert preview_freshness["unknown_source_details"] == [
        {
            "message": "a plan preview does not observe source freshness",
            "reason": "error",
            "source_name": "raw_refunds",
        }
    ]
    assert synced["cursor_window_starts"] == {
        name: live_models[name]["cursor_bounds"]["start"]
        for name in test_case.expected_window_models
    }
    for name in (*test_case.expected_window_models, *test_case.expected_unplanned_cursor_models):
        for model in (live_models[name], preview_models.pop(name)):
            assert model["action"] == "incremental_append"
            assert model["reason"] == "normal_incremental"
        _ = live_models.pop(name)
    assert preview_models == live_models
    assert preview == live


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedPlanPreviewTestCase(
            description="an edited model is the only change the copy names",
            edited_model="sql/models/order_count.sql",
            edit=("AS order_count", "AS order_count, 1 AS batch"),
            expected_changed={
                "counts": ["definition"],
                "reach": "job",
                "runs": [
                    {
                        "node": "model:order_count",
                        "reason": "planner change (query_changed)",
                        "downstream_of": None,
                    }
                ],
                "held_back": [],
                "unchanged": ["model:daily_orders", "model:orders", "model:refunds"],
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_edited_model_when_a_changed_plan_is_requested_then_the_copy_names_what_changed(
    test_case: ChangedPlanPreviewTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: str = _PROJECT_FILES[test_case.edited_model].replace(*test_case.edit)
    write_files(project_dir=project, files={test_case.edited_model: edited})
    live_run: subprocess.CompletedProcess[str] = live_plan(project, _SELECTION, "--changed")
    assert live_run.returncode == 0, live_run.stderr

    refused, preview = plan_from_copy(tmp_path, project, _SELECTION, {"changed": True})

    assert refused.returncode != 0
    assert preview.get("changed") == test_case.expected_changed, (
        tmp_path / "location.log"
    ).read_text()
    assert preview["changed"] == json.loads(live_run.stdout)["changed"]


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedBuildPublicationTestCase(
            description="an edited model publishes as changed and its reader as downstream",
            edit=("AS order_count", "AS order_count, 1 AS batch"),
            expected_published={
                "changed": ["model:order_count"],
                "downstream": ["model:order_summary"],
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_edited_model_when_the_location_plans_what_changed_then_it_publishes_what_the_changed_plan_runs(
    test_case: ChangedBuildPublicationTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    summary: str = (
        'MODEL (description "Order count summary.", materialized table);\n\n'
        'SELECT order_count FROM __ref("order_count")\n'
    )
    write_files(
        project_dir=project, files=_PROJECT_FILES | {"sql/models/order_summary.sql": summary}
    )
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: str = _PROJECT_FILES["sql/models/order_count.sql"].replace(*test_case.edit)
    write_files(project_dir=project, files={"sql/models/order_count.sql": edited})
    # The selection the server freezes `sqb build --changed` to: every node a build runs.
    selection: list[str] = sorted([*_SELECTION, "model:order_summary", "seed:raw_orders"])

    published, plan = changed_build_and_plan_from_copy(tmp_path, project, selection)

    assert published == changed_build_report(plan), (tmp_path / "location.log").read_text()
    assert published == test_case.expected_published


@pytest.mark.parametrize(
    "test_case",
    [
        ChangedBuildReplanTestCase(
            description="a publication and a sync that mirrors a new build each plan again",
            edit=("AS order_count", "AS order_count, 1 AS batch"),
            expected_published={"changed": ["model:order_count"], "downstream": []},
            expected_rebuilt={"changed": [], "downstream": []},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_location_when_syncs_complete_then_it_replans_only_after_publication_or_an_applied_change(
    test_case: ChangedBuildReplanTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    built_at: float = time.monotonic()
    edited: str = _PROJECT_FILES["sql/models/order_count.sql"].replace(*test_case.edit)
    write_files(project_dir=project, files={"sql/models/order_count.sql": edited})
    control: Path = tmp_path / "control/.sqlbuild/control.sqlite3"
    log: Path = tmp_path / "location.log"

    def changed_build() -> dict[str, list[str]]:
        with closing(sqlite3.connect(control)) as database:
            row: tuple[str] | None = database.execute(
                "SELECT report_json FROM sqb_changed_builds"
            ).fetchone()
        assert row is not None, "the location reported its changed build"
        return json.loads(row[0])

    with (
        snapshot_api(tmp_path / "control") as api,
        location_process(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "serve",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
            ],
            log,
            tmp_path / "location.stdout",
            env=os.environ | {"SQLBUILD_LOCATION_TOKEN": api.bearer},
        ) as location,
    ):
        _ = wait_for_generation(api, "development", 1, location, log)
        first: str = wait_for_log_count(location, log, _PLANNED, 1)
        assert first.index(_SYNCED) < first.index(_PLANNED), (
            "a publication plans what changed once its sync completes, not from the old copy"
        )
        assert changed_build() == test_case.expected_published, log.read_text()

        _ = wait_for_location_work(api, request_warehouse_sync(control), location, log)
        _ = wait_for_log_count(location, log, _SYNCED, 2)
        # A later plan request runs on a later turn, after any plan the completed sync started.
        _ = wait_for_location_work(api, request_plan(control, ["model:orders"]), location, log)
        assert log.read_text().count(_PLANNED) == 1, "a sync that applied nothing keeps the count"

        time.sleep(max(0.0, built_at + ORDER_SKEW_SECONDS - time.monotonic()))
        rebuilt: subprocess.CompletedProcess[str] = run_sqb(
            project, "build", "--target", "development"
        )
        assert rebuilt.returncode == 0, rebuilt.stderr
        _ = wait_for_location_work(api, request_warehouse_sync(control), location, log)
        _ = wait_for_log_count(location, log, _PLANNED, 2)
        # The log shows which sync the second plan followed and what each sync read.
        assert changed_build() == test_case.expected_rebuilt, (
            f"a sync that mirrored the new build plans again\n{log.read_text()}"
        )


@pytest.mark.parametrize(
    "test_case",
    [
        HeldPlanTestCase(
            description=(
                "only the plan that sets variables compiles the project again; the other two "
                "and the changed-build count plan from the published project, and the same "
                "request again returns the kept plan unless it asks to plan again"
            ),
            settings=(
                {"changed": True},
                {"changed": True, "vars": {"batch": "2"}},
                {"changed": True},
                {"changed": True},
            ),
            fresh=frozenset({2}),
            expected_runs=((), ("model:order_count",), (), ()),
            expected_compiles=1,
            expected_held_plans=3,
            expected_held_test_plannings=1,
            expected_reused=(False, False, False, True),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_plan_requests_when_the_location_plans_them_then_only_variables_compile_the_project_again(
    test_case: HeldPlanTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    files: dict[str, str] = _PROJECT_FILES | {
        "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"] + '[vars]\nbatch = "1"\n',
        "sql/models/order_count.sql": _PROJECT_FILES["sql/models/order_count.sql"].replace(
            "AS order_count", "AS order_count, @@batch AS batch"
        ),
        "tests/unit/test_refunds.sql": _REFUNDS_TEST,
    }
    write_files(project_dir=project, files=files)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    requests: list[tuple[list[str], dict[str, object]]] = [
        (["model:order_count"], settings) for settings in test_case.settings
    ]

    plans: list[dict[str, Any]] = plans_from_copy(tmp_path, project, requests, test_case.fresh)

    log: str = (tmp_path / "location.log").read_text()
    assert tuple(map(changed_runs, plans)) == test_case.expected_runs, log
    assert tuple("reused_from" in plan["timings"] for plan in plans) == test_case.expected_reused
    assert log.count("Compiling project...") == test_case.expected_compiles, log
    assert log.count(_HELD) == test_case.expected_held_plans, log
    # Only the published project plans every SQL test, once; the plan with variables plans its own.
    assert log.count(_HELD_TESTS) == test_case.expected_held_test_plannings, log
    assert log.count(_SYNCED_READING) == test_case.expected_held_plans + 1, log
    assert "Warehouse connected" not in log, "a synced copy opens no warehouse connection"


@pytest.mark.parametrize(
    "test_case",
    [
        UnknownSourceAccessTestCase(
            description="a source the synced state does not list is unknown, not probed",
            added_files={
                "sql/sources/customers.yml": dedent(
                    """
                    sources:
                      - name: raw_customers
                        description: Customers loaded outside this project.
                        schema: main
                        table: raw_customers
                        columns:
                          - name: customer_id
                            type: INTEGER
                    """
                ).strip()
                + "\n",
                "sql/models/customers.sql": (
                    'MODEL (description "Customers.", materialized table);\n'
                    'SELECT customer_id FROM __source("raw_customers")\n'
                ),
            },
            selection=["model:customers"],
            expected_unknown="read access to these sources",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unlisted_source_when_the_location_previews_a_plan_then_its_read_access_is_unknown(
    test_case: UnknownSourceAccessTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=_PROJECT_FILES)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    write_files(project_dir=project, files=test_case.added_files)

    _, preview = plan_from_copy(tmp_path, project, test_case.selection, {})

    assert "error" not in preview, preview
    assert test_case.expected_unknown in preview["synced_copy"]["unknown_until_run"], preview


@pytest.mark.parametrize(
    "test_case",
    [
        SyncedFreshnessTestCase(
            description="a warehouse source's metadata freshness is unknown, not a failed lookup",
            files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "snowflake"\ndefault_target = "dev"\n'
                    '[connections.warehouse]\naccount = "example-account"\nuser = "example_user"\n'
                    '[targets.dev]\nconnection = "warehouse"\ndatabase = "ANALYTICS"\n'
                    'schema = "DEV"\n'
                ),
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_orders
                        description: Raw orders.
                        database: RAW
                        schema: SHOP
                        table: ORDERS
                        columns:
                          - name: order_id
                            type: VARCHAR
                    """
                ).strip()
                + "\n",
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', materialized table);\n"
                    'SELECT order_id FROM __source("raw_orders")\n'
                ),
            },
            expected_absent="Freshness metadata  FAIL",
            expected_line="Freshness metadata  OK (1 source, 1 unknown)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_synced_copy_when_a_preview_reads_source_freshness_then_it_is_unknown_without_a_failure(
    test_case: SyncedFreshnessTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=test_case.files)

    outcome, log = plan_before_sync(tmp_path, project, {"changed": True})

    assert outcome["kind"] == "plan", log
    assert "error" not in outcome["document"], outcome
    assert test_case.expected_absent not in log, log
    assert test_case.expected_line in log, log


@pytest.mark.parametrize(
    "test_case",
    [
        FailedPlanTestCase(
            description="a full_refresh false caller of a changed function cannot rebuild",
            files=_FUNCTION_CALLER_FILES,
            edited_file="sql/functions/sql/udf__adjust_amount.sql",
            edit=("amount + 1", "amount + 100"),
            selection=["model:order_amounts"],
            expected_error_model="order_amounts",
            expected_code="S203",
            expected_reason="model 'order_amounts' sets full_refresh false",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_model_that_cannot_be_planned_when_the_location_plans_it_then_its_error_row_is_stored(
    test_case: FailedPlanTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(project_dir=project, files=test_case.files)
    built: subprocess.CompletedProcess[str] = run_sqb(project, "build", "--target", "development")
    assert built.returncode == 0, built.stderr
    edited: Path = project / test_case.edited_file
    _ = edited.write_text(
        edited.read_text(encoding="utf-8").replace(*test_case.edit), encoding="utf-8"
    )

    published, plan = changed_build_and_plan_from_copy(tmp_path, project, test_case.selection)

    rows: dict[str, dict[str, Any]] = {model["name"]: model for model in plan["models"]}
    error: dict[str, Any] = rows[test_case.expected_error_model]["error"]
    assert plan["has_errors"] is True, plan
    assert rows[test_case.expected_error_model]["action"] == "error", plan
    assert error["code"] == test_case.expected_code, plan
    assert test_case.expected_reason in error["message"], plan
    assert published["changed"] == [] and published["downstream"] == [], published
    assert published["failing"] == (
        f"Can't plan 1 model: {test_case.expected_error_model}: {error['message']}"
    ), published


@pytest.mark.parametrize(
    "test_case",
    [
        LocationCantRunTestCase(
            description="the held project plans what can't run",
            settings={},
            expected_cant_run=_CANT_RUN,
        ),
        LocationCantRunTestCase(
            description="variables compile the project again and still plan what can't run",
            settings={"vars": {"batch": "2"}},
            expected_cant_run=_CANT_RUN,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_provider_missing_secrets_when_the_location_plans_then_the_plan_says_what_cant_run(
    test_case: LocationCantRunTestCase, tmp_path: Path
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files={
            "sqlbuild_project.toml": (
                'name = "orders"\nadapter = "duckdb"\n'
                '[connections.local]\ndatabase = "warehouse.db"\n'
                '[targets.dev]\nschema = "main"\n[vars]\nbatch = "1"\n'
            ),
            "sql/models/orders.sql": (
                "MODEL (description 'Orders.', materialized table);\n"
                'SELECT CAST(code AS BIGINT) AS amount FROM __source("raw_regions")\n'
            ),
        }
        | REGION_PROVIDER_FILES,
    )

    outcome, log = plan_before_sync(
        tmp_path, project, test_case.settings, ("source:raw_regions", "model:orders")
    )

    assert outcome["kind"] == "plan", log
    assert outcome["document"].get("cant_run") == test_case.expected_cant_run, (outcome, log)
