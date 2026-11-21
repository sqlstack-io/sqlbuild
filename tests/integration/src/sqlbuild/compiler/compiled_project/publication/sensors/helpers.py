"""Drive real sensor ticks and `sqb sensor try` against a scheduled worker, recording each call."""

import json
import sqlite3
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ScheduledWorker,
    SensorFunctionsTestCase,
    SensorHarness,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    sensor_calls,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    browser,
    pending_login,
    request,
    sign_in,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.person.helpers import (
    provision_person,
)


def sensor_module(name: str, settings: str) -> str:
    """A decorated sensor whose function hands its call to the recorder provider."""
    return (
        "from sqlbuild.orchestration import RunEventKind, run_event, sensor\n\n\n"
        f"@sensor({settings})\n"
        f"def {name}(ctx, recorder):\n"
        f"    return recorder.call(ctx, {name!r})\n"
    )


def set_mode(harness: SensorHarness, sensor: str, mode: str) -> None:
    """Make `sensor`'s function answer as `mode` from its next call on."""
    current: dict[str, str] = json.loads(harness.modes.read_text())
    harness.modes.write_text(json.dumps(current | {sensor: mode}))


def run_cli(harness: SensorHarness, arguments: list[str]) -> subprocess.CompletedProcess[str]:
    """Run the installed `sqb` with the person's saved session and the recorder's files."""
    return subprocess.run(
        [harness.binary, *arguments],
        cwd=harness.root,
        env=harness.environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )


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


def wait_until(condition: Callable[[], Any], what: str, timeout: float = 45) -> Any:
    """The first truthy value of `condition()`, polled until `timeout`."""
    deadline: float = time.monotonic() + timeout
    while not (value := condition()):
        assert time.monotonic() < deadline, what
        time.sleep(0.1)
    return value


def person_reads(harness: SensorHarness) -> tuple[Any, str]:
    """An administrator's signed-in browser client and its CSRF token."""
    admin: dict[str, Any] = provision_person(
        harness.root / "control", "create-user", "orders-admin"
    )
    client: Any
    client, _ = browser(harness.origin, harness.certificate)
    csrf: str = sign_in(
        client, harness.origin, "password", admin["name"], "orders-person-password-2026"
    )
    return client, csrf


def read_json(client: Any, url: str) -> Any:
    """A person read the client may make."""
    status, payload = request(client, url)
    assert status == 200, payload
    return json.loads(payload)


def trigger(client: Any, target_path: str, name: str) -> dict[str, Any]:
    """The listed trigger named `name`."""
    return next(filter(lambda t: t["name"] == name, read_json(client, f"{target_path}/triggers")))


def evaluations(client: Any, target_path: str, name: str) -> list[dict[str, Any]]:
    """The sensor's recent ticks, newest first."""
    trigger_id: str = trigger(client, target_path, name)["trigger_id"]
    return read_json(client, f"{target_path}/triggers/{trigger_id}/sensor")["evaluations"]


