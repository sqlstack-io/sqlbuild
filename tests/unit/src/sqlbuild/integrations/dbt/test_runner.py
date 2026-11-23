from __future__ import annotations

import sys
from pathlib import Path

import pytest

from sqlbuild._native import (
    DbtCliOptions,
    DbtCommandResult,
    DbtInteropRuntimeError,
    DbtLsNode,
    DbtLsResult,
    DbtRunner,
    build_dbt_compile_argv,
    build_dbt_debug_argv,
    build_dbt_deps_argv,
    build_dbt_ls_argv,
    dbt_fqn_selector_term,
    parse_dbt_ls_json_lines,
)
from tests.unit.src.sqlbuild.integrations.dbt._test_types import (
    DbtArgvTestCase,
    DbtLsParseTestCase,
    DbtRunnerCommandTestCase,
    DbtRunnerLaunchFailureTestCase,
    DbtRunnerMemoTestCase,
    DbtRunnerProcessTestCase,
)
from tests.unit.src.sqlbuild.integrations.dbt.helpers import (
    RecordingDbtInvoker,
    build_dbt_cli_options,
)

PROJECT_ROOT: Path = Path("/repo")
OPTIONS_ARGV_SUFFIX: tuple[str, ...] = (
    "--project-dir",
    "/repo/dbt",
    "--profiles-dir",
    "/repo/profiles",
    "--target",
    "prod",
    "--target-path",
    "/repo/target/dbt",
    "--vars",
    '{"run_date":"2026-01-01"}',
    "--state",
    "/repo/state",
    "--defer",
)


