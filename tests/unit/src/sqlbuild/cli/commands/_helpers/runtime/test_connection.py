from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    LocalConfig,
    LocalTargetConfig,
    ProjectConfig,
    ProjectConfigError,
    TargetConfig,
    build_effective_connection_config,
    resolve_connection_config,
    resolve_project_connection_config,
    resolve_target_connection_config,
)
from sqlbuild.adapter.contract.types import BuiltinAdapter
from sqlbuild.compiler.compile.exceptions import CompileInputError
from tests.unit.src.sqlbuild.cli.commands._helpers.runtime._test_types import (
    ExplicitMemoryConnectionTestCase,
    MissingConnectionTestCase,
    NamedConnectionBehaviorTestCase,
    OfflineConnectionResolutionTestCase,
    ResolveConnectionConfigWarningTestCase,
    ResolveDbtProfileConnectionConfigTestCase,
    ResolveEnvironmentConnectionConfigTestCase,
    ResolveProjectConnectionConfigTestCase,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.runtime.helpers import write_dbt_profile


@pytest.mark.parametrize(
    "test_case",
    [
        ResolveProjectConnectionConfigTestCase(
            description="uses project target and local connection precedence",
            project_dir_name="demo_project",
            expected_connection={
                "database": "demo_project/local.duckdb",
                "warehouse": "dev_wh",
                "role": "local_role",
            },
            expected_warning_fragment=(
                "Warning: DuckDB adapter is active, but connection contains "
                "Snowflake-like keys: role, warehouse. If this is a Snowflake local config, "
                'add `adapter = "snowflake"` at the top of sqlbuild_local.toml, before any section.'
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_project_inputs_when_resolving_connection_then_uses_effective_connection(
    test_case: ResolveProjectConnectionConfigTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    project_dir: Path = tmp_path / test_case.project_dir_name
    project_dir.mkdir()
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="duckdb",
            default_target="dev",
            connection={"database": "project.duckdb", "warehouse": "project_wh"},
            targets={
                "dev": TargetConfig(connection={"database": "dev.duckdb", "warehouse": "dev_wh"})
            },
        ),
        local_config=LocalConfig(connection={"database": "local.duckdb", "role": "local_role"}),
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs,
        project_dir=project_dir,
    )

    expected_connection: dict[str, object] = dict(test_case.expected_connection)
    expected_connection["database"] = str(tmp_path / str(expected_connection["database"]))
    assert connection == expected_connection
    captured_err: str = capsys.readouterr().err
    assert test_case.expected_warning_fragment in captured_err


@pytest.mark.parametrize(
    "test_case",
    [
        ResolveConnectionConfigWarningTestCase(
            description="does not warn when snowflake adapter has snowflake keys",
            raw_config={"database": "SQB_DB", "warehouse": "SQB_WH", "role": "SQB_ROLE"},
            adapter_name=BuiltinAdapter.SNOWFLAKE,
            expected_connection={
                "database": "SQB_DB",
                "warehouse": "SQB_WH",
                "role": "SQB_ROLE",
            },
            expected_warning="",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_connection_when_resolving_then_emits_expected_warning(
    test_case: ResolveConnectionConfigWarningTestCase,
    tmp_path: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    connection: dict[str, object] = resolve_connection_config(
        raw_config=test_case.raw_config,
        project_dir=tmp_path,
        adapter_name=test_case.adapter_name,
    )

    captured_err: str = capsys.readouterr().err
    assert connection == test_case.expected_connection
    assert captured_err == test_case.expected_warning


@pytest.mark.parametrize(
    "test_case",
    [
        NamedConnectionBehaviorTestCase(
            description="locked precedence with one interpolation pass",
            expected_connection={
                "layer": "local_legacy",
                "legacy_project": True,
                "account": "expanded-account",
                "project_named": True,
                "local_named": True,
                "target": True,
                "local": True,
                "legacy_local": True,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_named_connection_layers_when_resolving_then_uses_locked_precedence_and_expands_once(
    test_case: NamedConnectionBehaviorTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("NAMED_ACCOUNT", "expanded-account")
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="snowflake",
            default_target="dev",
            connection={"layer": "project", "legacy_project": True},
            connections={
                "developer": {
                    "layer": "project_named",
                    "account": "${ENV:NAMED_ACCOUNT}",
                    "project_named": True,
                }
            },
            targets={
                "dev": TargetConfig(
                    connection_name="developer",
                    connection={"layer": "project_target", "target": True},
                )
            },
        ),
        local_config=LocalConfig(
            connections={"developer": {"layer": "local_named", "local_named": True}},
            targets={"dev": LocalTargetConfig(connection={"layer": "local_target", "local": True})},
            connection={"layer": "local_legacy", "legacy_local": True},
        ),
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs, project_dir=tmp_path
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        NamedConnectionBehaviorTestCase(
            description="catalog credentials never inherit writer fields",
            expected_connection={
                "account": "catalog-account",
                "user": "catalog-reader",
                "warehouse": "catalog-warehouse",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_catalog_connection_when_resolving_then_only_its_own_named_layers_and_secrets_apply(
    test_case: NamedConnectionBehaviorTestCase,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.delenv("SQLBUILD_MISSING_WRITER_PASSWORD", raising=False)
    monkeypatch.setenv("SQLBUILD_CATALOG_USER", "catalog-reader")
    discovered: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="orders",
            adapter="snowflake",
            default_target="development",
            connection={"password": "${ENV:SQLBUILD_MISSING_WRITER_PASSWORD}"},
            connections={
                "writer": {"account": "writer-account", "private_key_file": "writer.pem"},
                "catalog": {"account": "catalog-account", "user": "${ENV:SQLBUILD_CATALOG_USER}"},
            },
            targets={
                "development": TargetConfig(
                    connection_name="writer",
                    catalog_connection="catalog",
                    connection={"role": "writer-role"},
                )
            },
        ),
        local_config=LocalConfig(
            connection={"token": "writer-token"},
            connections={"catalog": {"warehouse": "catalog-warehouse"}},
            targets={"development": LocalTargetConfig(connection={"user": "writer-user"})},
        ),
    )
    assert (
        build_effective_connection_config(discovered_inputs=discovered, catalog=True)
        == test_case.expected_connection
    )
    with pytest.raises(CompileInputError, match="SQLBUILD_MISSING_WRITER_PASSWORD"):
        build_effective_connection_config(discovered_inputs=discovered)
    discovered.local_config.targets["development"] = LocalTargetConfig(catalog_connection="missing")
    with pytest.raises(CompileInputError, match="catalog_connection: Unknown connection 'missing'"):
        build_effective_connection_config(discovered_inputs=discovered, catalog=True)


@pytest.mark.parametrize(
    "test_case",
    [
        NamedConnectionBehaviorTestCase(
            description="local-only connection reference",
            expected_connection={"database": ":memory:", "custom": "kept"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_local_only_named_connection_when_resolving_then_it_is_allowed(
    test_case: NamedConnectionBehaviorTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="duckdb",
            default_target="dev",
            targets={"dev": TargetConfig(connection_name="developer")},
        ),
        local_config=LocalConfig(
            connections={"developer": {"database": ":memory:", "custom": "kept"}}
        ),
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs, project_dir=tmp_path
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        NamedConnectionBehaviorTestCase(
            description="unknown effective reference",
            expected_connection={},
            expected_error_fragment="Unknown connection 'missing'.*connections.missing",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_unknown_named_connection_when_resolving_then_it_fails_before_connecting(
    test_case: NamedConnectionBehaviorTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="duckdb",
            default_target="dev",
            targets={"dev": TargetConfig(connection_name="missing")},
        ),
        local_config=LocalConfig(),
    )

    with pytest.raises(CompileInputError, match=str(test_case.expected_error_fragment)):
        resolve_project_connection_config(discovered_inputs=discovered_inputs, project_dir=tmp_path)


@pytest.mark.parametrize(
    "test_case",
    [
        NamedConnectionBehaviorTestCase(
            description="named dbt profile route",
            expected_connection={"account": "acct", "database": "ANALYTICS"},
        )
    ],
    ids=lambda case: case.description,
)
def test_given_named_dbt_profile_connection_when_resolving_then_routes_downstream(
    test_case: NamedConnectionBehaviorTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="snowflake",
            default_target="dev",
            connections={
                "dbt": {
                    "source": "dbt_profile",
                    "profile": "analytics",
                    "target": "dev",
                    "profiles_dir": "profiles",
                }
            },
            targets={"dev": TargetConfig(connection_name="dbt")},
        ),
        local_config=LocalConfig(),
    )

    write_dbt_profile(
        tmp_path / "profiles",
        profile="analytics",
        output={"type": "snowflake", "account": "acct", "database": "ANALYTICS"},
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs, project_dir=tmp_path
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        ResolveDbtProfileConnectionConfigTestCase(
            description="merges user connection overrides over resolved dbt profile connection",
            raw_config={
                "source": "dbt_profile",
                "profile": "analytics",
                "target": "dev",
                "profiles_dir": "profiles",
                "account": "override-acct",
                "client_request_mfa_token": True,
            },
            profile_connection={
                "account": "profile-acct",
                "user": "analytics",
                "database": "RAW",
                "schema": "ANALYTICS",
            },
            expected_connection={
                "account": "override-acct",
                "user": "analytics",
                "database": "RAW",
                "schema": "ANALYTICS",
                "client_request_mfa_token": True,
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_dbt_profile_connection_when_resolving_then_merges_user_overrides(
    test_case: ResolveDbtProfileConnectionConfigTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(name="demo", adapter="snowflake"),
        local_config=LocalConfig(),
    )

    write_dbt_profile(
        tmp_path / "profiles",
        profile="analytics",
        output={"type": "snowflake", **test_case.profile_connection},
    )

    connection: dict[str, object] = resolve_connection_config(
        raw_config=test_case.raw_config,
        project_dir=tmp_path,
        adapter_name=BuiltinAdapter.SNOWFLAKE,
        discovered_inputs=discovered_inputs,
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        ResolveEnvironmentConnectionConfigTestCase(
            description="resolves target connection with expanded env vars and local overrides",
            target_name="prod",
            expected_connection={
                "account": "test-account",
                "warehouse": "local_wh",
                "database": "analytics",
                "schema": "prod_schema",
            },
        )
    ],
    ids=lambda case: case.description,
)
def test_given_target_connection_when_resolving_then_it_expands_effective_config(
    test_case: ResolveEnvironmentConnectionConfigTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("TEST_ACCOUNT", "test-account")
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="demo",
            adapter="snowflake",
            connection={
                "account": "${ENV:TEST_ACCOUNT}",
                "warehouse": "base_wh",
                "database": "analytics",
            },
            targets={
                "prod": TargetConfig(connection={"schema": "prod_schema"}),
            },
        ),
        local_config=LocalConfig(connection={"warehouse": "local_wh"}),
    )

    connection: dict[str, object] = resolve_target_connection_config(
        discovered_inputs=discovered_inputs,
        project_dir=tmp_path,
        target_name=test_case.target_name,
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        MissingConnectionTestCase(
            description="default target without any connection",
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                default_target="dev",
                targets={"dev": TargetConfig(schema="dev")},
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment=(
                "target dev has no connection; add [connections.<name>] (or set connection = "
                '"<name>" on [targets.dev]) in sqlbuild_project.toml or sqlbuild_local.toml'
            ),
        ),
        MissingConnectionTestCase(
            description="selected local target without any connection",
            project_config=ProjectConfig(name="shop", adapter="duckdb"),
            local_config=LocalConfig(targets={"scratch": LocalTargetConfig(schema="scratch")}),
            selected_target="scratch",
            expected_error_fragment="target scratch has no connection",
        ),
        MissingConnectionTestCase(
            description="no target and no connection",
            project_config=ProjectConfig(name="shop", adapter="duckdb"),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="no target is selected and no top-level [connection] is configured",
        ),
        MissingConnectionTestCase(
            description="no target selected while only named connections exist",
            project_config=ProjectConfig(
                name="shop", adapter="duckdb", connections={"local": {"database": "shop.duckdb"}}
            ),
            local_config=LocalConfig(),
            selected_target=None,
            expected_error_fragment="no target is selected and no top-level [connection] is configured",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_configured_connection_when_resolving_then_it_fails_before_connecting(
    test_case: MissingConnectionTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config, local_config=test_case.local_config
    )

    with pytest.raises(ProjectConfigError) as error_info:
        resolve_project_connection_config(
            discovered_inputs=discovered_inputs,
            project_dir=tmp_path,
            selected_target=test_case.selected_target,
        )

    assert error_info.value.code == "D001"
    assert test_case.expected_error_fragment in str(error_info.value)
    assert str(tmp_path / "sqlbuild_project.toml") in str(error_info.value)


@pytest.mark.parametrize(
    "test_case",
    [
        MissingConnectionTestCase(
            description="named target without any connection",
            project_config=ProjectConfig(
                name="shop", adapter="duckdb", targets={"prod": TargetConfig(schema="prod")}
            ),
            local_config=LocalConfig(),
            selected_target="prod",
            expected_error_fragment="target prod has no connection",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_target_without_connection_when_resolving_target_then_it_fails_before_connecting(
    test_case: MissingConnectionTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config, local_config=test_case.local_config
    )

    with pytest.raises(ProjectConfigError, match=test_case.expected_error_fragment):
        resolve_target_connection_config(
            discovered_inputs=discovered_inputs,
            project_dir=tmp_path,
            target_name=str(test_case.selected_target),
        )


@pytest.mark.parametrize(
    "test_case",
    [
        ExplicitMemoryConnectionTestCase(
            description="named in-memory connection",
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                default_target="dev",
                connections={"memory": {"database": ":memory:"}},
                targets={"dev": TargetConfig(schema="main")},
            ),
            local_config=LocalConfig(),
            expected_connection={"database": ":memory:"},
        ),
        ExplicitMemoryConnectionTestCase(
            description="top level in-memory connection without a target",
            project_config=ProjectConfig(
                name="shop", adapter="duckdb", connection={"database": ":memory:"}
            ),
            local_config=LocalConfig(),
            expected_connection={"database": ":memory:"},
        ),
        ExplicitMemoryConnectionTestCase(
            description="local top level in-memory connection",
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                default_target="dev",
                targets={"dev": TargetConfig(schema="main")},
            ),
            local_config=LocalConfig(connection={"database": ":memory:"}),
            expected_connection={"database": ":memory:"},
        ),
        ExplicitMemoryConnectionTestCase(
            description="inline target in-memory connection",
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                default_target="dev",
                targets={"dev": TargetConfig(schema="main", connection={"database": ":memory:"})},
            ),
            local_config=LocalConfig(),
            expected_connection={"database": ":memory:"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_explicit_memory_connection_when_resolving_then_it_is_used(
    test_case: ExplicitMemoryConnectionTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config, local_config=test_case.local_config
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs, project_dir=tmp_path
    )

    assert connection == test_case.expected_connection


@pytest.mark.parametrize(
    "test_case",
    [
        OfflineConnectionResolutionTestCase(
            description="target without any connection resolves empty for offline modes",
            project_config=ProjectConfig(
                name="shop",
                adapter="duckdb",
                default_target="dev",
                targets={"dev": TargetConfig(schema="dev")},
            ),
            local_config=LocalConfig(),
            expected_connection={},
        ),
        OfflineConnectionResolutionTestCase(
            description="no target and no connection resolves empty for offline modes",
            project_config=ProjectConfig(name="shop", adapter="duckdb"),
            local_config=LocalConfig(),
            expected_connection={},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_no_configured_connection_when_resolving_for_offline_mode_then_it_is_not_required(
    test_case: OfflineConnectionResolutionTestCase, tmp_path: Path
) -> None:
    discovered_inputs: DiscoveredProjectInputs = DiscoveredProjectInputs(
        project_config=test_case.project_config, local_config=test_case.local_config
    )

    connection: dict[str, object] = resolve_project_connection_config(
        discovered_inputs=discovered_inputs, project_dir=tmp_path, require_connection=False
    )

    assert connection == test_case.expected_connection