def assert_ticks(
    worker: ScheduledWorker,
    harness: SensorHarness,
    test_case: SensorFunctionsTestCase,
    source: dict[str, Any],
) -> tuple[dict[str, Any], tuple[Any, str]]:
    """Real ticks: a raise fails with its traceback and retries; a good tick starts the run once."""
    event_id: str = f"run:{source['run_id']}"
    wait_until(
        lambda: len(tick_rows(worker.control_store, "orders_done_sensor", "failed")) >= 2,
        "a failed event is retried after the minimum interval",
    )
    failed: dict[str, Any] = tick_rows(worker.control_store, "orders_done_sensor", "failed")[0]
    assert (failed["request"]["event"]["id"], failed["outcome"]["code"]) == (
        event_id,
        test_case.expected_failure_code,
    )
    assert "Traceback (most recent call last)" in failed["outcome"]["error"]
    assert test_case.expected_failure in failed["outcome"]["error"]
    assert not any(
        map(lambda secret: secret in json.dumps(failed), test_case.expected_masked.values())
    )
    client, csrf = person_reads(harness)
    target_path: str = (
        f"{harness.origin}/api/control/locations/{worker.api.location_id}/targets/development"
    )
    failing: dict[str, Any] = trigger(client, target_path, "orders_done_sensor")["sensor_failing"]
    assert (failing["event_id"], failing["failed_ticks"] >= 1) == (event_id, True)
    assert trigger(client, target_path, "orders_done_sensor").get("job_key") is None
    latest: dict[str, Any] = next(
        filter(
            lambda e: e["outcome"] != "running",
            evaluations(client, target_path, "orders_done_sensor"),
        )
    )
    assert (latest["outcome"], latest["event_id"]) == ("failed", event_id)
    assert test_case.expected_failure in latest["error"]

    # A call past timeout_seconds times out, and the location kills it before it wakes.
    set_mode(harness, "orders_done_sensor", "sleep")
    wait_until(
        lambda: tick_rows(worker.control_store, "orders_done_sensor", "timed_out"),
        "a call past its timeout times out",
    )
    time.sleep(5)
    assert sensor_calls(harness.calls, {"sensor": "orders_done_sensor", "woke": True}) == []

    # The next good tick delivers the same event, starts the run once and withdraws the failure.
    set_mode(harness, "orders_done_sensor", "request")
    export: dict[str, Any] = wait_for_run(
        worker.api, worker.process, worker.log, job_key=test_case.expected_started_job
    )
    started: dict[str, Any] = wait_until(
        lambda: next(
            filter(
                lambda e: e["outcome"] == "started",
                evaluations(client, target_path, "orders_done_sensor"),
            ),
            None,
        ),
        "a started tick",
    )
    assert (started["event_id"], started["run_ids"]) == (event_id, [export["run_id"]])
    assert trigger(client, target_path, "orders_done_sensor").get("sensor_failing") is None
    with sqlite3.connect(worker.control_store) as control:
        assert control.execute(
            "SELECT run_key FROM sqb_run_requests WHERE run_id=?", (export["run_id"],)
        ).fetchall() == [(f"{event_id}:{test_case.expected_started_job}",)]
    ticked: list[dict[str, Any]] = sensor_calls(harness.calls, {"sensor": "orders_done_sensor"})
    assert {(c["event_type"], c["event_id"], c["job"], c["status"]) for c in ticked} == {
        ("RunEvent", event_id, "annual_orders_job", "success")
    }, "at least once: every delivery carries the same event ID"
    assert {(c["status_type"], c["url"]) for c in ticked} == {
        ("RunEventKind", f"{harness.origin}/runs/{source['run_id']}")
    }
    polled: list[dict[str, Any]] = wait_until(
        lambda: sensor_calls(harness.calls, {"sensor": "orders_poll_sensor"}), "a polling tick"
    )
    assert (polled[0]["event_type"], polled[0]["cursor"]) == ("NoneType", None)
    return export, (client, csrf)


def cli_login(harness: SensorHarness, login: tuple[Any, str]) -> None:
    """Save a CLI session for the signed-in administrator, approved in the browser."""
    client, csrf = login
    with pending_login(
        [
            harness.binary,
            "login",
            "--server",
            harness.origin,
            "--ca-cert",
            str(harness.certificate),
            "--no-browser",
        ],
        harness.environment,
        harness.root,
        harness.root / "cli-login.log",
    ) as (process, url):
        assert (
            request(
                client,
                harness.origin + "/api/auth/cli/approve",
                {"browser_code": url.split("#", 1)[1], "approve": True},
                csrf=csrf,
            )[0]
            == 204
        )
        _ = process.communicate(timeout=20)
        assert process.returncode == 0


def recorded(harness: SensorHarness, store: Path) -> tuple[int, int, int]:
    """Function calls, recorded tick phases and run requests so far."""
    with sqlite3.connect(store) as control:
        return (
            len(harness.calls.read_text().splitlines()),
            control.execute("SELECT count(*) FROM sqb_sensor_evaluation_events").fetchone()[0],
            control.execute("SELECT count(*) FROM sqb_run_requests").fetchone()[0],
        )


