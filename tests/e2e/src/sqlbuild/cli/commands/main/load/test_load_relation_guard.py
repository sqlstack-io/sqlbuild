"""Real-CLI coverage of the run-time hard-coded relation-name guard under sqb load."""

from __future__ import annotations

import json
import subprocess
from itertools import chain
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.load._test_types import (
    LoaderRelationGuardE2ETestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb
from tests.unit.src.sqlbuild.python_nodes.helpers import node_module_files

_SOURCES: str = (
    "sources:\n"
    "  - name: raw_regions\n    description: Test source raw_regions.\n    managed: true\n"
    "  - name: raw_customers\n    description: Test source raw_customers.\n    managed: true\n"
)
_LOADERS: str = (
    "from sqlbuild.loaders import loader\n\n\n"
    "@loader(write_strategy='replace')\n"
    "def raw_regions(ctx):\n"
    "    '''Test loader raw_regions.'''\n    return [{'id': 1}]\n\n\n"
    "@loader(write_strategy='replace', depends_on=[raw_regions])\n"
    "def raw_customers(ctx):\n"
    "    '''Test loader raw_customers.'''\n    table = 'raw_' + 'regions'\n"
    "    ctx.query(f'SELECT count(*) FROM {table}')\n"
    "    return [{'id': 1}]\n"
)
_HARD_CODED_WARNING: str = (
    "[P008] loader 'raw_customers' named source:raw_regions as 'raw_regions' in SQL; "
    "use ctx.loader(raw_regions) instead of the relation name"
)


@pytest.mark.parametrize(
    "test_case",
    [
        LoaderRelationGuardE2ETestCase(
            description="dynamic SQL naming a source warns while the load succeeds",
            enforce_explicit=True,
            expected_warnings=(_HARD_CODED_WARNING,),
            expected_final_line_prefix="\u2713 Completed with warnings  PASS=1  WARN=1  FAIL=0",
        ),
        LoaderRelationGuardE2ETestCase(
            description="enforcement disabled loads without warnings",
            enforce_explicit=False,
            expected_warnings=(),
            expected_final_line_prefix="\u2713 Completed successfully  PASS=2  WARN=0  FAIL=0",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_loader_hard_codes_a_source_when_loading_then_it_warns_like_build(
    test_case: LoaderRelationGuardE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="guarded_loads",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "guarded_loads"\nadapter = "duckdb"\n\n'
                '[connection]\ndatabase = "warehouse.duckdb"\n\n'
                f"[references]\nenforce_explicit = {str(test_case.enforce_explicit).lower()}\n"
            ),
            "sql/sources/raw.yml": _SOURCES,
            **node_module_files("python/nodes/loaders/raw.py", _LOADERS),
        },
    )

    text: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "load"), project_dir=project_dir
    )
    machine: subprocess.CompletedProcess[str] = run_sqb(
        command=("load", "--json"), project_dir=project_dir
    )

    assert text.returncode == 0, text.stdout + text.stderr
    assert machine.returncode == 0, machine.stdout + machine.stderr
    assert all(warning in text.stdout for warning in test_case.expected_warnings)
    assert ("Warnings:" in text.stdout) is bool(test_case.expected_warnings)
    assert text.stdout.strip().splitlines()[-1].startswith(test_case.expected_final_line_prefix)
    assert all(warning in machine.stderr for warning in test_case.expected_warnings)
    stored: list[str] = list(
        chain.from_iterable(
            asset.get("warnings") or () for asset in json.loads(machine.stdout)["assets"]
        )
    )
    assert len(stored) == len(test_case.expected_warnings)
    assert all(
        warning in stored_warning
        for stored_warning, warning in zip(stored, test_case.expected_warnings, strict=True)
    )
