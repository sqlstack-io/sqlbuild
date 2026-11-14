"""Calendar callbacks remain in locations; native publication and refresh preserve good state."""

import json
import sqlite3
import subprocess
import sys
from dataclasses import replace
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any
from urllib.error import HTTPError

import duckdb
import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    TYPE_PROOF_FILES,
    duckshop_orchestration,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    CalendarRefreshTestCase,
    CalendarRejectionTestCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    current_snapshot,
    location_process,
    scheduled_worker,
    snapshot_api,
    wait_for_generation,
    wait_for_location_health,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.graph.helpers import write_files


@pytest.mark.parametrize(
    "test_case",
    [
        CalendarRejectionTestCase(
            "naive time", "[datetime(2026, 1, 1)]", "timezone-aware datetimes"
        ),
        CalendarRejectionTestCase("exclusive end", "[window.end]", "half-open evaluation window"),
        CalendarRejectionTestCase(
            "before start",
            "[window.start - timedelta(microseconds=1)]",
            "half-open evaluation window",
        ),
        CalendarRejectionTestCase(
            "non-datetime", "['2026-01-01T00:00:00Z']", "timezone-aware datetimes"
        ),
        CalendarRejectionTestCase(
            "unbounded generator", "repeat(window.start)", "exceeds 10000 upcoming times"
        ),
        CalendarRejectionTestCase("zero horizon", "[]", "positive fixed duration", horizon="0s"),
        CalendarRejectionTestCase(
            "overflowing horizon",
            "[]",
            "positive fixed duration",
            horizon="999999999999999999999999999d",
        ),
        CalendarRejectionTestCase(
            "unknown zone", "[]", "is not an IANA time zone", timezone="Unknown/Orders"
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_calendar_results_when_publishing_through_the_cli_then_validation_is_bounded_and_no_snapshot_is_committed(
    test_case: CalendarRejectionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '\n[targets.development]\nschema = "main"\n',
            "orchestration/jobs/order_reporting_job.toml": (
                'name = "order_reporting_job"\n'
                'description = "Order reporting."\n'
                'select = "orders"\n'
                f'schedule = {{ calendar = "deliveries_calendar", timezone = {json.dumps(test_case.timezone)} }}\n'
            ),
            "orchestration/calendars/deliveries_calendar.py": (
                "from datetime import datetime, timedelta\nfrom itertools import repeat\n"
                "from sqlbuild.orchestration import calendar\n"
                f"@calendar(name='deliveries_calendar', description='Delivery times.', horizon={test_case.horizon!r})\n"
                f"def deliveries(window):\n    return {test_case.expression}\n"
            ),
        },
    )
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        result: subprocess.CompletedProcess[str] = subprocess.run(
            [
                str(Path(sys.executable).with_name("sqb")),
                "--project-dir",
                str(project),
                "location",
                "publish",
                "--server",
                api.url,
                "--location-id",
                api.location_id,
                "--target",
                "development",
                "--generation",
                "1",
            ],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        assert result.returncode != 0
        assert test_case.expected_message in result.stderr
        with pytest.raises(HTTPError) as failure:
            current_snapshot(api, "development")
        assert failure.value.code == 404


@pytest.mark.parametrize(
    "test_case",
    [
        CalendarRefreshTestCase("offset-aware calendar refresh", "America/New_York"),
        CalendarRefreshTestCase("UTC calendar refresh", "UTC"),
    ],
    ids=lambda case: case.description,
)
def test_given_a_served_calendar_when_time_passes_and_a_provider_fails_then_refresh_reuses_the_interpreter_and_keeps_last_good(
    test_case: CalendarRefreshTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project: Path = tmp_path / "project"
    data: Path = tmp_path / "fulfillment.json"
    evaluations: Path = tmp_path / "evaluations.jsonl"
    data.write_text(json.dumps({"offsets": [3, 0, 3]}))
    monkeypatch.setenv("SQLBUILD_FULFILLMENT_DATA", str(data))
    monkeypatch.setenv("SQLBUILD_CALENDAR_EVALUATIONS", str(evaluations))
    write_files(
        project_dir=project,
        files=TYPE_PROOF_FILES
        | {
            "sqlbuild_project.toml": TYPE_PROOF_FILES["sqlbuild_project.toml"]
            + '\n[connections.local]\ndatabase = "warehouse.duckdb"\n'
            + '\n[targets.development]\nschema = "main"\n',
            **{
                path: text.replace("America/New_York", test_case.expected_timezone)
                for path, text in duckshop_orchestration("calendar_orders").items()
            },
            "orchestration/jobs/production_reporting_job.toml": (
                'name = "production_reporting_job"\n'
                'description = "Other target."\n'
                'select = "orders"\n'
                'targets = ["production"]\n'
                'schedule = { calendar = "excluded_calendar", timezone = "UTC" }\n'
            ),
            "orchestration/calendars/excluded_calendar.py": (
                "from sqlbuild.orchestration import calendar\n"
                "@calendar(name='excluded_calendar', description='Other target.', horizon='8s')\n"
                "def excluded_calendar(window):\n    raise ValueError('excluded calendar must not evaluate')\n"
            ),
            **duckshop_orchestration("platform_orders"),
            "providers/fulfillment.py": (
                "import json, os\nfrom pathlib import Path\nfrom datetime import timedelta, timezone\n"
                "from sqlbuild.provider.classes.provider import Provider\n"
                "class Fulfillment(Provider):\n"
                '    """Upcoming synthetic fulfillment times."""\n'
                "    def setup(self, ctx):\n        self.record({'phase': 'setup'})\n"
                "    def teardown(self):\n        self.record({'phase': 'teardown'})\n"
                "    def record(self, value):\n"
                "        with Path(os.environ['SQLBUILD_CALENDAR_EVALUATIONS']).open('a') as out:\n"
                "            out.write(json.dumps(value | {'pid': os.getpid()}) + '\\n')\n"
                "    def calendar(self, window):\n"
                "        self.record({'phase': 'evaluate', 'start': window.start.isoformat(), 'end': window.end.isoformat(), 'zone': window.start.tzinfo.key})\n"
                "        data = json.loads(Path(os.environ['SQLBUILD_FULFILLMENT_DATA']).read_text())\n"
                "        if data.get('fail'):\n            raise ValueError('synthetic calendar unavailable')\n"
                "        start = window.start.astimezone(timezone.utc)\n"
                "        return [start + timedelta(seconds=offset) for offset in data['offsets']]\n"
            ),
        },
    )
    with snapshot_api(tmp_path / "control") as api:
        monkeypatch.setenv("SQLBUILD_LOCATION_TOKEN", api.bearer)
        monkeypatch.setenv("SQLBUILD_CONTROL_DATABASE_URL", "unsupported://must-not-open")
        log: Path = tmp_path / "location.log"
        publications: Path = tmp_path / "publications.jsonl"
        arguments: list[str] = [
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
        ]
        with location_process(arguments, log, publications) as process:
            first: dict[str, Any] = wait_for_generation(api, "development", 1, process, log)
            second: dict[str, Any] = wait_for_generation(api, "development", 2, process, log)
            assert first["nodes"] == second["nodes"]
            assert second["changes"] == []
            assert first["documents"]["triggers"].keys() == {
                "order_reporting_job",
                "overdue_orders_sensor",
            }
            assert first["documents"]["triggers"]["order_reporting_job"]["declared_at"] == {
                "path": "orchestration/jobs/order_reporting_job.toml",
                "line": 2,
            }
            with sqlite3.connect(tmp_path / "control/.sqlbuild/control.sqlite3") as control:
                stored, next_due = control.execute(
                    "SELECT definition_json, next_due_at FROM sqb_triggers JOIN sqb_trigger_state USING (trigger_id) WHERE kind = 'schedule'"
                ).fetchone()
            original_times: list[str] = first["documents"]["triggers"]["order_reporting_job"][
                "schedule"
            ]["times"]
            assert json.loads(stored)["schedule"]["times"] == sorted(
                original_times
                + second["documents"]["triggers"]["order_reporting_job"]["schedule"]["times"]
            )
            assert datetime.fromisoformat(next_due) == datetime.fromisoformat(original_times[0])
            observations: list[dict[str, Any]] = [
                json.loads(line) for line in evaluations.read_text().splitlines()
            ]
            assert [row["phase"] for row in observations] == ["setup", "evaluate", "teardown"] * 2
            successful: list[dict[str, Any]] = observations[1::3]
            assert len(successful) == 2
            assert len({row["pid"] for row in observations}) == 1
            assert all(row["zone"] == test_case.expected_timezone for row in successful)
            for snapshot, evaluation in zip([first, second], successful, strict=True):
                start: datetime = datetime.fromisoformat(evaluation["start"])
                end: datetime = datetime.fromisoformat(evaluation["end"])
                assert end - start == timedelta(seconds=8)
                schedule: dict[str, Any] = snapshot["documents"]["triggers"]["order_reporting_job"][
                    "schedule"
                ]
                assert schedule["kind"] == "calendar"
                assert [datetime.fromisoformat(value) for value in schedule["times"]] == [
                    start + timedelta(seconds=offset) for offset in [0, 3]
                ]
                assert datetime.fromisoformat(schedule["refresh_at"]) == start + timedelta(
                    seconds=4
                )
            data.write_text(json.dumps({"fail": True}))
            _ = wait_for_location_health(
                api, "development", {"kind": "compile_failed", "generation": 3}, process, log
            )
            assert current_snapshot(api, "development")["record"] == second["record"]
            observations = [json.loads(line) for line in evaluations.read_text().splitlines()]
            assert [row["phase"] for row in observations] == ["setup", "evaluate", "teardown"] * 3
            data.write_text(json.dumps({"offsets": [1]}))
            (project / "models/stg_orders.sql").write_text(
                TYPE_PROOF_FILES["models/stg_orders.sql"].replace(
                    "CAST(5 AS INTEGER)", "CAST(19 AS INTEGER)"
                )
            )
            third: dict[str, Any] = wait_for_generation(api, "development", 3, process, log)
            assert "CAST(19 AS INTEGER)" in third["nodes"]["model:stg_orders"]["raw_sql"]
            assert (
                len(third["documents"]["triggers"]["order_reporting_job"]["schedule"]["times"]) == 1
            )
        assert [
            json.loads(line)["generation"] for line in publications.read_text().splitlines()
        ] == [1, 2, 3]
        assert api.bearer not in log.read_text() + publications.read_text()
        assert not (project / ".sqlbuild/control.sqlite3").exists()


@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="calendar evaluation belongs only to the location",
            model_sql="SELECT 29 AS amount",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(29,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_published_calendar_when_a_real_worker_compiles_then_it_does_not_evaluate_the_calendar_again(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    evaluated: Path = tmp_path / "evaluated"
    test_case = replace(
        test_case,
        files=test_case.files
        | {
            "orchestration/jobs/order_reporting_job.toml": (
                'name = "order_reporting_job"\n'
                'description = "Order reporting."\n'
                'select = "orders"\n'
                'schedule = { cron = "0 0 * * *", timezone = "UTC", '
                'starts_at = "2000-01-01T00:00:00Z" }\n'
            ),
            "orchestration/jobs/deliveries_job.toml": (
                'name = "deliveries_job"\n'
                'description = "Upcoming deliveries."\n'
                'select = "orders"\n'
                'schedule = { calendar = "deliveries_calendar", timezone = "UTC" }\n'
            ),
            "orchestration/calendars/deliveries_calendar.py": (
                "from pathlib import Path\nfrom sqlbuild.orchestration import calendar\n"
                "@calendar(name='deliveries_calendar', description='Upcoming deliveries.', horizon='1d')\n"
                "def deliveries(window):\n"
                f"    marker = Path({str(evaluated)!r})\n"
                "    if marker.exists():\n        raise ValueError('calendar evaluated inside a run worker')\n"
                "    marker.write_text('evaluated')\n    return []\n"
            ),
        },
        default_job_files={},
    )
    with scheduled_worker(tmp_path, monkeypatch, test_case) as worker:
        assert evaluated.read_text() == "evaluated"
        completed: dict[str, Any] = wait_for_run(worker.api, worker.process, worker.log)
        assert completed["status"] == test_case.expected_status, worker.log.read_text()
        with duckdb.connect(str(worker.project / "warehouse.duckdb"), read_only=True) as warehouse:
            assert warehouse.sql(test_case.warehouse_query).fetchall() == test_case.expected_rows
