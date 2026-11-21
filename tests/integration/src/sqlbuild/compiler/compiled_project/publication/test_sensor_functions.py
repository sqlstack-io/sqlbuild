"""A sensor's function runs in the serving location per event, and `sqb sensor try` runs it locally."""

import json
import sqlite3
import subprocess
import sys
import time
from functools import partial
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    sensor_calls,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    browser,
    gateway,
    pending_login,
    request,
    sign_in,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    provision_person,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    environment,
)

RECORDER: str = (
    "import json, os, time\nfrom pathlib import Path\n"
    "from sqlbuild.orchestration import RunRequest, SkipReason\n"
    "from sqlbuild.provider.classes.provider import Provider\n"
    "class Recorder(Provider):\n"
    '    """Records each sensor call and answers as the mode file says."""\n'
    "    def call(self, ctx, sensor):\n"
    "        mode = json.loads(Path(os.environ['SQLBUILD_SENSOR_MODE']).read_text())[sensor]\n"
    "        event = ctx.event\n"
    "        status = getattr(event, 'status', None)\n"
    "        record = {'sensor': sensor, 'mode': mode, 'event_type': type(event).__name__,\n"
    "                  'event_id': getattr(event, 'id', None), 'job': getattr(event, 'job', None),\n"
    "                  'status': status, 'status_type': type(status).__name__,\n"
    "                  'url': getattr(event, 'url', None), 'cursor': ctx.cursor}\n"
    "        with Path(os.environ['SQLBUILD_SENSOR_CALLS']).open('a') as out:\n"
    "            out.write(json.dumps(record) + '\\n')\n"
    "        if mode == 'raise':\n            raise ValueError('orders feed is down')\n"
    "        if mode == 'sleep':\n            time.sleep(30)\n"
    "        if mode == 'skip':\n            return SkipReason('Nothing to export')\n"
    "        if mode == 'request':\n            return RunRequest(job='orders_export_job')\n"
    "        if mode == 'list':\n"
    "            return [RunRequest(job='orders_export_job', run_key='a'),"
    " RunRequest(job='orders_export_job', run_key='b')]\n"
    "        if mode == 'bad':\n            return 'orders_export_job'\n"
    "        return None\n"
)


def sensor_module(name: str, settings: str) -> str:
    """A decorated sensor whose function hands its call to the recorder provider."""
    return (
        "from sqlbuild.orchestration import RunEventKind, run_event, sensor\n\n\n"
        f"@sensor({settings})\n"
        f"def {name}(ctx, recorder):\n"
        f"    return recorder.call(ctx, {name!r})\n"
    )


FILES: dict[str, str] = {
    "providers/recorder.py": RECORDER,
    "orchestration/jobs/orders_export_job.toml": (
        'name = "orders_export_job"\ndescription = "Export orders."\nselect = "orders"\n'
    ),
    "orchestration/sensors/orders_done_sensor.py": sensor_module(
        "orders_done_sensor",
        'description="React when orders succeed", '
        'when=run_event(job="annual_orders_job", kinds=[RunEventKind.SUCCESS]), '
        "minimum_interval_seconds=1, timeout_seconds=5",
    ),
    "orchestration/sensors/orders_failed_sensor.py": sensor_module(
        "orders_failed_sensor",
        'description="React when orders fail", '
        'when=run_event(job="annual_orders_job", kinds=[RunEventKind.FAILED])',
    ),
    "orchestration/sensors/orders_poll_sensor.py": sensor_module(
        "orders_poll_sensor", 'description="Poll orders", minimum_interval_seconds=3600'
    ),
}


def tick_rows(store: Path, sensor: str, phase: str) -> list[dict[str, Any]]:
    """The recorded facts of `sensor`'s ticks in `phase`, oldest first."""
    with sqlite3.connect(store) as control:
        rows: list[tuple[str]] = control.execute(
            "SELECT e.fact_json FROM sqb_sensor_evaluation_events e "
            "JOIN sqb_triggers t ON t.trigger_id=e.trigger_id "
            "WHERE t.name=? AND e.phase=? ORDER BY e.occurred_at",
            (sensor, phase),
        ).fetchall()
    return [json.loads(row) for (row,) in rows]


