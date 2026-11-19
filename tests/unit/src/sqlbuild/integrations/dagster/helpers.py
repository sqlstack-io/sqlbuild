from __future__ import annotations

import json
from collections.abc import Mapping, Set
from enum import StrEnum
from pathlib import Path
from typing import Any

from sqlbuild._native import IntegrationResultEnvelope
from sqlbuild.integrations.dagster.classes.sqlbuild_dagster_translator import (
    SqlBuildDagsterTranslator,
)


def assert_exhaustive_enum_partition[EnumT: StrEnum](
    *, all_members: Set[EnumT], included: Set[EnumT], excluded: Set[EnumT]
) -> None:
    assert included.isdisjoint(excluded)
    assert included | excluded == all_members


def integration_result_payload(
    *, command: str, asset: Mapping[str, object], event_type: str = "resource_attempt_completed"
) -> dict[str, object]:
    asset_payload: dict[str, object] = dict(asset)
    duration_ms: object | None = asset_payload.pop("duration_ms", None)
    error_code: object | None = asset_payload.pop("error_code", None)
    _ = asset_payload.pop("error_message", None)
    _ = asset_payload.pop("error_help", None)
    terminal_facts: dict[str, tuple[object | None, object | None, object | None]] = {
        "resource_attempt_completed": (None, None, None),
        "resource_attempt_failed": (error_code, "RuntimeError", None),
        "resource_attempt_skipped": (None, None, "explicit"),
    }
    canonical_error_code, canonical_error_type, canonical_skip_code = terminal_facts[event_type]
    return {
        "schema_version": 1,
        "record_kind": "integration_result",
        "event_id": f"event-{asset['name']}",
        "event_sequence": 0,
        "event_type": event_type,
        "occurred_at": "2026-09-02T12:00:00+00:00",
        "invocation_id": "invocation-1",
        "run_id": "run-1",
        "resource_id": f"{asset['kind']}:{asset['name']}",
        "resource_attempt_id": f"attempt-{asset['name']}",
        "operation_id": None,
        "statement_id": None,
        "resource_kind": asset["kind"],
        "resource_name": asset["name"],
        "attempt_number": 1,
        "duration_ms": duration_ms,
        "output_kind": "asset",
        "command": command,
        "error_code": canonical_error_code,
        "error_type": canonical_error_type,
        "skip_code": canonical_skip_code,
        "skip_mode": None,
        "asset": asset_payload,
        "checks": [],
    }


def build_check_integration_envelope(
    *,
    check_id: str,
    name: str,
    event_id: str,
    attempt_id: str,
    event_sequence: int,
    command: str = "audit",
    check_overrides: Mapping[str, object] | None = None,
) -> IntegrationResultEnvelope:
    check: dict[str, object] = {
        "kind": "audit",
        "audit_definition_name": "test_audit",
        "name": name,
        "check_id": check_id,
        "dag_check_id": check_id,
        "passed": True,
        "status": "pass",
        "severity": "error",
        "asset_name": "orders",
        "attachment_kind": "model",
        "attached_column_name": "order_id",
        "attached_target_name": "orders",
        "run_scope_phase": "final",
        "row_count": 0,
        "reused": False,
        **(check_overrides or {}),
    }
    return IntegrationResultEnvelope.from_json(
        json.dumps(
            {
                "schema_version": 1,
                "record_kind": "integration_result",
                "event_id": event_id,
                "event_sequence": event_sequence,
                "event_type": "resource_attempt_completed",
                "occurred_at": "2026-09-02T12:00:00+00:00",
                "invocation_id": "invocation-1",
                "run_id": "run-1",
                "resource_id": check_id,
                "resource_attempt_id": attempt_id,
                "operation_id": None,
                "statement_id": None,
                "resource_kind": "audit",
                "resource_name": name,
                "attempt_number": 1,
                "duration_ms": 1,
                "output_kind": "check",
                "command": command,
                "checks": [check],
            }
        )
    )


