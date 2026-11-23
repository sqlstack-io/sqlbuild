"""Loading and validation of per-target command-group warehouses."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    ProjectConfigError,
    TargetWarehousesConfig,
    discover_project_inputs,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    TargetWarehousesConfigErrorTestCase,
    TargetWarehousesConfigTestCase,
    TargetWarehousesDiscoveryTestCase,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers.helpers import (
    load_local_config,
    load_project_config,
)

_SNOWFLAKE_PROJECT: str = """
name = "shop"
adapter = "snowflake"

[connections.main]
warehouse = "ANALYTICS_WH"

[targets.dev]
connection = "main"
schema = "DEV"
""".strip()
_SHARED_GROUPS: str = '\n\n[targets.dev.warehouses]\nbuild = "BUILD_WH"\nquery = "ADHOC_WH"\n'
_LOCAL_DUCKDB_OVERRIDE: str = (
    'adapter = "duckdb"\n\n[connections.local]\ndatabase = "dev.duckdb"\n\n'
    '[targets.dev]\nconnection = "local"\n'
)
_SNOWFLAKE_SUBCLASS_ADAPTER: str = """
from sqlbuild.adapters.snowflake.classes.snowflake_adapter import SnowflakeAdapter


class SnowflakePlusAdapter(SnowflakeAdapter):
    adapter_name = "snowflake_plus"
""".lstrip()
_DUCKDB_SUBCLASS_ADAPTER: str = """
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter


class DuckDbPlusAdapter(DuckDbAdapter):
    adapter_name = "duckdb_plus"
""".lstrip()


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWarehousesConfigTestCase(
            description="project target sets both groups",
            contents=_SNOWFLAKE_PROJECT
            + '\n\n[targets.dev.warehouses]\nbuild = "BUILD_WH"\nquery = "ADHOC_WH"\n',
            expected_warehouses=TargetWarehousesConfig(build="BUILD_WH", query="ADHOC_WH"),
        ),
        TargetWarehousesConfigTestCase(
            description="project target sets a sync warehouse for its location",
            contents=_SNOWFLAKE_PROJECT + '\n\n[targets.dev.warehouses]\nsync = "SYNC_WH"\n',
            expected_warehouses=TargetWarehousesConfig(sync="SYNC_WH"),
        ),
        TargetWarehousesConfigTestCase(
            description="target without warehouses keeps both groups unset",
            contents=_SNOWFLAKE_PROJECT,
            expected_warehouses=TargetWarehousesConfig(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_target_warehouses_when_loading_then_groups_are_typed(
    test_case: TargetWarehousesConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.contents, encoding="utf-8")

    config_warehouses: TargetWarehousesConfig = (
        load_project_config(project_dir=tmp_path).targets["dev"].warehouses
    )

    assert config_warehouses == test_case.expected_warehouses


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWarehousesConfigTestCase(
            description="local target sets only the query group",
            contents='[targets.dev.warehouses]\nquery = "ADHOC_WH"\n',
            expected_warehouses=TargetWarehousesConfig(query="ADHOC_WH"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_local_target_warehouses_when_loading_then_groups_are_typed(
    test_case: TargetWarehousesConfigTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_local.toml").write_text(test_case.contents, encoding="utf-8")

    config_warehouses: TargetWarehousesConfig = (
        load_local_config(project_dir=tmp_path).targets["dev"].warehouses
    )

    assert config_warehouses == test_case.expected_warehouses


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWarehousesConfigErrorTestCase(
            description="unknown group key is rejected with a suggestion",
            project_contents=_SNOWFLAKE_PROJECT
            + '\n\n[targets.dev.warehouses]\nbiuld = "BUILD_WH"\n',
            expected_error_fragments=(
                "targets.dev.warehouses has unknown keys: biuld; allowed keys: build, query",
            ),
            expected_help_fragments=("did you mean 'build'?",),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="unknown group key in the local file is rejected",
            project_contents=_SNOWFLAKE_PROJECT,
            local_contents='[targets.dev.warehouses]\nadhoc = "ADHOC_WH"\n',
            expected_error_fragments=("targets.dev.warehouses has unknown keys: adhoc",),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="blank warehouse name is rejected",
            project_contents=_SNOWFLAKE_PROJECT + '\n\n[targets.dev.warehouses]\nquery = "  "\n',
            expected_error_fragments=(
                "targets.dev.warehouses.query must be a non-empty warehouse name",
            ),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="adapter without warehouses rejects project groups",
            project_contents="""
