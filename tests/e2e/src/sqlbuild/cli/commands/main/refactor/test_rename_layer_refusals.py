"""CLI e2e coverage for the one-step `sqb mv` hint when `sqb rename` crosses layer folders."""

from __future__ import annotations

import shlex
import shutil
import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.refactor._test_types import (
    LayerMovePlaceholderE2ETestCase,
    LayerMoveRefusalE2ETestCase,
    OtherRenameRefusalE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.refactor.helpers import (
    layer_move_hint,
    project_text,
    run_printed_command,
    sqb,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project

_LAYER_RULES: str = 'select = ["SQBRPROJECT102", "SQBRPROJECT105"]'
_PROJECT_TOML: str = (
    'name = "orders"\nadapter = "duckdb"\n\n[connection]\ndatabase = "orders.duckdb"\n\n'
    f"[rules]\n{_LAYER_RULES}\n"
)
_CLEAN_WEB_PATH: str = (
    "sql/models/orders/intermediate/clean/web/orders__int_clean__order_lines__web.sql"
)
_UNLAYERED_WEB_PATH: str = "sql/models/orders/web/order_lines_web.sql"
_MART_PATH: str = "sql/models/orders/mart/orders__mart__order_lines.sql"
_ENRICHED_NAME: str = "orders__int_enriched__order_lines__web"
_STAGING: str = (
    'MODEL (\n  description "Staged order lines",\n  materialized table,\n);\n\n'
    "SELECT 1 AS order_line_id, 10 AS customer_id\n"
)
_WEB_LINES: str = (
    'MODEL (\n  description "Web order lines",\n  materialized table,\n);\n\n'
    'SELECT order_line_id, customer_id\nFROM __ref("orders__stg__order_lines")\n'
)
_MART: str = (
    'MODEL (\n  description "Order lines",\n  materialized table,\n);\n\n'
    'SELECT order_line_id, customer_id\nFROM __ref("{upstream}")\n'
)
_LAYERED_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_TOML,
    "sql/models/orders/staging/orders__stg__order_lines.sql": _STAGING,
    _CLEAN_WEB_PATH: _WEB_LINES,
    _MART_PATH: _MART.format(upstream="orders__int_clean__order_lines__web"),
}
_UNLAYERED_FILES: dict[str, str] = {
    "sqlbuild_project.toml": _PROJECT_TOML,
    "sql/models/orders/staging/orders__stg__order_lines.sql": _STAGING,
    _UNLAYERED_WEB_PATH: _WEB_LINES,
    _MART_PATH: _MART.format(upstream="order_lines_web"),
}


@pytest.mark.parametrize(
    "test_case",
    [
        LayerMoveRefusalE2ETestCase(
            description="int_clean to int_enriched mirrors the web sub-folder",
            project_files=_LAYERED_FILES,
            old_name="orders__int_clean__order_lines__web",
            new_name=_ENRICHED_NAME,
            expected_hint=(
                f"{_ENRICHED_NAME} belongs under sql/models/orders/intermediate/enriched/",
                "  Rename and move in one step:",
                "    sqb mv orders__int_clean__order_lines__web "
                f"sql/models/orders/intermediate/enriched/web/{_ENRICHED_NAME}.sql "
                "--project-dir {project_dir}",
                "  (change the folder if you want it somewhere else)",
            ),
            expected_moved_file=f"sql/models/orders/intermediate/enriched/web/{_ENRICHED_NAME}.sql",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_into_another_layer_when_refused_then_printed_mv_renames_and_moves(
    test_case: LayerMoveRefusalE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.project_files
    )
    intended_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="intended",
        repo_files={
            **test_case.project_files,
            "sqlbuild_project.toml": _PROJECT_TOML.replace(_LAYER_RULES, "select = []"),
        },
    )
    before: dict[str, str] = project_text(project_dir)

    refused: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", test_case.old_name, test_case.new_name
    )
    after_refusal: dict[str, str] = project_text(project_dir)
    hint: tuple[str, ...] = layer_move_hint(refused.stdout)
    moved: subprocess.CompletedProcess[str] = run_printed_command(hint[2].strip())
    intended_rename: subprocess.CompletedProcess[str] = sqb(
        intended_dir, "rename", test_case.old_name, test_case.new_name
    )
    (intended_dir / test_case.expected_moved_file).parent.mkdir(parents=True)
    _ = shutil.move(
        intended_dir / Path(_CLEAN_WEB_PATH).with_name(f"{test_case.new_name}.sql"),
        intended_dir / test_case.expected_moved_file,
    )
    compiled: subprocess.CompletedProcess[str] = sqb(project_dir, "compile")

    assert (
        refused.returncode,
        after_refusal == before,
        hint,
        moved.returncode,
        intended_rename.returncode,
        compiled.returncode,
    ) == (
        1,
        True,
        tuple(
            line.replace("{project_dir}", shlex.quote(str(project_dir)))
            for line in test_case.expected_hint
        ),
        0,
        0,
        0,
    ), (refused.stdout, moved.stdout, moved.stderr, compiled.stdout)
    assert {**project_text(project_dir), "sqlbuild_project.toml": ""} == {
        **project_text(intended_dir),
        "sqlbuild_project.toml": "",
    }


@pytest.mark.parametrize(
    "test_case",
    [
        LayerMovePlaceholderE2ETestCase(
            description="no current layer folder to mirror",
            project_files=_UNLAYERED_FILES,
            old_name="order_lines_web",
            new_name=_ENRICHED_NAME,
            expected_hint=(
                f"{_ENRICHED_NAME} belongs under a layer folder named intermediate/enriched/",
                "  Rename and move in one step:",
                f"    sqb mv order_lines_web 'sql/models/<folder>/{_ENRICHED_NAME}.sql' "
                "--project-dir {project_dir}",
                "  (replace <folder>/ with the folder the model should live in)",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_into_another_layer_without_a_layer_folder_when_refused_then_hint_has_placeholder(
    test_case: LayerMovePlaceholderE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.project_files
    )
    before: dict[str, str] = project_text(project_dir)

    refused: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", test_case.old_name, test_case.new_name
    )

    assert (
        refused.returncode,
        project_text(project_dir) == before,
        layer_move_hint(refused.stdout),
    ) == (
        1,
        True,
        tuple(
            line.replace("{project_dir}", shlex.quote(str(project_dir)))
            for line in test_case.expected_hint
        ),
    ), refused.stdout


@pytest.mark.parametrize(
    "test_case",
    [
        OtherRenameRefusalE2ETestCase(
            description="a name outside the layer grammar fails another rule",
            project_files={
                **_LAYERED_FILES,
                "sqlbuild_project.toml": _PROJECT_TOML.replace(
                    _LAYER_RULES,
                    'select = ["SQBRPROJECT102", "SQBRPROJECT104", "SQBRPROJECT105"]',
                ),
            },
            old_name="orders__int_clean__order_lines__web",
            new_name="order_lines_web",
            expected_codes=("SQBRPROJECT104",),
        ),
        OtherRenameRefusalE2ETestCase(
            description="a layer move that also fails another rule",
            project_files={
                **_LAYERED_FILES,
                "sqlbuild_project.toml": _PROJECT_TOML.replace(
                    _LAYER_RULES,
                    'select = ["SQBRPROJECT102", "SQBRPROJECT105", "SQBRMODEL103"]',
                ),
            },
            old_name="orders__int_clean__order_lines__web",
            new_name="orders__int_v__order_lines__web",
            expected_codes=("SQBRMODEL103", "SQBRPROJECT105"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_rename_refused_for_another_reason_when_renaming_then_no_mv_hint_is_printed(
    test_case: OtherRenameRefusalE2ETestCase, tmp_path: Path
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path, project_name="orders", repo_files=test_case.project_files
    )

    refused: subprocess.CompletedProcess[str] = sqb(
        project_dir, "rename", test_case.old_name, test_case.new_name
    )

    assert (
        refused.returncode,
        layer_move_hint(refused.stdout),
        tuple(f"── {code}  " in refused.stdout for code in test_case.expected_codes),
        refused.stdout.rstrip().endswith("Compiled: failed; no files changed"),
    ) == (1, (), (True,) * len(test_case.expected_codes), True), refused.stdout


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
