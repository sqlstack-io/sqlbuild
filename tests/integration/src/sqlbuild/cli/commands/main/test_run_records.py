"""Every build records its start and end in each schema it writes, and a failed write never fails it."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

from sqlbuild._native import run_cli as main
from sqlbuild.adapter.contract.classes.runs_table import RunsTableMixin
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    InterruptedRunRecordTestCase,
    KilledRunRecordTestCase,
    RunRecordFailureTestCase,
    RunRecordSchemasTestCase,
    RunRecordTestCase,
    SeedRunRecordTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    RUN_RECORD_COLUMNS,
    RUN_RECORD_MARTS_FILES,
    RUN_RECORD_MARTS_SCHEMA,
    built_tables,
    prepare_warehouse,
    run_rows,
    runs_tables,
    schema_run_ids,
    schema_run_phases,
    schema_run_statuses,
    warehouse_schemas,
    write_run_record_project,
)

_PAIR: list[str] = ["started", "finished"]
_SUCCEEDED_TWO: str = '{"succeeded":2,"warned":0,"failed":0,"skipped":0,"not_started":0}'


@pytest.mark.parametrize(
    "test_case",
    [
        RunRecordTestCase(
            description="a job build records its selection, the job and only the names of its vars",
            build_args=(
                "--select",
                "job:nightly_job",
                "--vars",
                '{"api_token": "s3cret-value", "batch": 2}',
            ),
            expected_exit_code=0,
            expected_rows=[
                (
                    "started",
                    "build",
                    '["job:nightly_job"]',
                    '["nightly_job"]',
                    '["api_token","batch"]',
                    None,
                    None,
                ),
                (
                    "finished",
                    "build",
                    '["job:nightly_job"]',
                    '["nightly_job"]',
                    '["api_token","batch"]',
                    "succeeded",
                    _SUCCEEDED_TWO,
                ),
            ],
        ),
        RunRecordTestCase(
            description="a bare job name counts as a named job",
            build_args=("--select", "nightly_job"),
            expected_exit_code=0,
            expected_rows=[
                ("started", "build", '["nightly_job"]', '["nightly_job"]', "[]", None, None),
                (
                    "finished",
                    "build",
                    '["nightly_job"]',
                    '["nightly_job"]',
                    "[]",
                    "succeeded",
                    _SUCCEEDED_TWO,
                ),
            ],
        ),
        RunRecordTestCase(
            description="a failing model beside successes finishes partial",
            build_args=("--select", "stg_orders", "broken_orders"),
            expected_exit_code=1,
            expected_rows=[
                ("started", "build", '["stg_orders","broken_orders"]', "[]", "[]", None, None),
                (
                    "finished",
                    "build",
                    '["stg_orders","broken_orders"]',
                    "[]",
                    "[]",
                    "partial",
                    '{"succeeded":1,"warned":0,"failed":1,"skipped":0,"not_started":0}',
                ),
            ],
        ),
        RunRecordTestCase(
            description="a failing task fails the run even though no model failed",
            build_args=("--select", "task:fail"),
            expected_exit_code=1,
            expected_rows=[
                ("started", "build", '["task:fail"]', "[]", "[]", None, None),
                (
                    "finished",
                    "build",
                    '["task:fail"]',
                    "[]",
                    "[]",
                    "failed",
                    '{"succeeded":0,"warned":0,"failed":1,"skipped":0,"not_started":0}',
                ),
            ],
        ),
        RunRecordTestCase(
            description="a project that turns off the host leaves it out",
            build_args=("--select", "stg_orders"),
            expected_exit_code=0,
            expected_rows=[
                ("started", "build", '["stg_orders"]', "[]", "[]", None, None),
                (
                    "finished",
                    "build",
                    '["stg_orders"]',
                    "[]",
                    "[]",
                    "succeeded",
                    '{"succeeded":1,"warned":0,"failed":0,"skipped":0,"not_started":0}',
                ),
            ],
            settings="\n[settings]\nrecord_run_host = false\n",
            expected_host_recorded=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_build_when_it_runs_then_it_records_its_start_and_end(
    test_case: RunRecordTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_record_project(project_dir=tmp_path, settings=test_case.settings)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests", *test_case.build_args]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert run_rows(project_dir=tmp_path, columns=RUN_RECORD_COLUMNS) == test_case.expected_rows
    identities: list[tuple[object, ...]] = run_rows(
        project_dir=tmp_path,
        columns="run_id, attempt, retry_of_run_id, orchestration_run_id, target_name, "
        "warehouse_user, os_user IS NOT NULL, host IS NOT NULL, sqlbuild_version IS NOT NULL",
    )
    assert len({identity[0] for identity in identities}) == 1
    assert {identity[1:] for identity in identities} == {
        (1, None, None, "dev", None, True, test_case.expected_host_recorded, True)
    }
    assert "s3cret-value" not in str(run_rows(project_dir=tmp_path, columns="*"))


@pytest.mark.parametrize(
    "test_case",
    [
        SeedRunRecordTestCase(
            description="a seed run records its start and end like a build",
            expected_rows=[
                ("started", "seed", "[]", "[]", "[]", None, None),
                (
                    "finished",
                    "seed",
                    "[]",
                    "[]",
                    "[]",
                    "succeeded",
                    '{"succeeded":1,"warned":0,"failed":0,"skipped":0,"not_started":0}',
                ),
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_seed_run_when_it_loads_then_it_records_its_start_and_end(
    test_case: SeedRunRecordTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_record_project(project_dir=tmp_path, settings="")
    (tmp_path / "seeds").mkdir()
    (tmp_path / "seeds" / "regions.csv").write_text("region_id\n1\n", encoding="utf-8")
    (tmp_path / "seeds" / "regions.yml").write_text(
        "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
        "      - name: region_id\n        type: INTEGER\n",
        encoding="utf-8",
    )

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", "seed"])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == 0, captured
    assert run_rows(project_dir=tmp_path, columns=RUN_RECORD_COLUMNS) == test_case.expected_rows


@pytest.mark.parametrize(
    "test_case",
    [
        RunRecordFailureTestCase(
            description="a view in the table's place makes every write fail; the build still succeeds",
            warehouse_setup="CREATE VIEW main._sqlbuild_runs AS SELECT 1 AS run_id",
            expected_exit_code=0,
            expected_warning="Warning: could not record that run",
            expected_tables=frozenset({"stg_orders"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_unwritable_runs_table_when_building_then_it_warns_and_succeeds(
    test_case: RunRecordFailureTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_record_project(project_dir=tmp_path, settings="")
    prepare_warehouse(project_dir=tmp_path, sql=test_case.warehouse_setup)

    exit_code: int = main(
        ["--project-dir", str(tmp_path), "--no-color", "build", "--no-tests", "-s", "stg_orders"]
    )
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert captured.count(test_case.expected_warning) == 2, captured
    assert built_tables(project_dir=tmp_path) == test_case.expected_tables


@pytest.mark.parametrize(
    "test_case",
    [
        InterruptedRunRecordTestCase(
            description="Ctrl+C while the started row is written stops the build before any node",
            expected_tables=frozenset(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_interrupt_while_recording_when_building_then_the_build_stops(
    test_case: InterruptedRunRecordTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    write_run_record_project(project_dir=tmp_path, settings="")

    def interrupt(self: object) -> str | None:
        raise KeyboardInterrupt

    monkeypatch.setattr(RunsTableMixin, "render_current_warehouse_user", interrupt)

    with pytest.raises(KeyboardInterrupt):
        main(
            [
                "--project-dir",
                str(tmp_path),
                "--no-color",
                "build",
                "--no-tests",
                "-s",
                "stg_orders",
            ]
        )

    assert built_tables(project_dir=tmp_path) == test_case.expected_tables


@pytest.mark.parametrize(
    "test_case",
    [
        KilledRunRecordTestCase(
            description="a build killed mid-run leaves only its started row",
            expected_phases=["started"],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_killed_build_when_reading_its_runs_then_only_started_remains(
    test_case: KilledRunRecordTestCase,
    tmp_path: Path,
) -> None:
    write_run_record_project(project_dir=tmp_path, settings="")

    completed: subprocess.CompletedProcess[str] = subprocess.run(
        [
            str(Path(sys.executable).with_name("sqb")),
            "--project-dir",
            str(tmp_path),
            "--no-color",
            "build",
            "--no-tests",
            "--select",
            "task:crash",
        ],
        capture_output=True,
        text=True,
        timeout=300,
        env={**os.environ, "NO_COLOR": "1"},
        check=False,
    )

    assert completed.returncode == 137, completed.stdout + completed.stderr
    assert [row[0] for row in run_rows(project_dir=tmp_path, columns="phase")] == (
        test_case.expected_phases
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RunRecordSchemasTestCase(
            description="a build writing two schemas records its pair in each under one run id",
            command_args=("build", "--no-tests", "--select", "stg_orders", "revenue"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"main": _PAIR, "marts": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="a build writing only a custom schema never creates the target's schema",
            command_args=("build", "--no-tests", "--select", "revenue"),
            settings='\n[defaults]\nschema = "analytics"\n' + RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"marts": _PAIR},
            expected_absent_schemas=frozenset({"analytics"}),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="an unwritable schema warns naming it while the other schema records",
            command_args=("build", "--no-tests", "--select", "stg_orders", "revenue"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="CREATE VIEW main._sqlbuild_runs AS SELECT 1 AS run_id",
            expected_exit_code=0,
            expected_phases={"marts": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(
                " started in main._sqlbuild_runs: ",
                " finished in main._sqlbuild_runs: ",
            ),
        ),
        RunRecordSchemasTestCase(
            description="a build loading a source records in the source's schema too",
            command_args=("build", "--no-tests", "--select", "+daily_events"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"landing": _PAIR, "marts": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="case spellings of one schema record once",
            command_args=("build", "--no-tests", "--select", "revenue", "revenue_upper"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"marts": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="a skipped model's schema records nothing",
            command_args=("build", "--no-tests", "--select", "stg_orders", "old_orders"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"main": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
            expected_finished_status="partial",
        ),
        RunRecordSchemasTestCase(
            description="a function's schema records the run that deploys it",
            command_args=("build", "--no-tests", "--select", "doubled"),
            settings=RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"marts": _PAIR, "udfs": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="a build that raises finishes failed in each schema",
            command_args=("build", "--no-tests", "--select", "stg_orders", "slow_total"),
            settings=RUN_RECORD_MARTS_SCHEMA
            + '\n[targets.dev.execution_limits]\nmax_duration = "2s"\n',
            warehouse_setup="",
            expected_exit_code=1,
            expected_phases={"main": _PAIR, "marts": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
            expected_finished_status="failed",
        ),
        RunRecordSchemasTestCase(
            description="a task-only run records in the target's default schema",
            command_args=("build", "--no-tests", "--select", "task:noop"),
            settings='\n[defaults]\nschema = "analytics"\n' + RUN_RECORD_MARTS_SCHEMA,
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"analytics": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
        RunRecordSchemasTestCase(
            description="a seed run records in its seeds' schema and not the target's",
            command_args=("seed",),
            settings='\n[defaults]\nseed_schema = "reference"\n',
            warehouse_setup="",
            expected_exit_code=0,
            expected_phases={"reference": _PAIR},
            expected_absent_schemas=frozenset(),
            expected_warnings=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_run_writing_schemas_when_it_runs_then_each_schema_records_it(
    test_case: RunRecordSchemasTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_run_record_project(
        project_dir=tmp_path, settings=test_case.settings, extra_files=RUN_RECORD_MARTS_FILES
    )
    prepare_warehouse(project_dir=tmp_path, sql=test_case.warehouse_setup or "SELECT 1")

    exit_code: int = main(["--project-dir", str(tmp_path), "--no-color", *test_case.command_args])
    captured: str = "".join(capsys.readouterr())

    assert exit_code == test_case.expected_exit_code, captured
    assert runs_tables(project_dir=tmp_path) == frozenset(test_case.expected_phases), captured
    assert (
        schema_run_phases(project_dir=tmp_path, schemas=test_case.expected_phases)
        == test_case.expected_phases
    )
    assert len(schema_run_ids(project_dir=tmp_path, schemas=test_case.expected_phases)) == 1
    assert schema_run_statuses(project_dir=tmp_path, schemas=test_case.expected_phases) == {
        None,
        test_case.expected_finished_status,
    }
    assert not test_case.expected_absent_schemas & warehouse_schemas(project_dir=tmp_path)
    assert captured.count("Warning: could not record that run") == len(
        test_case.expected_warnings
    ), captured
    assert all(warning in captured for warning in test_case.expected_warnings), captured
