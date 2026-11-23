"""E2E tests for provider command wiring."""

from __future__ import annotations

import json
import subprocess
from collections.abc import Sequence
from pathlib import Path
from textwrap import dedent
from typing import cast

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.providers._test_types import (
    ProviderCommandConcurrencyE2ETestCase,
    ProviderCommandDiagnosticE2ETestCase,
    ProviderCommandE2ETestCase,
    ProviderCommandFailureE2ETestCase,
    ProviderCommandSideEffectE2ETestCase,
    ProviderCustomMaterializationE2ETestCase,
    ProviderHookContextConflictE2ETestCase,
    ProviderHookDiagnosticE2ETestCase,
    ProviderHookE2ETestCase,
    ProviderHookMaterializationE2ETestCase,
    ProviderPlanOutputE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    prepare_inline_project,
    query_duckdb,
    run_sqb,
)

PROVIDER_FAILURE_PROJECT_TOML: str = (
    dedent(
        """
    name = "provider_failure_project"
    adapter = "duckdb"

    [connection]
    database = "provider_failure_project.duckdb"
    """
    ).strip()
    + "\n"
)

PROVIDER_MARKER_FILE: str = (
    dedent(
        """
    from pathlib import Path

    from pydantic import Field
    from sqlbuild.providers import Provider


    class MarkerProvider(Provider):
        '''Test provider MarkerProvider.'''
        marker_path: str = Field(validation_alias="MARKER_PATH")

        def setup(self, ctx):
            self.mark("setup")

        def teardown(self):
            self.mark("teardown")

        def mark(self, value):
            path = Path(self.marker_path)
            existing = path.read_text(encoding="utf-8") if path.exists() else ""
            path.write_text(existing + value + "\\n", encoding="utf-8")
    """
    ).strip()
    + "\n"
)


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderPlanOutputE2ETestCase(
            description="plan output shows provider usage for selected Python surfaces",
            expected_text_fragments=(
                "Providers",
                "\u2514\u2500\u2500 marker_provider",
                "    ├── asset publish_orders (MarkerProvider)",
                "    ├── custom materialization copy_table (MarkerProvider)",
                "    ├── hook mark_pre (MarkerProvider)",
                "    └── loader raw_orders (MarkerProvider)",
            ),
            expected_provider_name="marker_provider",
            expected_used_by=(
                ("asset", "publish_orders", "marker_provider"),
                ("custom materialization", "copy_table", "marker_provider"),
                ("hook", "mark_pre", "marker_provider"),
                ("loader", "raw_orders", "marker_provider"),
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_usages_when_planning_then_text_and_json_include_selected_surfaces(
    test_case: ProviderPlanOutputE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_plan_output_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "provider_plan_output_project"
                adapter = "duckdb"
                default_target = "dev"

                [connection]
                database = "provider_plan_output_project.duckdb"

                [targets.dev]
                schema = "main"
                defer_sources_to = "dev"
                """
            ).strip()
            + "\n",
            "python/providers/marker.py": dedent(
                """
                from sqlbuild.providers import Provider


                class MarkerProvider(Provider):
                    '''Test provider MarkerProvider.'''
                    label: str = "plan-output"
                """
            ).strip()
            + "\n",
            "python/nodes/loaders/raw_orders.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.loaders import loader


                @loader
                def raw_orders(marker_provider: MarkerProvider):
                    '''Test loader raw_orders.'''
                    return [{"order_id": 1, "amount": 10}]
                """
            ).strip()
            + "\n",
            "python/nodes/tasks/publish_orders.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.assets import asset


                @asset
                def publish_orders(marker_provider: MarkerProvider):
                    '''Test task publish_orders.'''
                    marker_provider.label
                """
            ).strip()
            + "\n",
            "sql/hooks/python/mark_pre.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(marker_provider: MarkerProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.label
                """
            ).strip()
            + "\n",
            "python/materializations/copy_table.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.executor.custom.models import (
                    MaterializationContext,
                    MaterializationResult,
                )


                def materialize(
                    ctx: MaterializationContext,
                    marker_provider: MarkerProvider,
                ) -> MaterializationResult:
                    marker_provider.label
                    ctx.execute_sql(f"CREATE TABLE {ctx.destination} AS {ctx.sql}")
                    return MaterializationResult(relation=ctx.destination)
                """
            ).strip()
            + "\n",
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_orders
                    description: Test source raw_orders.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: order_id
                        type: INTEGER
                      - name: amount
                        type: INTEGER
                """
            ).strip()
            + "\n",
            "sql/models/stg_orders.sql": dedent(
                """
                MODEL (description "Test model stg_orders.",
                  materialized copy_table,
                  pre_hooks [python("mark_pre")]
                );

                SELECT order_id, amount
                FROM __source("raw_orders")
                """
            ).strip()
            + "\n",
        },
    )

    text_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--verbose"),
        project_dir=project_dir,
    )
    assert text_result.returncode == 0, text_result.stdout + text_result.stderr
    expected_text_fragment: str
    for expected_text_fragment in test_case.expected_text_fragments:
        assert expected_text_fragment in text_result.stdout

    json_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "plan", "--json"),
        project_dir=project_dir,
    )
    assert json_result.returncode == 0, json_result.stdout + json_result.stderr
    payload: dict[str, object] = json.loads(json_result.stdout)
    providers_obj: object = payload["providers"]
    assert isinstance(providers_obj, list)
    providers: list[object] = providers_obj
    assert len(providers) == 1
    provider: dict[str, object] = cast(dict[str, object], providers[0])
    assert provider["name"] == test_case.expected_provider_name
    used_by_obj: object = provider["used_by"]
    assert isinstance(used_by_obj, list)
    used_by: Sequence[object] = used_by_obj
    actual_used_by: tuple[tuple[str, str, str], ...] = tuple(
        (
            str(cast(dict[str, object], entry)["kind"]),
            str(cast(dict[str, object], entry)["name"]),
            str(cast(dict[str, object], entry)["parameter"]),
        )
        for entry in used_by
    )
    assert actual_used_by == test_case.expected_used_by


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderCommandE2ETestCase(
            description="providers are available in run build check and load commands",
            expected_exit_code=0,
            expected_marker_entries=(
                "setup",
                "run_task:provider-e2e",
                "teardown",
                "setup",
                "build_asset:provider-e2e",
                "teardown",
                "setup",
                "check:provider-e2e",
                "teardown",
                "setup",
                "load:provider-e2e",
                "teardown",
            ),
            expected_loaded_rows=((1, "provider-e2e"),),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_with_provider_when_running_commands_then_provider_is_injected(
    test_case: ProviderCommandE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_command_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "provider_command_project"
                adapter = "duckdb"

                [connection]
                database = "provider_command_project.duckdb"
                """
            ).strip()
            + "\n",
            "python/providers/marker.py": dedent(
                """
                from pathlib import Path

                from pydantic import Field
                from sqlbuild.providers import Provider


                class MarkerProvider(Provider):
                    '''Test provider MarkerProvider.'''
                    marker_path: str = Field(validation_alias="MARKER_PATH")
                    label: str = "provider-e2e"

                    def setup(self, ctx):
                        self.mark("setup")

                    def teardown(self):
                        self.mark("teardown")

                    def mark(self, value):
                        path = Path(self.marker_path)
                        existing = path.read_text(encoding="utf-8") if path.exists() else ""
                        path.write_text(existing + value + "\\n", encoding="utf-8")
                """
            ).strip()
            + "\n",
            "python/nodes/tasks/provider_task.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.assets import asset


                @asset
                def provider_task(ctx, marker_provider: MarkerProvider):
                    '''Test task provider_task.'''
                    marker_provider.mark(f"run_task:{marker_provider.label}")
                """
            ).strip()
            + "\n",
            "python/nodes/assets/provider_asset.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.assets import asset


                @asset
                def provider_asset(ctx, marker_provider: MarkerProvider):
                    '''Test asset provider_asset.'''
                    marker_provider.mark(f"build_asset:{marker_provider.label}")
                """
            ).strip()
            + "\n",
            "python/nodes/checks/provider_check.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.checks import check


                @check(depends_on=())
                def provider_check(ctx, marker_provider: MarkerProvider):
                    '''Test check provider_check.'''
                    marker_provider.mark(f"check:{marker_provider.label}")
                    return True
                """
            ).strip()
            + "\n",
            "python/nodes/loaders/provider_loader.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.loaders import loader


                @loader
                def raw_provider_events(ctx, marker_provider: MarkerProvider):
                    '''Test loader raw_provider_events.'''
                    marker_provider.mark(f"load:{marker_provider.label}")
                    return [{"event_id": 1, "label": marker_provider.label}]
                """
            ).strip()
            + "\n",
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_provider_events
                    description: Test source raw_provider_events.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: event_id
                        type: INTEGER
                      - name: label
                        type: VARCHAR
                """
            ).strip()
            + "\n",
        },
    )
    env: dict[str, str] = {"MARKER_PATH": str(marker_path)}

    run_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "provider_task"),
        project_dir=project_dir,
        env=env,
    )
    build_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "build", "--select", "provider_asset"),
        project_dir=project_dir,
        env=env,
    )
    check_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "check", "--select", "provider_check"),
        project_dir=project_dir,
        env=env,
    )
    load_result: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "load", "--select", "raw_provider_events"),
        project_dir=project_dir,
        env=env,
    )

    for result in (run_result, build_result, check_result, load_result):
        assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr

    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries
    loaded_rows: list[tuple[object, ...]] = query_duckdb(
        db_path=project_dir / "provider_command_project.duckdb",
        sql="SELECT event_id, label FROM raw_provider_events ORDER BY event_id",
    )
    assert tuple(loaded_rows) == test_case.expected_loaded_rows


