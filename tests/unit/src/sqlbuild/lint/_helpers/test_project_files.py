"""Unit tests for collecting authored SQL files for lint runs."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    LintConfig,
    LintRunResult,
    collect_lint_project_files,
    discover_project_inputs,
    run_lint,
)
from tests.unit.src.sqlbuild.lint._helpers._test_types import (
    CollectProjectFilesTestCase,
    SelectedProjectFilesTestCase,
)
from tests.unit.src.sqlbuild.lint._helpers.helpers import (
    files_resolving_to,
    write_linked_orders_project,
    write_orders_lint_project,
)


@pytest.mark.parametrize(
    "test_case",
    [
        CollectProjectFilesTestCase(
            description="identical text shares the discovered string",
            model_bytes=b"MODEL (materialized table);\nSELECT 1 AS order_id\n",
            expected_contents="MODEL (materialized table);\nSELECT 1 AS order_id\n",
        ),
        CollectProjectFilesTestCase(
            description="carriage returns keep the exact authored text",
            model_bytes=b"MODEL (materialized table);\r\nSELECT 1 AS order_id\r\n",
            expected_contents="MODEL (materialized table);\r\nSELECT 1 AS order_id\r\n",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_project_when_collecting_files_then_returns_exact_authored_text(
    test_case: CollectProjectFilesTestCase, tmp_path: Path
) -> None:
    model_path: Path = write_orders_lint_project(
        project_dir=tmp_path, model_bytes=test_case.model_bytes
    )
    inputs: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)

    files: dict[Path, str] = collect_lint_project_files(project_dir=tmp_path)
    shared: LintRunResult = run_lint(
        project_dir=tmp_path, config=LintConfig(), discovered_inputs=inputs
    )
    fresh: LintRunResult = run_lint(project_dir=tmp_path, config=LintConfig())

    assert files[model_path] == test_case.expected_contents
    assert shared.source_texts == fresh.source_texts
    assert shared.violations == fresh.violations
    assert shared.files_checked == fresh.files_checked == 2


@pytest.mark.parametrize(
    "test_case",
    [
        SelectedProjectFilesTestCase(
            description="regular_file",
            selected="sql/models/orders/orders.sql",
            expected_present=("sql/models/orders/orders.sql",),
        ),
        SelectedProjectFilesTestCase(
            description="file_reached_through_links",
            selected="shared/customers.sql",
            expected_present=("sql/models/orders/customers_link.sql",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_selected_resolved_paths_when_collecting_files_then_matches_resolving_each_file(
    test_case: SelectedProjectFilesTestCase, tmp_path: Path
) -> None:
    write_linked_orders_project(project_dir=tmp_path)
    selected: frozenset[Path] = frozenset({(tmp_path / test_case.selected).resolve()})
    every_file: dict[Path, str] = collect_lint_project_files(project_dir=tmp_path)

    collected: dict[Path, str] = collect_lint_project_files(
        project_dir=tmp_path, selected_paths=selected
    )
    from_sources: dict[Path, str] = dict(
        run_lint(
            project_dir=tmp_path,
            config=LintConfig(native_enabled=False),
            selected_paths=selected,
            source_files=every_file,
        ).source_texts
    )

    relative: set[str] = {path.relative_to(tmp_path).as_posix() for path in collected}
    assert relative >= set(test_case.expected_present)
    assert from_sources == collected
    assert collected == files_resolving_to(files=every_file, selected=selected)


if __name__ == "__main__":
    raise SystemExit(pytest.main([__file__, "-n", "auto", "--dist", "loadfile"]))