def wait_until(condition: Any, what: str, timeout: float = 45) -> Any:
    """The first truthy value of `condition()`, polled until `timeout`."""
    deadline: float = time.monotonic() + timeout
    while not (value := condition()):
        assert time.monotonic() < deadline, what
        time.sleep(0.1)
    return value


def assert_ticks(
    worker: Any, harness: dict[str, Any], source: dict[str, Any]
) -> tuple[dict[str, Any], tuple[Any, str]]:
    """Real ticks: a raise fails with its traceback and retries; a good tick starts the run once."""
    store: Path = worker.control_store
    origin: str = harness["origin"]
    calls: Path = harness["calls"]
    mode = harness["mode"]
    event_id: str = f"run:{source['run_id']}"
    # A raise fails the tick with its traceback, leaves the event undelivered and retries it.
    wait_until(
        lambda: len(tick_rows(store, "orders_done_sensor", "failed")) >= 2,
        "a failed event is retried after the minimum interval",
    )
    failed: dict[str, Any] = tick_rows(store, "orders_done_sensor", "failed")[0]
    assert failed["request"]["event"]["id"] == event_id
    assert failed["outcome"]["code"] == "sensor_function_failed"
    assert "Traceback (most recent call last)" in failed["outcome"]["error"]
    assert "ValueError: orders feed is down" in failed["outcome"]["error"]
    admin: dict[str, Any] = provision_person(harness["root"] / "control", "create-user", "orders-admin")
    client, _ = browser(origin, harness["certificate"])
    csrf: str = sign_in(client, origin, "password", admin["name"], "orders-person-password-2026")
    target_path: str = (
        f"{origin}/api/control/locations/{worker.api.location_id}/targets/development"
    )

    def trigger(name: str) -> dict[str, Any]:
        status, payload = request(client, f"{target_path}/triggers")
        assert status == 200, payload
        return next(t for t in json.loads(payload) if t["name"] == name)

    def sensor_state(name: str) -> dict[str, Any]:
        status, payload = request(
            client, f"{target_path}/triggers/{trigger(name)['trigger_id']}/sensor"
        )
        assert status == 200, payload
        return json.loads(payload)

    failing: dict[str, Any] = trigger("orders_done_sensor")["sensor_failing"]
    assert (failing["event_id"], failing["failed_ticks"] >= 1) == (event_id, True)
    assert trigger("orders_done_sensor").get("job_key") is None
    latest: dict[str, Any] = next(
        e for e in sensor_state("orders_done_sensor")["evaluations"] if e["outcome"] != "running"
    )
    assert latest["outcome"] == "failed" and latest["event_id"] == event_id
    assert "ValueError: orders feed is down" in latest["error"]

    # The next good tick delivers the same event, starts the run once and withdraws the failure.
    mode("orders_done_sensor", "request")
    export: dict[str, Any] = wait_for_run(
        worker.api, worker.process, worker.log, job_key="orders_export_job"
    )
    started: dict[str, Any] = wait_until(
        lambda: next(
            (
                e
                for e in sensor_state("orders_done_sensor")["evaluations"]
                if e["outcome"] == "started"
            ),
            None,
        ),
        "a started tick",
    )
    assert (started["event_id"], started["run_ids"]) == (event_id, [export["run_id"]])
    assert trigger("orders_done_sensor").get("sensor_failing") is None
    with sqlite3.connect(store) as control:
        assert control.execute(
            "SELECT run_key FROM sqb_run_requests WHERE run_id=?", (export["run_id"],)
        ).fetchall() == [(f"{event_id}:orders_export_job",)]
    ticked: list[dict[str, Any]] = sensor_calls(calls, {"sensor": "orders_done_sensor"})
    assert {(c["event_type"], c["event_id"], c["job"], c["status"]) for c in ticked} == {
        ("RunEvent", event_id, "annual_orders_job", "success")
    }, "at least once: every delivery carries the same event ID"
    assert {(c["status_type"], c["url"]) for c in ticked} == {
        ("RunEventKind", f"{origin}/runs/{source['run_id']}")
    }
    polled: list[dict[str, Any]] = wait_until(
        lambda: sensor_calls(calls, {"sensor": "orders_poll_sensor"}), "a polling tick"
    )
    assert (polled[0]["event_type"], polled[0]["cursor"]) == ("NoneType", None)
    return export, (client, csrf)


