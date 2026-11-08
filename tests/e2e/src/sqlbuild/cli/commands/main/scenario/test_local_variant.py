"""E2E coverage of local scenario replay over VARIANT fixtures, which need current DuckDB storage."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    CapturedVariantTextScenarioTestCase,
    LocalVariantScenarioTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    build_local_variant_scenario_project_files,
    build_snowflake_variant_scenario_project_files,
    write_snowflake_variant_snapshot,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        LocalVariantScenarioTestCase(
            description="a captured VARIANT fixture replays locally in strict mode",
            repo_files=build_local_variant_scenario_project_files(),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        LocalVariantScenarioTestCase(
            description="an expected VARIANT object compares equal to the replayed fixture",
            repo_files=build_local_variant_scenario_project_files(
                output="o.payload",
                expected='\'{"qty": 2, "tags": ["a"]}\'::JSON::VARIANT AS payload',
            ),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        LocalVariantScenarioTestCase(
            description="a cast-built VARIANT object compares equal to the replayed fixture",
            repo_files=build_local_variant_scenario_project_files(
                fixture="{'qty': 2, 'note': 'x'}::VARIANT",
                output="o.payload",
                expected="{'note': 'x', 'qty': 2}::VARIANT AS payload",
            ),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        LocalVariantScenarioTestCase(
            description="a cast-built VARIANT scalar compares equal to the replayed fixture",
            repo_files=build_local_variant_scenario_project_files(
                fixture="2::VARIANT", output="o.payload", expected="2::VARIANT AS payload"
            ),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        LocalVariantScenarioTestCase(
            description="a quoted VARIANT output column compares by its JSON value",
            repo_files=build_local_variant_scenario_project_files(
                fixture="{'qty': 2, 'note': 'x'}::VARIANT",
                output='o.payload AS "Payload"',
                expected='\'{"note": "x", "qty": 2}\'::JSON::VARIANT AS "Payload"',
            ),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="PASS=1  FAIL=0  ERROR=0  SKIP=0  TOTAL=1",
        ),
        LocalVariantScenarioTestCase(
            description="a different cast-built VARIANT value still fails the comparison",
            repo_files=build_local_variant_scenario_project_files(
                fixture="{'qty': 2}::VARIANT",
                output="o.payload",
                expected="{'qty': 3}::VARIANT AS payload",
            ),
            scenario_name="orders__keep_payload",
            expected_stdout_fragment="mismatched=1 missing=1",
            expected_returncode=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_captured_variant_fixture_when_replaying_locally_then_the_scenario_passes(
    test_case: LocalVariantScenarioTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="local_variant", repo_files=test_case.repo_files
    )
    captured: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "capture", test_case.scenario_name),
        project_dir=project_dir,
    )
    assert captured.returncode == 0, captured.stdout + captured.stderr

    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "scenario",
            "test",
            "--local",
            "--strict",
            test_case.scenario_name,
        ),
        project_dir=project_dir,
    )

    assert replayed.returncode == test_case.expected_returncode, replayed.stdout + replayed.stderr
    assert "X699" not in replayed.stdout
    assert test_case.expected_stdout_fragment in replayed.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        CapturedVariantTextScenarioTestCase(
            description="a PARSE_JSON expected value is refused as a JSON divergence",
            captured_payload_text='{\n  "note": "x",\n  "qty": 2\n}',
            expected_payload_json='{"qty": 2, "note": "x"}',
            expected_returncode=1,
            expected_stdout_fragment="fn:PARSE_JSON needs known divergence `json_fidelity`",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_snowflake_variant_text_snapshot_when_replaying_locally_then_parse_json_is_refused(
    test_case: CapturedVariantTextScenarioTestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="snowflake_variant",
        repo_files=build_snowflake_variant_scenario_project_files(
            expected_payload_json=test_case.expected_payload_json
        ),
    )
    write_snowflake_variant_snapshot(
        project_dir=project_dir,
        scenario_name="payloads__variant_compare",
        payload_text=test_case.captured_payload_text,
    )

    replayed: subprocess.CompletedProcess[str] = run_sqb(
        command=(
            "--no-color",
            "scenario",
            "test",
            "--local",
            "--strict",
            "payloads__variant_compare",
        ),
        project_dir=project_dir,
    )

    assert replayed.returncode == test_case.expected_returncode, replayed.stdout + replayed.stderr
    assert test_case.expected_stdout_fragment in replayed.stdout, replayed.stdout
