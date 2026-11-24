"""E2E tests for scenario table promotion matching build configuration."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.scenario._test_types import (
    PromotionConflictE2ETestCase,
    PromotionConflictReuseE2ETestCase,
    ScenarioPromotionE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.scenario.helpers import (
    COMPILE_REUSE_HIT_LINE,
    build_promotion_project_files,
    list_scenario_relation_names,
    run_reusable_compile,
    write_project_settings,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    run_sqb,
    table_exists,
)

_MATCHING_COLUMNS: str = (
    "  columns (\n    total_amount (type BIGINT),\n    order_count (type BIGINT),\n  ),\n"
)
_ENFORCED_DEFAULTS: str = 'contract = "enforced"\n'
_CONFLICT_FRAGMENTS: tuple[str, ...] = (
    "error[K011]",
    "sql/models/order_totals.sql",
    "model 'order_totals': contract enforced requires staged table promotion",
    'sqlbuild_project.toml sets [settings] table_promotion_mode = "immediate"',
    "remove that line to use the default staged promotion, or set this in sqlbuild_project.toml:",
    'table_promotion_mode = "staged"',
    "`contract enforced` in its MODEL header",
)


@pytest.mark.parametrize(
    "test_case",
    (
        ScenarioPromotionE2ETestCase(
            description="project enforced contract runs the contract check and expectations",
            defaults_config=_ENFORCED_DEFAULTS,
            settings_config="",
            model_columns=_MATCHING_COLUMNS,
            expected_exit_code=0,
            expected_stdout_fragments=(
                "expect    expected order_totals",
                "PASS=1  FAIL=0  TOTAL=1",
            ),
            expect_staged_promotion=True,
        ),
        ScenarioPromotionE2ETestCase(
            description="explicit immediate setting is honoured without a contract",
            defaults_config="",
            settings_config='[settings]\ntable_promotion_mode = "immediate"\n',
            model_columns="",
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expect_staged_promotion=False,
        ),
        ScenarioPromotionE2ETestCase(
            description="explicit staged setting enables declared type enforcement",
            defaults_config="",
            settings_config='[settings]\ntable_promotion_mode = "staged"\n',
            model_columns=_MATCHING_COLUMNS,
            expected_exit_code=0,
            expected_stdout_fragments=("PASS=1  FAIL=0  TOTAL=1",),
            expect_staged_promotion=True,
        ),
        ScenarioPromotionE2ETestCase(
            description="runtime contract failure is reported and staging is cleaned up",
            defaults_config=_ENFORCED_DEFAULTS,
            settings_config="",
            model_columns=_MATCHING_COLUMNS.replace(
                "  ),\n", "    discount_amount (type BIGINT),\n  ),\n"
            ),
            expected_exit_code=1,
            expected_stdout_fragments=(
                "error[K008]",
                "runtime contract missing columns: discount_amount",
                "PASS=0  FAIL=1  TOTAL=1",
            ),
            expect_staged_promotion=True,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_promotion_configuration_when_running_scenario_then_matches_build_promotion(
    test_case: ScenarioPromotionE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_promotion",
        repo_files=build_promotion_project_files(test_case),
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "scenario", "test"),
        project_dir=project_dir,
    )

    output: str = result.stdout + result.stderr
    assert result.returncode == test_case.expected_exit_code, output
    for fragment in test_case.expected_stdout_fragments:
        assert fragment in result.stdout, output
    for fragment in test_case.unexpected_stdout_fragments:
        assert fragment not in output
    model_sql: str = (
        project_dir / "target/run/scenarios/order_totals_pass/sql/models/order_totals.sql"
    ).read_text(encoding="utf-8")
    assert ("__staging" in model_sql) is test_case.expect_staged_promotion, model_sql
    assert list_scenario_relation_names(db_path=project_dir / "scenario_promotion.duckdb") == ()


@pytest.mark.parametrize(
    "test_case",
    (
        PromotionConflictE2ETestCase(
            description="enforced defaults with immediate promotion fail before warehouse work",
            settings_config='[settings]\ntable_promotion_mode = "immediate"\n',
            expected_fragments=_CONFLICT_FRAGMENTS,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_enforced_contract_and_immediate_promotion_when_running_then_fails_at_compile(
    test_case: PromotionConflictE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_promotion",
        repo_files=build_promotion_project_files(
            ScenarioPromotionE2ETestCase(
                description=test_case.description,
                defaults_config=_ENFORCED_DEFAULTS,
                settings_config=test_case.settings_config,
                model_columns=_MATCHING_COLUMNS,
                expected_exit_code=1,
                expected_stdout_fragments=(),
                expect_staged_promotion=False,
            )
        ),
    )
    db_path: Path = project_dir / "scenario_promotion.duckdb"

    results: dict[str, subprocess.CompletedProcess[str]] = {
        name: run_sqb(command=("--no-color", *command), project_dir=project_dir)
        for name, command in (
            ("compile", ("compile",)),
            ("plan", ("plan",)),
            ("build", ("build",)),
            ("scenario", ("scenario", "test")),
            ("scenario_local", ("scenario", "test", "--local")),
        )
    }

    for name, result in results.items():
        output: str = result.stdout + result.stderr
        assert result.returncode == 1, (name, output)
        for fragment in test_case.expected_fragments[2:]:
            assert fragment in output, (name, fragment, output)
    assert all(
        fragment in results["compile"].stdout for fragment in test_case.expected_fragments
    ), results["compile"].stdout
    assert "order_totals START" not in results["build"].stdout + results["build"].stderr
    assert not db_path.exists() or not table_exists(db_path=db_path, table_name="order_totals")
    assert not db_path.exists() or list_scenario_relation_names(db_path=db_path) == ()


@pytest.mark.parametrize(
    "test_case",
    (
        PromotionConflictReuseE2ETestCase(
            description="promotion setting edits invalidate warm compile reuse",
            settings_steps=(
                "",
                "",
                '[settings]\ntable_promotion_mode = "immediate"\n',
                '[settings]\ntable_promotion_mode = "immediate"\n',
                '[settings]\ntable_promotion_mode = "staged"\n',
            ),
            expected_exit_codes=(0, 0, 1, 1, 0),
            expected_reused=(False, True, False, True, False),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_warm_compile_when_promotion_setting_changes_then_k011_follows_the_setting(
    test_case: PromotionConflictReuseE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="scenario_promotion",
        repo_files=build_promotion_project_files(
            ScenarioPromotionE2ETestCase(
                description=test_case.description,
                defaults_config=_ENFORCED_DEFAULTS,
                settings_config="",
                model_columns=_MATCHING_COLUMNS,
                expected_exit_code=0,
                expected_stdout_fragments=(),
                expect_staged_promotion=True,
            )
        ),
    )
    base_toml: str = (project_dir / "sqlbuild_project.toml").read_text(encoding="utf-8")

    outcomes: list[tuple[int, bool, bool]] = []
    for settings in test_case.settings_steps:
        write_project_settings(project_dir=project_dir, base_toml=base_toml, settings=settings)
        result: subprocess.CompletedProcess[str] = run_reusable_compile(project_dir=project_dir)
        output: str = result.stdout + result.stderr
        outcomes.append(
            (result.returncode, COMPILE_REUSE_HIT_LINE in output, "error[K011]" in output)
        )

    assert tuple(code for code, _, _ in outcomes) == test_case.expected_exit_codes
    assert tuple(reused for _, reused, _ in outcomes) == test_case.expected_reused
    assert tuple(k011 for _, _, k011 in outcomes) == tuple(
        code != 0 for code in test_case.expected_exit_codes
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