def build_dagster_test_dag() -> Mapping[str, Any]:
    return {
        "version": 1,
        "project_name": "dagster_project",
        "nodes": [
            {
                "id": "source:raw_orders",
                "kind": "source",
                "name": "raw_orders",
                "asset_key": ["raw", "orders"],
                "path": "sources/raw.yml",
            },
            {
                "id": "loader:shared_order_feed",
                "kind": "loader",
                "name": "shared_order_feed",
                "asset_key": ["shared_order_feed"],
                "path": "python/loaders/orders.py",
            },
            {
                "id": "loader:raw_orders_loader",
                "kind": "loader",
                "name": "raw_orders_loader",
                "asset_key": ["raw_orders_loader"],
                "path": "python/loaders/orders.py",
            },
            {
                "id": "seed:waffle_types",
                "kind": "seed",
                "name": "waffle_types",
                "asset_key": ["analytics", "waffle_types"],
                "path": "seeds/waffle_types.csv",
            },
            {
                "id": "udf:normalize_email",
                "kind": "udf",
                "name": "normalize_email",
                "asset_key": ["analytics", "normalize_email"],
                "path": "functions/normalize_email.sql",
                "language": "sql",
            },
            {
                "id": "model:orders",
                "kind": "model",
                "name": "orders",
                "asset_key": ["analytics", "orders"],
                "path": "models/orders.sql",
                "description": "Clean orders",
                "sql": 'MODEL (materialized table);\n\nSELECT * FROM __source("raw", "orders")',
                "tags": ["daily"],
                "materialization_type": "table",
            },
            {
                "id": "model:customers",
                "kind": "model",
                "name": "customers",
                "asset_key": ["analytics", "customers"],
                "path": "models/customers.sql",
                "materialization_type": "view",
            },
        ],
        "edges": [
            {"from_id": "loader:shared_order_feed", "to_id": "loader:raw_orders_loader"},
            {"from_id": "loader:raw_orders_loader", "to_id": "source:raw_orders"},
            {"from_id": "source:raw_orders", "to_id": "model:orders"},
            {"from_id": "udf:normalize_email", "to_id": "model:orders"},
        ],
        "checks": [
            {
                "id": "audit:not_null:model:orders:order_id",
                "kind": "audit",
                "name": "not_null",
                "checked_asset_ids": ["model:orders"],
                "path": "audits/not_null.sql",
                "attached_column_name": "order_id",
            },
            {
                "id": "audit:freshness:source:raw_orders:loaded_at",
                "kind": "audit",
                "name": "freshness",
                "checked_asset_ids": ["source:raw_orders"],
                "path": "audits/freshness.sql",
                "attached_column_name": "loaded_at",
            },
            {
                "id": "sql_scenario:orders_minimal",
                "kind": "scenario",
                "name": "orders_minimal",
                "checked_asset_ids": ["model:orders"],
                "path": "tests/scenarios/orders_minimal.sql",
            },
            {
                "id": "sql_scenario:customers_minimal",
                "kind": "scenario",
                "name": "customers_minimal",
                "checked_asset_ids": ["model:customers"],
                "path": "tests/scenarios/customers_minimal.sql",
            },
        ],
    }