name = "shop"
adapter = "duckdb"

[targets.dev]
schema = "dev"

[targets.dev.warehouses]
build = "BUILD_WH"
""".strip(),
            expected_error_fragments=(
                "[targets.dev.warehouses] selects a build warehouse, but the 'duckdb' adapter "
                "has no warehouse to select",
                'sqlbuild_project.toml sets [targets.dev.warehouses] build = "BUILD_WH"',
            ),
            expected_help_fragments=(
                "remove the [targets.dev.warehouses] section from sqlbuild_project.toml",
                "need an adapter with a session warehouse",
            ),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="adapter without warehouses rejects local groups",
            project_contents=(
                'name = "shop"\nadapter = "postgres"\n\n[targets.dev]\nschema = "dev"\n'
            ),
            local_contents='[targets.dev.warehouses]\nquery = "ADHOC_WH"\n',
            expected_error_fragments=(
                "the 'postgres' adapter has no warehouse to select",
                'sqlbuild_local.toml sets [targets.dev.warehouses] query = "ADHOC_WH"',
            ),
            expected_help_fragments=(
                "remove the [targets.dev.warehouses] section from sqlbuild_local.toml",
            ),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="local adapter override rejects local groups",
            project_contents=_SNOWFLAKE_PROJECT,
            local_contents=_LOCAL_DUCKDB_OVERRIDE
            + '\n[targets.dev.warehouses]\nquery = "ALICE_WH"\n',
            expected_error_fragments=(
                "the 'duckdb' adapter has no warehouse to select",
                'sqlbuild_local.toml sets [targets.dev.warehouses] query = "ALICE_WH"',
            ),
            expected_help_fragments=(
                "remove the [targets.dev.warehouses] section from sqlbuild_local.toml",
            ),
        ),
        TargetWarehousesConfigErrorTestCase(
            description="custom adapter extending duckdb rejects shared groups",
            project_contents=_SNOWFLAKE_PROJECT.replace('"snowflake"', '"duckdb_plus"')
            + _SHARED_GROUPS,
            adapter_file_contents=_DUCKDB_SUBCLASS_ADAPTER,
            expected_error_fragments=("the 'duckdb_plus' adapter has no warehouse to select",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_target_warehouses_when_discovering_then_config_error_explains_fix(
    test_case: TargetWarehousesConfigErrorTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.project_contents, encoding="utf-8")
    (tmp_path / "sqlbuild_local.toml").write_text(test_case.local_contents, encoding="utf-8")
    (tmp_path / "python" / "nodes" / "python" / "adapters").mkdir(parents=True)
    (tmp_path / "python" / "nodes" / "python" / "adapters" / "custom.py").write_text(
        test_case.adapter_file_contents, encoding="utf-8"
    )

    with pytest.raises(ProjectConfigError) as error:
        discover_project_inputs(project_dir=tmp_path)

    assert all(fragment in str(error.value) for fragment in test_case.expected_error_fragments)
    assert all(
        fragment in (error.value.help or "") for fragment in test_case.expected_help_fragments
    )


@pytest.mark.parametrize(
    "test_case",
    [
        TargetWarehousesDiscoveryTestCase(
            description="local duckdb override leaves shared groups unused",
            project_contents=_SNOWFLAKE_PROJECT + _SHARED_GROUPS,
            local_contents=_LOCAL_DUCKDB_OVERRIDE,
            expected_build="BUILD_WH",
        ),
        TargetWarehousesDiscoveryTestCase(
            description="custom adapter extending snowflake accepts shared groups",
            project_contents=_SNOWFLAKE_PROJECT.replace('"snowflake"', '"snowflake_plus"')
            + _SHARED_GROUPS,
            adapter_file_contents=_SNOWFLAKE_SUBCLASS_ADAPTER,
            expected_build="BUILD_WH",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_adapter_layers_when_discovering_target_warehouses_then_config_is_accepted(
    test_case: TargetWarehousesDiscoveryTestCase, tmp_path: Path
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(test_case.project_contents, encoding="utf-8")
    (tmp_path / "sqlbuild_local.toml").write_text(test_case.local_contents, encoding="utf-8")
    (tmp_path / "python" / "nodes" / "python" / "adapters").mkdir(parents=True)
    (tmp_path / "python" / "nodes" / "python" / "adapters" / "custom.py").write_text(
        test_case.adapter_file_contents, encoding="utf-8"
    )

    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    assert discovered.project_config.targets["dev"].warehouses.build == test_case.expected_build


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
