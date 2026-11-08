"""Native project assembly equals Python's recorded resource assembly and its errors."""

from __future__ import annotations

import os
import random
from collections import Counter
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import assemble_project
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.helpers import (
    GoldenEntry,
    golden_differences,
    golden_entry,
    golden_name,
    read_golden,
)
from tests.integration.src.sqlbuild.compiler.project_assembly._test_types import (
    AssemblyDeferralTestCase,
    EdgeAssemblyTestCase,
    GeneratedAssemblyParityTestCase,
    OptOutTestCase,
    TruncatedHookTestCase,
    WindowsEnvironmentTestCase,
)
from tests.integration.src.sqlbuild.compiler.project_assembly.helpers import (
    EDGE_ASSEMBLY_PROJECTS,
    assemble_with,
    assembly_outcome,
    generated_assembly_files,
    project_inputs,
    python_resource_calls,
    recorded_assembly,
    rejected_opt_outs,
    seed_yml,
    windows_environ,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_PROJECT_TOML: str = (
    'name = "assembly"\nadapter = "duckdb"\n\n[connection]\ndatabase = ":memory:"\n\n'
)
_ORDERS_MODEL: str = 'MODEL (description "Orders");\n\nSELECT 1 AS n\n'
_REQUIRED_ANALYSIS_TOML: str = (
    'name = "assembly"\nadapter = "duckdb"\n\n'
    '[connection]\ndatabase = ":memory:"\n\n[settings]\nrequire_sql_analysis = true\n'
)
_OPT_OUT_HEADER: str = 'MODEL (\n  description "Orders",\n  sql_analysis false,\n'
_SEED_FILES: dict[str, str] = {
    "seeds/countries.csv": "n\n1\n",
    "models/orders.sql": _ORDERS_MODEL,
}


@pytest.mark.parametrize(
    "test_case",
    [
        GeneratedAssemblyParityTestCase(
            description="managed sources, templated seeds and targets, hooks, audits, a function",
            seed=20261009,
            count=8,
            model_count=12,
            environment={"SQB_ASSEMBLY_LOADERS": "loaders"},
            expected_minimum_environment_reads=4,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_projects_when_assembling_natively_then_matches_python_golden(
    test_case: GeneratedAssemblyParityTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _ = [monkeypatch.setenv(name, value) for name, value in test_case.environment.items()]
    calls: Counter[str] = python_resource_calls(monkeypatch=monkeypatch)
    rng: random.Random = random.Random(test_case.seed)
    project_dirs: list[Path] = [tmp_path / f"project_{index}" for index in range(test_case.count)]
    inputs: list[CompileInputsView] = [
        project_inputs(
            project_dir=project_dir,
            files=generated_assembly_files(rng=rng, model_count=test_case.model_count),
        )
        for project_dir in project_dirs
    ]
    native: list[tuple[object, tuple[tuple[str, ...], bool]]] = [
        recorded_assembly(inputs=item, monkeypatch=monkeypatch) for item in inputs
    ]
    golden: list[GoldenEntry] = [
        golden_entry("project_assembly", view, masked=(str(project_dir),))
        for project_dir, view in zip(project_dirs, native, strict=True)
    ]

    assert (
        golden_differences(
            read_golden(golden_name("project_assembly", test_case.description)), golden
        )
        == []
    )
    assert sum(len(reads[0]) for _, reads in native) >= test_case.expected_minimum_environment_reads
    assert calls == Counter()


@pytest.mark.parametrize(
    "test_case",
    [
        EdgeAssemblyTestCase(
            description="error and python value edges",
            projects=EDGE_ASSEMBLY_PROJECTS,
            expected_raised=22,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_projects_python_rejects_when_assembling_natively_then_matches_python_golden(
    test_case: EdgeAssemblyTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("SQB_EDGE_UNSET", raising=False)
    project_dirs: list[Path] = [
        tmp_path / f"edge_{index}" for index in range(len(test_case.projects))
    ]
    outcomes: list[tuple[object, ...]] = [
        assembly_outcome(
            project_dir=project_dir,
            files=files,
            effective_vars=effective_vars,
            monkeypatch=monkeypatch,
        )
        for project_dir, (_, files, effective_vars) in zip(
            project_dirs, test_case.projects, strict=True
        )
    ]
    golden: list[GoldenEntry] = [
        golden_entry("project_assembly", outcome, masked=(str(project_dir),))
        for project_dir, outcome in zip(project_dirs, outcomes, strict=True)
    ]

    assert (
        golden_differences(
            read_golden(golden_name("project_assembly", test_case.description)), golden
        )
        == []
    )
    assert sum(outcome[0] != "ok" for outcome in outcomes) == test_case.expected_raised


@pytest.mark.parametrize(
    "test_case",
    [
        AssemblyDeferralTestCase(
            description="model SQL the parser rejects",
            files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": 'MODEL (description "Orders");\n\nSELECT FROM WHERE (\n',
            },
            expected_warning=(
                "the duckdb parser cannot read Syntax error at or near 'WHERE' at line 3, column 13"
            ),
        ),
        AssemblyDeferralTestCase(
            description="hook SQL the parser rejects",
            files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": (
                    'MODEL (\n  description "Orders",\n  pre_hooks [inline_sql("SELECT FROM (")],\n'
                    ");\n\nSELECT 1 AS n\n"
                ),
            },
            expected_warning=(
                "the duckdb parser cannot read Syntax error at or near end of input"
                " at line 1, column 14 of its SQL"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_model_sql_the_parser_rejects_when_assembling_then_skips_its_analysis(
    test_case: AssemblyDeferralTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    inputs: CompileInputsView = project_inputs(project_dir=tmp_path, files=test_case.files)

    project: CompiledProjectView = assemble_with(inputs=inputs, monkeypatch=monkeypatch)

    assert [
        (item.code, test_case.expected_warning in item.message) for item in project.diagnostics
    ] == [("P026", True)]


@pytest.mark.parametrize(
    "test_case",
    [
        WindowsEnvironmentTestCase(
            description="a lower-case ENV name Windows resolves to an upper-case variable",
            environment={"APP_SCHEMA": "landing"},
            seed_schema="${coalesce(ENV:app_schema, 'main')}",
            expected_schema="landing",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_windows_environment_when_assembling_natively_then_env_lookups_match_python(
    test_case: WindowsEnvironmentTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(os, "environ", windows_environ(test_case.environment))
    inputs: CompileInputsView = project_inputs(
        project_dir=tmp_path,
        files={
            "sqlbuild_project.toml": _PROJECT_TOML,
            "seeds/lookups.yml": seed_yml(test_case.seed_schema),
            **_SEED_FILES,
        },
    )

    native: CompiledProjectView = assemble_with(inputs=inputs, monkeypatch=monkeypatch)

    assert native.seeds[0].destination.schema == test_case.expected_schema


@pytest.mark.parametrize(
    "test_case",
    [
        OptOutTestCase(
            description="parseable, unparseable and unparseable-hook opt-outs",
            models={
                "models/parses.sql": f"{_OPT_OUT_HEADER});\n\nSELECT 1 AS n\n",
                "models/broken_query.sql": f"{_OPT_OUT_HEADER});\n\nSELECT FROM WHERE (\n",
                "models/broken_hook.sql": (
                    f'{_OPT_OUT_HEADER}  post_hooks [inline_sql("SELECT FROM (")],\n);\n\n'
                    "SELECT 1 AS n\n"
                ),
            },
            expected_rejected={"broken_hook": False, "broken_query": False, "parses": True},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_sql_analysis_opt_outs_when_attaching_natively_then_python_rejections_match(
    test_case: OptOutTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    files: dict[str, str] = {"sqlbuild_project.toml": _REQUIRED_ANALYSIS_TOML, **test_case.models}

    native: dict[str, bool] = rejected_opt_outs(
        project_dir=tmp_path / "native",
        files=files,
        monkeypatch=monkeypatch,
    )

    assert native == test_case.expected_rejected


@pytest.mark.parametrize(
    "test_case",
    [
        TruncatedHookTestCase(
            description="a T-SQL hook the parser once panicked on",
            files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "models/orders.sql": (
                    'MODEL (\n  description "Orders",\n'
                    '  pre_hooks [inline_sql("SELECT IF(region > 1, re")],\n);\n\nSELECT 1 AS n\n'
                ),
            },
            dialect="tsql",
            expected_message_fragment="the tsql parser cannot read Incorrect syntax near 'IF'.",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_truncated_hook_sql_when_assembling_then_reports_it_without_python_retry(
    test_case: TruncatedHookTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    inputs: CompileInputsView = project_inputs(
        project_dir=tmp_path / "project",
        files=test_case.files,
        sql_analysis_dialect=test_case.dialect,
    )
    calls: Counter[str] = python_resource_calls(monkeypatch=monkeypatch)

    project: CompiledProjectView = assemble_project(
        inputs=inputs,
        inference_profile=ExpressionInferenceProfile(sql_analysis_dialect=test_case.dialect),
    )

    assert [
        (item.code, test_case.expected_message_fragment in item.message)
        for item in project.diagnostics
    ] == [("P026", True)]
    assert calls == Counter()
