"""Real cold compilation preserves publication gates and unchanged artifact files."""

import json
import os
import shutil
from contextlib import ExitStack
from pathlib import Path
from tempfile import TemporaryDirectory

import duckdb
import pytest
from filelock import FileLock

import sqlbuild._native as native_module
from sqlbuild._native import run_cli as main
from tests.integration.src.sqlbuild.cli.commands.main._test_types import (
    AbandonedStagingCompileTestCase,
    BackgroundTestPlanningCompileTestCase,
    BackgroundTestPlanningFailureTestCase,
    CrossDeviceArtifactsCompileTestCase,
    PreparedArtifactsCompileTestCase,
)
from tests.integration.src.sqlbuild.cli.commands.main.helpers import (
    NO_RULES_CONFIG,
    SELECTED_RULES_CONFIG,
    TEST_PLANNING_MODELS,
    compile_project_json,
    compile_then_edit_one_test,
    compiled_files,
    create_owned_staging_directory,
    has_second_filesystem,
    keep_native_planner,
    keep_project_unchanged,
    skip_artifact_preparation,
    staging_directories,
    unavailable_staging_directory,
    use_prepared_artifacts,
    use_staging_directory_factory,
    write_prepared_artifacts_project,
    write_staging_directory,
    write_test_planning_project,
)

BACKGROUND_SQL_TEST_PLANS: str = "background_sql_test_plans"

_SECOND_FILESYSTEM: Path = Path("/dev/shm")


