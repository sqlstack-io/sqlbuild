import json
import subprocess
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.cli.recall._test_types import (
    DocumentedQueryTestCase,
    FailureCauseTestCase,
    GitRevisionTestCase,
    IntegrationColumnsTestCase,
    InvocationOriginTestCase,
    InvocationPrefixTestCase,
)
from tests.integration.src.sqlbuild.cli.recall.helpers import (
    DEPLOYED_REVISION,
    NO_CI,
    checkout_project,
    copy_invocation_row,
    deployed_project,
    documented_commands,
    recall_rows,
    run,
    selected_columns,
    worktree_project,
    write_project,
)

BROKEN_TARGET: str = (
    "\n[connections.broken]\n"
    'database = "missing_directory/warehouse.duckdb"\n\n'
    "[targets.broken]\n"
    'connection = "broken"\n'
    'schema = "dev"\n'
)

ORCHESTRATED_CONTEXT: str = (
    '{"integration": {"name": "dagster", "job_name": "orders_job", "run_id": "run-7",'
    ' "retry_number": 1}}'
)
LEGACY_CONTEXT: str = (
    "{'integration': mappingproxy({'job_name': 'orders_job', 'name': 'dagster',"
    " 'retry_number': 0, 'run_id': 'run-6'})}"
)
WARNING_AUDITS: str = '\n[settings]\ndefault_audit_severity = "warn"\n'
CUSTOMERS_MODEL: str = (
    'MODEL (\n  description "One row per customer",\n  materialized table,\n'
    "  columns (email (audits [not_null])),\n);\n\n"
    "SELECT 1 AS customer_id, CAST(NULL AS VARCHAR) AS email\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        DocumentedQueryTestCase(
            description="an orchestrated job's runs are found by their integration job",
            question=0,
            columns=("status", "integration_run_id", "integration_retry"),
            expected_rows=[["completed", "run-7", 1]],
        ),
        DocumentedQueryTestCase(
            description="scheduled builds are listed by their origin",
            question=1,
            columns=("command", "origin", "status"),
            expected_rows=[["build", "integration", "completed"]],
        ),
        DocumentedQueryTestCase(
            description="the slowest models of a run are its model builds by resource",
            question=2,
            columns=("resource_name", "resource_kind"),
            expected_rows=[["customers", "table"], ["orders", "table"]],
        ),
        DocumentedQueryTestCase(
            description="audits that did not pass are listed",
            question=3,
            columns=("audit_name", "outcome", "violation_count"),
            expected_rows=[["not_null", "warn", 1]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_orchestrated_build_when_running_the_documented_queries_then_they_answer(
    test_case: DocumentedQueryTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=WARNING_AUDITS)
    _ = (project_dir / "sql" / "models" / "customers.sql").write_text(
        CUSTOMERS_MODEL, encoding="utf-8"
    )
    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", SQLBUILD_INVOCATION_CONTEXT_JSON=ORCHESTRATED_CONTEXT
    )
    invocation_id: str = str(
        recall_rows(project_dir, "SELECT invocation_id FROM invocations")[0][0]
    )
    commands: list[list[str]] = documented_commands(invocation_id=invocation_id)

    answered: subprocess.CompletedProcess[bytes] = run(
        project_dir, *commands[test_case.question], "--json"
    )
    printed: subprocess.CompletedProcess[bytes] = run(project_dir, *commands[-1])

    assert built.returncode == 0, built.stderr
    assert len(commands) == 5
    assert answered.returncode == 0, answered.stderr
    # Row order follows measured durations, so compare the rows without their order.
    assert sorted(
        selected_columns(result=json.loads(answered.stdout), columns=test_case.columns),
        key=repr,
    ) == sorted(test_case.expected_rows, key=repr)
    assert printed.returncode == 0, printed.stderr
    assert len(printed.stdout.splitlines()) == 50
    assert printed.stdout.endswith(built.stdout.splitlines(keepends=True)[-1])


@pytest.mark.parametrize(
    "test_case",
    [
        FailureCauseTestCase(
            description="a debug connection failure reported by exit code",
            args=("debug", "--target", "broken"),
            expected_failure=["debug", "failed", 1, "ConnectionError", "C980"],
            expected_message_prefix="connection test: ",
        ),
        FailureCauseTestCase(
            description="a raised coded error",
            args=("build", "--select", "no_such_model"),
            expected_failure=["build", "failed", None, "PlannerInputError", "S007"],
            expected_message_prefix="unknown selector name 'no_such_model'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_failed_command_when_recorded_then_its_cause_is_structured(
    test_case: FailureCauseTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop", extra_toml=BROKEN_TARGET)

    failed: subprocess.CompletedProcess[bytes] = run(project_dir, *test_case.args)
    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT command, status, exit_code, error_type, error_code, error_message FROM invocations",
    )

    assert failed.returncode != 0
    assert [row[:5] for row in recorded] == [test_case.expected_failure]
    assert str(recorded[0][5]).startswith(test_case.expected_message_prefix)


@pytest.mark.parametrize(
    "test_case",
    [
        GitRevisionTestCase(
            description="the deployment's override wins",
            prepare=deployed_project,
            environment={"SQLBUILD_GIT_REVISION": DEPLOYED_REVISION, "SQLBUILD_GIT_DIRTY": "true"},
            expected_dirty=1,
        ),
        GitRevisionTestCase(
            description="a git worktree reads HEAD through its .git file",
            prepare=worktree_project,
            environment={},
            expected_dirty=None,
        ),
        GitRevisionTestCase(
            description="a plain checkout reads HEAD from .git",
            prepare=checkout_project,
            environment={},
            expected_dirty=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_deployment_or_checkout_when_recorded_then_the_git_revision_is_known(
    test_case: GitRevisionTestCase, tmp_path: Path
) -> None:
    project_dir, expected_revision = test_case.prepare(tmp_path)

    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build", **test_case.environment)

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert recall_rows(project_dir, "SELECT git_revision, git_dirty FROM invocations") == [
        [expected_revision, test_case.expected_dirty]
    ]


@pytest.mark.parametrize(
    "test_case",
    [
        InvocationOriginTestCase(
            description="a local configuration check records its mode",
            args=("debug", "--no-connection"),
            environment=NO_CI,
            expected_rows=[["debug", "local", "config_only"]],
        ),
        InvocationOriginTestCase(
            description="a CI connection check is recorded as CI",
            args=("debug",),
            environment={**NO_CI, "GITLAB_CI": "true"},
            expected_rows=[["debug", "ci", "config_and_connection"]],
        ),
        InvocationOriginTestCase(
            description="a CI variable set to false is not CI",
            args=("build",),
            environment={**NO_CI, "CI": "false"},
            expected_rows=[["build", "local", None]],
        ),
        InvocationOriginTestCase(
            description="an integration's run is recorded as integration, also in CI",
            args=("build",),
            environment={
                **NO_CI,
                "CI": "1",
                "SQLBUILD_INVOCATION_CONTEXT_JSON": ORCHESTRATED_CONTEXT,
            },
            expected_rows=[["build", "integration", None]],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_command_when_recorded_then_its_origin_and_mode_are_known(
    test_case: InvocationOriginTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")

    completed: subprocess.CompletedProcess[bytes] = run(
        project_dir, *test_case.args, **test_case.environment
    )

    assert completed.returncode == 0, completed.stderr.decode(errors="replace")
    assert (
        recall_rows(project_dir, "SELECT command, origin, command_mode FROM invocations")
        == test_case.expected_rows
    )


@pytest.mark.parametrize(
    "test_case",
    [
        IntegrationColumnsTestCase(
            description="a JSON context exposes the integration's job and run",
            stored_context=ORCHESTRATED_CONTEXT,
            expected_columns=["dagster", "orders_job", "run-7", 1],
        ),
        IntegrationColumnsTestCase(
            description="an older writer's Python repr context reads as NULL",
            stored_context=LEGACY_CONTEXT,
            expected_columns=[None, None, None, None],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_integration_context_when_queried_then_its_fields_are_columns(
    test_case: IntegrationColumnsTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    built: subprocess.CompletedProcess[bytes] = run(
        project_dir, "build", SQLBUILD_INVOCATION_CONTEXT_JSON=ORCHESTRATED_CONTEXT
    )
    copy_invocation_row(
        project_dir / "logs" / "recall.sqlite3",
        invocation_id="copied",
        external_context=test_case.stored_context,
    )

    recorded: list[list[object]] = recall_rows(
        project_dir,
        "SELECT external_context, integration, integration_job, integration_run_id, "
        "integration_retry FROM invocations WHERE invocation_id = 'copied'",
    )
    written: list[list[object]] = recall_rows(
        project_dir, "SELECT external_context FROM invocations WHERE invocation_id <> 'copied'"
    )

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert [row[1:] for row in recorded] == [test_case.expected_columns]
    assert json.loads(str(written[0][0])) == json.loads(ORCHESTRATED_CONTEXT)


@pytest.mark.parametrize(
    "test_case",
    [
        InvocationPrefixTestCase(
            description="the short form names the invocation",
            prefix_length=8,
            expected_exit_code=0,
            expected_error_code=None,
        ),
        InvocationPrefixTestCase(
            description="a shorter unique prefix names the invocation",
            prefix_length=3,
            expected_exit_code=0,
            expected_error_code=None,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_unique_invocation_prefix_when_printing_output_then_it_names_the_invocation(
    test_case: InvocationPrefixTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    invocation_id: str = str(
        recall_rows(project_dir, "SELECT invocation_id FROM invocations")[0][0]
    )

    printed: subprocess.CompletedProcess[bytes] = run(
        project_dir, "recall", "output", invocation_id[: test_case.prefix_length]
    )
    exact: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "output", invocation_id)

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert printed.returncode == test_case.expected_exit_code, printed.stderr
    assert printed.stdout == exact.stdout
    assert printed.stdout.endswith(built.stdout.splitlines(keepends=True)[-1])


@pytest.mark.parametrize(
    "test_case",
    [
        InvocationPrefixTestCase(
            description="an ambiguous prefix fails and lists the matches",
            prefix_length=8,
            expected_exit_code=1,
            expected_error_code="C973",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_ambiguous_invocation_prefix_when_printing_output_then_it_lists_the_matches(
    test_case: InvocationPrefixTestCase, tmp_path: Path
) -> None:
    project_dir: Path = write_project(tmp_path / "orders_shop")
    built: subprocess.CompletedProcess[bytes] = run(project_dir, "build")
    invocation_id: str = str(
        recall_rows(project_dir, "SELECT invocation_id FROM invocations")[0][0]
    )
    prefix: str = invocation_id[: test_case.prefix_length]
    copy_id: str = prefix + "0" * (len(invocation_id) - len(prefix))
    copy_invocation_row(
        project_dir / "logs" / "recall.sqlite3", invocation_id=copy_id, external_context="{}"
    )

    printed: subprocess.CompletedProcess[bytes] = run(project_dir, "recall", "output", prefix)

    assert built.returncode == 0, built.stderr.decode(errors="replace")
    assert printed.returncode == test_case.expected_exit_code
    assert f"[{test_case.expected_error_code}]" in printed.stderr.decode()
    assert f"'{prefix}' matches several invocations; use a longer prefix: " in (
        printed.stderr.decode()
    )
    assert copy_id in printed.stderr.decode()
    assert invocation_id in printed.stderr.decode()
