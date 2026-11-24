"""Helpers for Dagster integration e2e tests."""

from __future__ import annotations

import time
from collections import defaultdict
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from threading import Event
from types import MappingProxyType
from typing import Any

from dagster import (
    AssetCheckEvaluation,
    AssetCheckSeverity,
    AssetKey,
    AssetMaterialization,
    ExecuteInProcessResult,
)

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import REPO_ROOT


def prepare_python_nodes_integration_project(root: Path) -> Path:
    """Create a small project with Python task, asset, loader, and check nodes.

    Its loader depends on a task that reads SQL, so the project allows warehouse round trips.
    """

    project_dir: Path = root / "python_nodes_project"
    models_dir: Path = project_dir / "sql" / "models"
    tasks_dir: Path = project_dir / "python" / "nodes" / "tasks"
    factories_dir: Path = project_dir / "python" / "nodes" / "factories"
    assets_dir: Path = project_dir / "python" / "nodes" / "assets"
    loaders_dir: Path = project_dir / "python" / "nodes" / "loaders"
    checks_dir: Path = project_dir / "python" / "nodes" / "checks"
    for directory in (models_dir, tasks_dir, factories_dir, assets_dir, loaders_dir, checks_dir):
        directory.mkdir(parents=True)
    (project_dir / "sqlbuild_project.toml").write_text(
        "\n".join(
            (
                'name = "python_nodes_project"',
                'adapter = "duckdb"',
                'default_target = "dev"',
                "",
                "[connection]",
                'database = "python_nodes.duckdb"',
                "",
                "[targets.dev]",
                'schema = "main"',
                "",
                "[settings]",
                "allow_warehouse_round_trips = true",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    (models_dir / "orders.sql").write_text(
        "MODEL (description 'Test model orders.', materialized view);\n\nSELECT 1 AS order_id\n",
        encoding="utf-8",
    )
    (factories_dir / "generated_nodes").mkdir()
    (factories_dir / "generated_nodes" / "factory.py").write_text(
        "\n".join(
            (
                "from sqlbuild.assets import asset",
                "from sqlbuild.checks import check",
                "from sqlbuild.factories import factory",
                "from sqlbuild.loaders import loader",
                "from sqlbuild.refs import model",
                "from sqlbuild.assets import asset",
                "",
                "@factory",
                "def generated_nodes():",
                "    @asset(depends_on=model('orders'), tags=['daily'], group='python')",
                "    def prepare_orders(ctx):",
                "        '''Test task prepare_orders.'''",
                "        return ctx.result(payload={'rows': 1})",
                "",
                "    @loader(name='warehouse_export', depends_on=(prepare_orders,))",
                "    def warehouse_export(ctx):",
                "        '''Test loader warehouse_export.'''",
                "        return [{'order_id': 1}]",
                "",
                "    @asset(",
                "        depends_on=(prepare_orders, warehouse_export),",
                "        tags=['external'],",
                "        group='exports',",
                "        columns=[{'name': 'order_id', 'type': 'integer'}],",
                "    )",
                "    def orders_export(ctx):",
                "        '''Test asset orders_export.'''",
                "        return ctx.result(materialized=True)",
                "",
                "    @check(depends_on=orders_export, tags=['quality'], group='exports')",
                "    def check_orders_export(ctx):",
                "        '''Test check check_orders_export.'''",
                "        return True",
                "",
                "    return [prepare_orders, warehouse_export, orders_export, check_orders_export]",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return project_dir


def write_sqb_capture_command(
    *, root: Path, command_log_path: Path, selector_log_path: Path
) -> tuple[str, ...]:
    """Write a wrapper that records sqb args and select-file contents before exec."""

    sqb_executable: Path = REPO_ROOT / ".venv" / "bin" / "sqb"
    script_path: Path = root / "capture_sqb.py"
    script_path.write_text(
        "\n".join(
            (
                "from __future__ import annotations",
                "import os",
                "import sys",
                "from pathlib import Path",
                f"command_log_path = Path({str(command_log_path)!r})",
                f"selector_log_path = Path({str(selector_log_path)!r})",
                f"sqb_executable = {str(sqb_executable)!r}",
                "args = sys.argv[1:]",
                "command_log_path.write_text(' '.join(args), encoding='utf-8')",
                "if '--select-file' in args:",
                "    selector_path = Path(args[args.index('--select-file') + 1])",
                "    selector_contents = selector_path.read_text(encoding='utf-8')",
                "    selector_log_path.write_text(selector_contents, encoding='utf-8')",
                "os.execv(sqb_executable, (sqb_executable, *args))",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ("python", str(script_path))


def write_sqb_streaming_command(
    *,
    root: Path,
    started_path: Path,
    release_path: Path,
    stdout_text: str,
    json_payload: str,
) -> tuple[str, ...]:
    """Write a wrapper that emits stdout before waiting for test release."""

    script_path: Path = root / "streaming_sqb.py"
    script_path.write_text(
        "\n".join(
            (
                "from __future__ import annotations",
                "import sys",
                "import time",
                "from pathlib import Path",
                f"started_path = Path({str(started_path)!r})",
                f"release_path = Path({str(release_path)!r})",
                f"stdout_text = {stdout_text!r}",
                f"json_payload = {json_payload!r}",
                "sys.stdout.write(stdout_text)",
                "sys.stdout.flush()",
                "started_path.write_text('started', encoding='utf-8')",
                "while not release_path.exists():",
                "    time.sleep(0.01)",
                "if '--json-output' in sys.argv[1:]:",
                "    json_output_path = Path(sys.argv[sys.argv.index('--json-output') + 1])",
                "    json_output_path.write_text(json_payload, encoding='utf-8')",
                "raise SystemExit(0)",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    return ("python", str(script_path))


def wait_for_captured_stdout_fragment(
    *, capsys: Any, expected_fragment: str, deadline: float
) -> str:
    """Wait until pytest capture has received a stdout fragment."""

    rendered_stdout: str = ""
    poll_wait: Event = Event()
    try:
        while time.monotonic() < deadline:
            rendered_stdout += capsys.readouterr().out
            _STDOUT_POLL_OUTCOMES[expected_fragment in rendered_stdout](rendered_stdout, poll_wait)
    except _CapturedStdout as captured:
        return captured.rendered_stdout
    return rendered_stdout


class _CapturedStdout(Exception):
    def __init__(self, rendered_stdout: str) -> None:
        self.rendered_stdout = rendered_stdout


def _finish_stdout_poll(rendered_stdout: str, poll_wait: Event) -> None:
    del poll_wait
    raise _CapturedStdout(rendered_stdout)


def _continue_stdout_poll(rendered_stdout: str, poll_wait: Event) -> None:
    del rendered_stdout
    poll_wait.wait(0.01)


_STDOUT_POLL_OUTCOMES: MappingProxyType[bool, Callable[[str, Event], None]] = MappingProxyType(
    {False: _continue_stdout_poll, True: _finish_stdout_poll}
)


def add_failing_daily_revenue_audits(*, project_dir: Path) -> None:
    """Add violation and measurement failures to the copied project."""

    audits_dir: Path = (
        project_dir / "sql" / "models" / "marts" / "_sqlbuild" / "_audits" / "generic"
    )
    audits_dir.mkdir(parents=True, exist_ok=True)
    failing_audit_sql: str = 'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE 1 = 1\n'
    (audits_dir / "forced_warning_failure.sql").write_text(
        failing_audit_sql,
        encoding="utf-8",
    )
    (audits_dir / "forced_error_failure.sql").write_text(
        failing_audit_sql,
        encoding="utf-8",
    )
    (audits_dir / "forced_measurement_warning.sql").write_text(
        """AUDIT (
  evaluation measurement,
  value measured_value,
  sample_count sample_count,
  sample_unit rows,
);

MEASURE (
  SELECT 95.0 AS measured_value, 100 AS sample_count
  FROM @relation
  LIMIT 1
);
""",
        encoding="utf-8",
    )
    model_path: Path = project_dir / "sql" / "models" / "marts" / "daily_revenue.sql"
    contents: str = model_path.read_text(encoding="utf-8")
    contents = contents.replace(
        """    expression_is_true (
      name "revenue_is_non_negative",
      expression "total_revenue_cents >= 0",
    ),""",
        """    expression_is_true (
      name "revenue_is_non_negative",
      expression "total_revenue_cents >= 0",
    ),
    forced_warning_failure (
      severity "warn",
    ),
    forced_error_failure (
      severity "error",
    ),
    forced_measurement_warning (
      minimum_samples 100,
      thresholds (warn (below 100), error (below 90)),
    ),""",
    )
    model_path.write_text(contents, encoding="utf-8")


def materialization_metadata_keys(
    *, result: ExecuteInProcessResult, asset_key: tuple[str, ...]
) -> frozenset[str]:
    """Return metadata keys for a materialized asset in a Dagster e2e result."""

    expected_asset_key: AssetKey = AssetKey(list(asset_key))
    metadata_keys_by_asset: dict[AssetKey, frozenset[str]] = {}
    for event in result.get_asset_materialization_events():
        materialization: AssetMaterialization = event.step_materialization_data.materialization
        metadata_keys_by_asset.setdefault(
            materialization.asset_key,
            frozenset(str(key) for key in materialization.metadata),
        )
    return metadata_keys_by_asset.get(expected_asset_key, frozenset())


def check_names_for_asset(
    *, result: ExecuteInProcessResult, asset_key: tuple[str, ...]
) -> frozenset[str]:
    """Return Dagster check names emitted for one asset in an e2e result."""

    expected_asset_key: AssetKey = AssetKey(list(asset_key))
    names_by_asset: defaultdict[AssetKey, set[str]] = defaultdict(set)
    for evaluation in result.get_asset_check_evaluations():
        names_by_asset[evaluation.asset_key].add(evaluation.check_name)
    return frozenset(names_by_asset[expected_asset_key])


def check_severity_for_asset(
    *, result: ExecuteInProcessResult, asset_key: tuple[str, ...], check_name: str
) -> AssetCheckSeverity | None:
    """Return Dagster-native severity for one emitted asset check."""

    expected_asset_key: AssetKey = AssetKey(list(asset_key))
    severity_by_check: dict[tuple[AssetKey, str], AssetCheckSeverity] = {}
    for evaluation in result.get_asset_check_evaluations():
        severity_by_check.setdefault(
            (evaluation.asset_key, evaluation.check_name), evaluation.severity
        )
    return severity_by_check.get((expected_asset_key, check_name))


def failed_check_severities_for_asset(
    *, result: ExecuteInProcessResult, asset_key: tuple[str, ...]
) -> dict[str, AssetCheckSeverity]:
    """Return failed Dagster check severities for one asset."""

    expected_asset_key: AssetKey = AssetKey(list(asset_key))
    severities_by_asset: defaultdict[AssetKey, dict[str, AssetCheckSeverity]] = defaultdict(dict)
    for evaluation in result.get_asset_check_evaluations():
        _CHECK_EVALUATION_COLLECTORS[evaluation.passed](severities_by_asset, evaluation)
    return severities_by_asset[expected_asset_key]


def _record_failed_check(
    severities_by_asset: defaultdict[AssetKey, dict[str, AssetCheckSeverity]],
    evaluation: AssetCheckEvaluation,
) -> None:
    severities_by_asset[evaluation.asset_key][evaluation.check_name] = evaluation.severity


def _ignore_passed_check(
    severities_by_asset: defaultdict[AssetKey, dict[str, AssetCheckSeverity]],
    evaluation: AssetCheckEvaluation,
) -> None:
    del severities_by_asset, evaluation


_CHECK_EVALUATION_COLLECTORS: MappingProxyType[
    bool,
    Callable[[defaultdict[AssetKey, dict[str, AssetCheckSeverity]], AssetCheckEvaluation], None],
] = MappingProxyType({False: _record_failed_check, True: _ignore_passed_check})


def dagster_compute_log_job_source(*, root: Path) -> str:
    """Return a reconstructable multiprocess Dagster job for compute-log verification."""

    return f"""from __future__ import annotations

import json
import sys
from pathlib import Path

import dagster as dg
from sqlbuild.integrations.dagster import SqlBuildCliResource

ROOT = Path({str(root)!r})
DAG_PATH = ROOT / "sqlbuild_dag.json"
DAG_PATH.write_text(json.dumps({{
    "version": 1,
    "project_name": "stdout_e2e",
    "nodes": [{{"id": "model:orders", "kind": "model", "name": "orders", "asset_key": ["orders"]}}],
    "edges": [],
    "checks": [],
}}))
COMMAND = ROOT / "emit_output.py"
COMMAND.write_text(
    "import json, os, sys\\n"
    "from pathlib import Path\\n"
    "event = {{'schema_version': 1, 'record_kind': 'integration_result', "
    "'event_id': 'event-orders', 'event_sequence': 0, "
    "'event_type': 'resource_attempt_completed', "
    "'occurred_at': '2026-09-02T12:00:00+00:00', 'invocation_id': 'invocation-1', "
    "'run_id': 'run-1', 'resource_id': 'model:orders', "
    "'resource_attempt_id': 'attempt-orders', 'operation_id': None, 'statement_id': None, "
    "'resource_kind': 'model', 'resource_name': 'orders', 'attempt_number': 1, "
    "'duration_ms': 1, 'output_kind': 'asset', 'command': 'build', "
    "'error_code': None, 'error_type': None, 'skip_code': None, 'skip_mode': None, "
    "'asset': {{'kind': 'model', 'name': 'orders', 'status': 'success'}}, 'checks': []}}\\n"
    "event_path = Path(os.environ['SQLBUILD_INTEGRATION_RESULT_PATH'])\\n"
    "event_path.write_text(json.dumps(event) + '\\\\n')\\n"
    "sys.stdout.write('PROGRESS LINE\\\\n    SELECT * FROM important_table;\\\\n')\\n"
    "sys.stdout.flush()\\n"
    "payload = {{'version': 1, 'command': 'build', 'status': 'success', "
    "'summary': {{'success_count': 1}}, 'assets': [event['asset']], 'checks': []}}\\n"
    "Path(sys.argv[sys.argv.index('--json-output') + 1]).write_text(json.dumps(payload))\\n"
)

@dg.multi_asset(specs=[dg.AssetSpec("orders")])
def sqlbuild_assets(context):
    resource = SqlBuildCliResource(
        project_dir=str(ROOT),
        dag_path=str(DAG_PATH),
        sqb_command=[sys.executable, str(COMMAND)],
    )
    yield from resource.cli(["build", "--verbose"], context=context).stream()

defs = dg.Definitions(assets=[sqlbuild_assets], jobs=[dg.define_asset_job("stdout_capture_job")])
"""


@dataclass(frozen=True)
class AuditCheckOutcome:
    """The asset checks one Dagster run declared and evaluated."""

    success: bool
    declared: frozenset[tuple[AssetKey, str]]
    declared_assets: frozenset[AssetKey]
    evaluated: frozenset[tuple[AssetKey, str]]
    all_passed: bool


ATTACHMENT_CHECK_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "attachment_checks"\n'
        'adapter = "duckdb"\n\n'
        "[connection]\n"
        'database = "attachment_checks.duckdb"\n'
    ),
    "sql/seeds/products.csv": "product_id,product_name\n1,widget\n2,gadget\n",
    "sql/seeds/products.yml": (
        "seeds:\n"
        "  - name: products\n"
        "    description: Product catalogue.\n"
        "    columns:\n"
        "      - name: product_id\n"
        "        type: INTEGER\n"
        "        audits:\n"
        "          - not_null\n"
        "      - name: product_name\n"
        "        type: VARCHAR\n"
    ),
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n"
        "    description: Raw orders.\n"
        "    expression: |\n"
        "      SELECT 1 AS order_id, 1 AS product_id\n"
        "      UNION ALL SELECT 2 AS order_id, 2 AS product_id\n"
        "    columns:\n"
        "      - name: order_id\n"
        "        type: INTEGER\n"
        "        audits:\n"
        "          - not_null\n"
        "      - name: product_id\n"
        "        type: INTEGER\n"
    ),
    "sql/models/orders.sql": (
        "MODEL (description 'Orders.',\n"
        "  columns (order_id (), product_id ()),\n"
        "  audits [not_null (column order_id)],\n"
        ");\n\n"
        'SELECT order_id, product_id FROM __source("raw_orders")\n'
    ),
    "sql/models/customers.sql": (
        "MODEL (description 'Customers.');\n\nSELECT 1 AS customer_id, 1 AS order_id\n"
    ),
    "sql/audits/singular/orders_have_customers.sql": (
        'AUDIT (name "orders_have_customers", severity error);\n\n'
        "SELECT o.order_id\n"
        'FROM __ref("orders") o\n'
        'LEFT JOIN __ref("customers") c ON o.order_id = c.order_id\n'
        "WHERE c.order_id IS NULL AND o.order_id = 1\n"
    ),
    "sql/audits/singular/orders_match_raw_orders.sql": (
        'AUDIT (name "orders_match_raw_orders", severity error);\n\n'
        "SELECT o.order_id\n"
        'FROM __ref("orders") o\n'
        'LEFT JOIN __source("raw_orders") r ON o.order_id = r.order_id\n'
        "WHERE r.order_id IS NULL\n"
    ),
}


def audit_check_outcome(*, result: ExecuteInProcessResult, assets: Any) -> AuditCheckOutcome:
    """The asset checks a Dagster run of `assets` declared and evaluated."""

    declared: frozenset[tuple[AssetKey, str]] = frozenset(
        (spec.asset_key, spec.name) for spec in assets.check_specs
    )
    evaluations: list[Any] = list(result.get_asset_check_evaluations())
    return AuditCheckOutcome(
        success=result.success,
        declared=declared,
        declared_assets=frozenset(asset_key for asset_key, _ in declared),
        evaluated=frozenset(
            (evaluation.asset_key, evaluation.check_name) for evaluation in evaluations
        ),
        all_passed=all(evaluation.passed for evaluation in evaluations),
    )
