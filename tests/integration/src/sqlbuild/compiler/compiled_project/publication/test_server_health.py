"""A server whose control database can't be reached answers health, liveness and API errors with the real cause."""

import json
from pathlib import Path
from typing import Any
from urllib.parse import SplitResult, urlsplit

import pytest

from tests.integration.src.sqlbuild.compiler.compiled_project.publication._test_types import (
    ServerOutageTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    control_database_row,
    forwarded_port,
    free_port,
    http_reply,
    outage_server,
    postgres_database,
    wait_for_health,
)

DNS_REASON: str = "Can't reach the control database: control-db.invalid doesn't resolve (DNS)"


@pytest.mark.parametrize(
    "test_case",
    [
        ServerOutageTestCase(
            description="a host that doesn't resolve",
            database_url="postgresql://sqb:hunter2-secret@control-db.invalid:5432/sqb",
            expected_reason=DNS_REASON,
            expected_case="name_lookup",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_an_unresolvable_control_database_when_asked_then_health_names_the_dns_failure(
    test_case: ServerOutageTestCase, tmp_path: Path
) -> None:
    with outage_server(tmp_path / "server", test_case.database_url) as (url, log):
        report: dict[str, Any] = wait_for_health(
            url,
            lambda report: control_database_row(report)["reason"] == test_case.expected_reason,
            30,
        )
        health_status: int = http_reply(f"{url}/api/health")[0]
        live: tuple[int, str] = http_reply(f"{url}/livez")[0::2]
        api_status, api_headers, api_text = http_reply(f"{url}/api/control/bootstrap")
        api_body: dict[str, Any] = json.loads(api_text)
        unknown_status, unknown_headers, unknown_text = http_reply(f"{url}/api/no-such-thing")
        assert {
            "health": (health_status, report["status"], control_database_row(report)),
            "live": live,
            "api": (
                api_status,
                api_body["error"],
                api_body["cause"]["case"],
                api_body["request_id"] == api_headers["x-request-id"],
            ),
            "unknown api path while waiting": (
                unknown_status,
                unknown_headers["content-type"],
                json.loads(unknown_text)["error"],
            ),
            "password in the log": "hunter2-secret" in log.read_text(),
        } == {
            "health": (
                503,
                "unavailable",
                {
                    "status": "unavailable",
                    "reason": test_case.expected_reason,
                    "case": test_case.expected_case,
                },
            ),
            "live": (200, "ok\n"),
            "api": (503, test_case.expected_reason, test_case.expected_case, True),
            "unknown api path while waiting": (503, "application/json", test_case.expected_reason),
            "password in the log": False,
        }, test_case.description


@pytest.mark.parametrize(
    "test_case",
    [
        ServerOutageTestCase(
            description="a refused connection that a database answers later",
            database_url="",
            expected_reason="Can't reach the control database at 127.0.0.1:{port}",
            expected_case="unreachable",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_control_database_that_comes_back_when_asked_then_health_recovers_without_a_restart(
    test_case: ServerOutageTestCase, tmp_path: Path
) -> None:
    port: int = free_port()
    with postgres_database() as database:
        address: SplitResult = urlsplit(database)
        reachable_later: str = database.replace(
            f"{address.hostname}:{address.port}", f"127.0.0.1:{port}"
        )
        with outage_server(tmp_path / "server", reachable_later) as (url, log):
            refused: dict[str, Any] = wait_for_health(
                url,
                lambda report: control_database_row(report)["case"] == test_case.expected_case,
                90,
            )
            with forwarded_port(port, (str(address.hostname), int(address.port or 5432))):
                recovered: dict[str, Any] = wait_for_health(
                    url, lambda report: report["status"] == "healthy", 120
                )
                bootstrap_status: int = http_reply(f"{url}/api/control/bootstrap")[0]
                unknown_status, _, unknown_text = http_reply(f"{url}/api/no-such-thing")
        assert {
            "while refused": control_database_row(refused),
            "after": (recovered["status"], control_database_row(recovered)),
            "api no longer refused": bootstrap_status != 503,
            "unknown api path": (unknown_status, json.loads(unknown_text)["error"]),
            "ready": "Control API ready: " in log.read_text(),
        } == {
            "while refused": {
                "status": "unavailable",
                "reason": test_case.expected_reason.format(port=port),
                "case": test_case.expected_case,
            },
            "after": ("healthy", {"status": "healthy", "reason": None, "case": None}),
            "api no longer refused": True,
            "unknown api path": (404, "no such API endpoint"),
            "ready": True,
        }, test_case.description