def build_python_augmented_dagster_test_dag() -> Mapping[str, Any]:
    dag: dict[str, Any] = dict(build_dagster_test_dag())
    dag["nodes"] = [
        *dag["nodes"],
        {
            "id": "asset:prepare_orders",
            "kind": "asset",
            "name": "prepare_orders",
            "asset_key": ["task", "prepare_orders"],
            "path": "python/tasks/prepare_orders.py",
            "tags": ["daily"],
            "group": "python",
            "meta": {"owner": "data"},
        },
        {
            "id": "asset:orders_export",
            "kind": "asset",
            "name": "orders_export",
            "asset_key": ["asset", "orders_export"],
            "path": "python/assets/orders_export.py",
            "tags": ["daily"],
            "group": "exports",
            "columns": [{"name": "order_id", "type": "integer"}],
            "column_lineage": {"order_id": [{"node": "prepare_orders", "column": "order_id"}]},
            "materialization_type": "python_asset",
        },
        {
            "id": "check:check_orders_export",
            "kind": "check",
            "name": "check_orders_export",
            "asset_key": ["check", "check_orders_export"],
            "path": "python/checks/check_orders_export.py",
            "tags": ["daily"],
        },
    ]
    dag["edges"] = [
        *dag["edges"],
        {"from_id": "model:orders", "to_id": "asset:prepare_orders"},
        {"from_id": "asset:prepare_orders", "to_id": "asset:orders_export"},
        {"from_id": "asset:orders_export", "to_id": "check:check_orders_export"},
    ]
    dag["checks"] = [
        *dag["checks"],
        {
            "id": "check:check_orders_export",
            "kind": "python_check",
            "name": "check_orders_export",
            "checked_asset_ids": ["asset:orders_export"],
            "path": "python/checks/check_orders_export.py",
            "severity": "error",
            "tags": ["daily"],
        },
    ]
    return dag


