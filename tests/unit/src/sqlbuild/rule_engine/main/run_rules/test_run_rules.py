from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    DiscoveredProjectInputs,
    ProjectGraph,
    RulesRunResult,
    build_project_graph,
    discover_project_inputs,
    load_rules_config,
    resolve_adapter,
    run_rules,
)
from sqlbuild.adapter.contract.classes.base_adapter import BaseAdapter
from tests.unit.src.sqlbuild.rule_engine.main.run_rules._test_types import (
    TypeProofRuleSkipTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    (
        TypeProofRuleSkipTestCase(
            description="analysis enabled evaluates type-proof rules",
            no_sql_analysis=False,
            settings_toml="",
            expected_skipped_rules=(),
            expected_finding_codes=("SQBRCONTRACT105",) * 4,
        ),
        TypeProofRuleSkipTestCase(
            description="run without SQL analysis skips type-proof rules",
            no_sql_analysis=True,
            settings_toml="",
            expected_skipped_rules=("SQBRCONTRACT105",),
            expected_finding_codes=(),
        ),
        TypeProofRuleSkipTestCase(
            description="project without SQL analysis skips type-proof rules",
            no_sql_analysis=False,
            settings_toml="\n[settings]\nsql_analysis = false\n",
            expected_skipped_rules=("SQBRCONTRACT105",),
            expected_finding_codes=(),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_sql_analysis_state_when_running_rules_then_type_proof_rules_follow_it(
    test_case: TypeProofRuleSkipTestCase,
    tmp_path: Path,
) -> None:
    (tmp_path / "sqlbuild_project.toml").write_text(
        'name = "orders"\nadapter = "duckdb"\n\n[rules]\nselect = ["SQBRCONTRACT105"]\n'
        + test_case.settings_toml,
        encoding="utf-8",
    )
    models: Path = tmp_path / "sql" / "models"
    models.mkdir(parents=True)
    (models / "orders.sql").write_text(
        "MODEL (description 'Test model orders.',\n"
        "  contract enforced,\n"
        "  columns (order_id (type INTEGER), status (type VARCHAR)),\n"
        ");\n"
        "SELECT order_id, status FROM (SELECT 1 AS order_id, 'placed' AS status) AS raw_orders\n"
        "UNION ALL\n"
        "SELECT 2 AS order_id, 'shipped' AS status\n",
        encoding="utf-8",
    )
    discovered: DiscoveredProjectInputs = discover_project_inputs(project_dir=tmp_path)
    adapter: BaseAdapter = resolve_adapter(adapter_name="duckdb", project_dir=tmp_path)
    graph: ProjectGraph = build_project_graph(
        discovered_inputs=discovered,
        adapter=adapter,
        no_sql_validation=test_case.no_sql_analysis,
    )

    result: RulesRunResult = run_rules(
        graph=graph,
        discovered_inputs=discovered,
        config=load_rules_config(project_dir=tmp_path),
        project_dir=tmp_path,
        dialect="duckdb",
        no_sql_analysis=test_case.no_sql_analysis,
    )

    assert result.skipped_type_proof_rules == test_case.expected_skipped_rules
    assert tuple(finding.code for finding in result.findings) == test_case.expected_finding_codes