def assert_trials(
    worker: ScheduledWorker,
    harness: SensorHarness,
    test_case: SensorFunctionsTestCase,
    runs: tuple[dict[str, Any], dict[str, Any]],
) -> None:
    """`sqb sensor try` with a saved session: local code, real providers, nothing recorded."""
    source, export = runs
    event_id: str = f"run:{source['run_id']}"
    common: list[str] = [
        "--server",
        harness.origin,
        "--ca-cert",
        str(harness.certificate),
        "--location-id",
        worker.api.location_id,
        "--target",
        "development",
        "--project-dir",
        str(worker.project),
    ]

    def attempt(sensor: str, *choice: str) -> subprocess.CompletedProcess[str]:
        return run_cli(harness, ["sensor", "try", sensor, *choice, *common])

    before: tuple[int, int, int] = recorded(harness, worker.control_store)
    listed: subprocess.CompletedProcess[str] = attempt("orders_done_sensor")
    assert listed.returncode == 0, listed.stderr
    assert event_id in listed.stdout and "--event <id>" in listed.stdout, listed.stdout
    assert recorded(harness, worker.control_store) == before, "a bare try calls nothing"
    for conflicting in (["--latest", "--event", event_id], ["--run", source["run_id"], "--latest"]):
        assert attempt("orders_done_sensor", *conflicting).returncode == 2
    assert attempt("orders_done_sensor", "--limit", "0").returncode == 2
    for trial in test_case.expected_trials:
        set_mode(harness, "orders_done_sensor", trial.mode)
        choice: list[str] = [
            part.format(event=event_id, run=source["run_id"]) for part in trial.choice
        ]
        tried: subprocess.CompletedProcess[str] = attempt("orders_done_sensor", *choice)
        assert (tried.returncode, trial.expected_stdout in tried.stdout) == (
            trial.expected_exit,
            True,
        ), (trial, tried.stdout, tried.stderr)
        assert "really happens" in tried.stderr
        assert not any(
            map(
                lambda secret: secret in tried.stdout + tried.stderr,
                test_case.expected_masked.values(),
            )
        ), tried.stdout
        assert (
            sensor_calls(harness.calls, {"sensor": "orders_done_sensor", "mode": trial.mode})[-1][
                "event_id"
            ]
            == event_id
        )
    calls, ticks, requests = recorded(harness, worker.control_store)
    assert (ticks, requests) == before[1:], "a trial records no tick and starts no run"
    assert calls == before[0] + len(test_case.expected_trials)
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
    assert sensor_calls(harness.calls, {"sensor": "orders_poll_sensor"})[-1]["event_type"] == (
        "NoneType"
    )


RECORDING_SENSORS: dict[str, str] = {
    "orchestration/sensors/orders_checks_sensor.py": (
        "import json, os\nfrom pathlib import Path\n"
        "from sqlbuild.orchestration import CheckEventKind, check_event, sensor\n\n\n"
        "@sensor(description='Record failed and warned checks.',\n"
        "        when=check_event(select='{select}', kinds=[CheckEventKind.FAILED, CheckEventKind.WARN]),\n"
        "        minimum_interval_seconds=1)\n"
        "def orders_checks_sensor(ctx):\n"
        "    with Path(os.environ['SQLBUILD_SENSOR_CALLS']).open('a') as out:\n"
        "        out.write(json.dumps({'sensor': 'orders_checks_sensor', 'check': ctx.event.check,\n"
        "                              'status': str(ctx.event.status)}) + '\\n')\n"
    ),
    "orchestration/sensors/orders_run_sensor.py": (
        "import json, os\nfrom pathlib import Path\n"
        "from sqlbuild.orchestration import RunEventKind, run_event, sensor\n\n\n"
        "@sensor(description='Record the finished run.',\n"
        "        when=run_event(job='annual_orders_job', kinds=[RunEventKind.COMPLETED]),\n"
        "        minimum_interval_seconds=1)\n"
        "def orders_run_sensor(ctx):\n"
        "    with Path(os.environ['SQLBUILD_SENSOR_CALLS']).open('a') as out:\n"
        "        out.write(json.dumps({'sensor': 'orders_run_sensor', 'status': str(ctx.event.status),\n"
        "                              'failed_checks': ctx.event.failed_checks,\n"
        "                              'failed_nodes': ctx.event.failed_nodes}) + '\\n')\n"
    ),
}


def recording_sensors(select: str) -> dict[str, str]:
    """Both recording sensors, the check sensor watching the checks on `select`."""
    return {path: text.replace("{select}", select) for path, text in RECORDING_SENSORS.items()}