def write_fake_sqb_command(
    *,
    root: Path,
    stdout: str = "",
    stderr: str = "",
    exit_code: int = 0,
    expected_args: tuple[str, ...] | None = None,
    environment_output_path: Path | None = None,
) -> list[str]:
    script_path: Path = root / "fake_sqb.py"
    script_path.write_text(
        "\n".join(
            (
                "from __future__ import annotations",
                "from pathlib import Path",
                "import os",
                "import sys",
                f"expected_args = {expected_args!r}",
                f"environment_output_path = {str(environment_output_path)!r}",
                "if environment_output_path is not None:",
                "    Path(environment_output_path).write_text(",
                "        os.environ.get('SQLBUILD_INVOCATION_CONTEXT_JSON', ''), encoding='utf-8'",
                "    )",
                "if expected_args is not None and tuple(sys.argv[1:]) != expected_args:",
                "    actual_args = tuple(sys.argv[1:])",
                "    sys.stderr.write(f'expected args {expected_args!r}, got {actual_args!r}\\n')",
                "    raise SystemExit(99)",
                "if len(sys.argv) >= 4 and tuple(sys.argv[1:3]) == ('compile', '--dag'):",
                f"    Path(sys.argv[3]).write_text({stdout!r}, encoding='utf-8')",
                "if '--json-output' in sys.argv[1:]:",
                "    json_output_path = Path(sys.argv[sys.argv.index('--json-output') + 1])",
                f"    json_output_path.write_text({stdout!r}, encoding='utf-8')",
                "elif any(arg.startswith('--json-output=') for arg in sys.argv[1:]):",
                "    json_output_arg = next("
                "arg for arg in sys.argv[1:] if arg.startswith('--json-output='))",
                "    json_output_path = Path(json_output_arg.split('=', 1)[1])",
                f"    json_output_path.write_text({stdout!r}, encoding='utf-8')",
                "else:",
                f"    sys.stdout.write({stdout!r})",
                f"sys.stderr.write({stderr!r})",
                f"raise SystemExit({exit_code})",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ["python", str(script_path)]


def write_blocking_fake_sqb_command(*, root: Path, release_path: Path) -> list[str]:
    script_path: Path = root / "blocking_fake_sqb.py"
    script_path.write_text(
        "\n".join(
            (
                "from pathlib import Path",
                "import sys",
                "import time",
                "sys.stdout.write('started without explicit flush\\n')",
                "sys.stderr.write('warning without explicit flush\\n')",
                f"release_path = Path({str(release_path)!r})",
                "while not release_path.exists():",
                "    time.sleep(0.01)",
                "sys.stdout.write('completed\\n')",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ["python", str(script_path)]


def write_blocking_execution_event_command(
    *, root: Path, release_path: Path, command: str, asset_name: str = "customers"
) -> list[str]:
    event_payload: str = (
        json.dumps(
            integration_result_payload(
                command=command,
                asset={
                    "kind": "model",
                    "name": asset_name,
                    "status": "success",
                },
            )
        )
        + "\n"
    )
    execution_payload: str = json.dumps(
        {
            "version": 1,
            "command": command,
            "status": "success",
            "summary": {"success_count": 1},
            "assets": [
                {
                    "kind": "model",
                    "name": asset_name,
                    "status": "success",
                }
            ],
            "checks": [],
        }
    )
    event_write_lines: tuple[str, ...] = (
        "with event_path.open('a', encoding='utf-8') as stream:\n"
        f"    stream.write({event_payload[: len(event_payload) // 2]!r})\n"
        "    stream.flush()\n"
        "    time.sleep(0.05)\n"
        f"    stream.write({event_payload[len(event_payload) // 2 :]!r})\n"
        "    stream.flush()",
    )
    script_path: Path = root / f"blocking_{command}_event.py"
    script_path.write_text(
        "\n".join(
            (
                "from pathlib import Path",
                "import os",
                "import sys",
                "import time",
                "event_path = Path(os.environ['SQLBUILD_INTEGRATION_RESULT_PATH'])",
                *event_write_lines,
                f"release_path = Path({str(release_path)!r})",
                "while not release_path.exists():",
                "    time.sleep(0.01)",
                "json_path = Path(sys.argv[sys.argv.index('--json-output') + 1])",
                f"json_path.write_text({execution_payload!r}, encoding='utf-8')",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ["python", str(script_path)]


def write_blocking_audit_event_command(*, root: Path, release_path: Path) -> list[str]:
    envelope: IntegrationResultEnvelope = build_check_integration_envelope(
        check_id="audit:not_null:model:orders:order_id",
        name="not_null",
        event_id="event-live-audit",
        attempt_id="attempt-live-audit",
        event_sequence=1,
    )
    event_payload: str = envelope.to_json()
    execution_payload: str = json.dumps(
        {
            "version": 1,
            "command": "audit",
            "status": "success",
            "summary": {"pass_count": 1},
            "assets": [],
            "checks": [json.loads(envelope.to_json())["checks"][0]],
        }
    )
    script_path: Path = root / "blocking_audit_event.py"
    script_path.write_text(
        "\n".join(
            (
                "from pathlib import Path",
                "import os",
                "import sys",
                "import time",
                "event_path = Path(os.environ['SQLBUILD_INTEGRATION_RESULT_PATH'])",
                "with event_path.open('a', encoding='utf-8') as stream:",
                f"    stream.write({event_payload!r})",
                "    stream.flush()",
                f"release_path = Path({str(release_path)!r})",
                "while not release_path.exists():",
                "    time.sleep(0.01)",
                "json_path = Path(sys.argv[sys.argv.index('--json-output') + 1])",
                f"json_path.write_text({execution_payload!r}, encoding='utf-8')",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ["python", str(script_path)]


def write_blocking_failed_execution_event_command(*, root: Path, release_path: Path) -> list[str]:
    asset: dict[str, object] = {
        "kind": "model",
        "name": "customers",
        "status": "failed",
        "failed_phase": "staging",
        "error_code": "R002",
        "error_message": "invalid identifier CUSTOMER_ID",
        "error_help": "Check the projected columns.",
        "staging_relation": "analytics.customers__staging",
        "duration_ms": 123,
    }
    event_payload: str = json.dumps(
        integration_result_payload(
            command="build", asset=asset, event_type="resource_attempt_failed"
        )
    )
    execution_payload: str = json.dumps(
        {
            "version": 1,
            "command": "build",
            "status": "failed",
            "summary": {"failure_count": 1},
            "assets": [asset],
            "checks": [],
        }
    )
    script_path: Path = root / "blocking_failed_build_event.py"
    script_path.write_text(
        "\n".join(
            (
                "from pathlib import Path",
                "import os",
                "import sys",
                "import time",
                "event_path = Path(os.environ['SQLBUILD_INTEGRATION_RESULT_PATH'])",
                "with event_path.open('a', encoding='utf-8') as stream:",
                f"    stream.write({event_payload + chr(10)!r})",
                f"    stream.write({event_payload + chr(10)!r})",
                "    stream.flush()",
                f"release_path = Path({str(release_path)!r})",
                "while not release_path.exists():",
                "    time.sleep(0.01)",
                "json_path = Path(sys.argv[sys.argv.index('--json-output') + 1])",
                f"json_path.write_text({execution_payload!r}, encoding='utf-8')",
                "raise SystemExit(1)",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ["python", str(script_path)]


def write_dagster_test_dag(*, root: Path) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    dag_path: Path = root / "sqlbuild_dag.json"
    dag_path.write_text(json.dumps(build_dagster_test_dag()), encoding="utf-8")
    return dag_path


def write_python_augmented_dagster_test_dag(*, root: Path) -> Path:
    root.mkdir(parents=True, exist_ok=True)
    dag_path: Path = root / "sqlbuild_dag.json"
    dag_path.write_text(json.dumps(build_python_augmented_dagster_test_dag()), encoding="utf-8")
    return dag_path


def assert_select_file_selector_behavior(
    *, command: tuple[str, ...], selectors: tuple[str, ...]
) -> None:
    assert "--select-file" in command
    select_file_index: int = command.index("--select-file") + 1
    assert not Path(command[select_file_index]).exists()
    assert all(selector not in command for selector in selectors)


def assert_positional_selector_behavior(
    *, command: tuple[str, ...], selectors: tuple[str, ...]
) -> None:
    assert "--select-file" not in command
    for selector in selectors:
        assert selector in command


def assert_json_output_file_behavior(*, command: tuple[str, ...]) -> None:
    assert "--json-output" in command
    assert "--json" not in command
    json_output_index: int = command.index("--json-output") + 1
    assert not Path(command[json_output_index]).exists()


def _selection_model_node(name: str) -> dict[str, Any]:
    return {
        "id": f"model:{name}",
        "kind": "model",
        "name": name,
        "asset_key": ["analytics", name],
        "path": f"models/{name}.sql",
        "materialization_type": "table",
    }


def _selection_edge(from_name: str, to_name: str) -> dict[str, str]:
    return {"from_id": f"model:{from_name}", "to_id": f"model:{to_name}"}


def build_dagster_selection_graph_dag() -> Mapping[str, Any]:
    return {
        "version": 1,
        "project_name": "dagster_selection_project",
        "nodes": [
            _selection_model_node("stg_orders"),
            _selection_model_node("orders"),
            _selection_model_node("customers"),
            _selection_model_node("raw_events"),
            _selection_model_node("events_left"),
            _selection_model_node("events_right"),
            _selection_model_node("events_joined"),
            _selection_model_node("events_report"),
            _selection_model_node("events_side"),
        ],
        "edges": [
            _selection_edge("stg_orders", "orders"),
            _selection_edge("stg_orders", "customers"),
            _selection_edge("raw_events", "events_left"),
            _selection_edge("raw_events", "events_right"),
            _selection_edge("raw_events", "events_side"),
            _selection_edge("events_left", "events_joined"),
            _selection_edge("events_right", "events_joined"),
            _selection_edge("events_joined", "events_report"),
        ],
        "checks": [],
    }


class RecordingDagsterTranslator(SqlBuildDagsterTranslator):
    """Default Dagster translator that records the checks and nodes it is asked about."""

    def __init__(self) -> None:
        self.asset_checks: list[Mapping[str, Any]] = []
        self.asset_nodes: list[Mapping[str, Any]] = []

    def is_asset_node(self, node: Mapping[str, Any]) -> bool:
        self.asset_nodes.append(node)
        return super().is_asset_node(node)

    def is_asset_check(self, check: Mapping[str, Any]) -> bool:
        self.asset_checks.append(check)
        return super().is_asset_check(check)