def assert_trials(
    worker: Any,
    harness: dict[str, Any],
    runs: tuple[dict[str, Any], dict[str, Any]],
    login: tuple[Any, str],
) -> None:
    """`sqb sensor try` with a saved session: local code, real providers, nothing recorded."""
    source, export = runs
    client, csrf = login
    store: Path = worker.control_store
    origin: str = harness["origin"]
    certificate: Path = harness["certificate"]
    calls: Path = harness["calls"]
    mode = harness["mode"]
    invoke = harness["invoke"]
    binary: str = harness["binary"]
    tmp_path: Path = harness["root"]
    client_environment: dict[str, str] = harness["environment"]
    event_id: str = f"run:{source['run_id']}"
    common: list[str] = [
        "--server",
        origin,
        "--ca-cert",
        str(certificate),
        "--location-id",
        worker.api.location_id,
        "--target",
        "development",
        "--project-dir",
        str(worker.project),
    ]
    with pending_login(
        [binary, "login", "--server", origin, "--ca-cert", str(certificate), "--no-browser"],
        client_environment,
        tmp_path,
        tmp_path / "cli-login.log",
    ) as (process, url):
        assert (
            request(
                client,
                origin + "/api/auth/cli/approve",
                {"browser_code": url.split("#", 1)[1], "approve": True},
                csrf=csrf,
            )[0]
            == 204
        )
        _ = process.communicate(timeout=20)
        assert process.returncode == 0

    def attempt(sensor: str, *choice: str) -> subprocess.CompletedProcess[str]:
        return invoke([binary, "sensor", "try", sensor, *choice, *common])

    def recorded() -> tuple[int, int, int]:
        with sqlite3.connect(store) as control:
            return (
                len(calls.read_text().splitlines()),
                control.execute("SELECT count(*) FROM sqb_sensor_evaluation_events").fetchone()[0],
                control.execute("SELECT count(*) FROM sqb_run_requests").fetchone()[0],
            )

    before: tuple[int, int, int] = recorded()
    listed: subprocess.CompletedProcess[str] = attempt("orders_done_sensor")
    assert listed.returncode == 0, listed.stderr
    assert event_id in listed.stdout and "--event <id>" in listed.stdout, listed.stdout
    assert recorded() == before, "a bare try lists events without calling the function"
    for conflicting in (
        ["--latest", "--event", event_id],
        ["--run", source["run_id"], "--latest"],
    ):
        refused: subprocess.CompletedProcess[str] = attempt("orders_done_sensor", *conflicting)
        assert refused.returncode == 2, refused.stderr
    assert attempt("orders_done_sensor", "--limit", "0").returncode == 2

    cases: list[tuple[str, tuple[str, ...], int, str]] = [
        ("skip", ("--latest",), 0, "Skipped: Nothing to export"),
        ("list", ("--event", event_id), 0, "The function asked for 2 runs; try starts none"),
        ("none", ("--run", source["run_id"]), 0, "Handled: the function returned None."),
        ("raise", ("--latest",), 1, "ValueError: orders feed is down"),
        ("sleep", ("--latest",), 1, "The function timed out after 5 seconds."),
        ("bad", ("--latest",), 1, "a sensor returns None, a SkipReason, a RunRequest"),
    ]
    for answer, choice, code, expected in cases:
        mode("orders_done_sensor", answer)
        tried: subprocess.CompletedProcess[str] = attempt("orders_done_sensor", *choice)
        assert (tried.returncode, expected in tried.stdout) == (code, True), (
            answer,
            tried.stdout,
            tried.stderr,
        )
        assert "really happens" in tried.stderr
        assert (
            sensor_calls(calls, {"sensor": "orders_done_sensor", "mode": answer})[-1]["event_id"]
            == event_id
        )
    calls_made, ticks, requests = recorded()
    assert (ticks, requests) == before[1:], "a trial records no tick and starts no run"
    assert calls_made == before[0] + len(cases)

    mismatch: subprocess.CompletedProcess[str] = attempt(
        "orders_done_sensor", "--run", export["run_id"]
    )
    assert mismatch.returncode == 1
    assert "but the sensor watches annual_orders_job" in mismatch.stderr
    unmatched: subprocess.CompletedProcess[str] = attempt(
        "orders_failed_sensor", "--run", source["run_id"]
    )
    assert unmatched.returncode == 1
    assert "ended success, which the sensor's kinds (failed) don't match" in unmatched.stderr
    nothing: subprocess.CompletedProcess[str] = attempt("orders_failed_sensor")
    assert (nothing.returncode, nothing.stdout) == (
        0,
        "No recent events match orders_failed_sensor's condition.\n",
    )
    assert attempt("orders_failed_sensor", "--latest").returncode == 1
    poll_listed: subprocess.CompletedProcess[str] = attempt("orders_poll_sensor")
    assert poll_listed.returncode == 0 and "polls instead of reacting" in poll_listed.stdout
    polled_once: subprocess.CompletedProcess[str] = attempt("orders_poll_sensor", "--latest")
    assert polled_once.returncode == 0 and "Handled" in polled_once.stdout, polled_once.stderr
    assert sensor_calls(calls, {"sensor": "orders_poll_sensor"})[-1]["event_type"] == "NoneType"


