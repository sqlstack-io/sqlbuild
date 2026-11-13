import json
import subprocess
from pathlib import Path

import duckdb
import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import (
    AuditEvidenceTestCase,
    ClosingTerminationTestCase,
    CostAttributionTestCase,
    FailedStatementSqlTestCase,
    RecordedSelectionTestCase,
    TerminatedCommandTestCase,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    EMITTED_TAIL,
    ORDERS_MODEL,
    recall_rows,
    run,
    terminated_build,
    terminated_while_closing,
    write_project,
    write_slow_import_sink,
)

CUSTOMERS_MODEL: str = (
    'MODEL (\n  description "One row per customer",\n  materialized table,\n'
    "  columns (email (audits [not_null(severity warn)])),\n);\n\n"
    "WITH customers(customer_id, email) AS (VALUES (1, 'ada@example.com'), (2, NULL), (3, NULL))\n"
    "SELECT customers.customer_id, customers.email FROM customers\n"
)

COSTED_ADAPTER: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n"
    "from sqlbuild.cost.models import RunCostSummary\n"
    "from sqlbuild.cost.types import CostCapability, CostStatus\n\n\n"
    "class DuckDbCostedAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_costed'\n\n"
    "    def cost_capability(self):\n"
    "        return CostCapability.SNOWFLAKE_QUERY_HISTORY\n\n"
    "    def collect_run_cost(self, *, usd_per_credit, rate_source, **kwargs):\n"
    "{collected}"
)


