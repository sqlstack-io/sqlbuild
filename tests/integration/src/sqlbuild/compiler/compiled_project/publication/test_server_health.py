"""A server whose control database can't be reached answers health, liveness and API errors with the real cause."""

from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

from tests.integration.src.sqlbuild.compiler.compiled_project.publication.helpers import (
    control_database_row,
    forwarded_port,
    free_port,
    http_reply,
    outage_server,
    postgres_database,
    wait_for_health,
)

UNRESOLVABLE_DATABASE: str = "postgresql://sqb:hunter2-secret@control-db.invalid:5432/sqb"
DNS_REASON: str = "Can't reach the control database: control-db.invalid doesn't resolve (DNS)"


def test_given_an_unresolvable_control_database_when_asked_then_health_names_the_dns_failure(
    tmp_path: Path,
) -> None:
    with outage_server(tmp_path / "server", UNRESOLVABLE_DATABASE) as (url, log):
        report: dict[str, Any] = wait_for_health(
            url, lambda report: control_database_row(report)["reason"] == DNS_REASON, 30
        )
        health_status, _, _ = http_reply(f"{url}/api/health")
        live: tuple[int, Any] = http_reply(f"{url}/livez")[0::2]
        api_status, api_headers, api_body = http_reply(f"{url}/api/control/bootstrap")
        unknown_status, unknown_headers, unknown_body = http_reply(f"{url}/api/no-such-thing")
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
                unknown_body["error"],
            ),
            "password in the log": "hunter2-secret" in log.read_text(),
        } == {
            "health": (
                503,
                "unavailable",
                {"status": "unavailable", "reason": DNS_REASON, "case": "name_lookup"},
            ),
            "live": (200, "ok\n"),
            "api": (503, DNS_REASON, "name_lookup", True),
            "unknown api path while waiting": (503, "application/json", DNS_REASON),
            "password in the log": False,
        }


def test_given_a_control_database_that_comes_back_when_asked_then_health_recovers_without_a_restart(
    tmp_path: Path,
) -> None:
    port: int = free_port()
    with postgres_database() as database:
        address = urlsplit(database)
        reachable_later: str = database.replace(
            f"{address.hostname}:{address.port}", f"127.0.0.1:{port}"
        )
        with outage_server(tmp_path / "server", reachable_later) as (url, log):
            refused: dict[str, Any] = wait_for_health(
                url, lambda report: control_database_row(report)["case"] == "unreachable", 90
            )
            with forwarded_port(port, (str(address.hostname), int(address.port or 5432))):
                recovered: dict[str, Any] = wait_for_health(
                    url, lambda report: report["status"] == "healthy", 120
                )
                bootstrap_status: int = http_reply(f"{url}/api/control/bootstrap")[0]
                unknown_status, _, unknown_body = http_reply(f"{url}/api/no-such-thing")
        assert {
            "while refused": control_database_row(refused),
            "after": (recovered["status"], control_database_row(recovered)),
            "api no longer refused": bootstrap_status != 503,
            "unknown api path": (unknown_status, unknown_body["error"]),
            "ready": "Control API ready: " in log.read_text(),
        } == {
            "while refused": {
                "status": "unavailable",
                "reason": f"Can't reach the control database at 127.0.0.1:{port}",
                "case": "unreachable",
            },
            "after": ("healthy", {"status": "healthy", "reason": None, "case": None}),
            "api no longer refused": True,
            "unknown api path": (404, "no such API endpoint"),
            "ready": True,
        }
