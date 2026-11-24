from __future__ import annotations

import os
import subprocess
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.dbt._test_types import (
    DbtPlanCliTestCase,
    DbtPlanErrorCliTestCase,
    DbtPlanHumanCliTestCase,
    DbtPlanRelativeProjectDirTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.dbt.helpers import (
    load_json_stdout,
    prepare_dbt_interop_project,
    skip_unless_dbt_is_runnable,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    REPO_ROOT,
    prepare_inline_project,
    run_sqb,
)

pytestmark: pytest.MarkDecorator = pytest.mark.dbt


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanCliTestCase(
            description="reports SQLBuild-only selected work and dbt skip",
            command=("dbt", "plan", "--json", "--select", "local_only"),
            expected_selected_models=("local_only",),
            expected_dbt_skipped=True,
            expected_sqlbuild_skipped=False,
        ),
        DbtPlanCliTestCase(
            description="reports dbt trailing plus anchors and downstream SQLBuild work",
            command=("dbt", "plan", "--json", "--select", "fact_orders+"),
            expected_selected_models=("downstream_orders", "event_time_orders", "mart_orders"),
            expected_dbt_skipped=False,
            expected_sqlbuild_skipped=False,
            expected_anchor_terms=("fact_orders+",),
        ),
        DbtPlanCliTestCase(
            description="routes explicit model path for SQLBuild model paths",
            command=("dbt", "plan", "--json", "--select", "path:sql/models/marts"),
            expected_selected_models=(
                "deprecated_orders",
                "downstream_orders",
                "event_time_orders",
                "mart_orders",
            ),
            expected_dbt_skipped=False,
            expected_sqlbuild_skipped=False,
            expected_path_translations=(),
        ),
        DbtPlanCliTestCase(
            description="reports mixed dbt and SQLBuild tag matches",
            command=("dbt", "plan", "--json", "--select", "tag:nightly"),
            expected_selected_models=("downstream_orders",),
            expected_dbt_skipped=False,
            expected_sqlbuild_skipped=False,
        ),
        DbtPlanCliTestCase(
            description="reports SQLBuild-only tag matches with dbt skipped",
            command=("dbt", "plan", "--json", "--select", "tag:sqb_only"),
            expected_selected_models=("deprecated_orders", "local_only"),
            expected_dbt_skipped=True,
            expected_sqlbuild_skipped=False,
        ),
        DbtPlanCliTestCase(
            description="reports SQLBuild tag excludes",
            command=(
                "dbt",
                "plan",
                "--json",
                "--select",
                "tag:sqb_only",
                "--exclude",
                "tag:deprecated",
            ),
            expected_selected_models=("local_only",),
            expected_dbt_skipped=True,
            expected_sqlbuild_skipped=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_interop_project_when_running_plan_json_then_outputs_expected_plan(
    test_case: DbtPlanCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command, project_dir=project_dir
    )

    assert result.returncode == 0, result.stderr or result.stdout
    payload: dict[str, object] = load_json_stdout(result.stdout)
    dbt_payload: object = payload["dbt"]
    sqlbuild_payload: object = payload["sqlbuild"]
    anchors_payload: object = payload["anchors"]
    path_translations_payload: object = payload["path_translations"]
    assert isinstance(dbt_payload, dict)
    assert isinstance(sqlbuild_payload, dict)
    assert isinstance(anchors_payload, Sequence)
    assert isinstance(path_translations_payload, Sequence)
    assert all(isinstance(anchor, dict) for anchor in anchors_payload)
    assert all(isinstance(translation, dict) for translation in path_translations_payload)
    typed_dbt_payload: Mapping[str, object] = cast(Mapping[str, object], dbt_payload)
    typed_sqlbuild_payload: Mapping[str, object] = cast(Mapping[str, object], sqlbuild_payload)
    assert typed_sqlbuild_payload["selected_models"] == list(test_case.expected_selected_models)
    assert typed_dbt_payload["skipped"] == test_case.expected_dbt_skipped
    assert typed_sqlbuild_payload["skipped"] == test_case.expected_sqlbuild_skipped
    assert [cast(Mapping[str, object], anchor)["term"] for anchor in anchors_payload] == list(
        test_case.expected_anchor_terms
    )
    assert [
        (
            cast(Mapping[str, object], translation)["from"],
            cast(Mapping[str, object], translation)["to"],
        )
        for translation in path_translations_payload
    ] == list(test_case.expected_path_translations)


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanRelativeProjectDirTestCase(
            description="resolves dbt config paths from relative SQLBuild project dir",
            command=("dbt", "plan", "--json", "--select", "tag:nightly"),
            expected_selected_models=("downstream_orders",),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_relative_project_dir_when_running_dbt_plan_then_resolves_dbt_config_paths(
    test_case: DbtPlanRelativeProjectDirTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)
    relative_project_dir: Path = Path(os.path.relpath(project_dir, REPO_ROOT))

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=relative_project_dir,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    payload: dict[str, object] = load_json_stdout(result.stdout)
    sqlbuild_payload: object = payload["sqlbuild"]
    assert isinstance(sqlbuild_payload, dict)
    typed_sqlbuild_payload: Mapping[str, object] = cast(Mapping[str, object], sqlbuild_payload)
    assert relative_project_dir != project_dir
    assert typed_sqlbuild_payload["selected_models"] == list(test_case.expected_selected_models)


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanHumanCliTestCase(
            description="outputs grouped dbt and SQLBuild plan sections",
            command=("dbt", "plan", "--select", "tag:nightly"),
            expected_stdout_fragments=(
                "Plan ready  5 selected resources",
                "dbt (4 selected, 0 required)",
                "dbt ls",
                "model (1)",
                "stg_orders",
                "SQLBuild (1)",
                "downstream_orders",
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_interop_project_when_running_human_plan_then_outputs_grouped_sections(
    test_case: DbtPlanHumanCliTestCase,
    tmp_path: Path,
) -> None:
    skip_unless_dbt_is_runnable()
    project_dir: Path = prepare_dbt_interop_project(tmp_path=tmp_path)

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    for expected_stdout_fragment in test_case.expected_stdout_fragments:
        assert expected_stdout_fragment in result.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        DbtPlanErrorCliTestCase(
            description="renders missing dbt project config with coded error",
            command=("dbt", "plan", "--json", "--select", "anything"),
            expected_stderr_fragments=(
                "error[C240]: dbt project directory is not configured",
                '= help: to point SQLBuild at the dbt project, set this in sqlbuild_project.toml:\n            [dbt]\n            project_dir = "path/to/dbt_project"',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_dbt_project_config_when_running_plan_then_renders_coded_error(
    test_case: DbtPlanErrorCliTestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="missing_dbt_config",
        repo_files={
            "sqlbuild_project.toml": 'name = "missing_dbt_config"\nadapter = "duckdb"\n',
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == 1
    expected_stderr_fragment: str
    for expected_stderr_fragment in test_case.expected_stderr_fragments:
        assert expected_stderr_fragment in result.stderr
