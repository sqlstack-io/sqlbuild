from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.plan._test_types import (
    RemovedConfigTestCase,
    RemovedHookTestCase,
    RemovedInterfaceTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb


@pytest.mark.parametrize(
    "test_case",
    [
        RemovedInterfaceTestCase(
            "clone lock flag removed",
            ("clone", "--from", "local", "--skip-locked"),
            "unrecognized arguments: --skip-locked",
        ),
        RemovedInterfaceTestCase("promote removed", ("promote",), "invalid choice: 'promote'"),
        RemovedInterfaceTestCase("rollback removed", ("rollback",), "invalid choice: 'rollback'"),
        RemovedInterfaceTestCase(
            "reconcile removed", ("reconcile",), "invalid choice: 'reconcile'"
        ),
        RemovedInterfaceTestCase("state removed", ("state",), "invalid choice: 'state'"),
        RemovedInterfaceTestCase(
            "plan environment removed",
            ("plan", "--virtual-env"),
            "unrecognized arguments: --virtual-env",
        ),
        RemovedInterfaceTestCase(
            "plan stale upstreams removed",
            ("plan", "--include-stale-upstreams"),
            "unrecognized arguments: --include-stale-upstreams",
        ),
        RemovedInterfaceTestCase(
            "plan changes only removed",
            ("plan", "--changes-only"),
            "unrecognized arguments: --changes-only",
        ),
        RemovedInterfaceTestCase(
            "build environment removed",
            ("build", "--virtual-env"),
            "unrecognized arguments: --virtual-env",
        ),
        RemovedInterfaceTestCase(
            "build stale upstreams removed",
            ("build", "--include-stale-upstreams"),
            "unrecognized arguments: --include-stale-upstreams",
        ),
        RemovedInterfaceTestCase(
            "build changes only removed",
            ("build", "--changes-only"),
            "unrecognized arguments: --changes-only",
        ),
        RemovedInterfaceTestCase(
            "partial diff removed",
            ("diff", "--allow-partial-diff"),
            "unrecognized arguments: --allow-partial-diff",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_removed_interface_when_running_cli_then_reports_unknown_argument(
    test_case: RemovedInterfaceTestCase, tmp_path: Path
) -> None:
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", *test_case.command), project_dir=tmp_path
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_error in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        RemovedConfigTestCase(
            "enabled virtual setting",
            "[settings]\nvirtual_environments = true\n",
            "virtual_environments",
        ),
        RemovedConfigTestCase(
            "removed unchanged default",
            '[defaults]\nrun_despite_unchanged = "always"\n',
            "run_despite_unchanged",
        ),
        RemovedConfigTestCase(
            "removed unchanged path default",
            '[path_defaults.staging]\nrun_despite_unchanged = "30d"\n',
            "run_despite_unchanged",
        ),
        RemovedConfigTestCase(
            "removed unchanged MODEL field",
            "MODEL (description 'Test model.', materialized table, run_despite_unchanged always); SELECT 1 AS order_id",
            "run_despite_unchanged",
            "sql/models/orders.sql",
            "",
        ),
        RemovedConfigTestCase(
            "disabled virtual setting",
            "[settings]\nvirtual_environments = false\n",
            "virtual_environments",
        ),
        RemovedConfigTestCase(
            "changes only setting", "[settings]\nchanges_only = false\n", "changes_only"
        ),
        RemovedConfigTestCase(
            "target changes only", "[targets.dev]\nchanges_only = false\n", "changes_only"
        ),
        RemovedConfigTestCase("target state", "[targets.dev.state]\nbackend = 'duckdb'\n", "state"),
        RemovedConfigTestCase(
            "checkpoint retention", "[janitor]\nmax_checkpoints = 20\n", "max_checkpoints"
        ),
        RemovedConfigTestCase(
            "local enabled virtual setting",
            "[settings]\nvirtual_environments = true\n",
            "virtual_environments",
            "sqlbuild_local.toml",
            "",
        ),
        RemovedConfigTestCase(
            "local disabled virtual setting",
            "[settings]\nvirtual_environments = false\n",
            "virtual_environments",
            "sqlbuild_local.toml",
            "",
        ),
        RemovedConfigTestCase(
            "local changes only setting",
            "[settings]\nchanges_only = false\n",
            "changes_only",
            "sqlbuild_local.toml",
            "",
        ),
        RemovedConfigTestCase(
            "local target changes only",
            "[targets.dev]\nchanges_only = false\n",
            "changes_only",
            "sqlbuild_local.toml",
            "",
        ),
        RemovedConfigTestCase(
            "local target state",
            "[targets.dev.state]\nbackend = 'duckdb'\n",
            "state",
            "sqlbuild_local.toml",
            "",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_removed_config_when_running_cli_then_explains_direct_mode(
    test_case: RemovedConfigTestCase, tmp_path: Path
) -> None:
    files: dict[str, str] = {
        "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
        "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized view);\nSELECT 1 AS order_id",
    }
    files[test_case.filename] = test_case.prefix + test_case.content
    project: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="removed_config", repo_files=files
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan"), project_dir=project
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_key in result.stdout + result.stderr
    assert (
        "were removed with virtual environments; projects run in direct mode"
        in result.stdout + result.stderr
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RemovedHookTestCase(
            "removed materialization hook",
            "'prepare_version', which was removed with virtual environments",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_prepare_version_hook_when_running_cli_then_rejects_removed_hook(
    test_case: RemovedHookTestCase, tmp_path: Path
) -> None:
    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="removed_hook",
        repo_files={
            "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n',
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized custom_order);\nSELECT 1 AS order_id",
            "python/materializations/custom_order.py": (
                "from sqlbuild.executor.custom.models import PrepareVersionContext\n\n"
                "def prepare_version(ctx: PrepareVersionContext):\n    pass\n\n"
                "def materialize(ctx):\n    pass\n"
            ),
        },
    )
    result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build"), project_dir=project
    )
    assert result.returncode == test_case.expected_exit_code
    assert test_case.expected_error in result.stdout + result.stderr


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