@pytest.mark.parametrize(
    "test_case",
    (
        BackgroundTestPlanningCompileTestCase(
            "cold compile with the compile cache",
            (),
            SELECTED_RULES_CONFIG,
            keep_project_unchanged,
        ),
        BackgroundTestPlanningCompileTestCase(
            "warm compile after one SQL test edit",
            (),
            SELECTED_RULES_CONFIG,
            compile_then_edit_one_test,
        ),
        BackgroundTestPlanningCompileTestCase(
            "no-cache compile above the staged artifact limit",
            ("--no-cache",),
            SELECTED_RULES_CONFIG,
            keep_project_unchanged,
            max_prepared_models=1,
        ),
        BackgroundTestPlanningCompileTestCase(
            "no-cache compile without selected rules",
            ("--no-cache",),
            NO_RULES_CONFIG,
            keep_project_unchanged,
        ),
        BackgroundTestPlanningCompileTestCase(
            "no-cache compile that stages every artifact",
            ("--no-cache",),
            SELECTED_RULES_CONFIG,
            keep_project_unchanged,
            expected_background_plans=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compile_mode_when_preparing_artifacts_in_background_then_output_matches_inline(
    test_case: BackgroundTestPlanningCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    use_prepared_artifacts(monkeypatch, model_range=(1, test_case.max_prepared_models))
    background: Path = tmp_path / "background"
    write_test_planning_project(
        project_dir=background,
        model_count=TEST_PLANNING_MODELS,
        rules_config=test_case.rules_config,
    )
    test_case.prepare_project(project_dir=background, capsys=capsys)
    inline: Path = tmp_path / "inline"
    shutil.copytree(background, inline)
    _ = native_module.compile_stage_work_info(reset=True)

    background_result: tuple[int, dict[str, object]] = compile_project_json(
        project_dir=background, compile_args=test_case.compile_args, capsys=capsys
    )
    with monkeypatch.context() as inline_patch:
        skip_artifact_preparation(inline_patch)
        inline_result: tuple[int, dict[str, object]] = compile_project_json(
            project_dir=inline, compile_args=test_case.compile_args, capsys=capsys
        )

    assert native_module.compile_stage_work_info().get(BACKGROUND_SQL_TEST_PLANS, 0) == len(
        test_case.expected_background_plans
    )
    assert background_result[0] == 0
    assert background_result == inline_result
    assert background_result[1]["has_errors"] is False
    artifacts: dict[str, bytes] = compiled_files(background)
    assert artifacts == compiled_files(inline)
    assert sum(path.startswith("tests/") for path in artifacts) == TEST_PLANNING_MODELS - 1
    assert staging_directories(background / "target") == []


@pytest.mark.parametrize(
    "test_case",
    (
        BackgroundTestPlanningFailureTestCase(
            "planner input error with the compile cache",
            (),
            ', cursor_start "2026-02-01"',
            keep_native_planner,
            "declares cursor_start or cursor_end",
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_background_planning_fails_when_compiling_then_error_matches_inline_planning(
    test_case: BackgroundTestPlanningFailureTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    test_case.patch_native_planner(monkeypatch)
    background: Path = tmp_path / "background"
    write_test_planning_project(
        project_dir=background,
        model_count=TEST_PLANNING_MODELS,
        rules_config=SELECTED_RULES_CONFIG,
        test_options=test_case.test_options,
    )
    inline: Path = tmp_path / "inline"
    shutil.copytree(background, inline)
    _ = native_module.compile_stage_work_info(reset=True)

    background_result: tuple[int, dict[str, object]] = compile_project_json(
        project_dir=background, compile_args=test_case.compile_args, capsys=capsys
    )
    with monkeypatch.context() as inline_patch:
        skip_artifact_preparation(inline_patch)
        inline_result: tuple[int, dict[str, object]] = compile_project_json(
            project_dir=inline, compile_args=test_case.compile_args, capsys=capsys
        )

    assert native_module.compile_stage_work_info().get(BACKGROUND_SQL_TEST_PLANS, 0) == 1
    assert background_result[0] == 1
    assert background_result == inline_result
    assert test_case.expected_error_fragment in json.dumps(background_result[1])
    artifacts: dict[str, bytes] = compiled_files(background)
    assert artifacts == compiled_files(inline)
    assert not any(path.startswith("tests/") for path in artifacts)


@pytest.mark.parametrize(
    "test_case",
    (
        PreparedArtifactsCompileTestCase(
            "cold staged artifacts", 128, create_owned_staging_directory
        ),
        PreparedArtifactsCompileTestCase(
            "unavailable staging storage", 128, unavailable_staging_directory
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_existing_artifacts_when_rules_fail_then_staged_changes_are_not_published(
    test_case: PreparedArtifactsCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    use_staging_directory_factory(monkeypatch, create_staging=test_case.staging_directory_factory)
    config_path: Path = tmp_path / "sqlbuild_project.toml"
    config: str = 'name = "orders"\nadapter = "duckdb"\n[rules]\nselect = ["SQLCLEAN021"]\n'
    config_path.write_text(config, encoding="utf-8")
    models: Path = tmp_path / "models"
    models.mkdir()
    header: str = "MODEL (description 'Test model.', materialized table, contract enforced, columns (order_id (type INTEGER)));\n"
    for index in range(test_case.model_count):
        (models / f"orders_{index:03}.sql").write_text(
            header + "SELECT CAST(1 AS INTEGER) AS order_id\n", encoding="utf-8"
        )
    arguments: list[str] = [
        "--project-dir",
        str(tmp_path),
        "compile",
        "--no-cache",
        "--json",
        "--manifest",
    ]
    assert main(arguments) == test_case.expected_exit_code
    first: dict[str, object] = json.loads(capsys.readouterr().out)
    assert first["has_errors"] is False
    compiled: Path = tmp_path / "target" / "compiled"
    before: dict[Path, bytes] = {path: path.read_bytes() for path in compiled.rglob("*.sql")}
    modified_times: dict[Path, int] = {path: path.stat().st_mtime_ns for path in before}
    assert len(before) == test_case.model_count
    assert staging_directories(tmp_path / "target") == []
    stale: Path = compiled / "models" / "obsolete.sql"
    stale.write_text("SELECT 0\n", encoding="utf-8")
    (models / "orders_000.sql").write_text(
        header + "SELECT CAST(2 AS INTEGER) AS order_id\n", encoding="utf-8"
    )
    rules: Path = tmp_path / "rules"
    rules.mkdir()
    (rules / "orders.py").write_text(
        "from sqlbuild.rules import Finding, Model, RuleContext, rule\n"
        '@rule(code="XSQBRARCH001", message="Model rejected", remediation="Update the model.")\n'
        "def check_orders(*, model: Model, ctx: RuleContext) -> list[Finding]:\n"
        '    return [ctx.finding(subject=model)] if model.name == "orders_000" else []\n',
        encoding="utf-8",
    )
    config_path.write_text(
        config.replace('select = ["SQLCLEAN021"]', 'select = ["XSQBRARCH001"]'), encoding="utf-8"
    )
    assert main(arguments) == 1
    failed: dict[str, object] = json.loads(capsys.readouterr().out)
    assert failed["has_errors"] is True
    changed: Path = compiled / "models" / "orders_000.sql"
    assert {path: path.read_bytes() for path in before} == before
    assert (tmp_path / "target" / "compiled-failed" / "models" / "orders_000.sql").read_bytes() == (
        b"-- sqb: compile failed for this model; not buildable\n"
        b"-- XSQBRARCH001 at models/orders_000.sql:1:1: Model rejected\n"
        b"SELECT CAST(2 AS INTEGER) AS order_id\n"
    )
    assert stale.exists()
    assert staging_directories(tmp_path / "target") == []

    config_path.write_text(config, encoding="utf-8")
    assert main(arguments) == 0
    final: dict[str, object] = json.loads(capsys.readouterr().out)
    assert final["has_errors"] is False
    assert not stale.exists()
    assert not (tmp_path / "target" / "compiled-failed").exists()
    for path in modified_times.keys() - {changed}:
        assert path.stat().st_mtime_ns == modified_times[path]
        assert path.read_bytes() == before[path]
    assert staging_directories(tmp_path / "target") == []
    with duckdb.connect() as connection:
        assert connection.execute(changed.read_text(encoding="utf-8")).fetchall() == [(2,)]


@pytest.mark.parametrize(
    "test_case",
    (
        AbandonedStagingCompileTestCase(
            description="only staging whose owner lock is free is removed",
            model_count=128,
            abandoned=(".sqlbuild-staging-killed",),
            locked=(".sqlbuild-staging-running",),
            expected_remaining=(".sqlbuild-staging-running", ".sqlbuild-staging-running.lock"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_leftover_staging_when_compiling_then_only_abandoned_staging_is_removed(
    test_case: AbandonedStagingCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_prepared_artifacts_project(project_dir=tmp_path, model_count=test_case.model_count)
    target: Path = tmp_path / "target"
    for name in test_case.abandoned:
        write_staging_directory(target_dir=target, name=name)
    with ExitStack() as owners:
        for name in test_case.locked:
            owners.enter_context(FileLock(target / f"{name}.lock"))
            write_staging_directory(target_dir=target, name=name)
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
        remaining: list[str] = staging_directories(target)

    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == 0
    assert payload["has_errors"] is False
    assert tuple(remaining) == test_case.expected_remaining
    assert len(list((target / "compiled" / "models").glob("*.sql"))) == test_case.model_count


@pytest.mark.skipif(
    not has_second_filesystem(_SECOND_FILESYSTEM),
    reason="needs /dev/shm on a different filesystem from the temporary directory",
)
@pytest.mark.parametrize(
    "test_case",
    (CrossDeviceArtifactsCompileTestCase("compiled directory on another filesystem", 128),),
    ids=lambda case: case.description,
)
def test_given_compiled_dir_on_other_filesystem_when_compiling_then_artifacts_are_published(
    test_case: CrossDeviceArtifactsCompileTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    write_prepared_artifacts_project(project_dir=tmp_path, model_count=test_case.model_count)
    with TemporaryDirectory(dir=_SECOND_FILESYSTEM) as other_filesystem:
        compiled: Path = Path(other_filesystem) / "compiled"
        compiled.mkdir()
        (tmp_path / "target").mkdir()
        (tmp_path / "target" / "compiled").symlink_to(compiled, target_is_directory=True)
        exit_code: int = main(["--project-dir", str(tmp_path), "compile", "--no-cache", "--json"])
        published: list[Path] = sorted((compiled / "models").glob("*.sql"))
        leftovers: list[str] = sorted(path.name for path in compiled.rglob(".*"))
        with duckdb.connect() as connection:
            last_value: object = connection.execute(
                published[-1].read_text(encoding="utf-8")
            ).fetchone()
        device_differs: bool = os.stat(compiled).st_dev != os.stat(tmp_path).st_dev

    payload: dict[str, object] = json.loads(capsys.readouterr().out)
    assert exit_code == test_case.expected_exit_code
    assert payload["has_errors"] is False
    assert device_differs
    assert (tmp_path / "target" / "compiled").is_symlink()
    assert len(published) == test_case.model_count
    assert last_value == (test_case.model_count - 1,)
    assert leftovers == []
    assert staging_directories(tmp_path / "target") == []


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
