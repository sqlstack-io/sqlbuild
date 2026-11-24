from __future__ import annotations

from collections.abc import Callable
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import DiscoveredProjectInputs, build_compile_inputs, discover_project_inputs
from tests.unit.src.sqlbuild.compiler.compile._helpers.helpers import (
    DUCKDB_COMPILE_ADAPTER_CONTEXT,
)
from tests.unit.src.sqlbuild.compiler.compile._test_types import (
    ResolvedConnectionCompileInputsTestCase,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView

_PROJECT_TOML: str = (
    'name = "demo"\nadapter = "duckdb"\n\n[connection]\ndatabase = "warehouse.duckdb"\n'
)


@pytest.mark.parametrize(
    "test_case",
    [
        ResolvedConnectionCompileInputsTestCase(
            description="resolved connection override becomes the effective connection",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id",
            },
            resolved_connection={"database": "/abs/warehouse.duckdb", "schema": "analytics"},
            expected_effective_connection={
                "database": "/abs/warehouse.duckdb",
                "schema": "analytics",
            },
        ),
        ResolvedConnectionCompileInputsTestCase(
            description="without override the merged project connection is used",
            repo_files={
                "sqlbuild_project.toml": _PROJECT_TOML,
                "sql/models/orders.sql": "MODEL (description 'Test model orders.', materialized table);\n\nSELECT 1 AS order_id",
            },
            resolved_connection=None,
            expected_effective_connection={"database": "warehouse.duckdb"},
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_resolved_connection_when_building_compile_inputs_then_uses_expected_connection(
    test_case: ResolvedConnectionCompileInputsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        resolved_connection=test_case.resolved_connection,
    )

    assert compile_inputs.effective_connection == test_case.expected_effective_connection