@pytest.mark.parametrize(
    "test_case",
    [
        DbtArgvTestCase(
            description="builds compile argv with common options",
            select=(),
            exclude=(),
            resource_types=(),
            expected_argv=("dbt", "compile", *OPTIONS_ARGV_SUFFIX),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_options_when_building_compile_argv_then_returns_expected_command(
    test_case: DbtArgvTestCase,
) -> None:
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: tuple[str, ...] = build_dbt_compile_argv(dbt_executable="dbt", options=options)

    assert result == test_case.expected_argv


@pytest.mark.parametrize(
    "test_case",
    [
        DbtArgvTestCase(
            description="builds deps argv with project profile target and vars only",
            select=(),
            exclude=(),
            resource_types=(),
            expected_argv=(
                "dbt",
                "deps",
                "--project-dir",
                "/repo/dbt",
                "--profiles-dir",
                "/repo/profiles",
                "--target",
                "prod",
                "--vars",
                '{"run_date":"2026-01-01"}',
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_options_when_building_deps_argv_then_returns_expected_command(
    test_case: DbtArgvTestCase,
) -> None:
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: tuple[str, ...] = build_dbt_deps_argv(dbt_executable="dbt", options=options)

    assert result == test_case.expected_argv


@pytest.mark.parametrize(
    "test_case",
    [
        DbtArgvTestCase(
            description="builds compile argv with full refresh",
            select=(),
            exclude=(),
            resource_types=(),
            expected_argv=("dbt", "compile", *OPTIONS_ARGV_SUFFIX, "--full-refresh"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_full_refresh_when_building_compile_argv_then_includes_full_refresh(
    test_case: DbtArgvTestCase,
) -> None:
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: tuple[str, ...] = build_dbt_compile_argv(
        dbt_executable="dbt",
        options=options,
        full_refresh=True,
    )

    assert result == test_case.expected_argv


@pytest.mark.parametrize(
    "test_case",
    [
        DbtArgvTestCase(
            description="builds debug argv with common options",
            select=(),
            exclude=(),
            resource_types=(),
            expected_argv=("dbt", "debug", *OPTIONS_ARGV_SUFFIX),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_options_when_building_debug_argv_then_returns_expected_command(
    test_case: DbtArgvTestCase,
) -> None:
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: tuple[str, ...] = build_dbt_debug_argv(dbt_executable="dbt", options=options)

    assert result == test_case.expected_argv


@pytest.mark.parametrize(
    "test_case",
    [
        DbtArgvTestCase(
            description="builds ls argv with only json output and common options",
            select=(),
            exclude=(),
            resource_types=(),
            expected_argv=("dbt", "ls", "--output", "json", *OPTIONS_ARGV_SUFFIX),
        ),
        DbtArgvTestCase(
            description="builds ls argv with selectors excludes and resource types",
            select=("tag:nightly+", "state:modified"),
            exclude=("tag:deprecated",),
            resource_types=("model",),
            expected_argv=(
                "dbt",
                "ls",
                "--output",
                "json",
                *OPTIONS_ARGV_SUFFIX,
                "--select",
                "tag:nightly+",
                "state:modified",
                "--exclude",
                "tag:deprecated",
                "--resource-type",
                "model",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_options_when_building_ls_argv_then_returns_expected_command(
    test_case: DbtArgvTestCase,
) -> None:
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: tuple[str, ...] = build_dbt_ls_argv(
        dbt_executable="dbt",
        options=options,
        select=test_case.select,
        exclude=test_case.exclude,
        resource_types=test_case.resource_types,
    )

    assert result == test_case.expected_argv


@pytest.mark.parametrize(
    "test_case",
    [
        DbtLsParseTestCase(
            description="parses json lines and ignores dbt log noise",
            stdout=(
                "10:00:00 Running with dbt=1.9.0\n"
                '{"unique_id":"model.analytics.orders","resource_type":"model",'
                '"package_name":"analytics","name":"orders",'
                '"fqn":["analytics","marts","orders"],'
                '"original_file_path":"sql/models/orders.sql"}\n'
                "not json\n"
                '{"unique_id":"source.analytics.raw.orders","resource_type":"source"}\n'
            ),
            expected_unique_ids=("model.analytics.orders", "source.analytics.raw.orders"),
            expected_resource_types=("model", "source"),
            expected_selector_terms=("fqn:analytics.marts.orders", "source.analytics.raw.orders"),
        ),
        DbtLsParseTestCase(
            description="ignores json objects without unique id",
            stdout='{"name":"orders"}\n{"unique_id":"model.analytics.orders"}\n',
            expected_unique_ids=("model.analytics.orders",),
            expected_resource_types=(None,),
            expected_selector_terms=("model.analytics.orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_ls_output_when_parsing_then_returns_unique_id_nodes(
    test_case: DbtLsParseTestCase,
) -> None:
    result: tuple[DbtLsNode, ...] = parse_dbt_ls_json_lines(stdout=test_case.stdout)

    assert tuple(node.unique_id for node in result) == test_case.expected_unique_ids
    assert tuple(node.resource_type for node in result) == test_case.expected_resource_types
    assert (
        tuple(
            dbt_fqn_selector_term(fqn=node.fqn, fallback=node.name or node.unique_id)
            for node in result
        )
        == test_case.expected_selector_terms
    )


@pytest.mark.parametrize(
    "test_case",
    [
        DbtRunnerMemoTestCase(
            description="memoizes identical ls argv within runner instance",
            command_result=DbtCommandResult(
                argv=("dbt", "ls"),
                returncode=0,
                stdout='{"unique_id":"model.analytics.orders"}\n',
            ),
            expected_call_count=1,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_dbt_ls_when_running_then_reuses_memoized_result(
    test_case: DbtRunnerMemoTestCase,
) -> None:
    invoker: RecordingDbtInvoker = RecordingDbtInvoker(test_case.command_result)
    runner: DbtRunner = DbtRunner(dbt_executable="dbt", invoker=invoker)
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    first: DbtLsResult = runner.ls(options=options, select=("tag:nightly",))
    second: DbtLsResult = runner.ls(options=options, select=("tag:nightly",))

    assert first is second
    assert len(invoker.calls) == test_case.expected_call_count
    assert tuple(node.unique_id for node in first.nodes) == ("model.analytics.orders",)


@pytest.mark.parametrize(
    "test_case",
    [
        DbtRunnerCommandTestCase(
            description="uses project dir as cwd for compile",
            command_result=DbtCommandResult(argv=("dbt", "compile"), returncode=0),
            expected_argv=("dbt", "compile", *OPTIONS_ARGV_SUFFIX),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_runner_when_running_compile_then_uses_project_dir_as_cwd(
    test_case: DbtRunnerCommandTestCase,
) -> None:
    invoker: RecordingDbtInvoker = RecordingDbtInvoker(test_case.command_result)
    runner: DbtRunner = DbtRunner(dbt_executable="dbt", invoker=invoker)
    options: DbtCliOptions = build_dbt_cli_options(PROJECT_ROOT)

    result: DbtCommandResult = runner.compile(options=options)

    assert result == test_case.command_result
    assert invoker.calls == [(test_case.expected_argv, options.project_dir)]


@pytest.mark.parametrize(
    "test_case",
    [
        DbtRunnerProcessTestCase(
            description="captures text output, stderr and the exit code of a real process",
            script="import sys\nprint('compiled')\nsys.stderr.write('warn\\r\\n')\nsys.exit(3)\n",
            expected_returncode=3,
            expected_stdout="compiled\n",
            expected_stderr="warn\n",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_process_when_invoked_then_captures_output_and_exit_code(
    test_case: DbtRunnerProcessTestCase,
    tmp_path: Path,
) -> None:
    script: Path = tmp_path / "fake_dbt.py"
    _ = script.write_text(test_case.script, encoding="utf-8")
    runner: DbtRunner = DbtRunner(dbt_executable="dbt")

    result: DbtCommandResult = runner.invoke(argv=(sys.executable, str(script)), cwd=tmp_path)

    assert result.returncode == test_case.expected_returncode
    assert result.stdout == test_case.expected_stdout
    assert result.stderr == test_case.expected_stderr


@pytest.mark.parametrize(
    "test_case",
    [
        DbtRunnerLaunchFailureTestCase(
            description="a missing executable raises the dbt launch error",
            executable_name="missing-dbt",
            expected_message="failed to execute dbt",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_missing_dbt_executable_when_invoked_then_raises_launch_error(
    test_case: DbtRunnerLaunchFailureTestCase,
    tmp_path: Path,
) -> None:
    runner: DbtRunner = DbtRunner(dbt_executable="dbt")
    executable: Path = tmp_path / test_case.executable_name

    with pytest.raises(DbtInteropRuntimeError, match=test_case.expected_message) as raised:
        _ = runner.invoke(argv=(str(executable),), cwd=None)

    assert isinstance(raised.value.__cause__, FileNotFoundError)
    assert raised.value.help == f"[Errno 2] No such file or directory: '{executable}'"