@pytest.mark.parametrize(
    "test_case",
    (
        ProviderCommandFailureE2ETestCase(
            description="run provider tears down when command fails after setup",
            command=(
                "--no-color",
                "build",
                "--no-tests",
                "--no-audits",
                "--select",
                "failing_task",
            ),
            repo_files={
                "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
                "python/providers/marker.py": PROVIDER_MARKER_FILE,
                "python/nodes/tasks/failing_task.py": dedent(
                    """
                    from python.providers.marker import MarkerProvider
                    from sqlbuild.assets import asset


                    @asset
                    def failing_task(ctx, marker_provider: MarkerProvider):
                        '''Test task failing_task.'''
                        marker_provider.mark("task")
                        raise RuntimeError("intentional provider failure")
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=("setup", "task", "teardown"),
        ),
        ProviderCommandFailureE2ETestCase(
            description="build provider tears down when command fails after setup",
            command=("--no-color", "build", "--select", "failing_asset"),
            repo_files={
                "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
                "python/providers/marker.py": PROVIDER_MARKER_FILE,
                "python/nodes/assets/failing_asset.py": dedent(
                    """
                    from python.providers.marker import MarkerProvider
                    from sqlbuild.assets import asset


                    @asset
                    def failing_asset(ctx, marker_provider: MarkerProvider):
                        '''Test asset failing_asset.'''
                        marker_provider.mark("asset")
                        raise RuntimeError("intentional provider failure")
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=("setup", "asset", "teardown"),
        ),
        ProviderCommandFailureE2ETestCase(
            description="check provider tears down when command fails after setup",
            command=("--no-color", "check", "--select", "failing_check"),
            repo_files={
                "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
                "python/providers/marker.py": PROVIDER_MARKER_FILE,
                "python/nodes/checks/failing_check.py": dedent(
                    """
                    from python.providers.marker import MarkerProvider
                    from sqlbuild.checks import check


                    @check(depends_on=())
                    def failing_check(ctx, marker_provider: MarkerProvider):
                        '''Test check failing_check.'''
                        marker_provider.mark("check")
                        raise RuntimeError("intentional provider failure")
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=("setup", "check", "teardown"),
        ),
        ProviderCommandFailureE2ETestCase(
            description="load provider tears down when command fails after setup",
            command=("--no-color", "load", "--select", "raw_provider_events"),
            repo_files={
                "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
                "python/providers/marker.py": PROVIDER_MARKER_FILE,
                "python/nodes/loaders/failing_loader.py": dedent(
                    """
                    from python.providers.marker import MarkerProvider
                    from sqlbuild.loaders import loader


                    @loader
                    def raw_provider_events(ctx, marker_provider: MarkerProvider):
                        '''Test loader raw_provider_events.'''
                        marker_provider.mark("load")
                        raise RuntimeError("intentional provider failure")
                    """
                ).strip()
                + "\n",
                "sql/sources/raw.yml": dedent(
                    """
                    sources:
                      - name: raw_provider_events
                        description: Test source raw_provider_events.
                        managed: true
                        write_strategy: table
                        columns:
                          - name: event_id
                            type: INTEGER
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=("setup", "load", "teardown"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_backed_command_failure_when_provider_was_setup_then_provider_tears_down(
    test_case: ProviderCommandFailureE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-failure-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_failure_project",
        repo_files=test_case.repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    (
        ProviderCommandSideEffectE2ETestCase(
            description="compile discovers typed providers without setup side effects",
            command=("--no-color", "compile"),
            expected_marker_exists=False,
        ),
        ProviderCommandSideEffectE2ETestCase(
            description="plan discovers typed providers without setup side effects",
            command=("--no-color", "plan"),
            expected_marker_exists=False,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_provider_project_when_running_compile_or_plan_then_provider_setup_is_not_called(
    test_case: ProviderCommandSideEffectE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-side-effect-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_side_effect_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "python/nodes/tasks/provider_task.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.assets import asset


                @asset
                def provider_task(ctx, marker_provider: MarkerProvider):
                    '''Test task provider_task.'''
                    marker_provider.mark("task")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": "MODEL (description 'Test model orders.');\n\nSELECT 1 AS id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == 0, result.stdout + result.stderr
    assert marker_path.exists() is test_case.expected_marker_exists


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderCommandDiagnosticE2ETestCase(
            description="alias-imported provider annotation gets CLI diagnostic",
            command=("--no-color", "build", "--no-tests", "--no-audits", "--select", "alias_task"),
            expected_error_fragment=(
                "Import project providers using the project-root providers package path"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_alias_imported_provider_annotation_when_running_command_then_cli_prints_guidance(
    test_case: ProviderCommandDiagnosticE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-alias-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_alias_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "python/nodes/tasks/alias_task.py": dedent(
                """
                import importlib.util
                import sys
                from pathlib import Path

                from sqlbuild.assets import asset

                provider_path = Path(__file__).parents[2] / "python" / "nodes" / "python" / "providers" / "marker.py"
                spec = importlib.util.spec_from_file_location("alias_marker", provider_path)
                alias_marker = importlib.util.module_from_spec(spec)
                sys.modules["alias_marker"] = alias_marker
                assert spec.loader is not None
                spec.loader.exec_module(alias_marker)
                AliasMarkerProvider = alias_marker.MarkerProvider


                @asset
                def alias_task(ctx, marker_provider: AliasMarkerProvider):
                    '''Test task alias_task.'''
                    marker_provider.mark("task")
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderCustomMaterializationE2ETestCase(
            description="custom materialization gets providers and tears down after failure",
            command=("--no-color", "build", "--select", "orders"),
            expected_marker_entries=("setup", "ctx", "injected", "teardown"),
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_custom_materialization_with_provider_when_it_fails_then_provider_tears_down(
    test_case: ProviderCustomMaterializationE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-custom-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_custom_materialization_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "python/materializations/marker_mat.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.executor.custom.models import MaterializationContext


                def materialize(
                    ctx: MaterializationContext,
                    marker_provider: MarkerProvider,
                ):
                    ctx.providers["marker_provider"].mark("ctx")
                    marker_provider.mark("injected")
                    raise RuntimeError("intentional custom provider failure")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized marker_mat);\n\nSELECT 1 AS id\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderCommandConcurrencyE2ETestCase(
            description="concurrent provider-backed source loaders share command provider session",
            command=("--no-color", "build", "--concurrency", "2"),
            expected_marker_entries=("setup", "alpha", "beta", "teardown"),
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_concurrent_provider_backed_nodes_when_running_command_then_share_provider_session(
    test_case: ProviderCommandConcurrencyE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-command-concurrent-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_command_concurrent_project",
        repo_files={
            "sqlbuild_project.toml": dedent(
                """
                name = "provider_command_concurrent_project"
                adapter = "duckdb"

                [connection]
                database = "provider_command_concurrent_project.duckdb"
                """
            ).strip()
            + "\n",
            "python/providers/concurrent_marker.py": dedent(
                f"""
                from pathlib import Path
                from time import sleep

                from sqlbuild.providers import Provider


                class ConcurrentMarkerProvider(Provider):
                    '''Test provider ConcurrentMarkerProvider.'''
                    marker_path: str = {str(marker_path)!r}

                    @property
                    def token(self):
                        return str(id(self))

                    def setup(self, ctx):
                        sleep(0.05)
                        self.mark("setup")

                    def teardown(self):
                        self.mark("teardown")

                    def mark(self, label):
                        with Path(self.marker_path).open("a", encoding="utf-8") as marker:
                            marker.write(f"{{label}}:{{self.token}}\\n")
                """
            ).strip()
            + "\n",
            "python/nodes/loaders/events.py": dedent(
                """
                from python.providers.concurrent_marker import ConcurrentMarkerProvider
                from sqlbuild.loaders import loader


                @loader
                def raw_alpha(ctx, concurrent_marker_provider: ConcurrentMarkerProvider):
                    '''Test loader raw_alpha.'''
                    concurrent_marker_provider.mark("alpha")
                    return [{"event_id": 1}]


                @loader
                def raw_beta(ctx, concurrent_marker_provider: ConcurrentMarkerProvider):
                    '''Test loader raw_beta.'''
                    concurrent_marker_provider.mark("beta")
                    return [{"event_id": 2}]
                """
            ).strip()
            + "\n",
            "sql/sources/raw.yml": dedent(
                """
                sources:
                  - name: raw_alpha
                    description: Test source raw_alpha.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: event_id
                        type: INTEGER
                  - name: raw_beta
                    description: Test source raw_beta.
                    managed: true
                    write_strategy: table
                    columns:
                      - name: event_id
                        type: INTEGER
                """
            ).strip()
            + "\n",
            "sql/models/fact_alpha.sql": (
                'MODEL (description "Test model fact_alpha.", materialized table);\n\nSELECT * FROM __source("raw_alpha")\n'
            ),
            "sql/models/fact_beta.sql": (
                'MODEL (description "Test model fact_beta.", materialized table);\n\nSELECT * FROM __source("raw_beta")\n'
            ),
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    marker_labels: tuple[str, ...] = tuple(
        entry.split(":", maxsplit=1)[0] for entry in marker_entries
    )
    marker_tokens: set[str] = {entry.split(":", maxsplit=1)[1] for entry in marker_entries}
    assert marker_labels[0] == test_case.expected_marker_entries[0]
    assert set(marker_labels[1:3]) == set(test_case.expected_marker_entries[1:3])
    assert marker_labels[3] == test_case.expected_marker_entries[3]
    assert len(marker_entries) == len(test_case.expected_marker_entries)
    assert len(marker_tokens) == 1


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookE2ETestCase(
            description="python hooks access providers through context and parameter injection",
            command=("--no-color", "build", "--select", "orders"),
            expected_marker_entries=(
                "setup",
                "pre_ctx",
                "pre_injected",
                "post_ctx",
                "post_injected",
                "teardown",
            ),
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hooks_with_provider_when_building_then_hooks_use_provider_session(
    test_case: ProviderHookE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider: MarkerProvider):
                    '''Test hook mark_pre.'''
                    ctx.providers.marker_provider.mark("pre_ctx")
                    marker_provider.mark("pre_injected")


                @hook
                def mark_post(hook_context, marker_provider: MarkerProvider):
                    '''Test hook mark_post.'''
                    hook_context.providers["marker_provider"].mark("post_ctx")
                    marker_provider.mark("post_injected")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre")],
                  post_hooks [python("mark_post")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookE2ETestCase(
            description="python hooks on view models use provider injection",
            command=("--no-color", "build", "--select", "orders_view"),
            expected_marker_entries=("setup", "view_pre", "view_post", "teardown"),
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_view_model_python_hooks_with_provider_when_building_then_hooks_use_provider(
    test_case: ProviderHookE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-view-hook-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_view_hook_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider: MarkerProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("view_pre")


                @hook
                def mark_post(ctx, marker_provider: MarkerProvider):
                    '''Test hook mark_post.'''
                    marker_provider.mark("view_post")
                """
            ).strip()
            + "\n",
            "sql/models/orders_view.sql": dedent(
                """
                MODEL (description "Test model orders_view.",
                  materialized view,
                  pre_hooks [python("mark_pre")],
                  post_hooks [python("mark_post")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookMaterializationE2ETestCase(
            description="incremental python hooks use provider injection",
            command=("--no-color", "build", "--select", "+incremental_orders"),
            model_relative_path="sql/models/incremental_orders.sql",
            model_sql=dedent(
                """
                MODEL (description "Test model incremental_orders.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor order_id,
                  cursor_type integer,
                  pre_hooks [python("mark_hook")],
                  post_hooks [python("mark_hook")]
                );

                SELECT order_id FROM __ref("order_rows")
                """
            ).strip()
            + "\n",
            extra_repo_files={
                "sql/models/order_rows.sql": 'MODEL (description "Test model order_rows.");\n\n'
                "SELECT 1 AS order_id\n"
            },
            expected_marker_entries=(
                "setup",
                "incremental_orders:pre_hooks",
                "incremental_orders:post_hooks",
                "teardown",
            ),
            expected_exit_code=0,
        ),
        ProviderHookMaterializationE2ETestCase(
            description="microbatch python hooks use provider injection",
            command=("--no-color", "build", "--select", "+hourly_activity"),
            model_relative_path="sql/models/hourly_activity.sql",
            model_sql=dedent(
                """
                MODEL (description "Test model hourly_activity.",
                  materialized incremental,
                  incremental_strategy delete_insert,
                  cursor activity_hour,
                  cursor_type timestamp,
                  cursor_grain hour,
                  cursor_inputs (
            fact_orders (column ordered_at, roles [filter, watermark]),
          ),
                  incremental_mode microbatch,
          microbatch_strategy watermark,
          cursor_watermark_mode all,
                  batch_size 1d,
                  pre_hooks [python("mark_hook")],
                  post_hooks [python("mark_hook")]
                );

                SELECT DATE_TRUNC('hour', ordered_at) AS activity_hour, COUNT(*) AS orders_placed
                FROM __ref("fact_orders")
                GROUP BY DATE_TRUNC('hour', ordered_at)
                """
            ).strip()
            + "\n",
            extra_repo_files={
                "sql/models/fact_orders.sql": dedent(
                    """
                    MODEL (description "Test model fact_orders.", materialized table);

                    SELECT 1 AS order_id, TIMESTAMP '2026-01-01 00:00:00' AS ordered_at
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=(
                "setup",
                "hourly_activity:pre_hooks",
                "hourly_activity:post_hooks",
                "teardown",
            ),
            expected_exit_code=0,
        ),
        ProviderHookMaterializationE2ETestCase(
            description="snapshot python hooks use provider injection",
            command=("--no-color", "build", "--select", "customer_snapshot"),
            model_relative_path="sql/models/customer_snapshot.sql",
            model_sql=dedent(
                """
                MODEL (description "Test model customer_snapshot.",
                  materialized snapshot,
                  unique_key [customer_id],
                  snapshot_strategy timestamp,
                  updated_at updated_at,
                  pre_hooks [python("mark_hook")],
                  post_hooks [python("mark_hook")]
                );

                SELECT 1 AS customer_id, 'basic' AS plan,
                  TIMESTAMP '2026-01-01 00:00:00' AS updated_at
                """
            ).strip()
            + "\n",
            extra_repo_files={},
            expected_marker_entries=(
                "setup",
                "customer_snapshot:pre_hooks",
                "customer_snapshot:post_hooks",
                "teardown",
            ),
            expected_exit_code=0,
        ),
        ProviderHookMaterializationE2ETestCase(
            description="custom materialization python hooks use provider injection",
            command=("--no-color", "build", "--select", "custom_orders"),
            model_relative_path="sql/models/custom_orders.sql",
            model_sql=dedent(
                """
                MODEL (description "Test model custom_orders.",
                  materialized copy_table,
                  pre_hooks [python("mark_hook")],
                  post_hooks [python("mark_hook")]
                );

                SELECT 1 AS order_id
                """
            ).strip()
            + "\n",
            extra_repo_files={
                "python/materializations/copy_table.py": dedent(
                    """
                    from sqlbuild.executor.custom.models import (
                        MaterializationContext,
                        MaterializationResult,
                    )


                    def materialize(ctx: MaterializationContext) -> MaterializationResult:
                        ctx.execute_sql(f"CREATE TABLE {ctx.destination} AS {ctx.sql}")
                        return MaterializationResult(relation=ctx.destination)
                    """
                ).strip()
                + "\n",
            },
            expected_marker_entries=(
                "setup",
                "custom_orders:pre_hooks",
                "custom_orders:post_hooks",
                "teardown",
            ),
            expected_exit_code=0,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_materialization_hooks_with_provider_when_building_then_hooks_use_provider(
    test_case: ProviderHookMaterializationE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-materialization-marker.log"
    repo_files: dict[str, str] = {
        "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
        "python/providers/marker.py": PROVIDER_MARKER_FILE,
        "sql/hooks/python/marker_hooks.py": dedent(
            """
            from python.providers.marker import MarkerProvider
            from sqlbuild.hooks import hook


            @hook
            def mark_hook(ctx, marker_provider: MarkerProvider):
                '''Test hook mark_hook.'''
                marker_provider.mark(f"{ctx.model_name}:{ctx.phase}")
            """
        ).strip()
        + "\n",
        test_case.model_relative_path: test_case.model_sql,
    }
    repo_files.update(test_case.extra_repo_files)
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_materialization_project",
        repo_files=repo_files,
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookE2ETestCase(
            description="untyped python hook parameter named like provider is injected by name",
            command=("--no-color", "build", "--select", "orders"),
            expected_marker_entries=("setup", "untyped", "teardown"),
            expected_exit_code=0,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_untyped_python_hook_provider_parameter_when_building_then_provider_is_injected(
    test_case: ProviderHookE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-untyped-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_untyped_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("untyped")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookE2ETestCase(
            description="python hook failure still tears down provider",
            command=("--no-color", "build", "--select", "orders"),
            expected_marker_entries=("setup", "pre", "teardown"),
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_with_provider_when_hook_fails_then_provider_tears_down(
    test_case: ProviderHookE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-failure-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_failure_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def failing_pre_hook(ctx, marker_provider: MarkerProvider):
                    '''Test hook failing_pre_hook.'''
                    marker_provider.mark("pre")
                    raise RuntimeError("intentional provider hook failure")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("failing_pre_hook")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookE2ETestCase(
            description="python post hook failure still tears down provider",
            command=("--no-color", "build", "--select", "orders"),
            expected_marker_entries=("setup", "post", "teardown"),
            expected_exit_code=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_post_hook_with_provider_when_hook_fails_then_provider_tears_down(
    test_case: ProviderHookE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-post-hook-failure-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_post_hook_failure_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def failing_post_hook(ctx, marker_provider: MarkerProvider):
                    '''Test hook failing_post_hook.'''
                    marker_provider.mark("post")
                    raise RuntimeError("intentional provider post hook failure")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  post_hooks [python("failing_post_hook")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode == test_case.expected_exit_code, result.stdout + result.stderr
    marker_entries: tuple[str, ...] = tuple(marker_path.read_text(encoding="utf-8").splitlines())
    assert marker_entries == test_case.expected_marker_entries


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookDiagnosticE2ETestCase(
            description="alias-imported hook provider annotation gets CLI diagnostic",
            command=("--no-color", "build", "--select", "orders"),
            expected_error_fragment=("annotated with MarkerProvider imported as 'alias_marker'"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_alias_imported_provider_annotation_on_hook_when_building_then_cli_prints_guidance(
    test_case: ProviderHookDiagnosticE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-alias-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_alias_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/alias_hooks.py": dedent(
                """
                import importlib.util
                import sys
                from pathlib import Path

                from sqlbuild.hooks import hook

                provider_path = Path(__file__).parents[2] / "python" / "nodes" / "python" / "providers" / "marker.py"
                spec = importlib.util.spec_from_file_location("alias_marker", provider_path)
                alias_marker = importlib.util.module_from_spec(spec)
                sys.modules["alias_marker"] = alias_marker
                assert spec.loader is not None
                spec.loader.exec_module(alias_marker)
                AliasMarkerProvider = alias_marker.MarkerProvider


                @hook
                def mark_pre(ctx, marker_provider: AliasMarkerProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("pre")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookDiagnosticE2ETestCase(
            description="hook provider annotation mismatch gets CLI diagnostic",
            command=("--no-color", "build", "--select", "orders"),
            expected_error_fragment=("Provider parameter 'marker_provider' expected OtherProvider"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_provider_annotation_mismatch_on_hook_when_building_then_cli_prints_mismatch(
    test_case: ProviderHookDiagnosticE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-mismatch-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_mismatch_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "python/providers/other.py": dedent(
                """
                from sqlbuild.providers import Provider


                class OtherProvider(Provider):
                    '''Test provider OtherProvider.'''
                    pass
                """
            ).strip()
            + "\n",
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.other import OtherProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider: OtherProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("pre")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookDiagnosticE2ETestCase(
            description="python hook argument cannot shadow provider injection",
            command=("--no-color", "build", "--select", "orders"),
            expected_error_fragment=(
                "argument 'marker_provider' conflicts with provider injection for "
                "parameter 'marker_provider'"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_kwarg_matches_provider_when_building_then_cli_prints_conflict(
    test_case: ProviderHookDiagnosticE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-conflict-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_conflict_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider: MarkerProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("pre")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                """
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre", marker_provider: "literal")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr


@pytest.mark.parametrize(
    "test_case",
    [
        ProviderHookContextConflictE2ETestCase(
            description="python hook argument cannot shadow ctx injection",
            command=("--no-color", "build", "--select", "orders"),
            context_parameter_name="ctx",
            expected_error_fragment=(
                "argument 'ctx' conflicts with reserved context parameter 'ctx'"
            ),
        ),
        ProviderHookContextConflictE2ETestCase(
            description="python hook argument cannot shadow context injection",
            command=("--no-color", "build", "--select", "orders"),
            context_parameter_name="context",
            expected_error_fragment=(
                "argument 'context' conflicts with reserved context parameter 'context'"
            ),
        ),
        ProviderHookContextConflictE2ETestCase(
            description="python hook argument cannot shadow _ctx injection",
            command=("--no-color", "build", "--select", "orders"),
            context_parameter_name="_ctx",
            expected_error_fragment=(
                "argument '_ctx' conflicts with reserved context parameter '_ctx'"
            ),
        ),
        ProviderHookContextConflictE2ETestCase(
            description="python hook argument cannot shadow hook_context injection",
            command=("--no-color", "build", "--select", "orders"),
            context_parameter_name="hook_context",
            expected_error_fragment=(
                "argument 'hook_context' conflicts with reserved context parameter 'hook_context'"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_hook_kwarg_matches_context_when_building_then_cli_prints_conflict(
    test_case: ProviderHookContextConflictE2ETestCase,
    tmp_path: Path,
) -> None:
    marker_path: Path = tmp_path / "provider-hook-context-conflict-marker.log"
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="provider_hook_context_conflict_project",
        repo_files={
            "sqlbuild_project.toml": PROVIDER_FAILURE_PROJECT_TOML,
            "python/providers/marker.py": PROVIDER_MARKER_FILE,
            "sql/hooks/python/marker_hooks.py": dedent(
                """
                from python.providers.marker import MarkerProvider
                from sqlbuild.hooks import hook


                @hook
                def mark_pre(ctx, marker_provider: MarkerProvider):
                    '''Test hook mark_pre.'''
                    marker_provider.mark("pre")
                """
            ).strip()
            + "\n",
            "sql/models/orders.sql": dedent(
                f"""
                MODEL (description "Test model orders.",
                  materialized table,
                  pre_hooks [python("mark_pre", {test_case.context_parameter_name}: "literal")]
                );

                SELECT 1 AS id
                """
            ).strip()
            + "\n",
        },
    )

    result: subprocess.CompletedProcess[str] = run_sqb(
        command=test_case.command,
        project_dir=project_dir,
        env={"MARKER_PATH": str(marker_path)},
    )

    assert result.returncode != 0, result.stdout + result.stderr
    assert test_case.expected_error_fragment in result.stdout + result.stderr
