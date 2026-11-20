"""A build that names jobs takes their settings: build-wide ones once, node-level ones per node."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    JobConcurrencyTestCase,
    JobFlagParityTestCase,
    JobFullRefreshTestCase,
    JobOutputTestCase,
    JobSettingsBuildTestCase,
    JobSettingsConflictTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    CONCURRENCY_VARIABLE,
    PARITY_NODES,
    job_flag_parity_outcome,
    peak_concurrency,
    query_duckdb_rows,
    write_project_files,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "demo"\nadapter = "duckdb"\ndefault_target = "dev"\n\n'
        '[connection]\ndatabase = "demo.duckdb"\n\n[targets.dev]\n\n[targets.prod]\n\n'
        '[settings]\nconcurrency = 4\n\n[vars]\namount = "1"\n'
    ),
    "models/stg_orders.sql": (
        "MODEL (description 'Staged orders.', materialized table);\n"
        "SELECT CAST(@@amount AS INTEGER) AS order_id\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Orders.', materialized table);\n"
        'SELECT order_id FROM __ref("stg_orders")\n'
    ),
    "models/customers.sql": (
        "MODEL (description 'Customers.', materialized table);\nSELECT 7 AS customer_id\n"
    ),
    "models/order_events.sql": (
        "MODEL (description 'Order events.', materialized incremental, "
        "incremental_strategy append);\nSELECT 1 AS event_id\n"
    ),
    "models/refund_events.sql": (
        "MODEL (description 'Refund events.', materialized incremental, "
        "incremental_strategy append);\nSELECT 1 AS event_id\n"
    ),
    "tests/unit/test_orders.sql": (
        "TEST();\n\nWITH\n__ref__stg_orders AS (SELECT 4 AS order_id),\n"
        "__expected__orders AS (SELECT 4 AS order_id)\nSELECT 1\n"
    ),
    "orchestration/jobs/orders_job.toml": (
        'name = "orders_job"\ndescription = "Orders."\nselect = "+orders"\n\n'
        '[settings]\nconcurrency = 1\nrun_tests = false\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/checked_orders_job.toml": (
        'name = "checked_orders_job"\ndescription = "Orders, tested."\nselect = "orders"\n\n'
        '[settings]\nrun_tests = true\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/crm_job.toml": (
        'name = "crm_job"\ndescription = "Customers."\nselect = "customers"\n\n'
        '[settings]\nvars = { amount = "9" }\n'
    ),
    "orchestration/jobs/events_job.toml": (
        'name = "events_job"\ndescription = "Events."\nselect = "order_events"\n\n'
        "[settings]\nfull_refresh = true\n"
    ),
    "orchestration/jobs/refund_events_job.toml": (
        'name = "refund_events_job"\ndescription = "Refund events."\nselect = "refund_events"\n'
    ),
    "orchestration/job_groups/events_job_group.toml": (
        'name = "events_job_group"\ndescription = "Events."\n'
        'jobs = ["events_job", "refund_events_job"]\n'
    ),
    "models/lenient_model.sql": (
        "MODEL (description 'Lenient.', materialized table, "
        "columns (id (type INTEGER, audits [not_null])));\nSELECT CAST(NULL AS INTEGER) AS id\n"
    ),
    "orchestration/jobs/lenient_job.toml": (
        'name = "lenient_job"\ndescription = "Lenient."\nselect = "lenient_model"\n\n'
        "[settings]\nignore_audit_failures = true\n"
    ),
    "orchestration/jobs/loads_off_job.toml": (
        'name = "loads_off_job"\ndescription = "No loads."\nselect = "customers"\n\n'
        "[settings]\nload_sources = false\n"
    ),
    **{
        f"models/slow_{name}.sql": (
            f"MODEL (description 'Slow {name}.', materialized table);\n"
            "SELECT sum(i * i % 7) AS total FROM range(25000000) AS t(i)\n"
        )
        for name in ("a", "b", "c", "d")
    },
    "orchestration/jobs/warehouse_job.toml": (
        'name = "warehouse_job"\ndescription = "On a warehouse."\nselect = "customers"\n\n'
        '[settings]\nwarehouse = "ORDERS_WH"\n'
    ),
    "orchestration/jobs/wide_job.toml": (
        'name = "wide_job"\ndescription = "Wide."\nselect = "customers"\n\n'
        '[settings]\nconcurrency = 6\nvars = { amount = "5" }\n'
    ),
    "orchestration/jobs/slow_job.toml": (
        'name = "slow_job"\ndescription = "Slow."\nselect = "slow_a slow_b"\n\n'
        "[settings]\nconcurrency = 1\n"
    ),
}

_FAIL_FAST_FILES: dict[str, str] = {
    "orchestration/jobs/fast_fail_job.toml": (
        'name = "fast_fail_job"\ndescription = "Fails fast."\nselect = "customers"\n\n'
        "[settings]\nfail_fast = true\n"
    ),
    "orchestration/job_groups/fail_job_group.toml": (
        'name = "fail_job_group"\ndescription = "Fails."\njobs = ["events_job", "fast_fail_job"]\n'
    ),
}
_GRACE_FILES: dict[str, str] = {
    "orchestration/jobs/grace_job.toml": (
        'name = "grace_job"\ndescription = "Takes over."\nselect = "customers"\n\n'
        "[settings]\nignore_dynamic_grace = true\n"
    ),
}
_LENIENT_PAIR_FILES: dict[str, str] = {
    "models/lenient_peer.sql": (
        "MODEL (description 'Lenient peer.', materialized table);\nSELECT 1 AS id\n"
    ),
    "audits/singular/lenient_pair.sql": (
        'AUDIT (name "lenient_pair", severity error);\n'
        'SELECT p.id FROM __ref("lenient_peer") p '
        'LEFT JOIN __ref("lenient_model") m ON p.id = m.id WHERE m.id IS NULL\n'
    ),
    "orchestration/jobs/lenient_peer_job.toml": (
        'name = "lenient_peer_job"\ndescription = "Lenient peer."\nselect = "lenient_peer"\n\n'
        "[settings]\nignore_audit_failures = true\n"
    ),
}
_STEADY_EVENTS_FILES: dict[str, str] = {
    "orchestration/jobs/steady_events_job.toml": (
        'name = "steady_events_job"\ndescription = "Events, kept."\nselect = "order_events"\n\n'
        "[settings]\nfull_refresh = false\n"
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        JobSettingsBuildTestCase(
            description="a named job's vars, concurrency and run_tests apply",
            build_args=("--select", "job:orders_job"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="a job: pattern names the jobs it matches",
            build_args=("--select", "job:orders_*"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="job flags go before -- so a selector after it still names the job",
            build_args=("--", "orders_job"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="a bare job name names the job",
            build_args=("--select", "orders_job"),
            expected_amount=5,
            expected_fragments=("(concurrency: 1)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="command-line flags beat the job's settings",
            build_args=(
                "--select",
                "job:orders_job",
                "--vars",
                '{"amount": "2"}',
                "--concurrency",
                "3",
            ),
            expected_amount=2,
            expected_fragments=("(concurrency: 3)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="an abbreviated command-line flag still beats the job's setting",
            build_args=("--select", "job:orders_job", "--concur", "2"),
            expected_amount=5,
            expected_fragments=("(concurrency: 2)", "TOTAL=2"),
        ),
        JobSettingsBuildTestCase(
            description="--no-job-settings builds with the project's settings",
            build_args=("--select", "job:orders_job", "--no-job-settings"),
            expected_amount=1,
            expected_fragments=("(concurrency: 4)", "TOTAL=3"),
        ),
        JobSettingsBuildTestCase(
            description="selecting the job's nodes without naming it takes none of its settings",
            build_args=("--select", "+orders"),
            expected_amount=1,
            expected_fragments=("(concurrency: 4)", "TOTAL=3"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_named_job_when_building_then_its_settings_apply_below_the_command_line(
    test_case: JobSettingsBuildTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv(CONCURRENCY_VARIABLE, raising=False)
    write_project_files(project_dir=tmp_path, files=_PROJECT_FILES)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", *test_case.build_args]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 0, captured
    assert query_duckdb_rows(
        db_path=tmp_path / "demo.duckdb", sql="SELECT order_id FROM orders"
    ) == ((test_case.expected_amount,),)
    assert {fragment: fragment in captured for fragment in test_case.expected_fragments} == (
        dict.fromkeys(test_case.expected_fragments, True)
    ), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobSettingsConflictTestCase(
            description="two named jobs with different vars cannot build together",
            command_args=("build", "--select", "job:orders_job", "job:crm_job"),
            expected_fragment=(
                'error[S027]: jobs orders_job and crm_job set vars differently ({"amount":"5"} '
                'and {"amount":"9"}); a build applies vars to everything it runs'
            ),
        ),
        JobSettingsConflictTestCase(
            description="a plan naming jobs with different vars fails the same way",
            command_args=("plan", "--select", "job:orders_job", "job:crm_job"),
            expected_fragment=(
                'error[S027]: jobs orders_job and crm_job set vars differently ({"amount":"5"} '
                'and {"amount":"9"}); a build applies vars to everything it runs'
            ),
        ),
        JobSettingsConflictTestCase(
            description="two named jobs setting a build-wide setting differently cannot build",
            command_args=("build", "--select", "job:events_job", "job:fast_fail_job"),
            expected_fragment=(
                "jobs events_job and fast_fail_job set fail_fast differently (not set and true); "
                "a build applies fail_fast to everything it runs  = help: make the jobs agree, "
                "pass --fail-fast to make it true for all, or build with --no-job-settings"
            ),
            extra_files=_FAIL_FAST_FILES,
        ),
        JobSettingsConflictTestCase(
            description="a job ignoring the dynamic grace needs the project to allow it",
            command_args=("compile",),
            expected_fragment=(
                "orchestration/jobs/grace_job.toml:6:24: ignore_dynamic_grace needs [dynamic] "
                "allow_ignore_grace = true in sqlbuild_project.toml"
            ),
            extra_files=_GRACE_FILES,
        ),
        JobSettingsConflictTestCase(
            description="two named jobs giving one node different full_refresh conflict",
            command_args=("build", "--select", "job:events_job", "job:steady_events_job"),
            expected_fragment=(
                "error[S027]: model:order_events is selected through jobs events_job and "
                "steady_events_job, which set full_refresh differently (true and false)"
            ),
            extra_files=_STEADY_EVENTS_FILES,
        ),
        JobSettingsConflictTestCase(
            description="two named jobs giving one node different run_tests conflict",
            command_args=("build", "--select", "job:orders_job", "job:checked_orders_job"),
            expected_fragment=(
                "error[S027]: model:orders is selected through jobs orders_job and "
                "checked_orders_job, which set run_tests differently (false and true)"
            ),
        ),
        JobSettingsConflictTestCase(
            description="a group whose jobs set different vars does not compile",
            command_args=("compile",),
            expected_fragment=(
                'jobs orders_job and crm_job set vars differently ({"amount":"5"} and '
                '{"amount":"9"}); a build applies vars to everything it runs; group '
                "daily_job_group builds its jobs together"
            ),
            extra_files={
                "orchestration/job_groups/daily_job_group.toml": (
                    'name = "daily_job_group"\ndescription = "Daily."\n'
                    'jobs = ["orders_job", "crm_job"]\n'
                ),
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_conflicting_job_settings_when_running_then_the_command_fails_naming_them(
    test_case: JobSettingsConflictTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files={**_PROJECT_FILES, **test_case.extra_files})

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command_args])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 1, captured
    assert test_case.expected_fragment in captured.replace("\n", " ").replace("  ", " "), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobFullRefreshTestCase(
            description="a named job's full_refresh rebuilds only its own incremental models",
            build_args=("--select", "job:events_job", "refund_events"),
            expected_counts=((1, 3),),
        ),
        JobFullRefreshTestCase(
            description="--full-refresh rebuilds every selected incremental model",
            build_args=("--select", "job:events_job", "refund_events", "--full-refresh"),
            expected_counts=((1, 1),),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_named_job_with_full_refresh_when_building_then_only_its_models_start_over(
    test_case: JobFullRefreshTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_project_files(project_dir=tmp_path, files=_PROJECT_FILES)
    arguments: list[str] = ["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests"]
    first: int = main([*arguments, "--select", "order_events", "refund_events"])
    second: int = main([*arguments, "--select", "order_events", "refund_events"])

    exit_code: int = main([*arguments, *test_case.build_args])
    captured: str = "".join(capsys.readouterr())

    assert (first, second, exit_code) == (0, 0, 0), captured
    assert (
        query_duckdb_rows(
            db_path=tmp_path / "demo.duckdb",
            sql="SELECT (SELECT count(*) FROM order_events), (SELECT count(*) FROM refund_events)",
        )
        == test_case.expected_counts
    )


@pytest.mark.parametrize(
    "test_case",
    [
        JobConcurrencyTestCase(
            description="a job's concurrency limits its own nodes while other nodes run beside them",
            build_args=("--select", "job:slow_job", "slow_c", "slow_d"),
            job_nodes=("slow_a", "slow_b"),
            build_nodes=("slow_a", "slow_b", "slow_c", "slow_d"),
            expected_job_peak=1,
            expected_least_build_peak=2,
        ),
        JobConcurrencyTestCase(
            description="without job settings the job's nodes run together",
            build_args=("--select", "job:slow_job", "slow_c", "slow_d", "--no-job-settings"),
            job_nodes=("slow_a", "slow_b"),
            build_nodes=("slow_a", "slow_b", "slow_c", "slow_d"),
            expected_job_peak=2,
            expected_least_build_peak=2,
        ),
        JobConcurrencyTestCase(
            description="SQLBUILD_CONCURRENCY sets the build's limit, not the job's",
            build_args=("--select", "job:slow_job", "slow_c", "slow_d"),
            job_nodes=("slow_a", "slow_b"),
            build_nodes=("slow_a", "slow_b", "slow_c", "slow_d"),
            expected_job_peak=1,
            expected_least_build_peak=2,
            environment_concurrency="8",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_job_concurrency_when_building_then_its_nodes_never_exceed_it(
    test_case: JobConcurrencyTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv(CONCURRENCY_VARIABLE, test_case.environment_concurrency)
    write_project_files(project_dir=tmp_path, files=_PROJECT_FILES)
    events: Path = tmp_path / "events.jsonl"

    exit_code: int = main(
        [
            "--project-dir",
            str(tmp_path),
            "--no-color",
            "build",
            *test_case.build_args,
            "--event-output",
            str(events),
        ]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 0, captured
    assert (
        peak_concurrency(events_path=events, names=test_case.job_nodes),
        peak_concurrency(events_path=events, names=test_case.build_nodes)
        >= test_case.expected_least_build_peak,
    ) == (test_case.expected_job_peak, True), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobOutputTestCase(
            description="a named job's concurrency above the configured limit raises the build's",
            command_args=("build", "--select", "job:wide_job"),
            expected_exit_code=0,
            expected_present=("(concurrency: 6)",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="the named jobs' limits add up",
            command_args=("build", "--select", "job:wide_job", "job:orders_job"),
            expected_exit_code=0,
            expected_present=("(concurrency: 7)",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="the run context says the named jobs set the limit",
            command_args=("build", "--verbose", "--select", "job:wide_job", "+orders"),
            expected_exit_code=0,
            expected_present=("concurrency  6 configured limit\n",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="a configured limit above the named jobs' stays the build's limit",
            command_args=("build", "--verbose", "--select", "job:orders_job", "customers"),
            expected_exit_code=0,
            expected_present=("concurrency  4 configured limit\n",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="an explicit --concurrency wins over the named jobs",
            command_args=("build", "--select", "job:wide_job", "--concurrency", "2"),
            expected_exit_code=0,
            expected_present=("(concurrency: 2)",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="a job warehouse on an adapter without one fails, naming the job",
            command_args=("build", "--select", "job:warehouse_job"),
            expected_exit_code=1,
            expected_present=(
                "error[C260]: job warehouse_job sets warehouse ORDERS_WH, but the 'duckdb' "
                "adapter has no warehouse to select",
            ),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="--no-job-settings leaves a job warehouse out on such an adapter",
            command_args=("build", "--no-job-settings", "--select", "job:warehouse_job"),
            expected_exit_code=0,
            expected_present=(),
            expected_absent=("C260",),
        ),
        JobOutputTestCase(
            description="a plan shows the job's full refresh",
            command_args=("plan", "--select", "job:events_job"),
            expected_exit_code=0,
            expected_present=("Plan ready  full refresh, 1 selected",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="plan --no-job-settings leaves out the job's full refresh",
            command_args=("plan", "--select", "job:events_job", "--no-job-settings"),
            expected_exit_code=0,
            expected_present=("Plan ready  1 selected",),
            expected_absent=("full refresh",),
        ),
        JobOutputTestCase(
            description="plan JSON names the jobs that full refresh their models",
            command_args=("plan", "--json", "--select", "job:events_job"),
            expected_exit_code=0,
            expected_present=('"full_refresh_jobs": [\n    "events_job"\n  ]',),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="a plan says whose audit failures will not stop the build",
            command_args=("plan", "--select", "job:lenient_job"),
            expected_exit_code=0,
            expected_present=(
                "Audit failures won't stop this build for the audits of job lenient_job",
            ),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="a named job's ignore_audit_failures lets its audit failures through",
            command_args=("build", "--select", "job:lenient_job"),
            expected_exit_code=0,
            expected_present=("1 audit failure ignored",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="the same node built without naming the job fails its audit",
            command_args=("build", "--select", "lenient_model"),
            expected_exit_code=1,
            expected_present=("not_null (id)",),
            expected_absent=("ignored",),
        ),
        JobOutputTestCase(
            description="the run context names the jobs whose models are fully refreshed",
            command_args=("build", "--verbose", "--select", "job:events_job"),
            expected_exit_code=0,
            expected_present=("full_refresh nodes of job events_job",),
            expected_absent=(),
        ),
        JobOutputTestCase(
            description="naming a group applies each job's settings to its own models",
            command_args=("plan", "--json", "--select", "job:events_job_group"),
            expected_exit_code=0,
            expected_present=('"full_refresh_jobs": [\n    "events_job"\n  ]',),
            expected_absent=("refund_events_job",),
        ),
        JobOutputTestCase(
            description="a group compiles when its disagreeing jobs never share a target",
            command_args=("compile",),
            expected_exit_code=0,
            expected_present=("Project compiled",),
            expected_absent=("D017",),
            extra_files={
                "orchestration/jobs/prod_fail_job.toml": (
                    'name = "prod_fail_job"\ndescription = "Prod."\nselect = "customers"\n'
                    'targets = ["prod"]\n\n[settings]\nfail_fast = true\n'
                ),
                "orchestration/jobs/dev_job.toml": (
                    'name = "dev_job"\ndescription = "Dev."\nselect = "customers"\n'
                    'targets = ["dev"]\n'
                ),
                "orchestration/job_groups/split_job_group.toml": (
                    'name = "split_job_group"\ndescription = "Split."\n'
                    'jobs = ["prod_fail_job", "dev_job"]\n'
                ),
            },
        ),
        JobOutputTestCase(
            description="a group whose jobs differ only in fail_fast compiles, each job a build",
            command_args=("compile",),
            expected_exit_code=0,
            expected_present=("Project compiled",),
            expected_absent=("D017",),
            extra_files=_FAIL_FAST_FILES,
        ),
        JobOutputTestCase(
            description="--fail-fast settles the named jobs' fail_fast disagreement",
            command_args=(
                "build",
                "--fail-fast",
                "--select",
                "job:events_job",
                "job:fast_fail_job",
            ),
            expected_exit_code=0,
            expected_present=("Completed successfully",),
            expected_absent=("S027",),
            extra_files=_FAIL_FAST_FILES,
        ),
        JobOutputTestCase(
            description="--full-refresh settles two jobs' full_refresh disagreement on one node",
            command_args=(
                "build",
                "--full-refresh",
                "--select",
                "job:events_job",
                "job:steady_events_job",
            ),
            expected_exit_code=0,
            expected_present=("Completed successfully",),
            expected_absent=("S027",),
            extra_files=_STEADY_EVENTS_FILES,
        ),
        JobOutputTestCase(
            description="a singular audit over a lenient job node and a node outside it fails",
            command_args=("build", "--select", "job:lenient_job", "lenient_peer"),
            expected_exit_code=1,
            expected_present=("lenient_pair",),
            expected_absent=("lenient_pair returned 1 row (ignored)",),
            extra_files=_LENIENT_PAIR_FILES,
        ),
        JobOutputTestCase(
            description="a singular audit whose nodes' jobs all ignore failures is ignored",
            command_args=("build", "--select", "job:lenient_job", "job:lenient_peer_job"),
            expected_exit_code=0,
            expected_present=("audit lenient_pair returned 1 row (ignored)",),
            expected_absent=(),
            extra_files=_LENIENT_PAIR_FILES,
        ),
        JobOutputTestCase(
            description="a job may ignore the dynamic grace where the project allows it",
            command_args=("compile",),
            expected_exit_code=0,
            expected_present=("Project compiled",),
            expected_absent=("ignore_dynamic_grace",),
            extra_files={
                **_GRACE_FILES,
                "sqlbuild_project.toml": _PROJECT_FILES["sqlbuild_project.toml"]
                + "\n[dynamic]\nallow_ignore_grace = true\n",
            },
        ),
        JobOutputTestCase(
            description="a command-line option beats a job option it excludes",
            command_args=("build", "--select", "job:loads_off_job", "--reload"),
            expected_exit_code=0,
            expected_present=("Completed successfully",),
            expected_absent=("not allowed with",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_named_job_when_planning_or_building_then_the_output_reflects_its_settings(
    test_case: JobOutputTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv(CONCURRENCY_VARIABLE, raising=False)
    write_project_files(project_dir=tmp_path, files={**_PROJECT_FILES, **test_case.extra_files})

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command_args])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert {fragment: fragment in captured for fragment in test_case.expected_present} == (
        dict.fromkeys(test_case.expected_present, True)
    ), captured
    assert {fragment: fragment in captured for fragment in test_case.expected_absent} == (
        dict.fromkeys(test_case.expected_absent, False)
    ), captured


@pytest.mark.parametrize(
    "test_case",
    [
        JobFlagParityTestCase(
            description="full_refresh, run_tests and ignore_audit_failures act as the flags did",
            job_args=("--select", "job:parity_job"),
            flag_plan_args=(
                "--no-job-settings",
                "--full-refresh",
                "--ignore-audit-failures",
                "--select",
                *PARITY_NODES,
            ),
            flag_build_args=(
                "--no-job-settings",
                "--full-refresh",
                "--no-tests",
                "--ignore-audit-failures",
                "--select",
                *PARITY_NODES,
            ),
            expected_function_reasons=(("udf__band", "full refresh"),),
            expected_full_refresh_counts=("2",),
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_single_named_job_when_building_then_it_matches_its_settings_as_global_flags(
    test_case: JobFlagParityTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    job_outcome: tuple[object, ...] = job_flag_parity_outcome(
        project_dir=tmp_path / "job",
        args=(test_case.job_args, test_case.job_args),
        capsys=capsys,
    )
    flag_outcome: tuple[object, ...] = job_flag_parity_outcome(
        project_dir=tmp_path / "flags",
        args=(test_case.flag_plan_args, test_case.flag_build_args),
        capsys=capsys,
    )

    assert (job_outcome == flag_outcome, job_outcome[:3]) == (
        True,
        (
            test_case.expected_function_reasons,
            test_case.expected_full_refresh_counts,
            test_case.expected_exit_code,
        ),
    )
