"""A sensor's function runs in the serving location per event, and `sqb sensor try` runs it locally."""

import json
import sys
from pathlib import Path
from typing import Any

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    SensorFunctionsTestCase,
    SensorHarness,
    SensorTrialCase,
    WorkerExecutionTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    scheduled_worker,
    wait_for_run,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.login.helpers import (
    gateway,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.sensors.helpers import (
    assert_ticks,
    assert_trials,
    cli_login,
    sensor_module,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.wheel.cli.helpers import (
    environment,
)

RECORDER: str = (
    "import json, os, time\nfrom pathlib import Path\nfrom urllib.parse import quote\n"
    "from pydantic import Field, SecretStr\n"
    "from pydantic_settings import SettingsConfigDict\n"
    "from sqlbuild.orchestration import RunRequest, SkipReason\n"
    "from sqlbuild.provider.classes.provider import Provider\n"
    "class Recorder(Provider):\n"
    '    """Records each sensor call and answers as the mode file says."""\n'
    "    feed_token: SecretStr = Field(alias='SQLBUILD_TEST_FEED_TOKEN')\n"
    "    api_key: SecretStr = Field(alias='SQLBUILD_TEST_API_KEY')\n"
    "    session_token: SecretStr = Field(alias='SQLBUILD_TEST_SESSION_TOKEN')\n"
    "    model_config = SettingsConfigDict(extra='forbid', arbitrary_types_allowed=True,\n"
    "                                      env_file=os.environ['SQLBUILD_TEST_ENV_FILE'])\n"
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
    "        if mode == 'raise':\n"
    "            feed = self.feed_token.get_secret_value()\n"
    "            raise ValueError(f'orders feed is down: GET https://feeds.example.test/v1/{feed}/orders'\n"
    "                             f'?next={quote(feed, safe=\"\")} refused '\n"
    "                             f'{json.dumps(self.api_key.get_secret_value())} for session '\n"
    "                             f'{self.session_token.get_secret_value()}')\n"
    "        if mode == 'sleep':\n            time.sleep(8)\n"
    "            with Path(os.environ['SQLBUILD_SENSOR_CALLS']).open('a') as out:\n"
    "                out.write(json.dumps({'sensor': sensor, 'woke': True}) + '\\n')\n"
    "        if mode == 'skip':\n            return SkipReason('Nothing to export')\n"
    "        if mode == 'request':\n            return RunRequest(job='orders_export_job')\n"
    "        if mode == 'list':\n"
    "            return [RunRequest(job='orders_export_job', run_key='a'),"
    " RunRequest(job='orders_export_job', run_key='b')]\n"
    "        if mode == 'bad':\n            return 'orders_export_job'\n"
    "        return None\n"
)

FILES: dict[str, str] = {
    "python/providers/recorder.py": RECORDER,
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


@pytest.mark.skipif(sys.platform != "linux", reason="Headless Linux credential-store acceptance")
@pytest.mark.parametrize(
    "test_case",
    [
        SensorFunctionsTestCase(
            description="sensor functions tick per event and try locally",
            worker=WorkerExecutionTestCase(
                description="orders with sensors",
                model_sql="SELECT 7 AS amount",
                warehouse_query="SELECT amount FROM orders",
                expected_status="success",
                expected_rows=[(7,)],
                files=FILES,
            ),
            expected_failure_code="sensor_function_failed",
            expected_failure=(
                "ValueError: orders feed is down: GET https://feeds.example.test/v1/****/orders"
                '?next=**** refused "****" for session ****'
            ),
            expected_started_job="orders_export_job",
            expected_trials=(
                SensorTrialCase(
                    description="a skip reports its reason",
                    mode="skip",
                    choice=("--latest",),
                    expected_exit=0,
                    expected_stdout="Skipped: Nothing to export",
                ),
                SensorTrialCase(
                    description="a list asks for its runs",
                    mode="list",
                    choice=("--event", "{event}"),
                    expected_exit=0,
                    expected_stdout="The function asked for 2 runs",
                ),
                SensorTrialCase(
                    description="None is handled",
                    mode="none",
                    choice=("--run", "{run}"),
                    expected_exit=0,
                    expected_stdout="Handled: the function returned None.",
                ),
                SensorTrialCase(
                    description="a raise shows its traceback",
                    mode="raise",
                    choice=("--latest",),
                    expected_exit=1,
                    expected_stdout="ValueError: orders feed is down",
                ),
                SensorTrialCase(
                    description="a slow call times out",
                    mode="sleep",
                    choice=("--latest",),
                    expected_exit=1,
                    expected_stdout="The function timed out after 5 seconds.",
                ),
                SensorTrialCase(
                    description="a string is refused",
                    mode="bad",
                    choice=("--latest",),
                    expected_exit=1,
                    expected_stdout="a sensor returns None, a SkipReason, a RunRequest",
                ),
            ),
            expected_masked={
                "SQLBUILD_TEST_FEED_TOKEN": "feed tok+7f3a/9c51",
                "SQLBUILD_TEST_API_KEY": 'orders"key\\48d1e2',
            },
            dotenv_secrets={"SQLBUILD_TEST_SESSION_TOKEN": "session-5b2e81c4"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sensor_functions_when_events_arrive_and_people_try_them_then_ticks_and_trials_report_what_the_function_did(
    test_case: SensorFunctionsTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    modes: Path = tmp_path / "modes.json"
    calls: Path = tmp_path / "calls.jsonl"
    calls.touch()
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
    for name, value in test_case.expected_masked.items():
        monkeypatch.setenv(name, value)
    dotenv: Path = tmp_path / "provider.env"
    dotenv.write_text(
        "".join(f"{name}={value}\n" for name, value in test_case.dotenv_secrets.items())
    )
    monkeypatch.setenv("SQLBUILD_TEST_ENV_FILE", str(dotenv))
    home: Path = tmp_path / "home"
    home.mkdir(parents=True)
    client_environment: dict[str, str] = environment({"HOME": str(home)})
    client_environment.pop("DBUS_SESSION_BUS_ADDRESS", None)
    client_environment.pop("XDG_RUNTIME_DIR", None)
    with (
        gateway(tmp_path) as (origin, certificate, transport),
        scheduled_worker(
            tmp_path,
            monkeypatch,
            test_case.worker,
            server_arguments=("--password-origin", origin),
            scheduler_settings={"server": {"password_origin": origin}},
        ) as worker,
    ):
        transport["backend"] = worker.api.url.removeprefix("http://")
        harness: SensorHarness = SensorHarness(
            root=tmp_path,
            origin=origin,
            certificate=certificate,
            modes=modes,
            calls=calls,
            binary=str(Path(sys.executable).with_name("sqb")),
            environment=client_environment,
        )
        source: dict[str, Any] = wait_for_run(
            worker.api, worker.process, worker.log, job_key="annual_orders_job"
        )
        assert source["status"] == test_case.worker.expected_status, worker.log.read_text()
        export, login = assert_ticks(worker, harness, test_case, source)
        cli_login(harness, login)
        assert_trials(worker, harness, test_case, (source, export))