@pytest.mark.parametrize(
    "test_case",
    [
        RecordedSelectionTestCase(
            description="a selector file's selectors are recorded with the file",
            selector_file="# nightly orders\norders\n\nemit_bytes\n",
            args=("--select-file", "nightly.txt"),
            expected_selection=['["orders","emit_bytes"]', 2, 0, '["nightly.txt"]'],
        ),
        RecordedSelectionTestCase(
            description="positional, option and file selectors are recorded together",
            selector_file="emit_bytes\n",
            args=("orders", "--select-file", "nightly.txt"),
            expected_selection=['["orders","emit_bytes"]', 2, 0, '["nightly.txt"]'],
        ),
        RecordedSelectionTestCase(
            description="a command without selectors records none",
            selector_file="",
            args=(),
            expected_selection=["[]", 0, 0, "[]"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selectors_when_building_then_the_effective_selection_is_recorded(
    test_case: RecordedSelectionTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    _ = (project_dir / "nightly.txt").write_text(test_case.selector_file, encoding="utf-8")

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build", *test_case.args)
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT selection, selection_count, selection_truncated, selector_files FROM invocations",
    )

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert recorded == [test_case.expected_selection]


@pytest.mark.parametrize(
    "test_case",
    [
        TerminatedCommandTestCase(
            description="during project discovery",
            orders_model=ORDERS_MODEL,
            environment={"ORDERS_IMPORT_SECONDS": "60"},
            wait_sql=(
                "SELECT invocation_id FROM invocations WHERE status = 'running' "
                "AND (julianday(last_heartbeat_at) - julianday(started_at)) * 86400 >= 2"
            ),
            expected_output=b"",
        ),
        TerminatedCommandTestCase(
            description="while a task runs",
            orders_model=ORDERS_MODEL,
            environment={"ORDERS_TASK_SLEEP_SECONDS": "60"},
            wait_sql=(
                "SELECT invocation_id FROM output_tail WHERE text LIKE '%after the warning%'"
            ),
            expected_output=EMITTED_TAIL,
        ),
        TerminatedCommandTestCase(
            description="while a long warehouse statement runs",
            orders_model=(
                'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
                "SELECT sum(range % 7) AS order_id FROM range(80000000000)\n"
            ),
            environment={},
            wait_sql=(
                "SELECT statement_id FROM statements "
                "WHERE statement_kind = 'CREATE' AND ended_at IS NULL"
            ),
            expected_output=b"",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sigterm_when_a_build_is_stopped_then_it_is_recorded_cancelled_with_its_real_end(
    test_case: TerminatedCommandTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_slow_import_sink(write_project(tmp_path / "orders_shop"))
    _ = (project_dir / "models" / "orders.sql").write_text(test_case.orders_model, encoding="utf-8")

    terminated, stop_seconds = terminated_build(
        project_dir,
        wait_sql=test_case.wait_sql,
        SQLBUILD_RECALL_HEARTBEAT_SECONDS="0.5",
        **test_case.environment,
    )
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT i.status, i.signal_number, i.exit_code, i.ended_at_estimated, "
        "i.ended_at > i.started_at, o.flush_complete "
        "FROM invocations i JOIN output_invocations o USING (invocation_id)",
    )
    starts: list[list[object]] = recall_rows(
        project_dir, "SELECT COUNT(*) FROM recall_events WHERE event_type = 'invocation_started'"
    )
    output: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "output", "last")

    assert terminated.returncode == 143
    assert stop_seconds < 10
    assert terminated.stderr.endswith(b"error: terminated by SIGTERM\n")
    assert b"Traceback" not in terminated.stderr
    assert recorded == [["cancelled", 15, 143, 0, 1, 1]]
    assert starts == [[1]]
    assert test_case.expected_output in output.stdout
    assert b"may be incomplete" not in output.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ClosingTerminationTestCase(
            description="a build waiting for a locked store at exit",
            shutdown_timeout="60s",
            expected_exit_code=143,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_sigterm_while_recall_closes_when_a_build_has_finished_then_it_exits_143(
    test_case: ClosingTerminationTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(
        tmp_path / "orders_shop",
        extra_toml=f'\n[recall]\nshutdown_timeout = "{test_case.shutdown_timeout}"\n',
    )

    terminated, stop_seconds = terminated_while_closing(project_dir)

    assert b"Completed" in terminated.stdout
    assert terminated.returncode == test_case.expected_exit_code, terminated.stderr.decode(
        errors="replace"
    )
    assert stop_seconds < 10
    assert b"Traceback" not in terminated.stderr
    assert terminated.stderr.endswith(b"error: terminated by SIGTERM\n")


@pytest.mark.parametrize(
    "test_case",
    [
        FailedStatementSqlTestCase(
            description="a statement the warehouse rejects keeps its SQL",
            model_sql="SELECT CAST('not a number' AS INTEGER) AS order_id",
            expected_sql_fragment="CAST('not a number' AS INTEGER)",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failed_statement_when_recalled_then_its_sql_is_readable(
    test_case: FailedStatementSqlTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    _ = (project_dir / "models" / "orders.sql").write_text(
        f'MODEL (\n  description "One row per order",\n  materialized table,\n);\n\n'
        f"{test_case.model_sql}\n",
        encoding="utf-8",
    )

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build", "orders")
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT s.sql_available, s.sql_unavailable_reason, a.text FROM statements s "
        "LEFT JOIN artifacts a ON a.invocation_id = s.invocation_id "
        "AND a.digest = 'sha256:' || s.sql_digest WHERE s.outcome = 'failed'",
    )

    assert built.returncode == 1
    assert [row[:2] for row in recorded] == [[1, None]]
    assert test_case.expected_sql_fragment in str(recorded[0][2])


@pytest.mark.parametrize(
    "test_case",
    [
        AuditEvidenceTestCase(
            description="a warning audit's violating rows are kept by default",
            recall_toml="",
            severity="warn",
            expected_exit_code=0,
            expected_evidence=["warn", 2, 2, 0, 1],
            expected_artifacts=[["audit_evidence", "application/json"]],
            expected_warehouse_evidence=[('[{"email":null},{"email":null}]',)],
        ),
        AuditEvidenceTestCase(
            description="error-only evidence does not collect a warning's rows",
            recall_toml='\n[recall]\naudit_evidence = "error"\n',
            severity="warn",
            expected_exit_code=0,
            expected_evidence=["warn", 2, 0, 0, 0],
            expected_artifacts=[],
            expected_warehouse_evidence=[(None,)],
        ),
        AuditEvidenceTestCase(
            description="error-only evidence collects a failing audit's rows",
            recall_toml='\n[recall]\naudit_evidence = "error"\n',
            severity="error",
            expected_exit_code=1,
            expected_evidence=["error", 2, 2, 0, 1],
            expected_artifacts=[["audit_evidence", "application/json"]],
            expected_warehouse_evidence=[('[{"email":null},{"email":null}]',)],
        ),
        AuditEvidenceTestCase(
            description="evidence turned off collects no rows",
            recall_toml='\n[recall]\naudit_evidence = "off"\n',
            severity="error",
            expected_exit_code=1,
            expected_evidence=["error", 2, 0, 0, 0],
            expected_artifacts=[],
            expected_warehouse_evidence=[(None,)],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failing_audit_when_recalled_then_its_evidence_follows_the_setting(
    test_case: AuditEvidenceTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=test_case.recall_toml)
    _ = (project_dir / "models" / "customers.sql").write_text(
        CUSTOMERS_MODEL.replace("severity warn", f"severity {test_case.severity}"),
        encoding="utf-8",
    )

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build", "customers")
    with duckdb.connect(str(project_dir / "warehouse.duckdb"), read_only=True) as warehouse:
        warehouse_evidence: list[tuple[object, ...]] = warehouse.execute(
            "SELECT evidence_json FROM dev._sqlbuild_audit_results"
        ).fetchall()
    evidence: list[list[object]] = recall_rows(
        project_dir,
        "SELECT outcome, violation_count, evidence_count, evidence_truncated, "
        "sample_digest IS NOT NULL FROM audit_results",
    )
    artifacts: list[list[object]] = recall_rows(
        project_dir,
        "SELECT a.kind, a.media_type, a.text FROM artifacts a JOIN audit_results r "
        "ON a.invocation_id = r.invocation_id AND a.ref_id = r.event_id",
    )

    assert built.returncode == test_case.expected_exit_code, built.stderr.decode(errors="replace")
    assert evidence == [test_case.expected_evidence]
    assert warehouse_evidence == test_case.expected_warehouse_evidence
    assert [row[:2] for row in artifacts] == test_case.expected_artifacts
    assert [json.loads(str(row[2])) for row in artifacts] == [
        [{"email": None}, {"email": None}] for _ in test_case.expected_artifacts
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        CostAttributionTestCase(
            description="a collected cost records its status",
            collected=(
                "        return RunCostSummary(status=CostStatus.PARTIAL, "
                "usd_per_credit=usd_per_credit, rate_source=rate_source)\n"
            ),
            expected_cost_status="partial",
        ),
        CostAttributionTestCase(
            description="a failed collection is recorded as failed",
            collected="        raise ConnectionError('query history unreachable')\n",
            expected_cost_status="collection_failed",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cost_attribution_when_a_build_ends_then_the_invocation_records_how_far_it_got(
    test_case: CostAttributionTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    toml: Path = project_dir / "sqlbuild_project.toml"
    _ = toml.write_text(
        toml.read_text(encoding="utf-8").replace('"duckdb"', '"duckdb_costed"', 1),
        encoding="utf-8",
    )
    adapter: Path = project_dir / "adapters" / "warehouse" / "duckdb_costed.py"
    adapter.parent.mkdir(parents=True)
    _ = adapter.write_text(COSTED_ADAPTER.format(collected=test_case.collected), encoding="utf-8")

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build", "orders")
    recorded: list[list[object]] = recall_rows(
        project_dir, "SELECT status, cost_status FROM invocations"
    )

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert recorded == [["completed", test_case.expected_cost_status]]
