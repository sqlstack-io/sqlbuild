"""Python nodes depending on a job or a tag get edges to its end nodes through the real pipeline."""

from __future__ import annotations

import json
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

from sqlbuild._native import (
    CompilePipelineOptions,
    CompilePipelineResult,
    DiscoveredProjectInputs,
    PlannerInputError,
    discover_project_inputs,
    run_cli,
    run_compile_pipeline,
)
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from sqlbuild.runtime.contracts.models import ConnectionHooks
from tests.integration.src.sqlbuild.compiler.pipeline._test_types import (
    SetDependencyErrorTestCase,
    SetDependencyPlanTestCase,
    WaitChangeTestCase,
)
from tests.integration.src.sqlbuild.compiler.pipeline.helpers import (
    python_sql_deps,
    set_dependency_project,
)

_GROUP_FILES: tuple[tuple[str, str], ...] = (
    (
        "models/customer_orders.sql",
        "MODEL (description 'Test model customer_orders.', materialized table);\n"
        'SELECT o.id FROM __ref("orders") AS o JOIN __ref("customers") AS c ON o.id = c.id\n',
    ),
    (
        "orchestration/jobs/wide_job.toml",
        'name = "wide_job"\ndescription = "Wide."\nselect = "orders customer_orders"\n',
    ),
    (
        "orchestration/jobs/reporting_job.toml",
        'name = "reporting_job"\ndescription = "Reporting."\nselect = "task:report customers"\n',
    ),
    (
        "orchestration/job_groups/daily_job_group.toml",
        'name = "daily_job_group"\ndescription = "Daily."\njobs = ["wide_job", "reporting_job"]\n',
    ),
)


@pytest.mark.parametrize(
    "test_case",
    [
        SetDependencyPlanTestCase(
            description="a job dependency reads the job's end nodes",
            depends_on='job("nightly_job")',
            expected_models=("order_items", "orders", "stg_orders"),
            expected_sql_deps=("model:order_items", "model:orders"),
        ),
        SetDependencyPlanTestCase(
            description="exclude removes nodes before reducing to end nodes",
            depends_on='job("nightly_job", exclude="order_items")',
            expected_models=("orders", "stg_orders"),
            expected_sql_deps=("model:orders",),
        ),
        SetDependencyPlanTestCase(
            description="a tag dependency reads its tagged end nodes",
            depends_on='tag("finance", exclude="orders")',
            expected_models=("order_items", "stg_orders"),
            expected_sql_deps=("model:order_items",),
        ),
        SetDependencyPlanTestCase(
            description="a model named and reached through a job is read once",
            depends_on='(model("orders"), job("nightly_job"))',
            expected_models=("order_items", "orders", "stg_orders"),
            expected_sql_deps=("model:orders", "model:order_items"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_set_dependency_when_planning_the_tasks_upstream_then_selects_the_set(
    test_case: SetDependencyPlanTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, set_dependency_project(depends_on=test_case.depends_on))
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    result: CompilePipelineResult = run_compile_pipeline(
        discovered_inputs=discovered,
        adapter=DuckDbAdapter(),
        options=CompilePipelineOptions(
            no_sql_validation=True, select=("+task:report",), resolve_python_run_selectors=True
        ),
        hooks=ConnectionHooks(),
    )

    assert (
        tuple(sorted(entry.name for entry in result.plan_output.model_entries))
        == test_case.expected_models
    )
    assert python_sql_deps(discovered, "report") == test_case.expected_sql_deps


@pytest.mark.parametrize(
    "test_case",
    [
        SetDependencyErrorTestCase(
            description="an unknown job",
            depends_on='job("weekly_job")',
            job_files=(),
            expected_fragments=(
                "task 'report' depends on job 'weekly_job', which the project does not declare",
            ),
        ),
        SetDependencyErrorTestCase(
            description="an unknown tag",
            depends_on='tag("marketing")',
            job_files=(),
            expected_fragments=("task 'report' depends on tag 'marketing', which no node carries",),
        ),
        SetDependencyErrorTestCase(
            description="a job its exclude empties",
            depends_on='job("nightly_job", exclude="stg_orders orders order_items")',
            job_files=(),
            expected_fragments=("which leaves no model, seed, source, loader, task or asset",),
        ),
        SetDependencyErrorTestCase(
            description="job group members that would wait for each other",
            depends_on='job("wide_job")',
            job_files=_GROUP_FILES,
            expected_fragments=(
                "a job dependency makes job group 'daily_job_group' wait in a loop",
                "'wide_job''s model 'customer_orders' reads model 'customers'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_unresolvable_set_dependency_when_planning_then_refuses_naming_the_cause(
    test_case: SetDependencyErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(
        tmp_path,
        set_dependency_project(depends_on=test_case.depends_on, job_files=test_case.job_files),
    )

    with pytest.raises(PlannerInputError) as raised:
        _ = run_compile_pipeline(
            discovered_inputs=discover_project_inputs(project_dir=tmp_path),
            adapter=DuckDbAdapter(),
            options=CompilePipelineOptions(
                no_sql_validation=True, resolve_python_run_selectors=True
            ),
            hooks=ConnectionHooks(),
        )

    text: str = f"{raised.value} {getattr(raised.value, 'help', '')}"
    assert all(fragment in text for fragment in test_case.expected_fragments), text


@pytest.mark.parametrize(
    "test_case",
    [
        WaitChangeTestCase(
            description="a job gaining an end node",
            edited_select="stg_orders orders order_items customers",
            expected_changes=(
                "report now also waits on model 'customers' through job 'nightly_job'",
            ),
        ),
        WaitChangeTestCase(
            description="a job losing an end node",
            edited_select="stg_orders orders",
            expected_changes=(
                "report no longer waits on model 'order_items' through job 'nightly_job'",
            ),
        ),
        WaitChangeTestCase(
            description="an unchanged job",
            edited_select="stg_orders orders order_items",
            expected_changes=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_built_job_dependency_when_the_job_changes_then_the_plan_lists_wait_changes(
    test_case: WaitChangeTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
    capsys: pytest.CaptureFixture[str],
) -> None:
    files: dict[str, str] = set_dependency_project(depends_on='job("nightly_job")')
    files["sqlbuild_project.toml"] = files["sqlbuild_project.toml"].replace(
        ":memory:", str(tmp_path / "warehouse.duckdb")
    )
    write_repo_files(tmp_path, files)
    assert run_cli(["--project-dir", str(tmp_path), "--no-color", "build"]) == 0
    job: Path = tmp_path / "orchestration/jobs/nightly_job.toml"
    _ = job.write_text(
        job.read_text().replace("stg_orders orders order_items", test_case.edited_select)
    )
    _ = capsys.readouterr()

    assert run_cli(["--project-dir", str(tmp_path), "--no-color", "plan", "--json"]) == 0

    document: dict[str, Any] = json.loads(capsys.readouterr().out)
    nodes: dict[str, dict[str, Any]] = {node["name"]: node for node in document["python_nodes"]}
    assert tuple(nodes["report"].get("wait_changes", ())) == test_case.expected_changes
