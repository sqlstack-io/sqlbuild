"""Command-group warehouse precedence for CLI connection resolution."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    CliCommand,
    CliUserError,
    DiscoveredProjectInputs,
    LocalTargetConfig,
    ParsedCliInvocation,
    ResolvedWarehouse,
    TargetWarehousesConfig,
    WarehouseGroup,
    command_warehouse_scope,
    current_command_warehouse_scope,
    describe_connection_warehouse,
    parse_cli_invocation,
    resolve_project_connection_config,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.runtime._test_types import (
    CommandWarehouseErrorTestCase,
    CommandWarehouseGroupTestCase,
    CommandWarehouseResolutionTestCase,
    WarehouseFlagParseTestCase,
    WarehouseFlagRejectedTestCase,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.runtime.helpers import (
    build_warehouse_discovered_inputs,
    write_case_adapter_file,
)

_PROJECT_GROUPS: TargetWarehousesConfig = TargetWarehousesConfig(build="BUILD_WH", query="ADHOC_WH")
_SNOWFLAKE_SUBCLASS_ADAPTER: str = (
    "from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter\n\n\n"
    "class SnowflakePlusAdapter(SnowflakeAdapter):\n"
    '    adapter_name = "snowflake_plus"\n'
)
_LOCAL_QUERY_OVERRIDE: dict[str, LocalTargetConfig] = {
    "dev": LocalTargetConfig(warehouses=TargetWarehousesConfig(query="LOCAL_ADHOC_WH"))
}


@pytest.mark.parametrize(
    "test_case",
    [
        CommandWarehouseResolutionTestCase(
            description="build uses the project build group",
            command="build",
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="BUILD_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] build",
        ),
        CommandWarehouseResolutionTestCase(
            description="query uses the project query group",
            command="query",
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="ADHOC_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] query",
        ),
        CommandWarehouseResolutionTestCase(
            description="local query group overrides the project for diff",
            command="diff",
            project_warehouses=_PROJECT_GROUPS,
            local_targets=_LOCAL_QUERY_OVERRIDE,
            expected_warehouse="LOCAL_ADHOC_WH",
            expected_source="sqlbuild_local.toml [targets.dev.warehouses] query",
        ),
        CommandWarehouseResolutionTestCase(
            description="local query override leaves the project build group for plan",
            command="plan",
            project_warehouses=_PROJECT_GROUPS,
            local_targets=_LOCAL_QUERY_OVERRIDE,
            expected_warehouse="BUILD_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] build",
        ),
        CommandWarehouseResolutionTestCase(
            description="a location syncs on the query warehouse, not the connection's",
            command="location",
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="ADHOC_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] query",
        ),
        CommandWarehouseResolutionTestCase(
            description="a location syncs on the sync warehouse when the target sets one",
            command="location",
            project_warehouses=TargetWarehousesConfig(
                build="BUILD_WH", query="ADHOC_WH", sync="SYNC_WH"
            ),
            expected_warehouse="SYNC_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] sync",
        ),
        CommandWarehouseResolutionTestCase(
            description="a location with only a build warehouse syncs on the connection's, not the build one",
            command="location",
            project_warehouses=TargetWarehousesConfig(build="XL_WH"),
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="a location with only a build warehouse and no connection warehouse syncs on the build one",
            command="location",
            project_warehouses=TargetWarehousesConfig(build="XL_WH"),
            connection={"account": "example-account"},
            expected_warehouse="XL_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] build",
        ),
        CommandWarehouseResolutionTestCase(
            description="a location whose target sets no warehouse keeps the connection's",
            command="location",
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="cli warehouse overrides the group",
            command="build",
            project_warehouses=_PROJECT_GROUPS,
            cli_warehouse="CLI_WH",
            expected_warehouse="CLI_WH",
            expected_source="--warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="unset group falls back to the connection warehouse",
            command="build",
            project_warehouses=TargetWarehousesConfig(query="ADHOC_WH"),
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="no configured warehouse keeps the connection default",
            command="query",
            connection={"account": "example-account"},
            expected_warehouse=None,
            expected_source="connection default",
        ),
        CommandWarehouseResolutionTestCase(
            description="command without a group keeps the connection warehouse",
            command="compile",
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="resolution outside a cli command keeps the connection warehouse",
            command=None,
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="selected target uses its own groups",
            command="build",
            project_warehouses=_PROJECT_GROUPS,
            selected_target="prod",
            expected_warehouse="PROD_BUILD_WH",
            expected_source="sqlbuild_project.toml [targets.prod.warehouses] build",
        ),
        CommandWarehouseResolutionTestCase(
            description="custom adapter extending snowflake applies groups",
            command="build",
            adapter="snowflake_plus",
            adapter_file_contents=_SNOWFLAKE_SUBCLASS_ADAPTER,
            project_warehouses=_PROJECT_GROUPS,
            expected_warehouse="BUILD_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] build",
        ),
        CommandWarehouseResolutionTestCase(
            description="local duckdb override ignores shared groups without error",
            command="query",
            local_adapter="duckdb",
            project_warehouses=TargetWarehousesConfig(
                build="BUILD_WH", query="${ENV:SQB_EXAMPLE_MISSING_QWH}"
            ),
            expected_warehouse="ANALYTICS_WH",
            expected_source="connection warehouse",
        ),
        CommandWarehouseResolutionTestCase(
            description="double-quoted group identifier is kept",
            command="query",
            project_warehouses=TargetWarehousesConfig(query='"Adhoc WH"'),
            expected_warehouse='"Adhoc WH"',
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] query",
        ),
        CommandWarehouseResolutionTestCase(
            description="environment template in a group is expanded",
            command="query",
            project_warehouses=TargetWarehousesConfig(query="${ENV:SQB_EXAMPLE_ADHOC_WH}"),
            expected_warehouse="ENV_ADHOC_WH",
            expected_source="sqlbuild_project.toml [targets.dev.warehouses] query",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_command_group_when_resolving_connection_then_warehouse_follows_precedence(
    test_case: CommandWarehouseResolutionTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("SQB_EXAMPLE_ADHOC_WH", "ENV_ADHOC_WH")
    monkeypatch.delenv("SQB_EXAMPLE_MISSING_QWH", raising=False)
    write_case_adapter_file(project_dir=tmp_path, contents=test_case.adapter_file_contents)
    discovered_inputs: DiscoveredProjectInputs = build_warehouse_discovered_inputs(test_case)

    with command_warehouse_scope(command=test_case.command, cli_warehouse=test_case.cli_warehouse):
        connection: dict[str, object] = resolve_project_connection_config(
            discovered_inputs=discovered_inputs,
            project_dir=tmp_path,
            selected_target=test_case.selected_target,
        )
        described: ResolvedWarehouse = describe_connection_warehouse(
            discovered_inputs=discovered_inputs,
            selected_target=test_case.selected_target,
            connection_config=connection,
        )

    assert connection.get("warehouse") == test_case.expected_warehouse
    assert described == ResolvedWarehouse(
        warehouse=test_case.expected_warehouse, source=test_case.expected_source
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CommandWarehouseErrorTestCase(
            description="duckdb rejects a cli warehouse",
            resolution=CommandWarehouseResolutionTestCase(
                description="duckdb",
                command="query",
                adapter="duckdb",
                connection={"database": ":memory:"},
                cli_warehouse="ADHOC_WH",
                expected_warehouse=None,
                expected_source="connection default",
            ),
            expected_code="C260",
            expected_message_fragment="'duckdb' adapter has no warehouse to select",
        ),
        CommandWarehouseErrorTestCase(
            description="injected group warehouse is rejected",
            resolution=CommandWarehouseResolutionTestCase(
                description="injected group",
                command="query",
                project_warehouses=TargetWarehousesConfig(query="x; drop table t"),
                expected_warehouse=None,
                expected_source="",
            ),
            expected_code="C261",
            expected_message_fragment=(
                "[targets.dev.warehouses] query resolves to 'x; drop table t', which is not a "
                "valid Snowflake warehouse identifier"
            ),
        ),
        CommandWarehouseErrorTestCase(
            description="injected cli warehouse is rejected",
            resolution=CommandWarehouseResolutionTestCase(
                description="injected cli",
                command="build",
                cli_warehouse="x; drop table t",
                expected_warehouse=None,
                expected_source="",
            ),
            expected_code="C261",
            expected_message_fragment="--warehouse 'x; drop table t' is not a valid Snowflake",
        ),
        CommandWarehouseErrorTestCase(
            description="unterminated quoted cli warehouse is rejected",
            resolution=CommandWarehouseResolutionTestCase(
                description="unterminated quote",
                command="build",
                cli_warehouse='"ADHOC',
                expected_warehouse=None,
                expected_source="",
            ),
            expected_code="C261",
            expected_message_fragment="is not a valid Snowflake warehouse identifier",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_without_warehouses_when_cli_warehouse_is_given_then_it_fails(
    test_case: CommandWarehouseErrorTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = build_warehouse_discovered_inputs(
        test_case.resolution
    )

    with (
        command_warehouse_scope(
            command=test_case.resolution.command,
            cli_warehouse=test_case.resolution.cli_warehouse,
        ),
        pytest.raises(CliUserError) as error,
    ):
        resolve_project_connection_config(discovered_inputs=discovered_inputs, project_dir=tmp_path)

    assert error.value.code == test_case.expected_code
    assert test_case.expected_message_fragment in error.value.message


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseFlagParseTestCase(
            description="build", argv=("build", "--warehouse", "W"), expected_warehouse="W"
        ),
        WarehouseFlagParseTestCase(
            description="plan", argv=("plan", "--warehouse", "W"), expected_warehouse="W"
        ),
        WarehouseFlagParseTestCase(
            description="query",
            argv=("query", "SELECT 1", "--warehouse", "W"),
            expected_warehouse="W",
        ),
        WarehouseFlagParseTestCase(
            description="diff", argv=("diff", "--warehouse", "W"), expected_warehouse="W"
        ),
        WarehouseFlagParseTestCase(
            description="debug", argv=("debug", "--warehouse", "W"), expected_warehouse="W"
        ),
        WarehouseFlagParseTestCase(
            description="contract subcommand",
            argv=("contract", "diff", "--from", "prod", "--warehouse", "W"),
            expected_warehouse="W",
        ),
        WarehouseFlagParseTestCase(
            description="scenario subcommand",
            argv=("scenario", "test", "--warehouse", "W"),
            expected_warehouse="W",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_grouped_command_when_parsing_warehouse_flag_then_value_is_kept(
    test_case: WarehouseFlagParseTestCase,
) -> None:
    parsed: ParsedCliInvocation = parse_cli_invocation(argv=test_case.argv, use_color=False)

    assert parsed.args is not None
    assert parsed.args.warehouse == test_case.expected_warehouse


@pytest.mark.parametrize(
    "test_case",
    [
        WarehouseFlagRejectedTestCase(
            description="compile never connects", argv=("compile", "--warehouse", "W")
        ),
        WarehouseFlagRejectedTestCase(
            description="lineage never connects", argv=("lineage", "--warehouse", "W")
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_command_without_group_when_parsing_warehouse_flag_then_it_is_rejected(
    test_case: WarehouseFlagRejectedTestCase,
    capsys: pytest.CaptureFixture[str],
) -> None:
    parsed: ParsedCliInvocation = parse_cli_invocation(argv=test_case.argv, use_color=False)

    assert parsed.exit_code == 2
    assert test_case.expected_error_fragment in capsys.readouterr().err


@pytest.mark.parametrize(
    "test_case",
    [
        CommandWarehouseGroupTestCase(
            description="standalone SQL tests use fixture data",
            command=CliCommand.TEST,
            expected_group=WarehouseGroup.QUERY,
        ),
        CommandWarehouseGroupTestCase(
            description="scenarios build against fixture data",
            command=CliCommand.SCENARIO,
            expected_group=WarehouseGroup.QUERY,
        ),
        CommandWarehouseGroupTestCase(
            description="janitor works on catalogue metadata",
            command=CliCommand.JANITOR,
            expected_group=WarehouseGroup.QUERY,
        ),
        CommandWarehouseGroupTestCase(
            description="audits scan built tables",
            command=CliCommand.AUDIT,
            expected_group=WarehouseGroup.BUILD,
        ),
        CommandWarehouseGroupTestCase(
            description="Python checks read real outputs",
            command=CliCommand.CHECK,
            expected_group=WarehouseGroup.BUILD,
        ),
        CommandWarehouseGroupTestCase(
            description="builds execute project resources",
            command=CliCommand.BUILD,
            expected_group=WarehouseGroup.BUILD,
        ),
        CommandWarehouseGroupTestCase(
            description="a location's syncs read metadata and small state tables",
            command=CliCommand.LOCATION,
            expected_group=WarehouseGroup.SYNC,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_cli_command_when_resolving_group_then_matches_its_workload(
    test_case: CommandWarehouseGroupTestCase,
) -> None:
    with command_warehouse_scope(command=test_case.command, cli_warehouse=None):
        assert current_command_warehouse_scope().group is test_case.expected_group


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
