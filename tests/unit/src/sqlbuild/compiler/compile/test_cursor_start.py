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
    CursorSafetyCompileInputsTestCase,
    CursorStartCompileErrorTestCase,
    CursorStartCompileInputsTestCase,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@pytest.mark.parametrize(
    "test_case",
    [
        CursorStartCompileInputsTestCase(
            description="model cursor_start overrides path and project defaults",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n'
                    "[defaults]\ncursor_start = 10\n\n"
                    "[path_defaults.events]\ncursor_start = 20\n"
                ),
                "sql/models/events/orders.sql": (
                    "MODEL (description 'Test model orders.',\nmaterialized incremental\nincremental_strategy delete_insert\n"
                    "cursor id\ncursor_type integer\ncursor_start 30\n);\n\nSELECT 1 AS id"
                ),
            },
            expected_cursor_start=30,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_start_layers_when_building_compile_inputs_then_model_uses_model_precedence(
    test_case: CursorStartCompileInputsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )
    assert (
        test_case.expected_cursor_start
        == compile_inputs.model_inputs[0].config.values["cursor_start"]
    )


@pytest.mark.parametrize(
    "test_case",
    [
        CursorSafetyCompileInputsTestCase(
            description="model safety overrides path and default policy fields",
            repo_files={
                "sqlbuild_project.toml": (
                    'name = "demo"\nadapter = "duckdb"\n\n'
                    '[defaults]\ncursor_start_max_ahead = "7d"\n'
                    'cursor_start_max_action = "error"\n'
                    'cursor_future_max_distance = "14d"\n'
                    'cursor_future_action = "error"\n\n'
                    '[path_defaults.events]\ncursor_start_max_ahead = "3d"\n'
                    'cursor_future_max_distance = "disabled"\n'
                ),
                "sql/models/events/orders.sql": (
                    "MODEL (description 'Test model orders.',\nmaterialized incremental\nincremental_strategy delete_insert\n"
                    "cursor event_at\ncursor_type timestamp\ncursor_grain day\n"
                    "cursor_start_max_ahead '0d'\ncursor_start_max_action cap\n"
                    "cursor_future_max_distance '0d'\ncursor_future_action cap\n"
                    ");\n\nSELECT DATE '2026-01-01' AS event_at"
                ),
            },
            expected_start_max_ahead="0d",
            expected_start_action="cap",
            expected_future_max_distance="0d",
            expected_future_action="cap",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_cursor_safety_layers_when_compiling_then_model_fields_use_model_precedence(
    test_case: CursorSafetyCompileInputsTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    compile_inputs: CompileInputsView = build_compile_inputs(
        discovered_inputs=discovered_inputs,
        adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
    )
    values: dict[str, object] = compile_inputs.model_inputs[0].config.values

    assert values["cursor_start_max_ahead"] == test_case.expected_start_max_ahead
    assert values["cursor_start_max_action"] == test_case.expected_start_action
    assert values["cursor_future_max_distance"] == test_case.expected_future_max_distance
    assert values["cursor_future_action"] == test_case.expected_future_action


@pytest.mark.parametrize(
    "test_case",
    [
        CursorStartCompileErrorTestCase(
            description="cursor_start requires cursor",
            repo_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.',\nmaterialized incremental\nincremental_strategy merge\n"
                    "cursor_type integer\ncursor_start 100\n);\n\nSELECT 1 AS id"
                ),
            },
            expected_error_fragment="cursor_start requires cursor",
        ),
        CursorStartCompileErrorTestCase(
            description="integer cursor_start rejects non whole number",
            repo_files={
                "sqlbuild_project.toml": 'name = "demo"\nadapter = "duckdb"\n',
                "sql/models/orders.sql": (
                    "MODEL (description 'Test model orders.',\nmaterialized incremental\nincremental_strategy merge\ncursor id\n"
                    "cursor_type integer\nunique_key [id]\n"
                    "cursor_start '3.14'\n);\n\nSELECT 1 AS id"
                ),
            },
            expected_error_fragment="cursor_start value '3.14' is not a whole number",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_cursor_start_when_building_compile_inputs_then_raises_clear_error(
    test_case: CursorStartCompileErrorTestCase,
    tmp_path: Path,
    write_repo_files: Callable[[Path, dict[str, str]], None],
) -> None:
    write_repo_files(tmp_path, test_case.repo_files)

    discovered_inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    with pytest.raises(ValueError, match=test_case.expected_error_fragment):
        build_compile_inputs(
            discovered_inputs=discovered_inputs,
            adapter_context=DUCKDB_COMPILE_ADAPTER_CONTEXT,
        )
