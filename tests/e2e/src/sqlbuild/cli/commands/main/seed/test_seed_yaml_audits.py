"""The seed YAML audit example in the seeds documentation builds as documented."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.seed._test_types import SeedYamlAuditE2ETestCase
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import prepare_inline_project, run_sqb

_SEED_YAML: str = """\
seeds:
  - name: waffle_types
    description: Waffle types
    columns:
      - name: waffle_type_id
        type: INTEGER
        audits:
          - not_null
          - unique
      - name: price_cents
        type: INTEGER
    audits:
      - expression_is_true:
          expression: "price_cents >= 0"
          severity: error
"""
_RELATION_AUDIT_SQL: str = "AUDIT ();\n\nSELECT *\nFROM @relation\nWHERE NOT (@expression)\n"


@pytest.mark.parametrize(
    "test_case",
    [
        SeedYamlAuditE2ETestCase(
            description="documented seed audits pass on valid rows",
            audit_sql=_RELATION_AUDIT_SQL,
            seed_csv="waffle_type_id,price_cents\n1,450\n2,525\n",
            expected_exit_code=0,
            expected_output_fragments=(
                "expression_is_true (waffle_types)",
                "PASS=4",
                "FAIL=0",
            ),
        ),
        SeedYamlAuditE2ETestCase(
            description="documented seed expression audit fails a negative price",
            audit_sql=_RELATION_AUDIT_SQL,
            seed_csv="waffle_type_id,price_cents\n1,450\n2,-1\n",
            expected_exit_code=1,
            expected_output_fragments=(
                "expression_is_true (waffle_types)",
                "waffle_types  (seed)",
                "FAIL=2",
            ),
        ),
        SeedYamlAuditE2ETestCase(
            description="model-only audit on a seed points at @relation",
            audit_sql='AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE NOT (@expression)\n',
            seed_csv="waffle_type_id,price_cents\n1,450\n",
            expected_exit_code=1,
            expected_output_fragments=(
                "seeds/waffle_types.yml is missing argument 'model' for generic audit "
                "'expression_is_true'; a seed audit receives 'seed' and 'relation', not 'model', "
                "so read the attached relation with @relation",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_documented_seed_yaml_audits_when_building_then_audits_run_on_the_seed(
    test_case: SeedYamlAuditE2ETestCase,
    tmp_path: Path,
) -> None:
    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="waffle_seed_audits",
        repo_files={
            "sqlbuild_project.toml": (
                'name = "waffle_seed_audits"\nadapter = "duckdb"\n'
                '[connection]\ndatabase = "waffles.duckdb"\n'
            ),
            "seeds/waffle_types.csv": test_case.seed_csv,
            "seeds/waffle_types.yml": _SEED_YAML,
            "audits/generic/expression_is_true.sql": test_case.audit_sql,
        },
    )

    built: subprocess.CompletedProcess[str] = run_sqb(command=("build",), project_dir=project_dir)

    output: str = built.stdout + built.stderr
    assert built.returncode == test_case.expected_exit_code, (test_case.description, output)
    assert all(fragment in output for fragment in test_case.expected_output_fragments), (
        test_case.description,
        output,
    )