@pytest.mark.skipif(sys.platform != "linux", reason="Headless Linux credential-store acceptance")
@pytest.mark.parametrize(
    "test_case",
    [
        WorkerExecutionTestCase(
            description="sensor functions tick per event and try locally",
            model_sql="SELECT 7 AS amount",
            warehouse_query="SELECT amount FROM orders",
            expected_status="success",
            expected_rows=[(7,)],
            files=FILES,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sensor_functions_when_events_arrive_and_people_try_them_then_ticks_and_trials_report_what_the_function_did(
    test_case: WorkerExecutionTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    modes: Path = tmp_path / "modes.json"
    calls: Path = tmp_path / "calls.jsonl"
    calls.touch()

    def mode(sensor: str, value: str) -> None:
        current: dict[str, str] = json.loads(modes.read_text())
        modes.write_text(json.dumps(current | {sensor: value}))

    modes.write_text(
        json.dumps(
            {
                "orders_done_sensor": "raise",
                "orders_failed_sensor": "none",
                "orders_poll_sensor": "none",
            }
        )
    )
    monkeypatch.setenv("SQLBUILD_SENSOR_MODE", str(modes))
    monkeypatch.setenv("SQLBUILD_SENSOR_CALLS", str(calls))
    home: Path = tmp_path / "home"
    home.mkdir()
    client_environment: dict[str, str] = environment({"HOME": str(home)})
    client_environment.pop("DBUS_SESSION_BUS_ADDRESS", None)
    client_environment.pop("XDG_RUNTIME_DIR", None)
    binary: str = str(Path(sys.executable).with_name("sqb"))
    invoke = partial(
        subprocess.run,
        cwd=tmp_path,
        env=client_environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )
    with (
        gateway(tmp_path) as (origin, certificate, transport),
        scheduled_worker(
            tmp_path, monkeypatch, test_case, server_arguments=("--password-origin", origin)
        ) as worker,
    ):
        transport["backend"] = worker.api.url.removeprefix("http://")
        source: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        assert source["status"] == "success", worker.log.read_text()

        harness: dict[str, Any] = {
            "origin": origin,
            "certificate": certificate,
            "calls": calls,
            "mode": mode,
            "invoke": invoke,
            "binary": binary,
            "root": tmp_path,
            "environment": client_environment,
        }
        export, login = assert_ticks(worker, harness, source)
        assert_trials(worker, harness, (source, export), login)
