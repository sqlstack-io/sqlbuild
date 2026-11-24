"""Shared builders for command-group warehouse resolution tests."""

from __future__ import annotations

import json
from pathlib import Path

from sqlbuild._native import (
    DiscoveredProjectInputs,
    LocalConfig,
    ProjectConfig,
    TargetConfig,
    TargetWarehousesConfig,
)
from tests.unit.src.sqlbuild.cli.commands._helpers.runtime._test_types import (
    CommandWarehouseResolutionTestCase,
)


def build_warehouse_discovered_inputs(
    test_case: CommandWarehouseResolutionTestCase,
) -> DiscoveredProjectInputs:
    """Build a `dev`/`prod` project whose `dev` target uses the case's warehouse groups."""

    return DiscoveredProjectInputs(
        project_config=ProjectConfig(
            name="shop",
            adapter=test_case.adapter,
            default_target="dev",
            connections={"main": dict(test_case.connection)},
            targets={
                "dev": TargetConfig(
                    connection_name="main", warehouses=test_case.project_warehouses
                ),
                "prod": TargetConfig(
                    connection_name="main",
                    warehouses=TargetWarehousesConfig(build="PROD_BUILD_WH"),
                ),
            },
        ),
        local_config=LocalConfig(
            adapter=test_case.local_adapter, targets=dict(test_case.local_targets)
        ),
    )


def write_case_adapter_file(*, project_dir: Path, contents: str) -> None:
    """Write the case's project-local adapter module, which may be empty."""

    (project_dir / "python" / "adapters").mkdir(parents=True, exist_ok=True)
    (project_dir / "python" / "adapters" / "custom.py").write_text(contents, encoding="utf-8")


def write_dbt_profile(profiles_dir: Path, *, profile: str, output: dict[str, object]) -> None:
    """Write a `profiles.yml` whose `profile` has one `dev` output with `output`'s fields."""

    profiles_dir.mkdir(parents=True, exist_ok=True)
    fields: str = "".join(f"      {name}: {json.dumps(value)}\n" for name, value in output.items())
    (profiles_dir / "profiles.yml").write_text(
        f"{profile}:\n  target: dev\n  outputs:\n    dev:\n{fields}", encoding="utf-8"
    )
